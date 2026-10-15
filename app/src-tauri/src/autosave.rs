// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Auto-save, in the app (charter-app#296, ADR 0051): one worker per held plane, beside its
//! file watcher, started when the plane is held and stopped when it is let go of.
//!
//! There is no daemon (ADR 0025): a plane is saved by itself only while the app holds it. What
//! decides is the core's ([`purlis_core::autosave`]); this module is the clock and the loop.
//!
//! On each look the worker:
//! - saves once the plane has been quiet for `[plane] autosave_after` ([`autosave::Quiet`]);
//! - saves at once after a chat in the plane ends ([`Poke::SessionEnded`]);
//! - on its first look, pushes what the last run left committed and unpushed (the launch);
//! - every [`FETCH_EVERY`], and when the window asks ([`Poke::Fetch`]), fetches the target
//!   branch — and fast-forwards a clean tree onto it only while auto-save is on;
//! - looks again at the open pull request of a `pr-merge` project, and of each `pr-merge`
//!   repo, so one whose checks passed is merged without the person saving again, and one that
//!   went red says so (ADR 0051, amended 2026-10-13). Paced by [`autosave::Looks`]: first once
//!   the pushed commit's checks could have started, then backing off to every five minutes,
//!   for two hours; never a busy poll, and whether auto-save is on or not, because the merge
//!   is what the save that opened the request was asked for;
//! - and after any of these that did something, tells the window what moved: the git standing,
//!   or everything when it moved the tree ([`what_it_did`]).
//!
//! Saving at quit is not the worker's: the app is exiting, so [`at_quit`] runs it bounded,
//! for every held plane at once.
//!
//! **Where the worker runs is where the project's chats run** ([`Saver`]): in the window's own
//! process, or, behind the start-time switch, in the session host (`crate::hostsaving`, #2002),
//! which runs this same worker and tells the window what each look did. Exactly one runs for a
//! project at any time: the window starts none of its own while its chats run in the host.
//!
//! **Workspace repos too, but only those that ask** (charter-app#299): a repo whose
//! `[repos.<name>] autosave` is on is saved the same three ways, after its own quiet period,
//! when a chat ends and at quit. Its save waits while any chat in its workspace is mid-turn
//! ([`MidTurn`]): that cycle is skipped, and its quiet period starts again once the turn ends.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use purlis_core::autosave::{self, Decision, Quiet};
use purlis_core::planegit::{self, Stage, Trigger};
use purlis_core::planesave;
use purlis_core::reposave;

/// How often the worker looks at its plane: one `git status`.
const LOOK_EVERY: Duration = Duration::from_secs(2);
/// How often it fetches the target branch while nothing asks sooner.
pub const FETCH_EVERY: Duration = Duration::from_secs(300);
/// The least time between two fetches, however often the window asks.
const FETCH_AT_MOST_EVERY: Duration = Duration::from_secs(60);
/// How long quitting gives the pushes.
pub const QUIT_BOUND: Duration = Duration::from_secs(5);
/// How often the worker reads which `pr-merge` repos have a pull request open: a directory
/// listing per workspace, so not at every look.
const REPO_REQUESTS_EVERY: Duration = Duration::from_secs(10);

/// What the worker can be told between looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Poke {
    /// A chat in the plane ended: save now, if there is anything to save.
    SessionEnded,
    /// The window came back into focus: fetch, unless the last fetch was a moment ago.
    Fetch,
}

/// The chats working in a workspace that are mid-turn, by name — [`crate::planes::Held::mid_turn_in`].
pub type MidTurn = Arc<dyn Fn(&str) -> Vec<String> + Send + Sync>;

/// Where the worker asks [`MidTurn`], once the plane it belongs to is built — the same slot
/// shape as the handoff answer's, and for the same reason: the plane holds the worker, so the
/// worker can only hold the plane weakly, and only after it exists.
type MidTurnSlot = Arc<std::sync::Mutex<Option<MidTurn>>>;

/// A plane's auto-save worker. Dropping it stops the loop at its next look: the flag is
/// what stops it, because a chat's end still holds a sender, and a channel alone would keep a
/// let-go plane's worker looking until the last of those was gone.
pub struct Worker {
    poke: Sender<Poke>,
    stop: Arc<AtomicBool>,
    mid_turn: MidTurnSlot,
    /// Never joined by a drop: a worker mid-save is left to finish rather than holding a
    /// plane's close open. [`Worker::stop_within`] waits for it, bounded.
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake it now rather than at its next look; a worker mid-save finishes that save.
        let _ = self.poke.send(Poke::Fetch);
    }
}

/// Told when auto-save saved the plane at a root, so the extensions that hear a plane being
/// saved are told as they are after the Save button (charter-app#343).
pub type Saved = Arc<dyn Fn(crate::planes::PlaneId, PathBuf) + Send + Sync + 'static>;

/// What one look that did something did: the window reads the git standing again, or
/// everything when the tree moved ([`what_it_did`]), and hears that the plane was saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Looked {
    /// It saved the plane.
    pub saved: bool,
    /// What it did moved files in the tree: a fast-forward, a save's rebase.
    pub moved_tree: bool,
}

/// Told what each look that did something did.
pub type Told = Arc<dyn Fn(Looked) + Send + Sync + 'static>;

