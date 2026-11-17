// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Draft skills in the Skill studio** (SK-9, #2772; spec #2762 decisions 13, 38, 50 and 52): the
//! window's half of `purlis_core::skills::verbs::drafts`.
//!
//! - **A chat's write to a skill's files is kept as a draft**, never written: it reaches the app
//!   on the proposed change's road (`Ask::Propose`), and [`drafted`] keeps it instead of holding
//!   it as a proposal. It is never an ask: the Inbox does not list it, and the chat's tab only
//!   says the draft is ready in the Skill studio.
//! - **The studio lists each draft with its diff** ([`skill_drafts`]), and offers **Try it**
//!   ([`skill_draft_try`]), **Accept** ([`skill_draft_accept`], one commit) and **Reject**
//!   ([`skill_draft_reject`]).
//! - **"Ask a chat to draft a skill"** ([`skill_draft_chat`]) opens a chat where the level's skills
//!   are reached from, with the core's prompt typed and never sent, as a curation chat is opened.
//!   **"Ask a chat to write them"** ([`skill_trigger_tests_chat`]) opens one the same way, asked
//!   for a skill's trigger tests (#3925); [`skill_outcome_evals_chat`] for its outcome evals, and
//!   [`skill_improve_chat`] for an edit from its recent runs and grades (#2780);
//!   [`skill_checks_chat`] for both kinds of a skill's checks, in one draft (ruling of 2026-11-02).
//! - **Try it** opens a scratch chat in a throwaway piece, armed with the built-in skills and the
//!   draft alone, on the profile the person picked or the project's default; the piece goes when
//!   the chat closes (`Chats::close`).
//!

use std::sync::Arc;

use serde::Serialize;
use specta::Type;

use purlis_core::hookwire::{Answer, ProposeAsk};
use purlis_core::reopen::{StudioJob, StudioWork};
use purlis_core::skills::verbs::drafts;

use crate::curation::{ChatTyped, Curating, StartRefused};
use crate::planes::{Held, PlaneId, Planes};
use crate::skillchange::{SkillChanged, SkillLevel};

/// **Keeps chat `ask.chat`'s change as a draft skill** where it is to a plane skill's file: the
/// answer, or `None` for a proposal that is not one, which is
/// then a proposed change as before. Nothing in the project is written.
pub fn drafted(held: &Held, ask: &ProposeAsk) -> Option<Answer> {
    let root = held.root();
    let file = drafts::draftable(root, &root.join(&ask.path))?;
    // Policy's `sandbox.proposed-changes = "off"` holds no draft either (PC-14).
    if let Some(why) = purlis_core::sandbox::policy::Locks::of(root).proposed_changes_refused() {
        return Some(Answer::No { why });
    }
    if ask.change.carried() > purlis_core::proposed::MOST_BYTES {
        return Some(Answer::No {
            why: purlis_core::proposed::UNPREPARED.to_owned(),
        });
    }
    Some(match drafts::take(root, ask.chat, &file, &ask.change) {
        Ok(_) => {
            // A studio chat's job is done once its draft came, for good (#5185).
            held.chats().studio_job_done(ask.chat);
            // The studio and the chat's tab read the drafts again when the asks move.
            held.chats().asks_may_have_moved();
            Answer::Said {
                lines: vec![purlis_core::repocmd::Say::Done(drafts::DRAFTED_SAID.into())],
                code: 0,
            }
        }
        Err(why) => Answer::No { why },
    })
}

/// **What the Skill studio started a chat for** (#5185), as the window reads it off the chat:
/// its header's chip, the skill page's running line and its Runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioLink {
    /// The skill, by its name: `None` for a new skill's draft.
    pub skill: Option<String>,
    pub level: SkillLevel,
    #[specta(type = StudioJobKind)]
    pub job: StudioWork,
    /// Whether the chat wrote its draft: once true, never running again.
    pub done: bool,
    /// When the studio started the chat (RFC 3339), where its record knows (#5248): what the
    /// Grades card's running job counts from.
    pub started: Option<String>,
    /// When its job stopped running (#5248): its draft came, its program ended, or it closed.
    pub finished: Option<String>,
}

/// **The studio's jobs, as the window types them**: the core's [`StudioWork`], which writes
/// itself as its word, typed for the window here, since purlis-core keeps no UI typing crate.
/// Never made: a test holds its words to the core's.
#[derive(Serialize, Type)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(not(test), allow(dead_code))]
enum StudioJobKind {
    Draft,
    TriggerTests,
    OutcomeEvals,
    Checks,
    Improve,
    Redo,
}

