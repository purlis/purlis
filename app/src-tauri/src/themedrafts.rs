// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Theme drafts in the window** (#2332, spec #2313): which chat's draft a workspace draws, the
//! strip that names it, and the person's Keep, Revert and the moves up a level.
//!
//! The theme engine decides everything about a draft ([`purlis_core::theme::drafts`]): where it
//! is, how it is read, whether it holds the hard floors, what Keep and Revert leave on disk. This
//! is the app's half: **which chats work where**, from this app's own record of its open chats
//! ([`crate::chats::Chats::drafting`]), never from a path a chat wrote; and the last good version
//! of each draft, held in memory, which stays drawn while a save breaks a hard floor.
//!
//! Keep, Revert, Revert all (the revert key) and the moves up a level are the person's presses in
//! the window: each is on the window-only list (`purlis_session_protocol::ui::WINDOW_ONLY`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

use purlis_core::theme::{self, drafts};

use crate::chats::Drafting;

/// One theme draft, as the strip and the close of its chat name it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct WindowDraft {
    /// The theme folder's name.
    pub folder: String,
    /// The chat that writes it: its number here, and what the person calls it.
    pub session: u32,
    pub chat: String,
    /// The workspace the chat works in, `None` at the plane root: where the draft is drawn.
    pub workspace: Option<String>,
    /// Where Keep puts it: `workspace` (the chat's workspace's `themes/`) or `project`.
    pub kept_to: String,
    /// Whether Keep replaces a theme folder that is there: the window asks the person first.
    pub replaces: bool,
    /// Whether the theme folder Keep puts it in is not what the draft started from: made,
    /// changed or deleted since ([`drafts::changed`]). The window says so, and Keep asks first.
    pub changed: bool,
    /// Whether the window draws it now: the draft, or its last good version.
    pub drawn: bool,
    /// The hard floors the draft as saved breaks, in the engine's words: why it is not drawn.
    pub broken: Vec<String>,
    /// Why the engine refused the draft as saved, when it did.
    pub refused: Vec<String>,
    /// What the refused draft's Notice may offer (#2647): `fonts` where every font file it waits
    /// on is one the project brings in the folder the draft is kept to, with those bytes, and
    /// the project asks again about font files alone, so its trust ask lifts the refusal; the
    /// rules of a refused theme otherwise. `None` for a draft not refused for a font.
    pub review: Option<crate::extensions::ThemeReview>,
    /// Whether the theme as kept is refused while the draft is drawn: its Notice, and any
    /// Review and allow on it, waits until the draft is reverted, and the strip says so.
    pub kept_refused: bool,
}

/// The chats of the plane `held` that work at `workspace` (`None`: the plane root).
pub(crate) fn chats_at(held: &crate::planes::Held, workspace: Option<&str>) -> Vec<Drafting> {
    held.chats()
        .drafting()
        .into_iter()
        .filter(|one| one.workspace.as_deref() == workspace)
        .collect()
}

/// Each draft's last good version, by its plane, its chat and its folder.
type GoodVersions = HashMap<(PathBuf, String, String), theme::Theme>;

/// The last good version of each draft, by plane, chat and folder: what stays drawn while a
/// save of it breaks a hard floor. In memory only; a draft's first draw after a launch has none.
fn last_good() -> std::sync::MutexGuard<'static, GoodVersions> {
    static GOOD: LazyLock<Mutex<GoodVersions>> = LazyLock::new(|| Mutex::new(HashMap::new()));
    GOOD.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The drafts of `chats` at the plane `plane`, in `workspace`, with each chat's record.
fn drafts_of<'a>(
    config: &Path,
    plane: &Path,
    workspace: Option<&str>,
    chats: &'a [Drafting],
) -> Vec<(drafts::Draft, &'a Drafting)> {
    let ids: Vec<&str> = chats.iter().map(|one| one.id.as_str()).collect();
    let place = drafts::Place { plane, workspace };
    drafts::at(Some(config), place, &ids)
        .into_iter()
        .filter_map(|draft| {
            let chat = chats.iter().find(|one| one.id == draft.chat)?;
            Some((draft, chat))
        })
        .collect()
}

