// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every plane this process holds open, and what it holds for each one.
//!
//! **The app used to hold exactly one.** `Hooks`, `Chats` and the plane's root were managed
//! as process-wide singletons, every command took `State<'_, Hooks>` or `State<'_, Chats>`,
//! and which plane those belonged to was whatever directory the process was launched from.
//! A project IS a plane, so an app that can only hold one can only ever show one project.
//!
//! What replaces it is a registry keyed by the plane's root. Nothing per-plane is managed by
//! Tauri any more, so there is no `State<'_, Chats>` to reach for: the only way to a board or
//! a chat is [`Planes::held`], and that takes the [`PlaneId`] the caller is acting for. A
//! command that forgets which plane it means does not compile.
//!
//! **A session number means nothing without its plane.** Each plane numbers its own chats
//! from one, and that number is what charter hands the chat as `CHARTER_SESSION_ID` — the
//! rung `.charter/sessions/<sid>.workspace` is keyed on, inside that plane. Numbering across
//! planes instead would make one plane's chat numbers depend on which other planes the
//! process happened to be holding, and put that dependency on disk. So the identity of a chat
//! is the pair, and every command that names a session names its plane beside it.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use purlis_core::engine::Size;
use purlis_core::instructions::Stamp;
use purlis_core::machine;
use purlis_core::planemodel::Model;
use purlis_core::reopen;
use purlis_core::reopen::Choice;

use crate::chats::Chats;
use crate::hooks::{self, Hooks, Moved};
use crate::host::{ChatBoard, SessionHost};
use crate::sessions::{Reporting, Sessions};

/// Which plane something is acting for.
///
/// It is the plane's root as the registry resolved it, and it is minted by [`Planes::open`]
/// alone — a caller hands one back, it never spells one. Two spellings of one directory would
/// otherwise be two entries in the registry holding two boards for one plane on disk, which
/// is the "acting on the wrong plane" defect wearing a different hat.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PlaneId(String);

impl PlaneId {
    /// The root this id names.
    pub fn root(&self) -> &Path {
        Path::new(&self.0)
    }

    /// The id of a root that has already been resolved.
    fn of(root: &Path) -> Self {
        Self(root.display().to_string())
    }

    /// The root it names, for a message about it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PlaneId {
    /// The id the session host's watch of a project it saves is started under (#2002): that
    /// watch tells nobody by it, and no registry of the host's holds it.
    pub(crate) fn saved_in_the_host(root: &Path) -> Self {
        Self::of(root)
    }
}

#[cfg(test)]
impl PlaneId {
    /// An id for a test that holds no registry — a worker, a teller — never for a command.
    pub fn for_tests(root: &Path) -> Self {
        Self::of(root)
    }
}

/// The operator's yes to acting on one plane's record.
///
/// **`.charter/app/reopen.json` is an execution input.** Putting a record back STARTS the
/// programs it names, and for a chat that was not on a profile what runs is decided from the
/// record alone. A plane is a DIRECTORY, and a directory arrives by zip, by shared folder or
/// on a stick as readily as by `git clone` — more readily, in fact, since `.charter/` is
/// gitignored and does not ride in through a clone at all. So "open this directory" must not
/// be able to mean "run what is written in it".
///
/// It has no constructor outside this module and holds nothing, which is the whole design:
/// [`Planes::open`] attaches a plane and cannot reopen it, and [`Planes::reopen`] is private
/// and takes one of these. A plane that has not been approved cannot reach the record by
/// construction rather than by a caller remembering to look.
///
/// **Two yeses, and they are both written down here.** The launch's own working directory —
/// the operator ran charter there, which is the act — and the operator's answer to the trust
/// ask ([`Planes::approve_and_open`]). Every other way in (an opener handing over a path, a
/// recents row, a second instance's argument) reaches the second of those and nothing else,
/// so adding a third is a deliberate edit to this file rather than an argument somebody
/// forgot to pass.
pub struct Approved(());

/// The one way this app writes a plane's record — **and `reopen::write` is named here and
/// nowhere else in it**, because writing that file and vouching for it are one act.
///
/// charter rewrites `.charter/app/reopen.json` every time a chat opens or closes, and the
/// machine store's trust fingerprint covers the programs that record would start. A
/// fingerprint taken only when the operator approved the plane therefore goes stale on their
/// very next click, and the next launch asks them about a chat they started themselves. An
/// operator trained to dismiss that question is worse off than one who was never asked, so
/// the two writes are not two things a caller has to remember to pair: there is one method,
/// and no path to the first without the second.
///
/// `Store::vouch` never creates an approval, only refreshes one, so a plane nobody has
/// approved stays unapproved however many times charter writes its record.
struct Records {
    root: PathBuf,
    /// Where this machine's store lives, resolved once at startup like the plane itself —
    /// it is an environment ladder, and a second reader of it is a second answer.
    config: Option<PathBuf>,
    /// Whether this plane's record is the app's to read and write.
    ///
    /// Set when the record is put back, which only an approved plane reaches. An attached
    /// plane nobody has approved is not recorded EITHER WAY: reading it would run what it
    /// names, and writing it would replace the operator's own record — of a plane they never
    /// said yes to — with whatever this process happens to have open, which is nothing.
    allowed: AtomicBool,
    /// The clone and the device this app writes the record from (V43), stamped onto every
    /// record it writes, so the next launch can tell whether it is still in this clone, on
    /// this device — or in a copy, or a move ([`reopen::arrive`]).
    clone_seat: reopen::CloneSeat,
}

impl Records {
    /// Writes what is open into the plane, then vouches for it.
    ///
    /// **Record first, vouch second, and the order is not a preference.**
    /// `machine::Contribution::of_record` reads the record back off disk, so a vouch taken before
    /// the write would fingerprint what was there a moment ago — which is the stale
    /// fingerprint this exists to prevent, arrived at from the other side.
    ///
    /// Neither half is worth interrupting the operator over. A record that cannot be written
    /// means the next launch of this plane comes back empty; a vouch that cannot be taken
    /// means one spurious question at that launch. Both are said and neither refuses.
    ///
    /// And once it is written, the theme drafts of the chats it no longer holds are deleted.
    fn write(&self, record: &reopen::Record) {
        if !self.allowed.load(Ordering::SeqCst) {
            return;
        }
        let record = reopen::Record {
            clone_seat: Some(self.clone_seat.clone()),
            ..record.clone()
        };
        if let Err(why) = reopen::write(&self.root, &record) {
            tracing::warn!(
                "purlis: what is open in {} was not recorded ({why})",
                self.root.display()
            );
            // Not vouched for: the fingerprint would then describe a record charter did not
            // manage to write, and the point of it is that it describes what is there.
            return;
        }
        self.vouch();
        // **The theme drafts of a chat the record no longer holds go** (#2708): it holds every
        // chat open, and every one a launch has still to start, so a draft deleted here is one
        // no strip names and no close asks about again.
        let open: Vec<&str> = record
            .chats
            .iter()
            .filter_map(|one| one.identity.id.as_deref())
            .collect();
        crate::themedrafts::sweep(&self.root, &open);
        // **And a closed chat's browser timeline** (#2017), with any screenshot its list no
        // longer names: what a crash or a quit left.
        purlis_core::browsertimeline::sweep(&self.root, &open);
        // **And a closed chat's Skill studio leftovers** (SK-9, #2772): the record of the levels
        // a sandboxed chat could have made, and a Try it's throwaway piece no chat stands in.
        crate::skilldrafts::sweep(&self.root, &record);
    }

    /// Every project this machine remembers opening (the machine store's recents), which is
    /// where a launch looks for the clone a copy was made from once the copy's original has
    /// moved (V43, D-V43x). Every open is remembered before its record is put back, so every
    /// launch registers its clone here. Empty where there is no store (ADR 0031).
    fn known(&self) -> Vec<PathBuf> {
        let Some(config) = self.config.as_deref() else {
            return Vec::new();
        };
        machine::read(config)
            .store
            .recents
            .into_iter()
            .map(|recent| recent.plane)
            .collect()
    }

    /// Re-fingerprints the plane, because charter itself just changed what opening it would
    /// do.
    ///
    /// A machine with no store — Windows, where `0600` has no expression and the store
    /// refuses outright (ADR 0031, charter-app#98) — simply has nothing to refresh. The app
    /// runs there; it just cannot remember planes between launches.
    fn vouch(&self) {
        let Some(config) = self.config.as_deref() else {
            return;
        };
        // The record's half alone: a vouch refreshes nothing else, so nothing else is read.
        let contributed = machine::Contribution::of_record(&self.root);
        let when = now();
        let root = self.root.clone();
        if let Err(why) = machine::update(config, move |store| {
            store.vouch(&root, contributed, when);
        }) && why.kind() != std::io::ErrorKind::Unsupported
        {
            tracing::warn!(
                "purlis: the record of {} was written but not vouched for ({why}); purlis                  may ask about this plane again at the next launch",
                self.root.display()
            );
        }
    }

    /// From here on this plane's record is charter's to write. Nothing before this point
    /// wrote one, so nothing before it vouched for one either.
    fn allow(&self) {
        self.allowed.store(true, Ordering::SeqCst);
    }
}

/// A reopen record as one read of the plane gave it back, or why charter would not read it.
///
/// **A type alias rather than the bare `Result`, so the thing being passed has a name.** What
/// travels from [`Planes::open_if_approved`] and [`Planes::approve_and_open`] down to
/// [`Held::reopen`] is not "a record" — it is *the record this open was decided on*, and the
/// reason it is passed rather than read is charter-app#123. `Reading::record` is where it
/// comes from.
type Read = Result<reopen::Record, std::io::Error>;

/// What the app holds for one open plane: its board, its chats, and where it is.
pub struct Held {
    id: PlaneId,
    root: PathBuf,
    hooks: Hooks,
    /// The window, for a move no hook reports: a chat closed, or a request ignored.
    tell: Teller,
    chats: Chats,
    records: Arc<Records>,
    /// The project's reopen record on its way to disk (#1949): written behind the window where
    /// the registry says so ([`Planes::recording_behind_the_window`]), and every write of it
    /// goes through here, so none lands after a newer one.
    recorded: Arc<WrittenBehind<reopen::Record>>,
    /// What tells the window the plane moved on disk (charter-app#264). None where the
    /// platform would not watch; the panels then read the plane when focused, as they did.
    watch: Mutex<Option<crate::planewatch::Watch>>,
    /// What the sidebar draws, kept current by what the watch and auto-save say moved
    /// (FD-10b, [`purlis_core::planemodel`]). In memory only.
    model: Arc<Mutex<Model>>,
    /// The plane's auto-save (charter-app#296): saves it after a quiet period and when a chat
    /// ends, and fetches what comes in. It runs where the plane's chats run: here, or in the
    /// session host (#2002). Stopped when the plane is let go of.
    autosave: Mutex<Option<Box<dyn crate::autosave::Saver>>>,
    /// What each open chat read of the plane's instructions when it started (charter#369), so
    /// the window can mark a chat still running on ones that have since changed.
    started_on: StartedOn,
    /// Curation prompts waiting for their chat to be ready for them (ADR 0061).
    typed: Arc<crate::curation::Typed>,
    /// The chats being smart-closed (ADR 0064).
    closing: Arc<crate::smartclose::Closing>,
    /// What the app remembers of the tasks its chats dispatched, and who waits on them (#1441).
    tasks: Arc<crate::dispatched::Tasks>,
    /// The window, for each step of a smart close.
    smart: crate::smartclose::Teller,
    /// The chats the person is stopping (#1448).
    stopping: crate::stopping::Stopping,
    /// The window, for each step of a stop.
    stops: crate::stopping::Teller,
    /// The window, for a change in how a chat stands that no file and no hook says (#1484).
    changes: crate::planewatch::Changed,
    /// The window, for each line of a dispatch as it is recorded (#1495).
    activity: crate::activity::Teller,
    /// Told each task's quiet Notice as it comes or goes (#1981).
    quiet: crate::silence::Teller,
    /// The Asks that wait on the person to take a chat's browser over (#2015).
    needs_you: crate::browserneedsyou::NeedsYou,
    /// How many brokered writes each chat has made lately (#1333).
    brokered: purlis_core::brokered::Rate,
    /// The vaults each chat was refused for its persona, until the person answers (#1430).
    vault_refusals: crate::vaultroute::Refusals,
    /// The workspaces each chat asked the person to remove, until they answer (qw356).
    removal_asks: crate::removalasks::Asked,
    /// Each chat's browsers, and what the person said they may reach (#2013).
    browsers: crate::chatbrowsers::ChatBrowsers,
    /// The person's Allows of a persona's tools on a pool, and the runner chats' commands that
    /// wait on one (#2047).
    pool_tools: crate::pooltools::PoolTools,
    /// Where the chats' browsers run: the window's session host, where it has one.
    browser_driver: Option<Arc<dyn crate::chatbrowsers::Driver>>,
    /// Told each time a browser is added to its chats' or taken from them (#2024).
    browsers_listed: Arc<dyn Fn() + Send + Sync>,
    /// Told each chat, by its id, whose browser timeline moved (#2017).
    browser_timeline: Arc<dyn Fn(String) + Send + Sync>,
    /// The changes each chat proposed to purlis's own configuration, until the person answers
    /// (#1975).
    proposals: crate::proposals::Proposals,
    /// The writes to the project its runner chats asked for, held for the person (#2046).
    runner_writes: crate::runnerwrites::RunnerWrites,
    /// The opens and dispatches its runner chats asked for, held for the person.
    runner_dispatches: crate::runnerdispatches::RunnerDispatches,
    /// The chats moving between this machine and a pool's runner (#2048).
    moves: Arc<crate::chatmoves::Moves>,
    /// The persona MCP servers each chat could not start, until the person answers (qw381).
    mcp_failures: Arc<crate::mcpfailed::Failures>,
    /// The chats whose program ended with their session host, until the person answers
    /// (#2007).
    lost_with_the_host: Arc<crate::hostlost::Lost>,
    /// The dispatches waiting on the person, and the dispatch grants made for one chat
    /// (#1437).
    dispatch_grants: crate::dispatchgrants::Store,
    /// The dispatches waiting on the person, as each was asked (#1437): what an Allow starts.
    held_dispatches: crate::handoff::HeldDispatches,
    /// The chats a dispatch was refused for a limit a slot frees, until one does (#1498).
    at_limits: crate::atlimit::AtLimits,
    /// Which chats run with their harness's permission prompts off (#1446).
    unattended: crate::handoff::Unattended,
    /// Which chats were waiting on the person at their last report: what counts a dispatch's
    /// needs-you once per wait (#1452).
    dispatches: crate::dispatches::Waiting,
    /// The files each task's file tools named while this app has run (#1511): in memory only.
    touched_files: crate::taskchanges::Touched,
    /// The daily expiry of what the chats of a dispatch said (#1556). Stopped when the plane
    /// is let go of.
    #[expect(dead_code, reason = "held for its drop, which stops the sweep")]
    talk_sweep: crate::activity::Daily,
    /// This plane, once it is in its `Arc`: what a program's end writes the record through.
    me: Arc<std::sync::OnceLock<std::sync::Weak<Held>>>,
}

/// `tell`, after each change has been applied to `model` — so a reader the window sends on
/// hearing it reads a model that already has it.
fn applying(
    model: Arc<Mutex<Model>>,
    root: PathBuf,
    tell: crate::planewatch::Changed,
) -> crate::planewatch::Changed {
    Arc::new(move |plane, what: crate::planewatch::What| {
        model
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .apply(&root, what.as_deref());
        // The window is told first, and nothing is settled on this thread: where the change
        // may be the project's file, this machine's acceptances of its dispatch grants are
        // settled on a thread of their own, and the window is told again then (#1506).
        tell(plane.clone(), what.clone());
        crate::dispatchgrants::project_moved(&plane, &root, what.as_deref());
    })
}

/// Starts the watch with `start`, then reads `model` again whole: it was read before the watch
/// was live, and a write in between — the CLI, another process, auto-save's launch
/// fast-forward — is one the watch will never tell (#933).
fn caught_up<W>(model: &Mutex<Model>, root: &Path, start: impl FnOnce() -> W) -> W {
    let started = start();
    model
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .apply(root, None);
    started
}

/// Each chat's [`Stamp`], by session, taken as it starts.
type StartedOn = Arc<Mutex<HashMap<u32, Stamp>>>;

/// The stamps, through a poisoned lock too: a panic elsewhere must not cost the marks.
fn lock_started(started_on: &StartedOn) -> MutexGuard<'_, HashMap<u32, Stamp>> {
    started_on.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A chat running on instructions the plane has changed since it started (charter#369): the
/// "control plane updated" the Python said in the transcript, said on the chat's tab instead.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PlaneUpdated {
    pub session: u32,
    /// The files that changed, by their path from the plane root: `CLAUDE.md`,
    /// `personas/steward/persona.md`, …
    pub files: Vec<String>,
    /// Whether its skills changed since it started, in a way its harness does not take in by
    /// itself (#2770): its pane offers a restart on the same conversation.
    pub skills: bool,
    /// The skills a chat started now would get that it lacks, by name (#3694): what its Skills
    /// view marks as new, once its pane's What changed is pressed.
    pub skills_added: Vec<String>,
    /// The skills it has that a chat started now would not get, by name.
    pub skills_gone: Vec<String>,
    /// The skills it has that changed since it started, by name.
    pub skills_edited: Vec<String>,
}

/// Why a task is not started fresh (#1489): what the window's question and the row say. Said
/// only where its brief cannot be handed to it again (#1609, [`Held::start_chat_fresh`]).
pub const A_TASK_IS_NOT_STARTED_FRESH: &str = "This chat is a task, and a fresh start would drop the brief it was given. Restart chat keeps its conversation. To change what it was asked, stop it and ask again.";

/// A project's reopen between its two halves ([`Held::begin_reopen`], [`Held::finish_reopen`]).
pub(crate) struct Reopening {
    begun: crate::restored::Begun,
    /// Where the record came from, where it was a move's or a copy's (V43), and the clone it
    /// names: what the project's app state follows ([`Held::follow_arrival`]).
    arrived: Option<(purlis_core::appstate::Arrived, reopen::CloneSeat)>,
    /// How many chats the record held.
    wanted: usize,
    /// Whether the record was read: only then does a dispatch nothing brings back end (#1513).
    read: bool,
}

impl Held {
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// This machine's store, as the app resolved it at startup, or `None` on a machine that
    /// keeps none: what names a piece log a chat's brokered git action writes (#1335).
    pub fn config(&self) -> Option<&Path> {
        self.records.config.as_deref()
    }

    /// What the sidebar draws, as the model holds it (FD-10b).
    ///
    /// **A plane that is not watched — or not wholly: a folder the platform would not watch —
    /// reads it fresh on every ask**, as the sidebar did before there was a model: nothing
    /// would ever tell the model that part of the plane moved.
    pub fn sidebar_model(&self) -> Model {
        let mut model = self.model.lock().unwrap_or_else(PoisonError::into_inner);
        if !self.trusted(&mut model) {
            *model = Model::read(&self.root);
        }
        model.clone()
    }

    /// Whether `model` (held under its lock) can be served as it is: the plane is watched,
    /// and every folder it reads is (the FD-10 review). A watch that was partial and is whole
    /// again has `model` read again whole first, since a change in the gap was told to nobody.
    fn trusted(&self, model: &mut Model) -> bool {
        let standing = self
            .watch
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(crate::planewatch::Watch::standing);
        match standing {
            None | Some(crate::planewatch::Standing::Partial) => false,
            Some(crate::planewatch::Standing::WholeAgain) => {
                model.apply(&self.root, None);
                true
            }
            Some(crate::planewatch::Standing::Whole) => true,
        }
    }

