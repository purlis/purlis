// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Whether the pinned engine can make its sandbox here** (#2835, ruled 2026-10-20).
//!
//! Chrome's sandbox makes user namespaces. Ubuntu 23.10 and later allow them only to a program
//! an AppArmor profile names (`kernel.apparmor_restrict_unprivileged_userns`), and Chrome's own
//! package installs a profile for itself. The engine purlis downloads into its data folder has
//! none, so without one it ends at its start, and purlis never runs it with its sandbox off.
//!
//! - **The `.deb` installs [`SHIPPED`]**, at `/etc/apparmor.d/purlis-engine`: it names the
//!   pinned engine in the default data folder alone, and allows it user namespaces and nothing
//!   more.
//! - **Elsewhere, the person is offered the profile** for this machine's engine, wherever its
//!   data folder is ([`profile_for`]), as a command they run themselves ([`install_command`]):
//!   purlis never runs anything as root. Or this machine's own Chrome, where one is installed.
//! - **It is told before the engine starts** where it can be ([`sandboxes`], from
//!   [`Machine::here`]), and from the engine's log where it cannot ([`said_no_usable_sandbox`]).

use std::path::{Path, PathBuf};

/// The profile the `.deb` installs, at [`PROFILE`].
pub const SHIPPED: &str = include_str!("purlis-engine.apparmor");

/// Where the `.deb` installs [`SHIPPED`].
pub const PROFILE: &str = "/etc/apparmor.d/purlis-engine";

/// Where the command the person is offered writes [`profile_for`] this machine's engine.
pub const HERE_PROFILE: &str = "/etc/apparmor.d/purlis-engine-here";

/// The setting that says whether AppArmor restricts user namespaces: `1` when it does.
pub const RESTRICTS: &str = "/proc/sys/kernel/apparmor_restrict_unprivileged_userns";

/// What the session host refuses a browser's start with where the pinned engine cannot make its
/// sandbox and nothing else is chosen: the window tells the person what to do, and the chat that
/// a browser is not available here.
pub const NO_ENGINE_SANDBOX: &str =
    "the browser engine cannot make its sandbox on this machine, and purlis never runs it without";

/// What the engine writes to its log as it ends for want of a sandbox.
const NO_USABLE_SANDBOX: &str = "No usable sandbox";

/// What decides whether the engine sandboxes on a machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Machine {
    /// AppArmor restricts user namespaces to the programs a profile names.
    pub restricts: bool,
    /// The text of each purlis profile installed ([`PROFILE`], [`HERE_PROFILE`]).
    pub profiles: Vec<String>,
    /// This user's home, which [`SHIPPED`] names the data folder under.
    pub home: Option<PathBuf>,
}

impl Machine {
    /// This machine, read now. Off Linux nothing restricts the engine's sandbox.
    pub fn here() -> Machine {
        if !cfg!(target_os = "linux") {
            return Machine::default();
        }
        let read = |path: &str| std::fs::read_to_string(path).ok();
        Machine {
            restricts: read(RESTRICTS).is_some_and(|value| value.trim() == "1"),
            profiles: [PROFILE, HERE_PROFILE]
                .into_iter()
                .filter_map(read)
                .collect(),
            home: dirs::home_dir(),
        }
    }
}

/// The pinned engine's program in `engines` (`<data>/engines/chrome-for-testing`), any version.
fn engine_in(engines: &Path) -> PathBuf {
    engines
        .join("0-linux64")
        .join("chrome-linux64")
        .join("chrome")
}

/// **Whether the pinned engine in `engines` can make its sandbox on `machine`**: nothing
/// restricts user namespaces there, or a purlis profile names it.
pub fn sandboxes(machine: &Machine, engines: &Path) -> bool {
    if !machine.restricts {
        return true;
    }
    let engine = engine_in(engines);
    machine.profiles.iter().any(|profile| {
        attachment(profile)
            .is_some_and(|pattern| matches(&pattern, &engine, machine.home.as_deref()))
    })
}

