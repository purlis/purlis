// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Pool setup tab's commands** (spec #4469, #4480): what `purlis_core::poolsetup` answers, as
//! the window reads it, and the choices the person makes there. Reading changes nothing here;
//! each change happens on its own button.

use crate::dockerpools::PoolAdding;
use crate::planes::{PlaneId, Planes};

/// One step's next action.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupAction {
    pub id: String,
    pub label: String,
    pub does: String,
    pub command: Option<String>,
    pub url: Option<String>,
}

/// A step by hand: the exact commands, and what running them means (#4481).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupSee {
    pub note: String,
    pub text: String,
}

/// One line of what Try it checks (#4483): `ok`, `missing`, `todo` or `skip`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupCheck {
    pub verdict: String,
    pub title: String,
    pub more: Option<String>,
}

/// One step as it stands.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupStep {
    pub id: String,
    pub name: String,
    pub state: String,
    pub line: Option<String>,
    pub findings: Vec<String>,
    pub next: Option<PoolSetupAction>,
    pub skippable: bool,
    pub see: Option<PoolSetupSee>,
    pub checks: Vec<PoolSetupCheck>,
}

/// What the tab draws.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupShown {
    /// The pool this setup made.
    pub pool: Option<String>,
    /// The name typed on the Pool step, for the box to hold.
    pub name: Option<String>,
    /// The step the person has reached.
    pub at: Option<String>,
    pub steps: Vec<PoolSetupStep>,
    /// The pool was made by hand under the name typed, and purlis noticed (#4481).
    pub noticed: bool,
    /// Pools this machine keeps that this setup isn't on.
    pub others: Vec<String>,
}

/// What the Pool step's button did: nothing made, and why, where a team's image breaks the
/// contract; none where the pool was made.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupCreated {
    pub breaks: Vec<String>,
}

