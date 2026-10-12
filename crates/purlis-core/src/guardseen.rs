// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! When the Bash guard was last seen running in a project, and under which harness —
//! `charter/guardseen.py`, ported for doctor's `guard seen` row (#994).
//!
//! `plane-root guard` answers whether the guard is **declared**: a hook line in a settings file,
//! or an enabled plugin that dispatches it. That is still not the fact the operator needs. A
//! declaration can be real while nothing dispatches it, so this records the other half:
//! reaching `purlis hook pretooluse` at all is proof the guard ran, under that harness.
//!
//! **An observation with an age, never a boolean.** A project worked in from a plain terminal
//! has no dispatch and is fine; one whose guard last ran weeks ago under a harness nobody uses
//! now is the incident. Doctor gives the date and the name, and the reader draws the line.
//!
//! **Overwritten, not appended**: this answers "when last", and a history of every tool call
//! would grow without bound for a question nobody asks. The file is `guard-seen.json` in the
//! state directory (`docs/plane-format.md`), machine-local and gitignored, written whole and at
//! 0600 through [`State`], as every other hook record is. Best-effort: a turn never fails over
//! it.

use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{Map, Value};

use crate::hookstate::State;

/// The record's file name, under the state directory.
pub const FILE_NAME: &str = "guard-seen.json";

/// What dispatched the guard: a plugin, which Claude Code says by `$CLAUDE_PLUGIN_ROOT` in the
/// hook's environment, or otherwise a `purlis hook pretooluse` line in a settings file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Plugin,
    Settings,
}

impl Source {
    /// The word the record carries.
    pub fn word(self) -> &'static str {
        match self {
            Self::Plugin => "plugin",
            Self::Settings => "settings",
        }
    }

    /// The source a hook with environment `env` was dispatched by.
    pub fn of(env: &dyn Fn(&str) -> Option<String>) -> Self {
        if env("CLAUDE_PLUGIN_ROOT").is_some_and(|v| !v.is_empty()) {
            Self::Plugin
        } else {
            Self::Settings
        }
    }
}

/// The harness registry's name for Claude Code, as a sighting records it.
pub const CLAUDE_CODE: &str = "claude-code";

/// The latest sighting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sighting {
    pub at: DateTime<Utc>,
    /// The harness the chat ran, when its environment named one.
    pub harness: Option<String>,
    /// `plugin` or `settings`, or `None` for a record that predates the field.
    pub source: Option<String>,
    /// For a Claude Code sighting, the config folder it ran under (`$CLAUDE_CONFIG_DIR`, or
    /// `~/.claude`); `None` where the record has none.
    pub claude_config_dir: Option<String>,
}

impl Sighting {
    /// Whether this sighting is Claude Code's: named so, or dispatched by a plugin, which only
    /// Claude Code has.
    pub fn is_claude_code(&self) -> bool {
        self.harness.as_deref() == Some(CLAUDE_CODE) || self.source.as_deref() == Some("plugin")
    }
}

/// Where a project's record is.
pub fn path(state: &State) -> PathBuf {
    state.dir().join(FILE_NAME)
}

/// Record that the guard just ran, `now`, under `harness`, dispatched by `source`. A Claude Code
/// sighting also records the config folder in use, `folder`, so a sighting under one folder is
/// never read as proof for a session under another. Best-effort: the error is the caller's to
/// drop.
pub fn mark(
    state: &State,
    harness: Option<&str>,
    source: Source,
    folder: Option<&Path>,
    now: DateTime<Utc>,
) -> std::io::Result<()> {
    let mut rec = Map::new();
    rec.insert(
        "harness".to_owned(),
        harness.map_or(Value::Null, |h| Value::String(h.to_owned())),
    );
    rec.insert("source".to_owned(), Value::String(source.word().to_owned()));
    rec.insert(
        "ts".to_owned(),
        Value::String(now.to_rfc3339_opts(SecondsFormat::Secs, false)),
    );
    if harness == Some(CLAUDE_CODE) || source == Source::Plugin {
        rec.insert(
            "claude_config_dir".to_owned(),
            folder
                .filter(|f| f.is_absolute())
                .map_or(Value::Null, |f| Value::String(f.display().to_string())),
        );
    }
    // `json.dumps(rec, sort_keys=True)`, the shape the plane format names.
    let mut text = crate::pyjson::dumps_sorted(&Value::Object(rec));
    text.push('\n');
    state.replace(&path(state), text.as_bytes())
}

/// The latest sighting, or `None`: none recorded, or one that is unreadable, not an object, or
/// carries no time it can be dated by. A preflight row renders whatever it finds.
pub fn last(state: &State) -> Option<Sighting> {
    let text = state.read_text(&path(state))?;
    let Ok(Value::Object(rec)) = serde_json::from_str::<Value>(&text) else {
        return None;
    };
    let at = rec
        .get("ts")
        .and_then(Value::as_str)
        .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())?
        .with_timezone(&Utc);
    let text_of = |key: &str| {
        rec.get(key)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };
    Some(Sighting {
        at,
        harness: text_of("harness"),
        source: text_of("source"),
        claude_config_dir: text_of("claude_config_dir"),
    })
}

