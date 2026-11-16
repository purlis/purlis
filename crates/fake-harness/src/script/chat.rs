// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One scripted chat**: the turns played as the model, each call's hooks fired in the real
//! harness's order with its payloads and environment, its I/O done for real, and what the model
//! would be shown written to the transcript.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::dialect::{Action, Dialect, lower_alnum};
use super::hooks::{self, Fired, Refusal, Spawn};
use super::keyboard::{Key, Keyboard};
use super::launch::Launch;
use super::mcp_call;
use super::routes::{self, Route};
use super::rules::{Refused, Rules};
use super::transcript::{Ran, ToolResult, Transcript};
use super::turns::{Call, Script, Turn};

/// What Claude Code's covering recording ran in, and what it reports where nothing says.
const CLAUDE_PERMISSION_MODE: &str = "default";

/// How long an MCP tool call is waited on.
const MCP_PATIENCE: Duration = Duration::from_secs(60);

pub struct Chat {
    dialect: Dialect,
    binary: PathBuf,
    argv: Vec<String>,
    launch: Launch,
    cwd: PathBuf,
    transcript: Transcript,
    script: Script,
    routes: Vec<Route>,
    rules: Rules,
    session_id: String,
    resumed: bool,
    model: String,
    keyboard: Mutex<Keyboard>,
    /// Claude Code's `prompt_id`, Codex's `turn_id`: one per prompt.
    turn_id: Mutex<Option<String>>,
    reported_start: AtomicBool,
    interrupted: AtomicBool,
    /// Lines typed while a turn ran: the prompts of the turns after it.
    queued: Mutex<Vec<String>>,
    /// Claude Code's task notifications, each with whether a response has passed since it was
    /// raised: it is submitted after the call that follows the sub-agent's end.
    notices: Mutex<Vec<(String, bool)>>,
    /// What the session's start said, waiting to be shown with the first prompt (Claude Code,
    /// after it; OpenCode, inside it).
    start_contexts: Mutex<Vec<String>>,
    /// Claude Code's background tasks, as its hooks are told them.
    background: Mutex<Vec<Value>>,
}

/// Who makes a call: the chat itself, or one of its sub-agents.
#[derive(Debug, Clone)]
struct Agent {
    id: String,
    kind: String,
}

/// The cursor a relaunch continues from: `$FAKE_HARNESS_STATE/<session id>.json`.
pub struct Cursor;

impl Cursor {
    fn file(session: &str) -> Option<PathBuf> {
        let dir = std::env::var_os("FAKE_HARNESS_STATE")?;
        Some(PathBuf::from(dir).join(format!("{session}.json")))
    }

    pub fn next_turn(session: &str) -> usize {
        Self::file(session)
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("next_turn").and_then(Value::as_u64))
            .map_or(0, |n| n as usize)
    }

    pub fn save(session: &str, next: usize) {
        if let Some(file) = Self::file(session) {
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(
                file,
                json!({"session": session, "next_turn": next}).to_string(),
            );
        }
    }
}

impl Chat {
    pub fn new(
        dialect: Dialect,
        binary: PathBuf,
        argv: Vec<String>,
        launch: Launch,
        cwd: PathBuf,
        transcript: Transcript,
        script: Script,
    ) -> Result<Chat, String> {
        let routes = routes::of(dialect, &binary, &argv);
        let rules = if dialect == Dialect::Claude {
            Rules::of_settings(launch.settings.as_deref(), &cwd)
        } else {
            Rules::default()
        };
        let resumed = launch.resume.is_some();
        let session_id = launch
            .resume
            .clone()
            .or_else(|| launch.session_id.clone())
            .unwrap_or_else(|| dialect.new_session_id());
        let model = launch
            .model
            .clone()
            .unwrap_or_else(|| dialect.default_model().to_owned());
        Ok(Chat {
            dialect,
            binary,
            argv,
            launch,
            cwd,
            transcript,
            script,
            routes,
            rules,
            session_id,
            resumed,
            model,
            keyboard: Mutex::new(Keyboard::start()),
            turn_id: Mutex::new(None),
            reported_start: AtomicBool::new(false),
            interrupted: AtomicBool::new(false),
            queued: Mutex::new(Vec::new()),
            notices: Mutex::new(Vec::new()),
            start_contexts: Mutex::new(Vec::new()),
            background: Mutex::new(Vec::new()),
        })
    }

    /// The whole session: its start, each turn as its prompt comes, and its end.
    pub fn play(self) -> Result<u8, String> {
        // The session id, as the pane shows it, for a person or a test to read.
        println!("{} session {}\r", self.dialect.name(), self.session_id);
        let mut next = if self.resumed {
            Cursor::next_turn(&self.session_id)
        } else {
            0
        };
        if self.dialect.starts_before_the_first_prompt() {
            self.session_start();
        }
        let mut first = self.launch.prompt.clone();
        loop {
            let prompt = match first.take() {
                Some(p) => Some((p, false)),
                None => self.next_prompt(),
            };
            let Some((text, typed)) = prompt else { break };
            let scripted = self.script.turns.get(next).cloned();
            let turn = scripted.clone().unwrap_or_else(|| Turn {
                prompt: text.clone(),
                calls: Vec::new(),
                says: None,
            });
            self.turn(&turn, &text, typed);
            if scripted.is_some() {
                next += 1;
                Cursor::save(&self.session_id, next);
            }
            if self.keyboard().ended() && self.queued_is_empty() {
                break;
            }
        }
        self.session_end();
        Ok(0)
    }

    fn keyboard(&self) -> std::sync::MutexGuard<'_, Keyboard> {
        self.keyboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn queued_is_empty(&self) -> bool {
        self.queued.lock().map(|q| q.is_empty()).unwrap_or(true)
    }

    /// The next prompt: one typed while the last turn ran, else the next line typed.
    fn next_prompt(&self) -> Option<(String, bool)> {
        if let Ok(mut queued) = self.queued.lock()
            && !queued.is_empty()
        {
            return Some((queued.remove(0), true));
        }
        loop {
            match self.keyboard().next(None)? {
                Key::Line(line) => return Some((line, true)),
                Key::Esc => continue,
                Key::End => return None,
            }
        }
    }

    /// Esc typed since the last look; a line typed meanwhile waits as the next prompt.
    fn esc_pressed(&self) -> bool {
        if self.interrupted.load(Ordering::SeqCst) {
            return true;
        }
        let mut lines = Vec::new();
        let escaped = self.keyboard().escaped(&mut lines);
        if let Ok(mut q) = self.queued.lock() {
            q.extend(lines);
        }
        if escaped {
            self.interrupted.store(true, Ordering::SeqCst);
        }
        escaped
    }

    // ---- the session ----------------------------------------------------------------------

