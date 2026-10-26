// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Attach browser proof** (#2026, spec #2008 story 57): screenshots the person picks in a
//! chat's browser timeline, attached to a pull or merge request with the person's own purlis
//! sign-in, after the person has seen exactly what goes. The core's
//! [`purlis_core::browserproof`] decides what is shown and refuses what changed since; this
//! is the window's three commands over it, each the window's alone (`ui::WINDOW_ONLY`).
//!
//! Only while the chat is open: a chat's timeline goes with it, and a chat this app does not
//! have open is refused here before anything is read.

use purlis_core::browserproof::{self, ShownShot};
use purlis_core::forge::{Caller, Forge, ForgeBackend};

/// A pull or merge request the window offers to attach proof to: the open request of a save of
/// one of the chat's workspace's repos.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProofRequest {
    /// Its page.
    pub url: String,
    /// The repo's name in the workspace.
    pub repo: String,
    /// What its forge calls it: `pull request` or `merge request`.
    pub request: String,
}

/// One screenshot as the person was shown it, handed back to attach.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ProofShot {
    /// Its number in the chat's timeline, to read with `chat_browser_timeline_shot`.
    pub shot: u32,
    /// When it was taken, in seconds since 1970.
    pub at: u32,
    /// SHA-256 of its bytes, lower-case hex.
    pub digest: String,
}

/// **What attaching would send**, shown to the person before anything goes, and handed back
/// unchanged to [`attach_browser_proof`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ShownProof {
    /// The request's page.
    pub request: String,
    /// `#12` or `!12`.
    pub number: String,
    /// The repo it is in, `owner/name`.
    pub path: String,
    /// Where the pictures are kept on the forge, and who can read them there.
    pub kept_at: String,
    /// The comment's words, before the pictures.
    pub words: String,
    /// The pictures, in the order they go.
    pub shots: Vec<ProofShot>,
}

impl From<browserproof::Shown> for ShownProof {
    fn from(shown: browserproof::Shown) -> Self {
        ShownProof {
            request: shown.request,
            number: shown.number,
            path: shown.path,
            kept_at: shown.kept_at,
            words: shown.words,
            shots: shown
                .shots
                .into_iter()
                .map(|one| ProofShot {
                    shot: u32::try_from(one.shot).unwrap_or(u32::MAX),
                    at: u32::try_from(one.at).unwrap_or(u32::MAX),
                    digest: one.digest,
                })
                .collect(),
        }
    }
}

impl From<ShownProof> for browserproof::Shown {
    fn from(shown: ShownProof) -> Self {
        browserproof::Shown {
            request: shown.request,
            number: shown.number,
            path: shown.path,
            kept_at: shown.kept_at,
            words: shown.words,
            shots: shown
                .shots
                .into_iter()
                .map(|one| ShownShot {
                    shot: u64::from(one.shot),
                    at: u64::from(one.at),
                    digest: one.digest,
                })
                .collect(),
        }
    }
}

/// The chat whose id is `chat`, refused unless this app has it open.
fn open_chat(held: &crate::planes::Held, chat: &str) -> Result<(), String> {
    if held.chats().id_is_open(chat) {
        Ok(())
    } else {
        Err("That chat is closed, and its screenshots went with it.".to_owned())
    }
}

/// **The requests the window offers for chat `chat`'s proof**: the open request of each save of
/// a repo of the chat's workspace. Read from git and the save journal, never the network. None
/// for a chat at the project's root. The person can name any other request by its address.
#[tauri::command]
#[specta::specta]
pub async fn browser_proof_requests(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    chat: String,
) -> Result<Vec<ProofRequest>, String> {
    let held = planes.held(&plane)?;
    open_chat(&held, &chat)?;
    let workspace = held
        .chats()
        .drafting()
        .into_iter()
        .find(|one| one.id == chat)
        .and_then(|one| one.workspace);
    let Some(workspace) = workspace else {
        return Ok(Vec::new());
    };
    let root = held.root().to_path_buf();
    let repos = tauri::async_runtime::spawn_blocking(move || {
        crate::saving::repos_saving(&root, &workspace)
    })
    .await
    .map_err(|err| format!("reading the repos' requests did not finish: {err}"))??;
    Ok(repos
        .into_iter()
        .filter(|repo| repo.stage == "pr-open")
        .filter_map(|repo| {
            Some(ProofRequest {
                url: repo.pr.filter(|url| url.starts_with("https://"))?,
                repo: repo.name,
                request: repo.request,
            })
        })
        .collect())
}

/// **What attaching screenshots `shots` of chat `chat` to the request at `request` would
/// send**, for the person to see first. Asks no forge.
#[tauri::command]
#[specta::specta]
pub async fn show_browser_proof(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    chat: String,
    request: String,
    shots: Vec<u32>,
) -> Result<ShownProof, String> {
    let held = planes.held(&plane)?;
    open_chat(&held, &chat)?;
    let picked: Vec<u64> = shots.into_iter().map(u64::from).collect();
    browserproof::show(held.root(), &chat, &request, &picked).map(ShownProof::from)
}

/// **Attach what the person was shown**, with the person's own purlis sign-in to the request's
/// forge and never through `gh` or `glab`: the press of Attach in the proof the window showed.
/// Answers the comment's page.
///
/// On a blocking thread: it asks the forge several times.
#[tauri::command]
#[specta::specta]
pub async fn attach_browser_proof(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    chat: String,
    shown: ShownProof,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    open_chat(&held, &chat)?;
    let root = held.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let signed_in = |forge: &Forge| -> Result<Box<dyn ForgeBackend>, String> {
            purlis_core::forge::signin::signed_in_backend(
                forge,
                &Caller::window(),
                purlis_core::netlog::Feature::Forge,
            )
            .map_err(|why| why.said().to_owned())
        };
        browserproof::attach(&root, &chat, &shown.into(), &signed_in, &Caller::window())
            .map(|attached| attached.comment)
    })
    .await
    .map_err(|err| format!("attaching the proof did not finish: {err}"))?
}
