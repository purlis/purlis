// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the real Codex keeps of what it is told, and what it takes in mid-chat, measured**
//! (#5841, #3002; ADR 0102 and ADR 0099, both amended 2026-11-09).
//!
//! These are measurements, not checks: each prints what it saw on lines that start with
//! `measured:`, and none fails on what Codex did. The only thing asserted is that the Codex run
//! is the version `tests/fixtures/harness-versions/codex.txt` names, since a measurement of
//! another version would be recorded under the wrong one.
//!
//! **No model and no credential.** Codex is pointed at a model provider on a loopback port that
//! refuses the connection, so a turn fails at its first request. Everything measured is decided
//! before that request: what a `SessionStart` hook's output becomes in the conversation, which
//! Codex records in the session's rollout (`$CODEX_HOME/sessions/**/rollout-*.jsonl`) before
//! it sends anything, and the skills catalogue `codex debug prompt-input` renders. The hooks are
//! trusted the way purlis trusts the hooks it arms on a sandboxed chat
//! ([`purlis_core::sandbox::codex::hook_trust`]), so `codex exec` runs them.
//!
//! `#[ignore]`d everywhere but CI's `Codex, measured` step, which installs the pinned build from
//! npm and names it in `PURLIS_CODEX`: run with `--run-ignored only` and that variable set.
//! Without it they fail, since they were asked for on purpose.

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use purlis_core::forklock;
use serde_json::{Value, json};

/// The version purlis supports, as `codex --version` answers.
const SUPPORTED: &str = include_str!("../../../../tests/fixtures/harness-versions/codex.txt");

/// The pinned Codex, checked against [`SUPPORTED`].
fn codex() -> PathBuf {
    let path = std::env::var_os("PURLIS_CODEX")
        .map(PathBuf::from)
        .expect("PURLIS_CODEX names the codex program to measure");
    let out = forklock::output(Command::new(&path).arg("--version").stdin(Stdio::null()))
        .expect("codex runs");
    let said = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        said.trim(),
        SUPPORTED.trim(),
        "the version measured is the one purlis supports"
    );
    path
}

/// A loopback port nothing listens on, so Codex's first model request is refused at once.
fn refused_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map_or(9, |address| address.port())
}

/// A Codex home and a folder to work in, under one temporary root.
struct Here {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Here {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        for folder in ["home/skills", "work"] {
            std::fs::create_dir_all(root.join(folder)).unwrap();
        }
        Self { _dir: dir, root }
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    fn work(&self) -> PathBuf {
        self.root.join("work")
    }

    /// A `SessionStart` hook, at `at`, that answers what `context` holds when it runs, and
    /// notes each run in `runs.log`.
    fn hook(&self, at: usize) -> PathBuf {
        let script = self.root.join(format!("hook{at}.sh"));
        let answer = self.root.join(format!("answer{at}.json"));
        let runs = self.root.join("runs.log");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\ncat >/dev/null\necho {at} >> '{}'\ncat '{}'\n",
                runs.display(),
                answer.display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        script
    }

    /// What the `SessionStart` hook at `at` answers from now on, as Codex's JSON.
    fn context(&self, at: usize, text: &str) {
        let answer = json!({"hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": text,
        }});
        self.answer(at, &answer.to_string());
    }

    /// What the `SessionStart` hook at `at` prints from now on, as it is.
    fn answer(&self, at: usize, printed: &str) {
        std::fs::write(self.root.join(format!("answer{at}.json")), printed).unwrap();
    }

    /// How many times the hooks ran.
    fn runs(&self) -> usize {
        std::fs::read_to_string(self.root.join("runs.log"))
            .map(|text| text.lines().count())
            .unwrap_or(0)
    }