impl From<&StudioJob> for StudioLink {
    fn from(job: &StudioJob) -> StudioLink {
        StudioLink {
            skill: job.skill.clone(),
            level: (&job.level).into(),
            job: job.job,
            done: job.done,
            started: job.started.clone(),
            finished: job.finished.clone(),
        }
    }
}

/// **A chat the studio started, closed here** (#5248), as a skill's Runs lists it: its number
/// (what a draft names its chat by), its name as it closed, when, and what it was started for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ClosedStudioChat {
    pub session: u32,
    pub name: String,
    /// When it closed (RFC 3339).
    pub closed: String,
    /// What it was started for, its `finished` always set.
    pub link: StudioLink,
}

impl From<&purlis_core::reopen::studio_chats::ClosedStudioChat> for ClosedStudioChat {
    fn from(one: &purlis_core::reopen::studio_chats::ClosedStudioChat) -> ClosedStudioChat {
        ClosedStudioChat {
            session: one.chat,
            name: one.name.clone(),
            closed: one.closed.clone(),
            link: StudioLink::from(&one.job),
        }
    }
}

/// **The chats the studio started that have closed here** (#5248), newest first: what a skill's
/// Runs keeps after a chat ends.
#[tauri::command]
#[specta::specta]
pub(crate) async fn studio_chats_closed(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<ClosedStudioChat>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the studio's chats", move || {
        Ok(purlis_core::reopen::studio_chats::closed(held.root())
            .iter()
            .map(ClosedStudioChat::from)
            .collect())
    })
    .await
}

/// One draft skill, as the studio lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillDraft {
    /// What Try it, Accept and Reject are handed back.
    pub id: String,
    pub name: String,
    pub level: SkillLevel,
    /// The chat that wrote it last, by its number.
    pub chat: Option<u32>,
    /// Whether that chat is open now, for an out-of-date draft (false for any other): "Ask the
    /// chat to redo it" types into it ([`skill_draft_redo_in`]), where a closed one's opens a
    /// chat afresh ([`skill_draft_redo`]).
    pub chat_open: bool,
    /// A new skill, rather than a change to one.
    pub new: bool,
    /// A file it changes changed in the project since: Accept is refused.
    pub stale: bool,
    pub files: Vec<SkillDraftFile>,
    /// **The extension's template New skill… made it from** (#3091), as it was offered; `null`
    /// for a chat's draft.
    #[specta(optional)]
    pub template: Option<String>,
}

/// One file of a draft: the project's text and the draft's, drawn as a diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillDraftFile {
    /// Below the skill's folder: `SKILL.md`, `scripts/run.sh`.
    pub path: String,
    pub before: Option<String>,
    pub after: String,
    /// The lines that changed, as git's one line diff marks them in the merge view.
    pub hunks: Vec<crate::piecefiles::GitHunk>,
    pub stale: bool,
}

impl From<drafts::Draft> for SkillDraft {
    fn from(draft: drafts::Draft) -> SkillDraft {
        SkillDraft {
            stale: draft.stale(),
            level: (&draft.level).into(),
            id: draft.id,
            name: draft.name,
            chat: draft.chat,
            chat_open: false,
            new: draft.new,
            // An extension's own words: every invisible character escaped, as a source's are.
            template: draft
                .template
                .as_deref()
                .map(purlis_core::skills::review::escaped_line),
            files: draft
                .files
                .into_iter()
                .map(|file| SkillDraftFile {
                    hunks: crate::proposals::lines_changed(
                        file.before.as_deref().unwrap_or(""),
                        &file.after,
                    ),
                    path: file.path,
                    before: file.before,
                    after: file.after,
                    stale: file.stale,
                })
                .collect(),
        }
    }
}

/// **Every draft skill of the project**, by name: what the studio lists, and what a chat's tab
/// asks to say "draft ready in Skill studio".
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_drafts(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<SkillDraft>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the draft skills", move || {
        let listed = drafts::list(held.root());
        // Whether its chat is open says only what an out-of-date draft's redo does: the chats
        // are looked at only where one is.
        let open = if listed.iter().any(drafts::Draft::stale) {
            held.chats().open_now()
        } else {
            Vec::new()
        };
        Ok(listed
            .into_iter()
            .map(|draft| {
                let chat_open = draft.stale() && drafting_chat(&open, &draft).is_some();
                SkillDraft {
                    chat_open,
                    ..SkillDraft::from(draft)
                }
            })
            .collect())
    })
    .await
}

