// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The command line**: the fake's own words (`--script`, `--dialect`, `--transcript`,
//! `--place`, `--routes`, `--version`), taken out wherever they stand, and the rest read as the
//! dialect's own program reads it, so purlis starts the fake with the words it starts the real
//! harness with.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::dialect::Dialect;

#[derive(Debug, Clone, Default)]
pub struct Own {
    pub script: Option<PathBuf>,
    pub dialect: Option<String>,
    pub transcript: Option<PathBuf>,
    pub places: BTreeMap<String, String>,
    pub routes: bool,
    pub version: bool,
}

/// The fake's own words, and the rest in order.
pub fn split(argv: &[String]) -> Result<(Own, Vec<String>), String> {
    let mut own = Own::default();
    let mut rest = Vec::new();
    let mut words = argv.iter();
    while let Some(word) = words.next() {
        let (flag, inline) = match word.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_owned())),
            _ => (word.as_str(), None),
        };
        let mut value = |name: &str| {
            inline
                .clone()
                .or_else(|| words.next().cloned())
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match flag {
            "--script" => own.script = Some(PathBuf::from(value("--script")?)),
            "--dialect" => own.dialect = Some(value("--dialect")?),
            "--transcript" => own.transcript = Some(PathBuf::from(value("--transcript")?)),
            "--place" => {
                let place = value("--place")?;
                let (name, path) = place
                    .split_once('=')
                    .ok_or_else(|| format!("--place {place:?} is not name=value"))?;
                own.places.insert(name.to_owned(), path.to_owned());
            }
            "--routes" => own.routes = true,
            "--version" | "-v" | "-V" => own.version = true,
            _ => rest.push(word.clone()),
        }
    }
    Ok((own, rest))
}

/// What a dialect's own words say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Launch {
    /// Claude Code's `--session-id`: the id purlis chose.
    pub session_id: Option<String>,
    /// The id the session resumes: Claude Code's `--resume`, Codex's `resume <id>`, OpenCode's
    /// `-s`.
    pub resume: Option<String>,
    /// Claude Code's `--name`.
    pub name: Option<String>,
    /// Claude Code's `--settings`.
    pub settings: Option<String>,
    /// Claude Code's `--plugin-dir`s.
    pub plugin_dirs: Vec<String>,
    pub permission_mode: Option<String>,
    pub model: Option<String>,
    /// The first message: the positional prompt (Claude Code, Codex) or `--prompt` (OpenCode).
    pub prompt: Option<String>,
}

/// The flags that take a value, per dialect; any other flag is read as taking none.
fn takes_a_value(dialect: Dialect, flag: &str) -> bool {
    match dialect {
        Dialect::Claude => matches!(
            flag,
            "--plugin-dir"
                | "--setting-sources"
                | "--tools"
                | "--settings"
                | "--session-id"
                | "--name"
                | "--resume"
                | "-r"
                | "--permission-mode"
                | "--model"
                | "--append-system-prompt"
                | "--add-dir"
                | "--allowedTools"
                | "--allowed-tools"
                | "--disallowedTools"
                | "--disallowed-tools"
                | "--fallback-model"
                | "--output-format"
                | "--input-format"
                | "--agents"
        ),
        Dialect::Codex => matches!(
            flag,
            "-c" | "--config"
                | "-m"
                | "--model"
                | "-C"
                | "--cd"
                | "-s"
                | "--sandbox"
                | "-a"
                | "--ask-for-approval"
                | "-p"
                | "--profile"
                | "-i"
                | "--image"
                | "--add-dir"
        ),
        Dialect::Opencode => matches!(
            flag,
            "-s" | "--session"
                | "--prompt"
                | "-m"
                | "--model"
                | "--agent"
                | "--port"
                | "--hostname"
                | "--log-level"
        ),
    }
}

