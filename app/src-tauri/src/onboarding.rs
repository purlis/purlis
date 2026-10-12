// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The guided steps' place, kept for the next launch (#1817): the window's two commands over
//! `purlis_core::onboarding`, which holds the file, its tier and the reasons.
//!
//! **The window's alone.** Both are on `WINDOW_ONLY`: where a person is in setting up this
//! machine is nothing a chat or a link asks about or writes.

use purlis_core::onboarding::{self, Door, Made, Progress, Step};

/// A step of the guided flow (`purlis_core::onboarding::Step`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum OnboardingStep {
    Welcome,
    SignIn,
    Project,
    Git,
    Harness,
}

/// The welcome's three doors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum OnboardingDoor {
    Create,
    Get,
    Open,
}

/// The project the steps made, which the harness step holds behind it: its root, where its
/// first chat starts, and the workspace strip an installer's shell is filed on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct OnboardingMade {
    pub plane: String,
    pub cwd: String,
    pub workspace: String,
}

/// Where the person is in the guided steps (`purlis_core::onboarding::Progress`). No secret is
/// ever part of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct OnboardingProgress {
    pub step: OnboardingStep,
    pub door: Option<OnboardingDoor>,
    pub signing_in: bool,
    pub git_missing: bool,
    pub made: Option<OnboardingMade>,
    pub finished_once: bool,
}

impl From<Progress> for OnboardingProgress {
    fn from(kept: Progress) -> Self {
        Self {
            step: match kept.step {
                Step::Welcome => OnboardingStep::Welcome,
                Step::SignIn => OnboardingStep::SignIn,
                Step::Project => OnboardingStep::Project,
                Step::Git => OnboardingStep::Git,
                Step::Harness => OnboardingStep::Harness,
            },
            door: kept.door.map(|door| match door {
                Door::Create => OnboardingDoor::Create,
                Door::Get => OnboardingDoor::Get,
                Door::Open => OnboardingDoor::Open,
            }),
            signing_in: kept.signing_in,
            git_missing: kept.git_missing,
            made: kept.made.map(|made| OnboardingMade {
                plane: made.plane,
                cwd: made.cwd,
                workspace: made.workspace,
            }),
            finished_once: kept.finished_once,
        }
    }
}

impl From<OnboardingProgress> for Progress {
    fn from(sent: OnboardingProgress) -> Self {
        Self {
            step: match sent.step {
                OnboardingStep::Welcome => Step::Welcome,
                OnboardingStep::SignIn => Step::SignIn,
                OnboardingStep::Project => Step::Project,
                OnboardingStep::Git => Step::Git,
                OnboardingStep::Harness => Step::Harness,
            },
            door: sent.door.map(|door| match door {
                OnboardingDoor::Create => Door::Create,
                OnboardingDoor::Get => Door::Get,
                OnboardingDoor::Open => Door::Open,
            }),
            signing_in: sent.signing_in,
            git_missing: sent.git_missing,
            made: sent.made.map(|made| Made {
                plane: made.plane,
                cwd: made.cwd,
                workspace: made.workspace,
            }),
            finished_once: sent.finished_once,
        }
    }
}

/// Where the person was in the guided steps when purlis last kept it, or nothing: no file, a
/// machine with no config home, or a file purlis will not resume at. The window then starts at
/// the welcome. On a blocking thread.
#[tauri::command]
#[specta::specta]
pub async fn onboarding_progress() -> Result<Option<OnboardingProgress>, String> {
    let Some(root) = purlis_core::machine::config_root() else {
        return Ok(None);
    };
    tauri::async_runtime::spawn_blocking(move || onboarding::read(&root).map(Into::into))
        .await
        .map_err(|err| format!("reading where the steps were did not finish: {err}"))
}

/// Keeps where the person is in the guided steps, for the next launch. The window's alone.
#[tauri::command]
#[specta::specta]
pub async fn keep_onboarding_progress(progress: OnboardingProgress) -> Result<(), String> {
    let root = purlis_core::machine::config_root().ok_or_else(|| {
        "this machine has no config home, so purlis cannot keep your place in the steps".to_owned()
    })?;
    tauri::async_runtime::spawn_blocking(move || onboarding::write(&root, &progress.into()))
        .await
        .map_err(|err| format!("keeping your place in the steps did not finish: {err}"))?
        .map_err(|err| format!("purlis could not keep your place in the steps: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window's words for the progress are the core's, both ways: what is kept is what is
    /// read back.
    #[test]
    fn the_windows_progress_is_the_cores_both_ways() {
        let sent = OnboardingProgress {
            step: OnboardingStep::Harness,
            door: Some(OnboardingDoor::Create),
            signing_in: true,
            git_missing: true,
            made: Some(OnboardingMade {
                plane: "/p".to_owned(),
                cwd: "/p/workspaces/app/app".to_owned(),
                workspace: "app".to_owned(),
            }),
            finished_once: false,
        };
        let core: Progress = sent.clone().into();
        assert_eq!(OnboardingProgress::from(core.clone()), sent);
        // The same spelling on the wire as on disk, so the two never drift.
        assert_eq!(
            serde_json::to_value(&sent).unwrap(),
            serde_json::to_value(&core).unwrap()
        );
    }
}