/// A project's auto-save, wherever it runs ([`Worker`] in this process, or the session host's).
/// Dropping it stops it.
pub trait Saver: Send + Sync {
    /// Something for it to act on at its next look.
    fn poker(&self) -> Sender<Poke>;

    /// Who it asks which chats are mid-turn in a workspace, where it asks the window.
    fn asks_mid_turn_of(&self, ask: MidTurn);

    /// Stops it as the app quits, by `deadline`: whether whoever saves the project said it had
    /// stopped by then (#2002). The window's own worker is dropped, as it always was: the save
    /// at quit waits on its claim.
    fn stop_by(self: Box<Self>, deadline: Instant) -> bool {
        let _ = deadline;
        true
    }
}

impl Saver for Worker {
    fn poker(&self) -> Sender<Poke> {
        self.poke.clone()
    }

    fn asks_mid_turn_of(&self, ask: MidTurn) {
        *self
            .mid_turn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ask);
    }
}

impl Worker {
    /// Start looking at the plane at `root`. `told` hears what each look that did something
    /// did.
    pub fn start(root: PathBuf, told: Told) -> Self {
        let (poke, poked) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let mid_turn: MidTurnSlot = Arc::default();
        let asking = Arc::clone(&mid_turn);
        let spawned = std::thread::Builder::new()
            .name("charter-autosave".into())
            .spawn(move || {
                #[cfg(test)]
                let _counted = running::Counted::on(&root);
                let mid_turn = |workspace: &str| {
                    let asked = asking
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    asked.map(|ask| ask(workspace)).unwrap_or_default()
                };
                // Every save and fetch here is the window's own git (#752).
                crate::forgeaccounts::window_git(|| {
                    run(&root, &poked, &stopping, &mid_turn, &*told);
                });
            });
        let thread = spawned
            .map_err(|why| tracing::warn!("purlis: auto-save did not start ({why}); save by hand"))
            .ok();
        Self {
            poke,
            stop,
            mid_turn,
            thread,
        }
    }
}

/// The worker for the plane at `root`, in this process: [`crate::host::SessionHost`]'s own,
/// where the window runs its chats. Until [`Saver::asks_mid_turn_of`] is said, it takes no
/// chat to be mid-turn, which is true: no chat of the plane has started before the plane is
/// built.
pub fn here(root: PathBuf, told: Told) -> Box<dyn Saver> {
    Box::new(Worker::start(root, told))
}

/// What a look that did something tells the window moved (#933).
///
/// **The git standing alone, when the tree did not move:** a commit takes what is already on
/// disk, a push and a fetch that brought nothing in move only refs, and only the git readers
/// read again.
///
/// **Everything, when it moved the tree** (`moved`): a fetch that fast-forwarded, a save that
/// rebased onto the remote. The watch would tell the files it moved only where it watches —
/// it is not recursive, and a project it could not watch has none — so not knowing is said.
pub(crate) fn what_it_did(moved: bool) -> crate::planewatch::What {
    (!moved).then(|| vec![purlis_core::planechange::saved()])
}

/// `HEAD` of the repository at `dir`, or `None` where it cannot be read.
fn head_of(dir: &Path) -> Option<String> {
    let read = purlis_core::worktree::git::run(
        dir,
        &["rev-parse", "--verify", "-q", "HEAD"],
        purlis_core::worktree::git::READ,
    )
    .ok()?;
    read.ok().then(|| read.line().trim().to_owned())
}

/// Whether a save moved the tree at `dir` beyond the commit it made of it: `HEAD` is neither
/// where it was (`before`) nor one commit on top of it, which is what a rebase onto commits
/// from the remote leaves. A head that cannot be read is not known, and so is told as moved.
fn tree_moved(dir: &Path, before: Option<&str>) -> bool {
    let (Some(before), Some(after)) = (before, head_of(dir)) else {
        return true;
    };
    if after == before {
        return false;
    }
    let parent = purlis_core::worktree::git::run(
        dir,
        &["rev-parse", "--verify", "-q", "HEAD^"],
        purlis_core::worktree::git::READ,
    )
    .ok()
    .filter(|read| read.ok())
    .map(|read| read.line().trim().to_owned());
    parent.as_deref() != Some(before)
}

/// The loop: a look every [`LOOK_EVERY`] or at a poke, until the worker is dropped.
fn run(
    root: &Path,
    poked: &Receiver<Poke>,
    stop: &AtomicBool,
    mid_turn: &dyn Fn(&str) -> Vec<String>,
    told: &dyn Fn(Looked),
) {
    let mut state = State::default();
    let mut poke = None;
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let now = Instant::now();
        let plane = state.look(root, now, poke);
        let repos = state.look_at_repos(root, now, poke, mid_turn);
        let repos = state.look_at_repo_requests(root, now, poke) || repos;
        if plane || repos {
            told(Looked {
                saved: std::mem::take(&mut state.saved),
                moved_tree: std::mem::take(&mut state.moved_tree),
            });
        }
        poke = match poked.recv_timeout(LOOK_EVERY) {
            Ok(poke) => Some(poke),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
    }
}