fn kept_to_word(kept_to: &drafts::KeptTo) -> String {
    match kept_to {
        drafts::KeptTo::Workspace(_) => "workspace",
        drafts::KeptTo::Project => "project",
    }
    .to_owned()
}

/// **What the window draws for the drafts where it is**: the first draft of the chats working
/// there, drawn by the engine with the hard floors and the consent floors held, or its last good
/// version; `None` when no chat there has a draft. The `Drawn` is `None` when neither the draft
/// nor a good version of it can be drawn, and the levels' theme is.
pub(crate) fn drafted_here(
    config: &Path,
    plane: &Path,
    workspace: Option<&str>,
    chats: &[Drafting],
    levels: &theme::Levels,
    light: bool,
    contrast: theme::Contrast,
) -> Option<(Option<theme::Drawn>, WindowDraft)> {
    if chats.is_empty() {
        return None;
    }
    let (draft, chat) = drafts_of(config, plane, workspace, chats)
        .into_iter()
        .next()?;
    let place = drafts::Place { plane, workspace };
    let read = drafts::read(Some(config), place, &draft, false);
    // The draft is drawn in the appearance the levels chose for its folder, else following the
    // system.
    let appearance = levels
        .each()
        .find(|(_, choice)| choice.theme == draft.folder)
        .map_or(Some("system".to_owned()), |(_, choice)| {
            choice.appearance.clone()
        });
    let key = (
        plane.to_path_buf(),
        draft.chat.clone(),
        draft.folder.clone(),
    );
    let good = last_good().get(&key).cloned();
    let drafted = drafts::drafted(
        read,
        good.as_ref(),
        appearance.as_deref(),
        if light {
            theme::Appearance::Light
        } else {
            theme::Appearance::Dark
        },
        contrast,
    );
    if let Some(theme) = &drafted.good {
        last_good().insert(key, theme.clone());
    }
    let window = WindowDraft {
        folder: draft.folder.clone(),
        session: chat.session,
        chat: chat.name.clone(),
        workspace: chat.workspace.clone(),
        kept_to: kept_to_word(&draft.kept_to),
        replaces: drafts::replaces(plane, &draft),
        changed: drafts::changed(plane, &draft),
        drawn: drafted.drawn.is_some(),
        broken: drafted.broken.iter().map(drafts::BrokenIn::said).collect(),
        refused: drafted.refused.iter().map(|one| one.said.clone()).collect(),
        review: draft_review(config, plane, &drafted.refused),
        kept_refused: false,
    };
    Some((drafted.drawn, window))
}

/// **What a refused draft's Notice may offer** (#2647): the faces its refusal waits on, each
/// `<path in the project> <sha-256>` of the folder the draft is kept to. Only where the project
/// itself brings every one of them, with those bytes, can its trust ask lift the refusal; then
/// the rules of any refused theme hold (`planes::theme_review`: font files alone, an approved
/// project). A draft's font the project does not bring is no project's to allow, so nothing is
/// offered for it.
fn draft_review(
    config: &Path,
    plane: &Path,
    refused: &[theme::Complaint],
) -> Option<crate::extensions::ThemeReview> {
    let mut faces: Vec<&str> = refused
        .iter()
        .filter_map(|one| one.awaits_approval.as_deref())
        .collect();
    if faces.is_empty() {
        return None;
    }
    faces.sort_unstable();
    faces.dedup();
    let brings = purlis_core::machine::Contribution::read(plane)
        .contributes
        .fonts;
    let all_brought = faces.iter().all(|face| {
        face.rsplit_once(' ').is_some_and(|(path, digest)| {
            brings
                .get(path)
                .is_some_and(|listed| listed.split(' ').next() == Some(digest))
        })
    });
    if !all_brought {
        return None;
    }
    crate::extensions::review_kept(plane, &faces, || crate::planes::theme_review(config, plane))
}