    /// The model, for the workspace panels to be served from (FD-10c) — or `None` for a plane
    /// that is not watched, or not wholly ([`Self::sidebar_model`]), whose panels are read
    /// fresh from the disk on every ask, as they were before there was a model.
    ///
    /// Held for as long as the guard lives, and the window's own commands wait on it
    /// ([`Self::workspaces_moved`]): a workspace's first ask reads its sections with the
    /// guard dropped (`panels::served`, #1160).
    pub fn watched_model(&self) -> Option<MutexGuard<'_, Model>> {
        let mut model = self.model.lock().unwrap_or_else(PoisonError::into_inner);
        self.trusted(&mut model).then_some(model)
    }

    /// The window wrote these stores itself — a todo recorded, a memory saved, a persona made
    /// — and reads its panels straight after: the model takes the change now rather than when
    /// the watch's batch arrives, which then only reads the same store once more (FD-10c).
    ///
    /// `stores` are plane-relative (`workspaces/alpha/todos`). One the plane format cannot
    /// place reads the model again whole, the watch's own rule for a path it cannot name.
    pub fn wrote(&self, stores: &[String]) {
        let paths: Vec<PathBuf> = stores.iter().map(|store| self.root.join(store)).collect();
        let changes =
            purlis_core::planechange::of_batch(&self.root, paths.iter().map(PathBuf::as_path));
        self.model
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .apply(&self.root, changes.as_deref());
    }

    /// The window moved the plane's workspaces itself — made, renamed or removed one, or set
    /// one LIVE — and reads the sidebar straight after: the model is read again
    /// now rather than when the watch's batch arrives, a quarter of a second or (on a busy
    /// Mac's FSEvents) seconds later.
    ///
    /// **A full re-read under the model's lock**, which the watch also takes to apply a
    /// change: a synchronous command calling this holds the window for both (#1007).
    pub fn workspaces_moved(&self) {
        self.model
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .apply(&self.root, None);
    }

    /// The chats waiting on the person, as the dispatch records count them (#1452).
    pub(crate) fn dispatches(&self) -> &crate::dispatches::Waiting {
        &self.dispatches
    }

    /// The files each task's file tools named while this app has run (#1511).
    pub(crate) fn touched_files(&self) -> &crate::taskchanges::Touched {
        &self.touched_files
    }

    /// The plane's hook channel itself. The app reads a chat's hook state through
    /// [`Self::board`]; this is for a test, and for a sandbox grant's audit (#1342).
    pub fn hooks(&self) -> &Hooks {
        &self.hooks
    }

    /// What the host's board says about each chat (FD-3): the one way the app reads a chat's
    /// hook state, or moves it.
    pub fn board(&self) -> &dyn ChatBoard {
        &self.hooks
    }

    pub fn chats(&self) -> &Chats {
        &self.chats
    }

    /// The curation prompts waiting for their chats to start.
    pub fn typed(&self) -> &crate::curation::Typed {
        &self.typed
    }

    /// The chats being smart-closed (ADR 0064).
    pub fn closing(&self) -> &crate::smartclose::Closing {
        &self.closing
    }

    /// The tasks this project's chats dispatched (#1441).
    pub fn tasks(&self) -> &crate::dispatched::Tasks {
        &self.tasks
    }

    /// [`Self::tasks`], for a wait that outlives its hold on the project.
    pub fn tasks_shared(&self) -> &Arc<crate::dispatched::Tasks> {
        &self.tasks
    }

    /// This project, held weakly: for work that must not keep a closed project open. Dangling
    /// before the project is in its `Arc`, which is before anything can ask.
    pub fn weak(&self) -> std::sync::Weak<Held> {
        self.me.get().cloned().unwrap_or_default()
    }

    /// The cap on each chat's brokered writes (#1333).
    pub fn brokered(&self) -> &purlis_core::brokered::Rate {
        &self.brokered
    }

    /// The dispatches waiting on the person, as each was asked (#1437).
    pub fn held_dispatches(&self) -> &crate::handoff::HeldDispatches {
        &self.held_dispatches
    }

    /// Which chats run with their harness's permission prompts off (#1446).
    pub fn unattended(&self) -> &crate::handoff::Unattended {
        &self.unattended
    }

    /// The vaults each chat was refused for its persona, for the Notice on its tab (#1430).
    pub fn vault_refusals(&self) -> &crate::vaultroute::Refusals {
        &self.vault_refusals
    }

    /// The Asks that wait on the person to take a chat's browser over (#2015).
    pub fn needs_you(&self) -> &crate::browserneedsyou::NeedsYou {
        &self.needs_you
    }

    /// The workspaces each chat asked the person to remove, for the question on its tab
    /// (qw356).
    pub fn removal_asks(&self) -> &crate::removalasks::Asked {
        &self.removal_asks
    }

    /// Each chat's browsers, and what the person said they may reach (#2013).
    pub fn browsers(&self) -> &crate::chatbrowsers::ChatBrowsers {
        &self.browsers
    }

    /// The person's Allows of a persona's tools on a pool, and the runner chats' commands that
    /// wait on one (#2047).
    pub fn pool_tools(&self) -> &crate::pooltools::PoolTools {
        &self.pool_tools
    }

    /// Where the chats' browsers run: none where the window has no session host.
    pub fn browser_driver(&self) -> Option<&Arc<dyn crate::chatbrowsers::Driver>> {
        self.browser_driver.as_ref()
    }

    /// Tells the window where the chats' browsers moved since `before`, a
    /// [`crate::chatbrowsers::ChatBrowsers::listing`] read earlier (#2024).
    pub fn browsers_listed_since(&self, before: u64) {
        // Each browser's window note follows what the window did to it (#2025).
        self.browsers.renote();
        if self.browsers.listing() != before {
            (self.browsers_listed)();
        }
    }

    /// Tells the window chat `chat`'s browser timeline moved (#2017).
    pub fn browser_timeline_moved(&self, chat: &str) {
        (self.browser_timeline)(chat.to_owned());
    }

    /// The changes each chat proposed to purlis's own configuration, for the person to apply
    /// (#1975).
    pub fn proposals(&self) -> &crate::proposals::Proposals {
        &self.proposals
    }

    /// The writes to the project its runner chats asked for, held for the person (#2046).
    pub fn runner_writes(&self) -> &crate::runnerwrites::RunnerWrites {
        &self.runner_writes
    }

    /// The opens and dispatches its runner chats asked for, held for the person.
    pub fn runner_dispatches(&self) -> &crate::runnerdispatches::RunnerDispatches {
        &self.runner_dispatches
    }

    /// The persona MCP servers each chat could not start, for the Notice on its tab (qw381).
    pub fn mcp_failures(&self) -> &crate::mcpfailed::Failures {
        &self.mcp_failures
    }

    /// The chats whose program ended with their session host, for the Notice on each pane
    /// (#2007).
    pub fn lost_with_the_host(&self) -> &crate::hostlost::Lost {
        &self.lost_with_the_host
    }

    /// The dispatches waiting on the person, and the grants made for one chat (#1437).
    pub fn dispatch_grants(&self) -> &crate::dispatchgrants::Store {
        &self.dispatch_grants
    }

    /// This project, as the window names it.
    pub fn plane_id(&self) -> &PlaneId {
        &self.id
    }

    /// The chats a dispatch was refused for a limit a slot frees (#1498).
    pub fn at_limits(&self) -> &crate::atlimit::AtLimits {
        &self.at_limits
    }

    /// The chats the person is stopping (#1448).
    pub fn stopping(&self) -> &crate::stopping::Stopping {
        &self.stopping
    }

    /// **Tells the window that what a chat's row says has changed** (#1484), where the change
    /// is one the app holds and nothing else reports: a task's question to the chat that asked
    /// was opened or answered, or the report it owes was settled. The rows ride the sidebar's
    /// answer, so this says that answer moved and no other
    /// ([`purlis_core::planechange::chats`]); the window reads it again. The board's own moves
    /// are told as they always were.
    pub fn rows_changed(&self) {
        (self.changes)(
            self.id.clone(),
            Some(vec![purlis_core::planechange::chats()]),
        );
    }

    /// **Tells the window the project's working tree moved under it** (qw348): the window's own
    /// Update brought commits in. Everything is read again, as after a fetch that
    /// fast-forwarded (#933): the watch tells the files it moved only where it watches.
    pub fn tree_moved(&self) {
        (self.changes)(self.id.clone(), None);
    }

    /// Tells the window chat `session`'s stop is at `phase`.
    pub fn tell_stop(&self, session: u32, phase: crate::stopping::StopPhase) {
        (self.stops)(crate::stopping::ChatStop {
            plane: self.id.clone(),
            session,
            phase,
        });
    }

    /// Tells the window `line`, which the app has just recorded on a dispatch of this project
    /// (#1495).
    pub fn tell_activity(&self, line: crate::activity::ActivityLine) {
        (self.activity)(crate::activity::ActivityHeard {
            plane: self.id.clone(),
            line,
        });
    }

    /// Tells the window task `session`'s quiet note (#1981): `said`, or none where it came
    /// down.
    pub fn tell_task_quiet(&self, session: u32, said: Option<String>) {
        (self.quiet)(crate::silence::TaskQuiet {
            plane: self.id.clone(),
            session,
            said,
        });
    }

    /// Tells the window chat `session`'s smart close is at `phase`.
    pub fn tell_smart_close(&self, session: u32, phase: crate::smartclose::Phase) {
        // A Smart close that ends without closing takes its "stop the chats below" with it:
        // the answer was for that close, and the next one asks again.
        if !matches!(
            phase,
            crate::smartclose::Phase::Queued | crate::smartclose::Phase::Sent
        ) {
            self.chats.stop_below_on_close(session, false);
        }
        (self.smart)(crate::smartclose::SmartClosing {
            plane: self.id.clone(),
            session,
            phase,
            record: None,
        });
    }

    /// Tells the window chat `session` saved its record, `record`, in a Smart close it held no
    /// pass for: its tab stayed open (`smartclose::Phase::KeptOpen`, #1361).
    pub fn tell_smart_kept_open(&self, session: u32, record: crate::smartclose::SavedRecord) {
        (self.smart)(crate::smartclose::SmartClosing {
            plane: self.id.clone(),
            session,
            phase: crate::smartclose::Phase::KeptOpen,
            record: Some(record),
        });
    }

    /// Tells the window chat `session`'s smart close ended on its record: it was closed.
    pub fn tell_smart_closed(&self, session: u32, record: Option<crate::smartclose::SavedRecord>) {
        (self.smart)(crate::smartclose::SmartClosing {
            plane: self.id.clone(),
            session,
            phase: crate::smartclose::Phase::Closed,
            record,
        });
    }

    /// Sends what a chat's pane sent — the operator's keys, a paste, a mouse report, or the
    /// terminal's own answer to a question the program asked — to the chat's program.
    ///
    /// **Anything but the terminal's own answer drops the chat's curation prompt**, if one is
    /// still waiting to be typed: the operator has begun something of their own, and a paste
    /// landing after it would land in the middle of it (SI-2, Q29). Dropped before the bytes
    /// are sent, under the lock that types a prompt, so the paste is either written before
    /// them or never.
    ///
    /// **And the operator typing cancels the chat's smart close** (ADR 0064) — before the bytes
    /// are sent, under the lock that sends its prompt — unless the chat is asking them something
    /// mid-turn, which their keys are answering. A wheel over the pane is not typing
    /// (`smartclose::typed_by_the_operator`).
    pub fn operator_input(&self, session: u32, bytes: &[u8]) -> Result<(), String> {
        if !crate::curation::only_the_terminal_answering(bytes) {
            self.typed.operator_sent(session);
        }
        if self
            .closing
            .operator_typed(session, bytes, || self.board().glance(session).asking)
        {
            self.tell_smart_close(session, crate::smartclose::Phase::Cancelled);
        }
        // **And the person submitting `/smart-close` into a chat that waits for them is what a
        // smart-close pass is issued on** (#1332): the line read from their own keys, never
        // what the chat reports (#1361). Marked before the bytes are sent, so it is there
        // before the hook the prompt fires can be.
        let submitted = self.closing.person_typed(
            session,
            bytes,
            std::time::Instant::now(),
            self.chats.harness(session),
            || {
                let glance = self.board().glance(session);
                glance.state == purlis_core::state::State::Waiting && !glance.asking
            },
        );
        // And a prompt the person sends a dispatched task is what its report says of them: that
        // they stepped in, and nothing of what they typed (#1442, #1463).
        crate::dispatched::person_typed(self, session, bytes, submitted);
        self.chats.sessions().input(session, bytes)
    }

    /// Every chat this plane has open whose start-time instructions — `CLAUDE.md`, the
    /// harness settings and sub-agents, a persona's charter — have changed since it started,
    /// by session (charter#369). Read from disk each time it is asked: the window asks when the
    /// plane changes, and a chat started after the change read the new ones and is not here.
    pub fn plane_updated(&self) -> Vec<PlaneUpdated> {
        let now = Stamp::of(&self.root);
        // The persona each chat started as, whose charter is the one it read.
        let personas: HashMap<u32, Option<String>> = self
            .chats
            .open_now()
            .into_iter()
            .map(|open| (open.session, open.persona))
            .collect();
        let skills: HashMap<u32, purlis_core::skills::SkillsChanged> =
            self.chats.skills_changed().into_iter().collect();
        let mut updated: Vec<PlaneUpdated> = lock_started(&self.started_on)
            .iter()
            .filter_map(|(session, then)| {
                let persona = personas.get(session).cloned().flatten();
                let files = now.changed_since(then, persona.as_deref());
                let changed = skills.get(session);
                let changed_skills = changed.cloned().unwrap_or_default();
                (!files.is_empty() || changed.is_some()).then_some(PlaneUpdated {
                    session: *session,
                    files,
                    skills: changed.is_some(),
                    skills_added: changed_skills.added,
                    skills_gone: changed_skills.gone,
                    skills_edited: changed_skills.edited,
                })
            })
            .collect();
        updated.sort_by_key(|one| one.session);
        updated
    }

    /// Puts back the chats this plane had open when it was last closed, which STARTS the
    /// programs its record names.
    ///
    /// Said out loud, on standard error, because an operator whose chats did not come back
    /// otherwise has nothing at all to look at — and neither does a CI log.
    ///
    /// The plane starts recording HERE, and not before: from this moment the app has read
    /// what was there, so writing over it is replacing its own answer rather than the
    /// operator's.
    ///
    /// **It does not read the record, and that is charter-app#123's whole fix.** The bytes are
    /// handed in by the caller, which got them from the same read that produced the
    /// contribution the operator was shown (`machine::Contribution::read`). This function used
    /// to open `.charter/app/reopen.json` itself, a moment after `approve_and_open` had
    /// compared what a *different* read of it said — so a write landing between the two was
    /// started without ever having been drawn in a dialog. There is now one read per open, and
    /// no way to write a second one without changing this signature.
    ///
    /// **`choice` is the operator's answer at the launch** (charter-app#250), and every open
    /// that is not one of the launch's projects is [`Choice::ReopenAll`], which is what an open
    /// always did. A fresh start writes the cleared record at once — the choice is the moment
    /// the old one stops being wanted, and a record left as it was would ask again at the next
    /// launch about chats the operator already declined.
    ///
    /// **In two halves** (`crate::restoring`): [`Self::begin_reopen`] reads and registers, and
    /// starts nothing; [`Self::finish_reopen`] starts the chats. This is both, in turn, on the
    /// thread that asks. A launch does the second behind the window ([`Planes::reopen`]), once
    /// its gate lets it (#2004); this is the tests' way in.
    #[cfg(test)]
    pub(crate) fn reopen(&self, size: Size, record: Read, choice: Choice) {
        let begun = self.begin_reopen(record, choice);
        self.finish_reopen(begun, size);
    }

    /// The first half of [`Self::reopen`]: everything it decides from the record, and the
    /// chats registered as coming back. Quick however many chats the record holds, and from
    /// here the window can draw each one's tab (`Chats::restoring_now`).
    pub(crate) fn begin_reopen(&self, record: Read, choice: Choice) -> Reopening {
        self.records.allow();
        // Whether the record was read: only then, or where the person chose to start fresh,
        // does a dispatch nothing brings back end (#1513). A record that could not be read
        // says nothing about which chats are gone.
        let read = record.is_ok();
        // A record another clone or device wrote is a copy's or a move's (V43): a copy's chats
        // get ids of their own before any of them starts, and are written with them once
        // they are back, so no two clones ever hold one chat's id.
        let mut arrived = None;
        let record = record.map(|record| {
            let was = record.clone_seat.clone();
            let (arrival, record) =
                reopen::arrive(record, &self.records.clone_seat, || self.records.known());
            let kind = match arrival {
                reopen::Arrival::Moved => Some(purlis_core::appstate::Arrived::Moved),
                reopen::Arrival::Copied => Some(purlis_core::appstate::Arrived::Copied),
                reopen::Arrival::Same | reopen::Arrival::Legacy => None,
            };
            arrived = kind.zip(was);
            if arrival == reopen::Arrival::Copied {
                tracing::info!(
                    "purlis: plane {} is a copy of another clone; its chats get new ids",
                    self.root.display()
                );
            }
            record
        });
        let record = match record {
            Ok(record) if choice == Choice::StartFresh => {
                let fresh = record.chosen(Choice::StartFresh);
                self.recorded.write_now(fresh.clone());
                tracing::info!(
                    "purlis: plane {}, started fresh as asked; nothing is reopened",
                    self.root.display()
                );
                fresh
            }
            Ok(record) => record,
            Err(why) => {
                // Not the same thing as an empty plane, and an operator told "nothing to
                // reopen" would go looking in the wrong place.
                tracing::warn!(
                    "purlis: the record of what was open in {} was refused ({why}); nothing \
                     is reopened and nothing will be recorded until it is repaired",
                    self.root.display()
                );
                reopen::Record::default()
            }
        };
        // A task that had reported when the app quit is not started again: it is a finished
        // row from its dispatch record (#1485). Before anything is put back.
        let record = crate::finished::put_back_without_the_finished(&self.root, &record);
        let wanted = record.chats.len();
        // A task that had not reported comes back on its conversation, told to carry on, and
        // one that cannot has ended by itself (#1513). **It reads the dispatch records as they
        // are now**: a conversion of those records (#1519) runs when the project is opened
        // (`Planes::open`), before this, and must stay there.
        let begun = crate::restored::begin(self, &record);
        Reopening {
            begun,
            arrived,
            wanted,
            read,
        }
    }

    /// The second half of [`Self::reopen`]: starts the chats [`Self::begin_reopen`]
    /// registered and settles what the launch owes its tasks. It answers when every chat has
    /// been tried.
    pub(crate) fn finish_reopen(&self, reopening: Reopening, size: Size) {
        let Reopening {
            begun,
            arrived,
            wanted,
            read,
        } = reopening;
        // The project's app state follows where its record came from (#3718), before any chat
        // is put back, on this thread and outside the registry's lock: a move carries its
        // drafts and grades, and a copy gets an id of its own and none of the original's.
        if let Some((arrived, was)) = &arrived {
            let _ = purlis_core::appstate::follow_arrival(&self.root, was, *arrived);
        }
        let back = crate::restored::finish(self, begun, size, read).len();
        // What the session host still runs of this project from before this window and no
        // chat put back reattached ends now, so nothing runs with no tab (#2006).
        self.chats.close_what_was_not_reattached();
        // A grade under way when purlis last stopped is recorded as interrupted, its chat ended
        // with the rest above (#3353).
        let _ = crate::skillgrade::record_interrupted(&self.root);
        // And its browsers, each with its chat, pinned past it, or as the person's own; what
        // nothing takes back ends too (#2025).
        crate::chatbrowsers::take_back(self);
        if wanted > 0 {
            tracing::info!(
                "purlis: plane {}, {back} of {wanted} chats back",
                self.root.display()
            );
            // Every chat that did not start stays recorded, a task among them (#1497): a
            // launch ends nothing. A task whose asking chat came back is drawn under it and
            // not across the window (`crate::unstarted`).
            for crate::chats::NotStarted { name, why, .. } in self.chats.would_not_start() {
                tracing::warn!("purlis: {name} did not start ({why}); it is still recorded");
            }
        } else {
            tracing::info!("purlis: plane {}, nothing to reopen", self.root.display());
        }
        // The limits' clock looks now, not at its next round: a session already past its
        // token limit says so at once (#1545).
        crate::overlimit::put_back(&self.id);
    }

    /// Ends a chat and takes it off the board — the tab's ×, or ending its pane — and tells
    /// the window.
    ///
    /// **The telling is the fix for charter-app#247.** The window's needs-you queue is the one
    /// the last `chat-moved` carried, and nothing else ever corrects it. Taking a chat off the
    /// board used to be silent, and the exit that follows a close cannot speak either: by the
    /// time it lands the board no longer has the chat, so `Board::exited` answers "nothing
    /// changed". A chat closed while it was asking for you therefore stayed in the queue, and
    /// in the red counts on its project and workspace tabs, until some other chat moved.
    ///
    /// **Off the board first, and told last.** Off first, so a hook that fires while the
    /// program is being ended finds no chat to move and tells nothing; told last, so a report
    /// the board took just before is told before this, never after it with the chat still
    /// asking. And off and told even when the session had already gone: either way the chat
    /// is gone from the app, and a window left believing otherwise is the defect.
    ///
    /// **A persona chat is never lost by a close** (#1443). One closed before it reported has
    /// its asking chat told it was stopped by the operator, while its record is still here to
    /// name its session record. The chats it asked that have reported and are at rest close
    /// with it, each on its session record ([`crate::handoff::close_reported`]); the ones at
    /// work are the person's choice, asked before the close ([`Self::close_chat_stopping`]),
    /// and where they are kept their reports go to the workspace this chat asked from.
    pub fn close_chat(&self, session: u32) -> Result<(), String> {
        // The chat above it, read before it goes: it is told and looked at once the lock is
        // let go (#1491).
        let above = self.chats.handed_from(session).map(|from| from.chat);
        let closed = {
            let deciding = self.chats.deciding();
            self.close_chat_held(session, &deciding)
        };
        self.stops_carry_on();
        // **The chat that asked is woken as for any report** (#1488, V100-6): purlis's word
        // that the person ended its task was left for its next turn, and where it may be typed
        // a line it is typed one now, the lock let go; and it is looked at as a chat that may
        // have nothing left to wait on (#1491). Nothing where nothing waits for it.
        self.told_of_a_close(above);
        closed
    }

    /// A chat started by chat `above` has closed, and the lock its close held is let go:
    /// `above` is typed purlis's line where something was left for it and it takes one (the
    /// word that the person stopped a task of its), and it and every chat above it are looked
    /// at as chats that may have nothing left to wait on (`crate::dispatched::settle`, #1491).
    /// The close itself looked already, under its lock, where nothing could be typed.
    pub(crate) fn told_of_a_close(&self, above: Option<u32>) {
        if let Some(above) = above
            && !self.chats.ending()
        {
            crate::dispatched::told_or_settled(self, above);
        }
    }

    /// What a stop asked for while a close held the deciding lock is carried out now that it
    /// is let go (`crate::stopping::carry_pending`): ending the next chat takes that lock.
    pub(crate) fn stops_carry_on(&self) {
        if let Some(me) = self.me.get().and_then(std::sync::Weak::upgrade) {
            crate::stopping::carry_pending(&me);
        }
    }

    /// [`Self::close_chat`], under the caller's hold of the lock a dispatch is decided under
    /// and a report is taken under: one hold for the whole close, so nothing is dispatched
    /// from the closing chat, and no report is taken, between its steps.
    pub(crate) fn close_chat_held(
        &self,
        session: u32,
        deciding: &crate::handoff::Deciding<'_>,
    ) -> Result<(), String> {
        use crate::handoff;

        // The person's answer when its Smart close began, where they gave one: what it
        // started while it wrote its record is stopped with it.
        if self.chats.takes_stop_below(session) {
            handoff::stop_below(self, session, deciding);
        }
        // A task closed before it reported was stopped by the person: the chat that asked is
        // told so, in the one word a stop is told in (D-T59-j3). One a stop has ended already
        // is settled, and nothing is said twice.
        if self.chats.owed_task_report(session).is_some() {
            handoff::operator_stopped(self, session, false, true, Vec::new(), deciding);
        }
        // **Settled before the program is ended, whatever became of the word** (D-1443-13,
        // D-T59-j15): the end of a program waits for this lock only while the chat still owes
        // a report, and this close holds the lock while it ends that program. A word that
        // could not be kept is logged above, and is not owed again by a chat that is going.
        if self.chats.owed_task_report(session).is_some() {
            self.chats.owes(session, purlis_core::reopen::Owed::Failed);
        }
        let reported = handoff::reported_by(self, session);
        // **What it queued while the app did not answer is delivered first** (#3902), while the
        // app still knows the chat and its tasks; what cannot be is kept on the dispatch record
        // it is about, and the spool clears its folder as it ends. A chat started again in its
        // own place (`in_its_place`) goes on, and keeps its queue.
        crate::dispatched::sender_ending(self, session);
        let closed = self.end_chat(session);
        if let Some(me) = self.me.get().and_then(std::sync::Weak::upgrade) {
            handoff::close_reported(&me, reported, deciding);
        }
        closed
    }

    /// **Stop them, and close** (#1443): the person's answer to what closing chat `session`
    /// asks. Every chat at work below it is stopped, by the stop every chat is stopped by
    /// (`crate::stopping`, D-T59-j3), and then, where `close`, the chat itself is closed,
    /// **under one hold**: the chat is still running until its own close, and between two
    /// calls it could start another that outlives the answer. Where it is to be smart-closed
    /// instead, the answer is remembered for the close its record brings.
    ///
    /// Answers every chat this closed now: `session`, where it was. The chats below end as
    /// their stops do, each after its one short turn, and the window is told of each.
    pub fn close_chat_stopping(&self, session: u32, close: bool) -> Vec<u32> {
        let above = self.chats.handed_from(session).map(|from| from.chat);
        let mut closed = Vec::new();
        {
            let deciding = self.chats.deciding();
            crate::handoff::stop_below(self, session, &deciding);
            if close {
                if let Err(why) = self.close_chat_held(session, &deciding) {
                    tracing::warn!("purlis: chat {session} did not end cleanly ({why})");
                }
                closed.push(session);
            } else {
                self.chats.stop_below_on_close(session, true);
            }
        }
        self.stops_carry_on();
        if close {
            self.told_of_a_close(above);
        }
        closed
    }

    /// Ends chat `session` and takes it off the board, and no more: [`Self::close_chat`]
    /// without what a close owes a persona chat. For a chat that is being started again in its
    /// own place, which is the same chat going on and not one that ended.
    fn end_chat(&self, session: u32) -> Result<(), String> {
        // **A stop of it is let go of before its program is ended** (#1448, D-T59-j15), and a
        // chat above it that waited for it to end waits no more. First, because the end of
        // that program is heard on another thread, which ends a chat still in a stop
        // (`stopping::exited`) and takes the deciding lock to do it: the lock a close holds
        // while it ends the program. With the stop already let go of, that thread finds
        // nothing to end. What letting go asks for (the chat above beginning its own stop) is
        // carried out by whoever holds the lock this runs under, once it is let go.
        crate::stopping::closed(self, session);
        // Before it is off the board, while its conversation is still known: a dispatch it
        // was working on ends with it (#1452), unless a chat started in its place carries it.
        crate::dispatches::ended(self, session);
        // What it asked the person to do in its browsers ends with them, each with an Update
        // (#2015).
        if self
            .needs_you
            .chat_closed(session, crate::sandboxing::now_secs())
            > 0
        {
            self.chats.asks_may_have_moved();
        }
        // What it asked for that still waited on the person is recorded as gone with it
        // (#1464), while the app still knows the chat, so its rows go just below with the rest.
        self.dispatch_grants.asker_closing(session);
        // The rows of the finished tasks it asked for go with it (#1485, V100-10), unless a
        // chat started again in its place carries on. Their records stay.
        crate::finished::asker_closed(self, session);
        // And where that dispatch gave it a worktree, that is looked at once it has gone
        // (#1453): asked now, while the app still knows the chat.
        let worktree = crate::dispatches::worktree_to_look_at(self, session);
        let gone = self.board().closed(session);
        lock_started(&self.started_on).remove(&session);
        self.typed.forget(session);
        // Closed by the operator's Close, or by its own record: either way nothing is owed.
        self.closing.forget(session);
        self.brokered.forget(session);
        self.vault_refusals.forget(session);
        self.removal_asks.forget(session);
        // What its runner's commands wait on goes with it (#2047).
        self.pool_tools.forget(session);
        // Its browsers end with it (#2013), and leave its rows and its tab (#2024); a pinned
        // one runs on, listed by what the chat was called (#2022).
        let was = self
            .chats
            .recorded_chat(session)
            .map(|chat| chat.name)
            .unwrap_or_default();
        let listing = self.browsers.listing();
        self.browsers
            .forget(self.browser_driver.as_ref(), session, &was);
        self.browsers_listed_since(listing);
        // Its browser timeline goes with it (#2017).
        if let Some(chat) = self
            .chats
            .recorded_chat(session)
            .and_then(|chat| chat.identity.id)
        {
            crate::chatbrowsers::timeline_closed(self, &chat);
        }
        self.proposals.forget(session);
        self.runner_writes.forget(session);
        self.runner_dispatches.forget(session);
        self.mcp_failures.forget(session);
        self.lost_with_the_host.forget(session);
        // Its id, read before it goes: its dispatch grant for "this chat" ends where its
        // sandbox grants do, when no session of the chat is left open (#1437, D-1348-1).
        let id = self
            .chats
            .recorded_chat(session)
            .and_then(|chat| chat.identity.id);
        // While its record is still here: who asked for it is read, so a wait on it says how
        // it ended (#1441).
        // And the chat above it, whatever started it: its held end is looked at once this
        // one has gone (#1491).
        let above = self.chats.handed_from(session).map(|from| from.chat);
        let task_of = self
            .chats
            .handed_from(session)
            .filter(|from| from.mode == purlis_core::reopen::Mode::Task)
            .and_then(|from| Some((from.chat, self.chats.shown_name(session)?)));
        // And how a dispatch record names it, as the chat that asked (#1513).
        let as_asker = crate::dispatches::chat_ref(self, session);
        // And the chat it resumed, whose reports it was handed (#1546).
        let resumed_from = self
            .chats
            .recorded_chat(session)
            .and_then(|chat| chat.identity.resumed_from);
        let closed = self.chats.close(session);
        let ended = id.filter(|id| !self.chats.id_is_open(id));
        self.dispatch_grants.chat_closed(session, ended.as_deref());
        // What it asked for and waited on goes with it, and so does how it was taken to run:
        // a chat started again in its place is marked afresh.
        self.held_dispatches.forget(session);
        self.unattended.forget(session);
        // Nothing else is remembered of it, and a command waiting on it is told (#1441).
        crate::dispatched::closed(self, session, task_of);
        // **Whatever waited on it waits no more** (#1491): the chat that asked for it and
        // every chat above that one may have held an end of turn for it, and a task of its
        // own may have been paused on a question to it. However it went: stopped, closed,
        // ended at its report, or a chat in the middle of a chain. Nothing is typed here.
        if let Some(above) = above {
            crate::dispatched::settle_above(self, above);
        }
        for (task, _) in self.chats.tasks_of(session) {
            crate::dispatched::settle(self, task);
        }
        // Its spool key does not outlive it (V99i): what its hooks spooled is recorded first.
        self.hooks.chat_ended(session);
        // Nothing will prompt it again, so a report waiting for its next turn goes to the
        // workspace it asked from, where the next chat to start reads it (charter-app#259).
        let moved = purlis_core::handback::orphan_kept(&self.root, session);
        // A task's report this chat never read is still owed to it, should the person reopen
        // it (#1513).
        crate::restored::unread_at_close(
            self,
            session,
            as_asker.as_ref(),
            resumed_from.as_deref(),
            &moved,
        );
        let orphaned: Vec<_> = moved.into_iter().map(|(report, _)| report).collect();
        (self.tell)(gone);
        // **A report that was still waiting for this chat now has nowhere to go** (#1448). It
        // is kept for the workspace, and the chat that wrote it, where it is still open, is a
        // needs-you item that says so: nobody else will read what it wrote. purlis's own word
        // that a chat was stopped is no report, and raises none.
        self.reports_have_nowhere_to_go(session, &orphaned);
        // A worktree whose branch is merged goes with its chat (#1453). On a thread of its
        // own, which holds no lock of this app's: it reads the dispatch's record and runs git,
        // and neither a close nor the thread that hears a program end waits for it.
        if let Some(id) = worktree {
            let root = self.root.clone();
            std::thread::spawn(move || {
                crate::dispatches::tidy_closed(&root, &id);
            });
        }
        closed
    }

    /// **Start fresh** (NO-3): chat `session` started again on the plane as it is now
    /// ([`crate::chats::Chats::start_fresh_unless`]), and then the old one ended here, as a close
    /// ends it — off the board, its program gone — so nothing the window does or fails to do can
    /// leave it running. A refused start ends nothing. The new one takes the old one's place in
    /// front.
    ///
    /// **A task starts fresh only when it is handed its brief again** (#1609, lifting #1489's
    /// refusal): a fresh start is a new conversation, and it is still its asker's task, still
    /// owing its report. So it starts only where [`crate::rebrief`] hands it the brief its
    /// dispatch was sent, confirmed against the digest this app kept. Where that cannot be
    /// confirmed (a dispatch from before this launch, a record rewritten since, none kept, or
    /// one that no longer passes a first message's checks), it is refused as before
    /// ([`A_TASK_IS_NOT_STARTED_FRESH`]): nothing is started and nothing ends, from whichever
    /// surface it was asked.
    pub fn start_chat_fresh(&self, session: u32, size: Size) -> Result<u32, String> {
        self.not_while_stopping(session)?;
        let task = self
            .chats
            .handed_from(session)
            .is_some_and(|from| from.mode == purlis_core::reopen::Mode::Task);
        let started = self.chats.start_fresh_unless(session, size, |told| {
            (task && !told.is_some_and(|told| told.handed))
                .then(|| A_TASK_IS_NOT_STARTED_FRESH.to_owned())
        })?;
        // The new one has started, so it is the answer whatever the old one's end says: a
        // program that had already ended answers its close with an error, and the new chat
        // must still reach the window.
        self.in_its_place(session, started);
        Ok(started)
    }

    /// **Restarts chat `session` on its conversation**: the one restart
    /// ([`crate::chats::Chats::restart`]), for a sandbox grant (#1342, spike #1347), for the
    /// persona grants a person allowed (#1362), and for the person's own Restart chat (#1428).
    /// The window asks once the chat's turn has ended. The new session takes the old one's
    /// place, as [`Self::start_chat_fresh`]'s does, and the old one is ended once it has
    /// started; a refused start leaves the old one running.
    ///
    /// Not while the chat waits on a permission prompt: its turn has not ended, and a restart
    /// would answer the prompt for the person. `None` then, and the window asks again at the
    /// chat's next move.
    pub fn restart_chat(&self, session: u32, size: Size) -> Result<Option<u32>, String> {
        if self.asks().asks.iter().any(|ask| ask.session == session) {
            return Ok(None);
        }
        // Nor while the person is stopping it (#1448): the window asks this on its own when a
        // turn ends, which is when a stop sends its prompt, and the stop is the later word.
        if self.stopping.is_stopping(session) {
            return Ok(None);
        }
        let started = self.chats.restart(session, size)?;
        self.in_its_place(session, started);
        Ok(Some(started))
    }

    /// **Starts chat `session` again without the sandbox** (#1342): the person's own choice on a
    /// block's Notice that purlis grants nothing for, for this chat's next run only, and audited
    /// as any opt-out is. In the old one's place, as [`Self::restart_chat`]'s is.
    pub fn restart_chat_without_sandbox(&self, session: u32, size: Size) -> Result<u32, String> {
        self.not_while_stopping(session)?;
        let started = self.chats.restart_without_sandbox(session, size)?;
        self.in_its_place(session, started);
        Ok(started)
    }

    /// **Chat `session`'s project settings no longer pass** (ADR 0067 as amended 2026-10-16):
    /// started again in its place now, by the core, whatever the window is doing. What it asked
    /// the person is refused (the asks were the old run's), a stop of it gives way, and the new
    /// run starts on the person's settings and purlis's. Where it cannot be started again (no
    /// conversation to resume, a start refused), the old run is ended all the same, and why is
    /// answered for its tab.
    pub fn contain_project_settings(&self, session: u32) -> Result<u32, String> {
        // A chat the person is stopping is not started again (#1448): it is ended, as the stop
        // would have ended it.
        if self.stopping.is_stopping(session) {
            crate::dispatched::sender_ending(self, session);
            if let Err(ended) = self.end_chat(session) {
                tracing::warn!("purlis: chat {session} did not end cleanly ({ended})");
            }
            return Err(
                "This project's Claude Code settings changed and a sandboxed chat does not load \
                 them as they are now, so purlis ended this chat, which you were stopping."
                    .to_owned(),
            );
        }
        for ask in self
            .asks()
            .asks
            .into_iter()
            .filter(|ask| ask.session == session)
        {
            if let Err(why) = self.answer_ask(session, &ask.ask, "deny") {
                tracing::warn!("purlis: chat {session}'s ask was not refused ({why})");
            }
        }
        match self.chats.restart_for_project_settings(session) {
            Ok(started) => {
                self.in_its_place(session, started);
                Ok(started)
            }
            Err(why) => {
                let said = format!(
                    "This project's Claude Code settings changed and a sandboxed chat does not \
                     load them as they are now, so purlis ended this chat: it could not be \
                     started again on its conversation ({why}). Start it again to go on."
                );
                // Kept with why among the chats that would not start, so a window that opens
                // later still says so.
                self.chats.keep_aside(session, &said);
                crate::dispatched::sender_ending(self, session);
                if let Err(ended) = self.end_chat(session) {
                    tracing::warn!("purlis: chat {session} did not end cleanly ({ended})");
                }
                Err(said)
            }
        }
    }

    /// **A chat the person is stopping is not started again** (#1448): a restart gives it a new
    /// number, which would take it out of its stop with nobody told.
    fn not_while_stopping(&self, session: u32) -> Result<(), String> {
        if self.stopping.is_stopping(session) {
            return Err(crate::stopping::NOT_STARTED_AGAIN.to_owned());
        }
        Ok(())
    }

    /// [`Self::in_its_place`], for a test in another file that starts the new one by hand.
    /// Not in the app.
    #[cfg(test)]
    pub(crate) fn in_its_place_in_a_test(&self, session: u32, started: u32) {
        self.in_its_place(session, started);
    }

    /// [`Self::in_its_place`], for a chat that moved (#2048): its run where it was ends.
    pub(crate) fn in_its_place_after_a_move(&self, session: u32, started: u32) {
        self.in_its_place(session, started);
    }

    /// The chats moving between this machine and a pool's runner (#2048).
    pub(crate) fn moves(&self) -> &Arc<crate::chatmoves::Moves> {
        &self.moves
    }

    /// `started` takes chat `session`'s place: the old one ends, and the new one is in front
    /// where the old one was.
    fn in_its_place(&self, session: u32, started: u32) {
        self.followed(session, started);
        let in_front = self.chats.front() == Some(session);
        if let Err(why) = self.end_chat(session) {
            tracing::warn!(
                "purlis: chat {session}, started again in its place, did not end cleanly ({why})"
            );
        }
        if in_front {
            self.chats.bring_to_front(Some(started));
        }
        self.stops_carry_on();
    }

    /// **Chat `session` is now `started`**: the same chat, started again under a new number.
    /// What knew it by the old number follows it (#1436): the tasks it dispatched
    /// ([`crate::chats::Chats::followed`]) and the reports already waiting for its next turn,
    /// which would otherwise go to its workspace when the old one closes, a moment from now.
    /// Under the lock a report is taken under, so one arriving meanwhile lands on one side.
    pub fn followed(&self, session: u32, started: u32) {
        let _deciding = self.chats.deciding();
        self.chats.followed(session, started);
        purlis_core::handback::moved(&self.root, session, started);
        // What it asked for and still waited on the person about is answered: the question
        // was the old run's, so the chat is told to ask again (#1437).
        crate::handoff::started_again(self, session, started);
        // And what the app remembers of it as a task and as an asking chat, with the messages
        // waiting for its next turn (#1441, #1442): a cancel, a question, the stepped-in mark.
        crate::dispatched::followed(self, session, started);
    }

    /// A chat `session` handed work to, shown as `from`, has reported back to it, and the
    /// window is told (charter-app#259). Nothing is typed into the chat, and it is no needs-you
    /// item: a report is the asking chat's to read (#1448).
    ///
    /// **It changes what the asking chat's row counts, and interrupts nobody** (#1491,
    /// V100-15): the move is told as one that only counts (`Moved::interrupts`), so no system
    /// notification is sent for a task that finished, whatever else that chat needs the
    /// person for.
    pub fn reported_back(&self, session: u32, from: &str) {
        if let Some(moved) = self.board().reported_back(session, from) {
            (self.tell)(moved.counting_only());
        }
    }

    /// The operator stopped `from`, a chat `session` started, and the window is told (#1448):
    /// the row says so. Nothing is typed into the chat, and it is no needs-you item.
    /// A count on its row and no interruption, as a report is ([`Self::reported_back`]).
    pub fn stopped_below(&self, session: u32, from: &str) {
        if let Some(moved) = self.board().stopped_below(session, from) {
            (self.tell)(moved.counting_only());
        }
    }

    /// **A task chat `session` asked for failed, ended without a report, or did not start**
    /// (#1491, V100-15): a needs-you item on `session` that says which task and why, whatever
    /// it and its other tasks are doing, and the window is told. The one place such an item is
    /// raised: a failed or blocked report and a report purlis wrote for a task that died come
    /// here from the one delivery (`crate::handoff::deliver`), and a dispatch that does not
    /// start is to call it too (#1497).
    pub fn task_failed(&self, session: u32, failed: purlis_core::state::FailedTask) {
        if let Some(moved) = self.board().task_failed(session, failed) {
            (self.tell)(moved);
        }
    }

    /// **The person looked at failure `id` of chat `session`, or cleared its row** (#1491):
    /// the needs-you item for that one task goes, and the window is told. Every other
    /// failure, and whatever else the chat needs them for, stays.
    pub fn task_failure_cleared(&self, session: u32, id: &str) {
        if let Some(moved) = self.board().failure_cleared(session, id) {
            (self.tell)(moved);
        }
    }

    /// The end of chat `session`'s turn that was held is the needs-you item now, and the
    /// window is told (#1491). Only `crate::dispatched::settle` decides that it is.
    pub(crate) fn rested(&self, session: u32) {
        if let Some(moved) = self.board().rested(session) {
            (self.tell)(moved);
        }
    }

    /// A sandbox block's Notice went up for chat `session`: where it waits on its own work, the
    /// wait ends and the person has the next move at once (#3878).
    pub(crate) fn blocked_while_waiting(&self, session: u32) {
        if let Some(moved) = self.board().blocked_while_waiting(session) {
            (self.tell)(moved);
        }
    }

    /// Chat `closed` has closed with `orphaned` still waiting for its next turn: each open chat
    /// it started that had sent its report, and whose report is one of those, needs the person.
    ///
    /// **Not a task that is ending at its report** (#1510, V100-64): its report is on its
    /// dispatch record, where the person reads it, and its program is ended as it would have
    /// been had the chat that asked stayed. What says so is the ledger's own word, set as the
    /// report was delivered, never a record on disk. What is left is a chat that stays open:
    /// one the person started from a tab (D-1443-9), a blocked task, a handoff's chat, and a
    /// task the person has taken up again since it reported.
    fn reports_have_nowhere_to_go(
        &self,
        closed: u32,
        orphaned: &[purlis_core::handback::Handback],
    ) {
        let waiting: Vec<&purlis_core::handback::Handback> = orphaned
            .iter()
            .filter(|report| report.stopped.is_none())
            .collect();
        if waiting.is_empty() {
            return;
        }
        for open in self.chats.open_now() {
            let Some(from) = open
                .from
                .as_ref()
                .filter(|from| from.chat == closed && from.report.reported())
            else {
                continue;
            };
            if self.tasks().ledger().ending(open.session) {
                continue;
            }
            let name = self
                .chats
                .shown_name(open.session)
                .unwrap_or_else(|| open.name.clone());
            if waiting.iter().any(|report| report.from == name) {
                self.needs_the_person(
                    open.session,
                    purlis_core::state::Need::ReportUndelivered {
                        asker: from.name.clone(),
                    },
                );
            }
        }
    }

    /// **The app found that chat `session` needs the person**, for `need`: a needs-you item on
    /// it, and the window is told (#1448). The one way a needs-you item is raised that no hook
    /// of the chat's own raised: a report with nowhere to go today, and a dispatch grant that
    /// is needed next (#1437).
    pub fn needs_the_person(&self, session: u32, need: purlis_core::state::Need) {
        if let Some(moved) = self.board().needs(session, need) {
            (self.tell)(moved);
        }
    }

    /// **purlis's scan refused a commit the app was making for chat `session`** (#1476): a
    /// needs-you item saying `what`, masked, and the window is told, as for a chat's own
    /// refused commit.
    pub fn commit_refused(&self, session: u32, what: &str) {
        if let Some(moved) = self.board().commit_refused(session, what) {
            (self.tell)(moved);
        }
    }

    /// Drops a chat's request for the operator without answering it — the needs-you item's
    /// Ignore — and tells the window (charter-app#248).
    ///
    /// **The core holds it, not the window.** The window's queue is the one the last
    /// `chat-moved` carried and it is replaced whole on every move, so an ignore kept in the
    /// window would be undone by the next move of any chat. On the board it lasts exactly as
    /// long as the request does: the chat's next `Stop` or `Notification` asks again.
    ///
    /// The snapshot is built under the board's hold (`ChatBoard::ignored`), and numbered there,
    /// so a report racing it is put in order by the window rather than by which thread won.
    pub fn ignore_needs_you(&self, session: u32) {
        (self.tell)(self.board().ignored(session));
    }

    /// The permission asks this plane's chats hold open, as the window lists them (HP-6).
    pub fn asks(&self) -> crate::asking::Asking {
        crate::asking::asking(&self.id, self.hooks.asks())
    }

    /// Answers chat `session`'s ask `ask` with `option`, as the operator in the window (HP-6).
    ///
    /// The chat's turn goes on, and the board says so; and a reported task is looked at again:
    /// a bound that passed while the ask was open is owed it from now (#1525).
    ///
    /// Only the prompt of the one who asked it goes (#1644): where the chat and a helper both
    /// asked, answering one leaves the other open. Who asked is read before the answer, since
    /// an answered ask is held no more; one this cannot read clears every prompt, as before.
    pub fn answer_ask(&self, session: u32, ask: &str, option: &str) -> Result<(), String> {
        let asker = ask
            .parse()
            .ok()
            .and_then(|id| self.hooks.asks().asker_of(&id))
            .unwrap_or(purlis_core::harness::hooked::Asker::Unread);
        self.hooks.answer(session, ask, option)?;
        if let Some(moved) = self.board().answered_by(session, &asker) {
            (self.tell)(moved);
        }
        crate::dispatched::end_look(self, session, purlis_core::dispatched::Looked::Moved);
        Ok(())
    }

    /// Whether chat `session` has a permission ask open in the window's needs-you list.
    pub fn asks_open_for(&self, session: u32) -> bool {
        let chat = session.to_string();
        self.hooks
            .asks()
            .pending(std::time::Instant::now())
            .iter()
            .any(|raised| raised.chat == chat)
    }

    /// Writes the record, ends every session, and stops listening — everything a plane holds
    /// in this process, and nothing it has on disk.
    ///
    /// The record is written BEFORE the sessions are ended, because ending them is what makes
    /// there be nothing to write. A write that fails is said and never refused over: the next
    /// launch of this plane reads no record and starts empty, which is worse than a line on
    /// standard error and better than an app that will not close a project.
    ///
    /// `to_update` is the quit that restarts charter to install an update (charter-app#251),
    /// and it is the one writer that sets [`reopen::Record::relaunch_after_update`]. It goes
    /// through the same gated [`Records::write`] as every other, so a plane whose record this
    /// launch never put back is left exactly as it was.
    fn let_go(&self, to_update: bool) {
        // Auto-save first: ending the chats below tells the worker each one ended, and a plane
        // being let go of is saved once, at quit, not by a worker racing that save.
        drop(
            self.autosave
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take(),
        );
        // The last record, after any written behind the window, and the writer ends with it.
        self.chats.write_last(|record| {
            self.recorded.write_last(reopen::Record {
                relaunch_after_update: to_update,
                ..record.clone()
            });
        });
        self.chats.end_all();
        self.hooks.stop();
        drop(
            self.watch
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take(),
        );
    }

    /// The chats working in `workspace` that are mid-turn, by the names the window shows —
    /// what a save of that workspace's repos waits for (charter-app#299, ADR 0051).
    ///
    /// **Mid-turn is the hook state `running`, or a turn that ended waiting on work of its
    /// own** (#3878, #3887: its monitors and background shells may be writing the clone), and
    /// nothing else: charter never reads a harness's output to decide (ADR 0018).
    ///
    /// **Who could be writing the clone is wider than who is filed under the workspace.** A
    /// chat counts when it works in the workspace, in a worktree of one of its clones wherever
    /// `$CHARTER_WORKTREES` or `[plane] worktrees` put that, or anywhere that is in no other
    /// workspace — the plane root, where working in a clone is done from a steward's chat. Only
    /// a chat that is plainly in another workspace is left out.
    pub fn mid_turn_in(&self, workspace: &str) -> Vec<String> {
        let on_disk = purlis_core::workspaces::Plane::open(&self.root);
        self.chats
            .open_now()
            .into_iter()
            .filter(|open| {
                open.cwd
                    .as_deref()
                    .and_then(|cwd| workspace_working_in(&on_disk, cwd))
                    .is_none_or(|there| there == workspace)
            })
            .filter(|open| self.board().glance(open.session).in_its_turn())
            .map(|open| open.label.clone().unwrap_or(open.name))
            .collect()
    }

    /// [`Held::mid_turn_in`] for every workspace of the plane, keyed by workspace — asked
    /// before a quit ends the chats, so the turns it cuts off are known (ADR 0051).
    pub fn mid_turn_everywhere(&self) -> HashMap<String, Vec<String>> {
        purlis_core::workspaces::Plane::open(&self.root)
            .workspaces()
            .unwrap_or_default()
            .into_iter()
            .map(|workspace| {
                let busy = self.mid_turn_in(&workspace);
                (workspace, busy)
            })
            .filter(|(_, busy)| !busy.is_empty())
            .collect()
    }

    /// The plane's auto-save, taken from it: the quit stops it ([`Planes::let_go_of_all`]).
    fn take_saver(&self) -> Option<Box<dyn crate::autosave::Saver>> {
        self.autosave
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// Tell the plane's auto-save worker something, if it has one.
    pub fn poke_autosave(&self, poke: crate::autosave::Poke) {
        if let Some(worker) = self
            .autosave
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            let _ = worker.poker().send(poke);
        }
    }
}