/// **The chat that drafted `draft`, where it is still open** among `open`: the chat of its number,
/// in the draft's own workspace for a workspace's draft (a project's or a persona's is written
/// from anywhere), so a number a later chat took in another workspace is never it.
fn drafting_chat<'a>(
    open: &'a [crate::chats::Open],
    draft: &drafts::Draft,
) -> Option<&'a crate::chats::Open> {
    let chat = draft.chat?;
    let workspace = draft.level.at.workspace();
    open.iter().find(|one| {
        one.session == chat && (workspace.is_none() || one.workspace.as_deref() == workspace)
    })
}

/// **Accepts a draft**: one commit of exactly its files.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_draft_accept(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: String,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("accepting the draft", move || {
        let changed = crate::skillchange::heard(held.root(), |cx| drafts::accept(cx, &id))?;
        crate::skillgrade::drafted::stop(&plane, &id);
        held.chats().asks_may_have_moved();
        Ok(changed)
    })
    .await
}

/// **Accepts a workspace or persona folder a chat could have made** (SK-9): its skills reach
/// chats from now on. The person's press in the studio, on a skill left out for it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_level_accept(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    folder: String,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("accepting the folder", move || {
        level_accepted(held.root(), &held.chats().skills_now(), &folder)?;
        Ok(SkillChanged::default())
    })
    .await
}

/// **Accepts the workspace and persona folders `folders` the studio listed** (#3718), each there
/// now: the person's one press where purlis cannot confirm which of them they accepted before
/// (`levels::to_confirm`).
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_levels_confirm(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    folders: Vec<String>,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("accepting the project's folders", move || {
        levels_confirmed(held.root(), &held.chats().skills_now(), &folders)?;
        Ok(SkillChanged::default())
    })
    .await
}

/// [`purlis_core::skills::levels::accept`], and then each open chat's kept skill stamp is read
/// again (#4813 review): which folders were accepted is part of what a chat would start on now,
/// and it is kept in the app state, which the plane's watch never tells of. Forgotten whether or
/// not the accept wrote all it meant to.
fn level_accepted(
    root: &std::path::Path,
    skills_now: &crate::chats::SkillsNow,
    folder: &str,
) -> Result<(), String> {
    let accepted = purlis_core::skills::levels::accept(root, folder);
    skills_now.forget();
    accepted
}

/// [`purlis_core::skills::levels::confirm`], and then each open chat's kept skill stamp is read
/// again, as [`level_accepted`].
fn levels_confirmed(
    root: &std::path::Path,
    skills_now: &crate::chats::SkillsNow,
    folders: &[String],
) -> Result<(), String> {
    let confirmed = purlis_core::skills::levels::confirm(root, folders);
    skills_now.forget();
    confirmed
}

/// [`purlis_core::skills::levels::vouch_if_made`], after the person's own create of the workspace
/// or persona folder `owner`, and then each open chat's kept skill stamp is read again (#4884):
/// the folders vouched for are part of what a chat would start on now, kept in the app state,
/// which the plane's watch never tells of. Forgotten whether or not it vouched, as
/// [`level_accepted`].
pub(crate) fn vouched_if_made(
    root: &std::path::Path,
    skills_now: &crate::chats::SkillsNow,
    owner: &str,
    was_there: bool,
) -> Option<String> {
    let note = purlis_core::skills::levels::vouch_if_made(root, owner, was_there);
    skills_now.forget();
    note
}

/// **Rejects a draft**: deletes it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_draft_reject(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("rejecting the draft", move || {
        drafts::reject(held.root(), &id)?;
        crate::skillgrade::drafted::stop(&plane, &id);
        held.chats().asks_may_have_moved();
        Ok(())
    })
    .await
}