/// The theme drafts of the chat `session`, as the window asks before it closes that chat: each
/// draft, where Keep puts it. Empty for a chat with none. The window's alone: it names the chat
/// and where it works, and what Keep would do, which only the window acts on, as only the
/// window closes a chat.
#[tauri::command]
#[specta::specta]
pub async fn theme_drafts_of_chat(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
) -> Result<Vec<WindowDraft>, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let chats: Vec<Drafting> = held
        .chats()
        .drafting()
        .into_iter()
        .filter(|one| one.session == session)
        .collect();
    let config = crate::extensions::config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(chat) = chats.first() else {
            return Vec::new();
        };
        drafts_of(&config, &root, chat.workspace.as_deref(), &chats)
            .into_iter()
            .map(|(draft, chat)| WindowDraft {
                replaces: drafts::replaces(&root, &draft),
                changed: drafts::changed(&root, &draft),
                folder: draft.folder,
                session: chat.session,
                chat: chat.name.clone(),
                workspace: chat.workspace.clone(),
                kept_to: kept_to_word(&draft.kept_to),
                drawn: false,
                broken: Vec::new(),
                refused: Vec::new(),
                review: None,
                kept_refused: false,
            })
            .collect()
    })
    .await
    .map_err(|err| format!("reading the chat's theme drafts did not finish: {err}"))
}

/// **The event the window holding a project hears a link's close on** (#2761): the chats the
/// close would end, at least one of which has a theme draft. The window asks the person, as it
/// asks about any close of its own, Keep or Revert included.
pub const CLOSE_HANDED: &str = "theme-drafts-close-handed";

/// What [`CLOSE_HANDED`] carries: the project and the chats a link's close would end.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct CloseHanded {
    pub plane: crate::planes::PlaneId,
    pub sessions: Vec<u32>,
}

/// The chats among `chats` that have a theme draft, by session: only whether one is there, never
/// what it says ([`drafts::has_drafts`]).
fn drafted_among(plane: &Path, chats: &[Drafting]) -> Vec<u32> {
    chats
        .iter()
        .filter(|chat| drafts::has_drafts(plane, &chat.id))
        .map(|chat| chat.session)
        .collect()
}

/// **A link's close of chats that have theme drafts is handed to the window** (#2761): closing a
/// chat, and its drafts' Keep and Revert, are the window's alone, so a link never settles a
/// draft. Where one of `sessions` is a chat open here with a draft, the window holding the
/// project is told the chats ([`CLOSE_HANDED`]). That window decides: it asks the person, Keep
/// or Revert first, and brings itself and the project forward only when it does, never over a
/// question already on screen; it closes nothing unasked.
///
/// **The link is answered the same whatever was found**: nothing of the drafts, nor whether the
/// window was told. At most [`drafts::MOST_CHATS`] chats are read from the list, each once.
#[tauri::command]
#[specta::specta]
pub async fn hand_drafts_close_to_the_window(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    showing: tauri::State<'_, crate::planes::Showing>,
    plane: crate::planes::PlaneId,
    sessions: Vec<u32>,
) -> Result<(), String> {
    use tauri::Emitter;
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let asked = asked_sessions(sessions);
    // The chats the link's close ends that are open here: the window's close asks about every
    // one of them.
    let ending: Vec<Drafting> = held
        .chats()
        .drafting()
        .into_iter()
        .filter(|one| asked.contains(&one.session))
        .collect();
    let handed: Vec<u32> = ending.iter().map(|one| one.session).collect();
    let drafted = tauri::async_runtime::spawn_blocking(move || drafted_among(&root, &ending))
        .await
        .unwrap_or_default();
    if drafted.is_empty() {
        return Ok(());
    }
    if let Some(window) = showing.holder(&plane) {
        let _ = app.emit_to(
            window.as_str(),
            CLOSE_HANDED,
            CloseHanded {
                plane,
                sessions: handed,
            },
        );
    }
    Ok(())
}