/// The workspace a chat working in `cwd` could be writing a clone of: the one `cwd` is in, or —
/// for a linked worktree anywhere on disk — the one its main clone is in. `None` for a
/// directory in no workspace at all.
pub(crate) fn workspace_working_in(
    plane: &purlis_core::workspaces::Plane,
    cwd: &Path,
) -> Option<String> {
    plane.workspace_of(cwd).or_else(|| {
        cwd.ancestors()
            .find_map(purlis_core::plane::main_worktree_of)
            .and_then(|main| plane.workspace_of(&main))
    })
}

/// Told whenever a chat moves, whichever plane it is in. The event carries its plane, so one
/// teller serves them all.
pub type Teller = Arc<dyn Fn(Moved) + Send + Sync + 'static>;

/// This machine's store, where the operator's pins and remembered projects are kept: the part
/// of [`Planes`] a command can carry off the window's thread (#1949).
pub(crate) struct MachineStore {
    config: Option<PathBuf>,
}

impl MachineStore {
    /// Pins or unpins a project, or one of its workspaces, in the machine store.
    ///
    /// **This one refuses rather than shrugging**, unlike [`Planes`]' `remember` and
    /// `record_approval`, and the difference is who is waiting. Those two are charter's own bookkeeping
    /// on a path the operator is already walking; this is a button they just pressed, and a
    /// pin that silently did not happen is a control that does not work. So the store's own
    /// sentence travels back — including the one a machine with no store gives
    /// (`Unsupported`, which on Windows is ADR 0031's refusal and is the honest answer to
    /// "pin this": charter cannot, here, and says so).
    pub fn pin(&self, root: &Path, workspace: Option<&str>, pinned: bool) -> Result<(), String> {
        let plane = root.to_path_buf();
        self.arranging(|store| match workspace {
            Some(name) => store.pin_workspace(&plane, name, pinned).map(drop),
            None => store.pin(&plane, pinned).map(drop),
        })
    }

    /// Puts a project's pinned workspaces in the order the operator dragged them into (SI-6),
    /// in the machine store beside the pins themselves — refusing as [`MachineStore::pin`] does, for
    /// the same reason: it is a drag the operator just finished.
    pub fn arrange_workspaces(&self, root: &Path, order: &[String]) -> Result<(), String> {
        let plane = root.to_path_buf();
        let order: Vec<&str> = order.iter().map(String::as_str).collect();
        self.arranging(|store| store.arrange_workspaces(&plane, &order).map(drop))
    }

    /// Pins a workspace back at the place it had among the project's workspace pins: the Undo
    /// of an unpin made in Settings › You › This machine (ST-2), refusing as [`MachineStore::pin`]
    /// does.
    pub fn pin_workspace_at(&self, root: &Path, workspace: &str, at: usize) -> Result<(), String> {
        let plane = root.to_path_buf();
        self.arranging(|store| store.pin_workspace_at(&plane, workspace, at).map(drop))
    }

    /// Re-points a remembered project that is gone at the folder the operator picked (NO-5),
    /// once the core has checked a project is there, and answers the project found.
    pub fn locate(&self, gone: &Path, picked: &Path) -> Result<PathBuf, String> {
        let config = self.store_home("re-point a project")?;
        machine::locate_project(config, gone, picked)
    }

    /// Forgets a project on this machine (ST-2): its recent, approval, pins and tabs. A
    /// project that is gone from the disk is forgotten the same way.
    pub fn forget(&self, root: &Path) -> Result<(), String> {
        let config = self.store_home("forget a project")?;
        match machine::forget_project(config, root) {
            Ok(true) => {
                // A forgotten project's mark that a runner would not hold it goes (#2846).
                crate::notheld::held(root);
                Ok(())
            }
            // Said, never dropped (#1240 F3): a row whose path the store does not hold, as
            // one that is not UTF-8 comes back from the window, would otherwise read as done.
            Ok(false) => Err(
                "Nothing changed: purlis does not remember that project on this machine."
                    .to_owned(),
            ),
            Err(why) => Err(format!("purlis could not forget that project: {why}")),
        }
    }

    /// Revokes this machine's approval of a project (ST-2): the next open asks again.
    pub fn revoke(&self, root: &Path) -> Result<(), String> {
        let config = self.store_home("revoke an approval")?;
        match machine::revoke_approval(config, root) {
            Ok(true) => Ok(()),
            // Said, never dropped (#1240 F3), as a forget that changed nothing is.
            Ok(false) => {
                Err("Nothing changed: this machine holds no approval of that project.".to_owned())
            }
            Err(why) => Err(format!("purlis could not revoke that approval: {why}")),
        }
    }

    /// The config home the machine store is in, or why there is none to `act` in.
    fn store_home(&self, act: &str) -> Result<&Path, String> {
        self.config.as_deref().ok_or_else(|| {
            format!("purlis has no config home on this machine, so it cannot {act}.")
        })
    }

    /// Changes how the operator arranged things in the machine store, and hands back the
    /// store's own refusal whole: the one path [`MachineStore::pin`] and [`MachineStore::arrange_workspaces`]
    /// both take, so a store that cannot be written says the same sentence for either.
    fn arranging(
        &self,
        act: impl FnOnce(&mut machine::Store) -> Result<(), String>,
    ) -> Result<(), String> {
        let Some(config) = self.config.as_deref() else {
            return Err(
                "purlis has no config home on this machine, so it cannot remember a pin."
                    .to_owned(),
            );
        };
        // The store's own refusal, out of the closure: `update` answers an `io::Error`, and
        // wrapping a bound the operator can act on ("unpin one first") in one would turn a
        // sentence they can follow into a sentence about a file.
        let mut refused = None;
        machine::update(config, |store| refused = act(store).err())
            .map_err(|why| format!("purlis could not write the pin down: {why}"))?;
        match refused {
            Some(why) => Err(why),
            None => Ok(()),
        }
    }
}

/// **A file written behind the window** (#1949): one writer, which always writes the newest
/// value it was handed, so a command hands its change over and the window goes on drawing, and
/// an older value never lands after a newer one. The window's tabs in this machine's store
/// (`window_holds_planes`, at every switch) and a project's reopen record (each tab moved) are
/// written this way.
pub(crate) struct WrittenBehind<T> {
    slot: Mutex<Behind<T>>,
    moved: Condvar,
    write: Box<dyn Fn(T) + Send + Sync>,
}

/// What the writer has to do: the newest value not yet written, whether a write is under way,
/// whether the writer has started, and whether it is done for good.
struct Behind<T> {
    newest: Option<T>,
    writing: bool,
    started: bool,
    done: bool,
}

impl<T: Send + 'static> WrittenBehind<T> {
    /// A writer that writes each value with `write`.
    pub(crate) fn new(write: impl Fn(T) + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            slot: Mutex::new(Behind {
                newest: None,
                writing: false,
                started: false,
                done: false,
            }),
            moved: Condvar::new(),
            write: Box::new(write),
        })
    }

    fn slot(&self) -> MutexGuard<'_, Behind<T>> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hands `value` over to be written, in place of any value still waiting.
    pub(crate) fn hand(self: &Arc<Self>, value: T) {
        let mut slot = self.slot();
        if slot.done {
            return;
        }
        slot.newest = Some(value);
        if !slot.started {
            let writer = Arc::clone(self);
            slot.started = std::thread::Builder::new()
                .name("purlis-written-behind".to_owned())
                .spawn(move || writer.write_each())
                .is_ok();
        }
        let started = slot.started;
        drop(slot);
        self.moved.notify_all();
        // No thread to write it: written here, as it always was.
        if !started {
            self.flush();
        }
    }

    /// The writer: each newest value, as it comes, until the last one is written.
    fn write_each(&self) {
        loop {
            let value = {
                let mut slot = self.slot();
                loop {
                    if let Some(newest) = slot.newest.take() {
                        slot.writing = true;
                        break newest;
                    }
                    if slot.done {
                        return;
                    }
                    slot = self
                        .moved
                        .wait(slot)
                        .unwrap_or_else(PoisonError::into_inner);
                }
            };
            (self.write)(value);
            self.slot().writing = false;
            self.moved.notify_all();
        }
    }

    /// Writes what is waiting now and answers once it is on disk, after any write under way.
    pub(crate) fn flush(&self) {
        let mut slot = self.slot();
        while slot.writing {
            slot = self
                .moved
                .wait(slot)
                .unwrap_or_else(PoisonError::into_inner);
        }
        // Under the lock, so the writer takes nothing older after it.
        if let Some(value) = slot.newest.take() {
            (self.write)(value);
        }
    }

    /// Writes `value` now, in place of whatever is waiting, after any write under way, and
    /// answers once it is on disk: nothing handed over before it lands after it.
    pub(crate) fn write_now(&self, value: T) {
        let mut slot = self.slot();
        while slot.writing {
            slot = self
                .moved
                .wait(slot)
                .unwrap_or_else(PoisonError::into_inner);
        }
        slot.newest = None;
        (self.write)(value);
    }

    /// [`Self::write_now`], as the last value: the writer ends, and nothing handed over after it
    /// is written.
    pub(crate) fn write_last(&self, value: T) {
        let mut slot = self.slot();
        while slot.writing {
            slot = self
                .moved
                .wait(slot)
                .unwrap_or_else(PoisonError::into_inner);
        }
        slot.newest = None;
        slot.done = true;
        (self.write)(value);
        drop(slot);
        self.moved.notify_all();
    }
}

/// The window's tabs on their way to this machine's store: the config home and the windows.
pub(crate) type ArrangedBehind = WrittenBehind<(PathBuf, Vec<machine::Window>)>;

/// Writes `windows` into the store under `config`. Never worth refusing anything over: an
/// arrangement that could not be filed costs the next launch its tabs.
fn write_arrangement(config: &Path, windows: Vec<machine::Window>) {
    if let Err(why) = machine::update(config, move |store| store.windows = windows)
        && why.kind() != std::io::ErrorKind::Unsupported
    {
        tracing::warn!(
            "purlis: the projects this window holds were not written down ({why}); the next \
             launch will not put them back"
        );
    }
}

/// A plane [`Planes::closing`] took out of the registry, still to be let go of.
pub(crate) struct Closing {
    plane: PlaneId,
    held: Arc<Held>,
    released: Released,
    closed: Closed,
    /// Taken while the plane is let go of: one close at a time, a while ([`LettingGo`]).
    one_at_a_time: Arc<LettingGo>,
    /// The project's turn, let go of last, when everything above is done.
    _turn: OpenTurn,
}

impl Closing {
    /// Lets go of the plane: its record is written, its chats ended, its hook socket released,
    /// and every window drawing it told. Only then may it be opened again.
    pub(crate) fn finish(self) {
        let _one_at_a_time = self.one_at_a_time.enter(LETTING_GO_WAITS);
        self.held.let_go(false);
        (self.released)(&self.held.root);
        (self.closed)(&self.plane);
    }
}