    fn session_start(&self) {
        if self.reported_start.swap(true, Ordering::SeqCst) {
            return;
        }
        let source = if self.resumed { "resume" } else { "startup" };
        let fired = self.fire("SessionStart", None, json!({"source": source}), None);
        for f in &fired {
            if let Some(mut context) = f.context() {
                match self.dialect {
                    Dialect::Codex => {
                        context = cut(&context, purlis_core::harness::codex::HOOK_OUTPUT_CUT);
                        self.transcript.context(&context, Some(&f.word));
                    }
                    // Claude Code shows it after the first prompt, saying whose it is; OpenCode's
                    // shim puts it on the first prompt.
                    Dialect::Claude | Dialect::Opencode => self
                        .start_contexts
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push(context),
                }
            }
        }
    }

    fn take_start_contexts(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .start_contexts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    fn session_end(&self) {
        if self.dialect != Dialect::Opencode {
            self.fire("SessionEnd", None, json!({"reason": "other"}), None);
        }
    }

    fn turn(&self, turn: &Turn, prompt: &str, typed: bool) {
        self.interrupted.store(false, Ordering::SeqCst);
        *self
            .turn_id
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(super::dialect::uuid());
        if self.dialect == Dialect::Codex && !self.reported_start.load(Ordering::SeqCst) {
            self.transcript.user_text(&self.codex_environment(), false);
        }
        if !self.dialect.starts_before_the_first_prompt() {
            self.session_start();
        }
        let fired = self.fire("UserPromptSubmit", None, json!({"prompt": prompt}), None);
        let contexts: Vec<String> = fired.iter().filter_map(Fired::context).collect();
        if let Some(refusal) = fired.iter().find_map(Fired::refusal) {
            // The prompt is erased, and the reason is the person's, not the model's.
            let _ = refusal;
            return;
        }
        match self.dialect {
            // OpenCode's shim rides what the hooks say on the prompt, as one more text part.
            Dialect::Opencode => {
                let mut notes = self.take_start_contexts();
                notes.extend(contexts);
                let notes = notes.join("\n\n");
                let text = if notes.is_empty() {
                    prompt.to_owned()
                } else {
                    format!("{prompt}\n{notes}")
                };
                self.transcript.user_text(&text, typed);
            }
            _ => {
                for c in &contexts {
                    self.transcript.context(c, Some("userpromptsubmit"));
                }
                self.transcript.user_text(prompt, typed);
                for c in self.take_start_contexts() {
                    self.transcript.context(
                        &format!("SessionStart hook additional context: {c}"),
                        Some("sessionstart"),
                    );
                }
            }
        }
        for response in Call::responses(&turn.calls) {
            if self.esc_pressed() {
                break;
            }
            self.response(&response, None);
            self.deliver_notices(false);
        }
        let interrupted = self.interrupted.load(Ordering::SeqCst);
        if !interrupted {
            self.deliver_notices(true);
        }
        if !interrupted || self.dialect == Dialect::Opencode {
            let mut stop = Map::new();
            stop.insert("stop_hook_active".to_owned(), false.into());
            if let Some(says) = &turn.says {
                stop.insert("last_assistant_message".to_owned(), says.clone().into());
            }
            if self.dialect == Dialect::Claude {
                stop.insert(
                    "background_tasks".to_owned(),
                    Value::Array(self.lock_background().clone()),
                );
                stop.insert("session_crons".to_owned(), json!([]));
            }
            let fired = self.fire("Stop", None, Value::Object(stop), None);
            for f in &fired {
                if let Some(refusal) = f.refusal() {
                    self.transcript.context(&refusal.reason, Some(&f.word));
                }
            }
        }
    }

    /// Claude Code's task notifications, each submitted as a prompt of its own once one more
    /// response has passed since its sub-agent ended, as Claude Code 2.1.325 submits it; at the
    /// turn's end, every one still waiting (`all`).
    fn deliver_notices(&self, all: bool) {
        let ready: Vec<String> = {
            let mut notices = self
                .notices
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let (ready, waiting): (Vec<_>, Vec<_>) =
                notices.drain(..).partition(|(_, ripe)| *ripe || all);
            *notices = waiting.into_iter().map(|(n, _)| (n, true)).collect();
            ready.into_iter().map(|(n, _)| n).collect()
        };
        for notice in ready {
            self.fire("UserPromptSubmit", None, json!({"prompt": notice}), None);
        }
    }

    // ---- calls ----------------------------------------------------------------------------

    /// One model response: its calls, at once where it asked for them at once.
    fn response(&self, calls: &[&Call], agent: Option<&Agent>) {
        if calls.len() == 1 {
            self.call(calls[0], agent);
            return;
        }
        // Each call's guard in order, then the calls together, then each one's result.
        let checked: Vec<Option<Checked>> = calls.iter().map(|c| self.check(c, agent)).collect();
        let outcomes: Vec<Option<Outcome>> = std::thread::scope(|scope| {
            let running: Vec<_> = checked
                .iter()
                .map(|c| c.as_ref().map(|c| scope.spawn(|| self.perform(c, agent))))
                .collect();
            running
                .into_iter()
                .map(|r| r.map(|h| h.join().unwrap_or(Outcome::Interrupted)))
                .collect()
        });
        for (c, outcome) in checked.iter().zip(outcomes) {
            if let (Some(c), Some(outcome)) = (c, outcome) {
                self.after(c, outcome, agent);
            }
        }
    }

    fn call(&self, call: &Call, agent: Option<&Agent>) {
        let Some(checked) = self.check(call, agent) else {
            return;
        };
        if checked.action == Action::Subagent {
            self.subagent(&checked, call);
            return;
        }
        let outcome = self.perform(&checked, agent);
        self.after(&checked, outcome, agent);
    }

    /// The call as the model asked for it, held to Claude Code's deny rules and to purlis's
    /// guard: `None` where it is refused, its result already told.
    fn check(&self, call: &Call, agent: Option<&Agent>) -> Option<Checked> {
        let id = self.dialect.new_call_id();
        let Some((tool, input)) = self.dialect.tool_of(call) else {
            self.transcript.tool_result(&ToolResult {
                id,
                step: call.step.clone(),
                text: format!(
                    "unsupported call: {} offers no {} tool",
                    self.dialect.name(),
                    call.tool
                ),
                is_error: Some(true),
                ran: None,
                refused_by: None,
            });
            return None;
        };
        let action = self.dialect.action_of(&tool, &input);
        let checked = Checked {
            id,
            step: call.step.clone(),
            tool,
            input,
            action,
            background: call.background,
        };
        if let Some(refused) = self.claude_rules(&checked) {
            self.transcript.tool_result(&refused);
            return None;
        }
        let (hook_tool, hook_input) = self.dialect.hook_tool(&checked.tool, &checked.input);
        let fired = self.fire(
            "PreToolUse",
            Some(&checked.tool),
            if self.dialect == Dialect::Opencode {
                json!({"tool_name": hook_tool, "tool_input": hook_input})
            } else {
                json!({"tool_name": hook_tool, "tool_input": hook_input, "tool_use_id": checked.id})
            },
            agent,
        );
        if let Some(refusal) = self.refusal(&fired, "PreToolUse") {
            let text = self.frame_refusal(&checked, &refusal);
            self.transcript.tool_result(&ToolResult {
                id: checked.id.clone(),
                step: checked.step.clone(),
                text,
                is_error: self.error_flag(true),
                ran: None,
                refused_by: Some(refusal),
            });
            return None;
        }
        if self.dialect == Dialect::Opencode && !self.opencode_asks(&checked) {
            self.transcript.tool_result(&ToolResult {
                id: checked.id.clone(),
                step: checked.step.clone(),
                text: "The user rejected permission to use this specific tool call.".to_owned(),
                is_error: None,
                ran: None,
                refused_by: None,
            });
            return None;
        }
        Some(checked)
    }

    /// Claude Code's `permissions.deny`, applied before any hook and any I/O.
    fn claude_rules(&self, c: &Checked) -> Option<ToolResult> {
        if self.dialect != Dialect::Claude {
            return None;
        }
        let path = match &c.action {
            Action::Write { path, .. } | Action::Edit { path, .. } | Action::Read(path) => {
                Some(path.as_str())
            }
            _ => None,
        };
        let text = match self.rules.refuses(&c.tool, path)? {
            Refused::NoSuchTool => format!(
                "<tool_use_error>Error: No such tool available: {0}. {0} is disabled for this session, in subagents as well as here.</tool_use_error>",
                c.tool
            ),
            Refused::Path => "<tool_use_error>File is in a directory that is denied by your permission settings.</tool_use_error>".to_owned(),
        };
        Some(ToolResult {
            id: c.id.clone(),
            step: c.step.clone(),
            text,
            is_error: Some(true),
            ran: None,
            refused_by: None,
        })
    }

    /// Whether the hooks refused, as the dialect reads their answers. OpenCode's shim refuses
    /// on a guard that could not answer too; Claude Code and Codex go on.
    fn refusal(&self, fired: &[Fired], event: &str) -> Option<Refusal> {
        if let Some(r) = fired.iter().find_map(Fired::refusal) {
            return Some(r);
        }
        if self.dialect != Dialect::Opencode || event != "PreToolUse" {
            return None;
        }
        fired.iter().find_map(|f| match f.exit {
            Some(0) if f.json().is_none() && !f.stdout.trim().is_empty() => Some(Refusal {
                word: f.word.clone(),
                reason: "purlis's guard answered something it cannot have meant, so this tool call is refused".to_owned(),
                by_exit_2: false,
            }),
            Some(0) => None,
            exit => {
                let err = f.stderr.trim();
                let why = match exit {
                    None => format!("no answer within {}s", self.timeout_of(&f.word)),
                    Some(code) if err.is_empty() => format!("exit {code}"),
                    Some(_) => err.to_owned(),
                };
                Some(Refusal {
                    word: f.word.clone(),
                    reason: format!(
                        "purlis's guard could not answer ({why}), and a guard that could not answer does not allow"
                    ),
                    by_exit_2: false,
                })
            }
        })
    }

    fn timeout_of(&self, word: &str) -> u64 {
        self.routes
            .iter()
            .find(|r| r.word == word)
            .map_or(0, |r| r.timeout)
    }

    /// OpenCode's own ask for a file outside the chat's folder (`external_directory`): its
    /// Notification and purlis's permission hook at once. Allowed once, as the recording's
    /// person allowed it, unless the hook answers `reject`.
    fn opencode_asks(&self, c: &Checked) -> bool {
        let path = match &c.action {
            Action::Write { path, .. } | Action::Edit { path, .. } | Action::Read(path) => path,
            _ => return true,
        };
        let path = Path::new(path);
        if path.starts_with(&self.cwd) {
            return true;
        }
        let parent = path.parent().unwrap_or(path).display().to_string();
        let glob = format!("{parent}/*");
        let notification = self.routes_for("Notification", None);
        let permission = self.routes_for("PermissionRequest", None);
        let mut both: Vec<&Route> = notification;
        both.extend(permission);
        let sid = self.session_id.clone();
        let ask = json!({
            "id": format!("per_{}", super::dialect::alnum(26)),
            "sessionID": sid,
            "permission": "external_directory",
            "patterns": [glob],
            "metadata": {"filepath": path.display().to_string(), "parentDir": parent},
            "always": [glob],
            "tool": {"messageID": format!("msg_{}", super::dialect::alnum(26)), "callID": c.id},
        });
        let note = self.payload(
            "Notification",
            json!({"notification_type": "permission_prompt", "message": "opencode asks permission to use external_directory"}),
            None,
        );
        // Each hook gets its own payload: the notification's, and the ask as OpenCode raised it.
        let env = self.env("Notification", None);
        let fired: Vec<Fired> = std::thread::scope(|scope| {
            let handles: Vec<_> = both
                .iter()
                .map(|route| {
                    let payload = if route.event == "Notification" {
                        &note
                    } else {
                        &ask
                    };
                    let env = &env;
                    scope.spawn(move || {
                        hooks::run_all(
                            &[route],
                            Spawn::Direct(&self.binary),
                            payload,
                            env,
                            &self.cwd,
                        )
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap_or_default())
                .collect()
        });
        !fired.iter().any(|f| {
            f.event == "PermissionRequest"
                && f.json()
                    .and_then(|j| j.get("reply").and_then(Value::as_str).map(str::to_owned))
                    == Some("reject".to_owned())
        })
    }

    /// The call's I/O, done for real.
    fn perform(&self, c: &Checked, _agent: Option<&Agent>) -> Outcome {
        match &c.action {
            Action::Shell(command) if c.background && self.dialect == Dialect::Claude => {
                self.background_shell(command, &c.input)
            }
            Action::Shell(command) => self.shell(command),
            Action::Write { path, content } => Outcome::File(self.write(path, content)),
            Action::Edit { path, old, new } => Outcome::File(self.edit(path, old, new)),
            Action::Read(path) => {
                Outcome::Read(std::fs::read_to_string(path).map_err(|e| (e, path.clone())))
            }
            Action::Web(url) => Outcome::Web(dial(url)),
            Action::Mcp { server, tool } => {
                let Some(entry) = mcp_call::server(self.dialect, server, &self.argv) else {
                    return Outcome::Mcp(Err(format!("no MCP server named {server}")));
                };
                Outcome::Mcp(mcp_call::call(&entry, tool, &c.input, MCP_PATIENCE))
            }
            Action::Subagent | Action::Unknown => Outcome::Unknown,
        }
    }

    fn shell(&self, command: &str) -> Outcome {
        let mut cmd = Command::new(shell_program());
        cmd.args(["-c", command])
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if self.dialect == Dialect::Opencode {
            // The shim's `shell.env`: the session, in every shell a tool opens.
            cmd.env("PURLIS_SESSION_ID", &self.session_id)
                .env("CHARTER_SESSION_ID", &self.session_id);
        }
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        let started = Instant::now();
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Outcome::Shell(
                    Ran {
                        stdout: String::new(),
                        stderr: format!("cannot run the shell: {e}"),
                        exit: Some(127),
                    },
                    started.elapsed(),
                );
            }
        };
        let read = |r: Option<Box<dyn std::io::Read + Send>>| {
            std::thread::spawn(move || {
                let mut s = String::new();
                if let Some(mut r) = r {
                    let _ = r.read_to_string(&mut s);
                }
                s
            })
        };
        let out = read(
            child
                .stdout
                .take()
                .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        );
        let err = read(
            child
                .stderr
                .take()
                .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        );
        let exit = loop {
            if let Ok(Some(status)) = child.try_wait() {
                break Some(status.code().unwrap_or(-1));
            }
            if self.esc_pressed() {
                let _ = Command::new("kill")
                    .args(["-9", &format!("-{}", child.id())])
                    .stderr(Stdio::null())
                    .status();
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let ran = Ran {
            stdout: out.join().unwrap_or_default(),
            stderr: err.join().unwrap_or_default(),
            exit,
        };
        if exit.is_none() {
            return Outcome::Interrupted;
        }
        Outcome::Shell(ran, started.elapsed())
    }

    fn background_shell(&self, command: &str, input: &Value) -> Outcome {
        let id = lower_alnum(9);
        let file = self.tasks_dir().join(format!("{id}.output"));
        let _ = std::fs::create_dir_all(self.tasks_dir());
        let spawned = std::fs::File::create(&file).and_then(|out| {
            let err = out.try_clone()?;
            Command::new(shell_program())
                .args(["-c", command])
                .current_dir(&self.cwd)
                .stdin(Stdio::null())
                .stdout(out)
                .stderr(err)
                .spawn()
        });
        if let Err(e) = spawned {
            return Outcome::Shell(
                Ran {
                    stdout: String::new(),
                    stderr: e.to_string(),
                    exit: Some(1),
                },
                Duration::ZERO,
            );
        }
        self.lock_background().push(json!({
            "id": id,
            "type": "shell",
            "status": "running",
            "description": input.get("description").cloned().unwrap_or(Value::Null),
            "command": command,
        }));
        Outcome::Background { id, file }
    }

    fn lock_background(&self) -> std::sync::MutexGuard<'_, Vec<Value>> {
        self.background
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Claude Code's folder for a session's task output.
    fn tasks_dir(&self) -> PathBuf {
        let tmp = std::env::var_os("CLAUDE_CODE_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        tmp.join(format!("claude-{}", uid()))
            .join(dashed(&self.cwd))
            .join(&self.session_id)
            .join("tasks")
    }

    /// A file written as the dialect writes one: Claude Code by a temporary file beside it,
    /// renamed into place.
    fn write(&self, path: &str, content: &str) -> Result<String, (std::io::Error, String)> {
        if self.dialect == Dialect::Claude {
            let tmp = format!("{path}.tmp.{}.{}", std::process::id(), hex(12));
            std::fs::write(&tmp, content).map_err(|e| (e, tmp.clone()))?;
            std::fs::rename(&tmp, path).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                (e, path.to_owned())
            })?;
            return Ok(path.to_owned());
        }
        std::fs::write(path, content).map_err(|e| (e, path.to_owned()))?;
        Ok(path.to_owned())
    }

    fn edit(&self, path: &str, old: &str, new: &str) -> Result<String, (std::io::Error, String)> {
        let before = std::fs::read_to_string(path).map_err(|e| (e, path.to_owned()))?;
        if !before.contains(old) {
            return Err((
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "String to replace not found in file.",
                ),
                path.to_owned(),
            ));
        }
        self.write(path, &before.replacen(old, new, 1))
    }

    /// The call's result: its hooks after, and what the model is shown.
    fn after(&self, c: &Checked, outcome: Outcome, agent: Option<&Agent>) {
        let (hook_tool, hook_input) = self.dialect.hook_tool(&c.tool, &c.input);
        let framed = self.frame(c, &outcome);
        // OpenCode goes idle as Esc aborts the command, before the command's own end is heard
        // (1.19.5: Stop, then the aborted call's PostToolUse, then Stop again).
        if outcome == Outcome::Interrupted && self.dialect == Dialect::Opencode {
            self.fire("Stop", None, json!({}), None);
        }
        if let Some((event, field, value)) = &framed.post {
            let mut payload = json!({"tool_name": hook_tool, "tool_input": hook_input});
            payload[*field] = value.clone();
            if self.dialect != Dialect::Opencode {
                payload["tool_use_id"] = c.id.clone().into();
            }
            if let Some(extra) = &framed.post_extra {
                for (k, v) in extra {
                    payload[k] = v.clone();
                }
            }
            let fired = self.fire(event, Some(&c.tool), payload, agent);
            let mut text = framed.text.clone();
            for f in &fired {
                if let Some(context) = f.context() {
                    if self.dialect == Dialect::Opencode {
                        if purlis_core::opencode::EFFECTFUL.contains(&c.tool.as_str()) {
                            text =
                                format!("{text}\n\n--- purlis ---\n{context}\n--- end purlis ---");
                        }
                    } else {
                        self.transcript.context(&context, Some(&f.word));
                    }
                }
            }
            if outcome != Outcome::Interrupted {
                self.transcript.tool_result(&ToolResult {
                    id: c.id.clone(),
                    step: c.step.clone(),
                    text,
                    is_error: framed.is_error,
                    ran: framed.ran,
                    refused_by: None,
                });
            }
            return;
        }
        if outcome != Outcome::Interrupted {
            self.transcript.tool_result(&ToolResult {
                id: c.id.clone(),
                step: c.step.clone(),
                text: framed.text,
                is_error: framed.is_error,
                ran: framed.ran,
                refused_by: None,
            });
        }
    }

    fn error_flag(&self, error: bool) -> Option<bool> {
        (self.dialect == Dialect::Claude).then_some(error)
    }

    /// What the model is shown of a refusal by purlis's guard, as the dialect frames it.
    fn frame_refusal(&self, c: &Checked, r: &Refusal) -> String {
        match self.dialect {
            Dialect::Claude if r.by_exit_2 => {
                let route = self
                    .routes
                    .iter()
                    .find(|route| route.word == r.word && route.event == "PreToolUse");
                let command = route.map_or_else(String::new, |route| route.command.clone());
                format!(
                    "PreToolUse:{} hook error: [{command}]: {}\n\nThis hook comes from the {} plugin.",
                    c.tool,
                    r.reason,
                    purlis_core::plugin::LOADED_AS
                )
            }
            Dialect::Claude => r.reason.clone(),
            Dialect::Codex => {
                let command = match &c.action {
                    Action::Shell(cmd) => cmd.clone(),
                    _ => c.input.as_str().unwrap_or_default().to_owned(),
                };
                format!(
                    "Command blocked by PreToolUse hook: {}. Command: {command}",
                    r.reason
                )
            }
            Dialect::Opencode => {
                if r.reason.is_empty() {
                    "purlis refused this tool call".to_owned()
                } else {
                    r.reason.clone()
                }
            }
        }
    }

    fn frame(&self, c: &Checked, outcome: &Outcome) -> Framed {
        let mut f = Framed::default();
        let d = self.dialect;
        match (d, outcome) {
            (_, Outcome::Interrupted) => {
                if d == Dialect::Opencode && matches!(c.action, Action::Shell(_)) {
                    f.post = Some((
                        "PostToolUse",
                        "tool_response",
                        "(no output)\n\n<shell_metadata>\nUser aborted the command\n</shell_metadata>".into(),
                    ));
                }
            }
            (Dialect::Claude, Outcome::Shell(ran, took)) => {
                let out = ran.stdout.trim_end_matches('\n');
                let err = ran.stderr.trim_end_matches('\n');
                f.ran = Some(ran.clone());
                if ran.exit == Some(0) {
                    f.text = [out, err]
                        .iter()
                        .filter(|s| !s.is_empty())
                        .copied()
                        .collect::<Vec<_>>()
                        .join("\n");
                    f.is_error = Some(false);
                    f.post = Some((
                        "PostToolUse",
                        "tool_response",
                        json!({"stdout": out, "stderr": err, "interrupted": false, "isImage": false, "noOutputExpected": false}),
                    ));
                } else {
                    let both = [err, out]
                        .iter()
                        .filter(|s| !s.is_empty())
                        .copied()
                        .collect::<Vec<_>>()
                        .join("\n");
                    f.text = format!("Exit code {}\n{both}", ran.exit.unwrap_or(-1));
                    f.is_error = Some(true);
                    f.post = Some(("PostToolUseFailure", "error", f.text.clone().into()));
                    f.post_extra = Some(vec![("is_interrupt".to_owned(), false.into())]);
                }
                f.push_duration(*took);
            }
            (Dialect::Claude, Outcome::Background { id, file }) => {
                f.text = format!(
                    "Command running in background with ID: {id}. Output is being written to: {}. You will be notified when it completes \u{2014} do not poll that file or sleep while waiting. Read the file only if the user asks for progress, or if you need a line it prints before you can go on, such as a server's ready line or port.",
                    file.display()
                );
                f.is_error = Some(false);
                f.post = Some((
                    "PostToolUse",
                    "tool_response",
                    json!({"stdout": "", "stderr": "", "interrupted": false, "isImage": false, "noOutputExpected": false, "backgroundTaskId": id}),
                ));
                f.push_duration(Duration::ZERO);
            }
            (Dialect::Codex, Outcome::Shell(ran, took)) => {
                let output = format!("{}{}", ran.stdout, ran.stderr);
                f.ran = Some(ran.clone());
                f.text = format!(
                    "Chunk ID: {}\nWall time: {:.4} seconds\nProcess exited with code {}\nOriginal token count: {}\nOutput:\n{output}",
                    lower_alnum(6),
                    took.as_secs_f64(),
                    ran.exit.unwrap_or(-1),
                    output.len().div_ceil(4)
                );
                f.post = Some(("PostToolUse", "tool_response", output.into()));
            }
            (Dialect::Opencode, Outcome::Shell(ran, _)) => {
                let output = format!("{}{}", ran.stdout, ran.stderr);
                f.ran = Some(ran.clone());
                f.text.clone_from(&output);
                let response = if ran.exit.is_some_and(|e| e != 0) {
                    json!({"stderr": output, "mixed": true})
                } else {
                    output.into()
                };
                f.post = Some(("PostToolUse", "tool_response", response));
                f.post_extra = ran.exit.map(|e| vec![("exit_code".to_owned(), e.into())]);
            }
            (_, Outcome::File(result)) => {
                let is_write = matches!(c.action, Action::Write { .. });
                match (d, result) {
                    (Dialect::Claude, Ok(path)) => {
                        f.text = if is_write {
                            format!("File created successfully at: {path}")
                        } else {
                            format!("The file {path} has been updated successfully.")
                        };
                        f.is_error = Some(false);
                        f.post = Some((
                            "PostToolUse",
                            "tool_response",
                            json!({"filePath": path, "type": if is_write {"create"} else {"update"}}),
                        ));
                    }
                    (Dialect::Claude, Err((e, path))) => {
                        f.text = node_error(e, path);
                        f.is_error = Some(true);
                    }
                    (Dialect::Codex, Ok(path)) => {
                        let mark = if is_write { "A" } else { "M" };
                        f.text = format!(
                            "Exit code: 0\nWall time: 0.0000 seconds\nOutput:\nSuccess. Updated the following files:\n{mark} {path}\n"
                        );
                        f.post = Some(("PostToolUse", "tool_response", f.text.clone().into()));
                    }
                    (Dialect::Codex, Err((_, path))) => {
                        f.text = format!(
                            "Exit code: 1\nWall time: 0.0000 seconds\nOutput:\nFailed to write file {}\n",
                            strip_tmp(path)
                        );
                    }
                    (Dialect::Opencode, Ok(_)) => {
                        f.text = if is_write {
                            "Wrote file successfully."
                        } else {
                            "Edit applied successfully."
                        }
                        .to_owned();
                        f.post = Some(("PostToolUse", "tool_response", json!({"output": f.text})));
                    }
                    (Dialect::Opencode, Err((_, path))) => {
                        f.text = format!(
                            "PermissionDenied: FileSystem.writeFile ({})",
                            strip_tmp(path)
                        );
                    }
                }
            }
            (_, Outcome::Read(result)) => match result {
                Ok(text) => {
                    f.text = text
                        .lines()
                        .enumerate()
                        .map(|(n, l)| format!("{:>6}\t{l}", n + 1))
                        .collect::<Vec<_>>()
                        .join("\n");
                    f.is_error = self.error_flag(false);
                    f.post = Some(("PostToolUse", "tool_response", json!({"output": text})));
                }
                Err((e, path)) => {
                    f.text = node_error(e, path);
                    f.is_error = self.error_flag(true);
                }
            },
            (_, Outcome::Web(result)) => {
                let url = match &c.action {
                    Action::Web(url) => url.clone(),
                    _ => String::new(),
                };
                match (d, result) {
                    (Dialect::Opencode, Ok(code)) if !(200..300).contains(code) => {
                        f.text = format!("StatusCode: non 2xx status code ({code} GET {url})");
                    }
                    (Dialect::Opencode, Err(_)) => {
                        f.text = format!("Transport error (GET {url})");
                    }
                    (_, Ok(code)) if (200..300).contains(code) => {
                        f.text = format!("{code} GET {url}");
                        f.is_error = self.error_flag(false);
                    }
                    (_, Ok(code)) => {
                        f.text = format!("Request failed with status code {code}");
                        f.is_error = self.error_flag(true);
                    }
                    (_, Err(e)) => {
                        f.text = e.clone();
                        f.is_error = self.error_flag(true);
                    }
                }
            }
            (_, Outcome::Mcp(result)) => match result {
                Ok(answered) => {
                    f.text = answered.text.clone();
                    f.is_error = self.error_flag(answered.is_error);
                    let event = if answered.is_error {
                        "PostToolUseFailure"
                    } else {
                        "PostToolUse"
                    };
                    if answered.is_error && d == Dialect::Claude {
                        f.post = Some((event, "error", answered.text.clone().into()));
                    } else if !answered.is_error {
                        f.post = Some((
                            event,
                            "tool_response",
                            json!([{"type": "text", "text": answered.text}]),
                        ));
                    }
                }
                Err(e) => {
                    f.text = e.clone();
                    f.is_error = self.error_flag(true);
                }
            },
            (_, Outcome::Background { .. }) | (_, Outcome::Unknown) => {
                f.text = format!("unsupported call: {}", c.tool);
                f.is_error = self.error_flag(true);
            }
        }
        f
    }

    /// A sub-agent: Claude Code's runs in the background and is told back as a notification;
    /// OpenCode's runs while its call waits.
    fn subagent(&self, c: &Checked, call: &Call) {
        let prompt = c
            .input
            .get("prompt")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let description = c
            .input
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let says = call.says.clone().unwrap_or_default();
        match self.dialect {
            Dialect::Claude => {
                let agent_id = lower_alnum(17);
                let kind = c
                    .input
                    .get("subagent_type")
                    .and_then(Value::as_str)
                    .unwrap_or("general-purpose")
                    .to_owned();
                let file = self.tasks_dir().join(format!("{agent_id}.output"));
                self.fire(
                    "PostToolUse",
                    Some(&c.tool),
                    json!({
                        "tool_name": c.tool, "tool_input": c.input, "tool_use_id": c.id,
                        "tool_response": {
                            "isAsync": true, "status": "async_launched", "agentId": agent_id,
                            "description": description, "resolvedModel": self.model, "prompt": prompt,
                            "outputFile": file.display().to_string(), "canReadOutputFile": true,
                            "canContinueAgent": true
                        },
                        "duration_ms": 0
                    }),
                    None,
                );
                self.lock_background().push(json!({
                    "id": agent_id, "type": "subagent", "status": "running",
                    "description": description, "agent_type": kind,
                }));
                let started = Instant::now();
                self.transcript.user_text(&prompt, false);
                let agent = Agent {
                    id: agent_id.clone(),
                    kind: kind.clone(),
                };
                for response in Call::responses(&call.calls) {
                    self.response(&response, Some(&agent));
                }
                self.fire(
                    "SubagentStop",
                    None,
                    json!({
                        "stop_hook_active": false,
                        "agent_transcript_path": self.transcript_path().with_extension("").join("subagents").join(format!("agent-{agent_id}.jsonl")).display().to_string(),
                        "last_assistant_message": says,
                        "background_tasks": *self.lock_background(),
                        "session_crons": [],
                    }),
                    Some(&agent),
                );
                self.lock_background()
                    .retain(|t| t["id"] != agent_id.as_str());
                self.transcript.tool_result(&ToolResult {
                    id: c.id.clone(),
                    step: c.step.clone(),
                    text: format!(
                        "Async agent launched successfully. (This tool result is internal metadata \u{2014} never quote or paste any part of it, including the agentId below, into a user-facing reply.)\nagentId: {agent_id} (internal ID - do not mention to user. Use SendMessage with to: '{agent_id}', summary: '<5-10 word recap>' to continue this agent.)\nThe agent is working in the background. You will be notified automatically when it completes. You know nothing about its results until that notification arrives \u{2014} do not report, assume, or predict them; continue other work or respond to the user in the meantime.\nDo not duplicate this agent's work \u{2014} avoid working with the same files or topics it is using.\noutput_file: {}\nDo NOT Read or tail this file via the shell tool \u{2014} it is the full subagent JSONL transcript and reading it will overflow your context. If the user asks for progress, say the agent is still running; you'll get a completion notification.",
                        file.display()
                    ),
                    // Claude Code 2.1.325 sends this one result with no `is_error`.
                    is_error: None,
                    ran: None,
                    refused_by: None,
                });
                self.notices
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push((format!(
                        "<task-notification>\n<task-id>{agent_id}</task-id>\n<tool-use-id>{}</tool-use-id>\n<output-file>{}</output-file>\n<status>completed</status>\n<summary>Agent \"{description}\" finished</summary>\n<note>A task-notification fires each time this agent stops with no live background children of its own. The user can send it another message and resume it, so the same task-id may notify more than once.</note>\n<result>{says}</result>\n<usage><subagent_tokens>{}</subagent_tokens><tool_uses>{}</tool_uses><duration_ms>{}</duration_ms></usage>\n</task-notification>",
                        c.id,
                        file.display(),
                        call.tokens.unwrap_or_else(|| says.len().div_ceil(4) as u64),
                        call.calls.len(),
                        started.elapsed().as_millis()
                    ), false));
            }
            Dialect::Opencode => {
                let child = self.dialect.new_session_id();
                self.transcript.user_text(&prompt, false);
                for response in Call::responses(&call.calls) {
                    self.response(&response, None);
                }
                let output = format!(
                    "<task id=\"{child}\" state=\"completed\">\n<task_result>\n{says}\n</task_result>\n</task>"
                );
                self.fire(
                    "PostToolUse",
                    Some(&c.tool),
                    json!({"tool_name": "Task", "tool_input": c.input, "tool_response": {"output": output}}),
                    None,
                );
                self.transcript.tool_result(&ToolResult {
                    id: c.id.clone(),
                    step: c.step.clone(),
                    text: output,
                    is_error: None,
                    ran: None,
                    refused_by: None,
                });
            }
            Dialect::Codex => {}
        }
    }

    // ---- hooks ----------------------------------------------------------------------------

    /// The routes of `event` that hear `tool`.
    fn routes_for(&self, event: &str, tool: Option<&str>) -> Vec<&Route> {
        let of_event = self.routes.iter().filter(|r| r.event == event);
        if self.dialect == Dialect::Opencode && tool.is_some() {
            let tool = tool.unwrap_or_default();
            let own: Vec<&Route> = of_event
                .clone()
                .filter(|r| r.matcher.as_deref() == Some(tool))
                .collect();
            if !own.is_empty() || event != "PreToolUse" {
                return own;
            }
            return of_event.filter(|r| r.matcher.is_none()).collect();
        }
        let hook_tool = tool.map(|t| self.dialect.hook_tool(t, &Value::Null).0);
        of_event
            .filter(|r| match &hook_tool {
                None => true,
                Some(t) => routes::hears(r.matcher.as_deref(), t),
            })
            .collect()
    }

    /// `event`'s hooks for `tool`, each handed `fields` with the dialect's common ones.
    fn fire(
        &self,
        event: &str,
        tool: Option<&str>,
        fields: Value,
        agent: Option<&Agent>,
    ) -> Vec<Fired> {
        let routes = self.routes_for(event, tool);
        if routes.is_empty() {
            return Vec::new();
        }
        let mut payload = self.payload(event, fields, agent);
        if self.dialect == Dialect::Opencode
            && let Some(tool) = tool
            && let Some(name) = routes.iter().find_map(|r| r.tool.clone())
        {
            payload["tool_name"] = name.into();
            let _ = tool;
        }
        let env = self.env(event, agent);
        let spawn = match self.dialect {
            Dialect::Opencode => Spawn::Direct(&self.binary),
            Dialect::Codex => Spawn::Shell(shell_program()),
            Dialect::Claude => Spawn::Shell("/bin/sh"),
        };
        hooks::run_all(&routes, spawn, &payload, &env, &self.cwd)
    }

    fn transcript_path(&self) -> PathBuf {
        match self.dialect {
            Dialect::Claude => config_dir("CLAUDE_CONFIG_DIR", ".claude")
                .join("projects")
                .join(dashed(&self.cwd))
                .join(format!("{}.jsonl", self.session_id)),
            _ => {
                let (date, stamp) = now_stamps();
                config_dir("CODEX_HOME", ".codex")
                    .join("sessions")
                    .join(date)
                    .join(format!("rollout-{stamp}-{}.jsonl", self.session_id))
            }
        }
    }

    /// A hook's payload: the dialect's common fields, then `fields`.
    fn payload(&self, event: &str, fields: Value, agent: Option<&Agent>) -> Value {
        let mut p = Map::new();
        let turn = self.turn_id.lock().ok().and_then(|t| t.clone());
        match self.dialect {
            Dialect::Claude => {
                p.insert("session_id".into(), self.session_id.clone().into());
                p.insert(
                    "transcript_path".into(),
                    self.transcript_path().display().to_string().into(),
                );
                p.insert("cwd".into(), self.cwd.display().to_string().into());
                match event {
                    "SessionStart" => {}
                    "SessionEnd" => {
                        p.insert("prompt_id".into(), super::dialect::uuid().into());
                    }
                    _ => {
                        if let Some(turn) = turn {
                            p.insert("prompt_id".into(), turn.into());
                        }
                        p.insert(
                            "permission_mode".into(),
                            self.launch
                                .permission_mode
                                .clone()
                                .unwrap_or_else(|| CLAUDE_PERMISSION_MODE.to_owned())
                                .into(),
                        );
                    }
                }
                if let Some(agent) = agent {
                    p.insert("agent_id".into(), agent.id.clone().into());
                    p.insert("agent_type".into(), agent.kind.clone().into());
                }
                if claude_effort(event) {
                    p.insert("effort".into(), json!({"level": "medium"}));
                }
                p.insert("hook_event_name".into(), event.into());
                if event == "SessionStart" {
                    p.insert("model".into(), self.model.clone().into());
                }
                if matches!(event, "SessionStart" | "UserPromptSubmit") {
                    p.insert(
                        "session_title".into(),
                        self.launch.name.clone().unwrap_or_default().into(),
                    );
                }
            }
            Dialect::Codex => {
                p.insert("session_id".into(), self.session_id.clone().into());
                if !matches!(event, "SessionStart" | "SessionEnd")
                    && let Some(turn) = turn
                {
                    p.insert("turn_id".into(), turn.into());
                }
                p.insert(
                    "transcript_path".into(),
                    self.transcript_path().display().to_string().into(),
                );
                p.insert("cwd".into(), self.cwd.display().to_string().into());
                p.insert("hook_event_name".into(), event.into());
                if event != "SessionEnd" {
                    p.insert("model".into(), self.model.clone().into());
                    p.insert("permission_mode".into(), "default".into());
                }
            }
            Dialect::Opencode => {
                p.insert("hook_event_name".into(), event.into());
                p.insert("session_id".into(), self.session_id.clone().into());
                p.insert("cwd".into(), self.cwd.display().to_string().into());
                // OpenCode's Stop carries no `stop_hook_active`.
            }
        }
        if let Value::Object(fields) = fields {
            for (k, v) in fields {
                if self.dialect == Dialect::Opencode && k == "stop_hook_active" {
                    continue;
                }
                if self.dialect == Dialect::Codex && event == "SessionEnd" && k != "reason" {
                    continue;
                }
                p.insert(k, v);
            }
        }
        Value::Object(p)
    }

    /// The variables a hook is run with beside the fake's own: what the harness adds.
    fn env(&self, event: &str, _agent: Option<&Agent>) -> Vec<(String, String)> {
        let mut env: Vec<(String, String)> = Vec::new();
        let pid = std::process::id().to_string();
        match self.dialect {
            Dialect::Claude => {
                let config = config_dir("CLAUDE_CONFIG_DIR", ".claude");
                env.extend([
                    ("CLAUDECODE".to_owned(), "1".to_owned()),
                    ("CLAUDE_CODE_ENTRYPOINT".to_owned(), "cli".to_owned()),
                    ("CLAUDE_CODE_SESSION_ID".to_owned(), self.session_id.clone()),
                    ("CLAUDE_CODE_CHILD_SESSION".to_owned(), "1".to_owned()),
                    ("CLAUDE_CODE_SESSION_ATTENDED".to_owned(), "1".to_owned()),
                    ("CLAUDE_PID".to_owned(), pid),
                    (
                        "CLAUDE_PROJECT_DIR".to_owned(),
                        self.cwd.display().to_string(),
                    ),
                    (
                        "CLAUDE_PLUGIN_DATA".to_owned(),
                        config
                            .join("plugins/data")
                            .join(purlis_core::plugin::LOADED_AS.replace('@', "-"))
                            .display()
                            .to_string(),
                    ),
                ]);
                if let Some(plugin) = self.launch.plugin_dirs.first() {
                    env.push(("CLAUDE_PLUGIN_ROOT".to_owned(), plugin.clone()));
                }
                if event == "SessionStart" {
                    let dir = config.join("session-env").join(&self.session_id);
                    let _ = std::fs::create_dir_all(&dir);
                    env.push((
                        "CLAUDE_ENV_FILE".to_owned(),
                        dir.join("sessionstart-hook-0.sh").display().to_string(),
                    ));
                }
                if claude_effort(event) {
                    env.push(("CLAUDE_EFFORT".to_owned(), "medium".to_owned()));
                }
                // What Claude Code sets for every hook it runs, measured on 2.1.325.
                env.extend(
                    [
                        ("AI_AGENT", "claude-code"),
                        ("COREPACK_ENABLE_AUTO_PIN", "0"),
                        ("NoDefaultCurrentDirectoryInExePath", "1"),
                        ("COLUMNS", "160"),
                        ("LINES", "50"),
                    ]
                    .map(|(k, v)| (k.to_owned(), v.to_owned())),
                );
                // And the `env` of the `--settings` it was started with, as it applies it.
                env.extend(self.settings_env());
            }
            Dialect::Codex => {}
            Dialect::Opencode => {
                env.extend([
                    ("AGENT".to_owned(), "1".to_owned()),
                    (
                        "npm_config_user_agent".to_owned(),
                        format!(
                            "bun/1 npm/? node/? {} {}",
                            std::env::consts::OS,
                            std::env::consts::ARCH
                        ),
                    ),
                    ("OPENCODE".to_owned(), "1".to_owned()),
                    ("OPENCODE_PID".to_owned(), pid),
                    ("PURLIS_SESSION_ID".to_owned(), self.session_id.clone()),
                    ("CHARTER_SESSION_ID".to_owned(), self.session_id.clone()),
                ]);
            }
        }
        env
    }

    /// The `env` of Claude Code's `--settings`.
    fn settings_env(&self) -> Vec<(String, String)> {
        let Some(settings) = self.launch.settings.as_deref() else {
            return Vec::new();
        };
        let text = if settings.trim_start().starts_with('{') {
            settings.to_owned()
        } else {
            std::fs::read_to_string(settings).unwrap_or_default()
        };
        serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v.get("env").and_then(Value::as_object).cloned())
            .map(|env| {
                env.into_iter()
                    .filter_map(|(k, v)| Some((k, v.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The context Codex sends as the user's first message.
    fn codex_environment(&self) -> String {
        let cwd = self.cwd.display();
        let (date, _) = now_stamps();
        let tz = std::env::var("TZ").unwrap_or_else(|_| "Etc/UTC".to_owned());
        format!(
            "<environment_context>\n  <cwd>{cwd}</cwd>\n  <shell>bash</shell>\n  <current_date>{}</current_date>\n  <timezone>{tz}</timezone>\n  <filesystem><workspace_roots><root>{cwd}</root></workspace_roots><permission_profile type=\"disabled\"><file_system type=\"unrestricted\" /></permission_profile></filesystem>\n</environment_context>",
            date.replace('/', "-")
        )
    }
}

#[derive(Debug, Clone)]
struct Checked {
    id: String,
    step: Option<String>,
    tool: String,
    input: Value,
    action: Action,
    background: bool,
}

#[derive(Debug)]
enum Outcome {
    Shell(Ran, Duration),
    Background { id: String, file: PathBuf },
    File(Result<String, (std::io::Error, String)>),
    Read(Result<String, (std::io::Error, String)>),
    Web(Result<u16, String>),
    Mcp(Result<mcp_call::Answered, String>),
    Interrupted,
    Unknown,
}

impl PartialEq for Outcome {
    fn eq(&self, other: &Self) -> bool {
        matches!((self, other), (Outcome::Interrupted, Outcome::Interrupted))
    }
}

#[derive(Debug, Default)]
struct Framed {
    text: String,
    is_error: Option<bool>,
    ran: Option<Ran>,
    /// The hook event after the call, the payload field its result goes in, and the result.
    post: Option<(&'static str, &'static str, Value)>,
    post_extra: Option<Vec<(String, Value)>>,
}

impl Framed {
    fn push_duration(&mut self, took: Duration) {
        self.post_extra
            .get_or_insert_with(Vec::new)
            .push(("duration_ms".to_owned(), (took.as_millis() as u64).into()));
    }
}

/// Whether Claude Code hands this event's hooks its effort: every one but the session's start
/// and end and a prompt's submission (2.1.325).
fn claude_effort(event: &str) -> bool {
    !matches!(event, "SessionStart" | "SessionEnd" | "UserPromptSubmit")
}

/// A web fetch, as a fetch in the harness's own process makes it: a plain `http://` URL is a
/// `GET` sent to the proxy the wrap names (`HTTP_PROXY`), an `https://` one a `CONNECT` to it
/// (`HTTPS_PROXY`), or either straight to the host where none is named. The status that came
/// back, or why none did.
fn dial(url: &str) -> Result<u16, String> {
    use std::io::{BufRead, BufReader, Write};
    let plain = url.starts_with("http://");
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split('/').next().unwrap_or_default();
    let target = if host.contains(':') {
        host.to_owned()
    } else if plain {
        format!("{host}:80")
    } else {
        format!("{host}:443")
    };
    let names: &[&str] = if plain {
        &["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"]
    } else {
        &["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"]
    };
    let proxy = names
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .filter(|p| !p.is_empty());
    let connect_to = |addr: &str| {
        use std::net::ToSocketAddrs;
        let addr = addr
            .to_socket_addrs()
            .map_err(|e| format!("cannot resolve {addr}: {e}"))?
            .next()
            .ok_or_else(|| format!("no address for {addr}"))?;
        std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(10))
            .map_err(|e| format!("cannot connect to {addr}: {e}"))
    };
    let (mut stream, request) = match &proxy {
        Some(proxy) => {
            let addr = proxy
                .split_once("://")
                .map_or(proxy.as_str(), |(_, r)| r)
                .trim_end_matches('/')
                .to_owned();
            let request = if plain {
                format!("GET {url} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n")
            } else {
                format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n")
            };
            (connect_to(&addr)?, request)
        }
        None if plain => {
            let path = rest.find('/').map_or("/", |at| &rest[at..]);
            (
                connect_to(&target)?,
                format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"),
            )
        }
        // No TLS is spoken: a dial that connects is all an `https://` fetch is held to here.
        None => return connect_to(&target).map(|_| 200),
    };
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    line.split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("no HTTP answer: {:?}", line.trim()))
}

/// An I/O error as Node's `fs` says it, as Claude Code passes it on.
fn node_error(e: &std::io::Error, path: &str) -> String {
    let (code, what) = match e.raw_os_error() {
        Some(13) => ("EACCES", "permission denied"),
        Some(1) => ("EPERM", "operation not permitted"),
        Some(2) => ("ENOENT", "no such file or directory"),
        Some(30) => ("EROFS", "read-only file system"),
        _ => return e.to_string(),
    };
    format!("{code}: {what}, open '{path}'")
}

/// A path without the temporary suffix a Claude Code write gives it.
fn strip_tmp(path: &str) -> String {
    path.split(".tmp.").next().unwrap_or(path).to_owned()
}

fn shell_program() -> &'static str {
    if Path::new("/bin/bash").exists() {
        "/bin/bash"
    } else {
        "/bin/sh"
    }
}

fn config_dir(var: &str, under_home: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(under_home)
    })
}

/// A folder's path as Claude Code names a folder after it: each `/` and `.` a `-`.
fn dashed(path: &Path) -> String {
    path.display()
        .to_string()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn uid() -> String {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}

fn hex(n: usize) -> String {
    lower_alnum(n)
        .chars()
        .map(|c| {
            let v = c as u32 % 16;
            char::from_digit(v, 16).unwrap_or('0')
        })
        .collect()
}

/// `YYYY/MM/DD`, and `YYYY-MM-DDTHH-MM-SS`, of now in UTC.
fn now_stamps() -> (String, String) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        format!("{y:04}/{m:02}/{d:02}"),
        format!(
            "{y:04}-{m:02}-{d:02}T{:02}-{:02}-{:02}",
            rem / 3600,
            (rem % 3600) / 60,
            rem % 60
        ),
    )
}

/// At most `n` bytes of `text`, cut on a character's boundary.
fn cut(text: &str, n: usize) -> String {
    if text.len() <= n {
        return text.to_owned();
    }
    let mut end = n;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}
