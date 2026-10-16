// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The Saving view and the title bar's save indicator (charter-app#294, ADR 0051): where the
//! plane's unsaved work sits, the save journal, and the save button.
//!
//! Both are the core's answers, re-shaped for the window and nothing more. The stage is
//! [`planegit::shared_standing`] — the one standing auto-save and the alerts read too (FD-11) —
//! read from git and the push record and never from the network; the
//! save is [`planegit::save_as`] with [`Trigger::Manual`] — the same function `charter save`
//! runs, so the button and the command cannot disagree about what a save does.

use std::path::Path;

use purlis_core::forge::gitcredential;
use purlis_core::planegit::{self, Trigger};
use purlis_core::planesave;
use purlis_core::repocmd::Say;
use purlis_core::reposave;

use crate::planes::{PlaneId, Planes};

/// How many journal lines the view shows, newest first.
const JOURNAL_SHOWN: usize = 50;

/// One save attempt, as the journal holds it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveEntry {
    /// Seconds since the epoch.
    pub at: f64,
    /// `plane`, or `repo:<workspace>/<name>`.
    pub target: String,
    pub trigger: String,
    pub mode: String,
    pub files: u32,
    pub commit: Option<String>,
    pub pr: Option<String>,
    pub outcome: String,
    pub detail: String,
}

/// One check that failed on a save's pull request: its name as the forge shows it, and its
/// page there when it has one. Both are made safe to show by the core
/// (`purlis_core::forge::checks::FailedCheck`): the name is whoever wrote the check's.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FailedCheck {
    pub name: String,
    /// An `https://` address, or `null`.
    pub url: Option<String>,
}

/// Where a `pr-merge` save's pull request stands with merging, as the last look at the forge
/// found it (ADR 0051, amended 2026-10-13). Never a block: a red request stays open, and a
/// later save pushes onto it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveMerge {
    /// `waiting`: its checks are running, have not had time to start, or are expected and
    /// have not started, and purlis looks again by itself. `red`: a check failed, required or
    /// not, and it stays open and unmerged. `queued`: its checks passed and it is in its
    /// branch's merge queue or train. `cannot-merge`: a conflict, a required review or another
    /// rule of the forge's. `merged`: purlis merged it, and the project has not moved onto
    /// what merged yet.
    pub state: String,
    /// Why, as a sentence fragment for the person.
    pub why: String,
    /// The checks that failed when it is `red`, at most twenty; empty otherwise.
    pub checks: Vec<FailedCheck>,
    /// What ends a `waiting` that nothing purlis reads will end, or `null`. `person`: the
    /// wait has no bound (no check where the repository's other requests have them, checks
    /// purlis cannot read at this commit, a required check that has not reported), and the
    /// window offers the person's own merge. `sign-in`: purlis's look has no sign-in for the
    /// forge, and the window offers one.
    pub needs: Option<String>,
    /// The request as its forge names it, `#12` or `!12`; `null` when its page does not end
    /// in a number.
    pub number: Option<String>,
    /// Where it stands, as one line that is the same for the same standing
    /// (`Merging::shown`): what **Merge as it stands** hands back with the commit, so that
    /// the merge is made only of what its question showed.
    pub shown: String,
}

impl SaveMerge {
    /// `merge`, for the request whose page is `page`, on a forge that calls a request
    /// `request` (`pull request` or `merge request`).
    pub(crate) fn of(merge: planegit::Merging, page: Option<&str>, request: &str) -> SaveMerge {
        let number = page
            .and_then(|page| page.trim_end_matches('/').rsplit('/').next())
            .filter(|last| !last.is_empty() && last.len() <= 10)
            .filter(|last| last.bytes().all(|b| b.is_ascii_digit()))
            .map(|digits| {
                let sigil = if request == "merge request" { '!' } else { '#' };
                format!("{sigil}{digits}")
            });
        SaveMerge {
            number,
            shown: merge.shown(),
            needs: merge.needs.map(|needs| needs.word().to_owned()),
            state: merge.state.word().to_owned(),
            why: merge.why,
            checks: merge
                .checks
                .into_iter()
                .map(|check| FailedCheck {
                    name: check.name,
                    url: check.url,
                })
                .collect(),
        }
    }
}