/// **"Ask a chat to draft a skill"** (decision 13): a chat where `level`'s skills live (#5184: the
/// project's root, the workspace's folder, or as the persona), on the project's default profile, with
/// the core's prompt typed and never sent, ending with `what` the skill should do where New
/// skill… asked it (S4, #3684).
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_draft_chat(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    what: String,
    columns: u16,
    rows: u16,
) -> Result<Curating, StartRefused> {
    let held = planes.held(&plane)?;
    crate::curation::off_the_window_refused("starting the chat", move || {
        let root = held.root().to_path_buf();
        let level: purlis_core::skills::verbs::Level = (&level).into();
        let (cwd, persona) = where_the_skill_lives(&root, &level)?;
        let (profile, _) = crate::curation::default_profile(&root, "no chat was opened")?;
        let asked = drafts::asking(&level, &what);
        open(
            &held,
            ChatTyped {
                profile,
                persona,
                cwd,
                label: "Draft a skill".to_owned(),
                prompt: asked.sentence.clone(),
                skill: Some(asked),
                then_a_space: false,
                starts_runs: None,
                studio: Some(StudioJob::new(
                    None,
                    level.clone(),
                    StudioWork::Draft,
                    chrono::Utc::now(),
                )),
            },
            purlis_core::engine::Size { columns, rows },
        )
    })
    .await
}

/// **"Ask a chat to write them"** (#3925): a chat where `level`'s skills are reached from, on the
/// project's default profile, with the core's prompt for the trigger tests of the skill `name`
/// typed and never sent ([`drafts::asking_for_trigger_tests`]). It starts as "Ask a chat to draft
/// a skill" does, through the same start; its write comes back as a draft.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_trigger_tests_chat(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    name: String,
    columns: u16,
    rows: u16,
) -> Result<Curating, StartRefused> {
    let held = planes.held(&plane)?;
    crate::curation::off_the_window_refused("starting the chat", move || {
        let level: purlis_core::skills::verbs::Level = (&level).into();
        let prompt = drafts::asking_for_trigger_tests(held.root(), &level, &name)?;
        open_on_a_skill(
            &held,
            on_a_skill(&level, &name, StudioWork::TriggerTests),
            format!("Trigger tests of {name}"),
            prompt,
            purlis_core::engine::Size { columns, rows },
        )
    })
    .await
}

/// **"Ask a chat to write outcome evals"** (#2780; decision 43): a chat where `level`'s skills are
/// reached from, with the core's prompt for the outcome evals of the skill `name` typed and never
/// sent ([`drafts::asking_for_outcome_evals`]). Its write comes back as a draft.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_outcome_evals_chat(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    name: String,
    columns: u16,
    rows: u16,
) -> Result<Curating, StartRefused> {
    let held = planes.held(&plane)?;
    crate::curation::off_the_window_refused("starting the chat", move || {
        let level: purlis_core::skills::verbs::Level = (&level).into();
        let prompt = drafts::asking_for_outcome_evals(held.root(), &level, &name)?;
        open_on_a_skill(
            &held,
            on_a_skill(&level, &name, StudioWork::OutcomeEvals),
            format!("Outcome evals of {name}"),
            prompt,
            purlis_core::engine::Size { columns, rows },
        )
    })
    .await
}

/// **"Ask a chat to write checks"** (ruling of 2026-11-02): a chat where `level`'s skills are
/// reached from, with the core's prompt for both kinds of checks of the skill `name`, its trigger
/// tests and its outcome evals, typed and never sent ([`drafts::asking_for_checks`]). Both files
/// come back in the skill's one draft, accepted whole.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_checks_chat(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    name: String,
    columns: u16,
    rows: u16,
) -> Result<Curating, StartRefused> {
    let held = planes.held(&plane)?;
    crate::curation::off_the_window_refused("starting the chat", move || {
        let level: purlis_core::skills::verbs::Level = (&level).into();
        let prompt = drafts::asking_for_checks(held.root(), &level, &name)?;
        open_on_a_skill(
            &held,
            on_a_skill(&level, &name, StudioWork::Checks),
            format!("Checks of {name}"),
            prompt,
            purlis_core::engine::Size { columns, rows },
        )
    })
    .await
}

/// **"Improve from runs"** (#2780; decision 59): a chat where `level`'s skills are reached from,
/// with the core's prompt to read the recent runs and grades of the skill `name` and draft an edit
/// typed and never sent ([`drafts::improving_from_runs`]). It never edits the skill: its write
/// comes back as a draft.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_improve_chat(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    name: String,
    columns: u16,
    rows: u16,
) -> Result<Curating, StartRefused> {
    let held = planes.held(&plane)?;
    crate::curation::off_the_window_refused("starting the chat", move || {
        let level: purlis_core::skills::verbs::Level = (&level).into();
        let prompt = drafts::improving_from_runs(held.root(), &level, &name)?;
        open_on_a_skill(
            &held,
            on_a_skill(&level, &name, StudioWork::Improve),
            format!("Improve {name}"),
            prompt,
            purlis_core::engine::Size { columns, rows },
        )
    })
    .await
}