/// What the worker remembers between looks.
#[derive(Debug, Default)]
struct State {
    quiet: Quiet,
    launched: bool,
    fetched: Option<Instant>,
    /// The last look saved the plane, and nobody has been told yet.
    saved: bool,
    /// A look since the window was last told moved the tree under the plane or a repo — a
    /// fast-forward, a save's rebase — and the window is told everything ([`what_it_did`]).
    moved_tree: bool,
    /// Each auto-saved repo's quiet period, by its clone's path.
    repos: HashMap<PathBuf, Quiet>,
    repos_launched: bool,
    /// When the plane's open pull request is looked at again.
    looks: autosave::Looks,
    /// The same for each `pr-merge` repo with one open, by its clone's path.
    repo_looks: HashMap<PathBuf, autosave::Looks>,
    /// When the `pr-merge` repos were last read for an open pull request.
    repo_requests_read: Option<Instant>,
}

/// How long ago `at`, in seconds since the epoch, was.
fn ago(at: Option<u64>) -> Option<Duration> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    at.map(|at| Duration::from_secs(now.saturating_sub(at)))
}

impl State {
    /// Look again at the open pull request of a `pr-merge` plane, when a look is due
    /// ([`autosave::Looks`]): merged if its checks passed since, and otherwise what the look
    /// found is kept for the standing. Answers whether the window has something to read.
    ///
    /// Asked as [`planegit::look_caller`] says for a look from the window: with the person's
    /// purlis sign-in where there is one, as a save pressed here would (ADR 0070 §5, amended
    /// 2026-10-14).
    fn look_at_request(
        &mut self,
        root: &Path,
        plane: &planesave::Plane,
        now: Instant,
        poke: Option<Poke>,
    ) -> bool {
        let open = (plane.mode.value == Some(planesave::Mode::PrMerge))
            .then(|| planegit::request_to_look_at(root))
            .flatten();
        let Some((pushed_at, head)) = open else {
            self.looks = autosave::Looks::default();
            return false;
        };
        let asked = poke == Some(Poke::Fetch);
        if !self.looks.due(now, &head, ago(pushed_at), asked) {
            return false;
        }
        let caller = planegit::look_caller(planegit::Looker::Window);
        match planegit::look(root, &caller, &mut |_| {}) {
            Ok(found) => {
                self.moved_tree |= found.moved;
                found.changed
            }
            // A save is running, or the remote is not the recorded one: the next look asks.
            Err(_) => false,
        }
    }

    /// The repos' half of [`Self::look_at_request`]: each `pr-merge` repo whose row says its
    /// pull request is still purlis's to merge. None, unless a table says `pr-merge`, and then
    /// nothing here costs more than the settings read. Answers whether any row changed.
    fn look_at_repo_requests(&mut self, root: &Path, now: Instant, poke: Option<Poke>) -> bool {
        let asked = poke == Some(Poke::Fetch);
        let read_due = self
            .repo_requests_read
            .is_none_or(|at| now.duration_since(at) >= REPO_REQUESTS_EVERY);
        if !(read_due || asked) {
            return false;
        }
        self.repo_requests_read = Some(now);
        let settings = planesave::Settings::read(root);
        let merging: Vec<String> = settings
            .repo_tables()
            .into_iter()
            .filter(|name| settings.repo(name).mode.value == planesave::Mode::PrMerge)
            .collect();
        if merging.is_empty() {
            self.repo_looks.clear();
            return false;
        }
        let Ok(workspaces) = purlis_core::workspaces::Plane::open(root).workspaces() else {
            return false;
        };
        let caller = planegit::look_caller(planegit::Looker::Window);
        let mut did = false;
        let mut open: Vec<PathBuf> = Vec::new();
        for workspace in workspaces {
            let Ok(found) = purlis_core::repos::clones(root, &workspace) else {
                continue;
            };
            for repo in found.repos.iter().filter(|r| merging.contains(&r.name)) {
                // The shared standing, as the row reads it (FD-11): no journal read per look.
                let standing = reposave::shared_standing(root, &workspace, repo);
                let waits = standing.stage == Stage::PrOpen
                    && standing.merge.as_ref().is_some_and(|merge| {
                        !matches!(
                            merge.state,
                            planegit::MergeState::Queued | planegit::MergeState::Merged
                        )
                    });
                let (true, Some(head)) = (waits, standing.head.as_deref()) else {
                    continue;
                };
                open.push(repo.path.clone());
                let looks = self.repo_looks.entry(repo.path.clone()).or_default();
                // When it was pushed is the journal's to say, and the first look waits out the
                // whole settle wait from when the row was first seen instead.
                if !looks.due(now, head, None, asked) {
                    continue;
                }
                if let Ok(found) = reposave::look(root, &workspace, repo, &caller) {
                    did |= found.changed;
                }
            }
        }
        self.repo_looks.retain(|path, _| open.contains(path));
        did
    }
}

