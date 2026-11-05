// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The check probe: a test-only **Check reporter** (ADR 0114, #5410), proven end to end through
//! purlis's real registry, executor and check store, as `extension-probe` and `studio-probe`
//! prove theirs.
//!
//! **It is never shipped and never installed on anyone's machine.** Written as a stranger's
//! extension would be: it does not link purlis's core, and it knows one thing about purlis, the
//! protocol — one line of JSON on stdin, `{"charter": 5, "check": {"output", "status"}}`, and
//! SARIF 2.1.0 back on stdout, exiting 0.
//!
//! It converts the one format it knows, a line `<file>:<line>: <message>`, into one result each,
//! and adds a result on no file saying the exit status it was handed. Two lines of output make it
//! misbehave on purpose, so a test can prove what purlis does with a bad reporter:
//!
//! - [`ANSWER_GARBAGE`] makes it print something that is not SARIF, and exit 0;
//! - [`EXIT_WITH`] followed by a number makes it exit with that status, printing nothing on stdout.

use serde_json::{Value, json};

/// The protocol this program speaks: 5, which has the Check reporter's question.
pub const PROTOCOL: u64 = 5;

/// The manifest this program is installed with, so `assemble` writes the one that is tested.
pub const MANIFEST: &str = include_str!("../purlis-extension.json");

/// The tool its SARIF names.
pub const TOOL: &str = "probe-check";

/// A line of a check's output that makes it answer something that is not SARIF.
pub const ANSWER_GARBAGE: &str = "check-probe: answer garbage";

/// A line of a check's output, followed by a number, that makes it exit with that status.
pub const EXIT_WITH: &str = "check-probe: exit with ";

/// What the program does with one request line: what it prints on stdout and stderr, and its
/// exit status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Printed {
    pub stdout: String,
    pub stderr: String,
    pub status: u8,
}

/// The answer to one request line. Never panics on what it is given.
pub fn answer(line: &str) -> Printed {
    let failed = |why: String| Printed {
        stdout: String::new(),
        stderr: format!("{why}\n"),
        status: 2,
    };
    let request: Value = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(why) => return failed(format!("the request was not JSON: {why}")),
    };
    let protocol = request.get("charter").and_then(Value::as_u64);
    if protocol != Some(PROTOCOL) {
        return failed(format!(
            "this program speaks protocol {PROTOCOL} and was asked in {protocol:?}"
        ));
    }
    let Some(check) = request.get("check") else {
        return failed("the request is not a check's output".to_owned());
    };
    let output = check.get("output").and_then(Value::as_str).unwrap_or("");
    let Some(status) = check.get("status").and_then(Value::as_i64) else {
        return failed("the request does not say the check's exit status".to_owned());
    };
    if output.lines().any(|line| line == ANSWER_GARBAGE) {
        return Printed {
            stdout: "this is not SARIF\n".to_owned(),
            stderr: String::new(),
            status: 0,
        };
    }
    if let Some(code) = output
        .lines()
        .find_map(|line| line.strip_prefix(EXIT_WITH))
        .and_then(|code| code.trim().parse::<u8>().ok())
    {
        return Printed {
            stdout: String::new(),
            stderr: format!("check-probe was told to exit with {code}\n"),
            status: code,
        };
    }
    Printed {
        stdout: format!("{}\n", sarif(output, status)),
        stderr: String::new(),
        status: 0,
    }
}

/// `output` as SARIF 2.1.0: one result for each `<file>:<line>: <message>` line, and one on no
/// file saying the exit status the check ended with.
pub fn sarif(output: &str, status: i64) -> Value {
    let mut results: Vec<Value> = output
        .lines()
        .filter_map(|line| {
            let (file, rest) = line.split_once(':')?;
            let (at, message) = rest.split_once(": ")?;
            let at: u64 = at.trim().parse().ok()?;
            Some(json!({
                "level": "error",
                "message": { "text": message },
                "locations": [{ "physicalLocation": {
                    "artifactLocation": { "uri": file },
                    "region": { "startLine": at },
                }}],
            }))
        })
        .collect();
    results.push(json!({
        "level": "note",
        "message": { "text": format!("the check exited with status {status}") },
    }));
    json!({
        "version": "2.1.0",
        "runs": [{ "tool": { "driver": { "name": TOOL } }, "results": results }],
    })
}

/// Put this program and its manifest in `dir`, the folder purlis is pointed at.
pub fn assemble(dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    let me = std::env::current_exe()?;
    std::fs::create_dir_all(dir.join("bin"))?;
    std::fs::write(dir.join("purlis-extension.json"), MANIFEST)?;
    let program = dir.join("bin").join("check-probe");
    // A rename over the old copy, never a write through it: on macOS writing a program that has
    // run invalidates its signature (stand-in's notes).
    let beside = dir.join("bin").join(".check-probe.new");
    std::fs::copy(&me, &beside)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&beside, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&beside, &program)?;
    Ok(program)
}
