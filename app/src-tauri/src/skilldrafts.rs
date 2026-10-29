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
//! - **Try it** opens a scratch chat in a throwaway piece, armed with the built-in skills and the
//!   draft alone, on the profile the person picked or the project's default; the piece goes when
//!   the chat closes (`Chats::close`).
//!
//! Behind the `skill-studio` flag until its finish line (#2786).

use std::sync::Arc;

use serde::Serialize;
use specta::Type;

use purlis_core::hookwire::{Answer, ProposeAsk};
use purlis_core::skills::verbs::drafts;

use crate::curation::{ChatTyped, Curating};
use crate::planes::{Held, PlaneId, Planes};
use crate::skillchange::{SkillChanged, SkillLevel};

/// **Keeps chat `ask.chat`'s change as a draft skill** where it is to a plane skill's file and
/// the Skill studio's flag is on: the answer, or `None` for a proposal that is not one, which is
/// then a proposed change as before. Nothing in the project is written.
pub fn drafted(held: &Held, ask: &ProposeAsk) -> Option<Answer> {
    if !purlis_core::skills::studio_on_here() {
        return None;
    }
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

/// One draft skill, as the studio lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillDraft {
    /// What Try it, Accept and Reject are handed back.
    pub id: String,
    pub name: String,
    pub level: SkillLevel,
    /// The chat that wrote it last, by its number.
    pub chat: Option<u32>,
    /// Whether that chat is open now, in the draft's own workspace: "Ask the chat to redo it"
    /// types into it ([`skill_draft_redo_in`]), where a closed one's opens a chat afresh
    /// ([`skill_draft_redo`]).
    pub chat_open: bool,
    /// A new skill, rather than a change to one.
    pub new: bool,
    /// A file it changes changed in the project since: Accept is refused.
    pub stale: bool,
    pub files: Vec<SkillDraftFile>,
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
/// asks to say "draft ready in Skill studio". None while the flag is off.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_drafts(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<SkillDraft>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the draft skills", move || {
        if !purlis_core::skills::studio_on_here() {
            return Ok(Vec::new());
        }
        let open = held.chats().open_now();
        Ok(drafts::list(held.root())
            .into_iter()
            .map(|draft| {
                let chat_open = drafting_chat(&open, &draft).is_some();
                SkillDraft {
                    chat_open,
                    ..SkillDraft::from(draft)
                }
            })
            .collect())
    })
    .await
}

/// **The chat that drafted `draft`, where it is still open** among `open`: the chat of its number
/// in the draft's own workspace (or at the project's root for a project's or a persona's draft),
/// so a number a later chat took elsewhere is never it.
fn drafting_chat<'a>(
    open: &'a [crate::chats::Open],
    draft: &drafts::Draft,
) -> Option<&'a crate::chats::Open> {
    let chat = draft.chat?;
    open.iter()
        .find(|one| one.session == chat && one.workspace.as_deref() == draft.level.at.workspace())
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
        crate::skillchange::flag_on()?;
        let changed = crate::skillchange::heard(held.root(), |cx| drafts::accept(cx, &id))?;
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
        crate::skillchange::flag_on()?;
        purlis_core::skills::levels::accept(held.root(), &folder)?;
        Ok(SkillChanged::default())
    })
    .await
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
        crate::skillchange::flag_on()?;
        drafts::reject(held.root(), &id)?;
        held.chats().asks_may_have_moved();
        Ok(())
    })
    .await
}