/// The chats a link's close names: the first [`drafts::MOST_CHATS`] of its list, each once.
fn asked_sessions(mut sessions: Vec<u32>) -> Vec<u32> {
    sessions.truncate(drafts::MOST_CHATS);
    sessions.sort_unstable();
    sessions.dedup();
    sessions
}

/// The draft `folder` of the chat `session` of the plane `held`, with the chat's workspace.
fn draft_of(
    held: &crate::planes::Held,
    session: u32,
    folder: &str,
) -> Result<(drafts::Draft, Option<String>), String> {
    let chat = held
        .chats()
        .drafting()
        .into_iter()
        .find(|one| one.session == session)
        .ok_or_else(|| "that chat is not open here, so its theme draft is not either".to_owned())?;
    let config = crate::extensions::config_root()?;
    let folders = theme::Folders::of(Some(&config), Some(held.root()), chat.workspace.as_deref());
    Ok((
        drafts::Draft {
            kept_to: drafts::kept_to(&folders, chat.workspace.as_deref(), folder),
            chat: chat.id,
            folder: folder.to_owned(),
        },
        chat.workspace,
    ))
}

/// **Keep** pressed: the engine copies the draft of the chat `session` into the real folder it is
/// a draft of and deletes the draft. Answers what it did, in a sentence. Only a draft the window
/// draws is kept, and only as the person was shown it: `kept_to` (`workspace` or `project`, as
/// `WindowDraft::kept_to` says), `replaces`, which the window sends once the person agreed to
/// replace the folder there, and `changed`, once they agreed to replace a folder that is not what
/// the draft started from. When any has changed since, nothing is kept.
#[tauri::command]
#[specta::specta]
pub async fn theme_draft_keep(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
    folder: String,
    kept_to: String,
    replaces: bool,
    changed: bool,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let (draft, workspace) = draft_of(&held, session, &folder)?;
    let config = crate::extensions::config_root()?;
    // What the person was shown: Keep does that, or nothing.
    let seen = drafts::Seen {
        kept_to: match (kept_to.as_str(), workspace.as_deref()) {
            ("project", _) => drafts::KeptTo::Project,
            ("workspace", Some(name)) => drafts::KeptTo::Workspace(name.to_owned()),
            _ => return Err(format!("{kept_to} is not where Keep puts a theme draft")),
        },
        replaces,
        changed,
    };
    tauri::async_runtime::spawn_blocking(move || {
        let place = drafts::Place {
            plane: &root,
            workspace: workspace.as_deref(),
        };
        let kept = drafts::keep(Some(&config), place, &draft, &seen)?;
        forget(&root, &draft);
        Ok(match kept.kept_to {
            drafts::KeptTo::Workspace(name) => {
                format!("The theme {folder} is kept in the workspace {name}'s themes.")
            }
            drafts::KeptTo::Project => {
                format!("The theme {folder} is kept in the project's themes, for every workspace.")
            }
        })
    })
    .await
    .map_err(|err| format!("keeping the theme draft did not finish: {err}"))?
}

/// **Revert** pressed: the engine deletes the draft of the chat `session`.
#[tauri::command]
#[specta::specta]
pub async fn theme_draft_revert(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
    folder: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let (draft, _) = draft_of(&held, session, &folder)?;
    tauri::async_runtime::spawn_blocking(move || {
        drafts::revert(&root, &draft)?;
        forget(&root, &draft);
        Ok(())
    })
    .await
    .map_err(|err| format!("reverting the theme draft did not finish: {err}"))?
}