/// **Closes let go of their projects one at a time, a while** (#1949, #2029's review): as they did
/// on the window's thread, rather than all ending their chats at once on a machine already busy.
/// A close waits for the one before it at most [`LETTING_GO_WAITS`], then goes on beside it and
/// says so: one close stuck for good never holds every later one. Once the quit has begun none
/// waits, so every close gets its share of the quit's own wait.
#[derive(Default)]
pub(crate) struct LettingGo {
    /// How many closes are letting go now.
    under_way: Mutex<usize>,
    /// Told each time one is done.
    done: Condvar,
    /// Set as the quit begins: closes stop waiting for each other, and an open that ends after
    /// it lets go of the project it built instead of holding it.
    quitting: AtomicBool,
}

impl LettingGo {
    /// This close's turn: once none is under way, or `within` has passed, or the quit began.
    fn enter(self: &Arc<Self>, within: std::time::Duration) -> LettingGoTurn {
        let until = std::time::Instant::now() + within;
        let mut under_way = self
            .under_way
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        while *under_way > 0 && !self.quitting.load(Ordering::SeqCst) {
            let left = until.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                tracing::warn!(
                    "purlis: a project's close has not let go of it within {}s; the next close \
                     goes on beside it",
                    within.as_secs()
                );
                break;
            }
            under_way = self
                .done
                .wait_timeout(under_way, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        *under_way += 1;
        LettingGoTurn {
            of: Arc::clone(self),
        }
    }

    /// The quit has begun: every close waiting for another goes on now.
    fn quit(&self) {
        self.quitting.store(true, Ordering::SeqCst);
        let _under_way = self
            .under_way
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        self.done.notify_all();
    }

    fn quitting(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }
}

/// One close's turn at letting go ([`LettingGo::enter`]); dropped, it lets the next one go on.
pub(crate) struct LettingGoTurn {
    of: Arc<LettingGo>,
}

impl Drop for LettingGoTurn {
    fn drop(&mut self) {
        let mut under_way = self
            .of
            .under_way
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *under_way = under_way.saturating_sub(1);
        self.of.done.notify_all();
    }
}

/// The planes this process holds, by root.
pub struct Planes {
    tell: Teller,
    /// The `charter` binary a hook runs and the plugin a chat loads, where the app found them.
    /// Every plane arms with the same ones: they are a property of this build, not of a project.
    shipped: crate::Shipped,
    /// Where this machine's store lives, or none on a machine with no config home at all.
    config: Option<PathBuf>,
    /// Whether a project's chats are started behind the window, on a thread of their own,
    /// once its record is registered ([`Self::restoring_behind_the_window`]). The app's own
    /// registry does; one a test makes starts them in turn, so what it asks next is settled.
    restores_behind: bool,
    /// Whether a project's reopen record is written behind the window
    /// ([`Self::recording_behind_the_window`]). The app's own registry does; one a test makes
    /// writes it as each change is made, so what it reads next is on disk.
    records_behind: bool,
    open: Mutex<HashMap<PlaneId, Arc<Held>>>,
    /// Told when a handoff has opened a chat in any plane (charter-app#204).
    arrivals: crate::handoff::Arrivals,
    /// Told when any plane changes on disk (charter-app#264).
    changes: crate::planewatch::Changed,
    /// Told when auto-save saved a plane (charter-app#343).
    saves: crate::autosave::Saved,
    /// Ends the session host, for a quit it did not answer in time (#2002).
    end_the_host: Arc<dyn Fn() + Send + Sync>,
    /// Told the root of each plane let go of, so the one watch per repo stops listening to its
    /// clones (FD-11).
    released: Released,
    /// Told each plane [`Planes::close`] let go of, so every window drawing it takes it out
    /// (#1242).
    closed: Closed,
    /// Told when a harness is started by hand in a shell tab of any plane (ADR 0062).
    by_hand: hooks::ByHandTeller,
    /// Told each file a chat's tool touched, confined and rated (FM-6).
    touches: hooks::TouchTeller,
    /// Told what a working chat is doing, each time its one line changes (#1493).
    doings: crate::doing::Teller,
    skill_runs: crate::skillrun::Teller,
    /// The app's executor, which asks each chat's router extension at its start (SK-15b): none
    /// for a registry a test makes, whose chats are then routed by the built-in router.
    routers: Option<Arc<purlis_core::executor::Executor>>,
    /// Told each sandbox block a chat's hook found, once kept in the network record (#1338,
    /// #1662).
    blocks: hooks::BlockTeller,
    /// The window's tabs on their way to this machine's store (#1949).
    arranged: Arc<ArrangedBehind>,
    /// Told how many hosts a chat's proxy refused past the ones it names (#1353).
    refused_more: hooks::RefusedMoreTeller,
    /// Told each chat whose project's Claude Code settings no longer pass (ADR 0067 as amended
    /// 2026-10-16).
    project_settings: crate::projectsettings::Teller,
    /// The window, for each chat move's stage (#2048).
    moves_told: Arc<dyn Fn(PlaneId, crate::chatmoves::ChatMove) + Send + Sync>,
    /// Told each vault a chat was refused for its persona (#1430).
    vault_refused: crate::vaultroute::Teller,
    /// Told each Ask about a chat's browser that ended with the browser (#2015).
    browser_asks_ended: crate::browserneedsyou::Teller,
    /// Told each persona MCP server a chat could not start (qw381).
    mcp_failed: crate::mcpfailed::Teller,
    /// Told each chat whose browsers ended because the person narrowed its reach (#2013).
    browsers_ended: crate::chatbrowsers::Teller,
    /// Told each project whose chats' browsers were added to or taken from (#2024).
    browsers_listed: crate::chatbrowsers::ListedTeller,
    /// Told each chat whose browser timeline moved, with its project (#2017).
    browser_timeline: crate::chatbrowsers::TimelineTeller,
    /// Where the chats' browsers run (#2013): the window's session host, once it is given.
    browser_driver: Option<Arc<dyn crate::chatbrowsers::Driver>>,
    /// Told each chat whose program ended with its session host (#2007).
    lost_with_the_host: crate::hostlost::Teller,
    /// Told each plane's chats that start or stop reconnecting to their runner (#2496).
    reconnecting: crate::reconnecting::Teller,
    /// Told each step of a smart close in any plane (ADR 0064).
    smart: crate::smartclose::Teller,
    /// Told each step of a stop in any plane (#1448).
    stops: crate::stopping::Teller,
    /// Told each line of a dispatch in any plane as it is recorded (#1495).
    activity: crate::activity::Teller,
    /// Told each task's quiet Notice as it comes or goes (#1981).
    quiet: crate::silence::Teller,
    /// Told a plane's permission asks each time they change (HP-6).
    asks: crate::asking::Teller,
    /// The launch's question and its answer — see [`Relaunching`].
    relaunching: Mutex<Relaunching>,
    /// The kill switch (OV-1): one for every plane this process holds, so a plane opened after
    /// a stop starts nothing either.
    kill_switch: Arc<crate::killswitch::KillSwitch>,
    /// Makes each plane's session host (FD-3): [`Sessions`] in the app, told where its chats
    /// report.
    hosting: Hosting,
    /// Whether the session host owns each plane's hook channel, where the window would bind
    /// it (#2000): the window then hears what the host hands on.
    hooks_in_the_host: bool,
    /// The host's event log (FD-9), shared by every project: one writer per device. None until
    /// the app opens it, and on a machine that has no data home.
    events: Option<hooks::Events>,
    /// Collects a plane's month-old per-session files as it opens (SC-7):
    /// [`purlis_core::retention::on_open`], or a test's stand-in.
    sweep: Sweep,
    /// The projects an open is between its sweep and holding them (#1027), or a close between
    /// taking them out and having let go of them (#1949), so an open of one waits for it: see
    /// [`Planes::opening`].
    turns: Arc<Turns>,
    /// Held by a close while it lets go of its project, so closes do so one at a time, as they
    /// did on the window's thread: nine closes at once end every chat of nine projects at once
    /// on a machine that is busy already, for nothing a person would see sooner.
    letting_go: Arc<LettingGo>,
    /// How long the quit waits for the opens and closes under way ([`QUIT_WAITS_FOR_TURNS`]).
    quit_waits: std::time::Duration,
    /// This machine's network record (#1662), shared by every project. None until the app
    /// opens it, and on a machine that has no data home: then nothing is recorded or listed.
    network: Option<purlis_core::sandboxblock::record::Record>,
    /// This machine's config home, where its pools are kept (#2679): `None` in a test, where
    /// no workspace runs on a pool.
    pools_in: Option<PathBuf>,
    /// What putting back a project's chats waits on, if anything (#2004).
    gate: Option<Gate>,
}

/// The projects an open or a close is under way for, one at a time each ([`OpenTurn`]).
#[derive(Default)]
struct Turns {
    /// Each project with a turn held, and whether an open or a close holds it.
    held: Mutex<HashMap<PlaneId, Turn>>,
    /// Told each time a turn ends, for one of the same project waiting on it.
    moved: Condvar,
}

/// What holds a project's turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Turn {
    Open,
    Close,
}

impl Turns {
    fn held(&self) -> MutexGuard<'_, HashMap<PlaneId, Turn>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The turn at project `id`, for `turn`, once nothing else holds it.
    fn take(self: &Arc<Self>, id: &PlaneId, turn: Turn) -> OpenTurn {
        let mut held = self.held();
        while held.contains_key(id) {
            held = self
                .moved
                .wait(held)
                .unwrap_or_else(PoisonError::into_inner);
        }
        held.insert(id.clone(), turn);
        OpenTurn {
            turns: Arc::clone(self),
            id: id.clone(),
        }
    }

