// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A built-in skill with two trigger tests each way, its plugin, the crowd, and a stand-in
//! harness that replays the scoring module's recorded output for that harness.
//!
//! Each stand-in answers only when it is started as a chat in the window would start it, and
//! otherwise exits without a result, which scores as a run cut short:
//!
//! - `claude`: the provider's key in `ANTHROPIC_API_KEY`; among the crowd when the crowd's
//!   plugin folder is among its words.
//! - `codex`: the key in `CODEX_API_KEY`, the built-ins' folder in `PURLIS_SKILLS_DIR` for its
//!   briefing, and `CODEX_HOME` of its own; among the crowd when that home's `skills` holds
//!   any. Its reads name the fixture's own plugin, as a real run's would.
//! - `opencode`: the key in `ANTHROPIC_API_KEY` and purlis's config in
//!   `OPENCODE_CONFIG_CONTENT`; among the crowd when that config names the crowd's skills.

use std::path::{Path, PathBuf};

use purlis_skills::grade::ci::Harness;

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub harness: Harness,
    pub plugin: PathBuf,
    pub crowd: PathBuf,
    pub cwd: PathBuf,
    pub program: PathBuf,
    /// Every word each start was given, one per line.
    pub argv: PathBuf,
}

const TRIGGERS: &str = r#"[
  {"query": "run the migration with the database password from the vault", "should_trigger": true},
  {"query": "store this API token so the deploy can use it", "should_trigger": true},
  {"query": "rename the email column in the users table", "should_trigger": false},
  {"query": "log in to the staging dashboard in a browser", "should_trigger": false}
]"#;

const SOURCES: &str = "# purlis-secrets: where each trigger test came from

Trigger bar: strict
Neighbours: purlis-browser

1. A mined transcript, paraphrased.
2. The skill's own description.
3. A plain coding request.
4. purlis-browser's own request: a near miss.
";

/// Where the recordings' runner checkout had purlis's plugin.
pub const RECORDED_PLUGIN: &str = "/home/runner/work/purlis/purlis/app/src-tauri/plugin";

pub fn recorded(harness: Harness, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../skills/src/grade/ci/recorded")
        .join(harness.word())
        .join(name)
}

/// The stand-in's answer on Claude Code: `among_crowd` and `bare` say whether a query about a
/// credential starts purlis-secrets in that listing; every other query starts nothing.
pub fn fixture(among_crowd: bool, bare: bool) -> Fixture {
    fixture_on(Harness::ClaudeCode, among_crowd, bare)
}