/// **The revert key**: every draft of the chats working at `workspace` (the plane root for
/// `None`) is deleted at once, so the window draws its kept theme again whatever the drafts
/// said. Answers how many were reverted.
#[tauri::command]
#[specta::specta]
pub async fn theme_drafts_revert_here(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
) -> Result<u32, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let chats = chats_at(&held, workspace.as_deref());
    let config = crate::extensions::config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut reverted = 0;
        for (draft, _) in drafts_of(&config, &root, workspace.as_deref(), &chats) {
            drafts::revert(&root, &draft)?;
            forget(&root, &draft);
            reverted += 1;
        }
        Ok(reverted)
    })
    .await
    .map_err(|err| format!("reverting the theme drafts did not finish: {err}"))?
}

/// **A chat's `purlis theme draft`** ([`purlis_core::hookwire::Ask::ThemeDraft`], #2332): the
/// draft is started, or its theme files saved, **in the drafts folder of the chat whose token
/// the line carries**, by this app's record of that chat and where it works; the line names no
/// chat. Answers what the window draws of it, as lines the command prints.
pub(crate) fn asked(
    held: &crate::planes::Held,
    ask: &purlis_core::hookwire::ThemeDraftAsk,
) -> purlis_core::hookwire::Answer {
    use purlis_core::hookwire::Answer;
    use purlis_core::repocmd::Say;
    let Some(chat) = held
        .chats()
        .drafting()
        .into_iter()
        .find(|one| one.session == ask.chat)
    else {
        return Answer::No {
            why: "this app has no record of this chat, so it keeps no theme draft for it"
                .to_owned(),
        };
    };
    // This machine's store as the app resolved it at startup: whose fonts the project approved.
    let config = held.config().map(Path::to_path_buf);
    let place = drafts::Place {
        plane: held.root(),
        workspace: chat.workspace.as_deref(),
    };
    let here = chat.workspace.as_deref().map_or_else(
        || "at the plane root".to_owned(),
        |name| format!("in the workspace {name}"),
    );
    let kept_to = |kept_to: &drafts::KeptTo| match kept_to {
        drafts::KeptTo::Workspace(name) => format!("the workspace {name}'s themes"),
        drafts::KeptTo::Project => "the project's themes, for every workspace".to_owned(),
    };
    let folder = &ask.folder;
    let Some(files) = &ask.files else {
        return match drafts::start(config.as_deref(), place, &chat.id, folder) {
            // Whether the draft was made now travels as its own field: the command makes its
            // working copy again from a draft made now.
            Ok(started) => {
                let said = format!(
                    "The theme draft {folder} is {}, drawn only {here}; Keep puts it in {}.",
                    if started.made { "started" } else { "there" },
                    kept_to(&started.kept_to)
                );
                Answer::Drafted {
                    lines: vec![if started.made {
                        Say::Done(said.into())
                    } else {
                        Say::Info(said.into())
                    }],
                    code: 0,
                    made: started.made,
                }
            }
            Err(why) => Answer::No { why },
        };
    };
    match drafts::save(config.as_deref(), place, &chat.id, folder, files) {
        Ok(saved) if !saved.broken.is_empty() => {
            let mut lines: Vec<Say> = saved
                .broken
                .iter()
                .take(8)
                .map(|one| Say::Warn(one.said().into()))
                .collect();
            lines.push(Say::Fail(format!(
                "The theme draft {folder} is saved, and not drawn: it breaks a hard floor, so the \
                 window keeps its last good version. Fix it and save again."
            ).into()));
            Answer::Drafted {
                lines,
                code: 1,
                made: saved.made,
            }
        }
        Ok(saved) if !saved.refused.is_empty() => {
            let mut lines: Vec<Say> = saved
                .refused
                .iter()
                .take(8)
                .map(|one| Say::Warn(one.said.clone().into()))
                .collect();
            lines.push(Say::Fail(format!(
                "The theme draft {folder} is saved, and not drawn: it is refused. Fix it and save \
                 again."
            ).into()));
            Answer::Drafted {
                lines,
                code: 1,
                made: saved.made,
            }
        }
        Ok(saved) => Answer::Drafted {
            lines: vec![Say::Done(format!(
                "The theme draft {folder} is saved and drawn {here}; Keep puts it in {}.",
                kept_to(&saved.kept_to)
            ).into())],
            code: 0,
            made: saved.made,
        },
        Err(why) => Answer::No { why },
    }
}

