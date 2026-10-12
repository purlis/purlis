// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where the person is in the guided steps** (#1817, spec #1810): a file beside
//! `machine.json`, so a quit in the middle of setting up resumes at the same step on the next
//! launch.
//!
//! The steps are the window's: welcome, sign in (when the act needs it), create, get or open a
//! project, the git check (when git is missing), pick your harness, and open the project. What
//! is kept is only which step, which door, the project the steps made (its root, and where its
//! first chat starts) and whether this machine has been through them once. **Nothing secret is
//! kept here, ever**: a forge token lives in the keyring (ADR 0077 §7), and a sign-in step that
//! is resumed asks again rather than finding anything here.
//!
//! **Tier: Machine, device-bound** (ADR 0069): it names absolute paths on this machine, and a
//! step half-done on one Mac means nothing on another. It is in purlis's machine directory
//! (`<config>/onboarding.json`) and not the Tauri app data directory, because a chat's sandbox
//! denies the config home whole, the scenario tests give every run a config home of its own,
//! and [`crate::machine::read_beside`] and [`crate::machine::write_beside`] already contain a
//! read and a write there (no link, a plain file, a size bound, written whole at `0600`).
//!
//! **A convenience, never a gate.** A file that is missing, unreadable, too big, not JSON, of
//! another version, or naming a step this purlis does not know reads as "no progress": the
//! window starts at the welcome, which is where a person with nothing kept starts anyway. One
//! that names a project is still only a path: the window opens it through the trust gate
//! (ADR 0035) like any other, so a file somebody else wrote here can at most suggest a path.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The file, inside [`crate::machine::DIR`].
pub const FILE: &str = "onboarding.json";

/// The format's version. Another one reads as no progress (a convenience, see the module doc).
pub const VERSION: u32 = 1;

/// The most a progress file may hold: a few paths and words. A larger one is not read.
pub const MAX_BYTES: u64 = 16 * 1024;

/// The longest path or name kept: anything longer is no progress.
const MAX_FIELD: usize = 4096;

/// A step of the guided flow, as the window names it. `open` is not one: opening the project
/// ends the steps, and what is kept then is "done" ([`Progress::finished_once`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    /// The welcome and its doors.
    Welcome,
    /// Sign in to GitHub or GitLab: skippable.
    SignIn,
    /// Create, get or open a project, behind the door that was pressed.
    Project,
    /// Git is missing, and the act that needs it waits.
    Git,
    /// Pick your harness, with the project the steps made held behind it.
    Harness,
}

/// The welcome's three doors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Door {
    Create,
    Get,
    Open,
}

/// The project the steps made or opened, which the harness step holds behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Made {
    /// The project's root, as the registry resolved it: absolute.
    pub plane: String,
    /// Where its first chat starts: the repo's clone for a project made from a repo, else the
    /// root.
    pub cwd: String,
    /// The workspace strip an installer's shell is filed on.
    pub workspace: String,
}

/// Where the person is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub step: Step,
    /// The door pressed, while there is one.
    pub door: Option<Door>,
    /// Whether this run of the steps took the sign-in, so it is counted among them.
    pub signing_in: bool,
    /// Whether this run found git missing, so the git check is counted among them.
    pub git_missing: bool,
    /// The project the steps made, from the harness step on.
    pub made: Option<Made>,
    /// Whether this machine has been through the steps to an open project once. After that,
    /// the harness step is shown only while no harness is installed.
    pub finished_once: bool,
}

/// The file as it is on disk: the progress and its version.
#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    #[serde(flatten)]
    progress: Progress,
}

impl Progress {
    /// Whether this progress is one the window can resume at: every path absolute and bounded,
    /// and a harness step with the project it holds.
    fn is_sound(&self) -> bool {
        match &self.made {
            None => self.step != Step::Harness,
            Some(made) => {
                Path::new(&made.plane).is_absolute()
                    && (made.cwd.is_empty() || Path::new(&made.cwd).is_absolute())
                    && [&made.plane, &made.cwd, &made.workspace]
                        .iter()
                        .all(|field| field.len() <= MAX_FIELD && !field.contains('\0'))
            }
        }
    }
}

/// What the machine keeps of the steps, or `None` for no progress: no file, or one this purlis
/// will not resume at (see the module doc). Never an error: a launch is not held up by it.
pub fn read(config_root: &Path) -> Option<Progress> {
    let text = crate::machine::read_beside(config_root, FILE, MAX_BYTES, "the steps' progress")
        .ok()
        .flatten()?;
    let on_disk: OnDisk = serde_json::from_str(&text).ok()?;
    (on_disk.version == VERSION && on_disk.progress.is_sound()).then_some(on_disk.progress)
}