impl State {
    /// One look at the plane at `root`. Answers whether anything was done the window should
    /// hear about.
    fn look(&mut self, root: &Path, now: Instant, poke: Option<Poke>) -> bool {
        // A chat that ended has most likely left work behind, wherever in the plane it wrote:
        // the standing is read again rather than shared from before it ended (FD-11).
        if poke == Some(Poke::SessionEnded) {
            planegit::touch(root);
        }
        let plane = planesave::Settings::read(root).plane;
        let mut did = false;

        let due = match self.fetched {
            None => true,
            Some(at) if poke == Some(Poke::Fetch) => now.duration_since(at) >= FETCH_AT_MOST_EVERY,
            Some(at) => now.duration_since(at) >= FETCH_EVERY,
        };
        if due {
            self.fetched = Some(now);
            if let Ok(incoming) = planegit::fetch(root, autosave::on(&plane)) {
                did |= incoming.behind > 0;
                self.moved_tree |= incoming.moved;
            }
        }
        // Whether auto-save is on or not: the merge is what the save that opened the request
        // was asked for, and a red one is the person's to hear about.
        did |= self.look_at_request(root, &plane, now, poke);
        // Off: what came in is still fetched and shown, and nothing else costs a git process.
        if !autosave::on(&plane) {
            return did;
        }

        // Shared with the title bar, the Saving tab and the alerts (FD-11): git runs for one
        // of them, and the save this may start reads the tree again for itself.
        let standing = planegit::shared_standing(root);
        let trigger = if !self.launched {
            self.launched = true;
            // What the last run committed and could not push, pushed first.
            (autosave::on(&plane) && standing.stage == Stage::Committed && standing.pushes)
                .then_some(Trigger::Launch)
        } else if poke == Some(Poke::SessionEnded) {
            (autosave::on(&plane)
                && standing.stage != Stage::Blocked
                && autosave::worth_saving(&standing))
            .then_some(Trigger::SessionEnd)
        } else {
            None
        };
        let trigger = trigger.or_else(|| {
            let seen = planegit::fingerprint_of(root, &standing);
            (self.quiet.tick(now, &plane, &standing, &seen) == Decision::Save
                && self.still_quiet(root, now))
            .then_some(Trigger::Quiet)
        });
        if let Some(trigger) = trigger {
            // A refusal is in the journal, in its own words; the stage says blocked.
            let before = head_of(root);
            self.saved = crate::saving::save_as(root, None, trigger).is_ok();
            self.moved_tree |= tree_moved(root, before.as_deref());
            did = true;
        }
        did
    }
}

impl State {
    /// The quiet period is over by the shared standing, which can be up to
    /// [`planegit::SHARED_FOR`] behind a new file: read the plane again before saving, and start
    /// the period again when it has moved (FD-11).
    fn still_quiet(&mut self, root: &Path, now: Instant) -> bool {
        let fresh = planegit::standing(root);
        let still = self
            .quiet
            .still(now, &planegit::fingerprint_of(root, &fresh));
        if !still {
            // The shared standing is behind: the next look reads it again.
            planegit::touch(root);
        }
        still
    }
}

impl State {
    /// One look at the workspace repos whose `[repos.<name>] autosave` is on — none, unless a
    /// table says so, and then nothing here costs a git process. Answers whether any was saved.
    fn look_at_repos(
        &mut self,
        root: &Path,
        now: Instant,
        poke: Option<Poke>,
        mid_turn: &dyn Fn(&str) -> Vec<String>,
    ) -> bool {
        let launched = std::mem::replace(&mut self.repos_launched, true);
        if poke == Some(Poke::SessionEnded) {
            planegit::touch(root);
        }
        let settings = planesave::Settings::read(root);
        let saved: Vec<String> = settings
            .repo_tables()
            .into_iter()
            .filter(|name| autosave::repo_on(&settings.repo(name)))
            .collect();
        if saved.is_empty() {
            self.repos.clear();
            return false;
        }
        let Ok(workspaces) = purlis_core::workspaces::Plane::open(root).workspaces() else {
            return false;
        };
        let mut did = false;
        for workspace in workspaces {
            let Ok(found) = purlis_core::repos::clones(root, &workspace) else {
                continue;
            };
            let mut busy: Option<Vec<String>> = None;
            for repo in found.repos.iter().filter(|r| saved.contains(&r.name)) {
                let quiet = self.repos.entry(repo.path.clone()).or_default();
                let busy = busy.get_or_insert_with(|| mid_turn(&workspace));
                if !busy.is_empty() {
                    // This cycle is skipped, and the quiet period starts again once the turn
                    // is over: the turn was the change.
                    *quiet = Quiet::default();
                    continue;
                }
                let standing = reposave::shared_standing(root, &workspace, repo);
                let settled = standing.stage != Stage::Blocked && standing.worth_saving();
                let trigger = if !launched {
                    (standing.stage == Stage::Committed && standing.pushes)
                        .then_some(Trigger::Launch)
                } else if poke == Some(Poke::SessionEnded) {
                    settled.then_some(Trigger::SessionEnd)
                } else {
                    None
                };
                let trigger = trigger.or_else(|| {
                    (quiet.tick_repo(now, &settings.repo(&repo.name), &standing) == Decision::Save
                        // Read again before saving, as the plane is (FD-11).
                        && {
                            let fresh = reposave::standing(root, &workspace, repo);
                            let still = quiet.still(now, &fresh.fingerprint());
                            if !still {
                                planegit::touch(&repo.path);
                            }
                            still
                        })
                    .then_some(Trigger::Quiet)
                });
                if let Some(trigger) = trigger {
                    // A refusal is in the journal, in its own words; the row says blocked.
                    let before = head_of(&repo.path);
                    let _ = reposave::save_as(
                        &reposave::Request {
                            plane: root,
                            workspace: &workspace,
                            name: &repo.name,
                            clone: &repo.path,
                            message: None,
                            no_push: false,
                            // Asked again right before `git add`, not answered from this look.
                            mid_turn: &|| mid_turn(&workspace),
                        },
                        trigger,
                        &mut |_| {},
                    );
                    self.moved_tree |= tree_moved(&repo.path, before.as_deref());
                    did = true;
                }
            }
        }
        did
    }
}