/// **The profile for the pinned engine in `engines`**, wherever that is: the program by its
/// path, any version, allowed user namespaces and nothing more. Every character AppArmor reads
/// as a pattern, a variable or a quote in the path is written as itself; a path with a control
/// character in it is offered no profile.
pub fn profile_for(engines: &Path) -> Option<String> {
    let folder = engines.display().to_string();
    if folder.chars().any(char::is_control) {
        return None;
    }
    let pattern = format!(
        "{}/*/chrome-linux64/chrome",
        as_itself(folder.trim_end_matches('/'))
    );
    Some(format!(
        "abi <abi/4.0>,\n\
         include <tunables/global>\n\
         \n\
         profile purlis-engine-here \"{pattern}\" flags=(unconfined) {{\n\
         \x20 userns,\n\
         }}\n"
    ))
}

/// `text` with each character AppArmor reads as a pattern, a variable or a quote escaped.
fn as_itself(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(
            c,
            '*' | '?' | '[' | ']' | '{' | '}' | '^' | '@' | '\\' | '"'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// **The command the person runs** to install [`profile_for`] `engines` at [`HERE_PROFILE`] and
/// load it: purlis offers it and never runs it. None where no profile is offered.
pub fn install_command(engines: &Path) -> Option<String> {
    Some(writing(&profile_for(engines)?, HERE_PROFILE))
}

/// **The command the person runs on another machine** whose engine cannot make its sandbox, a
/// runner (#2990): it installs [`SHIPPED`] at [`PROFILE`], as the `.deb` does, which names the
/// pinned engine in any user's default data folder, and loads it. purlis offers it and never
/// runs it.
pub fn install_shipped_command() -> String {
    writing(SHIPPED, PROFILE)
}

/// The command that writes `profile` to `at` line by line and loads it.
fn writing(profile: &str, at: &str) -> String {
    let lines: Vec<String> = profile.trim_end().lines().map(single_quoted).collect();
    format!(
        "printf '%s\\n' {} | sudo tee {at} >/dev/null && sudo apparmor_parser -r {at}",
        lines.join(" ")
    )
}

/// `text` as one word for a POSIX shell.
fn single_quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// Whether the engine's log says it ended for want of a sandbox.
pub fn said_no_usable_sandbox(log: &str) -> bool {
    log.contains(NO_USABLE_SANDBOX)
}

/// The program pattern a profile names, unquoted.
pub fn attachment(profile: &str) -> Option<String> {
    let line = profile
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("profile "))?;
    let rest = line.strip_prefix("profile ")?.split_once(' ')?.1.trim();
    if let Some(quoted) = rest.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = quoted.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => out.push(chars.next()?),
                '"' => return Some(out),
                c => out.push(c),
            }
        }
        return None;
    }
    rest.split_whitespace().next().map(str::to_owned)
}

/// Whether AppArmor's `pattern` names `path`, `@{HOME}` read as `home`: `*` is any run of
/// characters but `/`, and `{a,b}` either word. What else a pattern may say is read as itself.
fn matches(pattern: &str, path: &Path, home: Option<&Path>) -> bool {
    let pattern = match home {
        Some(home) => pattern.replace("@{HOME}", home.display().to_string().trim_end_matches('/')),
        None if pattern.contains("@{HOME}") => return false,
        None => pattern.to_owned(),
    };
    alternatives(&pattern)
        .iter()
        .any(|one| glob(one.as_bytes(), path.display().to_string().as_bytes()))
}

/// Each spelling `{a,b}` groups in `pattern` give it (not nested).
fn alternatives(pattern: &str) -> Vec<String> {
    let (Some(open), Some(close)) = (pattern.find('{'), pattern.find('}')) else {
        return vec![pattern.to_owned()];
    };
    if close < open {
        return vec![pattern.to_owned()];
    }
    let (head, rest) = pattern.split_at(open);
    let (group, tail) = rest[1..].split_at(close - open - 1);
    let tail = &tail[1..];
    group
        .split(',')
        .flat_map(|word| alternatives(&format!("{head}{word}{tail}")))
        .collect()
}

/// `*` as any run of bytes but `/`.
fn glob(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => (0..=text.len())
            .take_while(|&n| !text[..n].contains(&b'/'))
            .any(|n| glob(rest, &text[n..])),
        Some((c, rest)) => text.first() == Some(c) && glob(rest, &text[1..]),
    }
}

#[cfg(test)]
mod tests;