    /// Nothing, once nothing holds the turn at `id` within `within`, waiting for it until then;
    /// or what still holds it.
    fn free_within(&self, id: &PlaneId, within: std::time::Duration) -> Option<Turn> {
        let until = std::time::Instant::now() + within;
        let mut held = self.held();
        while let Some(turn) = held.get(id).copied() {
            let left = until.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return Some(turn);
            }
            held = self
                .moved
                .wait_timeout(held, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        None
    }

    /// Waits until no open or close holds a turn, or `within` has passed: how many still do
    /// then.
    fn all_done_within(&self, within: std::time::Duration) -> usize {
        let until = std::time::Instant::now() + within;
        let mut held = self.held();
        loop {
            let under_way = held.len();
            let left = until.saturating_duration_since(std::time::Instant::now());
            if under_way == 0 || left.is_zero() {
                return under_way;
            }
            held = self
                .moved
                .wait_timeout(held, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// One open of a project, from before its sweep until it holds the project or finds it held,
/// or one close, from taking it out of the registry until it has let go of it
/// ([`Planes::opening`]). Dropped, it lets the next open of that project go on.
struct OpenTurn {
    turns: Arc<Turns>,
    id: PlaneId,
}

impl Drop for OpenTurn {
    fn drop(&mut self) {
        self.turns.held().remove(&self.id);
        self.turns.moved.notify_all();
    }
}

/// How long an open the window asks for waits for a close or another open of the same project,
/// before it says so instead (#1949).
const CLOSE_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// What an open says when a close of the same project has not let go of it within
/// [`CLOSE_WAIT`].
const STILL_CLOSING: &str =
    "purlis is still letting go of this project, ending its chats. Open it again in a moment.";

/// What an open says when another open of the same project, from another window or a link,
/// has not finished within [`CLOSE_WAIT`].
const STILL_OPENING: &str =
    "purlis is still opening this project for another window. Open it again in a moment.";

/// How long a close waits for the one before it to let go of its project, before it lets go of
/// its own beside it (#2029's review): one close stuck for good holds the next only this long.
const LETTING_GO_WAITS: std::time::Duration = std::time::Duration::from_secs(3);

/// How long the quit waits for the opens and closes still under way (#1949). A close writes its
/// project's last record and ends its chats, and the process ending under it would cut both off;
/// an open would hold its project after the quit emptied the registry, and nothing would let go
/// of it.
const QUIT_WAITS_FOR_TURNS: std::time::Duration = std::time::Duration::from_secs(10);

/// What collects a plane's month-old per-session files as [`Planes::open`] opens it.
type Sweep = Arc<dyn Fn(&Path) + Send + Sync>;

/// Told the root of a plane the app let go of.
pub type Released = Arc<dyn Fn(&Path) + Send + Sync>;

/// Told a plane [`Planes::close`] let go of (#1242).
pub type Closed = Arc<dyn Fn(&PlaneId) + Send + Sync>;

/// The event every window drawing a plane is sent when the core has closed it (#1242). The
/// window takes the project's tab out, with its chats and views, whoever asked the close.
pub(crate) const CLOSED: &str = "plane-closed";

/// What `plane-closed` carries.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaneClosed {
    pub plane: PlaneId,
}

/// Makes a plane's session host, given where its chats are to report and where the plane is.
pub type Hosting = Arc<dyn Fn(Option<Reporting>, &Path) -> Box<dyn SessionHost> + Send + Sync>;

/// What putting back a project's chats waits on (#2004): `Ok` to start them, or why not.
pub type Gate = Arc<dyn Fn() -> Result<(), String> + Send + Sync>;

/// Where the launch's question stands (charter-app#250): what it holds back until the
/// operator answers, and what the answer was.
///
/// **Nothing the launch would put back starts before the answer.** The launch's own plane is
/// attached at once — its board, its socket, its tab — but its record waits here as `owed`,
/// and the window restores the other projects only after it has answered. The answer is taken
/// once: a webview that reloads asks again, and a second put-back would be a second copy of
/// every chat.
#[derive(Default)]
struct Relaunching {
    /// The plane the launch's working directory named, attached and not yet put back.
    owed: Option<PlaneId>,
    /// Whether the operator has answered. From then on there is no question to ask.
    decided: bool,
    /// The roots the answer said to start fresh, each taken the first time it is put back.
    /// Empty after "Reopen all", which is what every open did before there was a question.
    fresh: HashSet<PathBuf>,
    /// Whether this launch follows a restart to update (charter-app#251): it took the word
    /// [`Planes::let_go_of_all_to_update`] left. A record's own
    /// [`reopen::Record::relaunch_after_update`] counts only when this is set, which is what
    /// keeps a flag left in a plane nobody reopened from speaking at a later launch.
    after_update: bool,
    /// What the person answered before that restart (#1078): this launch's answer, so the
    /// question is not asked again ([`Planes::relaunch_ask`]) and [`Planes::relaunch`] takes
    /// it over the window's "nothing to ask" default.
    chosen: Option<Choice>,
}

/// What a launch would put back, for the question it asks before putting any of it back.
#[derive(Debug)]
pub struct Relaunchable {
    /// Every project with a chat or a view tab to put back, the launch's own first.
    pub projects: Vec<Waiting>,
    /// Whether any of their records was written by a quit that restarted charter to install an
    /// update (charter-app#251), which changes what the question says.
    pub after_update: bool,
}

/// One project's share of [`Relaunchable`].
#[derive(Debug)]
pub struct Waiting {
    pub root: PathBuf,
    pub chats: usize,
    pub views: usize,
}

impl Planes {
    /// A registry holding nothing, which is what the app comes up as before a plane is
    /// opened — and stays as, perfectly happily, when there is no plane to open.
    pub fn telling(tell: Teller, shipped: crate::Shipped, config: Option<PathBuf>) -> Self {
        Self {
            tell,
            shipped,
            kill_switch: crate::killswitch::KillSwitch::kept_in(config.clone()),
            config,
            restores_behind: false,
            records_behind: false,
            open: Mutex::new(HashMap::new()),
            // Nobody to tell yet. A registry with no window still opens a handed-off chat;
            // it simply has no strip to put it on until one asks what is open.
            arrivals: Arc::new(|_| {}),
            changes: Arc::new(|_, _| {}),
            saves: Arc::new(|_, _| {}),
            end_the_host: Arc::new(|| {}),
            released: Arc::new(|_| {}),
            closed: Arc::new(|_| {}),
            by_hand: Arc::new(|_| {}),
            touches: Arc::new(|_| {}),
            doings: Arc::new(|_| {}),
            skill_runs: Arc::new(|_| {}),
            routers: None,
            blocks: Arc::new(|_| {}),
            arranged: ArrangedBehind::new(|(config, windows)| write_arrangement(&config, windows)),
            refused_more: Arc::new(|_| {}),
            project_settings: Arc::new(|_| {}),
            moves_told: Arc::new(|_, _| {}),
            vault_refused: Arc::new(|_| {}),
            browser_asks_ended: Arc::new(|_| {}),
            mcp_failed: Arc::new(|_| {}),
            browsers_ended: Arc::new(|_| {}),
            browsers_listed: Arc::new(|_| {}),
            browser_timeline: Arc::new(|_, _| {}),
            browser_driver: None,
            lost_with_the_host: Arc::new(|_| {}),
            reconnecting: Arc::new(|_| {}),
            smart: Arc::new(|_| {}),
            stops: Arc::new(|_| {}),
            activity: Arc::new(|_| {}),
            quiet: Arc::new(|_| {}),
            asks: Arc::new(|_| {}),
            relaunching: Mutex::new(Relaunching::default()),
            hosting: Arc::new(|reporting, _: &Path| Box::new(Sessions::reporting_to(reporting))),
            hooks_in_the_host: false,
            events: None,
            sweep: Arc::new(|root| {
                purlis_core::retention::on_open(root, std::time::SystemTime::now());
            }),
            turns: Arc::default(),
            letting_go: Arc::default(),
            quit_waits: QUIT_WAITS_FOR_TURNS,
            network: None,
            pools_in: None,
            gate: None,
        }
    }

    /// Puts back no project's chats before `gate` lets it (#2004): what a window whose chats run
    /// in its own process waits on, so a session host a crashed window left ends the chats it
    /// kept first ([`crate::sessionhost::SessionHost::before_chats_are_put_back`]). A gate that
    /// refuses puts nothing back and leaves the record as it is, for the next launch.
    pub fn putting_back_chats_after(mut self, gate: Gate) -> Self {
        self.gate = Some(gate);
        self
    }

    /// Starts each project's chats behind the window (`crate::restoring`): an open answers
    /// once the record is registered, and the chats start a few at a time on a thread of
    /// their own, each told to the window as it does.
    pub fn restoring_behind_the_window(mut self) -> Self {
        self.restores_behind = true;
        self
    }

    /// Writes each project's reopen record behind the window (#1949): a tab moved, focused,
    /// pinned or renamed changes the record at once, in order, and hands the write to one
    /// writer per project, which writes the newest. A close and the quit write the last one
    /// themselves, after it.
    pub fn recording_behind_the_window(mut self) -> Self {
        self.records_behind = true;
        self
    }

    /// What this build ships that a chat is armed with: the `charter` its hooks run and the
    /// plugin. The doctor's `plugin-install` fix installs these.
    pub(crate) fn shipped(&self) -> &crate::Shipped {
        &self.shipped
    }

    /// Records every hook call of every project this registry holds into `events` (FD-9),
    /// and each line of the kill switch's journal (#1375).
    pub fn recording_events(mut self, events: Option<hooks::Events>) -> Self {
        if let Some(events) = &events {
            self.kill_switch.record_into(events.clone());
        }
        self.events = events;
        self
    }

    /// Waits `within` at the quit for the opens and closes under way, instead of
    /// [`QUIT_WAITS_FOR_TURNS`]: for a test of one that outlasts it.
    #[cfg(test)]
    pub(crate) fn quitting_within(mut self, within: std::time::Duration) -> Self {
        self.quit_waits = within;
        self
    }

    /// Collects each plane it opens with `sweep` in place of the month's retention: for a test
    /// that needs a sweep it can hold.
    #[cfg(test)]
    pub(crate) fn sweeping_with(mut self, sweep: Sweep) -> Self {
        self.sweep = sweep;
        self
    }

    /// Records every block and Allow of every project this registry holds into `network`
    /// (#1662).
    pub fn recording_network(
        mut self,
        network: Option<purlis_core::sandboxblock::record::Record>,
    ) -> Self {
        self.network = network;
        self
    }

    /// Reads this machine's pools, and where each workspace runs its chats, from its config home
    /// `config` (#2679).
    pub fn keeping_pools_in(mut self, config: Option<PathBuf>) -> Self {
        self.pools_in = config;
        self
    }

    /// This machine's config home, where its pools are kept, where the app keeps one (#2679).
    pub(crate) fn pools_in(&self) -> Option<PathBuf> {
        self.pools_in.clone()
    }

    /// This machine's network record, where the app keeps one (#1662).
    pub(crate) fn network(&self) -> Option<&purlis_core::sandboxblock::record::Record> {
        self.network.as_ref()
    }

    /// Runs every plane's sessions on the hosts `hosting` makes, rather than in this process:
    /// the session host, behind the start-time switch (`crate::hostedsessions`, #1998).
    pub fn running_sessions_on(mut self, hosting: Hosting) -> Self {
        self.hosting = hosting;
        self
    }

    /// Hears every plane's hooks through the session host, which owns each plane's hook channel
    /// (#2000): nothing is bound in this process, and each plane's session host is told what
    /// hears the lines it hands on ([`SessionHost::hear_through`]).
    pub fn hearing_hooks_through_the_host(mut self) -> Self {
        self.hooks_in_the_host = true;
        self
    }

    /// Tells `arrivals` whenever a handoff opens a chat, so the window can put it on a strip.
    pub fn telling_arrivals(mut self, arrivals: crate::handoff::Arrivals) -> Self {
        self.arrivals = arrivals;
        self
    }

    /// Tells the window a chat it did not open itself has started: the person's own dispatch
    /// from a chat's tab (#1438), which the window files as it files a chat's.
    pub fn arrived(&self, arrived: crate::handoff::Arrived) {
        (self.arrivals)(arrived);
    }

    /// Tells `changes` whenever a plane this registry holds changes on disk, so the window reads
    /// it again (`crate::planewatch`).
    pub fn telling_changes(mut self, changes: crate::planewatch::Changed) -> Self {
        self.changes = changes;
        self
    }

    /// Tells `saves` whenever auto-save saves a plane this registry holds, so the extensions
    /// that hear a plane being saved are told (charter-app#343).
    pub fn telling_saves(mut self, saves: crate::autosave::Saved) -> Self {
        self.saves = saves;
        self
    }

    /// Ends the session host with `end`, at a quit whose stop of a project's saving it did not
    /// answer in time, before the save at quit runs (#2002).
    pub fn ending_the_host_with(mut self, end: Arc<dyn Fn() + Send + Sync>) -> Self {
        self.end_the_host = end;
        self
    }

    /// Tells `released` the root of every plane this registry lets go of (FD-11).
    pub fn telling_released(mut self, released: Released) -> Self {
        self.released = released;
        self
    }

    /// Tells `closed` every plane [`Self::close`] lets go of, so no window goes on drawing a
    /// project the core no longer holds (#1242).
    ///
    /// **The quit is not told** ([`Self::let_go_of_all`]): the windows go with the process,
    /// and a window emptied on the way out would report holding nothing and wipe the
    /// arrangement the next launch puts back.
    pub fn telling_closed(mut self, closed: Closed) -> Self {
        self.closed = closed;
        self
    }

    /// Tells `by_hand` whenever a harness is started by hand in a shell tab of a plane this
    /// registry holds, so the window can put a banner on that tab (ADR 0062).
    pub fn telling_by_hand(mut self, by_hand: hooks::ByHandTeller) -> Self {
        self.by_hand = by_hand;
        self
    }

    /// Tells `asks` a plane's permission asks each time they change, so the window can list them
    /// in needs-you and answer them there (HP-6).
    pub fn telling_asks(mut self, asks: crate::asking::Teller) -> Self {
        self.asks = asks;
        self
    }

    /// Tells `touches` each file a chat's tool touches in a plane this registry holds, once
    /// confined to the chat's folder, so the window can mark it in the tree (FM-6).
    pub fn telling_touches(mut self, touches: hooks::TouchTeller) -> Self {
        self.touches = touches;
        self
    }

    /// Tells `skill_runs` the skill run open in a chat of a plane this registry holds, each time
    /// it changes (SK-3).
    pub fn telling_skill_runs(mut self, skill_runs: crate::skillrun::Teller) -> Self {
        self.skill_runs = skill_runs;
        self
    }

    /// Asks each chat's router extension at its start with `executor` (SK-15b, ADR 0102 §5): the
    /// app's own, so quitting stops what it asked too.
    pub fn asking_routers_with(mut self, executor: Arc<purlis_core::executor::Executor>) -> Self {
        self.routers = Some(executor);
        self
    }

    /// Tells `doings` what a working chat of a plane this registry holds is doing, each time
    /// its one line changes, so the window can say it under the chat's name (#1493).
    pub fn telling_doings(mut self, doings: crate::doing::Teller) -> Self {
        self.doings = doings;
        self
    }

    /// Tells `blocks` each sandbox block a chat's hook finds in a plane this registry holds, so
    /// the window can show it as a Notice on the chat's tab (#1338).
    pub fn telling_blocks(mut self, blocks: hooks::BlockTeller) -> Self {
        self.blocks = blocks;
        self
    }

    /// Tells the window `told`, a block as it stands now: the block a person chose the target
    /// of, in place of the one that named none (#1884).
    pub(crate) fn tell_block(&self, told: hooks::ChatBlocked) {
        (self.blocks)(told);
    }

    /// [`Self::tell_block`] as a handle of its own, for a command whose work runs off the
    /// window's thread and outlives the registry's borrow (#1949).
    pub(crate) fn block_teller(&self) -> hooks::BlockTeller {
        Arc::clone(&self.blocks)
    }

    /// Who is told each chat move's stage (#2048).
    pub(crate) fn telling_moves(
        mut self,
        told: Arc<dyn Fn(PlaneId, crate::chatmoves::ChatMove) + Send + Sync>,
    ) -> Self {
        self.moves_told = told;
        self
    }

    /// Tells `told` each chat, in a plane this registry holds, whose project's Claude Code
    /// settings changed so that it no longer loads them, so the window restarts it (ADR 0067 as
    /// amended 2026-10-16).
    pub(crate) fn telling_project_settings(mut self, told: crate::projectsettings::Teller) -> Self {
        self.project_settings = told;
        self
    }

    /// Tells `more` how many hosts a chat's proxy refused past the ones it names, in a plane this
    /// registry holds, each time that grows, so its host Notice can say so (#1353).
    pub fn telling_refused_more(mut self, more: hooks::RefusedMoreTeller) -> Self {
        self.refused_more = more;
        self
    }

    /// Runs the browsers of the chats of every plane this registry holds in `driver` (#2013).
    pub fn driving_browsers_with(mut self, driver: Arc<dyn crate::chatbrowsers::Driver>) -> Self {
        self.browser_driver = Some(driver);
        self
    }

    /// Tells `refused` each vault a chat is refused for its persona in a plane this registry
    /// holds, so the window can show the ways forward on the chat's tab (#1430).
    pub fn telling_vault_refusals(mut self, refused: crate::vaultroute::Teller) -> Self {
        self.vault_refused = refused;
        self
    }

    /// Tells `ended` each Ask about a chat's browser that ended with the browser, so the Inbox
    /// keeps it as an Update (#2015).
    pub fn telling_browser_asks_ended(mut self, ended: crate::browserneedsyou::Teller) -> Self {
        self.browser_asks_ended = ended;
        self
    }

    /// Who is told each chat whose browsers ended because the person narrowed its reach
    /// (#2013), or removed Always for its persona (#2241).
    pub fn browsers_ended(&self) -> &crate::chatbrowsers::Teller {
        &self.browsers_ended
    }

    /// Tells `ended` each chat whose browsers ended because the person narrowed its reach, so
    /// the window says so on its tab (#2013).
    pub fn telling_browsers_ended(mut self, ended: crate::chatbrowsers::Teller) -> Self {
        self.browsers_ended = ended;
        self
    }

    /// Tells `listed` each project whose chats' browsers were added to or taken from, so the
    /// window draws them under their chats and on their tabs (#2024).
    pub fn telling_browsers_listed(mut self, listed: crate::chatbrowsers::ListedTeller) -> Self {
        self.browsers_listed = listed;
        self
    }

    /// Tells `moved` each chat whose browser timeline moved, so a timeline tab of it reads it
    /// again (#2017).
    pub fn telling_browser_timelines(mut self, moved: crate::chatbrowsers::TimelineTeller) -> Self {
        self.browser_timeline = moved;
        self
    }

    /// Tells `failed` each persona MCP server a chat could not start after its second try, in
    /// a plane this registry holds, so the window can say so on the chat's tab (qw381).
    pub fn telling_mcp_failures(mut self, failed: crate::mcpfailed::Teller) -> Self {
        self.mcp_failed = failed;
        self
    }

    /// Tells `lost` each chat whose program ended with its session host, in a plane this
    /// registry holds, so the window can say so on the chat's pane, with Resume (#2007).
    pub fn telling_lost_chats(mut self, lost: crate::hostlost::Teller) -> Self {
        self.lost_with_the_host = lost;
        self
    }

    /// Tells `reconnecting` each plane's chats that start or stop reconnecting while the link to
    /// the runner they run on is down, so the window can say so over each one's screen (#2496).
    pub fn telling_reconnecting(mut self, reconnecting: crate::reconnecting::Teller) -> Self {
        self.reconnecting = reconnecting;
        self
    }

    /// Tells `smart` each step of a smart close in a plane this registry holds, so the window can
    /// draw the tab wrapping up and close it when its record lands (ADR 0064).
    pub fn telling_smart_close(mut self, smart: crate::smartclose::Teller) -> Self {
        self.smart = smart;
        self
    }

    /// Tells `stops` each step of a stop in a plane this registry holds, so the window can say
    /// a chat is stopping and take its tab away when it has ended (#1448).
    pub fn telling_stops(mut self, stops: crate::stopping::Teller) -> Self {
        self.stops = stops;
        self
    }

    /// Tells `activity` each line of a dispatch in a plane this registry holds as the app
    /// records it, so an open Activity tab follows the work (#1495).
    pub fn telling_task_quiet(mut self, quiet: crate::silence::Teller) -> Self {
        self.quiet = quiet;
        self
    }

    /// Tells `activity` each line of a dispatch in a plane this registry holds as the app
    /// records it (#1495).
    pub fn telling_activity(mut self, activity: crate::activity::Teller) -> Self {
        self.activity = activity;
        self
    }

    /// Opens `root`: binds its hook socket and arms its board. Answers with the id every
    /// later call names it by.
    ///
    /// **Its record is not touched.** Putting one back starts programs, so it needs the
    /// operator's yes — see [`Approved`] and [`Planes::reopen`]. An opened plane with no yes
    /// behind it is a live, empty project: chats can be started in it, and nothing that was
    /// written in the directory runs.
    ///
    /// **A root already open is answered with the id it already has, and nothing is bound a
    /// second time.** `Listener::bind` removes a socket left behind by a process that is
    /// gone, which is right for a stale one and would be a disaster for a live one: the
    /// sessions of the plane already open carry that path in their environment, so their
    /// hooks would report onto a board belonging to a second `Chats` that numbers its chats
    /// from one. Every state would land on the wrong chat.
    pub fn open(&self, root: &Path) -> PlaneId {
        // Resolved once, here. Everything below — the socket, the record, the registry key —
        // is this one spelling of the plane, so nothing downstream has to resolve anything.
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        // **The app's own line of the fence** (charter-app#129). Every per-plane thing this
        // app holds — the board, the chats, the hook socket in `.charter/app/`, the machine
        // store entry made a line below — hangs off this call and off no other, because a
        // `PlaneId` is minted here alone. So a fenced build that must not touch a plane has
        // exactly one place to say so, and a caller that reached a root some other way than
        // `plane::resolve` is held all the same.
        purlis_core::fence::hold(purlis_core::fence::Act::Open, &root);
        let id = PlaneId::of(&root);
        // Remembered HERE, and therefore under the spelling the line above settled on. Done
        // in the caller instead, it would be done against whatever path that caller happened
        // to hold — and a machine store keyed on `/var/…` while every approval is keyed on
        // `/private/var/…` is two entries for one project, each answering half the question.
        // It is not an approval: `Store::remember` carries an existing one over and creates
        // none, so opening a plane a hundred times does not become consent to it.
        self.remember(&root);
        // Temps an older charter was killed in front of, which nothing writing today will
        // ever rename away (#440). Only its own old names, and only stale ones.
        purlis_core::leftovers::sweep_plane(&root);
        // And the persona tool gate's per-session ceilings, which nothing else removes.
        purlis_core::personagate::sweep_ceilings(&root, std::time::SystemTime::now());
        if let Some(config) = self.config.as_deref() {
            purlis_core::leftovers::sweep_config(config);
        }
        // The per-session files no chat has written for a month (SC-7). Only for a plane this
        // app does not hold yet, so only when no chat of it is running in this app: the chats
        // its reopen record will bring back are what it keeps.
        //
        // **Not under the registry's lock** (#1027): a sweep reads every aged trace, which on a
        // plane used for months takes a while, and the lock is what every other project's open,
        // close and lookup takes. So whether the plane is held is asked under the lock, and the
        // sweep runs outside it, before this open holds the plane. **An open of the same root
        // waits for this one** ([`Self::opening`]): it then finds the plane held and sweeps
        // nothing, so no sweep ever runs beside a chat of a plane this app holds.
        let _opening = self.opening(&id);
        if !self.map().contains_key(&id) {
            (self.sweep)(&root);
            // The app's own state an older purlis kept in the plane's state folder moves to the
            // data home, once (#3718): before any chat is put back, so what arms them reads it
            // there, and outside the registry's lock, as the sweep is.
            let _ = purlis_core::appstate::move_out_of_the_plane(&root);
        }

        let mut open = self.map();
        if let Some(already) = open.get(&id) {
            return already.id.clone();
        }
        // The credential files a brokered `secret exec` of this project left when the app was
        // stopped before it could remove them (#1407). Here, where no run of the project's can
        // be live: it is not open in this app.
        purlis_core::secrets::brokered::sweep(&purlis_core::secrets::Ctx::new(
            &root,
            purlis_core::secrets::Env::from_process(),
        ));
        // The reports kept for a workspace a sandboxed chat renamed: that chat's sandbox does
        // not let it move them, so the app does, here, where it is not sandboxed (D-T59-19).
        if let Err(why) = purlis_core::wscmd::rename::kept_reports_follow(&root) {
            tracing::warn!("purlis: reports kept for a renamed workspace were not moved ({why})");
        }
        // And a dispatch whose persona chat that record does not bring back has ended (#1452).
        // It first makes an older build's handoff that still owes a report the task it now is
        // (#1519): before any chat is put back, so what puts them back reads the newer shape.
        purlis_core::dispatchrecord::settle_on_open(&root, chrono::Utc::now());
        // And what the chats of a dispatch said to each other is kept for 30 days after it
        // ended, whichever of them is still brought back (#1495, D-1495-12).
        purlis_core::dispatchrecord::expire_talk(&root, chrono::Utc::now());
        // Then the worktrees of the dispatches that have ended are looked at, once (#1453):
        // one whose branch is merged, and that no chat coming back stands in, is taken away.
        // **Which chats come back is read here**, beside the settle above and before this
        // open rewrites the record; the look itself runs git, so it is on a thread of its own
        // and opening a project does not wait for it.
        if let Some(at_open) = purlis_core::dispatchplace::at_open(&root) {
            let root = root.clone();
            std::thread::spawn(move || crate::dispatches::tidy_opened(&root, &at_open));
        }
        let held = Arc::new(self.hold(id.clone(), root));
        let _ = held.me.set(Arc::downgrade(&held));
        // What becomes of its chats' browsers, in its event log (#2842). A `Weak`, as the
        // answer below holds one: the plane's browsers must not keep a closed plane alive.
        held.browsers.saying({
            let held = Arc::downgrade(&held);
            Arc::new(move |number, said| {
                let Some(held) = held.upgrade() else {
                    return;
                };
                let kind = said.kind();
                if let Err(why) = held.hooks().record_browser(held.root(), number, said) {
                    tracing::warn!("purlis: {kind} was not written to the event log ({why})");
                }
            })
        });
        // A handoff from one of this plane's chats is answered by this plane, which is the
        // only one holding the asking chat's record. A `Weak`, because the plane holds the
        // socket that holds this answer: a strong handle would keep a closed plane alive.
        held.hooks.answer_with({
            let held = Arc::downgrade(&held);
            let plane = id.clone();
            let tickets = purlis_core::hookwire::Tickets::default();
            let arrivals = Arc::clone(&self.arrivals);
            let refused = Arc::clone(&self.vault_refused);
            Arc::new(move |connection, ask| {
                // A vault's key names (qw385): answered with the window's teller, so a refusal
                // raises the vault's Notice on the chat's tab as a brokered run's does.
                if let purlis_core::hookwire::Ask::Keys { chat, vault } = &ask {
                    return match held.upgrade() {
                        Some(held) => {
                            crate::vaults::keys_for_chat(&held, &plane, &*refused, *chat, vault)
                        }
                        None => purlis_core::hookwire::Answer::No {
                            why: "this project has been closed".to_owned(),
                        },
                    };
                }
                // A wait is the one ask that takes minutes: it holds the project weakly, so
                // closing it is not held up by a chat waiting on a report (#1441).
                if let purlis_core::hookwire::Ask::Task(asked) = &ask {
                    match asked.what {
                        purlis_core::dispatched::What::Wait { .. } => {
                            return crate::dispatched::wait(&held, asked, connection);
                        }
                        purlis_core::dispatched::What::AwaitAnswer { .. } => {
                            return crate::dispatched::await_answer(&held, asked, connection);
                        }
                        _ => {}
                    }
                }
                match held.upgrade() {
                    Some(held) => {
                        crate::handoff::answer(&held, &plane, &tickets, connection, ask, &*arrivals)
                    }
                    None => purlis_core::hookwire::Answer::No {
                        why: "this project has been closed".to_owned(),
                    },
                }
            })
        });
        // The person's answer to a dispatch that waited on them starts it, or tells the chat
        // that asked it was kept blocked (#1437). On a thread of its own: an Allow is a press
        // in the window, and a chat's start takes seconds. Weak for the handoff's reason.
        held.dispatch_grants().answers_with({
            let held = Arc::downgrade(&held);
            let plane = id.clone();
            let arrivals = Arc::clone(&self.arrivals);
            Arc::new(move |answer| {
                // A dispatch whose asking chat is closing is recorded on the thread closing
                // it, while the app still knows that chat (#1464): nothing starts for it.
                if answer.asker_closed {
                    if let Some(held) = held.upgrade() {
                        crate::handoff::answered(&held, &plane, answer, &*arrivals);
                    }
                    return;
                }
                let (held, plane, arrivals, answer) = (
                    held.clone(),
                    plane.clone(),
                    Arc::clone(&arrivals),
                    answer.clone(),
                );
                std::thread::spawn(move || {
                    if let Some(held) = held.upgrade() {
                        crate::handoff::answered(&held, &plane, &answer, &*arrivals);
                    }
                });
            })
        });
        // A dispatch waiting on this machine's memory starts once memory frees, or gives up
        // past the bound (#1467): looked at on a thread of the project's own, which ends with
        // the project. Weak for the handoff's reason.
        {
            let held = Arc::downgrade(&held);
            let plane = id.clone();
            let arrivals = Arc::clone(&self.arrivals);
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(crate::handoff::MEMORY_LOOKED_AT_EVERY);
                    let Some(held) = held.upgrade() else { break };
                    // A reading stood in for the machine's is a test's, which looks itself.
                    if held.held_dispatches().memory().stood_in() {
                        continue;
                    }
                    crate::handoff::memory_freed(
                        &held,
                        &plane,
                        std::time::Instant::now(),
                        &*arrivals,
                    );
                }
            });
        }
        // Its chats' browsers gone unused stop (#2023): looked at on a thread of the project's
        // own, which ends with the project. Weak for the handoff's reason.
        {
            let held = Arc::downgrade(&held);
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(crate::chatbrowsers::IDLE_LOOKED_AT_EVERY);
                    let Some(held) = held.upgrade() else { break };
                    crate::chatbrowsers::idle_swept(&held, crate::sandboxing::now_secs());
                }
            });
        }
        // A brokered `secret exec` from one of this plane's chats is run by this plane, which
        // holds the chat's record: its persona and its folder (#1407). Weak for the handoff's
        // reason.
        held.hooks.exec_secrets_with({
            let held = Arc::downgrade(&held);
            let plane = id.clone();
            let refused = Arc::clone(&self.vault_refused);
            let failed = Arc::clone(&self.mcp_failed);
            Arc::new(move |ask, reader, writer, blocked| match held.upgrade() {
                Some(held) => {
                    crate::vaults::run_brokered(
                        &held, &plane, &*refused, &*failed, ask, reader, writer, blocked,
                    );
                }
                None => purlis_core::secrets::brokered::not_answered(writer),
            })
        });
        // A curation chat's prompt is typed when its harness reports its start (ADR 0061), and
        // the socket's thread is where that report arrives. A person's typed `/smart-close` is
        // heard here too: only a report the board took, so a harness nested in the chat never
        // issues its chat a pass (#1332). Weak for the handoff's reason.
        // What a chat waits on that is not the person, asked as the end of its turn is applied
        // (#1491): the tasks below it and the chat that dispatched it are this project's
        // records, which the board does not hold. Weak for the handoff's reason; a project
        // that is going holds nothing back.
        held.hooks.waits_by({
            let held = Arc::downgrade(&held);
            Arc::new(move |chat, turn_ends| {
                held.upgrade()
                    .map_or_else(purlis_core::state::Waits::default, |held| {
                        crate::dispatched::waits_as(&held, chat, turn_ends)
                    })
            })
        });
        held.hooks.when_heard({
            let held = Arc::downgrade(&held);
            Arc::new(move |report| {
                let Some(strong) = held.upgrade() else { return };
                if strong.typed.heard(report) {
                    crate::curation::type_when_it_reads_keys(&strong, report.chat);
                }
                crate::smartclose::heard(&strong, report);
            })
        });
        // A smart close queued for its chat's turn to end is sent then, and one whose record is
        // saved closes its tab (ADR 0064). Weak for the handoff's reason.
        held.hooks.when_reported({
            let held = Arc::downgrade(&held);
            Arc::new(move |report| {
                if let Some(held) = held.upgrade() {
                    crate::smartclose::reported(&held, report);
                    // Whether its harness runs with its permission prompts off, which a
                    // dispatch from it is held to (#1446). Only a report the board took: a
                    // harness nested in the chat never marks the chat.
                    held.unattended()
                        .heard(report.chat, report.detail.unattended);
                    // And the mode its own harness runs in, which a task it dispatches starts
                    // from (#3787): the same report, its own harness's alone, and never a line
                    // the chat sends.
                    crate::handoff::heard_mode(&held, report);
                    // The model its own harness says the chat runs on, for a commit's
                    // `Assisted-by` (#1021): judged against the conversation the board holds
                    // for the chat now, so a nested harness's report names none.
                    let now = held
                        .hooks
                        .board()
                        .conversation(report.chat)
                        .map(str::to_owned);
                    held.chats().heard_model(report, now.as_deref());
                    // What an Allow queued for this chat's turn to end is sent then (#1430).
                    crate::vaultroute::reported(&held, report.chat);
                    // A persona chat that has just come to wait on the person's input: its
                    // dispatch needed them once more (#1452, #1457).
                    crate::dispatches::needed_you(&held, report.chat);
                    // And whatever waited for this chat to move: the line that says its tasks
                    // reported (#1441).
                    crate::dispatched::heard(&held, report);
                    // And a chat the person is stopping, or a task its asking chat is
                    // cancelling (#1459), is sent its last-turn prompt when the turn it was in
                    // ends, and is ended when that last turn does (#1448).
                    crate::stopping::reported(&held, report.chat);
                }
            })
        });
        // What a chat's turn ended saying (#1981): kept for a task alone, in memory. Weak for
        // the handoff's reason.
        held.hooks.when_output({
            let held = Arc::downgrade(&held);
            Arc::new(move |output| {
                if let Some(held) = held.upgrade() {
                    crate::dispatched::said(&held, output);
                }
            })
        });
        // What a chat's own tool hook says it is doing (#1981): activity, which a task's quiet
        // stretch is measured from. Weak for the handoff's reason.
        held.hooks.when_doing({
            let held = Arc::downgrade(&held);
            Arc::new(move |chat, said| {
                if let Some(held) = held.upgrade() {
                    crate::silence::doing(&held, chat, &said);
                }
            })
        });
        held.hooks.when_saved({
            let held = Arc::downgrade(&held);
            Arc::new(move |saved| {
                if let Some(held) = held.upgrade() {
                    crate::smartclose::saved(&held, &saved);
                }
            })
        });
        // A file a chat's tool touched (FM-6): confined to the chat's own folder and rated
        // before the window hears of it, and written nowhere (D-86a). Weak for the handoff's
        // reason.
        held.hooks.when_touching({
            let held = Arc::downgrade(&held);
            let plane = id.clone();
            let touches = Arc::clone(&self.touches);
            let gate = Mutex::new(purlis_core::touching::Gate::default());
            Arc::new(move |touching| {
                let Some(strong) = held.upgrade() else { return };
                // And kept for a task's Changes tab, by its chat, in memory only (#1511).
                crate::taskchanges::touched(&strong, &touching);
                let folder = strong
                    .chats()
                    .chat_at(touching.chat)
                    .and_then(|chat| chat.cwd);
                if let Some(told) = hooks::touched(
                    &plane,
                    folder.as_deref(),
                    &gate,
                    &touching,
                    std::time::Instant::now(),
                ) {
                    touches(told);
                }
            })
        });
        // What a working chat is doing (#1493): already a kind and one passed name, held in
        // memory and written nowhere.
        held.hooks.doings().tell_to(Arc::clone(&self.doings));
        held.hooks
            .skill_runs()
            .tell_to(Arc::clone(&self.skill_runs));
        // And the tokens of its open run (SK-26, #3694): what its harness last said the chat
        // spent, read from the same record a run's recorded tokens are. Weak for the handoff's
        // reason.
        held.hooks.skill_runs().count_tokens_by({
            let held = Arc::downgrade(&held);
            Arc::new(move |chat| {
                let held = held.upgrade()?;
                let id = held.chats().chat_at(chat)?.id?;
                let spent = purlis_core::usage::spent(held.root(), &id)?;
                (spent.input_tokens.is_some() || spent.output_tokens.is_some()).then(|| {
                    spent
                        .input_tokens
                        .unwrap_or_default()
                        .saturating_add(spent.output_tokens.unwrap_or_default())
                })
            })
        });
        // A connection a Claude Code chat's server dialled in (#3316): parked at the chat's
        // door. The first each door is given, and every one refused, is audited: each request
        // the door carries spends one, so a park after the first says nothing new. Weak for the
        // handoff's reason.
        #[cfg(unix)]
        held.hooks.park_slots_with({
            let held = Arc::downgrade(&held);
            let plane = id.clone();
            // A refusal is audited once a minute for each chat, port and reason at most: a
            // server that keeps dialling is said once, never a row a dial.
            const SERVE_REFUSALS_SAID_EVERY: std::time::Duration =
                std::time::Duration::from_secs(60);
            type Said = HashMap<(u32, u16, &'static str), std::time::Instant>;
            let audited: Arc<Mutex<Said>> = Arc::default();
            Arc::new(move |ask, slot| {
                let Some(held) = held.upgrade() else {
                    purlis_core::hookwire::serve::refused(
                        slot,
                        purlis_core::hookwire::serve::NOTHING_SERVES,
                    );
                    return;
                };
                let refused = match held.chats().park_slot(&ask, slot) {
                    Ok(false) => return,
                    Ok(true) => None,
                    Err(why) => {
                        let now = std::time::Instant::now();
                        let mut said = audited.lock().unwrap_or_else(PoisonError::into_inner);
                        said.retain(|_, at| now.duration_since(*at) < SERVE_REFUSALS_SAID_EVERY);
                        if said.insert((ask.chat, ask.serve, why), now).is_some() {
                            return;
                        }
                        Some(why)
                    }
                };
                if let Err(why) =
                    held.hooks
                        .audit_serving(plane.root(), ask.chat, ask.serve, refused)
                {
                    tracing::warn!("purlis: a chat's server's connection was not audited ({why})");
                }
            })
        });
        // A sandbox block a chat's hook found (#1338): kept in this machine's network record
        // with the chat it came from (#1662), then shown on the chat's tab. Of what the line
        // named, only a refused host is kept.
        held.hooks.when_blocked({
            let plane = id.clone();
            let blocks = Arc::clone(&self.blocks);
            let network = self.network.clone();
            // Weak for the handoff's reason. The block is held for the chat before the window
            // is told of it, so an answer to it can be checked against what was heard (#1508).
            let held = Arc::downgrade(&held);
            Arc::new(move |block| {
                let strong = held.upgrade();
                // Whether the chat started with the certificate check, by the app's own record
                // of its start: what a refused check's Notice says turns on it.
                let has_check = strong
                    .as_ref()
                    .is_some_and(|held| held.chats().has_certificate_check(block.line.chat));
                let mut told = hooks::blocked_holding(&plane, &block, has_check);
                if let Some(strong) = strong {
                    // A runner's service is said with that runner's name (#2510).
                    told = hooks::on_its_runner(
                        told,
                        strong.chats().runner_of(block.line.chat).as_deref(),
                    );
                    // A server asking for a port the chat serves on already (#2019).
                    told = hooks::on_its_serving(
                        told,
                        &strong.chats().served_ports(block.line.chat),
                        strong.chats().dials_in(block.line.chat),
                    );
                    // Held while the person answers (#1666): the chat's own board says so,
                    // never anything the block's sender said.
                    told.held = strong.chats().asking(told.session, told.target.as_deref());
                    if let Some(network) = &network {
                        crate::network::record_block(
                            network,
                            plane.root(),
                            strong.chats(),
                            &block,
                            crate::sandboxing::now_secs(),
                        );
                    }
                    // A block that is the chat's own to retry (#3861): its hook told it the
                    // command to retry with, and the person has nothing to answer, so it is
                    // kept in the record above and raises no Notice, and the chat waits on
                    // nobody for it.
                    let chat_dir = strong
                        .chats()
                        .chat_at(block.line.chat)
                        .and_then(|chat| chat.cwd);
                    if hooks::its_own_to_retry(&block, chat_dir.as_deref()) {
                        return;
                    }
                    crate::taskblocks::heard(strong.chats(), &told);
                    // The chat now waits on the person for this Notice, until its turn moves
                    // on: what the chat that asked for it is told (#1663). purlis's own block
                    // is a bug to report, which the chat is not told to wait on, and neither is
                    // a block it only read back (#1884), whose quiet line nobody has to answer.
                    if !told.ours && told.quiet.is_none() {
                        let turn = strong.board().glance(told.session).turns;
                        strong.chats().blocks().raised(told.session, turn);
                        // A block heard while it waits on its own work is an ask (#3878).
                        strong.blocked_while_waiting(told.session);
                    }
                }
                blocks(told);
            })
        });
        // The repeats of a block the throttle held back (#1681) are kept in the network record
        // on a line of their own once their minute is over, and shown nowhere.
        if let Some(network) = &self.network {
            let plane = id.clone();
            let network = network.clone();
            let weak = Arc::downgrade(&held);
            held.hooks.when_repeated(Arc::new(move |over| {
                let Some(strong) = weak.upgrade() else { return };
                let (now, at) = (crate::sandboxing::now_secs(), std::time::Instant::now());
                for repeated in &over {
                    crate::network::record_repeated(
                        &network,
                        plane.root(),
                        strong.chats(),
                        repeated,
                        now,
                        at.saturating_duration_since(repeated.last),
                    );
                }
            }));
        }
        // A host purlis's own proxy refused a chat it wraps (Codex, opencode) takes the road a
        // hook's block takes (#1663).
        if let Some(hear) = held.hooks.block_hearer() {
            held.chats().tell_refusals_to(hear);
        }
        // And how many it refused past the ones it names (#1353), a count and never a host.
        held.chats().tell_more_refused_to({
            let (plane, told) = (id.clone(), Arc::clone(&self.refused_more));
            Arc::new(move |session, more| {
                told(hooks::RefusedMore {
                    plane: plane.clone(),
                    session,
                    more,
                });
            })
        });
        // A chat whose project's Claude Code settings no longer pass is started again by the
        // core itself, or ended, and the window is told to redraw it (ADR 0067 as amended
        // 2026-10-16). Weak for the handoff's reason.
        held.chats().tell_project_settings_to({
            let weak = Arc::downgrade(&held);
            let (plane, told) = (id.clone(), Arc::clone(&self.project_settings));
            Arc::new(move |session| {
                if let Some(strong) = weak.upgrade() {
                    let (chat, notices, ended) = match strong.contain_project_settings(session) {
                        Ok(started) => (
                            crate::drawn(&strong, started).ok(),
                            strong.chats().start_notes(started),
                            None,
                        ),
                        Err(why) => (None, Vec::new(), Some(why)),
                    };
                    told(crate::projectsettings::Rescreened {
                        plane: plane.clone(),
                        session,
                        chat,
                        notices,
                        ended,
                    });
                }
            })
        });
        // A permission hook's ask is read by the harness this app started its chat as, never
        // by the one the chat's environment names (#1703). Weak for the handoff's reason.
        held.hooks.asks().reading_by({
            let weak = Arc::downgrade(&held);
            Box::new(move |chat| {
                let strong = weak.upgrade()?;
                let harness = strong.chats().harness(chat.parse().ok()?)?;
                Some(purlis_core::harness::hooked::Source::of(harness))
            })
        });
        // A live ask held for its chat (#1709) is in the asks registry before its Notice is
        // raised, and whether or not the block throttle lets the Notice through: the window is
        // told its asks so it reads them again, and the notifier with it. Weak for the
        // handoff's reason.
        held.chats().tell_asks_moved_to({
            let weak = Arc::downgrade(&held);
            let asks = Arc::clone(&self.asks);
            Arc::new(move || {
                if let Some(strong) = weak.upgrade() {
                    asks(strong.asks());
                }
            })
        });
        // purlis's word on a chat's held connections (#1666) is handed to its next turn on
        // the road the person's words take: the app's memory, never a file. Weak for the
        // handoff's reason.
        // A chat whose reach the person narrowed while it runs: its browsers end, and its tab
        // says so (#2013). Weak for the handoff's reason.
        held.chats().tell_narrowed_to({
            let weak = Arc::downgrade(&held);
            let plane = id.clone();
            let told = Arc::clone(&self.browsers_ended);
            Arc::new(move |session| {
                if let Some(strong) = weak.upgrade() {
                    crate::chatbrowsers::narrowed(&strong, &plane, session, &*told);
                }
            })
        });
        held.chats().tell_network_words_to({
            let weak = Arc::downgrade(&held);
            Arc::new(move |session, word| {
                if let Some(strong) = weak.upgrade() {
                    strong.tasks().ledger().talk.outcome_word(session, word);
                }
            })
        });
        // Every connection purlis's own proxy carried for a chat it wraps is kept in this
        // machine's network record (#1664), coalesced by the proxy, under the chat whose port
        // it came in on. Weak for the handoff's reason.
        if let Some(network) = self.network.clone() {
            let plane = id.clone();
            let weak = Arc::downgrade(&held);
            held.chats()
                .tell_connections_to(Arc::new(move |session, who, target, by, times| {
                    if let Some(strong) = weak.upgrade() {
                        crate::network::record_connections(
                            &network,
                            plane.root(),
                            strong.chats(),
                            &crate::network::Connections {
                                session,
                                who,
                                target,
                                by,
                                times,
                            },
                            crate::sandboxing::now_secs(),
                        );
                    }
                }));
        }
        // The conversation a chat's own harness moves it onto — the first one Codex or
        // opencode names, a Claude Code `/clear` — is the one the record resumes it by (Q10).
        // Weak for the handoff's reason.
        held.hooks.when_it_follows({
            let held = Arc::downgrade(&held);
            Arc::new(move |session, id, run| {
                if let Some(held) = held.upgrade() {
                    held.chats().follow_conversation(session, id, run);
                }
            })
        });
        // Auto-save of a workspace's repos waits for its chats' turns, which only the plane
        // knows. Weak for the handoff's reason: the plane holds the worker.
        if let Some(worker) = held
            .autosave
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            let held = Arc::downgrade(&held);
            worker.asks_mid_turn_of(Arc::new(move |workspace| {
                held.upgrade()
                    .map(|held| held.mid_turn_in(workspace))
                    .unwrap_or_default()
            }));
        }
        // A quit that began while this opened has let go of every project already, and nothing
        // after it lets go of one (#1949): the project is let go of here, not held.
        if self.letting_go.quitting() {
            drop(open);
            held.let_go(false);
            (self.released)(&held.root);
            return id;
        }
        open.insert(id.clone(), Arc::clone(&held));
        id
    }

    /// The record of a plane this registry is already holding, read against the root the
    /// registry settled on.
    ///
    /// **For the launch, which has no dialog to have shown one.** Every other caller of
    /// [`Self::reopen`] arrives with the record already in hand, from the same read that
    /// produced the contribution the operator approved (charter-app#123). A plane the registry
    /// is not holding has no record here rather than a guessed path: the id is a `display()`
    /// of the root, and spelling it back into a path substitutes for bytes that are not UTF-8.
    fn record_of(&self, plane: &PlaneId) -> Read {
        match self.held(plane) {
            Ok(held) => reopen::read_or_refusal(&held.root),
            Err(why) => Err(std::io::Error::other(why)),
        }
    }

    /// Puts back what the plane held, which **starts the programs its record names**.
    ///
    /// Private, and it takes [`Approved`] — a value nothing outside this module can build.
    /// That is the gate: [`Planes::open`] above attaches a plane and has no way to reach
    /// this, so a plane the operator has not said yes to cannot run anything.
    ///
    /// `record` is the bytes the caller decided on, never a path for this to read: see
    /// [`Held::reopen`] and charter-app#123.
    fn reopen(&self, plane: &PlaneId, _yes: &Approved, record: Read) {
        // The handle is taken and the lock dropped BEFORE anything starts: reopening runs
        // programs, and a program that dies at once tells the board, which tells the window,
        // which asks this very registry what the chat is called.
        let Ok(held) = self.held(plane) else { return };
        // A project the launch's answer said to start fresh is started fresh the first time it
        // is put back, and only that time. That first time may be later in the day — a restore
        // whose trust ask was declined, opened afterwards from the recents — which is still
        // the operator's answer for that project; after it, an open is an ordinary one.
        let choice = if self.relaunching().fresh.remove(held.root()) {
            Choice::StartFresh
        } else {
            Choice::ReopenAll
        };
        // **Registered here, started behind the window** (`crate::restoring`): the open that
        // asked answers at once, with every chat's tab to draw, and the programs start on a
        // thread of their own. Where no thread can be had, or a test asked for it, in turn.
        //
        // **Nothing starts before the gate lets it** (#2004), on whichever thread starts them: a
        // gate that refuses leaves each chat registered and unstarted, and its record as it was.
        let gate = self.gate.clone();
        let let_through = move |root: &Path| match &gate {
            Some(gate) => gate()
                .inspect_err(|why| {
                    tracing::warn!(
                        "purlis: plane {}: its chats are not put back: {why}",
                        root.display()
                    );
                })
                .is_ok(),
            None => true,
        };
        if !self.restores_behind {
            let begun = held.begin_reopen(record, choice);
            if let_through(held.root()) {
                held.finish_reopen(begun, STARTING);
            }
            return;
        }
        let begun = held.begin_reopen(record, choice);
        let behind = Arc::clone(&held);
        let begun = Mutex::new(Some(begun));
        let begun = Arc::new(begun);
        let taken = Arc::clone(&begun);
        let through = let_through.clone();
        let spawned = std::thread::Builder::new()
            .name("purlis-restore".into())
            .spawn(move || {
                let begun = taken.lock().unwrap_or_else(PoisonError::into_inner).take();
                if let Some(begun) = begun
                    && through(behind.root())
                {
                    behind.finish_reopen(begun, STARTING);
                }
            });
        if spawned.is_err() {
            let begun = begun.lock().unwrap_or_else(PoisonError::into_inner).take();
            if let Some(begun) = begun
                && let_through(held.root())
            {
                held.finish_reopen(begun, STARTING);
            }
        }
    }

    /// What this launch would put back, or nothing when there is nothing to ask about — no
    /// project holds a chat or a view tab — or the operator has already answered.
    ///
    /// `restoring` is the projects the window is about to restore (`opener::planes_to_restore`),
    /// named by the paths the window will open them by. **Reading a record here starts
    /// nothing**: the counts are all this takes from it, and each project is read again, by the
    /// one read its own open makes, when it is actually put back (charter-app#123).
    pub fn relaunch_ask(&self, restoring: &[PathBuf]) -> Option<Relaunchable> {
        let relaunching = self.relaunching();
        // Answered already: by the window, or before the restart that started this launch.
        if relaunching.decided || relaunching.chosen.is_some() {
            return None;
        }
        let roots = self.launch_roots(&relaunching, restoring);
        let restarted = relaunching.after_update;
        drop(relaunching);
        let mut flagged = false;
        let projects: Vec<Waiting> = roots
            .into_iter()
            .filter_map(|root| {
                // A record charter refuses to read puts nothing back, so it is not asked about;
                // the refusal is said where it always was, when the project is opened.
                let record = reopen::read_or_refusal(&root).ok()?;
                flagged |= record.relaunch_after_update;
                record.holds_anything().then_some(Waiting {
                    chats: record.chats.len(),
                    views: record.views.len(),
                    root,
                })
            })
            .collect();
        (!projects.is_empty()).then_some(Relaunchable {
            projects,
            after_update: restarted && flagged,
        })
    }

    /// The operator's answer to [`Self::relaunch_ask`] — or the answer a launch with nothing
    /// to ask about takes without asking, which is [`Choice::ReopenAll`].
    ///
    /// **Taken once.** A second answer — a reloaded window — changes nothing.
    ///
    /// Puts the launch's own plane back, which it has owed since [`at_launch`] attached it,
    /// and marks every project in `restoring` for a fresh start when that is the answer, so
    /// the window's restore opens them through the ordinary gate and they come back empty.
    ///
    /// **This is where the launch's yes — the first of the two approvals [`Approved`] names —
    /// is spent now**, and it covers the one plane [`at_launch`] wrote down as owed, never a
    /// plane the caller names.
    pub fn relaunch(&self, choice: Choice, restoring: &[PathBuf]) {
        let owed = {
            let mut relaunching = self.relaunching();
            if relaunching.decided {
                return;
            }
            relaunching.decided = true;
            // The answer given before the restart is the person's; the window's, with no
            // question asked, is only the default (#1078).
            let choice = relaunching.chosen.unwrap_or(choice);
            if choice == Choice::StartFresh {
                relaunching.fresh = self
                    .launch_roots(&relaunching, restoring)
                    .into_iter()
                    .collect();
            }
            relaunching.owed.take()
        };
        // The lock is dropped before anything starts, for `reopen`'s reason: a program that
        // dies at once tells the window, which asks this registry about it.
        if let Some(plane) = owed {
            self.reopen(&plane, &Approved(()), self.record_of(&plane));
        }
    }

    /// Every project this launch puts back, by root: its own plane first, then each project
    /// the window will restore. The one list both the question and the answer are about, so
    /// what is asked about and what is started fresh cannot drift apart.
    fn launch_roots(&self, relaunching: &Relaunching, restoring: &[PathBuf]) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = relaunching
            .owed
            .as_ref()
            .and_then(|plane| self.held(plane).ok())
            .map(|held| held.root.clone())
            .into_iter()
            .collect();
        for path in restoring {
            // A project that is no longer a plane is the restore's own news, and it says so.
            if let Ok(root) = plane_at(path)
                && !roots.contains(&root)
            {
                roots.push(root);
            }
        }
        roots
    }

    /// Whether this launch follows a restart to update — it took the word the restart left.
    pub fn restarted_to_update(&self) -> bool {
        self.relaunching().after_update
    }

    fn relaunching(&self) -> MutexGuard<'_, Relaunching> {
        self.relaunching
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Opens a plane the operator asked for — **only if this machine has already recorded
    /// their yes to exactly what it contributes now.**
    ///
    /// The consent is read HERE, from the store, against the plane as it is on this disk at
    /// this instant, and the answer decides between the two arms. A caller cannot open the
    /// plane and then think about trust, because there is no way to open it that does not
    /// come through this function or [`Self::approve_and_open`], and both end in the one
    /// private mint below.
    ///
    /// **An ask attaches nothing.** [`Self::open`] is safe to call on an unapproved plane —
    /// it binds a socket and starts no program — but an operator who cancels the dialog would
    /// be left with a live, empty project in the registry that they never asked for, and a
    /// hook socket bound in a stranger's directory. So the ask arm touches the plane only to
    /// read what it would contribute.
    pub fn open_if_approved(&self, root: &Path) -> Result<Opening, String> {
        let root = plane_at(root)?;
        // A close of it still letting go of it is waited for, a while (#1949).
        self.not_closing(&root)?;
        // **A plane this process is already holding is answered, not asked about.** "Open it"
        // then means "show me that project", which is what a recents row and a second launch
        // both mean when they name a plane already on screen. Asking again would put a dialog
        // in front of chats that are already running, about a grant that is already in force,
        // and answering it would reach [`Self::minted`] — see the guard there for what that
        // would have cost.
        let id = PlaneId::of(&root);
        if self.held(&id).is_ok() {
            self.remember(&root);
            return Ok(Opening::Open(id));
        }
        // ONE read, and both arms are cut from it (charter-app#123). The ask arm throws the
        // record away on purpose: nothing is started, and by the time the operator clicks,
        // `approve_and_open` reads again to compare what they read against what is there now.
        // The open arm keeps it, because it is about to run it.
        let reading = machine::Contribution::read(&root);
        if let Some(asking) = self.asking(&root, reading.contributes) {
            return Ok(Opening::Ask(asking));
        }
        Ok(Opening::Open(self.minted(&root, reading.record)))
    }

    /// **The trust ask about a project this process already holds, when it asks about font
    /// files alone** (#2647): what it contributes now and how that differs from what this
    /// machine approved, read as [`Self::open_if_approved`] reads it; `None` when the approval
    /// already holds all of it.
    ///
    /// What the refused theme's Review and allow shows: a project's font file added or changed
    /// while it is open is drawn only once the approval holds it. **Refused when the project
    /// asks for anything more than font files** ([`machine::Consent::grew_in_fonts_alone`]): a
    /// chat that can write the project can change a face whenever it likes, so the ask it can
    /// raise from a theme is kept to the faces, and the rest is asked where the project is
    /// opened. Asking attaches nothing and records nothing. The person's yes goes through
    /// [`Self::approve_and_open`] like any other, which reads the project again and refuses if
    /// it is not what was shown, and whose mint hands back the id a held plane already has
    /// without putting its record back again.
    pub fn review(&self, plane: &PlaneId) -> Result<Option<Asking>, String> {
        let root = self.held(plane)?.root().to_path_buf();
        let contributes = machine::Contribution::read(&root).contributes;
        match self.asking(&root, contributes) {
            Some(asking) if asking.first() => Err(NOT_APPROVED.to_owned()),
            Some(asking) if !asking.consent.grew_in_fonts_alone() => {
                Err(MORE_THAN_FONTS.to_owned())
            }
            asking => Ok(asking),
        }
    }

    /// The question about `root`, which contributes `contributes` as just read, when this
    /// machine's store does not already hold the yes to it.
    fn asking(&self, root: &Path, contributes: machine::Contribution) -> Option<Asking> {
        asking_of(self.config.as_deref(), root, contributes)
    }

    /// The operator's yes to a plane, and the open it authorises.
    ///
    /// **The click IS the consent; the store is where it is REMEMBERED.** The difference
    /// matters on a machine that has no store at all — Windows, where `0600` has no
    /// expression and `machine` refuses outright (ADR 0031, charter-app#98). There the answer
    /// cannot be written down, so charter asks again at the next launch and opens on this
    /// one. A gate that refused to open anything where it could not file the answer would not
    /// be a gate; it would be the app turned off.
    ///
    /// **`shown` is the contribution the dialog drew, and it is checked against the plane
    /// again here.** Between the ask and the click, anything on the machine — including a
    /// chat running in another plane — can rewrite this plane's `.claude/settings.json` or
    /// its reopen record. Without this check the approval would record whatever was on disk
    /// at CLICK time, so the operator could approve, and charter could then start, a program
    /// they never read. `approve_profile` makes exactly this check about a profile's command
    /// line, for exactly this reason, and a review probe is what found it missing there.
    ///
    /// **And the record compared here is the record that runs** (charter-app#123). The check
    /// above used to be made against one read of `.charter/app/reopen.json` and `put_back`
    /// then made its own, so a write landing in between was executed without having been
    /// shown. `Contribution::read` hands back the bytes it judged and they travel down to
    /// `put_back` unchanged, which closes that window by construction rather than by narrowing
    /// it.
    ///
    /// What that does **not** close is ADR 0035's own stated limit: the fingerprint is *"not a
    /// defence against an agent that set out to forge the fingerprint"*. A chat that can write
    /// this plane can write it before the dialog is drawn just as easily as after. What closes
    /// is the gap between a human clicking a button and the app reading a file.
    pub fn approve_and_open(
        &self,
        root: &Path,
        shown: &machine::Contribution,
    ) -> Result<PlaneId, String> {
        self.approving(root, shown, || {})
    }

    /// [`Self::approve_and_open`], with a seam between the read and the open.
    ///
    /// `between` exists for one test and nothing else: charter-app#123 is a window, and a
    /// window can only be shown to have closed by a writer that lands *in* it. A test that
    /// writes before the call is testing the comparison above, which was already there; a test
    /// that writes after it is testing nothing at all. This is the same seam, for the same
    /// reason, that `rewrite::replace_through` keeps so a test can plant its link at the path
    /// that is actually opened.
    fn approving(
        &self,
        root: &Path,
        shown: &machine::Contribution,
        between: impl FnOnce(),
    ) -> Result<PlaneId, String> {
        let root = plane_at(root)?;
        self.not_closing(&root)?;
        let reading = machine::Contribution::read(&root);
        if &reading.contributes != shown {
            return Err(format!(
                "{} changed while you were reading it, so nothing was approved and nothing was \
                 opened. Open it again to see what it contributes now.",
                purlis_core::shown::short(&root.display().to_string())
            ));
        }
        between();
        self.record_approval(&root, reading.contributes);
        Ok(self.minted(&root, reading.record))
    }

    /// Attaches the plane, remembers that it was opened, and puts its record back.
    ///
    /// **The one place the operator-facing [`Approved`] is minted**, reached from exactly two
    /// callers: a consent the store had already recorded, and the operator's own click.
    ///
    /// [`Planes::open`] remembers the plane on the way through, and it has to happen before
    /// the record is put back: [`Records::write`] vouches for the record it just wrote, and
    /// `Store::vouch` refreshes an entry rather than creating one — so a plane not yet in the
    /// list would be written and then not vouched for, and the very next launch would ask
    /// about a record charter itself had written.
    /// **And the record is put back once per open, never once per yes.** `Planes::open` hands
    /// back the id a plane already has and binds nothing a second time; `reopen` has no such
    /// rule, because at the launch there is nothing to have put back yet. Reaching it for a
    /// plane this process is already holding would start a second copy of every chat the
    /// record names, beside the copies already running — so the question is asked here, where
    /// both callers pass, rather than at each of them.
    fn minted(&self, root: &Path, record: Read) -> PlaneId {
        let already = self.held(&PlaneId::of(root)).is_ok();
        let id = self.open(root);
        if !already {
            self.reopen(&id, &Approved(()), record);
        }
        id
    }

    /// Puts `root` at the front of this machine's list of planes.
    ///
    /// **Never an approval.** `Store::remember` carries an existing [`machine::Trust`] over
    /// and creates none, so opening a plane a hundred times does not become consent to it.
    ///
    /// **And the first open pins its most active workspaces** (ADR 0054), in the same write:
    /// the workspace strip draws what is pinned, so a plane nobody has pinned anything in
    /// would otherwise open to a strip holding only the workspace you are in. The store
    /// records that it did, so every later open leaves the pins as the operator left them.
    fn remember(&self, root: &Path) {
        let Some(config) = self.config.as_deref() else {
            return;
        };
        let when = now();
        let plane = root.to_path_buf();
        if let Err(why) = machine::update(config, move |store| {
            store.remember(&plane, when);
            store.pin_the_most_active(&plane);
        }) && why.kind() != std::io::ErrorKind::Unsupported
        {
            tracing::warn!(
                "purlis: {} was opened but not added to the list of recent planes ({why})",
                root.display()
            );
        }
    }

    /// Writes the operator's answer down, so they are asked once per plane per machine.
    ///
    /// Best effort, and never worth refusing the open over: an approval that could not be
    /// filed costs one more question at the next launch, and refusing here would cost the
    /// operator the project they just said yes to. `Unsupported` is silent because it is not
    /// a fault — it is a platform with no store, and the opener says so in its own words
    /// rather than on a standard error nobody double-clicking an icon can read.
    fn record_approval(&self, root: &Path, contributed: machine::Contribution) {
        let Some(config) = self.config.as_deref() else {
            return;
        };
        let when = now();
        let plane = root.to_path_buf();
        if let Err(why) = machine::update(config, move |store| {
            store.approve(&plane, when, contributed);
        }) && why.kind() != std::io::ErrorKind::Unsupported
        {
            tracing::warn!(
                "purlis: {} was opened, but your approval of it was not recorded ({why}); \
                 purlis will ask about it again",
                root.display()
            );
        }
    }

    /// Writes down which projects each window is holding, so the next cold launch can put
    /// them back (ADR 0033, spec decision 28).
    ///
    /// **Written whenever the arrangement changes, not only at the quit.** The record is the
    /// same either way at a quit, and a charter that was killed — or a machine that lost
    /// power — still comes back to the projects that were open. `RunEvent::Exit` was the
    /// other candidate and it is the one that loses everything to a crash.
    ///
    /// **Ids are turned back into roots HERE**, through the registry, rather than by spelling
    /// a `PlaneId` back into a path. The id is a `display()` of the root, which SUBSTITUTES
    /// for a byte that is not UTF-8 — so a plane whose path is not UTF-8 would be written down
    /// as a path that is not the one that is open, and then opened at the next launch.
    ///
    /// Never worth refusing anything over: an arrangement that could not be filed costs the
    /// next launch its tabs, and there is nothing the operator could do about it here.
    /// `Unsupported` is silent, because it is Windows having no store at all (ADR 0031) rather
    /// than a fault.
    ///
    /// Written by the one writer [`WrittenBehind`] keeps, as every arrangement is, and waited
    /// for here: what this answers after is on disk.
    pub fn remember_arrangement(&self, windows: &[Holding]) {
        if self.remember_arrangement_behind(windows) {
            self.arranged.flush();
        }
    }

    /// [`Self::remember_arrangement`] without the wait, for a window's report of its tabs,
    /// which it makes at every project switch (#1949): the store is one file written whole and
    /// synced, a write the window is not to wait for. Answers whether there was anything to
    /// write.
    pub(crate) fn remember_arrangement_behind(&self, windows: &[Holding]) -> bool {
        let Some(config) = self.config.as_deref() else {
            return false;
        };
        let arranged: Vec<machine::Window> = {
            let open = self.map();
            windows
                .iter()
                .filter_map(|holding| {
                    let front = holding.front();
                    let kept: Vec<(&PlaneId, PathBuf)> = holding
                        .planes
                        .iter()
                        .filter_map(|id| Some((id, open.get(id)?.root.clone())))
                        .collect();
                    if kept.is_empty() {
                        return None;
                    }
                    // The index is found again in the list that SURVIVED, never carried over
                    // from the one that went in: a plane the registry has already let go of
                    // shifts everything after it, and an `active` off by one is a window that
                    // comes back on the wrong project.
                    let active = front
                        .and_then(|front| kept.iter().position(|(id, _)| *id == front))
                        .unwrap_or(0);
                    Some(machine::Window {
                        planes: kept.into_iter().map(|(_, root)| root).collect(),
                        active,
                    })
                })
                .collect()
        };
        self.arranged.hand((config.to_path_buf(), arranged));
        true
    }

    /// Where this machine's store is — the pins a workspace rename moves — or `None` on a
    /// machine that keeps none.
    pub fn config(&self) -> Option<&Path> {
        self.config.as_deref()
    }

    /// This machine's store, as a handle of its own: for a command that writes it off the
    /// window's thread (#1949), where the registry cannot go.
    pub(crate) fn machine_store(&self) -> MachineStore {
        MachineStore {
            config: self.config.clone(),
        }
    }

    /// [`MachineStore::forget`].
    #[cfg(test)]
    pub fn forget(&self, root: &Path) -> Result<(), String> {
        self.machine_store().forget(root)
    }

    /// [`MachineStore::revoke`].
    #[cfg(test)]
    pub fn revoke(&self, root: &Path) -> Result<(), String> {
        self.machine_store().revoke(root)
    }

    /// [`Self::remembered`] to be read later, wherever: for a command that reads it off the
    /// window's thread (#1949).
    pub(crate) fn remembered_later(&self) -> impl FnOnce() -> machine::Loaded + Send + 'static {
        let config = self.config.clone();
        move || match config.as_deref() {
            Some(config) => machine::read(config),
            None => machine::Loaded::default(),
        }
    }

    /// Everything this machine remembers, with what it would not take back named.
    ///
    /// The store alone: whether each remembered path is still THERE is a `stat` per row and
    /// is deliberately not asked here — see [`machine::still_a_plane`], and the caller that
    /// asks it off the thread that draws.
    pub fn remembered(&self) -> machine::Loaded {
        match self.config.as_deref() {
            Some(config) => machine::read(config),
            // No config home at all. Not an error and not a refusal: it is a machine that
            // remembers nothing, which is the same shape as a first launch.
            None => machine::Loaded::default(),
        }
    }

    /// Everything one plane needs, built and wired but not yet started.
    fn hold(&self, id: PlaneId, root: PathBuf) -> Held {
        // A socket that cannot be opened is not worth refusing to open a plane over: it comes
        // up with every chat `unknown` and says so, which is a working project with one
        // feature missing rather than no project at all.
        let at = hooks::socket_for(Some(&root));
        let hooks = if self.hooks_in_the_host {
            Ok(Hooks::relayed(
                id.clone(),
                &at,
                Arc::clone(&self.tell),
                Arc::clone(&self.by_hand),
            ))
        } else {
            Hooks::listening_on(
                id.clone(),
                &at,
                Arc::clone(&self.tell),
                Arc::clone(&self.by_hand),
            )
        };
        let hooks = hooks.unwrap_or_else(|why| {
            tracing::warn!(
                "purlis: no hook channel at {} ({why}); every chat in {} will show as \
                     unknown",
                at.socket.display(),
                root.display()
            );
            Hooks::deaf(id.clone())
        });
        if let Some(events) = &self.events {
            hooks.record_into(events.clone());
            // What this project's hooks spooled while no host took their lines, before a chat
            // is started and issued a token (FD-30).
            hooks.drain_spool();
        }
        hooks.tell_asks_to(Arc::clone(&self.asks));
        let reporting = hooks.reporting();

        // The origin device of every chat this project mints (ADR 0066). None where the machine
        // store has no id to give: the chats still get ids, and their device reads `unknown`.
        let device = self
            .config
            .as_deref()
            .and_then(|config| purlis_core::machine::device_id(config).ok());
        let records = Arc::new(Records {
            root: root.clone(),
            config: self.config.clone(),
            allowed: AtomicBool::new(false),
            clone_seat: reopen::CloneSeat::of(&root, device.clone()),
        });
        let recorded = WrittenBehind::new({
            let records = Arc::clone(&records);
            move |record: reopen::Record| records.write(&record)
        });
        let recorder: Box<dyn Fn(&reopen::Record) + Send + Sync> = if self.records_behind {
            let recorded = Arc::clone(&recorded);
            Box::new(move |record| recorded.hand(record.clone()))
        } else {
            let records = Arc::clone(&records);
            Box::new(move |record| records.write(record))
        };
        // The project every chat started here is of, sandbox and all (#1410).
        let mut chats = Chats::on_host(recorder, (self.hosting)(reporting, &root), root.clone());
        // What the session host hands on from the hook channel it owns for this plane (#2000).
        if let Some(relayed) = hooks.relayed_from_the_host() {
            chats.sessions().hear_through(relayed);
        }
        chats.arming_with(self.shipped.clone());
        if let Some(executor) = &self.routers {
            chats.asks_routers_with(Arc::new(purlis_core::skills::router::OnThisMachine {
                executor: Arc::clone(executor),
                project: root.clone(),
            }));
        }
        chats.stopped_by(Arc::clone(&self.kill_switch));
        chats.on_device(device);
        chats.keeping_pools_in(self.pools_in.clone());
        // Each run a start begins goes into the host's event log, before the chat's program
        // can send its first hook line (ADR 0066, #834): a chat put back after a relaunch
        // carries on under its id, and its first event is `run.started {cause: reopen}`.
        if let Some(events) = &self.events {
            let events = events.clone();
            // The key the hook channel records this project's chats under.
            let root = id.root().to_path_buf();
            // A run its start may still put aside is held until the start goes on (#2250), with
            // its start's trust event (#2252); a line of it that comes first is written after
            // them.
            chats.when_a_run_begins(Box::new({
                let (events, root) = (events.clone(), root.clone());
                move |session, run, cause, attributes, said| {
                    let begun = events.record(purlis_core::eventlog::Entry::Begin {
                        plane: root.clone(),
                        number: session,
                        chat: run.chat.to_owned(),
                        run: run.run.to_owned(),
                        cause,
                        attributes: attributes.clone(),
                        trust: match &said {
                            crate::chats::RunSaid::Held(trust) => trust.clone().map(Box::new),
                            crate::chats::RunSaid::Now => None,
                        },
                        held: said != crate::chats::RunSaid::Now,
                    });
                    if let Err(why) = begun {
                        tracing::warn!("purlis: a run was not written to the event log ({why})");
                    }
                }
            }));
            // Its routing record, under the run its start began (SK-15b), held with a held run.
            chats.when_a_chat_is_routed(Box::new({
                let (events, root) = (events.clone(), root.clone());
                move |session, record| {
                    let routed = events.record(purlis_core::eventlog::Entry::SkillRouted {
                        plane: root.clone(),
                        number: session,
                        record: record.clone(),
                    });
                    if let Err(why) = routed {
                        tracing::warn!(
                            "purlis: a chat's routing record was not written to the event log \
                             ({why})"
                        );
                    }
                }
            }));
            // A start put aside is taken back: its program's later lines are of no run (#2252).
            chats.when_a_held_start_is_settled(Box::new(move |session, how| {
                let plane = root.clone();
                let settled = events.record(match how {
                    crate::chats::HeldStart::GoesOn => purlis_core::eventlog::Entry::Begun {
                        plane,
                        number: session,
                    },
                    crate::chats::HeldStart::TakenBack => purlis_core::eventlog::Entry::TakenBack {
                        plane,
                        number: session,
                    },
                });
                if let Err(why) = settled {
                    tracing::warn!("purlis: a run was not written to the event log ({why})");
                }
            }));
        }
        // What each start decided about a chat's sandbox (ADR 0067 §7): its trust event, in the
        // event log under the run the start is about to begin and made durable before the chat's
        // program runs — an `Err` refuses a person's opt-out — and this machine's local count of
        // new chats with and without it (ruling V78 d), which `charter doctor` and Project
        // settings show.
        {
            let events = self.events.clone();
            let project = root.clone();
            chats.when_the_sandbox_is_decided(Box::new(move |decided| {
                if let Some(change) = decided.change {
                    let (Some(events), Some(run)) = (&events, decided.run) else {
                        return Err("purlis's event log is not open on this machine".to_owned());
                    };
                    events
                        .record(purlis_core::eventlog::Entry::Trust {
                            chat: run.chat.to_owned(),
                            run: run.run.to_owned(),
                            kind: change.kind().to_owned(),
                            body: change.body(decided.harness, decided.persona),
                        })
                        .map_err(|why| format!("the event log refused it: {why}"))?;
                }
                if let Some(counted) = decided.counted {
                    purlis_core::sandbox::local::count(&project, counted)
                        .map_err(|why| why.to_string())?;
                }
                Ok(())
            }));
        }

        // **Before a single session is started, because putting the record back starts them.**
        // A harness fires `SessionStart` at its own exec, and a board that learned the chat's
        // number afterwards would miss it — for a chat that is then idle, waiting for a first
        // prompt, no second event ever comes and it reads `unknown` for the rest of the run.
        // What the chat will read of the plane's instructions is taken here too, before its
        // program can read them (charter#369).
        let started_on: StartedOn = Arc::default();
        // When each chat began to start, for its MCP servers' starts (qw381).
        let mcp_failures: Arc<crate::mcpfailed::Failures> = Arc::default();
        // The chats whose program ends with their session host (#2007).
        let lost_with_the_host: Arc<crate::hostlost::Lost> = Arc::default();
        {
            let board = hooks.shared_board();
            chats.when_one_starts(Box::new({
                let board = Arc::clone(&board);
                let started_on = Arc::clone(&started_on);
                let mcp_failures = Arc::clone(&mcp_failures);
                let root = root.clone();
                move |session, harness, conversation, graded| {
                    mcp_failures.began(session, std::time::Instant::now());
                    // A grade's chat is drawn nowhere, so it is never in the queue (#3583).
                    hooks::held_board(&board).opened_as(session, harness, conversation, graded);
                    // Read before the lock is taken: a stamp is a read of several files.
                    let stamp = Stamp::of(&root);
                    lock_started(&started_on).insert(session, stamp);
                }
            }));
            // A chat reattached in the session host is on the board the host kept (#2006): it
            // is opened there only where that board did not come.
            chats.when_one_is_reattached(Box::new({
                let board = Arc::clone(&board);
                let started_on = Arc::clone(&started_on);
                let root = root.clone();
                move |session, harness, conversation, graded| {
                    let stamp = Stamp::of(&root);
                    {
                        let mut board = hooks::held_board(&board);
                        if !board.chats().any(|chat| chat == session) {
                            board.opened_as(session, harness, conversation, graded);
                        }
                    }
                    lock_started(&started_on).insert(session, stamp);
                }
            }));
            // And taken back if the program then fails to start: the announcement has to come
            // first, so it can be about a chat that never happens.
            let started_on = Arc::clone(&started_on);
            let mcp_failures = Arc::clone(&mcp_failures);
            chats.when_one_does_not_start(Box::new(move |session| {
                hooks::held_board(&board).closed(session);
                lock_started(&started_on).remove(&session);
                mcp_failures.did_not_start(session);
            }));
        }
        // Read once here, and from then on only as far as each change the watch and auto-save
        // name (FD-10b).
        let model = Arc::new(Mutex::new(Model::read(&root)));
        let changes = applying(Arc::clone(&model), root.clone(), Arc::clone(&self.changes));
        // Before a chat can end, so its end is one the worker hears. Where the chats run: in
        // the session host, the host saves the plane, and this window starts none of its own.
        let autosave = chats.sessions().keep_saving(&root, {
            let changes = Arc::clone(&changes);
            let saves = Arc::clone(&self.saves);
            let plane = id.clone();
            let root = root.clone();
            Arc::new(move |looked: crate::autosave::Looked| {
                changes(
                    plane.clone(),
                    crate::autosave::what_it_did(looked.moved_tree),
                );
                if looked.saved {
                    saves(plane.clone(), root.clone());
                }
            })
        });
        // No hook can report a program dying (the process is gone), so the operating system
        // does. That is not charter reading a harness's output (ADR 0018) — it is the
        // process's own exit status, and the only honest source for `failed`.
        // The plane itself, once it is held, for what a program's end has to reach. Weak for
        // the handoff's reason; set by `open` as soon as the plane is in its `Arc`.
        let me: Arc<std::sync::OnceLock<std::sync::Weak<Held>>> = Arc::default();
        let typed = Arc::new(crate::curation::Typed::default());
        let closing = Arc::new(crate::smartclose::Closing::default());
        {
            let board = hooks.shared_board();
            let tell = Arc::clone(&self.tell);
            let plane = id.clone();
            let poke = std::sync::Mutex::new(autosave.poker());
            let typed = Arc::clone(&typed);
            let closing = Arc::clone(&closing);
            let smart = Arc::clone(&self.smart);
            let me = Arc::clone(&me);
            let every_agent = Arc::clone(&self.kill_switch);
            // The event log, and the key the hook channel records this project's chats under.
            let events = self.events.clone();
            let events_root = id.root().to_path_buf();
            // Who hears of a chat whose program ended with its session host (#2007).
            let lost = Arc::clone(&lost_with_the_host);
            let tell_lost = Arc::clone(&self.lost_with_the_host);
            chats
                .sessions()
                .when_one_ends(Arc::new(move |session, exit| {
                    // A curation prompt still waiting is for a chat that is gone: never typed.
                    typed.forget(session);
                    // **Its program ended with its session host** (#2007): what its run is left
                    // in is read from the board as it was, before its end moves it, and said on
                    // its pane with Resume. It did not end by itself: like a quit, it fails no
                    // task, and a task chat reports when it is resumed.
                    let lost_with = me
                        .get()
                        .and_then(std::sync::Weak::upgrade)
                        .filter(|held| held.chats.sessions().lost_with_its_host(session));
                    let with_the_host = lost_with.is_some();
                    if let Some(held) = lost_with {
                        let resumes_by_id = held
                            .chats
                            .harness(session)
                            .is_some_and(purlis_core::harness::Harness::resumes_by_id);
                        let (state, asking, resumable) = {
                            let board = hooks::held_board(&board);
                            (
                                board.state(session),
                                board.asking(session),
                                board.conversation(session).is_some(),
                            )
                        };
                        if let Some(left_in) =
                            crate::hostlost::after_the_host(state, asking, resumes_by_id, resumable)
                        {
                            lost.note(session, crate::hostlost::LostRun { left_in, resumable });
                            tell_lost(crate::hostlost::LostChat {
                                plane: plane.clone(),
                                session,
                                left_in,
                                resumable,
                            });
                        }
                    }
                    // A run ended so that the same chat starts again at once (ADR 0067 as
                    // amended 2026-10-16) has not ended the chat: what follows its place says
                    // so. Its sub-agents' runs still end with it, stopped as a close stops
                    // them (ADR 0076 §6), and nothing else here is done for it. Where the
                    // session host writes the event log, it ends them as the program ends
                    // there (#2003).
                    if me
                        .get()
                        .and_then(std::sync::Weak::upgrade)
                        .is_some_and(|held| held.chats.replaced(session))
                    {
                        if let Some(events) = events.as_ref().and_then(hooks::Events::held) {
                            let (state, cause) = how_its_run_ended(
                                Some(purlis_core::state::run::StopBy::Closed),
                                &exit,
                            );
                            its_children_end(events, &events_root, session, state, cause);
                        }
                        return;
                    }
                    // A chat that ended on its own while it wrapped up wrote no record, and the
                    // window says so (ADR 0064). One closed by Close or by its record was let go
                    // of before its program was ended, so it says nothing here.
                    // One whose record the app had already written under its pass ended the
                    // way it was asked to, and is told as closed on that record (#1332).
                    if let Some(record) = closing.ended(session) {
                        smart(crate::smartclose::SmartClosing {
                            plane: plane.clone(),
                            session,
                            phase: if record.is_some() {
                                crate::smartclose::Phase::Closed
                            } else {
                                crate::smartclose::Phase::Ended
                            },
                            record,
                        });
                    }
                    let changed = hooks::held_board(&board).exited(session, hooks::code_of(&exit));
                    // **Its sub-agents' runs end with its own** (ADR 0076 §6, #1084), in the
                    // end state and for the cause the host knows its run ended by: its own stop,
                    // or the exit status. Never a hook line, which the chat writes. Where the
                    // session host writes the event log, it records this as the program ends
                    // there, window or no window (#2003).
                    if let Some(log) = events.as_ref().and_then(hooks::Events::held) {
                        let held = me.get().and_then(std::sync::Weak::upgrade);
                        let by = the_host_s_stop(
                            every_agent.is_stopped(),
                            held.as_ref().is_some_and(|held| held.chats.ending()),
                            held.as_ref()
                                .is_some_and(|held| held.stopping().is_stopping(session)),
                            held.as_ref()
                                .is_some_and(|held| held.chats.recorded_chat(session).is_none()),
                        );
                        let (state, cause) = how_its_run_ended(by, &exit);
                        its_children_end(log, &events_root, session, state, cause);
                    }
                    // A persona chat that still owed its asking chat a report has ended
                    // without one: that chat is told it failed, now (#1443). Before the
                    // window is told, so what it reads next already says so. **Only for a
                    // program that ended on its own** (D-1443-10): not at a quit or as the
                    // project is let go of, and not when every agent was stopped. Those chats
                    // are kept, and report when they are started again.
                    //
                    // **One word for one end** (D-T59-j3). A task its asking chat cancelled
                    // reports as cancelled, so that is written first (#1441) and nothing is
                    // then owed. A chat the person is stopping is told of by its stop, below
                    // (#1448), and is not also said to have failed by itself.
                    if let Some(held) = me.get().and_then(std::sync::Weak::upgrade)
                        && !held.chats.ending()
                        && !every_agent.is_stopped()
                        && !held.stopping().is_stopping(session)
                        && !with_the_host
                    {
                        crate::dispatched::moved(&held, session);
                        crate::handoff::its_program_ended(&held, session);
                    }
                    if changed {
                        tell(hooks::now(&board, plane.clone(), session));
                    }
                    // A chat that ended is the moment its work is done: auto-save hears it.
                    let _ = poke
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .send(crate::autosave::Poke::SessionEnded);
                    // And the record no longer names the ended program's pid (V82, #1018).
                    if let Some(held) = me.get().and_then(std::sync::Weak::upgrade) {
                        held.chats.a_program_ended();
                        // A restart it was owed waited for a pause that will not come: nothing
                        // restarts it (#1986).
                        held.chats.its_program_ended(session);
                        // A command waiting on it is told, and so is whatever waited for it
                        // to move (#1441).
                        crate::dispatched::moved(&held, session);
                        // A chat being stopped whose program ended on its own has ended: the
                        // chat that asked is told, and its tab goes (#1448).
                        crate::stopping::exited(&held, session);
                    }
                }));
        }
        // A runner's chats are reconnecting while its link is down, and drawn again once it is
        // back (#2496).
        {
            let tell = Arc::clone(&self.reconnecting);
            let plane = id.clone();
            chats
                .sessions()
                .when_reconnecting(Arc::new(move |sessions, reconnection| {
                    tell(crate::reconnecting::ChatsReconnecting {
                        plane: plane.clone(),
                        sessions,
                        reconnection,
                    });
                }));
        }
        // A pool runner's chats' skill runs, logged in this plane's skill log (SK-20b, #3084).
        {
            let me = Arc::clone(&me);
            chats
                .sessions()
                .when_skill_runs_heard(Arc::new(move |pool, run| {
                    if let Some(held) = me.get().and_then(std::sync::Weak::upgrade) {
                        held.chats.skill_run_heard(pool, run);
                    }
                }));
        }

        // Never fatal, as the socket above is not: a plane that cannot be watched still opens,
        // and its panels read it when a workspace is focused.
        let watch = caught_up(&model, &root, || {
            crate::planewatch::Watch::start(id.clone(), &root, changes)
                .map_err(|why| {
                    tracing::warn!(
                        "purlis: {} is not watched ({why}); its panels will not follow changes \
                         made outside this window",
                        root.display()
                    );
                })
                .ok()
        });

        // The Asks about its chats' browsers, each that ends with its browser told to the
        // window for the Inbox (#2015).
        let needs_you = crate::browserneedsyou::NeedsYou::new({
            let told = Arc::clone(&self.browser_asks_ended);
            let plane = id.clone();
            Arc::new(move |ended| told(crate::browserneedsyou::AskEnded::of(&plane, ended)))
        });
        // What the chats of a dispatch said, expired once a day while the project is open
        // (#1556): the open and a timeline's read expire only what they read.
        let talk_sweep = crate::activity::Daily::start(root.clone(), Arc::clone(&me));
        // Where its browsers' notes are kept for (#2025).
        let browsers_root = root.clone();
        // The window is told of this project's chats' browsers as they come and go (#2024).
        let browsers_listed: Arc<dyn Fn() + Send + Sync> = {
            let listed = Arc::clone(&self.browsers_listed);
            let plane = id.clone();
            Arc::new(move || listed(plane.clone()))
        };
        // And of each of its chats whose browser timeline moved (#2017).
        let browser_timeline: Arc<dyn Fn(String) + Send + Sync> = {
            let moved = Arc::clone(&self.browser_timeline);
            let plane = id.clone();
            Arc::new(move |chat| moved(plane.clone(), chat))
        };
        let moves = Arc::new(crate::chatmoves::Moves::default());
        moves.telling({
            let (plane, told) = (id.clone(), Arc::clone(&self.moves_told));
            Arc::new(move |one| told(plane.clone(), one))
        });
        Held {
            id,
            root,
            hooks,
            tell: Arc::clone(&self.tell),
            chats,
            records,
            recorded,
            watch: Mutex::new(watch),
            model,
            autosave: Mutex::new(Some(autosave)),
            started_on,
            typed,
            closing,
            tasks: Arc::default(),
            smart: Arc::clone(&self.smart),
            stopping: crate::stopping::Stopping::default(),
            stops: Arc::clone(&self.stops),
            changes: Arc::clone(&self.changes),
            activity: Arc::clone(&self.activity),
            quiet: Arc::clone(&self.quiet),
            needs_you,
            brokered: purlis_core::brokered::Rate::default(),
            vault_refusals: crate::vaultroute::Refusals::default(),
            removal_asks: crate::removalasks::Asked::default(),
            browsers: crate::chatbrowsers::ChatBrowsers::default()
                .noting(self.browser_driver.clone(), &browsers_root)
                .telling(Arc::clone(&browsers_listed))
                .limited_by(Arc::new(purlis_core::browserlimits::Limits::here)),
            browser_driver: self.browser_driver.clone(),
            pool_tools: crate::pooltools::PoolTools::default(),
            browsers_listed,
            browser_timeline,
            proposals: crate::proposals::Proposals::default(),
            runner_writes: crate::runnerwrites::RunnerWrites::default(),
            runner_dispatches: crate::runnerdispatches::RunnerDispatches::default(),
            moves,
            mcp_failures,
            lost_with_the_host,
            dispatch_grants: crate::dispatchgrants::Store::default(),
            held_dispatches: crate::handoff::HeldDispatches::default(),
            at_limits: crate::atlimit::AtLimits::default(),
            unattended: crate::handoff::Unattended::default(),
            dispatches: crate::dispatches::Waiting::default(),
            touched_files: crate::taskchanges::Touched::default(),
            talk_sweep,
            me,
        }
    }

    /// Closes a plane: its record is written, its sessions are ended, its socket is released,
    /// and every window drawing it is told, whoever asked ([`Self::telling_closed`]).
    ///
    /// **Nothing of the plane on disk is touched** beyond the record the app already keeps
    /// there. Closing a project is the app letting go of it, never the plane going away.
    #[cfg(test)]
    pub fn close(&self, plane: &PlaneId) -> Result<(), String> {
        self.closing(plane)?.finish();
        Ok(())
    }

    /// [`Self::close`] in two halves (#1949): the plane is taken out of the registry now, so
    /// nothing asks it again, and the [`Closing`] answered lets go of it, which writes its
    /// record and ends its chats, wherever that may take its time.
    ///
    /// **The close holds the project's turn until it has let go of it** (#1949): an open of the
    /// same project waits for the close, so it never binds the socket, nor starts the chats,
    /// of a project whose old chats are still ending, and the windows are told it closed before
    /// it can be open again.
    pub(crate) fn closing(&self, plane: &PlaneId) -> Result<Closing, String> {
        let turn = self.turns.take(plane, Turn::Close);
        let held = self
            .map()
            .remove(plane)
            .ok_or_else(|| no_such(plane, "close"))?;
        Ok(Closing {
            plane: plane.clone(),
            held,
            released: Arc::clone(&self.released),
            closed: Arc::clone(&self.closed),
            one_at_a_time: Arc::clone(&self.letting_go),
            _turn: turn,
        })
    }

    /// The kill switch (OV-1, ADR 0071): interrupts and ends every chat and shell in every
    /// plane this process holds, in every window, and starts no chat until [`Self::rearm`].
    /// Answers how many it stopped.
    ///
    /// The switch is thrown BEFORE a single program is ended, so nothing can start in the
    /// second the ending takes; see [`crate::host::SessionHost::open`] for the start that was
    /// already under way. A switch that could not be kept on disk still stops everything here,
    /// and the error is the sentence saying so.
    pub fn stop_every_agent(&self, by: purlis_core::halt::Actor) -> Result<usize, String> {
        let kept = self.kill_switch.stop(by);
        let stopped = self.end_every_chat();
        kept.map(|()| stopped)
    }

    /// Ends every program in every plane at once, and returns when they are gone: what the
    /// switch does, and what the app does on hearing `charter stop --all` throw it. The chats
    /// stay, each reading as one whose program ended.
    pub fn end_every_chat(&self) -> usize {
        // The registry's lock is let go of before anything ends: an ending tells the board,
        // which tells the window, which may ask this registry something.
        let held: Vec<Arc<Held>> = self.map().values().map(Arc::clone).collect();
        crate::sessions::all_at_once(
            "charter-stopping-plane",
            held.into_iter()
                .map(|held| move || held.chats().sessions().stop_every_program()),
        )
        .into_iter()
        .sum()
    }

    /// Whether every agent is stopped, by the window or by `charter stop --all`.
    pub fn is_stopped(&self) -> bool {
        self.kill_switch.is_stopped()
    }

    /// Lets chats start again: the window's alone. Nothing that was stopped is restarted; the
    /// operator reopens what they want. Answers whether agents had been stopped.
    pub fn rearm(&self) -> Result<bool, String> {
        self.kill_switch.rearm()
    }

    /// The switch itself, for the watch that hears another process throw it.
    pub fn kill_switch(&self) -> Arc<crate::killswitch::KillSwitch> {
        Arc::clone(&self.kill_switch)
    }

    /// Every project this registry holds, the registry's lock let go of.
    pub fn every_held(&self) -> Vec<Arc<Held>> {
        self.map().values().map(Arc::clone).collect()
    }

    /// **Stop all stopped every browser** (#2024): each project's chats' browsers leave their
    /// rows and tabs, and the window is told.
    pub fn chat_browsers_stopped(&self) {
        for held in self.every_held() {
            let listing = held.browsers().listing();
            held.browsers().all_stopped(held.browser_driver());
            held.browsers_listed_since(listing);
        }
    }

    /// The plane a command is acting for, or the one sentence saying it is not open.
    pub fn held(&self, plane: &PlaneId) -> Result<Arc<Held>, String> {
        self.map()
            .get(plane)
            .map(Arc::clone)
            .ok_or_else(|| no_such(plane, "act on"))
    }

    /// Every plane this process holds, in no particular order.
    pub fn open_now(&self) -> Vec<PlaneId> {
        let mut open: Vec<_> = self.map().keys().cloned().collect();
        open.sort_by(|one, two| one.0.cmp(&two.0));
        open
    }

    /// The way out: every plane's record written, every plane's sessions ended.
    ///
    /// Tauri ends the process itself, which runs no destructor and waits for no thread, so
    /// this is called from the exit event and not left to a drop that never happens.
    ///
    /// The registry is emptied FIRST and let go of afterwards. Ending a session tells the
    /// board, which tells the window, which asks this registry what the chat is called — and
    /// a lock still held here would be a deadlock on the way out, in the one path an operator
    /// cannot escape by clicking something else.
    pub fn let_go_of_all(&self) {
        self.let_go_of_every_plane(false);
    }

    /// [`Self::let_go_of_all`], for the quit that restarts charter to install an update
    /// (charter-app#251): each plane's record says so, and the launch after the restart is
    /// left word of it ([`reopen::RESTARTED_TO_UPDATE`]), so its question can say why it is
    /// asking.
    ///
    /// **Everything is on disk before the restart is even asked for**, which is what makes a
    /// relaunch that fails lose nothing: the next launch, however it comes, reads the same
    /// records and asks the same question.
    ///
    /// The records first and the word after them. Word with no flagged record behind it says
    /// nothing; a flagged record with no word is an ordinary relaunch, which is the smaller
    /// wrong of the two if only one of the writes lands.
    pub fn let_go_of_all_to_update(&self) {
        self.let_go_of_all_to_update_choosing(None);
    }

    /// [`Self::let_go_of_all_to_update`], with what the person answered before the restart:
    /// every chat put back, or a fresh start (#1078). The launch after it takes that answer and
    /// does not ask "Reopen your sessions?" again.
    pub fn let_go_of_all_to_update_choosing(&self, chosen: Option<Choice>) {
        self.let_go_of_every_plane(true);
        if let Some(config) = self.config.as_deref()
            && let Err(why) = reopen::mark_restart_to_update_choosing(config, chosen)
        {
            tracing::warn!(
                "purlis: the launch after this restart will not say it followed an update \
                 ({why}); what was open is recorded all the same"
            );
        }
    }

    /// Empties the registry, then lets go of each plane it held. See [`Self::let_go_of_all`].
    fn let_go_of_every_plane(&self, to_update: bool) {
        // From here an open that ends lets go of its project instead of holding it (#1949): the
        // flag is read under the registry's lock, which the drain below takes too. And every close
        // waiting its turn goes on now, so each gets its share of the wait below.
        self.letting_go.quit();
        // Every open and close still under way first (#1949): a close is out of the registry
        // already, so nothing below reaches it, and its last record and its chats' end are its
        // own; an open holds its project in the registry once it is done, and is let go of below.
        let still = self.turns.all_done_within(self.quit_waits);
        if still > 0 {
            tracing::warn!(
                "purlis: {still} project(s) were still being opened or let go of as purlis quit; \
                 what they write may not be written"
            );
        }
        // The window's tabs as last handed over, on disk before anything ends: after the wait,
        // since a close that let go meanwhile told the window, which reported them again.
        self.arranged.flush();
        let all: Vec<_> = self.map().drain().map(|(_, held)| held).collect();
        // Every command purlis is running with a vault for a chat ends here, with everything it
        // started that stayed in its process group (#1407).
        purlis_core::secrets::brokered::stop_every_run();
        // Every plane's saving stops at once, against one short deadline, so a save at quit
        // never overlaps the session host's; a host that does not answer is ended first
        // (#2002).
        crate::autosave::stop_every_saver_by(
            all.iter().filter_map(|held| held.take_saver()).collect(),
            std::time::Instant::now() + crate::autosave::QUIT_STOPS_WITHIN,
            &*self.end_the_host,
        );
        // Before the chats are ended: a turn this quit cuts off leaves its repo half-written,
        // and that repo is not saved (charter-app#299).
        let roots: Vec<_> = all
            .iter()
            .map(|held| (held.root.clone(), held.mid_turn_everywhere()))
            .collect();
        // Where each project's browsers are, before they end with its chats (#2022).
        for held in &all {
            crate::chatbrowsers::kept_at_quit(held);
        }
        for held in all {
            held.let_go(to_update);
            (self.released)(&held.root);
        }
        // Every way out of the app lets go of every plane here — a quit, and a restart to
        // update — so this is where each is saved: after its chats have ended, so what they
        // last wrote is in it (ADR 0051).
        crate::autosave::at_quit(roots);
    }

    /// The registry, whether or not a thread panicked while holding it. What it holds is
    /// still the best answer there is, and refusing to draw anything at all would be worse.
    /// This open's turn at project `id` (#1027): it waits while another open of the same project
    /// is between its sweep and holding it, and an open of any other project waits for nothing.
    fn opening(&self, id: &PlaneId) -> OpenTurn {
        self.turns.take(id, Turn::Open)
    }

    /// Refuses, with why, an open of `root` while a close of it is still letting go of it after
    /// [`CLOSE_WAIT`] (#1949); waits for it until then. An open of a project being closed would
    /// otherwise bind its socket and start its chats while the close still ends the old ones.
    fn not_closing(&self, root: &Path) -> Result<(), String> {
        self.not_closing_within(root, CLOSE_WAIT)
    }

    /// [`Self::not_closing`], waiting at most `within`.
    fn not_closing_within(&self, root: &Path, within: std::time::Duration) -> Result<(), String> {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        match self.turns.free_within(&PlaneId::of(&root), within) {
            None => Ok(()),
            Some(Turn::Close) => Err(STILL_CLOSING.to_owned()),
            Some(Turn::Open) => Err(STILL_OPENING.to_owned()),
        }
    }

    fn map(&self) -> MutexGuard<'_, HashMap<PlaneId, Arc<Held>>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The one sentence for a plane the process is not holding.
