// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Is there a git to run, asked before purlis ever runs one (#1813, spec #1810 Q5).
//!
//! # Why this is asked first, and on macOS asked of `xcode-select`
//!
//! On a Mac without Apple's Command Line Tools, `/usr/bin/git` is still there: it is a stub
//! that, when run, opens the system's "install the command line developer tools" dialog and
//! fails. The runner looks in `/usr/bin` first ([`super::GIT_DIRS`]), so on such a Mac the
//! first git purlis ran used to be that stub, and the person met a system dialog nobody had
//! explained. `xcode-select -p` answers the same question without that side effect: it prints
//! the developer directory and exits 0 when the tools are there, and exits non-zero when they
//! are not. VS Code asks it before its first git for the same reason.
//!
//! Elsewhere there is no stub, so the question is the runner's own lookup: the fixed
//! directories, then the absolute entries of `PATH` ([`super::git_binary`]).
//!
//! # One gate
//!
//! [`require`] is called by the one function that spawns git ([`super::spawn_with`]), so every
//! verb, the doctor's `git` row included, is held to it. A missing git is a
//! [`super::GitUnavailable`] whose [`super::GitUnavailable::missing`] says how to install it,
//! and no process is started. The window asks [`check`] before an action that needs git, and
//! draws its guided screen from the [`Install`] it gets.
//!
//! # Cost
//!
//! A git found once is remembered for the life of the process: git does not uninstall itself
//! between two calls, and the runner is called many times a second. A git found missing is
//! asked again every time, so that Check again, after an install, sees it.

use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::sandbox::Os;
use crate::sandbox::backend::{self, Missing as SandboxMissing};

/// Where macOS keeps the tool that says whether its developer tools are installed.
const XCODE_SELECT: &str = "/usr/bin/xcode-select";

/// How long `xcode-select -p` is given. It reads one setting and answers at once; a wait this
/// long means something is wrong, and is taken as "not there" (fail closed: the guided screen,
/// never the system's stub).
const XCODE_SELECT_WAIT: Duration = Duration::from_secs(10);

/// Where to get git when no package manager is known.
pub const DOWNLOAD_PAGE: &str = "https://git-scm.com/downloads";

/// Whether system git can be run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presence {
    Present,
    Missing(Install),
}

/// How a missing git is installed on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Install {
    /// macOS: Apple's Command Line Tools, which [`start_install`] opens Apple's own installer
    /// for (`xcode-select --install`).
    DeveloperTools,
    /// Linux: the distribution's package manager, as `/etc/os-release` names it. Shown, never
    /// run: it needs `sudo`.
    Package { command: String },
    /// Anywhere else, or a distribution purlis has no package manager for: the download page.
    Download { page: &'static str },
}

impl std::fmt::Display for Install {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeveloperTools => write!(
                f,
                "git is not installed on this Mac: install Apple's command line developer tools \
                 (xcode-select --install)"
            ),
            Self::Package { command } => {
                write!(f, "git is not installed: install it with {command}")
            }
            Self::Download { page } => write!(f, "git is not installed: get it from {page}"),
        }
    }
}

/// The error a missing git is, inside [`super::GitUnavailable`]: what to install.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct GitMissing(pub Install);

impl super::GitUnavailable {
    /// How to install git, when this is git missing rather than git failing to start.
    pub fn missing(&self) -> Option<&Install> {
        self.0
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<GitMissing>())
            .map(|GitMissing(install)| install)
    }
}

/// What [`check_with`] looks at, so a test can hand it a stand-in for each.
#[derive(Debug, Clone, Copy)]
pub struct Probe<'a> {
    pub os: Os,
    /// The program asked on macOS (`/usr/bin/xcode-select` for real).
    pub xcode_select: &'a Path,
    /// The runner's fixed directories, searched first.
    pub fixed: &'a [&'a str],
    /// The inherited `PATH`, searched after them.
    pub path: Option<&'a OsStr>,
    /// The text of `/etc/os-release`, for a Linux hint. Empty elsewhere.
    pub os_release: &'a str,
}