/// Where the plane's unsaved work sits, how far a save goes, and what the last saves did.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaneSaving {
    /// `blocked`, `changed`, `committed`, `pr-open` or `saved`.
    pub stage: String,
    /// What the next save would commit.
    pub changed: Vec<String>,
    /// Commits the remote does not have; `null` when there is nothing to count against.
    pub ahead: Option<u32>,
    pub pr: Option<String>,
    /// What the forge the plane's origin is on calls a request: `pull request` or `merge
    /// request` — `pull request` when charter does not know the forge.
    pub request: String,
    pub blocked: Option<String>,
    /// The target branch: `[plane] branch`, or the one the plane has checked out.
    pub branch: String,
    /// Whether a save would push. When it would not, a commit is as far as a save goes.
    pub pushes: bool,
    /// Commits the last fetch found on the remote that were not pulled; `null` when there is
    /// nothing to count against.
    pub behind: Option<u32>,
    /// Why the last push did not land, when it failed rather than conflicted.
    pub push_failed: Option<String>,
    /// The LIVE workspaces, whose charter, memory and todos a save publishes (charter-app#301).
    pub live: Vec<String>,
    /// The files to settle, when conflicts are why the save is blocked.
    pub conflicts: Vec<String>,
    /// What a save cannot do here that is not a block (a request mode with no forge to open it on).
    pub notice: Option<String>,
    /// `[plane] mode`, or `null` when the plane names none.
    pub mode: Option<String>,
    /// Where the mode came from: `charter.toml`, `charter.local.toml`, `[memory] share`, or
    /// `default`.
    pub mode_from: String,
    /// The newest [`JOURNAL_SHOWN`] saves, newest first.
    pub journal: Vec<SaveEntry>,
    /// Each file the save is blocked in by what looks like a credential: its path, line and
    /// kind, and what was matched with the value hidden; `null` when that is not why.
    #[specta(optional)]
    pub secrets: Option<Vec<crate::blockedsave::SecretAt>>,
    /// What the window asks about the project's remote before automatic saves push it with the
    /// person's sign-in (#1848, #1852); `null` when nothing.
    #[specta(optional)]
    pub remote: Option<crate::remotequestion::RemoteQuestion>,
    /// Where the open pull request of a `pr-merge` project stands with merging; `null` when
    /// none is open or the mode does not merge it.
    #[specta(optional)]
    pub merge: Option<SaveMerge>,
    /// How the person settles the block in a terminal, when it is a save's replay onto the
    /// remote that conflicted (qw382): the git commands, as a sentence; `null` otherwise.
    #[specta(optional)]
    pub by_hand: Option<String>,
    /// What git is stopped part-way in, by kind: `merge`, `rebase`, `am`, `cherry-pick`,
    /// `revert`, `bisect`, or `conflicts` for unmerged files alone; `null` when it is not.
    #[specta(optional)]
    pub stopped_by: Option<String>,
}

/// The plane's save standing.
///
/// On a blocking thread: it asks git.
#[tauri::command]
#[specta::specta]
pub async fn plane_saving(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<PlaneSaving, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || saving_of(&root))
        .await
        .map_err(|err| format!("reading the project's save state did not finish: {err}"))
}

/// Save the plane, as the save button does: `message`, or the generated one when it is empty.
/// Answers every line the save said, or its refusal.
///
/// On a blocking thread: it commits, and may push.
#[tauri::command]
#[specta::specta]
pub async fn save_plane(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    heard: tauri::State<'_, crate::heard::Heard>,
    plane: PlaneId,
    message: Option<String>,
) -> Result<Vec<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let at = root.clone();
    let saved = tauri::async_runtime::spawn_blocking(move || save(&at, message.as_deref()))
        .await
        .map_err(|err| format!("the save did not finish: {err}"))?;
    // Told once it is saved, on a thread of its own (charter-app#343).
    if saved.is_ok() {
        heard.tell(
            &app,
            plane,
            root,
            purlis_core::extension::events::Event::PlaneSaved,
        );
    }
    saved
}