/// Every clone whose `[repos.<name>] autosave` is on, in every workspace of the plane at
/// `root`, as `(workspace, clone)`.
fn auto_saved_repos(root: &Path) -> Vec<(String, purlis_core::repos::Repo)> {
    let settings = planesave::Settings::read(root);
    let saved: Vec<String> = settings
        .repo_tables()
        .into_iter()
        .filter(|name| autosave::repo_on(&settings.repo(name)))
        .collect();
    if saved.is_empty() {
        return Vec::new();
    }
    let workspaces = purlis_core::workspaces::Plane::open(root)
        .workspaces()
        .unwrap_or_default();
    workspaces
        .into_iter()
        .flat_map(|workspace| {
            let found = purlis_core::repos::clones(root, &workspace)
                .map(|found| found.repos)
                .unwrap_or_default();
            found
                .into_iter()
                .filter(|repo| saved.contains(&repo.name))
                .map(move |repo| (workspace.clone(), repo))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// How long a quit gives every plane's saving, together, to stop (#2002): longer than the
/// session host takes to stop a save in the middle (`crate::hostsaving::STOP_WITHIN`).
pub const QUIT_STOPS_WITHIN: Duration = Duration::from_secs(3);

/// Stops every one of `savers` at once, by `deadline`, and calls `end_the_host` when any did
/// not say it had stopped by then (#2002): the session host saving a project is not answering,
/// and is ended before the save at quit, which would otherwise run beside its save.
pub fn stop_every_saver_by(
    savers: Vec<Box<dyn Saver>>,
    deadline: Instant,
    end_the_host: &dyn Fn(),
) {
    let stopped = std::thread::scope(|scope| {
        let stopping: Vec<_> = savers
            .into_iter()
            .map(|saver| scope.spawn(move || saver.stop_by(deadline)))
            .collect();
        // Every one joined, each in its own time: none waits on another.
        stopping
            .into_iter()
            .map(|stopping| stopping.join().unwrap_or(false))
            .collect::<Vec<bool>>()
            .into_iter()
            .all(|stopped| stopped)
    });
    if !stopped {
        tracing::warn!("purlis: the session host did not stop saving in time, so it is ended");
        end_the_host();
    }
}

/// The window came back into focus: fetch the plane's target branch, unless it was fetched a
/// moment ago. Answers at once; what the fetch finds reaches the window as a plane change.
#[tauri::command]
#[specta::specta]
pub fn plane_fetch(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<(), String> {
    planes.held(&plane)?.poke_autosave(Poke::Fetch);
    Ok(())
}

/// Save every plane in `roots` as the app quits, all at once (ADR 0051), and return within
/// [`QUIT_BOUND`] and a moment more whatever happens: a commit that waits on a signer or a
/// slow hook is left to finish on its own rather than holding the app open.
///
/// Each plane's repos are saved beside it — only those whose `[repos.<name>] autosave` is on.
///
/// `roots` carries, beside each plane, who was mid-turn in which workspace before the quit
/// ended the chats: those workspaces' repos are not saved, and their journal says why.
pub fn at_quit(roots: Vec<(PathBuf, HashMap<String, Vec<String>>)>) {
    let (done, finished) = mpsc::channel();
    let mut started = 0;
    for (root, cut_off) in roots {
        for (workspace, repo) in auto_saved_repos(&root) {
            let done = done.clone();
            let plane = root.clone();
            let busy = cut_off.get(&workspace).cloned().unwrap_or_default();
            started += 1;
            std::thread::spawn(move || {
                crate::forgeaccounts::window_git(|| {
                    autosave::repo_at_quit(&plane, &workspace, &repo, &busy, QUIT_BOUND);
                });
                let _ = done.send(());
            });
        }
        let done = done.clone();
        started += 1;
        std::thread::spawn(move || {
            crate::forgeaccounts::window_git(|| autosave::at_quit(&root, QUIT_BOUND));
            let _ = done.send(());
        });
    }
    let until = Instant::now() + QUIT_BOUND + Duration::from_secs(1);
    for _ in 0..started {
        let left = until.saturating_duration_since(Instant::now());
        if finished.recv_timeout(left).is_err() {
            return;
        }
    }
}

/// How many workers look at each plane in this process, for the tests that prove exactly one
/// saves a project.
#[cfg(test)]
pub(crate) mod running {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{LazyLock, Mutex, PoisonError};

    static ON: LazyLock<Mutex<HashMap<PathBuf, usize>>> = LazyLock::new(Mutex::default);

    /// How many workers look at the plane at `root` now.
    pub(crate) fn on(root: &Path) -> usize {
        ON.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(root)
            .copied()
            .unwrap_or(0)
    }

    /// One worker's loop, counted while it runs.
    pub(super) struct Counted(PathBuf);

    impl Counted {
        pub(super) fn on(root: &Path) -> Counted {
            *ON.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(root.to_path_buf())
                .or_default() += 1;
            Counted(root.to_path_buf())
        }
    }

    impl Drop for Counted {
        fn drop(&mut self) {
            let mut on = ON.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(count) = on.get_mut(&self.0) {
                *count = count.saturating_sub(1);
            }
        }
    }
}

impl Worker {
    /// Drop it and wait up to `bound` for its thread to end, a save it is in the middle of
    /// with it, and so the save's claim on the plane: whether it did (#2002).
    pub fn stop_within(mut self, bound: Duration) -> bool {
        let thread = self.thread.take();
        drop(self);
        let until = Instant::now() + bound;
        let Some(thread) = thread else { return true };
        while Instant::now() < until {
            if thread.is_finished() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn answers_of(what: crate::planewatch::What) -> Option<Vec<crate::planewatch::PlaneAnswer>> {
        crate::planewatch::PlaneChanged::of(
            serde_json::from_value(serde_json::json!("/home/dev/plane")).expect("an id"),
            what,
        )
        .answers
    }

    #[test]
    fn a_look_that_left_the_tree_alone_tells_the_git_standings_alone() {
        // #933: a commit, a push or a fetch that brought nothing in moved the git standing
        // and no file, and used to be told as "anything may have moved".
        assert_eq!(
            answers_of(what_it_did(false)),
            Some(vec![crate::planewatch::PlaneAnswer::Git])
        );
    }

    #[test]
    fn a_look_that_moved_the_tree_tells_every_reader() {
        // A fetch that fast-forwarded, a save that rebased: files moved that the watch does
        // not see (it is not recursive) or that nobody watches at all.
        assert_eq!(answers_of(what_it_did(true)), None);
    }

    #[test]
    fn a_save_that_commits_the_tree_as_it_was_has_not_moved_it() {
        let dir = plane("[plane]\nmode = \"commit\"\n");
        let mut state = State::default();
        let at = Instant::now();
        state.look(dir.path(), at, None);
        std::fs::write(dir.path().join("note.md"), "n").unwrap();

        assert!(state.look(
            dir.path(),
            at + Duration::from_secs(1),
            Some(Poke::SessionEnded)
        ));

        assert_eq!(commits(dir.path()), 1);
        assert!(
            !state.moved_tree,
            "a commit of the tree as it was moved nothing"
        );
    }

    #[test]
    fn a_head_that_moved_past_the_saves_own_commit_moved_the_tree() {
        let dir = plane("[plane]\nmode = \"commit\"\n");
        let root = dir.path();
        let before = head_of(root);
        assert!(!tree_moved(root, before.as_deref()), "nothing happened");

        git(
            root,
            &["commit", "-q", "--allow-empty", "-m", "the save's own"],
        );
        assert!(
            !tree_moved(root, before.as_deref()),
            "one commit on top is the save's"
        );

        // What a rebase onto the remote leaves: other commits under the save's own.
        git(
            root,
            &["commit", "-q", "--allow-empty", "-m", "replayed on top"],
        );
        assert!(tree_moved(root, before.as_deref()));
        assert!(
            tree_moved(root, None),
            "a head that could not be read is not known"
        );
    }

    fn git(dir: &Path, args: &[&str]) {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(dir)
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid");
        let out = purlis_core::forklock::output(&mut command).expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A plane with one commit, its own identity, and no remote.
    fn plane(toml: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("charter.toml"), toml).unwrap();
        std::fs::write(root.join(".gitignore"), ".charter/\n").unwrap();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.name", "t"]);
        git(root, &["config", "user.email", "t@example.invalid"]);
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "one"]);
        dir
    }

    #[test]
    fn a_fetch_that_fast_forwards_the_plane_tells_the_window_everything() {
        // #1028: the fetch end to end. `origin` is on a forge charter knows, mapped by the
        // plane's own config onto a local bare repository, never the network; somebody else's
        // commit waits there, and auto-save is on, so the look fast-forwards onto it.
        let dir = plane("[plane]\nmode = \"commit\"\n");
        let root = dir.path().canonicalize().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let elsewhere = outside.path().canonicalize().unwrap();
        let forge = elsewhere.join("forge/acme");
        std::fs::create_dir_all(&forge).unwrap();
        git(&forge, &["init", "-q", "--bare", "-b", "main", "plane.git"]);
        let bare = forge.join("plane.git").display().to_string();
        git(
            &root,
            &["remote", "add", "origin", "git@github.com:acme/plane.git"],
        );
        git(
            &root,
            &[
                "config",
                &format!("url.file://{}/.insteadOf", forge.display()),
                "https://github.com/acme/",
            ],
        );
        git(&root, &["push", "-q", &bare, "main"]);
        let theirs = elsewhere.join("theirs");
        git(
            &elsewhere,
            &["clone", "-q", &bare, &theirs.display().to_string()],
        );
        std::fs::write(theirs.join("theirs.md"), "theirs\n").unwrap();
        git(&theirs, &["add", "-A"]);
        git(&theirs, &["commit", "-q", "-m", "theirs"]);
        git(&theirs, &["push", "-q", "origin", "main"]);

        let mut state = State::default();
        assert!(
            state.look(&root, Instant::now(), None),
            "a fetch that brought a commit in is something to tell"
        );

        assert!(
            root.join("theirs.md").is_file(),
            "the clean tree was not fast-forwarded"
        );
        assert!(state.moved_tree, "a fast-forward moved the tree");
        // What the loop tells the window (`tell(moved_tree)`): `null`, every answer, so a view
        // whose files no watch covers reads again too.
        assert_eq!(answers_of(what_it_did(state.moved_tree)), None);
    }

    fn commits(root: &Path) -> usize {
        planegit::journal(root)
            .iter()
            .filter(|l| l["outcome"] == "committed")
            .count()
    }

    #[test]
    fn a_chat_that_ends_saves_the_plane_at_once_when_auto_save_is_on() {
        let dir = plane("[plane]\nmode = \"commit\"\n");
        let mut state = State::default();
        let at = Instant::now();
        state.look(dir.path(), at, None);
        std::fs::write(dir.path().join("note.md"), "n").unwrap();

        assert!(state.look(
            dir.path(),
            at + Duration::from_secs(1),
            Some(Poke::SessionEnded)
        ));

        let last = &planegit::journal(dir.path())[0];
        assert_eq!(
            (last["trigger"].as_str(), last["outcome"].as_str()),
            (Some("session-end"), Some("committed"))
        );
    }

    #[test]
    fn a_quiet_plane_is_saved_after_its_quiet_period_and_not_before() {
        let dir = plane("[plane]\nmode = \"commit\"\nautosave_after = \"30s\"\n");
        let mut state = State::default();
        let at = Instant::now();
        state.look(dir.path(), at, None);
        std::fs::write(dir.path().join("note.md"), "n").unwrap();
        // The plane's watcher seeing the write (FD-11): the shared standing is read again.
        planegit::touch(dir.path());

        assert!(!state.look(dir.path(), at + Duration::from_secs(2), None));
        assert!(!state.look(dir.path(), at + Duration::from_secs(31), None));
        assert_eq!(commits(dir.path()), 0);
        assert!(state.look(dir.path(), at + Duration::from_secs(32), None));
        assert_eq!(commits(dir.path()), 1);
    }

    #[test]
    fn a_file_the_shared_standing_has_not_seen_yet_restarts_the_quiet_period() {
        // FD-11: the shared standing may be up to ten seconds behind a new file, so the save a
        // quiet period earns is checked against the tree as it is first.
        let dir = plane("[plane]\nmode = \"commit\"\nautosave_after = \"30s\"\n");
        let mut state = State::default();
        let at = Instant::now();
        state.look(dir.path(), at, None);
        std::fs::write(dir.path().join("note.md"), "n").unwrap();
        planegit::touch(dir.path());
        assert!(!state.look(dir.path(), at + Duration::from_secs(2), None));

        // Written a moment before the quiet period ends, and not yet in the shared standing.
        std::fs::write(dir.path().join("late.md"), "l").unwrap();

        assert!(!state.look(dir.path(), at + Duration::from_secs(33), None));
        assert_eq!(commits(dir.path()), 0, "a file seconds old was saved");
        assert!(state.look(dir.path(), at + Duration::from_secs(64), None));
        assert_eq!(commits(dir.path()), 1);
    }

    #[test]
    fn a_plane_nobody_has_chosen_a_mode_for_is_never_saved_by_itself() {
        let dir = plane("[memory]\nshare = \"local\"\n");
        let mut state = State::default();
        let at = Instant::now();
        state.look(dir.path(), at, None);
        std::fs::write(dir.path().join("note.md"), "n").unwrap();
        for s in [2, 40, 600] {
            state.look(dir.path(), at + Duration::from_secs(s), None);
        }
        state.look(
            dir.path(),
            at + Duration::from_secs(601),
            Some(Poke::SessionEnded),
        );
        assert!(planegit::journal(dir.path()).is_empty());
    }

    /// A plane saying `toml`, with a workspace `alpha` holding a clone `widget`.
    fn plane_with_a_repo(toml: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = plane(toml);
        let clone = dir.path().join("workspaces/alpha/widget");
        std::fs::create_dir_all(&clone).unwrap();
        git(&clone, &["init", "-q", "-b", "main"]);
        git(&clone, &["config", "user.name", "t"]);
        git(&clone, &["config", "user.email", "t@example.invalid"]);
        std::fs::write(clone.join("README.md"), "one").unwrap();
        git(&clone, &["add", "-A"]);
        git(&clone, &["commit", "-q", "-m", "one"]);
        (dir, clone)
    }

    fn repo_commits(root: &Path) -> usize {
        planegit::journal(root)
            .iter()
            .filter(|l| l["target"] == "repo:alpha/widget" && l["outcome"] == "committed")
            .count()
    }

    #[test]
    fn a_repo_with_auto_save_on_is_saved_after_its_quiet_period_but_never_mid_turn() {
        let (dir, clone) = plane_with_a_repo(
            "[repos.widget]\nmode = \"commit\"\nautosave = true\nautosave_after = \"30s\"\n",
        );
        let root = dir.path();
        let mut state = State::default();
        let at = Instant::now();
        let working = |_: &str| vec!["alpha.1".to_owned()];
        let idle = |_: &str| Vec::new();
        state.look_at_repos(root, at, None, &idle);
        std::fs::write(clone.join("a.md"), "a").unwrap();
        // Told, as the watcher or the shared standing's age tells it in the app (FD-11).
        planegit::touch(root);

        // A chat in alpha is mid-turn: every cycle is skipped, however long it runs.
        for s in [1, 31, 90, 300] {
            assert!(!state.look_at_repos(root, at + Duration::from_secs(s), None, &working));
        }
        assert!(
            !state.look_at_repos(
                root,
                at + Duration::from_secs(301),
                Some(Poke::SessionEnded),
                &working
            ),
            "a chat ending elsewhere while one here still works is not a reason"
        );
        assert_eq!(repo_commits(root), 0);

        // The turn ends; the quiet period starts then.
        assert!(!state.look_at_repos(root, at + Duration::from_secs(302), None, &idle));
        assert!(!state.look_at_repos(root, at + Duration::from_secs(331), None, &idle));
        assert!(state.look_at_repos(root, at + Duration::from_secs(332), None, &idle));
        assert_eq!(repo_commits(root), 1);
    }

    #[test]
    fn a_file_in_a_repo_the_shared_standing_has_not_seen_restarts_its_quiet_period() {
        let (dir, clone) = plane_with_a_repo(
            "[repos.widget]\nmode = \"commit\"\nautosave = true\nautosave_after = \"30s\"\n",
        );
        let root = dir.path();
        let mut state = State::default();
        let at = Instant::now();
        let idle = |_: &str| Vec::new();
        state.look_at_repos(root, at, None, &idle);
        std::fs::write(clone.join("a.md"), "a").unwrap();
        planegit::touch(root);
        assert!(!state.look_at_repos(root, at + Duration::from_secs(1), None, &idle));

        // A new file, not yet in the shared standing, a moment before the period ends.
        std::fs::write(clone.join("late.md"), "l").unwrap();

        assert!(!state.look_at_repos(root, at + Duration::from_secs(32), None, &idle));
        assert_eq!(repo_commits(root), 0, "a file seconds old was saved");
        assert!(state.look_at_repos(root, at + Duration::from_secs(63), None, &idle));
        assert_eq!(repo_commits(root), 1);
    }

    #[test]
    fn a_turn_that_starts_during_a_repo_auto_save_is_seen_right_before_it_stages() {
        let (dir, clone) = plane_with_a_repo(
            "[repos.widget]\nmode = \"commit\"\nautosave = true\nautosave_after = \"30s\"\n",
        );
        let root = dir.path();
        let mut state = State::default();
        let at = Instant::now();
        let idle = |_: &str| Vec::new();
        state.look_at_repos(root, at, None, &idle);
        std::fs::write(clone.join("a.md"), "a").unwrap();
        // Told, as the watcher or the shared standing's age tells it in the app (FD-11).
        planegit::touch(root);
        state.look_at_repos(root, at + Duration::from_secs(1), None, &idle);

        // Nobody at this look; somebody by the time the save asks again.
        let asked = std::cell::Cell::new(0);
        let starting = |_: &str| {
            asked.set(asked.get() + 1);
            if asked.get() > 2 {
                vec!["alpha.1".to_owned()]
            } else {
                Vec::new()
            }
        };
        state.look_at_repos(root, at + Duration::from_secs(40), None, &starting);

        assert_eq!(
            repo_commits(root),
            0,
            "a turn's half-written file was committed"
        );
        let line = planegit::journal(root).pop().expect("the skipped save");
        assert_eq!(line["outcome"], "skipped");
    }

    #[test]
    fn a_repo_whose_table_leaves_auto_save_off_is_never_saved_by_itself() {
        let (dir, clone) = plane_with_a_repo("[repos.widget]\nmode = \"commit\"\n");
        let root = dir.path();
        let mut state = State::default();
        let at = Instant::now();
        let idle = |_: &str| Vec::new();
        std::fs::write(clone.join("a.md"), "a").unwrap();
        for s in [0, 31, 62, 400] {
            state.look_at_repos(root, at + Duration::from_secs(s), None, &idle);
        }
        state.look_at_repos(
            root,
            at + Duration::from_secs(401),
            Some(Poke::SessionEnded),
            &idle,
        );
        assert_eq!(repo_commits(root), 0);
    }

    /// A saver whose stop is answered at once, or never.
    struct Stops {
        answers: bool,
    }

    impl Saver for Stops {
        fn poker(&self) -> Sender<Poke> {
            mpsc::channel().0
        }

        fn asks_mid_turn_of(&self, _ask: MidTurn) {}

        fn stop_by(self: Box<Self>, deadline: Instant) -> bool {
            if !self.answers {
                std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
            }
            self.answers
        }
    }

    #[test]
    fn a_quit_stops_every_saver_at_once_and_ends_a_host_that_did_not_answer() {
        let ended = std::sync::atomic::AtomicUsize::new(0);
        let started = Instant::now();
        let deadline = started + Duration::from_millis(400);

        stop_every_saver_by(
            vec![
                Box::new(Stops { answers: false }),
                Box::new(Stops { answers: true }),
                Box::new(Stops { answers: false }),
            ],
            deadline,
            &|| {
                ended.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            },
        );

        assert_eq!(ended.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(
            started.elapsed() < Duration::from_millis(750),
            "the savers were stopped one after another: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_quit_whose_savers_all_stopped_leaves_the_host_alone() {
        let ended = std::sync::atomic::AtomicBool::new(false);

        stop_every_saver_by(
            vec![
                Box::new(Stops { answers: true }),
                Box::new(Stops { answers: true }),
            ],
            Instant::now() + Duration::from_millis(400),
            &|| ended.store(true, std::sync::atomic::Ordering::SeqCst),
        );

        assert!(!ended.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn a_worker_let_go_of_stops_even_while_a_chats_end_still_holds_its_sender() {
        let dir = plane("[plane]\nmode = \"commit\"\n");
        let worker = Worker::start(dir.path().to_path_buf(), Arc::new(|_| {}));
        let held_by_a_chat = worker.poker();

        assert!(
            worker.stop_within(Duration::from_secs(5)),
            "the worker kept looking"
        );
        drop(held_by_a_chat);
    }
}