/// Keeps `progress`, replacing what the file held: whole, `0600`, never through a link.
///
/// A progress the window could not resume at is refused rather than written, so the file only
/// ever holds what [`read`] gives back.
pub fn write(config_root: &Path, progress: &Progress) -> io::Result<()> {
    if !progress.is_sound() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the window sent steps' progress purlis could not resume at",
        ));
    }
    crate::machine::supported()?;
    let dir = crate::machine::private_dir(config_root)?;
    let text = serde_json::to_string_pretty(&OnDisk {
        version: VERSION,
        progress: progress.clone(),
    })
    .expect("progress is plain data serde can always write");
    crate::machine::write_beside(config_root, &dir.join(FILE), (text + "\n").as_bytes())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn at(step: Step) -> Progress {
        Progress {
            step,
            door: None,
            signing_in: false,
            git_missing: false,
            made: None,
            finished_once: false,
        }
    }

    fn made() -> Made {
        Made {
            plane: "/Users/someone/Projects/team".to_owned(),
            cwd: "/Users/someone/Projects/team/workspaces/app/app".to_owned(),
            workspace: "app".to_owned(),
        }
    }

    #[test]
    fn a_machine_that_kept_nothing_has_no_progress() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(read(home.path()), None);
    }

    #[test]
    fn each_step_kept_is_the_step_read_back() {
        let home = tempfile::tempdir().unwrap();
        for step in [Step::Welcome, Step::SignIn, Step::Project, Step::Git] {
            let kept = Progress {
                door: Some(Door::Get),
                signing_in: true,
                ..at(step)
            };
            write(home.path(), &kept).unwrap();
            assert_eq!(read(home.path()), Some(kept));
        }
        let holding = Progress {
            made: Some(made()),
            finished_once: true,
            git_missing: true,
            ..at(Step::Harness)
        };
        write(home.path(), &holding).unwrap();
        assert_eq!(read(home.path()), Some(holding));
    }

    #[test]
    fn the_file_is_private_to_the_operator_and_says_its_version() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        write(home.path(), &at(Step::Project)).unwrap();
        let file = crate::machine::dir(home.path()).join(FILE);
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let text = std::fs::read_to_string(&file).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["step"], "project");
    }

    #[test]
    fn a_file_purlis_cannot_resume_at_is_no_progress() {
        let home = tempfile::tempdir().unwrap();
        write(home.path(), &at(Step::Welcome)).unwrap();
        let file = crate::machine::dir(home.path()).join(FILE);
        let unsound = [
            "not json".to_owned(),
            r#"{"version":2,"step":"project","door":null,"signing_in":false,"git_missing":false,"made":null,"finished_once":false}"#.to_owned(),
            r#"{"version":1,"step":"somewhere","door":null,"signing_in":false,"git_missing":false,"made":null,"finished_once":false}"#.to_owned(),
            // A harness step with no project to hold.
            r#"{"version":1,"step":"harness","door":null,"signing_in":false,"git_missing":false,"made":null,"finished_once":false}"#.to_owned(),
            // A project that is not an absolute path.
            r#"{"version":1,"step":"harness","door":null,"signing_in":false,"git_missing":false,"made":{"plane":"team","cwd":"","workspace":""},"finished_once":false}"#.to_owned(),
            "x".repeat(usize::try_from(MAX_BYTES).unwrap() + 1),
        ];
        for text in unsound {
            std::fs::write(&file, &text).unwrap();
            assert_eq!(read(home.path()), None, "{}", &text[..text.len().min(80)]);
        }
    }

    #[test]
    fn a_link_where_the_file_goes_is_never_followed() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let target = elsewhere.path().join("planted.json");
        write(home.path(), &at(Step::Welcome)).unwrap();
        let file = crate::machine::dir(home.path()).join(FILE);
        std::fs::remove_file(&file).unwrap();
        std::os::unix::fs::symlink(&target, &file).unwrap();
        // Refused or written in the link's place: either way, never through it.
        let _ = write(home.path(), &at(Step::Project));
        assert!(!target.exists(), "a write went through a link");
        std::fs::write(&target, r#"{"version":1,"step":"git","door":null,"signing_in":false,"git_missing":true,"made":null,"finished_once":false}"#).unwrap();
        if std::fs::symlink_metadata(&file)
            .unwrap()
            .file_type()
            .is_symlink()
        {
            assert_eq!(read(home.path()), None, "a read went through a link");
        }
    }

    #[test]
    fn progress_it_could_not_resume_at_is_never_written() {
        let home = tempfile::tempdir().unwrap();
        let refused = write(home.path(), &at(Step::Harness)).unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(read(home.path()), None);
    }
}