/// `argv`, the rest of the fake's words, read as `dialect`'s program reads its own.
pub fn read(dialect: Dialect, argv: &[String]) -> Launch {
    let mut launch = Launch::default();
    let mut positional: Vec<String> = Vec::new();
    let mut at = 0;
    while at < argv.len() {
        let word = &argv[at];
        let (flag, inline) = match word.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
            _ => (word.clone(), None),
        };
        if flag == "--mcp-config" && dialect == Dialect::Claude {
            // Every word up to the next flag.
            at += 1;
            while at < argv.len() && !argv[at].starts_with("--") {
                at += 1;
            }
            continue;
        }
        if flag.starts_with('-') && flag.len() > 1 {
            let value = if takes_a_value(dialect, &flag) {
                match inline {
                    Some(v) => Some(v),
                    None => {
                        at += 1;
                        argv.get(at).cloned()
                    }
                }
            } else {
                None
            };
            match (dialect, flag.as_str()) {
                (Dialect::Claude, "--session-id") => launch.session_id = value,
                (Dialect::Claude, "--resume" | "-r") | (Dialect::Opencode, "-s" | "--session") => {
                    launch.resume = value;
                }
                (Dialect::Claude, "--name") => launch.name = value,
                (Dialect::Claude, "--settings") => launch.settings = value,
                (Dialect::Claude, "--plugin-dir") => launch.plugin_dirs.extend(value),
                (Dialect::Claude, "--permission-mode") => launch.permission_mode = value,
                (_, "--model" | "-m") => launch.model = value,
                (Dialect::Opencode, "--prompt") => launch.prompt = value,
                _ => {}
            }
        } else {
            positional.push(word.clone());
        }
        at += 1;
    }
    match dialect {
        Dialect::Codex if positional.first().map(String::as_str) == Some("resume") => {
            launch.resume = positional.get(1).cloned();
            launch.prompt = positional.get(2).cloned();
        }
        Dialect::Claude | Dialect::Codex => launch.prompt = positional.last().cloned(),
        // OpenCode's positional is the project, never a prompt.
        Dialect::Opencode => {}
    }
    launch
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(w: &[&str]) -> Vec<String> {
        w.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn each_dialect_reads_its_own_resume_argument() {
        assert_eq!(
            read(Dialect::Claude, &words(&["--resume", "abc", "--name", "chat 1"])).resume,
            Some("abc".to_owned())
        );
        assert_eq!(
            read(Dialect::Codex, &words(&["-c", "a=1", "resume", "abc"])).resume,
            Some("abc".to_owned())
        );
        assert_eq!(
            read(Dialect::Opencode, &words(&["-s", "ses_abc"])).resume,
            Some("ses_abc".to_owned())
        );
    }

    #[test]
    fn the_first_message_is_the_dialects_own() {
        let claude = read(
            Dialect::Claude,
            &words(&["--mcp-config", "{}", "--settings", "{}", "--session-id", "s", "hello"]),
        );
        assert_eq!(claude.prompt.as_deref(), Some("hello"));
        assert_eq!(claude.session_id.as_deref(), Some("s"));
        let codex = read(Dialect::Codex, &words(&["-c", "hooks.Stop=[]", "hello"]));
        assert_eq!(codex.prompt.as_deref(), Some("hello"));
        let opencode = read(Dialect::Opencode, &words(&["--prompt", "hello", "."]));
        assert_eq!(opencode.prompt.as_deref(), Some("hello"));
    }

    #[test]
    fn the_fakes_own_words_are_taken_out_wherever_they_stand() {
        let (own, rest) = split(&words(&[
            "--plugin-dir",
            "p",
            "--script",
            "s.json",
            "--dialect=codex",
            "--place",
            "work=/w",
            "x",
        ]))
        .unwrap();
        assert_eq!(own.script, Some(PathBuf::from("s.json")));
        assert_eq!(own.dialect.as_deref(), Some("codex"));
        assert_eq!(own.places.get("work").map(String::as_str), Some("/w"));
        assert_eq!(rest, words(&["--plugin-dir", "p", "x"]));
    }
}
