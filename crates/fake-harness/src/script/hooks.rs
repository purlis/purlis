// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Running a hook** the way the harness runs it: its command through a shell (Claude Code,
//! Codex) or its program's words without one (OpenCode's shim), the payload on standard input,
//! the chat's environment, and its timeout, after which it is killed and the harness goes on.
//! Every hook of one event starts at once, in the order armed, as the harnesses start them.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::routes::Route;

/// What one hook did.
#[derive(Debug, Clone)]
pub struct Fired {
    pub word: String,
    pub event: String,
    /// `None` when it ran past its timeout and was killed.
    pub exit: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// How a hook's command is started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spawn<'a> {
    /// `/bin/sh -c <command>`.
    Shell,
    /// `<binary> hook <word>`, as OpenCode's shim spawns it.
    Direct(&'a Path),
}

/// Each of `routes` run at once, each with `payload` on its standard input, answered in the
/// order armed.
pub fn run_all(
    routes: &[&Route],
    spawn: Spawn<'_>,
    payload: &Value,
    env: &[(String, String)],
    cwd: &Path,
) -> Vec<Fired> {
    std::thread::scope(|scope| {
        let running: Vec<_> = routes
            .iter()
            .map(|route| {
                let started = start(route, spawn, payload, env, cwd);
                // Started in the order armed: each process is begun before the next.
                scope.spawn(move || finish(route, started))
            })
            .collect();
        running
            .into_iter()
            .map(|r| r.join().unwrap_or_else(|_| panicked()))
            .collect()
    })
}

fn panicked() -> Fired {
    Fired {
        word: String::new(),
        event: String::new(),
        exit: Some(-1),
        stdout: String::new(),
        stderr: "the hook's thread panicked".to_owned(),
    }
}

type Started = Result<std::process::Child, String>;

fn start(
    route: &Route,
    spawn: Spawn<'_>,
    payload: &Value,
    env: &[(String, String)],
    cwd: &Path,
) -> Started {
    let mut command = match spawn {
        Spawn::Shell => {
            let mut c = Command::new("/bin/sh");
            c.args(["-c", &route.command]);
            c
        }
        Spawn::Direct(binary) => {
            let mut c = Command::new(binary);
            c.args(["hook", &route.word]);
            c
        }
    };
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command
        .envs(env.iter().map(|(k, v)| (k, v)))
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start hook {}: {e}", route.word))?;
    if let Some(mut stdin) = child.stdin.take() {
        let text = payload.to_string();
        std::thread::spawn(move || {
            let _ = stdin.write_all(text.as_bytes());
        });
    }
    Ok(child)
}

fn finish(route: &Route, started: Started) -> Fired {
    let mut fired = Fired {
        word: route.word.clone(),
        event: route.event.clone(),
        exit: Some(-1),
        stdout: String::new(),
        stderr: String::new(),
    };
    let mut child = match started {
        Ok(child) => child,
        Err(e) => {
            fired.stderr = e;
            return fired;
        }
    };
    let pipe = |r: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut r) = r {
                let _ = r.read_to_string(&mut text);
            }
            text
        })
    };
    let out = pipe(
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    );
    let err = pipe(
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    );
    let deadline = Instant::now() + Duration::from_secs(route.timeout);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => break None,
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(_) => break None,
        }
    };
    match status {
        Some(status) => {
            fired.exit = Some(status.code().unwrap_or(-1));
            fired.stdout = out.join().unwrap_or_default();
            fired.stderr = err.join().unwrap_or_default();
        }
        None => {
            // Past its timeout: the hook and whatever it started are ended, and what it wrote
            // is not read, as the harness reads nothing of a hook it cut off.
            kill_group(child.id());
            let _ = child.kill();
            let _ = child.wait();
            fired.exit = None;
        }
    }
    fired
}

fn kill_group(pid: u32) {
    let _ = Command::new("kill")
        .args(["-9", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

impl Fired {
    /// Its standard output as JSON, where it was JSON.
    pub fn json(&self) -> Option<Value> {
        let text = self.stdout.trim();
        if text.is_empty() {
            return None;
        }
        serde_json::from_str(text).ok()
    }

    /// The context it asked to add to the conversation: `hookSpecificOutput.additionalContext`
    /// of an answer that exited 0.
    pub fn context(&self) -> Option<String> {
        if self.exit != Some(0) {
            return None;
        }
        self.json()?
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
    }

    /// Why it refused the call or the turn, if it did: exit 2 with its standard error, or a
    /// `deny` (`permissionDecision`) or `block` (`decision`) answer with its reason.
    pub fn refusal(&self) -> Option<Refusal> {
        if self.exit == Some(2) {
            return Some(Refusal {
                word: self.word.clone(),
                reason: self.stderr.trim().to_owned(),
                by_exit_2: true,
            });
        }
        if self.exit != Some(0) {
            return None;
        }
        let answer = self.json()?;
        let specific = answer.get("hookSpecificOutput");
        let denied = specific
            .and_then(|s| s.get("permissionDecision"))
            .and_then(Value::as_str)
            == Some("deny");
        let blocked = answer.get("decision").and_then(Value::as_str) == Some("block");
        if !denied && !blocked {
            return None;
        }
        let reason = specific
            .and_then(|s| s.get("permissionDecisionReason"))
            .or_else(|| answer.get("reason"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        Some(Refusal {
            word: self.word.clone(),
            reason,
            by_exit_2: false,
        })
    }
}

/// A hook's refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub word: String,
    pub reason: String,
    pub by_exit_2: bool,
}
