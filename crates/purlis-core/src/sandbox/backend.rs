// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Whether this machine can apply a sandbox at all: the operating system's mechanism, and the
//! programs a harness's sandbox runs through. Asked before a chat starts, so a machine that
//! cannot is told what is missing, and how to install it, rather than handed a harness that
//! exits.

use std::fmt;

use super::Os;

/// Where macOS keeps Seatbelt's front end. Always there on a supported macOS.
pub const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// The word [`missing_for`] asks [`installed`] about for purlis's own wrap on Linux: not a
/// program, the kernel's Landlock at [`purlis_linux_sandbox::MIN_ABI`] or later (#1040).
pub const LANDLOCK: &str = "landlock";

/// What this machine lacks for a sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missing {
    /// Programs the backend runs through, by name, on `os`.
    Programs { os: Os, absent: Vec<&'static str> },
    /// No backend exists for this operating system yet (Windows: M46, #565).
    NoBackend(Os),
    /// This Linux kernel has no Landlock at [`purlis_linux_sandbox::MIN_ABI`], which purlis's
    /// own wrap needs (#1040).
    Landlock,
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Programs { os, absent } => {
                let (verb, it) = if absent.len() == 1 {
                    ("is", "it")
                } else {
                    ("are", "them")
                };
                write!(f, "{} {verb} not installed", absent.join(" and "))?;
                // SD-30's prerequisite install, named where the chat is refused. The package is
                // `bubblewrap` for the `bwrap` program on every major distribution.
                if *os == Os::Linux {
                    let packages: Vec<&str> = absent
                        .iter()
                        .map(|it| if *it == "bwrap" { "bubblewrap" } else { it })
                        .collect();
                    let packages = packages.join(" ");
                    write!(
                        f,
                        " — install {it} with `sudo apt install {packages}` or \
                         `sudo dnf install {packages}`"
                    )?;
                }
                Ok(())
            }
            // M46 Windows, #565.
            Self::NoBackend(Os::Windows) => {
                f.write_str("purlis has no sandbox backend on Windows yet")
            }
            Self::NoBackend(_) => {
                f.write_str("purlis has no sandbox backend on this operating system")
            }
            Self::Landlock => write!(
                f,
                "{}, and this kernel does not have it",
                purlis_linux_sandbox::NEEDS_LANDLOCK
            ),
        }
    }
}

/// What is missing on `os` for a sandbox to be applied, or `None` when nothing is. `has` says
/// whether a program is installed, by its name.
///
/// - **macOS**: Seatbelt, through `sandbox-exec`.
/// - **Linux**: bubblewrap for the filesystem and `socat` for the network proxy, the two
///   programs Claude Code's sandbox runs through.
/// - **Windows**: no backend yet (ruling V21, 3; M46 Windows, #565).
pub fn missing(os: Os, has: &dyn Fn(&str) -> bool) -> Option<Missing> {
    let needs: &[&'static str] = match os {
        Os::MacOs => &["sandbox-exec"],
        Os::Linux => &["bwrap", "socat"],
        Os::Windows | Os::Other => return Some(Missing::NoBackend(os)),
    };
    let absent: Vec<&'static str> = needs.iter().copied().filter(|it| !has(it)).collect();
    (!absent.is_empty()).then_some(Missing::Programs { os, absent })
}

/// What is missing on `os` for a sandbox to be applied to a chat whose harness is `wrapped`
/// whole in purlis's own wrap (Codex and opencode, and Claude Code on a runner, #4083), or `None`
/// when nothing is.
///
/// On Linux purlis's own wrap is Landlock, a seccomp filter and the supervisor (#1040), which
/// need the kernel's Landlock and no program; Claude Code's own sandbox needs what [`missing`]
/// names. Elsewhere, as [`missing`].
pub fn missing_for(os: Os, wrapped: bool, has: &dyn Fn(&str) -> bool) -> Option<Missing> {
    if os == Os::Linux && wrapped {
        return (!has(LANDLOCK)).then_some(Missing::Landlock);
    }
    missing(os, has)
}

/// What a person can do where this kernel lacks the Landlock purlis's own wrap needs
/// ([`purlis_linux_sandbox::landlock_way_on`]).
pub fn landlock_way_on(opt_out: bool) -> String {
    purlis_linux_sandbox::landlock_way_on(opt_out)
}

/// SD-30's install action: the command that installs what `missing` names with the package
/// manager of the distribution `os_release` (the text of `/etc/os-release`) says this is, or
/// `None` where there is nothing to install or charter does not know the distribution.
///
/// **Typed, never run** (ruling V78 c). Installing needs `sudo`, so the app types this into a
/// shell tab at the project root and the person presses Return, unlike FR-29's installers
/// (V65). It is built here from charter's own table and the programs found missing, so no text
/// from a project, a plane or the window reaches the shell.
pub fn install_command(missing: &Missing, os_release: &str) -> Option<String> {
    let Missing::Programs {
        os: Os::Linux,
        absent,
    } = missing
    else {
        return None;
    };
    let field = |key: &str| -> Vec<String> {
        os_release
            .lines()
            .filter_map(|line| line.trim().strip_prefix(key)?.strip_prefix('='))
            .flat_map(|value| {
                value
                    .trim()
                    .trim_matches(['"', '\''])
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let mut names = field("ID");
    names.extend(field("ID_LIKE"));
    // The distribution itself first, then what it is like.
    const MANAGERS: [(&[&str], &str); 5] = [
        (&["debian", "ubuntu"], "sudo apt install"),
        (&["fedora", "rhel", "centos"], "sudo dnf install"),
        (&["arch"], "sudo pacman -S"),
        (&["opensuse", "suse"], "sudo zypper install"),
        (&["alpine"], "sudo apk add"),
    ];
    let manager = names.iter().find_map(|name| {
        MANAGERS
            .iter()
            .find(|(ids, _)| {
                ids.iter()
                    .any(|id| name == id || name.starts_with(&format!("{id}-")))
            })
            .map(|(_, manager)| *manager)
    })?;
    let packages: Vec<&str> = absent
        .iter()
        .map(|it| if *it == "bwrap" { "bubblewrap" } else { it })
        .collect();
    Some(format!("{manager} {}", packages.join(" ")))
}

/// Whether `program` is installed on this machine, as [`missing`] asks it; for [`LANDLOCK`],
/// whether the kernel has the Landlock purlis's own wrap needs.
pub fn installed(program: &str) -> bool {
    if program == LANDLOCK {
        #[cfg(target_os = "linux")]
        return purlis_linux_sandbox::abi() >= purlis_linux_sandbox::MIN_ABI;
        #[cfg(not(target_os = "linux"))]
        return false;
    }
    if program == "sandbox-exec" {
        return crate::programs::runnable(std::path::Path::new(SANDBOX_EXEC));
    }
    crate::programs::on_path(program)
}
