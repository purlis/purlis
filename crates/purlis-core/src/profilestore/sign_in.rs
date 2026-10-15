// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A subscription profile is signed in through its harness's own flow** (#2035, ADR 0095 §3).
//!
//! Signing in runs the harness's own sign-in command (`claude auth login`, `codex login`,
//! `opencode auth login`) with the profile's folder variables set, in a terminal the person
//! drives: a shell tab in the window, or their own terminal with `purlis profile sign-in`. The
//! harness keeps what it signs in with in the profile's folder. purlis reads none of what the
//! command prints, and never opens a file it writes ([`Login`]).
//!
//! **Whether a profile is signed in is the harness's own answer** ([`standing`]): its status
//! command's exit status where it has one (Claude Code's `auth status`, Codex's
//! `login status`), with nothing it prints read; otherwise whether its credential file is
//! there, by its name alone (opencode's `auth.json`), never opened. Each harness's words are
//! here, beside [`crate::harness_declaration`]'s own list of sign-in variables, not yet
//! declaration data. Every [`Look`] can run a program and look for a name, and none can read a
//! file.
//!
//! **A chat is refused only where the harness says it is signed out** ([`refusal`]), and only
//! on a profile purlis made: an adopted folder is signed in by the person's own sign-in and is
//! never asked, and a built-in or a clone's profile runs as it always did. A harness that cannot
//! tell, or does not answer in time, never stops a chat that might work.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::MachineProfile;
use crate::profiles::{Profile, Source};

/// How long a status command has to answer: as long as a program's `--version`
/// ([`crate::sandbox::program::PATIENCE`]), since a program the system has not seen before is
/// assessed on its first run.
pub const PATIENCE: Duration = crate::sandbox::program::PATIENCE;

/// Whether a profile is signed in on this device, as its harness says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// The harness says it is signed in.
    SignedIn,
    /// The harness says it is not: a chat on it is refused, with Sign in as the way out.
    SignedOut,
    /// The harness did not say, or has no way to: a chat on it starts.
    CannotTell,
}

impl Standing {
    /// The word the window and the command line say it by.
    pub fn word(self) -> &'static str {
        match self {
            Self::SignedIn => "signed-in",
            Self::SignedOut => "signed-out",
            Self::CannotTell => "cannot-tell",
        }
    }
}

/// What purlis may do to tell whether a profile is signed in: run a program and see how it
/// exits, and look for a name. Nothing here reads a file, and nothing a program prints is kept.
pub trait Look {
    /// The exit status of `program`, found where a chat's program is found, run with `args`,
    /// `env` set over this process's own environment, its input empty and its output thrown
    /// away; `None` where it could not be found or run, was ended by a signal, or did not end
    /// within [`PATIENCE`].
    fn status(&self, program: &str, args: &[&str], env: &[(String, String)]) -> Option<i32>;
    /// Whether something is at `path`, by its name, without opening it.
    fn is_there(&self, path: &Path) -> bool;
}

/// This machine: programs run for real, and names looked for on its disk.
pub struct ThisMachine;

impl Look for ThisMachine {
    fn status(&self, program: &str, args: &[&str], env: &[(String, String)]) -> Option<i32> {
        use std::process::{Command, Stdio};
        let program = crate::programs::resolve(program).ok()?;
        let mut command = Command::new(program);
        command
            .args(args)
            .envs(env.iter().map(|(n, v)| (n, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = crate::forklock::spawn(&mut command).ok()?;
        let until = Instant::now() + PATIENCE;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.code(),
                Ok(None) if Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
            }
        }
    }

    fn is_there(&self, path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok()
    }
}

/// How a harness's status is told.
enum Status {
    /// Its own command, run in the profile's folder: exit 0 signed in, 1 signed out.
    Command(&'static [&'static str]),
    /// A file of this name under its own folder in the profile's data folder: there, it is
    /// signed in; not there, it may still run on a model that needs no sign-in.
    Named {
        own: &'static str,
        file: &'static str,
    },
}

/// A harness's own sign-in, and how its status is told.
struct Flow {
    kind: &'static str,
    /// The words after its program that sign it in, in its own terminal flow.
    login: &'static [&'static str],
    status: Status,
}

/// The harnesses purlis ships, each by its own documentation and help: Claude Code 2.1's
/// `auth login` and `auth status`, Codex's `login` and `login status`, and opencode's
/// `auth login`, which keeps what it signs in with in `opencode/auth.json` under its data
/// folder and has no status that says signed out.
const FLOWS: [Flow; 3] = [
    Flow {
        kind: "claude",
        login: &["auth", "login"],
        status: Status::Command(&["auth", "status"]),
    },
    Flow {
        kind: "codex",
        login: &["login"],
        status: Status::Command(&["login", "status"]),
    },
    Flow {
        kind: "opencode",
        login: &["auth", "login"],
        status: Status::Named {
            own: "opencode",
            file: "auth.json",
        },
    },
];

fn flow(kind: &str) -> Option<&'static Flow> {
    FLOWS.iter().find(|flow| flow.kind == kind)
}

/// The value `profile` sets its harness's `variable` to, from its declared folder variables.
fn folder(profile: &Profile, variable: Option<&str>) -> Option<PathBuf> {
    let variable = variable?;
    profile
        .env
        .iter()
        .find(|(name, _)| name == variable)
        .map(|(_, value)| PathBuf::from(value))
}