///
/// It names the plane, because with several open "no plane" alone tells the operator nothing
/// about which one went.
fn no_such(plane: &PlaneId, doing: &str) -> String {
    format!(
        "purlis has no plane open at {}, so there is nothing to {doing}",
        purlis_core::shown::short(plane.as_str())
    )
}

/// What came of being asked to open a plane.
pub enum Opening {
    /// It is open, and this is the id every later call names it by.
    Open(PlaneId),
    /// The operator has to be asked first. **Nothing was attached and nothing was started.**
    Ask(Asking),
}

/// What the store in `config` says about opening `root` as it stands right now.
///
/// **Every unreadable state is "ask".** No config home, a store charter would not read, a
/// platform charter keeps no store on: [`machine::read`] answers with an empty store in each
/// case, and an empty store's [`machine::Store::consent`] is [`machine::Consent::New`].
/// Silence is never a yes — `profiletrust` opens its module documentation refusing exactly
/// that, and this is the same answer about a bigger grant.
fn consent_of(
    config: Option<&Path>,
    root: &Path,
    contributes: &machine::Contribution,
) -> machine::Consent {
    let Some(config) = config else {
        return machine::Consent::New;
    };
    machine::read(config).store.consent(root, contributes)
}

/// The question about `root`, which contributes `contributes` as just read, when the store in
/// `config` does not already hold the yes to it.
fn asking_of(
    config: Option<&Path>,
    root: &Path,
    contributes: machine::Contribution,
) -> Option<Asking> {
    let consent = consent_of(config, root, &contributes);
    consent.must_ask().then(|| Asking {
        root: root.to_path_buf(),
        contributes,
        consent,
    })
}

