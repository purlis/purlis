// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How a run is sandboxed, as its host applied it** (#2636): read from what the host starts
//! (the program it runs, the words it runs it with, and whether a confinement holds it), never
//! from anything a start says of itself. What a window that comes back judges the run by, and
//! every path that treats a sandboxed chat and an unsandboxed one apart.
//!
//! - A program Seatbelt wraps ([`super::backend::SANDBOX_EXEC`]) runs in `seatbelt`.
//! - One purlis's own Linux wrap runs (`purlis _sandbox`) runs in `landlock`.
//! - Claude Code with its own sandbox on in the settings it is started with runs in that
//!   sandbox: `seatbelt` on macOS, `bubblewrap` on Linux.
//! - Any other a confinement holds runs in `proxy`: it is never read as off.
//! - One with none of these is off.
//!
//! **Read as sandboxed where in doubt.** A line that looks sandboxed and is not is read as
//! sandboxed, which only refuses more; a confined run is never read as off.

use std::ffi::{OsStr, OsString};

use purlis_session_protocol::session::RunSandbox;

use super::Os;

/// How a run of `program` with `args` is sandboxed on `os`, where `confined` says whether a
/// confinement holds it (module docs).
pub fn applied_to(program: &OsStr, args: &[OsString], confined: bool, os: Os) -> RunSandbox {
    let by = |word: &str| RunSandbox::By(word.to_owned());
    if program == OsStr::new(super::backend::SANDBOX_EXEC) {
        return by("seatbelt");
    }
    if args.first().map(OsString::as_os_str) == Some(OsStr::new(purlis_linux_sandbox::COMMAND)) {
        return by("landlock");
    }
    if claude_codes_own_is_on(args)
        && let Some(word) = os.backend_word(Some(crate::harness::Harness::ClaudeCode))
    {
        return by(word);
    }
    if confined {
        by("proxy")
    } else {
        RunSandbox::Off
    }
}

/// Whether `args` start Claude Code with its own sandbox on: the settings after `--settings`
/// turn it on, as JSON, or in the chat's own settings file purlis wrote where they were too long
/// for one word (`hoststart::settings_in_a_file`). A file named as one purlis writes that no
/// longer holds what was written is read as on: in doubt, sandboxed.
fn claude_codes_own_is_on(args: &[OsString]) -> bool {
    let Some(settings) = args
        .iter()
        .position(|arg| arg == "--settings")
        .and_then(|flag| args.get(flag + 1))
        .and_then(|settings| settings.to_str())
    else {
        return false;
    };
    let on =
        |settings: &serde_json::Value| settings.pointer("/sandbox/enabled") == Some(&true.into());
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(settings) {
        return on(&json);
    }
    let path = std::path::Path::new(settings);
    match crate::hoststart::settings_from_its_file(path) {
        Some(read) => on(&read),
        None => crate::hoststart::names_a_settings_file(path),
    }
}

#[cfg(test)]
mod tests;