/// The job `work` on the skill `name` at `level`, not done yet.
fn on_a_skill(
    level: &purlis_core::skills::verbs::Level,
    name: &str,
    work: StudioWork,
) -> StudioJob {
    StudioJob::new(
        Some(name.to_owned()),
        level.clone(),
        work,
        chrono::Utc::now(),
    )
}

/// A chat where the skill of `job` lives, on the project's default profile, with `prompt` typed
/// and never sent, and `job` on its record (#5185).
fn open_on_a_skill(
    held: &Arc<Held>,
    job: StudioJob,
    label: String,
    skill: purlis_core::skillprompt::SkillPrompt,
    size: purlis_core::engine::Size,
) -> Result<Curating, StartRefused> {
    let root = held.root();
    let (cwd, persona) = where_the_skill_lives(root, &job.level)?;
    let (profile, _) = crate::curation::default_profile(root, "no chat was opened")?;
    open(
        held,
        ChatTyped {
            profile,
            persona,
            cwd,
            label,
            prompt: skill.sentence.clone(),
            skill: Some(skill),
            then_a_space: false,
            starts_runs: None,
            studio: Some(job),
        },
        size,
    )
}

/// **Try it** (decision 50): a scratch chat in a throwaway piece, armed with the built-in skills
/// and the draft `id` alone, on `profile` (the project's default where none is picked), with the
/// core's prompt typed and never sent. Tries a closed chat never ended (a quit, a crash) are
/// ended first. Nothing is left once the chat closes but the skill runs it recorded.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_draft_try(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: String,
    profile: Option<String>,
    columns: u16,
    rows: u16,
) -> Result<Curating, StartRefused> {
    let held = planes.held(&plane)?;
    crate::curation::off_the_window_refused("starting Try it", move || {
        let root = held.root().to_path_buf();
        let standing: Vec<std::path::PathBuf> = held
            .chats()
            .open_now()
            .into_iter()
            .filter_map(|open| open.cwd)
            .filter_map(|cwd| std::fs::canonicalize(cwd).ok())
            .collect();
        drafts::sweep_tries(&root, &standing);
        let draft = drafts::one(&root, &id)?;
        let profile = try_profile(&held, &draft, profile)?;
        let tried = drafts::try_out(&root, &id)?;
        let asked = drafts::trying(&root, &tried, &draft.name);
        let started = open(
            &held,
            ChatTyped {
                profile,
                persona: None,
                cwd: tried.piece.clone(),
                label: format!("Try {}", draft.name),
                prompt: asked.sentence.clone(),
                skill: Some(asked),
                then_a_space: false,
                starts_runs: None,
                studio: None,
            },
            purlis_core::engine::Size { columns, rows },
        );
        if started.is_err()
            && let Err(why) = drafts::end_try(&root, &tried.id)
        {
            tracing::warn!("purlis: a Try it that did not start was not ended ({why})");
        }
        started
    })
    .await
}

/// The profile Try it starts `draft`'s chat on: `picked`, where the person picked one, else the
/// harness and profile of the draft's own level (decision 50): the chat that drafted it where it
/// still runs there, else another chat of that workspace (or of the project's root), else the
/// project's default.
fn try_profile(
    held: &Arc<Held>,
    draft: &drafts::Draft,
    picked: Option<String>,
) -> Result<String, String> {
    if let Some(profile) = picked {
        return Ok(profile);
    }
    let chats_open = held.chats().open_now();
    let at_level = |one: &&crate::chats::Open| {
        one.workspace.as_deref() == draft.level.at.workspace() && one.profile.is_some()
    };
    let of_level = chats_open
        .iter()
        .filter(at_level)
        .find(|one| Some(one.session) == draft.chat)
        .or_else(|| chats_open.iter().find(at_level))
        .and_then(|one| one.profile.clone());
    match of_level {
        Some(profile) => Ok(profile),
        None => Ok(crate::curation::default_profile(held.root(), "Try it did not start")?.0),
    }
}