/// Why a refused theme's Review and allow is not offered, and where the project is reviewed
/// instead (#2647).
pub(crate) const MORE_THAN_FONTS: &str = "this project asks for more than font files since you \
     approved it, so it is not allowed from a theme. Close the project and open it again to \
     review everything it asks for";

/// Why a refused theme's Review and allow is not offered for a project this machine never
/// approved (#2647): it is reviewed where it is opened.
pub(crate) const NOT_APPROVED: &str = "this project has not been approved on this machine, so \
     its font files are not allowed from a theme. Close the project and open it again to review \
     everything it asks for";

/// **What a refused theme's Notice may offer for the project at `root`** (#2647), from the
/// store in `config`: Review and allow when it asks again about font files alone; no button,
/// and why, when it asks for more or was never approved here; `None` when it asks about
/// nothing. It reads and hashes what the project contributes, so the window's theme asks it
/// only when the faces its refusal waits on change ([`crate::extensions::review_kept`]).
pub(crate) fn theme_review(config: &Path, root: &Path) -> Option<crate::extensions::ThemeReview> {
    use crate::extensions::ThemeReview;
    let contributes = machine::Contribution::read(root).contributes;
    let asking = asking_of(Some(config), root, contributes)?;
    Some(if asking.first() {
        ThemeReview::Unapproved
    } else if asking.consent.grew_in_fonts_alone() {
        ThemeReview::Fonts
    } else {
        ThemeReview::More
    })
}