/// Answer the question a plane with no mode is asked once (ADR 0051): how far its saves go.
/// Written as `[plane] mode` into `charter.toml`, through the core's own writer, and answered
/// with the plane's save standing as it now is.
#[tauri::command]
#[specta::specta]
pub async fn choose_plane_mode(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    mode: String,
) -> Result<PlaneSaving, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || choose_mode(&root, &mode))
        .await
        .map_err(|err| format!("choosing the mode did not finish: {err}"))?
}

/// [`choose_plane_mode`], without a runtime.
pub fn choose_mode(root: &Path, mode: &str) -> Result<PlaneSaving, String> {
    use purlis_core::settings::{self, Edit, Step, Value, Which};
    if planesave::Mode::parse(mode).is_none() {
        return Err(format!(
            "{mode} is not a mode — one of off, commit, push, pr, pr-merge"
        ));
    }
    let read = settings::read(root, Which::Shared)?;
    let text = settings::edited(
        &read.text,
        &[Edit {
            path: vec![Step::Key("plane".into()), Step::Key("mode".into())],
            value: Some(Value::Text(mode.to_owned())),
        }],
    )?;
    settings::save(
        root,
        Which::Shared,
        read.exists.then_some(read.text.as_str()),
        &text,
    )
    .map_err(|reasons| reasons.join("\n"))?;
    // The mode changes what the shared standing says (FD-11): it is read again, not answered
    // from before the choice.
    planegit::touch(root);
    Ok(saving_of(root))
}

/// [`plane_saving`], without a runtime.
pub fn saving_of(root: &Path) -> PlaneSaving {
    let standing = planegit::shared_standing(root);
    let plane = planesave::Settings::read(root).plane;
    let mode_from = if plane.from_share {
        "[memory] share".to_owned()
    } else {
        plane.mode.source.file().unwrap_or("default").to_owned()
    };
    let mut journal: Vec<SaveEntry> = planegit::journal(root)
        .iter()
        .rev()
        .take(JOURNAL_SHOWN)
        .map(entry_of)
        .collect();
    journal.shrink_to_fit();
    let remote = standing
        .pushes
        .then(|| crate::remotequestion::of(root, root))
        .flatten();
    let request = purlis_core::forge::request_words_of(root, root)
        .request_noun()
        .to_owned();
    PlaneSaving {
        remote,
        by_hand: standing
            .rebase_by_hand
            .then(|| planegit::BY_HAND.to_owned()),
        stopped_by: standing.stopped_by.map(str::to_owned),
        merge: standing
            .merge
            .clone()
            .map(|merge| SaveMerge::of(merge, standing.pr.as_deref(), &request)),
        secrets: crate::blockedsave::shown(&standing.secrets),
        // Said with the file and line for a secret-shaped value, so the title bar names it.
        blocked: standing.blocked_said(),
        stage: standing.stage.word().to_owned(),
        changed: standing.changed,
        ahead: standing.ahead,
        pr: standing.pr,
        request,
        branch: standing.branch,
        pushes: standing.pushes,
        behind: standing.behind,
        push_failed: standing.push_failed,
        live: purlis_core::wscmd::live_workspaces(root)
            .into_iter()
            .collect(),
        conflicts: standing.conflicts,
        notice: standing.notice,
        mode: plane.mode.value.map(|m| m.as_str().to_owned()),
        mode_from,
        journal,
    }
}

/// [`save_plane`], without a runtime.
pub fn save(root: &Path, message: Option<&str>) -> Result<Vec<String>, String> {
    save_as(root, message, Trigger::Manual)
}

/// A save of the plane at `root`, started by `trigger` — the button, or auto-save
/// (`autosave.rs`). One at a time per plane, whoever started it.
pub fn save_as(
    root: &Path,
    message: Option<&str>,
    trigger: Trigger,
) -> Result<Vec<String>, String> {
    let message = message.map(str::trim).filter(|m| !m.is_empty());
    let mut said: Vec<Say> = Vec::new();
    let mut say = |line: Say| said.push(line);
    // The person's own Save confirms the remote it pushes to; an automatic one is held to it.
    let confirmed = trigger == Trigger::Manual;
    let code = crate::forgeaccounts::window_git(|| {
        gitcredential::confirmed_if(confirmed, || {
            planegit::save_as(
                &planegit::Request {
                    root,
                    message,
                    sign: false,
                    no_push: false,
                    cwd: root,
                    provenance: None,
                },
                trigger,
                &mut say,
            )
        })
    });
    crate::workspaces::ran(code, said)
}

