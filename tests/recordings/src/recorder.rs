// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The recorder** (T1.1-03 item 1): the program a recording's hook entries name in place of
//! `purlis` (`purlis_core::plugin::BINARY_ENV`). For `hook <word>` it writes what the harness
//! handed the call (its standard input, its environment, when it began) before running the real
//! `purlis hook`, and what that answered after, then answers the harness the same. Every other
//! command is the real `purlis`'s, run in its place unchanged.
//!
//! It is told, in its environment:
//! - [`REAL`]: the real `purlis`;
//! - [`OUT`]: the file it appends its lines to, one JSON object each ([`Line`]);
//! - [`STALL`], optionally: a hook word whose first call it holds past any hook's timeout before
//!   running the real hook, so a recording shows what the harness does with a hook that never
//!   answers. A call it held writes no reply unless the harness let it finish.
//!
//! - [`EXIT2`], optionally: a hook word whose first call it answers itself with exit 2 and a
//!   reason on standard error, without running the real hook, so a recording shows what the
//!   harness does with a hook's exit 2.
//!
//! These are its own, and none of them is recorded. A call it held or answered itself says so in
//! its lines (`forced`).

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// The real `purlis`.
pub const REAL: &str = "PURLIS_RECORDER_REAL";
/// The file the recorder appends to.
pub const OUT: &str = "PURLIS_RECORDER_OUT";
/// A hook word whose first call is held past every hook's timeout.
pub const STALL: &str = "PURLIS_RECORDER_STALL";

/// A hook word whose first call is answered with exit 2.
pub const EXIT2: &str = "PURLIS_RECORDER_EXIT2";

/// What the recorder says on standard error when it answers a call with exit 2 itself.
pub const EXIT2_REASON: &str = "purlis-recorder: refused with exit 2 for the recording";

/// How long a held call is held: past the longest timeout purlis gives a hook entry.
pub const HELD: Duration = Duration::from_secs(90);

/// The variables of a hook's environment a recording keeps, by prefix: what purlis sets for a
/// chat and what each harness sets for its hooks. The rest is the machine's.
pub const KEPT: &[&str] = &["PURLIS_", "CHARTER_", "CLAUDE", "CODEX_", "OPENCODE"];

/// One line of the recorder's file: a call as it began, or its reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    /// The call's own id, the same on its two lines.
    pub id: String,
    /// `call` or `reply`.
    pub phase: String,
    /// Milliseconds since the epoch.
    pub at_ms: u64,
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    /// `stall` or `exit2`, on a call the recorder held or answered itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forced: Option<String>,
}

/// What a credential's value is recorded as: never its value. A chat's token is minted afresh
/// for every chat, and a recording holds no credential (the mask never has to hide one).
pub const NOT_RECORDED: &str = "<credential: not recorded>";

/// The endings of a variable's name that hold a credential.
const CREDENTIALS: &[&str] = &["_TOKEN", "_KEY", "_SECRET", "_PASSWORD"];

/// The variables of `env` a recording keeps ([`KEPT`]), the recorder's own left out, each
/// credential by its name alone ([`NOT_RECORDED`]).
pub fn kept(env: impl IntoIterator<Item = (String, String)>) -> BTreeMap<String, String> {
    env.into_iter()
        .filter(|(name, _)| {
            KEPT.iter().any(|prefix| name.starts_with(prefix))
                && ![REAL, OUT, STALL, EXIT2].contains(&name.as_str())
        })
        .map(|(name, value)| {
            if CREDENTIALS.iter().any(|end| name.ends_with(end)) {
                (name, NOT_RECORDED.to_owned())
            } else {
                (name, value)
            }
        })
        .collect()
}

/// Every line of the recorder's file at `path`, in the order they were written.
pub fn read(path: &Path) -> Result<Vec<Line>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("{}: {e}: {l}", path.display())))
        .collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Appends `line` to `out`, holding the file's lock so lines of parallel calls never interleave.
fn append(out: &Path, line: &Line) -> std::io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(out)?;
    file.lock()?;
    let mut text = serde_json::to_string(line).map_err(std::io::Error::other)?;
    text.push('\n');
    file.write_all(text.as_bytes())?;
    file.unlock()
}

