// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The model transcript** (T1.1-04 item 6): JSONL, one line per thing the model would see, in
//! the order it would see them. Each line has the shape of what T1.1-03's projection keeps of a
//! real harness's requests (`purlis_recordings::projection::Shown`): `kind` (`user_text`,
//! `context` or `tool_result`), `text` as the harness frames it, and a tool result's
//! `tool_use_id` and `is_error`. Beside those, what the line came from: a prompt's `typed`, a
//! tool result's `stdout`, `stderr` and `exit`, the errno of a native call the OS refused
//! (`os_error`), the `hook` whose context or refusal it carries, and a refusal's `reason`.
//!
//! Its file is `--transcript <file>`, or `$FAKE_HARNESS_TRANSCRIPT`; with neither it is not
//! written.

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use serde_json::{Map, Value};

pub struct Transcript {
    file: Option<Mutex<File>>,
}

impl Transcript {
    pub fn open(path: Option<&Path>) -> Result<Transcript, String> {
        let file = match path {
            None => None,
            Some(path) => Some(Mutex::new(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|e| format!("cannot open the transcript {}: {e}", path.display()))?,
            )),
        };
        Ok(Transcript { file })
    }

    fn write(&self, line: Map<String, Value>) {
        let Some(file) = &self.file else {
            return;
        };
        let mut file = file
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = writeln!(file, "{}", Value::Object(line));
        let _ = file.flush();
    }

    /// A prompt, or a line typed into the chat (`typed`).
    pub fn user_text(&self, text: &str, typed: bool) {
        let mut line = base("user_text", text);
        if typed {
            line.insert("typed".to_owned(), Value::Bool(true));
        }
        self.write(line);
    }

    /// Context a hook (or the harness) put in front of the model.
    pub fn context(&self, text: &str, hook: Option<&str>) {
        let mut line = base("context", text);
        if let Some(hook) = hook {
            line.insert("hook".to_owned(), hook.into());
        }
        self.write(line);
    }

    /// A tool's result as the harness hands it to the model.
    pub fn tool_result(&self, result: &ToolResult) {
        let mut line = base("tool_result", &result.text);
        line.insert("tool_use_id".to_owned(), result.id.clone().into());
        if let Some(step) = &result.step {
            line.insert("step".to_owned(), step.clone().into());
        }
        if let Some(is_error) = result.is_error {
            line.insert("is_error".to_owned(), is_error.into());
        }
        if let Some(ran) = &result.ran {
            line.insert("stdout".to_owned(), ran.stdout.clone().into());
            line.insert("stderr".to_owned(), ran.stderr.clone().into());
            line.insert("exit".to_owned(), ran.exit.map_or(Value::Null, Value::from));
        }
        if let Some(os_error) = &result.os_error {
            line.insert("os_error".to_owned(), os_error.clone().into());
        }
        if let Some(refusal) = &result.refused_by {
            line.insert("hook".to_owned(), refusal.word.clone().into());
            line.insert("reason".to_owned(), refusal.reason.clone().into());
            if refusal.by_exit_2 {
                line.insert("hook_exit".to_owned(), 2.into());
            }
        }
        self.write(line);
    }
}

fn base(kind: &str, text: &str) -> Map<String, Value> {
    let mut line = Map::new();
    line.insert("kind".to_owned(), kind.into());
    line.insert("text".to_owned(), text.into());
    line
}

/// What a command printed and how it ended.
#[derive(Debug, Clone, Default)]
pub struct Ran {
    pub stdout: String,
    pub stderr: String,
    /// `None` when it was interrupted.
    pub exit: Option<i32>,
}

/// A tool's result, framed for the model.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub id: String,
    /// The script's step the call came from, where it named one.
    pub step: Option<String>,
    pub text: String,
    pub is_error: Option<bool>,
    pub ran: Option<Ran>,
    pub refused_by: Option<super::hooks::Refusal>,
    /// What the OS said of a native call it refused (`EACCES`), beside what the model is shown.
    pub os_error: Option<String>,
}
