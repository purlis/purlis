// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The sealed answers, for the window** (M75 slice 07a, #7015, part 2; spec §8.10): the state
//! the open project's answers are in, the nevers imported from the old files, and the device id
//! the person confirms. The window draws them; slice 07b refuses on the state.
//!
//! - [`answers_shown`] registers the project at its first open, **imports the person's old
//!   nevers once** (Q320), and reads where the answers stand. It is the window's alone.
//! - [`confirm_device_id`] is **the person's press**: the one place this app makes an
//!   [`answers::Press`]. It names the id the window showed, so an id that changed since is never
//!   confirmed unseen. It is the window's alone, never a link's: a chat's request is not the
//!   person's press.

use purlis_core::secrets::answers::{self, Device, Import, State};

use crate::planes::PlaneId;

/// The device id, as Settings shows it: a display label only, with no rename.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct DeviceShown {
    /// `confirmed`, `unconfirmed` (sealed, waiting for the person) or `unset` (no keyring, a
    /// registry that does not read, or no project opened yet).
    pub state: String,
    /// The id, where one is sealed.
    pub id: Option<String>,
}

/// Where the open project's sealed answers stand.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct AnswersShown {
    /// `fresh`, `holds`, `no-keyring`, `unverifiable` or `deny`.
    pub state: String,
    /// Why the answers are in the deny state, one sentence each; empty otherwise.
    pub deny: Vec<String>,
    /// The nevers imported from the old files, each as `asking -> target`, while those files
    /// are still there to delete.
    pub imported: Vec<String>,
    /// The old files that are safe to delete.
    pub old_files: Vec<String>,
    /// What the import could not do, said for the person; none when it went through or had
    /// nothing to do.
    pub import_failed: Option<String>,
    /// What purlis ignored or cannot do, one sentence each.
    pub notices: Vec<String>,
    /// The device id.
    pub device: DeviceShown,
}

fn state_word(state: &State) -> &'static str {
    match state {
        State::Fresh => "fresh",
        State::Holds => "holds",
        State::NoKeyring => "no-keyring",
        State::Unverifiable => "unverifiable",
        State::Deny(_) => "deny",
    }
}

fn device_shown() -> DeviceShown {
    match answers::device() {
        Device::Confirmed(id) => DeviceShown {
            state: "confirmed".to_owned(),
            id: Some(id),
        },
        Device::Unconfirmed(id) => DeviceShown {
            state: "unconfirmed".to_owned(),
            id: Some(id),
        },
        Device::None => DeviceShown {
            state: "unset".to_owned(),
            id: None,
        },
    }
}

/// Everything [`answers_shown`] reads, for the project at `root`, after registering it and
/// importing its old nevers.
fn read(root: &std::path::Path) -> AnswersShown {
    // First open: registered before anything is sealed, so a process inside is never plane-less.
    let _ = answers::register(root);
    let import_failed = match answers::import_nevers(root) {
        Ok(Import::Nothing | Import::Already | Import::NotKept | Import::Imported(_)) => None,
        Err(why) => Some(why.to_string()),
    };
    let standing = answers::standing(root);
    let old_files = answers::old_files(root);
    let imported = if old_files.is_empty() || answers::imported_nevers(root) != Some(true) {
        Vec::new()
    } else {
        standing
            .entries
            .iter()
            .filter(|entry| entry.kind == answers::NEVER)
            .map(|entry| entry.text.clone())
            .collect()
    };
    AnswersShown {
        state: state_word(&standing.state).to_owned(),
        deny: standing.causes().iter().map(answers::Deny::said).collect(),
        imported,
        old_files,
        import_failed,
        notices: standing.notices.clone(),
        device: device_shown(),
    }
}

/// **Where the open project's sealed answers stand**, registering it and importing its old
/// nevers on the way (once). The window's alone.
#[tauri::command]
#[specta::specta]
pub(crate) async fn answers_shown(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
) -> Result<AnswersShown, String> {
    planes.held(&plane)?;
    let root = plane.root().to_path_buf();
    crate::off_the_window("reading the sealed answers", move || Ok(read(&root))).await
}

/// **The person confirms the device id the window showed.** Made here and nowhere else: the
/// press is the person's in their window, never a chat's, so this command is the window's alone
/// (`session_protocol::ui::WINDOW_ONLY`). Refused where the id is not the one shown.
#[tauri::command]
#[specta::specta]
pub(crate) async fn confirm_device_id(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    id: String,
) -> Result<DeviceShown, String> {
    planes.held(&plane)?;
    crate::off_the_window("confirming the device id", move || {
        answers::confirm_device(&answers::Press::of_the_persons_press(), &id)?;
        Ok(device_shown())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_device_is_said_in_one_of_three_words() {
        // Outside the app there is no registry to read: unset, never a guess.
        let shown = device_shown();
        assert!(["confirmed", "unconfirmed", "unset"].contains(&shown.state.as_str()));
        assert_eq!(shown.id.is_some(), shown.state != "unset");
    }

    #[test]
    fn every_state_has_its_own_word() {
        let words = [
            state_word(&State::Fresh),
            state_word(&State::Holds),
            state_word(&State::NoKeyring),
            state_word(&State::Unverifiable),
            state_word(&State::Deny(Vec::new())),
        ];
        let mut unique = words.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), words.len());
    }
}