/// **"Ask the chat to redo it"** (S9), for a draft the project changed a file of since: the
/// core's redo prompt typed into chat `session`, the chat that drafted it, unsent — or copied,
/// with why, where it cannot be typed now. Refused where `session` is not that chat, open in the
/// draft's workspace. A redo the chat then writes is drafted on the project's copy (`drafts::take`).
// On the window's thread, as every command that types into a chat is (`useskill::use_skill`).
// Not a doc comment past here, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub(crate) fn skill_draft_redo_in(
    planes: tauri::State<'_, Planes>,
    clipboard: tauri::State<'_, crate::vaults::SystemClipboard>,
    plane: PlaneId,
    id: String,
    session: u32,
) -> Result<crate::references::Handed, String> {
    let held = planes.held(&plane)?;
    let draft = drafts::one(held.root(), &id)?;
    let open = held.chats().open_now();
    if drafting_chat(&open, &draft).map(|one| one.session) != Some(session) {
        return Err(NOT_ITS_CHAT.to_owned());
    }
    crate::references::typed_or_copied(
        &held,
        session,
        drafts::redoing_in_its_chat(&draft.level, &draft.name, &stale_of(&draft)),
        REDO_COPIED,
        &|text| clipboard.put_text(text),
    )
}

/// The files of `draft` the project changed since, below the skill's folder.
fn stale_of(draft: &drafts::Draft) -> Vec<&str> {
    draft
        .files
        .iter()
        .filter(|file| file.stale)
        .map(|file| file.path.as_str())
        .collect()
}

/// What [`skill_draft_redo_in`] refuses a chat with that is not the draft's own, open now.
const NOT_ITS_CHAT: &str =
    "the chat that drafted it is not open any more, so nothing was typed: ask a chat to redo it";

/// What the clipboard is said to hold where the redo prompt could not be typed, and the rule it
/// is typed by: at the chat's prompt, as Use skill… types a skill.
const REDO_COPIED: crate::references::Copied = crate::references::Copied {
    on: "the request is",
    it: "it is",
    may: |_, now| purlis_core::reference::at_its_prompt(now),
};

/// **"Ask a chat to redo it"** (S9), where the chat that drafted it is closed: a chat where the
/// draft's level's skills are reached from, as "Ask a chat to draft a skill" opens one, with the
/// core's redo prompt typed and never sent, which the person ends with what the change was.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_draft_redo(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    id: String,
    columns: u16,
    rows: u16,
) -> Result<Curating, StartRefused> {
    let held = planes.held(&plane)?;
    crate::curation::off_the_window_refused("starting the chat", move || {
        let root = held.root().to_path_buf();
        let draft = drafts::one(&root, &id)?;
        let (cwd, persona) = where_the_skill_lives(&root, &draft.level)?;
        let (profile, _) = crate::curation::default_profile(&root, "no chat was opened")?;
        let asked = drafts::redoing(&draft.level, &draft.name, &stale_of(&draft));
        open(
            &held,
            ChatTyped {
                profile,
                persona,
                cwd,
                label: format!("Redo {}", draft.name),
                prompt: asked.sentence.clone(),
                skill: Some(asked),
                then_a_space: false,
                starts_runs: None,
                studio: Some(StudioJob::new(
                    Some(draft.name.clone()),
                    draft.level.clone(),
                    StudioWork::Redo,
                    chrono::Utc::now(),
                )),
            },
            purlis_core::engine::Size { columns, rows },
        )
    })
    .await
}

/// **Where a chat the studio starts for a skill of `level` starts** (#5184): where the skill
/// lives. A project's skill at the project's root, a workspace's in its workspace's folder, and a
/// persona's at the project's root as that persona, the only chats its skills reach. The folder,
/// and the persona it runs as.
fn where_the_skill_lives(
    root: &std::path::Path,
    level: &purlis_core::skills::verbs::Level,
) -> Result<(std::path::PathBuf, Option<String>), String> {
    if let Some(persona) = level.at.persona() {
        return Ok((root.to_path_buf(), Some(persona.to_owned())));
    }
    match level.at.workspace() {
        None => Ok((root.to_path_buf(), None)),
        Some(ws) => Ok((
            purlis_core::workspaces::Plane::open(root)
                .workspace(ws)
                .map_err(|_| {
                    format!("this project has no workspace {ws:?}, so no chat was opened")
                })?
                .dir()
                .to_path_buf(),
            None,
        )),
    }
}