/// **The theme folders whose font files an ask is about** (#2647): each changed font file's
/// theme folder, and the workspace it is in (`None` for the project's own `themes/`), in the
/// order the changes name them, once each. What Review and allow names in its dialog.
pub(crate) fn font_themes(consent: &machine::Consent) -> Vec<(String, Option<String>)> {
    let mut themes: Vec<(String, Option<String>)> = Vec::new();
    for change in consent.changes() {
        let (machine::Change::FontAdded(path) | machine::Change::FontChanged(path)) = change else {
            continue;
        };
        let parts: Vec<&str> = path.split('/').collect();
        let found = match parts.as_slice() {
            ["themes", theme, "fonts", _] => Some(((*theme).to_owned(), None)),
            ["workspaces", workspace, "themes", theme, "fonts", _] => {
                Some(((*theme).to_owned(), Some((*workspace).to_owned())))
            }
            _ => None,
        };
        if let Some(one) = found
            && !themes.contains(&one)
        {
            themes.push(one);
        }
    }
    themes
}

/// What the operator is being asked about, when a plane must be approved before it opens.
pub struct Asking {
    /// The plane's root as charter resolved it — canonical, and the spelling the approval is
    /// recorded against. Never the one the caller typed: an approval filed under one spelling
    /// of a directory and read back under another is an approval nobody gets to use.
    pub root: PathBuf,
    /// What it would contribute, as it is on this disk at this instant.
    pub contributes: machine::Contribution,
    /// Whether nothing has approved it, or it now does more than what was approved — and, for
    /// the second, every way it differs.
    pub consent: machine::Consent,
}

impl Asking {
    /// Whether nothing has approved this plane at all, rather than its having changed since it
    /// was approved.
    ///
    /// The two read differently to an operator — one is "open this?" and the other is "this is
    /// not what you said yes to" — so the difference is named once, here, and not spelled out
    /// again by every caller that has to tell them apart.
    pub fn first(&self) -> bool {
        matches!(self.consent, machine::Consent::New)
    }
}

/// The plane a path names, resolved to the one spelling everything downstream uses.
///
/// **`find_root`, never `resolve`.** `resolve` honours `$CHARTER_ROOT`, so a window built on
/// it would answer "the plane you picked" with a completely different directory whenever that
/// variable happened to be set — an operator picking a folder and being handed somebody else's
/// plane. `find_root` walks up from the directory itself and looks at nothing else, which is
/// what "open this folder" means. (The launch's own resolution is the opposite case and stays
/// on `resolve`: there the variable IS the operator saying which plane they meant.)
///
/// **Canonicalised first, and a symlink is followed rather than refused** — which is the
/// other way round from [`machine::still_a_plane`], deliberately. A path the operator is
/// picking right now has no approval behind it yet, so following the link and approving what
/// it really points at is honest. A REMEMBERED path that has become a link is the case where
/// an approval already exists for whatever it pointed at before, and charter cannot tell the
/// two apart, so that one is dropped.
fn plane_at(root: &Path) -> Result<PathBuf, String> {
    let shown = purlis_core::shown::short(&root.display().to_string());
    let here = root
        .canonicalize()
        .map_err(|why| format!("purlis cannot open {shown}: {why}"))?;
    purlis_core::plane::find_root(&here).map_err(|_| {
        format!(
            "{shown} is not a plane: purlis found no {} or {} there or in any directory above \
             it",
            purlis_core::names::PLANE_MANIFEST.reads[0],
            purlis_core::names::PLANE_MANIFEST.write,
        )
    })
}

/// Now, in seconds since the epoch — and zero on a clock that says it is before 1970, because
/// a timestamp in the recents list is worth no launch at all.
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// What one window is holding: its projects as tabs, and which of them is in front.
///
/// **The tabs and the front one are one value, not two.** A window that told charter its front
/// plane in one call and its tab strip in another would have two answers to "what is on
/// screen" and nothing keeping them in step — which is the singleton `Plane` ADR 0033 had to
/// undo, wearing a tab bar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Holding {
    /// Left to right, as the project tabs show them.
    pub planes: Vec<PlaneId>,
    /// Which tab is in front. `None` is the opener: the window holds projects the operator is
    /// not looking at, or holds none at all.
    pub active: Option<usize>,
}

impl Holding {
    /// The plane the operator is actually looking at, if any.
    ///
    /// An `active` past the end reads as none rather than panicking or wrapping: it can only
    /// arrive from a window that is mid-change, and "not looking" is the answer that sends a
    /// notification rather than swallowing one.
    pub fn front(&self) -> Option<&PlaneId> {
        self.planes.get(self.active?)
    }
}

/// What each window is holding, and which of its projects it has in front.
///
/// **This is the map #111 named as missing, and the reason it was missing is the reason it is
/// needed.** `asknotify::looking` asks a chat's OWN plane whether that chat is in front,
/// because every plane numbers its chats from one — and with two planes open that is only
/// half the question. Plane A's chat 3 can be in front *of plane A* while the window is
/// showing plane B, and the notification the operator actually needed is the one that gets
/// suppressed. The window is the only thing that knows which plane it is drawing, so the
/// window says.
///
/// It is also what a cold launch restores from: the arrangement written into this machine's
/// store is [`Self::arrangement`] and nothing else, so "what charter remembers about the
/// window" and "what the window told charter" cannot disagree.
///
/// Keyed by window label rather than held as one value, because a window is what the operator
/// arranges projects in, and there can be several (ADR 0033, amended 2026-09-26): a project tab
/// split out into its own OS window is held by that window and by no other.
///
/// **One project is held by at most one window**, and this is the module that keeps that true.
/// Moving a project takes it out of the window it was in and puts it in the other in one step,
/// under one lock, so there is no moment at which two windows hold it or none does. Every
/// question the rest of the app asks about window identity — which window a chat's event goes
/// to, whether the operator is looking at a chat, what a closed window hands back, what a split
/// window was given to draw — is answered here.
#[derive(Default)]
pub struct Showing {
    held: Mutex<HashMap<String, Holding>>,
    /// The last split window's number, so a label is never used twice in one process.
    split: std::sync::atomic::AtomicU32,
}

impl Showing {
    fn held(&self) -> MutexGuard<'_, HashMap<String, Holding>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A window says what it is holding and which project it has in front.
    ///
    /// A window holding nothing is forgotten rather than recorded as empty: it is an opener
    /// with nothing open, and an empty window in the arrangement would restore as nothing at
    /// the next launch while still being a row charter had to write down.
    ///
    /// **A window cannot claim a project another window holds.** What a window says can be a
    /// moment behind — it reports its tabs after it draws them, and a project moved out of it
    /// is still on the tabs it reported just before — and a project only changes window through
    /// [`Self::move_into`] or [`Self::close_into`]. So a project another window holds is left
    /// out of what this one is recorded as holding.
    pub fn in_window(&self, window: &str, mut holding: Holding) {
        let mut showing = self.held();
        let elsewhere: Vec<PlaneId> = holding
            .planes
            .iter()
            .filter(|plane| {
                showing
                    .iter()
                    .any(|(label, other)| label != window && other.planes.contains(plane))
            })
            .cloned()
            .collect();
        for plane in &elsewhere {
            holding.take_out(plane);
        }
        if holding.planes.is_empty() {
            showing.remove(window);
        } else {
            showing.insert(window.to_owned(), holding);
        }
    }

    /// Whether `window` has `plane` in front.
    ///
    /// A window that has never said is treated as showing nothing, so a notification is sent
    /// rather than suppressed. That is the cheap way round: a notification the operator did
    /// not need costs a glance, and one they needed and did not get costs a chat sitting
    /// unanswered.
    ///
    /// **A project the window HOLDS but is not looking at is not in front**, which is the
    /// whole of what project tabs added to this question: fifty chats can be live in a tab
    /// behind the one on screen, and a notification about one of them is exactly the
    /// notification the operator needs.
    pub fn is_showing(&self, window: &str, plane: &PlaneId) -> bool {
        self.held().get(window).and_then(Holding::front) == Some(plane)
    }

    /// The window holding `plane`, in front or behind — or none, when no window has said it
    /// holds it yet (it is being opened, or the window holding it has not drawn it).
    pub fn holder(&self, plane: &PlaneId) -> Option<String> {
        self.held()
            .iter()
            .find(|(_, holding)| holding.planes.contains(plane))
            .map(|(label, _)| label.clone())
    }

    /// What `window` holds, as it was last said or handed to it. A split window asks this once
    /// it is drawn, because it was made to hold what it was handed and nothing else.
    pub fn holding(&self, window: &str) -> Option<Holding> {
        self.held().get(window).cloned()
    }

    /// Every window holding something, by label, in [`Self::arrangement`]'s order.
    pub fn windows(&self) -> Vec<(String, Holding)> {
        let showing = self.held();
        let mut labels: Vec<&String> = showing.keys().collect();
        labels.sort_by_key(|label| crate::windows::order_key(label));
        labels
            .into_iter()
            .filter_map(|label| Some((label.clone(), showing.get(label)?.clone())))
            .collect()
    }

    /// Every window's arrangement, in a stable order: the main window first, then the split
    /// windows in the order they were made.
    ///
    /// Ordered by window label rather than by whatever the map iterates, so that writing the
    /// arrangement twice with nothing changed writes the same bytes twice — and the main window
    /// first, because the first remembered window is the one a cold launch puts back into it.
    pub fn arrangement(&self) -> Vec<Holding> {
        self.windows()
            .into_iter()
            .map(|(_, holding)| holding)
            .collect()
    }

    /// A label for a new split window, never one this process has used before.
    pub fn fresh_label(&self) -> String {
        let taken = self.held();
        loop {
            let next = self.split.fetch_add(1, Ordering::Relaxed) + 1;
            let label = crate::windows::split_label(next);
            if !taken.contains_key(&label) {
                return label;
            }
        }
    }

    /// Moves `planes` from the window `from` into `to`, **in one step**, under one lock.
    ///
    /// A project may be moved by the window holding it, or when no window holds it yet (a
    /// restore opens projects before it moves them into their window). **One held by a third
    /// window is refused**, and answered, and nothing moves: a window cannot take a project out
    /// from under another. The check and the move are one step, so two moves cannot both pass
    /// it.
    ///
    /// A window a project leaves keeps its other projects, and if the one it had in front was
    /// the one that left, the tab beside it comes forward — `closeTab`'s rule one scope up, and
    /// the same rule the window follows when a project is closed. A window left holding nothing
    /// is forgotten. `front`, when it is one of `planes`, is what `to` has in front afterwards;
    /// otherwise `to` keeps what it had in front.
    pub fn move_into(
        &self,
        from: &str,
        planes: &[PlaneId],
        front: Option<&PlaneId>,
        to: &str,
    ) -> Result<(), PlaneId> {
        let mut showing = self.held();
        for plane in planes {
            let theirs = showing.iter().any(|(label, holding)| {
                label != from && label != to && holding.planes.contains(plane)
            });
            if theirs {
                return Err(plane.clone());
            }
        }
        for (label, holding) in showing.iter_mut() {
            if label == to {
                continue;
            }
            for plane in planes {
                holding.take_out(plane);
            }
        }
        showing.retain(|_, holding| !holding.planes.is_empty());
        let target = showing.entry(to.to_owned()).or_default();
        for plane in planes {
            if !target.planes.contains(plane) {
                target.planes.push(plane.clone());
            }
        }
        if let Some(at) =
            front.and_then(|front| target.planes.iter().position(|plane| plane == front))
        {
            target.active = Some(at);
        }
        if target.planes.is_empty() {
            showing.remove(to);
        }
        Ok(())
    }

    /// A window is closed: whatever it held goes to `into`, behind what `into` has in front,
    /// and the window is forgotten. Answers what was handed over.
    ///
    /// Nothing is ended. A split window's chats are children of this process like every other
    /// chat, and a close that ended them would end the day's work (ADR 0033, amended
    /// 2026-09-26) — so its projects go back to the window they were split from.
    pub fn close_into(&self, window: &str, into: &str) -> Vec<PlaneId> {
        let mut showing = self.held();
        let Some(gone) = showing.remove(window) else {
            return Vec::new();
        };
        if window == into {
            return Vec::new();
        }
        let target = showing.entry(into.to_owned()).or_default();
        for plane in &gone.planes {
            if !target.planes.contains(plane) {
                target.planes.push(plane.clone());
            }
        }
        gone.planes
    }
}

impl Holding {
    /// Takes one project out, bringing the tab beside it forward when it was the one in front.
    fn take_out(&mut self, plane: &PlaneId) {
        let Some(at) = self.planes.iter().position(|held| held == plane) else {
            return;
        };
        self.planes.remove(at);
        self.active = match self.active {
            _ if self.planes.is_empty() => None,
            Some(front) if front == at => Some(at.saturating_sub(1)),
            Some(front) if front > at => Some(front - 1),
            other => other,
        };
    }
}

/// Whether this launch puts back the window set charter remembers.
///
/// `--no-restore` starts clean (spec decision 28), and it exists for the launch after the one
/// that restored something the operator did not want. It is read from the process's own
/// arguments once, at startup, and never again: a second launch's arguments reach the running
/// process through the single-instance plugin, and a `--no-restore` typed then would be about
/// a restore that happened hours ago.
pub struct Restoring(bool);

/// The argument that starts clean. Spelled once, so the flag and the test for it are one word.
pub const NO_RESTORE: &str = "--no-restore";

impl Restoring {
    /// What this launch's arguments say.
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Self {
        Self(!args.into_iter().any(|arg| arg == NO_RESTORE))
    }

    /// [`Self::from_args`], unless this launch follows a restart to update (charter-app#251),
    /// which always puts the window set back.
    ///
    /// Tauri's restart starts the new process with the old one's arguments, so a charter that
    /// was started with `--no-restore` would otherwise come back from Restart to update without
    /// the projects it was holding a moment earlier — and without asking about their chats. The
    /// flag was about the launch it was typed at, which the restart continues rather than
    /// repeats.
    pub fn after(args: impl IntoIterator<Item = String>, restarted_to_update: bool) -> Self {
        Self(restarted_to_update || Self::from_args(args).0)
    }

    pub fn wanted(&self) -> bool {
        self.0
    }
}

/// The window set a cold launch has to put back, and every project it would not take back.
pub struct Restorable {
    /// Every project to open again, window by window and left to right within each: what
    /// the launch's question asks about, whichever window each goes back into.
    pub planes: Vec<PathBuf>,
    /// The windows, the main window's first. A window every one of whose projects was
    /// dropped is not here: an empty window would restore as nothing.
    pub windows: Vec<RestoredWindow>,
    /// One line per window entry the store itself would not take back.
    pub dropped: Vec<String>,
    /// The projects that have moved or gone, which the window offers Locate… and Forget for
    /// (NO-5).
    pub gone: Vec<crate::opener::GoneProject>,
}

/// One remembered window, checked against this disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoredWindow {
    /// Its projects, left to right as its tabs were.
    pub planes: Vec<PathBuf>,
    /// Which of them was in front, as an index into `planes` **after** the drops — so a
    /// window whose front project has gone comes back on one that is still there.
    pub active: Option<usize>,
}

/// The arrangement charter remembered, checked against this disk.
///
/// **A project that has moved or is gone is dropped with a line saying so, never an error
/// dialog** (ADR 0033). It is `put_back`'s rule for a chat whose profile has gone, and it is
/// the same reason: a restore is a convenience, and a convenience that blocks the launch is
/// worse than the thing it was restoring.
///
/// **Nothing here opens anything.** Opening a project starts the programs its record names, so
/// it goes through the trust gate like every other open — this only says which projects to
/// offer that gate. A restore that minted its own approval would be ADR 0035 turned off for
/// every project the operator had ever had open at once.
///
/// **Each remembered window comes back as a window** (ADR 0033, amended 2026-09-26). The first
/// is put back into the main window; the rest are split windows again. A project two windows
/// both claimed is kept by the first, because one project is one tab in one window.
pub fn restorable(loaded: machine::Loaded) -> Restorable {
    let mut planes: Vec<PathBuf> = Vec::new();
    let mut windows = Vec::new();
    // What the store itself would not take back, already dropped with a reason by the read.
    // Only the window rows: a remembered RECENT charter would not take back is the opener's
    // news, and saying it twice on one launch is saying it twice.
    let dropped: Vec<String> = loaded
        .dropped
        .iter()
        .filter(|why| matches!(why, machine::Dropped::Window { .. }))
        .map(ToString::to_string)
        .collect();
    let mut gone: Vec<crate::opener::GoneProject> = Vec::new();
    for window in loaded.store.windows {
        let was_active = window.active;
        let mut these: Vec<PathBuf> = Vec::new();
        let mut active = None;
        for (at, plane) in window.planes.into_iter().enumerate() {
            if let Err(why) = machine::still_a_plane(&plane) {
                let line = crate::opener::GoneProject::of(&plane, &why);
                // Two windows that both held it say so once.
                if !gone.contains(&line) {
                    gone.push(line);
                }
                continue;
            }
            // One project is one tab, in one window: a second tab on one plane would be a
            // second `PlaneView` drawing one board.
            if planes.contains(&plane) {
                continue;
            }
            if at == was_active {
                active = Some(these.len());
            }
            planes.push(plane.clone());
            these.push(plane);
        }
        if these.is_empty() {
            continue;
        }
        // A front tab that was dropped leaves the window on the first project that survived,
        // rather than on none: the operator asked for these projects, and an opener in front
        // of them is a screen they have to click past.
        windows.push(RestoredWindow {
            active: active.or(Some(0)),
            planes: these,
        });
    }
    Restorable {
        planes,
        windows,
        dropped,
        gone,
    }
}

/// The size a session starts at. The pane it lands in tells it the real one at once, and a
/// chat put back at a launch has no pane yet to ask.
const STARTING: Size = Size {
    columns: 80,
    rows: 24,
};

/// What a launch had to go on, and what came of it.
///
/// **Three states, and none of them is an error.** The app has to come up holding no plane at
/// all and stay useful — that is what an opener attaches to — and "there is no plane where you
/// launched me" is a different thing to tell an operator from "you have not opened one yet".
/// A window double-clicked from the dock has a working directory of `/` and is the second;
/// `charter` run in a directory that is in no plane is the first.
///
/// They are told apart by shape rather than by reading a sentence: `plane` set is a plane in
/// hand, `from` set without it is a directory that is in no plane, and neither is a launch
/// that was given nothing to go on.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct Launch {
    /// The plane this launch opened, and the id every command for it names.
    pub plane: Option<PlaneId>,
    /// The directory the launch was given, when it could be read. A HINT and nothing more:
    /// it is resolved once, here, and no command consults the working directory again.
    pub from: Option<String>,
    /// Why no plane was opened, in the resolver's own words.
    pub why: Option<String>,
}

/// Resolves the launch's working directory to a plane, and opens it.
///
/// **One rule, one answer, and this line is the whole of it.** `purlis_core::plane::resolve`
/// is the resolver the CLI asks and the one every command in this app used to ask — except
/// the launch, which asked `find_root`. The two disagree about `$CHARTER_ROOT`, so under the
/// scenario tests the app bound its hook socket and wrote its record against one plane while
/// the sidebar, the picker and every launch read another. Two resolvers disagreeing in a
/// worktree is what M2.16 cost a day to find; this was the same defect one layer up.
///
/// After this, the working directory is never consulted again: a plane is named explicitly
/// from here on.
///
/// `root` is [`launch_root`] of `cwd`, resolved before the window was built so the window opens
/// on the plane it names (#2602): resolved once, and taken here as it was answered.
pub fn at_launch(
    planes: &Planes,
    cwd: std::io::Result<PathBuf>,
    root: Option<Result<PathBuf, String>>,
) -> Launch {
    resolving_with(planes, cwd, |cwd| {
        root.unwrap_or_else(|| purlis_core::plane::resolve(cwd).map_err(|why| why.to_string()))
    })
}

/// **The plane the launch's working directory `cwd` is in**, by the one resolver
/// (`purlis_core::plane::resolve`), or why none; `None` when the directory could not be read.
pub fn launch_root(cwd: &std::io::Result<PathBuf>) -> Option<Result<PathBuf, String>> {
    let cwd = cwd.as_ref().ok()?;
    Some(purlis_core::plane::resolve(cwd).map_err(|why| why.to_string()))
}

/// [`at_launch`], with the resolver named — so a test can drive the three states without
/// reaching into the process's environment, which it shares with every other test.
fn resolving_with(
    planes: &Planes,
    cwd: std::io::Result<PathBuf>,
    resolve: impl FnOnce(&Path) -> Result<PathBuf, String>,
) -> Launch {
    // Taken whatever this launch opens, so the word a restart to update left is spent by the
    // launch after it and by no later one (charter-app#251). This runs once per process, from
    // `setup`: a second launch's arguments reach the running app through the single-instance
    // plugin and never come back through here to take the word a second time.
    let restarted = planes.config.as_deref().and_then(reopen::take_restart);
    {
        let mut relaunching = planes.relaunching();
        relaunching.after_update = restarted.is_some();
        relaunching.chosen = restarted.and_then(|restart| restart.chosen);
    }
    let cwd = match cwd {
        Ok(cwd) => cwd,
        // Nothing to go on at all. Not an error: an app launched from an icon has no useful
        // working directory to speak of, and it still has to come up.
        Err(err) => {
            return Launch {
                plane: None,
                from: None,
                why: Some(format!("purlis cannot read the current directory: {err}")),
            };
        }
    };
    let from = Some(cwd.display().to_string());
    match resolve(&cwd) {
        Ok(root) => {
            let plane = planes.open(&root);
            // **The first of the two approvals this module mints.** The operator ran charter
            // in this directory; that act is the yes, and it is the yes for THIS plane and no
            // other.
            //
            // `open` above remembered it but did NOT approve it, and the difference is ADR
            // 0035's: running charter here is consent to open this plane now, and the
            // recorded approval is an answer to a question the operator was shown and read.
            // So this plane appears in the opener's list — an operator who has only ever
            // launched from a terminal must not find that list empty — and opening it from
            // that list asks once, as every other plane does.
            //
            // **It is not put back HERE** (charter-app#250). The launch asks the operator
            // whether they want what was open back, and nothing starts before the answer — so
            // the plane is written down as owed, and [`Planes::relaunch`] spends this yes when
            // the window has the answer. That is where the record is read, once, against the
            // root the registry settled on (`record_of`): `Planes::open` canonicalises, and
            // reading `/var/…` while the plane is held as `/private/var/…` is the
            // two-spellings-of-one-directory defect the id exists to stop.
            planes.relaunching().owed = Some(plane.clone());
            Launch {
                plane: Some(plane),
                from,
                why: None,
            }
        }
        Err(why) => {
            tracing::warn!(
                "purlis: no plane here, so nothing is reopened and nothing is recorded \
                 (a plane is the nearest directory at or above this one with a charter.toml)"
            );
            Launch {
                plane: None,
                from,
                why: Some(why),
            }
        }
    }
}

/// **What the host ended chat `session`'s program for, if it was the host** (ADR 0076 §2,
/// D-1084-1): the kill switch, then a quit or the project let go of, then the person's stop of
/// it, then a close of its tab (a task ended at its report is closed too). `None` where the
/// program ended by itself, which its exit status then tells. Read from the host's own state,
/// never from anything a chat sent.
fn the_host_s_stop(
    killed: bool,
    quitting: bool,
    stopping: bool,
    closed: bool,
) -> Option<purlis_core::state::run::StopBy> {
    use purlis_core::state::run::StopBy;
    if killed {
        Some(StopBy::Killed)
    } else if quitting {
        Some(StopBy::Quit)
    } else if stopping {
        Some(StopBy::Operator)
    } else if closed {
        Some(StopBy::Closed)
    } else {
        None
    }
}

/// What a chat's end leaves in the event log, read and written as both binaries' session hosts
/// do (#2494).
pub(crate) use purlis_core::hookchannel::{how_its_run_ended, its_children_end};

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "planes/tests/restoring.rs"]
mod restoring_tests;

#[cfg(all(test, unix))]
#[path = "planes/tests/review.rs"]
mod review_tests;