/// Whether anyone has worked in the project at `root`: a clone in one of its workspaces.
///
/// The gate on `guard seen` warning at all. A project made five minutes ago has had no chat to
/// dispatch a guard, and a warning on day one is how a row stops being read. **Something a
/// person made**, never session state: that is written by the very hooks whose absence is being
/// looked for. Python also counted a persona, but `purlis init` makes one, so here a persona is
/// no evidence anybody worked anywhere.
pub fn worked_in(root: &Path) -> bool {
    let Ok(workspaces) = std::fs::read_dir(root.join("workspaces")) else {
        return false;
    };
    workspaces.flatten().any(|ws| {
        let name = ws.file_name();
        !name.to_string_lossy().starts_with('.')
            && ws.path().is_dir()
            && std::fs::read_dir(ws.path()).is_ok_and(|children| {
                children
                    .flatten()
                    .any(|c| c.path().is_dir() && c.path().join(".git").exists())
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> (tempfile::TempDir, State) {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        (dir, State::of(&root))
    }

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn nothing_recorded_is_no_sighting() {
        let (_d, state) = state();
        assert_eq!(last(&state), None);
    }

    #[test]
    fn a_sighting_is_read_back_with_its_harness_and_source() {
        let (_d, state) = state();
        let now = at("2026-10-13T01:02:03Z");
        mark(&state, Some("codex"), Source::Settings, None, now).unwrap();
        assert_eq!(
            last(&state),
            Some(Sighting {
                at: now,
                harness: Some("codex".to_owned()),
                source: Some("settings".to_owned()),
                claude_config_dir: None,
            })
        );
        // The format the plane format names: sorted keys, seconds, a trailing newline.
        assert_eq!(
            std::fs::read_to_string(path(&state)).unwrap(),
            "{\"harness\": \"codex\", \"source\": \"settings\", \"ts\": \"2026-10-13T01:02:03+00:00\"}\n"
        );
    }

    #[test]
    fn a_claude_code_sighting_carries_the_folder_it_ran_under() {
        let (_d, state) = state();
        let now = at("2026-10-13T01:02:03Z");
        mark(
            &state,
            Some(CLAUDE_CODE),
            Source::Plugin,
            Some(Path::new("/h/.claude")),
            now,
        )
        .unwrap();
        let seen = last(&state).unwrap();
        assert!(seen.is_claude_code());
        assert_eq!(seen.claude_config_dir.as_deref(), Some("/h/.claude"));
        // A relative folder names nothing a reader can compare, so it is recorded as none.
        mark(
            &state,
            Some(CLAUDE_CODE),
            Source::Settings,
            Some(Path::new(".")),
            now,
        )
        .unwrap();
        assert_eq!(last(&state).unwrap().claude_config_dir, None);
    }

    #[test]
    fn the_latest_sighting_replaces_the_one_before() {
        let (_d, state) = state();
        mark(
            &state,
            Some("a"),
            Source::Settings,
            None,
            at("2026-10-01T00:00:00Z"),
        )
        .unwrap();
        mark(
            &state,
            Some("b"),
            Source::Settings,
            None,
            at("2026-10-02T00:00:00Z"),
        )
        .unwrap();
        assert_eq!(last(&state).unwrap().harness.as_deref(), Some("b"));
    }

    #[test]
    fn a_record_with_no_time_reads_as_none() {
        let (_d, state) = state();
        state
            .write(&path(&state), b"{\"harness\": \"x\"}\n")
            .unwrap();
        assert_eq!(last(&state), None);
        state.write(&path(&state), b"not json").unwrap();
        assert_eq!(last(&state), None);
    }

    #[test]
    fn the_source_is_the_plugin_only_when_claude_code_says_so() {
        assert_eq!(
            Source::of(&|k| (k == "CLAUDE_PLUGIN_ROOT").then(|| "/p".to_owned())),
            Source::Plugin
        );
        assert_eq!(Source::of(&|_| None), Source::Settings);
        assert_eq!(
            Source::of(&|k| (k == "CLAUDE_PLUGIN_ROOT").then(String::new)),
            Source::Settings
        );
    }

    #[test]
    fn a_project_is_worked_in_once_a_workspace_holds_a_clone() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(!worked_in(root));
        std::fs::create_dir_all(root.join("workspaces/alpha/notes")).unwrap();
        assert!(!worked_in(root), "a folder is not a clone");
        std::fs::create_dir_all(root.join("workspaces/alpha/svc/.git")).unwrap();
        assert!(worked_in(root));
    }
}