/// Opens `chat` with its prompt typed once its harness has started, as a curation chat is. A
/// refused start says the approval its profile needs, where it needs one.
fn open(
    held: &Arc<Held>,
    chat: ChatTyped,
    size: purlis_core::engine::Size,
) -> Result<Curating, StartRefused> {
    let root = held.root().to_path_buf();
    let (label, cwd, profile) = (chat.label.clone(), chat.cwd.clone(), chat.profile.clone());
    let persona = chat.persona.clone();
    let typed =
        crate::curation::start_typed(held, chat, size, crate::curation::when_typed, |why| {
            format!("{why}, so nothing was typed and no chat was opened.")
        })
        .map_err(StartRefused::on(&profile, &root))?;
    Ok(Curating {
        session: typed.session,
        name: typed.name,
        label,
        persona,
        harness: typed.ready.declaration().map(|d| d.name.clone()),
        workspace: purlis_core::workspaces::Plane::open(&root).workspace_of(&cwd),
    })
}

/// **Ends the Try it chat `cwd` was the throwaway piece of**, in the project at `root`, once its
/// program has had time to end, where no other open chat stands in it (a restart starts before
/// the chat it replaces ends). Nothing for any other folder.
pub(crate) fn closed_in(root: &std::path::Path, cwd: &std::path::Path) {
    if !drafts::a_piece(cwd) {
        return;
    }
    let (root, cwd) = (root.to_path_buf(), cwd.to_path_buf());
    std::thread::spawn(move || {
        std::thread::sleep(purlis_core::skills::TIDY_AFTER_A_CLOSE);
        if let Err(why) = drafts::end_try_at(&root, &cwd) {
            tracing::warn!("purlis: a Try it's throwaway folder was not removed ({why})");
        }
    });
}