    /// The `-c` words that arm `hooks` on `SessionStart`, with the config that trusts exactly
    /// them and points Codex at a provider that refuses, written into its home.
    fn armed(&self, hooks: &[PathBuf]) -> Vec<String> {
        let groups: Vec<String> = hooks
            .iter()
            .map(|hook| {
                format!(
                    "{{hooks=[{{type=\"command\",command={},timeout=30}}]}}",
                    toml::Value::from(hook.display().to_string())
                )
            })
            .collect();
        let armed = vec![
            "-c".to_owned(),
            format!("hooks.SessionStart=[{}]", groups.join(",")),
        ];
        let mut config: toml::Table = format!(
            "model_provider = \"standin\"\nmodel = \"gpt-5.5\"\n\
             [model_providers.standin]\nname = \"standin\"\n\
             base_url = \"http://127.0.0.1:{}/v1\"\nwire_api = \"responses\"\n\
             request_max_retries = 0\nstream_max_retries = 0\n",
            refused_port()
        )
        .parse()
        .unwrap();
        let mut hooks = toml::Table::new();
        hooks.insert(
            "state".to_owned(),
            toml::Value::Table(purlis_core::sandbox::codex::hook_trust(&armed)),
        );
        config.insert("hooks".to_owned(), toml::Value::Table(hooks));
        std::fs::write(self.home().join("config.toml"), config.to_string()).unwrap();
        armed
    }

    /// `codex` run here with `args`, its home this one's and nothing on stdin.
    fn command(&self, codex: &Path, args: &[String]) -> Command {
        let mut command = Command::new(codex);
        command
            .args(args)
            .current_dir(self.work())
            .env("CODEX_HOME", self.home())
            .env("HOME", &self.root)
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("no_proxy", "127.0.0.1,localhost")
            .stdin(Stdio::null());
        command
    }

    /// Every developer message the session's rollout holds, in order, with the user's prompts
    /// as `user: …`.
    fn conversation(&self) -> Vec<String> {
        let mut said = Vec::new();
        for file in rollouts(&self.home().join("sessions")) {
            for line in std::fs::read_to_string(file).unwrap().lines() {
                let Ok(record) = serde_json::from_str::<Value>(line) else {
                    continue;
                };
                if record["type"] != "response_item" {
                    continue;
                }
                let role = record["payload"]["role"].as_str().unwrap_or_default();
                for part in record["payload"]["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    let text = part["text"].as_str().unwrap_or_default();
                    match role {
                        "developer" => said.push(text.to_owned()),
                        "user" if text.starts_with("turn ") => said.push(format!("user: {text}")),
                        _ => {}
                    }
                }
            }
        }
        said
    }
}