/// Whether git is there, as `probe` sees it. Runs no git.
pub fn check_with(probe: &Probe<'_>) -> Presence {
    let present = match probe.os {
        Os::MacOs => developer_tools_installed(probe.xcode_select),
        _ => found_on_disk(probe.fixed, probe.path),
    };
    if present {
        Presence::Present
    } else {
        Presence::Missing(install_for(probe.os, probe.os_release))
    }
}

/// How git is installed on `os`.
fn install_for(os: Os, os_release: &str) -> Install {
    match os {
        Os::MacOs => Install::DeveloperTools,
        Os::Linux => backend::install_command(
            &SandboxMissing::Programs {
                os: Os::Linux,
                absent: vec!["git"],
            },
            os_release,
        )
        .map_or(
            Install::Download {
                page: DOWNLOAD_PAGE,
            },
            |command| Install::Package { command },
        ),
        Os::Windows | Os::Other => Install::Download {
            page: DOWNLOAD_PAGE,
        },
    }
}

/// The runner's own lookup found a git (anything but its bare-name fallback), or, for
/// Windows, a `git.exe` on `PATH`: that lookup names `git` alone, and the runner's bare-name
/// fallback is what finds `git.exe` there.
fn found_on_disk(fixed: &[&str], path: Option<&OsStr>) -> bool {
    super::git_binary_in(fixed, path).0.is_absolute()
        || path
            .into_iter()
            .flat_map(crate::programs::searchable)
            .any(|dir| dir.join("git.exe").is_file())
}

/// `xcode-select -p` exited 0, within [`XCODE_SELECT_WAIT`]. Anything else — not there, not
/// startable, a non-zero exit, no answer — is "not installed".
fn developer_tools_installed(xcode_select: &Path) -> bool {
    let mut command = Command::new(xcode_select);
    command
        .arg("-p")
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let Ok(mut child) = crate::forklock::spawn(&mut command) else {
        return false;
    };
    let deadline = Instant::now() + XCODE_SELECT_WAIT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// A git seen once in this process.
static SEEN: AtomicBool = AtomicBool::new(false);

/// Whether git is there on this machine: [`check_with`] on the real `xcode-select`, the
/// runner's directories and this process's `PATH`. Remembered once present.
pub fn check() -> Presence {
    #[cfg(test)]
    if let Some(forced) = forced::get() {
        return forced;
    }
    if SEEN.load(Ordering::Relaxed) {
        return Presence::Present;
    }
    let os = Os::this();
    let os_release = if os == Os::Linux {
        std::fs::read_to_string("/etc/os-release").unwrap_or_default()
    } else {
        String::new()
    };
    let path = std::env::var_os("PATH");
    let presence = check_with(&Probe {
        os,
        xcode_select: Path::new(XCODE_SELECT),
        fixed: &super::GIT_DIRS,
        path: path.as_deref(),
        os_release: &os_release,
    });
    if presence == Presence::Present {
        SEEN.store(true, Ordering::Relaxed);
    }
    presence
}

/// [`check`], as the runner's gate: `Ok` when git is there, else the error to return in place
/// of starting it.
pub fn require() -> Result<(), super::GitUnavailable> {
    match check() {
        Presence::Present => Ok(()),
        Presence::Missing(install) => Err(super::GitUnavailable(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            GitMissing(install),
        ))),
    }
}

/// Opens Apple's own installer for the command line developer tools (`xcode-select
/// --install`), which shows its dialog and returns. macOS only; the install itself runs in
/// the system's dialog, and [`check`] says when it is done.
pub fn start_install() -> Result<(), String> {
    start_install_with(Os::this(), Path::new(XCODE_SELECT))
}