/// [`fixture`], on `harness`.
pub fn fixture_on(harness: Harness, among_crowd: bool, bare: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let plugin = dir.path().join("plugin");
    let skill = plugin.join("skills/purlis-secrets");
    std::fs::create_dir_all(skill.join("evals")).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: purlis-secrets\ndescription: Use a credential held in a purlis vault.\n---\n",
    )
    .unwrap();
    std::fs::write(skill.join("evals/triggers.json"), TRIGGERS).unwrap();
    std::fs::write(skill.join("evals/triggers.sources.md"), SOURCES).unwrap();
    let browser = plugin.join("skills/purlis-browser");
    std::fs::create_dir_all(&browser).unwrap();
    std::fs::write(
        browser.join("SKILL.md"),
        "---\nname: purlis-browser\ndescription: Drive a browser for purlis.\n---\n",
    )
    .unwrap();
    // The opencode shim the bundle carries, which an opencode chat is armed with.
    let shim = purlis_core::opencode::shim_in(&plugin);
    std::fs::create_dir_all(shim.parent().unwrap()).unwrap();
    std::fs::write(&shim, "export default {}\n").unwrap();
    let crowd = dir.path().join("crowd");
    let vault = crowd.join("skills/vault-credentials");
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(
        vault.join("SKILL.md"),
        "---\nname: vault-credentials\ndescription: Read secrets from a vault CLI.\n---\n",
    )
    .unwrap();
    let cwd = dir.path().join("plane");
    std::fs::create_dir_all(&cwd).unwrap();
    let argv = dir.path().join("argv");
    let pick = |starts: bool| {
        recorded(
            harness,
            if starts {
                "started.jsonl"
            } else {
                "not-started.jsonl"
            },
        )
        .display()
        .to_string()
    };
    let (name, armed, crowded) = match harness {
        Harness::ClaudeCode => (
            "claude",
            r#"[ -n "$ANTHROPIC_API_KEY" ] || { echo 'no key' >&2; exit 3; }"#.to_owned(),
            format!(
                r#"crowd=no
for word in "$@"; do [ "$word" = '{}' ] && crowd=yes; done"#,
                crowd.display()
            ),
        ),
        Harness::Codex => (
            "codex",
            format!(
                r#"[ "$1" = --version ] && {{ echo 'codex-cli 0.167.0'; exit 0; }}
[ -n "$CODEX_API_KEY" ] || {{ echo 'no key' >&2; exit 3; }}
[ -n "$ANTHROPIC_API_KEY$OPENAI_API_KEY" ] && {{ echo 'another key' >&2; exit 3; }}
[ "$PURLIS_SKILLS_DIR" = '{skills}' ] || {{ echo "not briefed: $PURLIS_SKILLS_DIR" >&2; exit 3; }}
[ -d "$CODEX_HOME" ] || {{ echo 'no home of its own' >&2; exit 3; }}"#,
                skills = plugin.join("skills").display(),
            ),
            r#"crowd=no
[ -n "$(ls "$CODEX_HOME/skills" 2>/dev/null)" ] && crowd=yes"#
                .to_owned(),
        ),
        Harness::Opencode => (
            "opencode",
            r#"[ "$1" = --version ] && { echo '1.19.5'; exit 0; }
[ -n "$ANTHROPIC_API_KEY" ] || { echo 'no key' >&2; exit 3; }
case "$OPENCODE_CONFIG_CONTENT" in *purlis*) ;; *) echo 'not armed' >&2; exit 3 ;; esac"#
                .to_owned(),
            format!(
                r#"crowd=no
case "$OPENCODE_CONFIG_CONTENT" in *'{}'*) crowd=yes ;; esac"#,
                crowd.join("skills").display()
            ),
        ),
    };
    let script = format!(
        r#"#!/bin/sh
printf '%s\n' "$@" >> '{argv}'
{armed}
{crowded}
for word in "$@"; do query="$word"; done
case "$query" in
  *vault*|*token*)
    if [ $crowd = yes ]; then out='{among}'; else out='{bare}'; fi ;;
  *) out='{nothing}' ;;
esac
sed 's#{recorded_plugin}#{plugin}#g' "$out"
"#,
        argv = argv.display(),
        among = pick(among_crowd),
        bare = pick(bare),
        nothing = pick(false),
        recorded_plugin = RECORDED_PLUGIN,
        plugin = plugin.display(),
    );
    let program = stand_in::program(dir.path(), name, &script);
    Fixture {
        dir,
        harness,
        plugin,
        crowd,
        cwd,
        program,
        argv,
    }
}

impl Fixture {
    pub fn ask<'a>(&'a self, key: Option<&'a str>) -> skill_grader::Ask<'a> {
        skill_grader::Ask {
            harness: self.harness,
            program: &self.program,
            purlis: Path::new("/usr/bin/true"),
            plugin: &self.plugin,
            crowd: &self.crowd,
            cwd: &self.cwd,
            pinned: match self.harness {
                Harness::ClaudeCode => "2.1.316",
                Harness::Codex => "0.167.0",
                Harness::Opencode => "1.19.5",
            },
            skills: vec!["purlis-secrets".to_owned()],
            key,
            records: self.dir.path().join("records"),
            parallel: 4,
        }
    }
}
