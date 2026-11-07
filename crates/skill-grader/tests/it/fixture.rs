// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A built-in skill with two trigger tests each way, its plugin, the crowd, and a stand-in
//! `claude` that replays the scoring module's recorded Claude Code output.

use std::path::{Path, PathBuf};

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub plugin: PathBuf,
    pub crowd: PathBuf,
    pub cwd: PathBuf,
    pub claude: PathBuf,
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

fn recorded(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../skills/src/grade/ci/recorded/claude-code")
        .join(name)
}

/// The stand-in's answer: `among_crowd` and `bare` say whether a query about a credential
/// starts purlis-secrets in that listing; every other query starts nothing.
pub fn fixture(among_crowd: bool, bare: bool) -> Fixture {
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
    let crowd = dir.path().join("crowd");
    std::fs::create_dir_all(crowd.join("skills")).unwrap();
    let cwd = dir.path().join("plane");
    std::fs::create_dir_all(&cwd).unwrap();
    let argv = dir.path().join("argv");
    let pick = |starts: bool| {
        recorded(if starts {
            "started.jsonl"
        } else {
            "not-started.jsonl"
        })
        .display()
        .to_string()
    };
    let script = format!(
        r#"#!/bin/sh
printf '%s\n' "$@" >> '{argv}'
[ -n "$ANTHROPIC_API_KEY" ] || {{ echo 'no key' >&2; exit 3; }}
crowd=no
for word in "$@"; do [ "$word" = '{crowd}' ] && crowd=yes; done
for word in "$@"; do query="$word"; done
case "$query" in
  *vault*|*token*)
    if [ $crowd = yes ]; then cat '{crowded}'; else cat '{bare}'; fi ;;
  *) cat '{nothing}' ;;
esac
"#,
        argv = argv.display(),
        crowd = crowd.display(),
        crowded = pick(among_crowd),
        bare = pick(bare),
        nothing = pick(false),
    );
    let claude = stand_in::program(dir.path(), "claude", &script);
    Fixture {
        dir,
        plugin,
        crowd,
        cwd,
        claude,
        argv,
    }
}

impl Fixture {
    pub fn ask<'a>(&'a self, key: Option<&'a str>) -> skill_grader::Ask<'a> {
        skill_grader::Ask {
            claude: &self.claude,
            purlis: Path::new("/usr/bin/true"),
            plugin: &self.plugin,
            crowd: &self.crowd,
            cwd: &self.cwd,
            pinned: "2.1.316",
            skills: vec!["purlis-secrets".to_owned()],
            key,
            records: self.dir.path().join("records"),
            parallel: 4,
        }
    }
}