/// [`start_install`] for `os` with `xcode_select` as the program, so a test can stand one in.
pub fn start_install_with(os: Os, xcode_select: &Path) -> Result<(), String> {
    if os != Os::MacOs {
        return Err(
            "purlis can start git's install on macOS only: install it as the window says".into(),
        );
    }
    let mut command = Command::new(xcode_select);
    command
        .arg("--install")
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = crate::forklock::spawn(&mut command)
        .map_err(|e| format!("purlis could not start Apple's installer: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("purlis could not start Apple's installer: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&out.stderr);
    let said = said.trim();
    Err(if said.is_empty() {
        "Apple's installer did not start".to_owned()
    } else {
        format!("Apple's installer did not start: {said}")
    })
}

/// A presence a test sets for its own thread, so the runner's gate can be driven to "missing"
/// on a machine that has git.
#[cfg(test)]
pub(crate) mod forced {
    use std::cell::RefCell;

    use super::Presence;

    thread_local! {
        static FORCED: RefCell<Option<Presence>> = const { RefCell::new(None) };
    }

    pub(crate) fn get() -> Option<Presence> {
        FORCED.with(|held| held.borrow().clone())
    }

    /// `presence` on this thread until the guard drops.
    pub(crate) fn set(presence: Presence) -> Guard {
        FORCED.with(|held| *held.borrow_mut() = Some(presence));
        Guard
    }

    pub(crate) struct Guard;

    impl Drop for Guard {
        fn drop(&mut self) {
            FORCED.with(|held| *held.borrow_mut() = None);
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use super::*;

    /// A program at `dir/name` that exits `code`, and writes its arguments to `dir/<name>.args`.
    fn stand_in(dir: &Path, name: &str, code: i32) -> PathBuf {
        let program = dir.join(name);
        let said = dir.join(format!("{name}.args"));
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf '%s ' \"$@\" > '{}'\nexit {code}\n",
                said.display()
            ),
        )
        .expect("the stand-in is written");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .expect("the stand-in is runnable");
        program
    }

    fn mac(xcode_select: &Path) -> Probe<'_> {
        Probe {
            os: Os::MacOs,
            xcode_select,
            fixed: &[],
            path: None,
            os_release: "",
        }
    }

    #[test]
    fn on_macos_git_is_present_when_xcode_select_names_the_developer_tools() {
        let dir = tempfile::tempdir().expect("a directory");
        let xcode_select = stand_in(dir.path(), "xcode-select", 0);
        assert_eq!(check_with(&mac(&xcode_select)), Presence::Present);
        let asked = std::fs::read_to_string(dir.path().join("xcode-select.args")).expect("asked");
        assert_eq!(asked.trim(), "-p");
    }

    #[test]
    fn on_macos_git_is_missing_when_xcode_select_says_no_tools_and_the_fix_is_apples_install() {
        let dir = tempfile::tempdir().expect("a directory");
        // `xcode-select -p` exits 2 on a Mac with no developer tools.
        let xcode_select = stand_in(dir.path(), "xcode-select", 2);
        assert_eq!(
            check_with(&mac(&xcode_select)),
            Presence::Missing(Install::DeveloperTools)
        );
    }

    #[test]
    fn on_macos_a_git_in_usr_bin_is_not_taken_for_git_present() {
        // The stub: `/usr/bin/git` is always on disk, and is not asked.
        let dir = tempfile::tempdir().expect("a directory");
        let git = stand_in(dir.path(), "git", 0);
        let xcode_select = stand_in(dir.path(), "xcode-select", 2);
        let fixed = [dir.path().to_str().expect("utf-8")];
        let probe = Probe {
            fixed: &fixed,
            ..mac(&xcode_select)
        };
        assert_eq!(
            check_with(&probe),
            Presence::Missing(Install::DeveloperTools)
        );
        assert!(git.is_file());
        assert!(
            !dir.path().join("git.args").exists(),
            "the git stand-in was run"
        );
    }

    #[test]
    fn on_macos_no_xcode_select_at_all_is_missing() {
        let probe = mac(Path::new("/definitely/not/xcode-select"));
        assert_eq!(
            check_with(&probe),
            Presence::Missing(Install::DeveloperTools)
        );
    }

    #[test]
    fn on_linux_git_is_present_when_the_runners_lookup_finds_one() {
        let dir = tempfile::tempdir().expect("a directory");
        stand_in(dir.path(), "git", 0);
        let path = dir.path().as_os_str();
        let probe = Probe {
            os: Os::Linux,
            xcode_select: Path::new("/unused"),
            fixed: &[],
            path: Some(path),
            os_release: "",
        };
        assert_eq!(check_with(&probe), Presence::Present);
        assert!(!dir.path().join("git.args").exists(), "git was run");
    }

    #[test]
    fn on_linux_a_missing_git_names_the_distributions_package_manager() {
        let empty = tempfile::tempdir().expect("a directory");
        let fixed = [empty.path().to_str().expect("utf-8")];
        let probe = |os_release| Probe {
            os: Os::Linux,
            xcode_select: Path::new("/unused"),
            fixed: &fixed,
            path: Some(empty.path().as_os_str()),
            os_release,
        };
        assert_eq!(
            check_with(&probe("ID=ubuntu\nID_LIKE=debian\n")),
            Presence::Missing(Install::Package {
                command: "sudo apt install git".into()
            })
        );
        assert_eq!(
            check_with(&probe("ID=\"fedora\"\n")),
            Presence::Missing(Install::Package {
                command: "sudo dnf install git".into()
            })
        );
        assert_eq!(
            check_with(&probe("ID=plan9\n")),
            Presence::Missing(Install::Download {
                page: DOWNLOAD_PAGE
            })
        );
    }

    #[test]
    fn on_windows_a_git_exe_on_path_is_present() {
        let dir = tempfile::tempdir().expect("a directory");
        std::fs::write(dir.path().join("git.exe"), "").expect("the stand-in is written");
        let probe = Probe {
            os: Os::Windows,
            xcode_select: Path::new("/unused"),
            fixed: &[],
            path: Some(dir.path().as_os_str()),
            os_release: "",
        };
        assert_eq!(check_with(&probe), Presence::Present);
    }

    #[test]
    fn on_windows_a_missing_git_points_at_the_download_page() {
        let empty = tempfile::tempdir().expect("a directory");
        let probe = Probe {
            os: Os::Windows,
            xcode_select: Path::new("/unused"),
            fixed: &[],
            path: Some(empty.path().as_os_str()),
            os_release: "",
        };
        assert_eq!(
            check_with(&probe),
            Presence::Missing(Install::Download {
                page: DOWNLOAD_PAGE
            })
        );
    }

    #[test]
    fn the_install_opens_apples_installer_on_macos_and_says_why_when_it_does_not() {
        let dir = tempfile::tempdir().expect("a directory");
        let fine = stand_in(dir.path(), "xcode-select", 0);
        assert_eq!(start_install_with(Os::MacOs, &fine), Ok(()));
        let asked = std::fs::read_to_string(dir.path().join("xcode-select.args")).expect("asked");
        assert_eq!(asked.trim(), "--install");

        let other = tempfile::tempdir().expect("a directory");
        let refusing = stand_in(other.path(), "xcode-select", 1);
        assert!(start_install_with(Os::MacOs, &refusing).is_err());
        assert!(start_install_with(Os::Linux, &fine).is_err());
    }

    #[test]
    fn the_runner_starts_no_git_while_git_is_missing_and_says_how_to_install_it() {
        let dir = tempfile::tempdir().expect("a directory");
        let _missing = forced::set(Presence::Missing(Install::DeveloperTools));
        let refused = super::super::run(dir.path(), &["--version"], super::super::READ)
            .expect_err("no git is started");
        assert_eq!(refused.missing(), Some(&Install::DeveloperTools));
        // git is installed where this runs, so a git started would have answered: the
        // refusal is the gate's, before any process.
        assert!(
            refused.to_string().contains("xcode-select --install"),
            "{refused}"
        );
    }

    #[test]
    fn the_runner_runs_git_once_it_is_there() {
        let dir = tempfile::tempdir().expect("a directory");
        let _present = forced::set(Presence::Present);
        let run = super::super::run(dir.path(), &["--version"], super::super::READ)
            .expect("git is started");
        assert!(run.ok(), "{run:?}");
    }
}