/// Whether this is the first call of `word` the recorder does `what` to: a marker beside `out`
/// says it was taken.
fn first(out: &Path, what: &str, word: &str) -> bool {
    let marker = out.with_extension(format!("{what}-{word}"));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(marker)
        .is_ok()
}

/// The recorder's whole run, with `args` its arguments after the program. Returns the exit code
/// to end with; for a command other than `hook` it does not return (the real `purlis` replaces
/// it).
pub fn run(args: Vec<String>) -> i32 {
    let Some(real) = std::env::var_os(REAL).map(PathBuf::from) else {
        let _ = writeln!(std::io::stderr(), "purlis-recorder: {REAL} is not set");
        return 1;
    };
    if args.first().map(String::as_str) != Some("hook") {
        return exec(&real, &args);
    }
    let Some(out) = std::env::var_os(OUT).map(PathBuf::from) else {
        let _ = writeln!(std::io::stderr(), "purlis-recorder: {OUT} is not set");
        return 1;
    };
    let mut stdin = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut stdin);
    let stdin = String::from_utf8_lossy(&stdin).into_owned();
    let id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    );
    // What it does in place of the real hook is decided first, so a call the harness ends
    // while it is held says so in the one line it leaves.
    let word = args.get(1).cloned().unwrap_or_default();
    let forced = if std::env::var(STALL).is_ok_and(|w| w == word) && first(&out, "stall", &word) {
        Some("stall")
    } else if std::env::var(EXIT2).is_ok_and(|w| w == word) && first(&out, "exit2", &word) {
        Some("exit2")
    } else {
        None
    };
    let call = Line {
        id: id.clone(),
        phase: "call".to_owned(),
        at_ms: now_ms(),
        argv: args.clone(),
        stdin: Some(stdin.clone()),
        env: Some(kept(std::env::vars())),
        exit: None,
        stdout: None,
        stderr: None,
        forced: forced.map(str::to_owned),
    };
    if let Err(e) = append(&out, &call) {
        let _ = writeln!(std::io::stderr(), "purlis-recorder: {}: {e}", out.display());
        return 1;
    }
    if forced == Some("stall") {
        std::thread::sleep(HELD);
    }
    let (exit, stdout, stderr) = if forced == Some("exit2") {
        (2, Vec::new(), format!("{EXIT2_REASON}\n").into_bytes())
    } else {
        match answer(&real, &args, &stdin) {
            Ok(answered) => answered,
            Err(e) => (1, Vec::new(), format!("purlis-recorder: {e}").into_bytes()),
        }
    };
    let reply = Line {
        id,
        phase: "reply".to_owned(),
        at_ms: now_ms(),
        argv: args,
        stdin: None,
        env: None,
        exit: Some(exit),
        stdout: Some(String::from_utf8_lossy(&stdout).into_owned()),
        stderr: Some(String::from_utf8_lossy(&stderr).into_owned()),
        forced: forced.map(str::to_owned),
    };
    let _ = append(&out, &reply);
    let _ = std::io::stdout().write_all(&stdout);
    let _ = std::io::stderr().write_all(&stderr);
    exit
}

/// The real `purlis hook`'s answer to `stdin`.
fn answer(real: &Path, args: &[String], stdin: &str) -> std::io::Result<(i32, Vec<u8>, Vec<u8>)> {
    let mut child = Command::new(real)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut pipe) = child.stdin.take() {
        // A hook that does not read its input closes the pipe; what it answers still counts.
        let _ = pipe.write_all(stdin.as_bytes());
    }
    let output = child.wait_with_output()?;
    Ok((
        output.status.code().unwrap_or(-1),
        output.stdout,
        output.stderr,
    ))
}

#[cfg(unix)]
fn exec(real: &Path, args: &[String]) -> i32 {
    use std::os::unix::process::CommandExt;
    let e = Command::new(real).args(args).exec();
    let _ = writeln!(
        std::io::stderr(),
        "purlis-recorder: {}: {e}",
        real.display()
    );
    127
}

#[cfg(not(unix))]
fn exec(real: &Path, args: &[String]) -> i32 {
    Command::new(real)
        .args(args)
        .status()
        .map_or(127, |s| s.code().unwrap_or(1))
}