/// **The drafts of the chats no longer open in the plane at `plane` are deleted** (#2708): every
/// chat's drafts folder whose chat is none of `open`, the chats its record holds, which are the
/// ones open now and the ones its next launch reopens. Their last good versions go with them.
/// Asked whenever that record is written, which is whenever a chat opens or closes.
pub(crate) fn sweep(plane: &Path, open: &[&str]) {
    if drafts::sweep(plane, open) > 0 {
        last_good().retain(|(at, chat, _), _| at != plane || open.contains(&chat.as_str()));
    }
}

/// The last good version of `draft` is gone with it.
fn forget(plane: &Path, draft: &drafts::Draft) {
    last_good().remove(&(
        plane.to_path_buf(),
        draft.chat.clone(),
        draft.folder.clone(),
    ));
}

/// **Use in the whole project** pressed: the engine moves the workspace's theme folder `folder`
/// to the project's `themes/`, never over one there. Answers what it did.
#[tauri::command]
#[specta::specta]
pub async fn theme_move_to_project(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: String,
    folder: String,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        drafts::move_to_project(&root, &workspace, &folder).map(|_| {
            format!(
                "The theme {folder} is the project's now, in every workspace, and the project's \
                 next save commits it for everyone, even from a local workspace. Its font files \
                 are asked about again with the project."
            )
        })
    })
    .await
    .map_err(|err| format!("moving the theme did not finish: {err}"))?
}