/// A core value's word, as it serializes.
#[cfg(unix)]
fn word(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|said| said.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// What the core answered, as the window takes it.
#[cfg(unix)]
pub fn shown(found: purlis_core::poolsetup::Resumed) -> PoolSetupShown {
    let purlis_core::poolsetup::Resumed {
        setup,
        name,
        noticed,
    } = found;
    PoolSetupShown {
        pool: setup.pool.clone(),
        name,
        noticed,
        others: setup.others.clone(),
        at: setup.at.as_ref().map(word),
        steps: setup
            .steps
            .iter()
            .map(|step| PoolSetupStep {
                id: word(&step.id),
                name: step.name.to_owned(),
                state: word(&step.state),
                line: step.line.clone(),
                findings: step.findings.clone(),
                next: step.next.as_ref().map(|next| PoolSetupAction {
                    id: word(&next.id),
                    label: next.label.clone(),
                    does: next.does.clone(),
                    command: next.command.clone(),
                    url: next.url.clone(),
                }),
                skippable: step.skippable,
                see: step.see.as_ref().map(|see| PoolSetupSee {
                    note: see.note.clone(),
                    text: see.text.clone(),
                }),
                checks: step
                    .checks
                    .iter()
                    .map(|check| PoolSetupCheck {
                        verdict: word(&check.verdict),
                        title: check.title.clone(),
                        more: check.more.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(unix)]
fn config_root() -> Result<std::path::PathBuf, String> {
    purlis_core::machine::config_root()
        .ok_or_else(|| "There is no config home on this machine to keep a pool in.".to_owned())
}

/// **Each step's state, read now**: the pool this setup made, else a new one's.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<PoolSetupShown, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("reading Pool setup", move || {
            use purlis_core::dockerpool::Docker;
            use purlis_core::poolsetup::{self, Here};
            let config = config_root()?;
            let docker = poolsetup::docker_here();
            let here = Here {
                config: &config,
                plane: Some(&root),
                docker: docker.as_ref(),
                engine: Docker::engine_on_this_machine(),
            };
            Ok(shown(
                poolsetup::resumed(&here).map_err(|why| why.to_string())?,
            ))
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = root;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps that the person chose Docker on Where.** Nothing else changes.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_choose_docker() -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", || {
            purlis_core::poolsetup::choose_kind(
                &config_root()?,
                purlis_core::poolsetup::Kind::Docker,
            )
            .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps the name typed on the Pool step**, or none, so coming back finds it.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_name(name: Option<String>) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            purlis_core::poolsetup::choose_name(&config_root()?, name.as_deref())
                .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = name;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps that this setup goes on with pool `name`**, once it has made it.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_pool(name: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            purlis_core::poolsetup::choose_pool(&config_root()?, &name)
                .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = name;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **The command that does what the Pool step's button would do with `adding`**: See what this
/// does, from the value the button sends (Q30). It reads nothing and changes nothing.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_command(adding: PoolAdding) -> Result<String, String> {
    #[cfg(unix)]
    {
        Ok(adding.asked().command())
    }
    #[cfg(not(unix))]
    {
        let _ = adding;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Makes the pool `adding` asks for**, on the Pool step's own button: what `purlis pool add`
/// runs, a team's image held to the contract first.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_create(adding: PoolAdding) -> Result<PoolSetupCreated, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("adding a pool", move || {
            use purlis_core::poolsetup::asked::{self, Created};
            let machine = crate::runnerlinks::Machine::here(&config_root()?)?;
            let desktop = machine.desktop_made()?.pinned();
            let made = asked::create(&machine.config, &machine.docker, &adding.asked(), &desktop)
                .map_err(|why| crate::dockerpools::refused_said(&why.to_string()))?;
            Ok(PoolSetupCreated {
                breaks: match made {
                    Created::Added => Vec::new(),
                    Created::Breaks(problems) => problems,
                },
            })
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = adding;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// One of this machine's profiles against a pool's limit (#4482).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupProfileSize {
    pub name: String,
    /// What a send to the runner carries, in MiB rounded up, and in files; null where unread.
    pub mib: Option<u32>,
    pub files: Option<u32>,
    /// Why it could not be measured.
    pub unread: Option<String>,
    /// Each limit it is over, in words, with what to raise it to.
    pub over: Vec<PoolSetupOver>,
    /// The project's default, or a profile an open chat runs on.
    pub in_use: bool,
}

/// A limit a profile is over.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupOver {
    /// The limit's key, as `pool_limit_set` takes it.
    pub key: String,
    pub said: String,
    /// What to raise it to, where a value would do.
    pub raise_to: Option<u32>,
}

/// This machine's profiles against pool `pool`'s limits (#4482).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupSizes {
    pub most_mib: u32,
    pub most_files: u32,
    pub profiles: Vec<PoolSetupProfileSize>,
}

/// A count as the window takes it.
#[cfg(unix)]
fn small(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// **This machine's profiles against pool `pool`'s limits**, the ones in use in `plane` marked:
/// measured as a send measures them, with no file read. It changes nothing.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_sizes(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    pool: String,
) -> Result<PoolSetupSizes, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("measuring your profiles", move || {
            let sizes = purlis_core::poolsetup::signins::sizes(&config_root()?, Some(&root), &pool)
                .map_err(|why| why.to_string())?;
            Ok(PoolSetupSizes {
                most_mib: small(sizes.most_mib),
                most_files: small(sizes.most_files),
                profiles: sizes
                    .profiles
                    .into_iter()
                    .map(|one| PoolSetupProfileSize {
                        name: one.name,
                        mib: one.mib.map(small),
                        files: one.files.map(small),
                        unread: one.unread,
                        over: one
                            .hits
                            .iter()
                            .map(|hit| PoolSetupOver {
                                key: hit.limit.limit().key.to_owned(),
                                said: hit.said(),
                                raise_to: hit.suggested.map(small),
                            })
                            .collect(),
                        in_use: one.in_use,
                    })
                    .collect(),
            })
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (root, pool);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps that the person skipped step `step` on pool `pool`, or takes the skip back.** Refused
/// for a step the ones after it need.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_skip(pool: String, step: String, skipped: bool) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            use purlis_core::poolsetup::{self, StepId};
            let step: StepId = serde_json::from_value(serde_json::Value::String(step))
                .map_err(|_| "Pool setup has no step of that name.".to_owned())?;
            let config = config_root()?;
            if skipped {
                poolsetup::skip(&config, &pool, step)
            } else {
                poolsetup::unskip(&config, &pool, step)
            }
            .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (pool, step, skipped);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Starts pool `pool`'s runner again**, on the Sign-ins step's own button: one a day without a
/// link stopped its own container, which is checked as it was made before it starts, as a link to
/// it would start it. It goes back to sleep by itself.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_wake(pool: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("waking the runner", move || {
            let machine = crate::runnerlinks::Machine::here(&config_root()?)?;
            let kept = purlis_core::runners::runner_for(&machine.config, &pool)
                .map_err(|why| why.to_string())?;
            crate::runnerlinks::checked_or_woken(&machine.docker, &machine.config, &kept, true)
                .map_err(|why| why.0)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = pool;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// Why Try it didn't pass, and the step that mends it, where one does (#4483).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupFailed {
    /// What happened and what to do, in a sentence or two.
    pub why: String,
    /// The step to go to, by its id: `signins` for a sign-in. Null where trying again is it.
    pub step: Option<String>,
}

/// What Try it found.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupTried {
    /// Why it didn't pass; null where a real chat started and the proof is kept.
    pub failed: Option<PoolSetupFailed>,
    /// The workspace its chat started in, where it got that far: where Open a chat opens.
    pub workspace: Option<String>,
}

/// **How long Try it waits for its chat's harness to report its start** once the chat is under
/// way, as long as a grade waits for its chat's (`skillgrade`'s `A_CHAT_BEGINS_WITHIN`). A chat
/// that waits on something no hook tells of, such as a question at its launch, has spent nothing
/// by then.
#[cfg(unix)]
const TRY_BEGINS_WITHIN: std::time::Duration = std::time::Duration::from_secs(120);

/// How often Try it looks whether its chat has reported its start.
#[cfg(unix)]
const TRY_LOOKS_EVERY: std::time::Duration = std::time::Duration::from_millis(500);

/// The size Try it's chat runs at: it is closed before anyone reads it.
#[cfg(unix)]
const TRY_SIZE: purlis_core::engine::Size = purlis_core::engine::Size {
    columns: 120,
    rows: 40,
};

/// The pools a Try it is under way on, so a second press waits for the first.
#[cfg(unix)]
static TRYING: std::sync::Mutex<std::collections::BTreeSet<String>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// **Try it** (#4483, Q32): starts a real chat on pool `pool`, on the Try it step's own button,
/// waits for the hook its harness reports its session's start by (ADR 0076), closes it, and keeps
/// that it started (`purlis_core::poolsetup::tried`). Nothing is typed into it. A start that is
/// refused, or a chat that never reports its start, comes back as `failed`, with the step that
/// mends it where one does.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_try(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    pool: String,
    workspace: Option<String>,
) -> Result<PoolSetupTried, String> {
    let held = planes.held(&plane)?;
    #[cfg(unix)]
    {
        crate::off_the_window("trying the pool", move || {
            purlis_core::runners::usable_pool_name(&pool)?;
            let config = config_root()?;
            let Some(_trying) = Trying::on(&pool) else {
                return Err(format!("Try it is already under way on {pool}."));
            };
            try_on(&held, &config, &pool, workspace.as_deref())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (held, pool, workspace);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **A Try it under way on one pool**: held for as long as it runs, and let go however it ends,
/// a panic too, so a second press waits for the first and a failed one never locks the pool.
#[cfg(unix)]
struct Trying(String);

#[cfg(unix)]
impl Trying {
    /// Holds `pool`, or none where a Try it is already under way on it.
    fn on(pool: &str) -> Option<Trying> {
        TRYING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(pool.to_owned())
            .then(|| Trying(pool.to_owned()))
    }
}

#[cfg(unix)]
impl Drop for Trying {
    fn drop(&mut self) {
        TRYING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.0);
    }
}

/// **Try it's chat, ended however Try it ends**: unwatched and closed on drop, a panic too, so
/// no chat it started is left running.
#[cfg(unix)]
struct TryChat<'a> {
    held: &'a std::sync::Arc<crate::planes::Held>,
    number: u32,
    session: Option<u32>,
}

#[cfg(unix)]
impl Drop for TryChat<'_> {
    fn drop(&mut self) {
        self.held.hooks().skill_runs().unwatch(self.number);
        if let Some(session) = self.session
            && let Err(why) = self.held.close_chat(session)
        {
            tracing::warn!("purlis: Try it's chat {session} did not close ({why})");
        }
    }
}

/// A failure of Try it, as the window takes it.
#[cfg(unix)]
fn failed(why: String, step: Option<purlis_core::poolsetup::StepId>) -> PoolSetupTried {
    PoolSetupTried {
        failed: Some(PoolSetupFailed {
            why,
            step: step.as_ref().map(word),
        }),
        workspace: None,
    }
}

/// **The step that mends a start the pool refused**: Sign-ins, where its runner found the
/// profile signed out (`purlis_core::profilestore::sign_in::is_refusal`); else none.
#[cfg(unix)]
fn step_of_a_refused_start(why: &str) -> Option<purlis_core::poolsetup::StepId> {
    purlis_core::profilestore::sign_in::is_refusal(why)
        .then_some(purlis_core::poolsetup::StepId::Signins)
}

/// Whether `workspaces/<name>` of the project at `root` is a folder of its own: never a link,
/// which could lead out of the project.
#[cfg(unix)]
fn a_workspace_folder(root: &std::path::Path, name: &str) -> bool {
    purlis_core::wscmd::workspace_dir(root, name)
        .and_then(|dir| dir.symlink_metadata().ok())
        .is_some_and(|meta| meta.is_dir())
}

/// **The workspace Try it starts its chat in, and its folder**: workspace `asked`, where the
/// tab is filed in one, else the project's first workspace by name. None where the project has
/// none.
#[cfg(unix)]
fn workspace_to_try(
    root: &std::path::Path,
    asked: Option<&str>,
) -> Option<(String, std::path::PathBuf)> {
    let found = |name: &str| {
        a_workspace_folder(root, name)
            .then(|| purlis_core::wscmd::workspace_dir(root, name))
            .flatten()
            .map(|dir| (name.to_owned(), dir))
    };
    if let Some(chosen) = asked.and_then(found) {
        return Some(chosen);
    }
    let mut names: Vec<String> = std::fs::read_dir(root.join("workspaces"))
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.iter().find_map(|name| found(name))
}

/// A wait in words: `2 minutes`, or `30 seconds` under a minute.
#[cfg(unix)]
fn said_as_time(wait: std::time::Duration) -> String {
    let counted =
        |count: u64, unit: &str| format!("{count} {unit}{}", if count == 1 { "" } else { "s" });
    match wait.as_secs() {
        secs @ 0..60 => counted(secs, "second"),
        secs => counted(secs.div_ceil(60), "minute"),
    }
}

/// **Waits for the chat `heard` tells of to report its session's start** (ADR 0076): its
/// harness's `SessionStart` hook, and nothing it prints. Why it didn't, said for pool `pool`,
/// where it ended first, its program was found gone (`still_running`, asked each look: a program
/// that exits reports no end of its own), or `within` passed.
#[cfg(unix)]
pub(crate) fn waited_for_its_start(
    heard: &std::sync::mpsc::Receiver<crate::skillrun::Watched>,
    pool: &str,
    within: std::time::Duration,
    still_running: impl Fn() -> bool,
) -> Result<(), String> {
    use crate::skillrun::Watched;
    use std::sync::mpsc::RecvTimeoutError;
    let began = std::time::Instant::now();
    loop {
        match heard.recv_timeout(TRY_LOOKS_EVERY.min(within)) {
            Ok(Watched::Began) => return Ok(()),
            Ok(Watched::Closed) | Err(RecvTimeoutError::Disconnected) => {
                return Err(format!(
                    "The chat on {pool} ended before it had started. Open a chat there yourself \
                     to see what it says, then try again."
                ));
            }
            Ok(_) | Err(RecvTimeoutError::Timeout) if !still_running() => {
                return Err(format!(
                    "The chat on {pool} ended before it had started. Open a chat there yourself \
                     to see what it says, then try again."
                ));
            }
            Ok(_) | Err(RecvTimeoutError::Timeout) => {
                if began.elapsed() >= within {
                    return Err(format!(
                        "The chat on {pool} didn't start within {}. Its harness may be waiting on \
                         a question, so open a chat there yourself and answer it, then try again.",
                        said_as_time(within)
                    ));
                }
            }
        }
    }
}

/// [`pool_setup_try`]'s work, off the window.
#[cfg(unix)]
fn try_on(
    held: &std::sync::Arc<crate::planes::Held>,
    config: &std::path::Path,
    pool: &str,
    workspace: Option<&str>,
) -> Result<PoolSetupTried, String> {
    use purlis_core::poolsetup::{self, Here, trying};
    use purlis_core::reopen::Chat;

    let root = held.root().to_path_buf();
    let docker = poolsetup::docker_here();
    let here = Here {
        config,
        plane: Some(&root),
        docker: docker.as_ref(),
        engine: purlis_core::dockerpool::Docker::engine_on_this_machine(),
    };
    let facts = poolsetup::read_pool(&here, pool).map_err(|why| why.to_string())?;
    let (set, _check) = purlis_core::profiles::for_launch(&root);
    let declared = purlis_core::harness_declaration::read(&root);
    let candidates: Vec<trying::Candidate> = set
        .profiles()
        .iter()
        .map(|profile| trying::Candidate {
            name: profile.name.clone(),
            reports_its_start: declared
                .get(&profile.kind)
                .and_then(purlis_core::harness_declaration::Declaration::ready_to_type)
                == Some(purlis_core::harness::ReadyToType::WhenItReportsItsStart),
        })
        .collect();
    let profile =
        match trying::profile_to_try(pool, &facts.sign_ins, set.default.as_deref(), &candidates) {
            Ok(profile) => profile,
            Err(not) => return Ok(failed(not.why, not.step)),
        };

    // A chat at the project's root runs on this machine, so Try it starts in a workspace.
    let Some((workspace, cwd)) = workspace_to_try(&root, workspace) else {
        return Ok(failed(
            "Try it starts its chat in a workspace, and this project has none yet. Add a \
             workspace, then try again."
                .to_owned(),
            None,
        ));
    };
    let number = held.chats().sessions().deal();
    let name = number.to_string();
    let start = purlis_core::start::Start {
        profile: Some(profile.clone()),
        persona: None,
        name: name.clone(),
        cwd: Some(cwd),
        resume: None,
        show_footer: false,
        resuming: None,
        // No opt-out of its own: sandboxed as the person's own new chat would be.
        without_sandbox: None,
        held: None,
        grants: Default::default(),
        // The person's press on Try it, in the window.
        made: purlis_core::sandbox::Made::Window,
    };
    let ready = match purlis_core::start::ready(&start, &root) {
        Ok(ready) => ready,
        Err(why) => return Ok(failed(crate::asking::start_refused(held, why), None)),
    };
    let chat = Chat {
        program: ready.program.clone(),
        args: Vec::new(),
        cwd: ready.cwd.clone(),
        name,
        resume: ready.session.clone(),
        active: false,
        profile: Some(profile),
        persona: None,
        show_footer: false,
        pinned: false,
        number: Some(number),
        label: Some(format!("Try it on {pool}")),
        from: None,
        renamed_from: None,
        // On this pool, whatever the project's or its workspaces' own setting says (#4536).
        runs_on: Some(purlis_core::machine::StartChoice::Pool(pool.to_owned())),
        ..Default::default()
    };
    // Watched before it starts, so its first hook is never missed.
    let heard = held.hooks().skill_runs().watch(number);
    let mut chat_held = TryChat {
        held,
        number,
        session: None,
    };
    let session = match held.chats().start_ready(&chat, &ready, TRY_SIZE) {
        Ok(session) => session,
        Err(why) => {
            let step = step_of_a_refused_start(&why);
            return Ok(failed(crate::asking::start_refused(held, why), step));
        }
    };
    chat_held.session = Some(session);
    tracing::info!("purlis: Try it started chat {number} on pool {pool}");
    let ended = waited_for_its_start(&heard, pool, TRY_BEGINS_WITHIN, || {
        held.chats().sessions().running().contains(&session)
    })
    .err();
    drop(chat_held);
    if let Some(why) = ended {
        return Ok(failed(why, None));
    }
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    poolsetup::tried(config, pool, at).map_err(|why| {
        format!("A chat started on {pool}, and purlis couldn't keep that it did: {why}")
    })?;
    Ok(PoolSetupTried {
        failed: None,
        workspace: Some(workspace),
    })
}

/// **Opens the Docker app**, on the person's press, where this system has one to open. Refused
/// elsewhere, with what to do instead.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_open_docker() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::off_the_window("opening Docker", || {
            let opened = purlis_core::forklock::output(
                std::process::Command::new("/usr/bin/open").args(["-a", "Docker"]),
            )
            .map_err(|why| why.to_string())?;
            if opened.status.success() {
                Ok(())
            } else {
                Err("purlis couldn't open Docker. Open it the way you usually do.".to_owned())
            }
        })
        .await
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("purlis can't open Docker here. Start it the way you usually do.".to_owned())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use super::{said_as_time, step_of_a_refused_start, waited_for_its_start};
    use crate::skillrun::Watched;
    use purlis_core::poolsetup::StepId;

    const RUNNING: fn() -> bool = || true;

    #[test]
    fn try_it_passes_on_its_chats_start_hook_and_nothing_before_it() {
        let (tell, heard) = channel();
        tell.send(Watched::TurnEnded).unwrap();
        tell.send(Watched::Began).unwrap();
        assert_eq!(
            waited_for_its_start(&heard, "dev-box", Duration::from_secs(5), RUNNING),
            Ok(())
        );
    }

    #[test]
    fn a_chat_that_ends_before_it_starts_fails_try_it_at_once() {
        let (tell, heard) = channel();
        tell.send(Watched::Closed).unwrap();
        let why =
            waited_for_its_start(&heard, "dev-box", Duration::from_secs(60), RUNNING).unwrap_err();
        assert!(why.contains("ended before it had started"), "{why}");
    }

    #[test]
    fn a_chat_that_never_reports_its_start_fails_try_it_once_the_wait_is_over() {
        let (_tell, heard) = channel();
        let why = waited_for_its_start(&heard, "dev-box", Duration::from_millis(50), RUNNING)
            .unwrap_err();
        assert!(why.contains("didn't start within"), "{why}");
    }

    #[test]
    fn a_chat_whose_program_is_gone_fails_try_it_at_once_and_says_it_ended() {
        let (_tell, heard) = channel();
        let began = std::time::Instant::now();
        let why =
            waited_for_its_start(&heard, "dev-box", Duration::from_secs(60), || false).unwrap_err();
        assert!(why.contains("ended before it had started"), "{why}");
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "it waited out the minute"
        );
    }

    #[test]
    fn a_start_its_runner_found_signed_out_sends_the_person_to_sign_ins() {
        let refused = "work is not signed in on the runner of pool dev-box. Nothing was started.";
        assert_eq!(step_of_a_refused_start(refused), Some(StepId::Signins));
        assert_eq!(step_of_a_refused_start("the runner did not answer"), None);
    }

    #[test]
    fn a_wait_is_said_in_words_with_its_unit_counted() {
        assert_eq!(said_as_time(Duration::from_secs(120)), "2 minutes");
        assert_eq!(said_as_time(Duration::from_secs(60)), "1 minute");
        assert_eq!(said_as_time(Duration::from_secs(1)), "1 second");
    }
}