/// How the window's Update from remote ended (qw348), as the Saving view says it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUpdate {
    /// `updated`: the project is at the remote's commits. `nothing`: nothing was incoming.
    /// `left`: the project was left as it was, and `reason` and `said` say why.
    pub outcome: String,
    /// Why it was left, as one word; `null` otherwise. `unsaved` is the one the view offers
    /// Save, then update for.
    pub reason: Option<String>,
    /// How it ended, as sentences for the person.
    pub said: String,
    /// The files the reason is about: where it conflicts, or what could not be written.
    pub files: Vec<String>,
    /// Whether the project is exactly as it was, or at the new commit: `false` only where an
    /// update stopped part-way and purlis could not put every file back, which `said` says.
    pub whole: bool,
    /// What the update said it was doing, in order.
    pub lines: Vec<String>,
}

/// **Update from remote**: bring the project's incoming commits in, as the person's own press
/// in the window (qw348). purlis runs it itself, under no chat's sandbox: a fetch of the
/// project's target branch from the remote recorded for it, held as every fetch is, then a
/// fast-forward, or a replay of the project's own commits on top. A tree with unsaved work is
/// never touched. Answers how it ended; the project is then at the new commit or exactly as
/// it was.
///
/// The window's alone (`WINDOW_ONLY`): it moves the project's working tree under every chat in
/// it, so no link and no chat may press it.
///
/// On a blocking thread: it fetches, and checks a tree out.
#[tauri::command]
#[specta::specta]
pub async fn update_project(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<ProjectUpdate, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let done = tauri::async_runtime::spawn_blocking(move || update(&root))
        .await
        .map_err(|err| format!("the update did not finish: {err}"))?;
    // The tree moved under every reader, where the watch does not look too (#933).
    if done.outcome == "updated" || !done.whole {
        held.tree_moved();
    }
    Ok(done)
}

/// [`update_project`], without a runtime.
pub fn update(root: &Path) -> ProjectUpdate {
    let mut lines: Vec<String> = Vec::new();
    let mut say = |line: Say| lines.push(line.to_string());
    // The window's own git (#752): its sign-in, offered to the recorded remote alone.
    let updated = crate::forgeaccounts::window_git(|| planegit::update(root, &mut say));
    let request = purlis_core::forge::request_words_of(root, root).request_noun();
    let said = updated.said(request);
    let (outcome, reason, files, whole) = match &updated {
        planegit::Updated::Moved(_) => ("updated", None, Vec::new(), true),
        planegit::Updated::Nothing(_) => ("nothing", None, Vec::new(), true),
        planegit::Updated::Left(left) => (
            "left",
            Some(left.word().to_owned()),
            left.files().to_vec(),
            left.as_it_was(),
        ),
    };
    ProjectUpdate {
        outcome: outcome.to_owned(),
        reason,
        said,
        files,
        whole,
        lines,
    }
}

/// How a press of Resolve, Continue or Abort on a save's conflict ended (qw382).
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettled {
    /// `saved`: the replay finished and what is committed was pushed, with no new commit
    /// (`said` says how the push went, and names files changed meanwhile, left unsaved).
    /// `stopped`: git stopped on a conflict in `files`, and the rebase is left stopped.
    /// `empty`: the commit being replayed has nothing left to commit, and Skip drops it.
    /// `aborted`: the project is back where it was. `refused`: nothing changed, and `said`
    /// says why.
    pub outcome: String,
    /// How it ended, as sentences for the person.
    pub said: String,
    /// The files git stopped in, for `stopped`.
    pub files: Vec<String>,
    /// What it said it was doing, in order, the save's lines included.
    pub lines: Vec<String>,
}