/// **Copy to my machine** pressed: the engine copies the project's or a workspace's theme folder
/// `folder` to the person's own themes, never over one there, with only the font files approved
/// with the project. Answers what it did.
#[tauri::command]
#[specta::specta]
pub async fn theme_copy_to_machine(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
    folder: String,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let config = crate::extensions::config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        drafts::copy_to_machine(&config, &root, workspace.as_deref(), &folder).map(|copied| {
            let mut said = format!("The theme {folder} is one of your own themes now.");
            for one in copied.left_out {
                said.push(' ');
                said.push_str(&one);
                said.push('.');
            }
            said
        })
    })
    .await
    .map_err(|err| format!("copying the theme did not finish: {err}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(session: u32, workspace: Option<&str>) -> Drafting {
        Drafting {
            session,
            id: purlis_core::reopen::mint(),
            name: format!("chat {session}"),
            workspace: workspace.map(str::to_owned),
        }
    }

    #[test]
    fn a_chat_s_draft_is_drawn_where_it_works_and_its_last_good_version_stays() {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config");
        let plane = home.path().join("plane");
        std::fs::create_dir_all(plane.join(".purlis")).unwrap();
        std::fs::create_dir_all(plane.join("workspaces/alpha")).unwrap();
        std::fs::create_dir_all(&config).unwrap();
        let one = chat(3, Some("alpha"));
        let place = drafts::Place {
            plane: &plane,
            workspace: Some("alpha"),
        };
        let path = drafts::start(Some(&config), place, &one.id, "dusk")
            .unwrap()
            .path;
        let write = |tokens: serde_json::Value| {
            let theme = serde_json::json!({
                "name": "Dusk",
                "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
            });
            std::fs::write(path.join("theme.json"), theme.to_string()).unwrap();
        };
        let levels = theme::Levels::default();
        let chats = vec![one.clone()];
        let ask = |workspace: Option<&str>| {
            drafted_here(
                &config,
                &plane,
                workspace,
                &chats,
                &levels,
                false,
                theme::Contrast::Standard,
            )
        };
        write(serde_json::json!({ "accent.base": "#4477dd" }));
        let (drawn, draft) = ask(Some("alpha")).expect("a draft");
        assert!(draft.drawn && draft.broken.is_empty());
        assert_eq!((draft.session, draft.chat.as_str()), (3, "chat 3"));
        let accent = |drawn: Option<theme::Drawn>| {
            drawn
                .and_then(|one| one.theme)
                .map(|one| one.value("accent.base").to_owned())
        };
        assert_eq!(accent(drawn).as_deref(), Some("#4477dd"));
        write(serde_json::json!({ "accent.base": "#dd7744", "surface.sunken": "#f2f2f2" }));
        let (drawn, draft) = ask(Some("alpha")).expect("a draft");
        assert!(!draft.broken.is_empty());
        assert_eq!(
            accent(drawn).as_deref(),
            Some("#4477dd"),
            "the last good one"
        );
    }

    #[test]
    fn a_link_s_close_is_handed_to_the_window_only_for_chats_that_have_a_draft() {
        // #2761: each chat's drafts read where it works; a chat with none is not handed.
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config");
        let plane = home.path().join("plane");
        std::fs::create_dir_all(plane.join(".purlis")).unwrap();
        std::fs::create_dir_all(plane.join("workspaces/alpha")).unwrap();
        std::fs::create_dir_all(&config).unwrap();
        let (drafting, root, none) = (chat(3, Some("alpha")), chat(4, None), chat(5, None));
        for one in [&drafting, &root] {
            let place = drafts::Place {
                plane: &plane,
                workspace: one.workspace.as_deref(),
            };
            drafts::start(Some(&config), place, &one.id, "dusk").unwrap();
        }
        assert_eq!(
            drafted_among(&plane, &[drafting, root, none.clone()]),
            [3, 4]
        );
        assert_eq!(drafted_among(&plane, &[none]), Vec::<u32>::new());
    }

    #[test]
    fn a_link_s_list_of_chats_is_read_each_once_and_capped() {
        assert_eq!(asked_sessions(vec![4, 3, 4, 3]), [3, 4]);
        let long: Vec<u32> = (0..10_000).rev().collect();
        assert_eq!(asked_sessions(long).len(), drafts::MOST_CHATS);
    }

    #[test]
    fn a_draft_refused_for_a_font_the_project_does_not_bring_offers_no_review() {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config");
        let plane = home.path().join("plane");
        std::fs::create_dir_all(plane.join(".purlis")).unwrap();
        std::fs::create_dir_all(plane.join("workspaces/alpha")).unwrap();
        std::fs::create_dir_all(&config).unwrap();
        let one = chat(4, Some("alpha"));
        let place = drafts::Place {
            plane: &plane,
            workspace: Some("alpha"),
        };
        let path = drafts::start(Some(&config), place, &one.id, "dusk")
            .unwrap()
            .path;
        // A face only the draft has: the project brings no such file, so its trust ask is no
        // way to draw it.
        std::fs::create_dir_all(path.join("fonts")).unwrap();
        let mut woff2 = b"wOF2".to_vec();
        woff2.extend_from_slice(&[0; 64]);
        std::fs::write(path.join("fonts/face.woff2"), woff2).unwrap();
        let theme = serde_json::json!({
            "name": "Dusk",
            "appearances": [{ "id": "dark", "appearance": "dark" }],
            "fonts": { "ui": { "family": "Face", "faces": [
                { "file": "fonts/face.woff2", "weight": 400 }
            ] } }
        });
        std::fs::write(path.join("theme.json"), theme.to_string()).unwrap();
        let (drawn, draft) = drafted_here(
            &config,
            &plane,
            Some("alpha"),
            &[one],
            &theme::Levels::default(),
            false,
            theme::Contrast::Standard,
        )
        .expect("a draft");
        assert!(drawn.is_none());
        assert!(!draft.refused.is_empty(), "{draft:?}");
        assert_eq!(draft.review, None);
    }
}