/// **Whether `profile` is signed in on this device**, as its harness says it through `look`.
pub fn standing(profile: &Profile, look: &dyn Look) -> Standing {
    let Some(flow) = flow(&profile.kind) else {
        return Standing::CannotTell;
    };
    match flow.status {
        Status::Command(words) => {
            let Some(program) = program_of(profile) else {
                return Standing::CannotTell;
            };
            match look.status(&program, words, &profile.env) {
                Some(0) => Standing::SignedIn,
                Some(1) => Standing::SignedOut,
                _ => Standing::CannotTell,
            }
        }
        Status::Named { own, file } => {
            let data = crate::harness_declaration::builtin(&profile.kind)
                .and_then(|d| folder(profile, d.folders.data_env.as_deref()));
            match data {
                Some(data) if look.is_there(&data.join(own).join(file)) => Standing::SignedIn,
                _ => Standing::CannotTell,
            }
        }
    }
}

/// `profile`'s program, as its command names it.
fn program_of(profile: &Profile) -> Option<String> {
    let home = crate::profiles::home().unwrap_or_default();
    crate::profiles::expanded_command(profile, &home)
        .into_iter()
        .next()
}

/// **Why a chat on `profile` may not start, or `None`**: only a profile purlis made on this
/// machine, signed in by a subscription as far as purlis can see (no key or cloud switch it is
/// handed, by its `env` or by `passed`, the project's `[chat_env] pass`), whose harness says it
/// is signed out.
pub fn refusal(profile: &Profile, passed: &[String], look: &dyn Look) -> Option<String> {
    if profile.source != Source::Machine {
        return None;
    }
    let declared = crate::harness_declaration::builtin(&profile.kind)?;
    if declared.profile_sign_in(&profile.env, passed).is_some() {
        return None;
    }
    (standing(profile, look) == Standing::SignedOut).then(|| {
        format!(
            "the profile '{name}' is not signed in on {here}. Nothing was started. Sign in runs \
             {title}'s own sign-in in a shell tab: press Sign in, or run purlis profile sign-in \
             {name} in your own terminal, then start the chat again.",
            name = crate::shown::short(&profile.name),
            here = super::this_device(),
            title = declared.title,
        )
    })
}

/// A harness's own sign-in, ready to run in a terminal the person drives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Login {
    /// The program, found where a chat's is.
    pub program: String,
    /// Its own sign-in words.
    pub args: Vec<String>,
    /// The profile's folder variables, set over the terminal's own environment.
    pub env: Vec<(String, String)>,
}

impl Login {
    /// **The one line a shell runs it by**: `env VAR=<folder> … <program> <words>`, each word
    /// quoted for the shell, and a line break. `env` sets the folders in every shell a person
    /// may have. A word with a control character in it is refused, so nothing typed can break
    /// the line in two.
    pub fn line(&self) -> Result<String, String> {
        let mut words = vec!["env".to_owned()];
        words.extend(self.env.iter().map(|(n, v)| format!("{n}={v}")));
        words.push(self.program.clone());
        words.extend(self.args.iter().cloned());
        if let Some(bad) = words.iter().find(|w| w.chars().any(char::is_control)) {
            return Err(format!(
                "{} has a control character in it, so purlis types no sign-in with it.",
                crate::shown::short(bad)
            ));
        }
        let quoted: Vec<String> = words
            .iter()
            .map(|w| crate::shellseg::shell_quote(w))
            .collect();
        Ok(format!("{}\n", quoted.join(" ")))
    }
}

/// The sign-in of `profile`, with `program` as its program.
fn login_for(profile: &Profile, program: &str) -> Login {
    Login {
        program: program.to_owned(),
        args: flow(&profile.kind)
            .map(|flow| flow.login.iter().map(|w| (*w).to_owned()).collect())
            .unwrap_or_default(),
        env: profile.env.clone(),
    }
}

/// **The sign-in of `record`**, one of this machine's profiles: its harness's own, in its
/// folder. Refused for a folder adopted in place, which is signed in by the person's own
/// sign-in, and for a profile whose harness purlis cannot find.
pub fn login_of(record: &MachineProfile) -> Result<Login, String> {
    let name = crate::shown::short(&record.name);
    if record.adopted.is_some() {
        return Err(format!(
            "'{name}' is {}'s own folder on {}, signed in by its own sign-in, so purlis has \
             nothing to sign in.",
            record.title,
            super::this_device()
        ));
    }
    let profile = crate::profiles::of_record(record)
        .filter(|profile| flow(&profile.kind).is_some())
        .ok_or_else(|| {
            format!("purlis cannot sign the profile '{name}' in: it runs no harness purlis ships.")
        })?;
    let home = crate::profiles::home().unwrap_or_default();
    let argv = crate::profiles::expanded_command(&profile, &home);
    let program = crate::programs::resolve_argv(&argv)
        .map_err(|gone| format!("{} Nothing was signed in.", gone.said()))?
        .into_iter()
        .next()
        .ok_or_else(|| format!("the profile '{name}' names no program."))?;
    Ok(login_for(&profile, &program))
}

/// Whether `record`, one of this machine's profiles, is signed in, as its harness says: what
/// the window and `purlis profile sign-in --check` say. `None` for a folder adopted in place,
/// which is signed in by the person's own sign-in and never asked.
pub fn standing_of(record: &MachineProfile, look: &dyn Look) -> Option<Standing> {
    if record.adopted.is_some() {
        return None;
    }
    crate::profiles::of_record(record).map(|profile| standing(&profile, look))
}

#[cfg(test)]
mod tests;