/// **Resolve a save's conflict from the window** (qw382): `step` is `start` (fetch and replay
/// the project's own commits, stopping where git stops), `continue` (carry on once the files in
/// conflict are settled; `files` are the ones the window showed), `skip` (drop a commit the
/// settling left empty), or `abort`. Where the replay finishes, what is committed is pushed,
/// and nothing new is committed.
///
/// The window's alone (`WINDOW_ONLY`): it moves the project's working tree under every chat in
/// it, and a chat's sandbox may never run it.
///
/// On a blocking thread: it fetches, and rewrites the tree.
#[tauri::command]
#[specta::specta]
pub async fn settle_save_conflict(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    step: String,
    files: Vec<String>,
) -> Result<SaveSettled, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let done = tauri::async_runtime::spawn_blocking(move || settle(&root, &step, &files))
        .await
        .map_err(|err| format!("it did not finish: {err}"))??;
    if done.outcome != "refused" {
        held.tree_moved();
    }
    Ok(done)
}

/// [`settle_save_conflict`], without a runtime.
pub fn settle(root: &Path, step: &str, files: &[String]) -> Result<SaveSettled, String> {
    use planegit::resolve::{self, Settled};
    let mut lines: Vec<String> = Vec::new();
    let settled = {
        let mut say = |line: Say| lines.push(line.to_string());
        match step {
            // The window's own git (#752): its sign-in, offered to the recorded remote alone.
            "start" => crate::forgeaccounts::window_git(|| resolve::start(root, &mut say)),
            "continue" => resolve::carry_on(root, files, &mut say),
            "skip" => resolve::skip(root),
            "abort" => resolve::abort(root),
            other => {
                return Err(format!(
                    "{other} is not a step: start, continue, skip or abort"
                ));
            }
        }
    };
    planegit::touch(root);
    let (outcome, said, files) = match settled {
        Settled::Replayed => ("saved", pushed_after(root, &mut lines), Vec::new()),
        Settled::Stopped(files) => (
            "stopped",
            "git stopped on a conflict. Settle the files below, in a chat or by hand, then \
             Continue. Abort puts the project back where it was."
                .to_owned(),
            files,
        ),
        Settled::Empty => (
            "empty",
            "Settled, this commit of the project's own changes nothing the remote does not \
             already have. Skip drops it and carries the replay on; Abort puts the project \
             back where it was."
                .to_owned(),
            Vec::new(),
        ),
        Settled::Aborted => (
            "aborted",
            "Aborted: the project is back where it was before Resolve.".to_owned(),
            Vec::new(),
        ),
        Settled::Refused(why) => ("refused", why, Vec::new()),
    };
    Ok(SaveSettled {
        outcome: outcome.to_owned(),
        said,
        files,
        lines,
    })
}

