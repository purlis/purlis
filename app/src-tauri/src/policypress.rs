// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The project's `[policy]` in the window** (rulings Q308, Q308a, Q308b; ADR 0125): what waits
//! for the person's press, what the file says that purlis could not read, and the press itself.
//!
//! A tightening of `[policy]` applies at once; **a loosening holds only once the person presses
//! Accept** here ([`accept_project_policy`], the window's alone), which the app seals on this
//! machine ([`purlis_core::enforcement::press`]). A chat can never accept it.

use purlis_core::enforcement;

use crate::planes::{PlaneId, Planes};

/// One family the project now runs looser than you accepted.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyWaiting {
    /// The family's key, as `[policy]` spells it.
    pub family: String,
    /// What you accepted: `off`, `log` or `enforce`.
    pub accepted: String,
    /// What the project's file says now.
    pub committed: String,
}

/// What the window says about the project's `[policy]` on this machine.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyState {
    /// Each family that waits for your press to loosen.
    pub waiting: Vec<PolicyWaiting>,
    /// Each value of `[policy]` purlis could not read, in one sentence, enforced meanwhile.
    pub refused: Vec<String>,
    /// Whether this machine keeps an acceptance whose seal does not hold, which counts as none.
    pub unsealed: bool,
    /// The digest of the committed text this was read from: what Accept names, so a press on a
    /// file that changed since it was shown accepts nothing.
    pub digest: String,
}

fn state_of(root: &std::path::Path) -> PolicyState {
    let standing = enforcement::standing(root);
    PolicyState {
        waiting: standing
            .waiting
            .into_iter()
            .map(|(family, accepted, committed)| PolicyWaiting {
                family: family.key().to_owned(),
                accepted: accepted.word().to_owned(),
                committed: committed.word().to_owned(),
            })
            .collect(),
        refused: standing.refused,
        unsealed: standing.unsealed,
        digest: standing.digest,
    }
}

/// What the commands here name their work, off the window's thread.
const READING_POLICY: &str = "reading the project's policy";

/// **The project's `[policy]` as it stands for you**: what waits for your press, and what the
/// file says that purlis could not read. Off the window's thread: it reads the project's files,
/// the administrator's policy and this machine's acceptance.
#[tauri::command]
#[specta::specta]
pub async fn project_policy_state(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<PolicyState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(READING_POLICY, move || Ok(state_of(&root))).await
}

/// **Your press: accept the project's `[policy]` as its committed file said it when it was
/// shown** (`shown`, the state's digest), looser where it is looser, at once. Sealed on this
/// machine by the app; refused where it cannot be sealed, or where the file changed since it was
/// shown. The window's alone.
#[tauri::command]
#[specta::specta]
pub async fn accept_project_policy(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: String,
) -> Result<PolicyState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(READING_POLICY, move || {
        enforcement::press(&root, &shown).map_err(|why| {
            format!("purlis could not keep your acceptance on this machine: {why}")
        })?;
        Ok(state_of(&root))
    })
    .await
}