/// The rollout files under `dir`.
fn rollouts(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(rollouts(&path));
        } else if path.extension().is_some_and(|ext| ext == "jsonl") {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// `lines` numbered lines tagged `tag`, 44 to 46 characters each below 10,000 lines.
fn numbered(tag: char, lines: usize) -> String {
    numbered_saying(tag, lines, "line")
}

/// [`numbered`], each line saying `word` before its number.
fn numbered_saying(tag: char, lines: usize, word: &str) -> String {
    (0..lines)
        .map(|n| format!("- {tag}{n:05}: {word} {n} (file: /x/{tag}{n:05}/SKILL.md)"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The numbers of `tag`'s lines in `text`, in order.
fn numbers_in(text: &str, tag: char) -> Vec<usize> {
    text.lines()
        .filter_map(|line| line.strip_prefix(&format!("- {tag}")))
        .filter_map(|rest| rest.get(..5)?.parse().ok())
        .collect()
}

/// One line about what became of `text`, `tag`'s lines, in the developer message that holds
/// them: its size in characters and bytes, the lines kept and, where Codex cut, how many bytes
/// of what was sent its start and its end kept.
fn kept(conversation: &[String], tag: char, text: &str) -> String {
    let sent = text.lines().count();
    let length = format!("{} chars, {} bytes", text.chars().count(), text.len());
    let Some(message) = conversation
        .iter()
        .find(|said| !numbers_in(said, tag).is_empty())
    else {
        return format!("{sent} lines ({length}) sent, none reached the conversation");
    };
    // The longest start of what was sent that the message holds, and the longest end.
    let start = text
        .char_indices()
        .map(|(at, _)| at)
        .chain([text.len()])
        .take_while(|at| message.contains(&text[..*at]))
        .last()
        .unwrap_or(0);
    let end = text
        .char_indices()
        .map(|(at, _)| at)
        .rev()
        .take_while(|at| message.contains(&text[*at..]))
        .last()
        .map_or(0, |at| text.len() - at);
    let numbers = numbers_in(message, tag);
    let gap = numbers.windows(2).find(|pair| pair[1] != pair[0] + 1);
    let marker = message
        .find("tokens truncated")
        .map(|at| &message[message[..at].rfind('…').unwrap_or(at)..at + "tokens truncated…".len()]);
    format!(
        "{sent} lines ({length}) sent; message is {} chars, {} bytes, with {} lines, first {:?}, \
         last {:?}, gap {gap:?}, sent bytes kept at the start {start} and the end {end}, marker \
         {marker:?}, warning line {}, full output saved {}",
        message.chars().count(),
        message.len(),
        numbers.len(),
        numbers.first(),
        numbers.last(),
        message.starts_with("Warning: truncated output"),
        message.contains("Full hook output saved to:"),
    )
}

/// One `codex exec` turn here, armed with `armed`, with `more` of Codex's own flags.
fn one_turn(here: &Here, codex: &Path, armed: &[String], more: &[&str]) {
    let mut args = vec!["exec".to_owned(), "--skip-git-repo-check".to_owned()];
    args.extend(armed.iter().cloned());
    args.extend(more.iter().map(|flag| (*flag).to_owned()));
    args.push("turn one".to_owned());
    let out = forklock::output(&mut here.command(codex, &args)).expect("codex exec runs");
    let printed =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    let hooks: Vec<&str> = printed.lines().filter(|l| l.starts_with("hook:")).collect();
    println!(
        "measured: codex exec ended {:?}, printed {hooks:?}",
        out.status.code()
    );
}

#[test]
#[ignore = "run by CI's `Codex, measured` step, with PURLIS_CODEX"]
fn measures_what_codex_keeps_of_a_long_session_start_hook_output() {
    let codex = codex();
    // Lines of 44 to 46 characters: below, at and past the 10,000 seen on the first run of this
    // measurement (9,995 whole, 10,181 cut), then far past it.
    for lines in [200, 219, 223, 600, 3_000] {
        let text = numbered('b', lines);
        let said = once(&codex, &text, true, &[]);
        println!("measured: one SessionStart hook: {said}");
    }
    // Whether it is characters or bytes: lines of three-byte dashes, under 10,000 characters and
    // over 10,000 bytes.
    let dashes = numbered_saying('b', 150, "———————————");
    let said = once(&codex, &dashes, true, &[]);
    println!("measured: one SessionStart hook of multi-byte lines: {said}");
    // Whether anything moves it: other models, Codex's own output limit, and a plain answer.
    let text = numbered('b', 600);
    for (what, json, more) in [
        ("model gpt-5.2", true, ["-m", "gpt-5.2"].as_slice()),
        (
            "a model Codex does not know",
            true,
            &["-m", "nonesuch-model"],
        ),
        (
            "tool_output_token_limit = 50000",
            true,
            &["-c", "tool_output_token_limit=50000"],
        ),
        ("a plain-text answer", false, &[]),
    ] {
        let said = once(&codex, &text, json, more);
        println!("measured: one SessionStart hook, {what}: {said}");
    }
}

/// What one turn keeps of `text` answered by one `SessionStart` hook, as Codex's JSON or plain,
/// with `more` of Codex's own flags.
fn once(codex: &Path, text: &str, json: bool, more: &[&str]) -> String {
    let here = Here::new();
    let hook = here.hook(0);
    if json {
        here.context(0, text);
    } else {
        here.answer(0, text);
    }
    let armed = here.armed(&[hook]);
    one_turn(&here, codex, &armed, more);
    kept(&here.conversation(), 'b', text)
}

#[test]
#[ignore = "run by CI's `Codex, measured` step, with PURLIS_CODEX"]
fn measures_whether_codex_keeps_each_session_start_hook_s_output_apart() {
    let codex = codex();
    for sizes in [[160, 160, 160, 160].as_slice(), &[600, 100]] {
        let here = Here::new();
        let tags = ['a', 'b', 'c', 'd'];
        let hooks: Vec<PathBuf> = (0..sizes.len()).map(|at| here.hook(at)).collect();
        let texts: Vec<String> = sizes
            .iter()
            .zip(tags)
            .map(|(lines, tag)| numbered(tag, *lines))
            .collect();
        for (at, text) in texts.iter().enumerate() {
            here.context(at, text);
        }
        let armed = here.armed(&hooks);
        one_turn(&here, &codex, &armed, &[]);
        let conversation = here.conversation();
        for (tag, text) in tags.into_iter().zip(&texts) {
            println!(
                "measured: {} SessionStart hooks, hook {tag}: {}",
                sizes.len(),
                kept(&conversation, tag, text)
            );
        }
    }
}

#[test]
#[ignore = "run by CI's `Codex, measured` step, with PURLIS_CODEX"]
fn measures_how_codex_bounds_its_own_skills_catalogue() {
    let codex = codex();
    for (skills, description) in [(100, 100), (300, 100), (600, 100), (2_000, 20)] {
        let here = Here::new();
        let filler = "x".repeat(description);
        for n in 0..skills {
            let name = format!("s{n:04}");
            let folder = here.home().join("skills").join(&name);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(
                folder.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: Skill {name} {filler}\n---\nbody\n"),
            )
            .unwrap();
        }
        let out = forklock::output(
            &mut here.command(&codex, &["debug".to_owned(), "prompt-input".to_owned()]),
        )
        .expect("codex debug prompt-input runs");
        let items: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
        let catalogue = items
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|item| item["content"].as_array().cloned().unwrap_or_default())
            .filter_map(|part| part["text"].as_str().map(str::to_owned))
            .find(|text| text.contains("<skills_instructions>"))
            .unwrap_or_default();
        let ours: Vec<&str> = catalogue
            .lines()
            .filter_map(|line| line.strip_prefix("- s"))
            .collect();
        let described = ours.iter().filter(|line| line.contains("Skill s")).count();
        println!(
            "measured: {skills} skills of {description}-char descriptions: catalogue {} chars, \
             {} of ours listed (last {:?}), {described} still described, omission said {}",
            catalogue.chars().count(),
            ours.len(),
            ours.last()
                .map(|line| line.split(':').next().unwrap_or_default()),
            catalogue.contains("omitted"),
        );
    }
}

/// A Codex app server on stdio, read on a thread of its own.
struct AppServer {
    child: std::process::Child,
    said: mpsc::Receiver<Value>,
    asked: u64,
    /// Every notification's method, in order.
    heard: Vec<String>,
}

impl AppServer {
    fn start(here: &Here, codex: &Path, armed: &[String]) -> Self {
        let mut args = vec!["app-server".to_owned()];
        args.extend(armed.iter().cloned());
        let mut child = forklock::spawn(
            here.command(codex, &args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null()),
        )
        .expect("codex app-server runs");
        let out = child.stdout.take().unwrap();
        let (tx, said) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if let Ok(message) = serde_json::from_str(&line)
                    && tx.send(message).is_err()
                {
                    break;
                }
            }
        });
        Self {
            child,
            said,
            asked: 0,
            heard: Vec::new(),
        }
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    /// The answer to `method` with `params`.
    fn ask(&mut self, method: &str, params: Value) -> Value {
        self.asked += 1;
        let id = self.asked;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        self.until(|message| message["id"] == id, Duration::from_secs(60))
    }

    /// The first message `wanted` takes, noting every notification on the way.
    fn until(&mut self, wanted: impl Fn(&Value) -> bool, within: Duration) -> Value {
        let end = Instant::now() + within;
        loop {
            let left = end.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "codex app-server went quiet: {:?}",
                self.heard
            );
            let Ok(message) = self.said.recv_timeout(left.min(Duration::from_secs(1))) else {
                continue;
            };
            if let Some(method) = message["method"].as_str() {
                self.heard.push(method.to_owned());
            }
            if wanted(&message) {
                return message;
            }
        }
    }

    /// The notifications heard over `wait`.
    fn heard_over(&mut self, wait: Duration) -> BTreeSet<String> {
        let from = self.heard.len();
        let end = Instant::now() + wait;
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            if let Ok(message) = self.said.recv_timeout(left)
                && let Some(method) = message["method"].as_str()
            {
                self.heard.push(method.to_owned());
            }
        }
        self.heard[from..].iter().cloned().collect()
    }

    fn turn(&mut self, thread: &str, text: &str) {
        self.ask(
            "turn/start",
            json!({"threadId": thread, "input": [{"type": "text", "text": text}]}),
        );
        self.until(
            |message| message["method"] == "turn/completed",
            Duration::from_secs(90),
        );
    }
}