/// **A closed chat's Skill studio leftovers go** (SK-9, #2772), each time the app writes its
/// record of the open chats in the project at `root`: the record of the levels a sandboxed chat
/// that is no longer on it could have made, and every Try it no chat on it stands in. At a launch
/// that is every one a quit or a crash left.
/// Where a record went, each open chat's kept skill stamp in `skills_now` is read again (#4884).
pub(crate) fn sweep(
    root: &std::path::Path,
    record: &purlis_core::reopen::Record,
    skills_now: &crate::chats::SkillsNow,
) {
    let ids: Vec<String> = record
        .chats
        .iter()
        .filter_map(|chat| chat.identity.id.clone())
        .collect();
    let cwds: Vec<std::path::PathBuf> = record
        .chats
        .iter()
        .filter_map(|chat| chat.cwd.clone())
        .collect();
    // A closed chat's record gone: what it could have made is left out of every chat now (#4884).
    if purlis_core::skills::levels::sweep(root, &ids) {
        skills_now.forget();
    }
    purlis_core::hoststart::sweep_settings(
        purlis_core::hoststart::chat_settings_here().as_deref(),
        root,
        &ids,
    );
    drafts::sweep_tries(root, &cwds);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plane with the workspace `ops`.
    fn a_plane() -> tempfile::TempDir {
        let plane = tempfile::tempdir().expect("a plane");
        std::fs::write(plane.path().join("charter.toml"), "schema = 1\n").expect("a manifest");
        std::fs::create_dir_all(plane.path().join("workspaces/ops")).expect("a workspace");
        plane
    }

    #[test]
    fn the_studios_accept_of_a_folder_makes_the_chats_read_their_skills_again() {
        // #4813 review: what a chat would start on now reads which folders were accepted, from
        // the app state the plane's watch never tells of.
        let plane = a_plane();
        let skills_now = crate::chats::SkillsNow::default();

        level_accepted(plane.path(), &skills_now, "workspaces/ops").expect("accepted");

        assert_eq!(skills_now.forgotten(), 1);
    }

    #[test]
    fn the_studios_accept_of_the_projects_folders_makes_the_chats_read_their_skills_again() {
        let plane = a_plane();
        let skills_now = crate::chats::SkillsNow::default();

        levels_confirmed(plane.path(), &skills_now, &["workspaces/ops".to_owned()])
            .expect("confirmed");

        assert_eq!(skills_now.forgotten(), 1);
    }

    const CHAT: &str = "01J0000000000000000000000A";

    #[test]
    fn a_vouch_after_the_persons_create_makes_the_chats_read_their_skills_again() {
        // #4884: the folder is vouched for in every open chat's record of the levels it could
        // have made, which is what a chat would start on now.
        let plane = a_plane();
        let skills_now = crate::chats::SkillsNow::default();
        purlis_core::skills::levels::note(plane.path(), CHAT, "7").expect("noted");
        std::fs::create_dir_all(plane.path().join("workspaces/new")).expect("made");

        vouched_if_made(plane.path(), &skills_now, "workspaces/new", false);

        assert_eq!(skills_now.forgotten(), 1);
    }

    #[test]
    fn a_sweep_that_takes_a_closed_chats_record_makes_the_chats_read_their_skills_again() {
        // #4884: what the closed chat could have made is left out of every chat from then on,
        // in the app state the plane's watch never tells of.
        let plane = a_plane();
        let skills_now = crate::chats::SkillsNow::default();
        purlis_core::skills::levels::note(plane.path(), CHAT, "7").expect("noted");
        let none_open = purlis_core::reopen::Record::default();

        // Just noted, so not swept yet: nothing changed, and the stamps are kept.
        sweep(plane.path(), &none_open, &skills_now);
        assert_eq!(skills_now.forgotten(), 0);

        noted_long_ago(plane.path(), CHAT);
        sweep(plane.path(), &none_open, &skills_now);
        assert_eq!(skills_now.forgotten(), 1);
    }

    #[test]
    fn a_project_skills_chat_starts_at_the_projects_root_as_no_persona() {
        // #5184: a studio chat starts where the skill lives.
        let plane = a_plane();

        let at = where_the_skill_lives(plane.path(), &purlis_core::skills::verbs::Level::project())
            .expect("a place");

        assert_eq!(at, (plane.path().to_path_buf(), None));
    }

    #[test]
    fn a_workspace_skills_chat_starts_in_its_workspaces_folder() {
        let plane = a_plane();

        let at = where_the_skill_lives(
            plane.path(),
            &purlis_core::skills::verbs::Level::workspace("ops"),
        )
        .expect("a place");

        assert_eq!(at, (plane.path().join("workspaces/ops"), None));
    }

    #[test]
    fn a_persona_skills_chat_starts_at_the_projects_root_as_that_persona() {
        // #5184: a persona's skills reach only the chats that run as it, so its chat is one.
        let plane = a_plane();

        let at = where_the_skill_lives(
            plane.path(),
            &purlis_core::skills::verbs::Level::persona("steward"),
        )
        .expect("a place");

        assert_eq!(at, (plane.path().to_path_buf(), Some("steward".to_owned())));
    }

    #[test]
    fn the_window_reads_a_studio_chats_skill_level_and_job_off_its_record() {
        // #5185: what the chat's chip and the skill page's line are drawn from.
        let started = chrono::DateTime::parse_from_rfc3339("2026-11-04T17:31:00Z")
            .expect("a time")
            .with_timezone(&chrono::Utc);
        let link = StudioLink::from(&StudioJob::new(
            Some("test-design".to_owned()),
            purlis_core::skills::verbs::Level::workspace("purlis-demo"),
            StudioWork::OutcomeEvals,
            started,
        ));

        assert_eq!(
            serde_json::to_value(link).expect("json"),
            serde_json::json!({
                "skill": "test-design",
                "level": {"at": "workspace", "name": "purlis-demo", "local": false},
                "job": "outcome-evals",
                "done": false,
                "started": "2026-11-04T17:31:00Z",
                "finished": null,
            })
        );
    }

    #[test]
    fn the_window_is_typed_every_studio_job_by_the_word_the_core_writes() {
        // #5248: the window's type is the core's jobs, no more and no fewer.
        let typed: std::collections::BTreeSet<String> = [
            StudioJobKind::Draft,
            StudioJobKind::TriggerTests,
            StudioJobKind::OutcomeEvals,
            StudioJobKind::Checks,
            StudioJobKind::Improve,
            StudioJobKind::Redo,
        ]
        .iter()
        .map(|kind| serde_json::to_string(kind).expect("json"))
        .collect();
        let written: std::collections::BTreeSet<String> = StudioWork::all()
            .map(|work| serde_json::to_string(&work).expect("json"))
            .collect();

        assert_eq!(typed, written);
    }

    /// Makes the record of `id` look as if it was written long ago, past the sweep's grace.
    fn noted_long_ago(root: &std::path::Path, id: &str) {
        let record = purlis_core::appstate::dir(root)
            .expect("the app state")
            .join(purlis_core::skills::levels::DIR)
            .join(format!("{id}.json"));
        let old =
            std::time::SystemTime::now() - purlis_core::skills::verbs::drafts::SWEEP_GRACE * 2;
        std::fs::File::options()
            .write(true)
            .open(record)
            .expect("the record")
            .set_modified(old)
            .expect("made old");
    }
}