/// After a replay that finished: the push of what is committed, as the person's own press, and
/// no new commit. Files changed meanwhile are named and left for the person.
fn pushed_after(root: &Path, lines: &mut Vec<String>) -> String {
    let (pushed, left) = {
        let mut say = |line: Say| lines.push(line.to_string());
        crate::forgeaccounts::window_git(|| {
            gitcredential::confirmed_if(true, || planegit::resolve::push_after(root, &mut say))
        })
    };
    let first = match pushed {
        Some(res) if res.outcome == planegit::Outcome::Pushed => {
            "Resolved: the project's own commits are on the remote's, and they are pushed."
                .to_owned()
        }
        Some(res) => format!(
            "Resolved: the project's own commits are on the remote's. The push after it did \
             not land ({}){}",
            res.outcome.word(),
            if res.detail.is_empty() {
                ".".to_owned()
            } else {
                format!(": {}", res.detail)
            }
        ),
        None => "Resolved: the project's own commits are on the remote's, and nothing was \
                 pushed: this project's saves stop at the commit, or a save was running."
            .to_owned(),
    };
    if left.is_empty() {
        return first;
    }
    format!(
        "{first} These files changed while it was resolved and were not saved: {}. Save them, \
         or remove them.",
        left.iter()
            .map(|path| format!("`{path}`"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// **Merge a save's pull request as it stands**, its failed and missing checks included: the
/// project's own request, or the one of `repo` in `workspace` when both are named (the
/// operator's ruling P1, 2026-10-13; ADR 0051).
///
/// The person's own, explicit press in the window, and the only thing that merges past a
/// check: a save and purlis's own look never do. Only the checks are passed over. A conflict,
/// a review the branch requires and every rule the forge itself enforces still refuse it, and
/// it merges only at the commit purlis pushed. Answers what happened, as a sentence.
///
/// The window's alone (`WINDOW_ONLY`): no link and no chat may press it.
///
/// On a blocking thread: it asks the forge, and may fetch and move the project's tree.
#[tauri::command]
#[specta::specta]
pub async fn merge_save_request(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    repo: Option<String>,
    head: String,
    shown: String,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let (said, moved) = tauri::async_runtime::spawn_blocking(move || {
        let seen = planegit::Seen { head, shown };
        merge_as_it_stands(&root, workspace.as_deref(), repo.as_deref(), &seen)
    })
    .await
    .map_err(|err| format!("the merge did not finish: {err}"))??;
    // The project moved onto what merged, under every reader (#933).
    if moved {
        held.tree_moved();
    }
    Ok(said)
}

/// What a fresh look at a save's request found, for the question **Merge as it stands…**
/// asks: the commit it is at and where it stands now. The press hands `head` and
/// `merge.shown` back, and the merge is made only of them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveLook {
    /// The commit the request is at; `null` when no request is open any more.
    pub head: Option<String>,
    /// Where it stands now; `null` when no request is open any more (it merged, or closed).
    pub merge: Option<SaveMerge>,
}

/// **Look at a save's request now**, before the person is asked whether to merge it as it
/// stands: every check is read afresh, so the question shows what is there and not what the
/// last look kept. As every look does, it merges a request whose checks all passed.
///
/// The window's alone (`WINDOW_ONLY`): it asks the forge with the person's sign-in, and may
/// merge and move the project's tree.
///
/// On a blocking thread: it asks the forge.
#[tauri::command]
#[specta::specta]
pub async fn look_at_save_request(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    repo: Option<String>,
) -> Result<SaveLook, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let (look, moved) = tauri::async_runtime::spawn_blocking(move || {
        look_now(&root, workspace.as_deref(), repo.as_deref())
    })
    .await
    .map_err(|err| format!("the look did not finish: {err}"))??;
    if moved {
        held.tree_moved();
    }
    Ok(look)
}

/// [`look_at_save_request`], without a runtime: what it found, and whether the project's tree
/// moved.
pub fn look_now(
    root: &Path,
    workspace: Option<&str>,
    repo: Option<&str>,
) -> Result<(SaveLook, bool), String> {
    use purlis_core::forge::Caller;
    match (workspace, repo) {
        (None, None) => {
            let looked = crate::forgeaccounts::window_git(|| {
                planegit::look(root, &Caller::window(), &mut |_: Say| {})
            })?;
            planegit::touch(root);
            let saving = saving_of(root);
            Ok((
                SaveLook {
                    head: planegit::seen(root).map(|seen| seen.head),
                    merge: saving.merge,
                },
                looked.moved,
            ))
        }
        (Some(workspace), Some(name)) => {
            let found =
                purlis_core::repos::clones(root, workspace).map_err(|why| why.to_string())?;
            let repo = found
                .repos
                .iter()
                .find(|repo| repo.name == name)
                .ok_or_else(|| format!("{workspace} holds no clone called {name}"))?;
            crate::forgeaccounts::window_git(|| {
                reposave::look(root, workspace, repo, &Caller::window())
            })?;
            let row = repos_saving(root, workspace)?
                .into_iter()
                .find(|row| row.name == name);
            Ok((
                SaveLook {
                    head: reposave::seen(root, workspace, repo).map(|seen| seen.head),
                    merge: row.and_then(|row| row.merge),
                },
                false,
            ))
        }
        _ => Err("name both the workspace and the repo, or neither for the project's own".into()),
    }
}

/// [`merge_save_request`], without a runtime: what happened, and whether the project's tree
/// moved.
pub fn merge_as_it_stands(
    root: &Path,
    workspace: Option<&str>,
    repo: Option<&str>,
    seen: &planegit::Seen,
) -> Result<(String, bool), String> {
    use purlis_core::forge::Caller;
    use purlis_core::planegit::MergedAsItStands;
    let merged = match (workspace, repo) {
        (None, None) => {
            let mut lines: Vec<String> = Vec::new();
            // The window's own git and sign-in (#752): the fetch after the merge is held to
            // the remote recorded for the project.
            let merged = crate::forgeaccounts::window_git(|| {
                planegit::merge_as_seen(root, &Caller::window(), seen, &mut |line: Say| {
                    lines.push(line.to_string());
                })
            })?;
            // What it said is the save journal's and the push record's; the answer is enough.
            drop(lines);
            merged
        }
        (Some(workspace), Some(name)) => {
            let found =
                purlis_core::repos::clones(root, workspace).map_err(|why| why.to_string())?;
            let repo = found
                .repos
                .iter()
                .find(|repo| repo.name == name)
                .ok_or_else(|| format!("{workspace} holds no clone called {name}"))?;
            crate::forgeaccounts::window_git(|| {
                reposave::merge_as_seen(root, workspace, repo, &Caller::window(), seen)
            })?
        }
        _ => {
            return Err(
                "name both the workspace and the repo, or neither for the project's own".into(),
            );
        }
    };
    Ok(match merged {
        MergedAsItStands::Moved => (
            "Merged as it stands, and the project is on what merged.".to_owned(),
            true,
        ),
        MergedAsItStands::Later(said) => (said, false),
    })
}

/// One workspace repo's save standing, for its row in the Saving view (charter-app#299).
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoSaving {
    pub name: String,
    /// `[repos.<name>] mode`: `off` when neither file says.
    pub mode: String,
    /// The file that decided the mode, or `default`.
    pub mode_from: String,
    /// Whether it is saved by itself — off unless its table turns it on.
    pub autosave: bool,
    /// `off` for a repo charter never saves; else `blocked`, `changed`, `committed`,
    /// `pr-open` or `saved`.
    pub stage: String,
    /// The branch the clone is on; `null` on none.
    pub branch: Option<String>,
    /// Files a save would take.
    pub changed: u32,
    /// Commits the remote's copy of the branch lacks; `null` for a branch never pushed.
    pub ahead: Option<u32>,
    pub pr: Option<String>,
    /// What the forge its origin is on calls a request: `pull request` or `merge request` —
    /// `pull request` when charter does not know the forge.
    pub request: String,
    pub blocked: Option<String>,
    /// Whether a save would push.
    pub pushes: bool,
    /// Where a request mode's pull request goes: `[repos.<name>] branch`, else the repo's default
    /// branch; `null` in the other modes, or when charter cannot tell — so the row and the Save
    /// all confirmation can say where a save goes before anyone presses it.
    pub target: Option<String>,
    /// Whether a request mode's save would commit on a branch of charter's own,
    /// `charter/<workspace>/…`, because the clone stands on its pull request's base or on the
    /// repo's default branch — a request mode never pushes either (`reposave`).
    pub own_branch: bool,
    /// What the window asks about the clone's remote before automatic saves push it with the
    /// person's sign-in (#1848, #1852); `null` when nothing.
    #[specta(optional)]
    pub remote: Option<crate::remotequestion::RemoteQuestion>,
    /// Where the repo's open pull request stands with merging, in `pr-merge`; `null`
    /// otherwise.
    #[specta(optional)]
    pub merge: Option<SaveMerge>,
}

/// Every clone in `workspace`, as the Saving view draws them. Read from git and the journal,
/// never the network.
///
/// On a blocking thread: it asks git once or twice per clone.
#[tauri::command]
#[specta::specta]
pub async fn workspace_saving(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
) -> Result<Vec<RepoSaving>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || repos_saving(&root, &workspace))
        .await
        .map_err(|err| format!("reading the repos' save state did not finish: {err}"))?
}