impl Drop for AppServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A skill named `name` in `folder`, described as `description`.
fn skill(folder: &Path, name: &str, description: &str) {
    let folder = folder.join(name);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\nBody of {name}.\n"),
    )
    .unwrap();
}

/// The skills `skills/list` names, without the ones Codex ships, as `name: description`.
fn listed(answer: &Value) -> Vec<String> {
    let mut names: Vec<String> = answer["result"]["data"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|entry| entry["skills"].as_array().cloned().unwrap_or_default())
        .filter(|skill| {
            !skill["path"]
                .as_str()
                .unwrap_or_default()
                .contains("/.system/")
        })
        .map(|skill| {
            format!(
                "{}: {}",
                skill["name"].as_str().unwrap_or_default(),
                skill["description"].as_str().unwrap_or_default()
            )
        })
        .collect();
    names.sort();
    names
}

#[test]
#[ignore = "run by CI's `Codex, measured` step, with PURLIS_CODEX"]
fn measures_what_codex_takes_in_of_a_skill_changed_mid_chat() {
    // One Codex process and one conversation, two turns. Between them: a skill of Codex's own
    // is edited and another added in `$CODEX_HOME/skills`, and what the `SessionStart` hook
    // would answer changes as purlis's briefing would if the plane's skills changed.
    let codex = codex();
    let here = Here::new();
    let skills = here.home().join("skills");
    skill(&skills, "alpha", "Alpha v1.");
    let hook = here.hook(0);
    here.context(0, "BRIEFING gamma v1");
    let armed = here.armed(&[hook]);
    let mut server = AppServer::start(&here, &codex, &armed);
    server.ask(
        "initialize",
        json!({"clientInfo": {"name": "purlis-measure", "version": "0"}}),
    );
    server.send(&json!({"jsonrpc": "2.0", "method": "initialized"}));
    let work = here.work().display().to_string();
    let before = server.ask("skills/list", json!({"cwds": [work]}));
    let started = server.ask("thread/start", json!({"cwd": work}));
    let thread = started["result"]["thread"]["id"]
        .as_str()
        .expect("a thread")
        .to_owned();
    server.turn(&thread, "turn one");
    let runs_after_one = here.runs();

    skill(&skills, "alpha", "Alpha v2, edited mid-chat.");
    skill(&skills, "beta", "Beta, added mid-chat.");
    here.context(0, "BRIEFING gamma v2");
    let heard = server.heard_over(Duration::from_secs(15));
    let after = server.ask("skills/list", json!({"cwds": [work]}));
    server.turn(&thread, "turn two");
    // Then the client asks Codex to read its skills again, and a third turn follows.
    let forced = server.ask("skills/list", json!({"cwds": [work], "forceReload": true}));
    server.turn(&thread, "turn three");
    drop(server);

    println!(
        "measured: skills/list before the change: {:?}",
        listed(&before)
    );
    println!("measured: notifications in the 15 s after the change: {heard:?}");
    println!("measured: skills/list after it: {:?}", listed(&after));
    println!(
        "measured: skills/list with forceReload, after turn two: {:?}",
        listed(&forced)
    );
    println!(
        "measured: SessionStart hook runs: {runs_after_one} after turn one, {} after turn three",
        here.runs()
    );
    for text in here.conversation() {
        let shown = if text.starts_with("user: ") {
            text
        } else if text.contains("<skills_instructions>") {
            let ours: Vec<&str> = text
                .lines()
                .filter(|line| ["- alpha", "- beta"].iter().any(|s| line.starts_with(s)))
                .map(|line| line.split(" (file:").next().unwrap_or(line))
                .collect();
            format!("skills catalogue {ours:?}")
        } else if text.contains("BRIEFING") {
            format!("hook output {text:?}")
        } else {
            continue;
        };
        println!("measured: conversation: {shown}");
    }
}