/// **"Ask a chat to draft a skill"** (decision 13): a chat where `level`'s skills are reached
/// from (the project's root, or the workspace's folder), on the project's default profile, with
/// the core's prompt typed and never sent.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_draft_chat(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    columns: u16,
    rows: u16,
) -> Result<Curating, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("starting the chat", move || {
        crate::skillchange::flag_on()?;
        let root = held.root().to_path_buf();
        let level: purlis_core::skills::verbs::Level = (&level).into();
        let cwd = where_level_is_reached(&root, &level)?;
        let (profile, _) = crate::curation::default_profile(&root, "no chat was opened")?;
        open(
            &held,
            ChatTyped {
                profile,
                persona: None,
                cwd,
                label: "Draft a skill".to_owned(),
                prompt: drafts::asking(&level),
                then_a_space: false,
                starts_runs: None,
            },
            purlis_core::engine::Size { columns, rows },
        )
    })
    .await
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
) -> Result<Curating, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("starting Try it", move || {
        crate::skillchange::flag_on()?;
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
        // The harness and profile of the draft's own level (decision 50): the chat that drafted
        // it where it still runs there, else another chat of that workspace (or of the
        // project's root), else the project's default.
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
        let profile = match profile.or(of_level) {
            Some(profile) => profile,
            None => crate::curation::default_profile(&root, "Try it did not start")?.0,
        };
        let tried = drafts::try_out(&root, &id)?;
        let started = open(
            &held,
            ChatTyped {
                profile,
                persona: None,
                cwd: tried.piece.clone(),
                label: format!("Try {}", draft.name),
                prompt: drafts::trying(&draft.name),
                then_a_space: false,
                starts_runs: None,
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
    crate::skillchange::flag_on()?;
    let draft = drafts::one(held.root(), &id)?;
    let open = held.chats().open_now();
    if drafting_chat(&open, &draft).map(|one| one.session) != Some(session) {
        return Err(NOT_ITS_CHAT.to_owned());
    }
    crate::references::typed_or_copied(
        &held,
        session,
        drafts::redoing(&draft.level, &draft.name, true),
        REDO_COPIED,
        &|text| clipboard.put_text(text),
    )
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
) -> Result<Curating, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("starting the chat", move || {
        crate::skillchange::flag_on()?;
        let root = held.root().to_path_buf();
        let draft = drafts::one(&root, &id)?;
        let cwd = where_level_is_reached(&root, &draft.level)?;
        let (profile, _) = crate::curation::default_profile(&root, "no chat was opened")?;
        open(
            &held,
            ChatTyped {
                profile,
                persona: None,
                cwd,
                label: format!("Redo {}", draft.name),
                prompt: drafts::redoing(&draft.level, &draft.name, false),
                then_a_space: false,
                starts_runs: None,
            },
            purlis_core::engine::Size { columns, rows },
        )
    })
    .await
}

/// Where a chat that writes `level`'s skills starts: the project's root, or its workspace's
/// folder.
fn where_level_is_reached(
    root: &std::path::Path,
    level: &purlis_core::skills::verbs::Level,
) -> Result<std::path::PathBuf, String> {
    match level.at.workspace() {
        None => Ok(root.to_path_buf()),
        Some(ws) => Ok(purlis_core::workspaces::Plane::open(root)
            .workspace(ws)
            .map_err(|_| format!("this project has no workspace {ws:?}, so no chat was opened"))?
            .dir()
            .to_path_buf()),
    }
}

/// Opens `chat` with its prompt typed once its harness has started, as a curation chat is.
fn open(
    held: &Arc<Held>,
    chat: ChatTyped,
    size: purlis_core::engine::Size,
) -> Result<Curating, String> {
    let root = held.root().to_path_buf();
    let (label, cwd) = (chat.label.clone(), chat.cwd.clone());
    let typed =
        crate::curation::start_typed(held, chat, size, crate::curation::when_typed, |why| {
            format!("{why}, so nothing was typed and no chat was opened.")
        })?;
    Ok(Curating {
        session: typed.session,
        name: typed.name,
        label,
        persona: None,
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
pub(crate) fn sweep(root: &std::path::Path, record: &purlis_core::reopen::Record) {
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
    purlis_core::skills::levels::sweep(root, &ids);
    purlis_core::hoststart::sweep_settings(
        purlis_core::hoststart::chat_settings_here().as_deref(),
        root,
        &ids,
    );
    drafts::sweep_tries(root, &cwds);
}
