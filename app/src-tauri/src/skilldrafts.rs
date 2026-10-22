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

/// What the chat's `purlis propose`, or its refused tool, is answered with where its write became
/// a draft.
pub const KEPT_SAID: &str = "kept as a draft skill: the person tries it and accepts or rejects \
                             it in the Skill studio.";

/// **Keeps chat `ask.chat`'s change as a draft skill** where it is to a plane skill's file and
/// the Skill studio's flag is on: the answer, or `None` for a proposal that is not one, which is
/// then a proposed change as before. Nothing in the project is written.
pub fn drafted(held: &Held, ask: &ProposeAsk) -> Option<Answer> {
    if !purlis_core::skills::studio_on_here() {
        return None;
    }
    let root = held.root();
    let file = drafts::draftable(root, &root.join(&ask.path))?;
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
                lines: vec![purlis_core::repocmd::Say::Done(KEPT_SAID.to_owned())],
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
    /// Its level as a person reads it: `project`, `local workspace ops`.
    pub label: String,
    /// The chat that wrote it last, by its number.
    pub chat: Option<u32>,
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
            label: draft.level.to_string(),
            level: (&draft.level).into(),
            id: draft.id,
            name: draft.name,
            chat: draft.chat,
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
        Ok(drafts::list(held.root())
            .into_iter()
            .map(SkillDraft::from)
            .collect())
    })
    .await
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
        let cwd = match level.workspace_name() {
            None => root.clone(),
            Some(ws) => purlis_core::workspaces::Plane::open(&root)
                .workspace(ws)
                .map_err(|_| format!("this project has no workspace {ws:?}, so no chat was opened"))?
                .dir()
                .to_path_buf(),
        };
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
        let temp = std::env::temp_dir();
        let standing: Vec<std::path::PathBuf> = held
            .chats()
            .open_now()
            .into_iter()
            .filter_map(|open| open.cwd)
            .filter_map(|cwd| std::fs::canonicalize(cwd).ok())
            .collect();
        for (left, piece) in drafts::tries(&root) {
            if !standing.contains(&piece)
                && let Err(why) = drafts::end_try(&root, &left, &temp)
            {
                tracing::warn!("purlis: a Try it left behind was not ended ({why})");
            }
        }
        let draft = drafts::one(&root, &id)?;
        let profile = match profile {
            Some(profile) => profile,
            None => crate::curation::default_profile(&root, "Try it did not start")?.0,
        };
        let tried = drafts::try_out(&root, &id, &temp)?;
        let started = open(
            &held,
            ChatTyped {
                profile,
                persona: None,
                cwd: tried.piece.clone(),
                label: format!("Try {}", draft.name),
                prompt: drafts::trying(&draft.name),
                then_a_space: false,
            },
            purlis_core::engine::Size { columns, rows },
        );
        if started.is_err()
            && let Err(why) = drafts::end_try(&root, &tried.id, &temp)
        {
            tracing::warn!("purlis: a Try it that did not start was not ended ({why})");
        }
        started
    })
    .await
}

/// Opens `chat` with its prompt typed once its harness has started, as a curation chat is.
fn open(
    held: &Arc<Held>,
    chat: ChatTyped,
    size: purlis_core::engine::Size,
) -> Result<Curating, String> {
    let root = held.root().to_path_buf();
    let (label, cwd) = (chat.label.clone(), chat.cwd.clone());
    let typed = crate::curation::start_typed(
        held,
        chat,
        size,
        |profile, declared, card| {
            declared
                .and_then(purlis_core::harness_declaration::Declaration::ready_to_type)
                .ok_or_else(|| match crate::curation::cannot_be_typed_into(card) {
                    Some(lacks) => format!("'{profile}' cannot open this chat: {lacks}"),
                    None => format!(
                        "'{profile}' starts a program purlis has not measured, so it cannot tell \
                         when to type into it, and no chat was opened."
                    ),
                })
        },
        |why| format!("{why}, so nothing was typed and no chat was opened."),
    )?;
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
    let is_a_piece = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(drafts::PIECE_PREFIX));
    if !is_a_piece {
        return;
    }
    let (root, cwd) = (root.to_path_buf(), cwd.to_path_buf());
    std::thread::spawn(move || {
        std::thread::sleep(purlis_core::skills::TIDY_AFTER_A_CLOSE);
        if let Err(why) = drafts::end_try_at(&root, &cwd, &std::env::temp_dir()) {
            tracing::warn!("purlis: a Try it's throwaway folder was not removed ({why})");
        }
    });
}