/// Save one repo of `workspace`, as its row's button does. Refused, with whom it waits for,
/// while any chat in the workspace is mid-turn: a save the operator asked for does not queue
/// itself to run later, unwatched.
///
/// On a blocking thread: it commits, and may push and open a pull request.
#[tauri::command]
#[specta::specta]
pub async fn save_repo(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    name: String,
    message: Option<String>,
) -> Result<Vec<String>, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        // Asked on this thread, twice — before the save and right before `git add` — so a
        // turn that starts while the save waits is still seen.
        let mid_turn = || held.mid_turn_in(&workspace);
        save_repo_in(&root, &workspace, &name, message.as_deref(), &mid_turn)
    })
    .await
    .map_err(|err| format!("the save did not finish: {err}"))?
}

/// [`workspace_saving`], without a runtime.
pub fn repos_saving(root: &Path, workspace: &str) -> Result<Vec<RepoSaving>, String> {
    let found = purlis_core::repos::clones(root, workspace).map_err(|why| why.to_string())?;
    let settings = planesave::Settings::read(root);
    Ok(found
        .repos
        .iter()
        .map(|repo| {
            let standing = reposave::shared_standing(root, workspace, repo);
            let opens_a_pr = matches!(
                standing.mode,
                planesave::Mode::Pr | planesave::Mode::PrMerge
            );
            let default = opens_a_pr
                .then(|| reposave::default_branch(root, &repo.name, &repo.path))
                .flatten();
            let target = opens_a_pr
                .then(|| settings.repo(&repo.name).branch.value.or(default.clone()))
                .flatten();
            let own_branch = opens_a_pr
                && standing
                    .branch
                    .as_ref()
                    .is_some_and(|on| Some(on) == target.as_ref() || Some(on) == default.as_ref());
            let remote = (standing.mode != planesave::Mode::Off && standing.pushes)
                .then(|| crate::remotequestion::of(root, &repo.path))
                .flatten();
            let request = purlis_core::forge::request_words_of(root, &repo.path)
                .request_noun()
                .to_owned();
            RepoSaving {
                remote,
                merge: standing
                    .merge
                    .clone()
                    .map(|merge| SaveMerge::of(merge, standing.pr.as_deref(), &request)),
                target,
                own_branch,
                stage: if standing.mode == planesave::Mode::Off {
                    "off".to_owned()
                } else {
                    standing.stage.word().to_owned()
                },
                name: standing.name,
                mode: standing.mode.as_str().to_owned(),
                mode_from: standing.mode_from.to_owned(),
                autosave: standing.autosave,
                branch: standing.branch,
                changed: standing.changed,
                ahead: standing.ahead,
                pr: standing.pr,
                request,
                blocked: standing.blocked,
                pushes: standing.pushes,
            }
        })
        .collect())
}

