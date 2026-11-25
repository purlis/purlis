// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`--script <file> --dialect claude|codex|opencode`** (T1.1-04, #6851): a harness that plays
//! a turn script as the model, and does everything else as the real harness does.
//!
//! - **Each turn** is a prompt and the calls the model asks for in it ([`turns`]). A shell
//!   call runs its command for real, and a native call (a write, an edit, a read, a fetch, an
//!   MCP tool) does its I/O for real in the fake's own process, so whatever confines the fake
//!   (purlis's wrap) decides each outcome; the script never states one.
//! - **Its hooks** are the ones purlis arms the dialect with ([`routes`], derived from
//!   purlis-core), fired in the real harness's order with its payloads and environment
//!   ([`dialect`], [`chat`]), and what they answer is acted on as the harness acts on it: a
//!   refusal stops the call, a hook past its timeout is killed and the call goes on.
//! - **Claude Code's `permissions.deny`** in the `--settings` it was started with hold its
//!   native tools before any hook and any I/O ([`rules`]).
//! - **The harness's own ask** is asked as the harness asks it, and answered on the keyboard:
//!   Claude Code's for a write or an edit in its `default` permission mode, as its `asks`
//!   recording shows it (`PermissionRequest`, then once the dialog has waited `Notification`);
//!   Codex's for a command the model asks to run escalated; OpenCode's for a file outside the
//!   chat's folder.
//! - **The model transcript** ([`transcript`]) is everything the model would be shown.
//! - **A relaunch** with the dialect's resume argument continues at the next turn, from a
//!   cursor in `$FAKE_HARNESS_STATE` kept by session id ([`chat::Cursor`]).
//!
//! `--routes` prints the routes as JSON and does nothing else; `--version` answers as the
//! dialect's program does, with the script's version.

mod chat;
pub mod dialect;
mod hooks;
mod keyboard;
mod launch;
mod mcp_call;
pub mod routes;
mod rules;
mod transcript;
mod turns;

use std::path::PathBuf;
use std::process::ExitCode;

/// Whether `argv` asks for a scripted harness.
pub fn wanted(argv: &[String]) -> bool {
    argv.iter()
        .any(|a| a == "--script" || a.starts_with("--script="))
}

pub fn main(argv: &[String]) -> ExitCode {
    keep_stderr();
    match run(argv) {
        Ok(code) => {
            trace(format_args!("script ended, exit {code}"));
            ExitCode::from(code)
        }
        Err(message) => {
            eprintln!("fake-harness: {message}");
            ExitCode::FAILURE
        }
    }
}

/// `$FAKE_HARNESS_STDERR`: a file standard error is appended to instead of the terminal, so a
/// driver that sees the fake end can print what it said last (the wrap job's hang, #7581).
fn keep_stderr() {
    if let Some(path) = std::env::var_os("FAKE_HARNESS_STDERR")
        && let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        #[cfg(unix)]
        let _ = rustix::stdio::dup2_stderr(&file);
        #[cfg(not(unix))]
        drop(file);
    }
}

/// One line on standard error, only where `$FAKE_HARNESS_TRACE` is set: what the fake was doing
/// when it stopped.
pub(crate) fn trace(line: std::fmt::Arguments<'_>) {
    if std::env::var_os("FAKE_HARNESS_TRACE").is_some() {
        eprintln!("fake-harness trace: {line}");
    }
}

fn run(argv: &[String]) -> Result<u8, String> {
    let (own, rest) = launch::split(argv)?;
    let dialect_name = own
        .dialect
        .clone()
        .ok_or("--script needs --dialect claude, codex or opencode")?;
    let dialect = dialect::Dialect::parse(&dialect_name)
        .ok_or_else(|| format!("{dialect_name:?} is no dialect: claude, codex or opencode"))?;
    let binary = hook_binary(&rest);
    if own.routes {
        let routes = routes::of(dialect, &binary, &rest);
        println!(
            "{}",
            serde_json::json!({
                "dialect": dialect.name(),
                "fires": routes::fires(dialect),
                "never_fired": routes::never_fired(dialect),
                "routes": routes,
            })
        );
        return Ok(0);
    }
    let script_path = own.script.clone().ok_or("--script needs a file")?;
    let cwd = std::env::current_dir().map_err(|e| format!("no current folder: {e}"))?;
    // `$FAKE_HARNESS_PLACES`, a JSON object, where a wrapper cannot pass `--place`.
    let mut places: std::collections::BTreeMap<String, String> =
        std::env::var("FAKE_HARNESS_PLACES")
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
    places.extend(own.places.clone());
    places
        .entry("work".to_owned())
        .or_insert_with(|| cwd.display().to_string());
    let script = turns::read(&script_path, &places)?;
    if own.version {
        let version = script.version.clone().unwrap_or_else(|| "0.0.0".to_owned());
        println!("{}", dialect.version_line(&version));
        return Ok(0);
    }
    let transcript_path = own
        .transcript
        .clone()
        .or_else(|| std::env::var_os("FAKE_HARNESS_TRANSCRIPT").map(PathBuf::from));
    let transcript = transcript::Transcript::open(transcript_path.as_deref())?;
    let launched = launch::read(dialect, &rest);
    chat::Chat::new(dialect, binary, rest, launched, cwd, transcript, script)?.play()
}

/// The `purlis` every hook runs: `$CHARTER_HOOK_BINARY` as purlis sets it for the chat, else the
/// one Codex's `-c hooks.` words name, else `purlis` on the `PATH`.
fn hook_binary(argv: &[String]) -> PathBuf {
    std::env::var_os(purlis_core::plugin::BINARY_ENV)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| routes::binary_in_words(argv))
        .unwrap_or_else(|| PathBuf::from("purlis"))
}