/// [`save_repo`], without a runtime.
pub fn save_repo_in(
    root: &Path,
    workspace: &str,
    name: &str,
    message: Option<&str>,
    mid_turn: &dyn Fn() -> Vec<String>,
) -> Result<Vec<String>, String> {
    let found = purlis_core::repos::clones(root, workspace).map_err(|why| why.to_string())?;
    let repo = found
        .repos
        .iter()
        .find(|repo| repo.name == name)
        .ok_or_else(|| format!("{workspace} holds no clone called {name}"))?;
    let message = message.map(str::trim).filter(|m| !m.is_empty());
    let mut said: Vec<Say> = Vec::new();
    // The person's own Save: it confirms the remote it pushes to.
    let code = crate::forgeaccounts::window_git(|| {
        gitcredential::confirmed_if(true, || {
            reposave::save_as(
                &reposave::Request {
                    plane: root,
                    workspace,
                    name,
                    clone: &repo.path,
                    message,
                    no_push: false,
                    mid_turn,
                },
                Trigger::Manual,
                &mut |line| said.push(line),
            )
        })
    });
    crate::workspaces::ran(code, said)
}

/// One journal line, with what it lacks read as empty rather than refused: the journal is
/// charter's own note, and a line an older charter wrote is still worth showing.
fn entry_of(line: &serde_json::Value) -> SaveEntry {
    let text = |key: &str| line.get(key).and_then(serde_json::Value::as_str);
    SaveEntry {
        at: line
            .get("at")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0),
        target: text("target").unwrap_or("plane").to_owned(),
        trigger: text("trigger").unwrap_or("").to_owned(),
        mode: text("mode").unwrap_or("").to_owned(),
        files: line
            .get("files")
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0),
        commit: text("commit").map(str::to_owned),
        pr: text("pr").map(str::to_owned),
        outcome: text("outcome").unwrap_or("").to_owned(),
        detail: text("detail").unwrap_or("").to_owned(),
    }
}

#[cfg(test)]
mod tests;
