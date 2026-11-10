// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A grade's chats run on the model the person picked (SK-60, #3240; spec #2762 decision 22, "on
//! a chosen harness and model"): one of those the profile's harness offers, started by its
//! adapter's own words after the profile's, and its dollar limit priced from it
//! ([`purlis_core::skills::grades::ready_to_grade_on`]). A model the harness does not offer is
//! refused, so nothing but a name it takes reaches the line.

use std::fs;
use std::path::Path;

use crate::support::{self, ClaudeProfile};

use purlis_core::profiles;
use purlis_core::sandbox;
use purlis_core::skills::grades;
use purlis_core::start::Start;

/// A project whose chats run in the full sandbox, on a Claude Code profile `work`: the
/// project's own, or, where purlis wraps a sandboxed Claude Code chat, this machine's
/// sandbox-only one ([`support::sandboxed_claude_profile`]).
struct Project {
    dir: tempfile::TempDir,
    _config: Option<tempfile::TempDir>,
    _outside: stand_in::NoChatWrites,
}

impl Project {
    /// The project for `test`, or `None` where a child ran it.
    fn new(test: &str) -> Option<Self> {
        let sandbox_only = match support::sandboxed_claude_profile(test) {
            ClaudeProfile::Ran => return None,
            ClaudeProfile::Project => None,
            ClaudeProfile::SandboxOnly(config) => Some(config),
        };
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").unwrap();
        let outside = stand_in::NoChatWrites::new();
        let (config, kept) = match sandbox_only {
            Some(config) => (config, None),
            None => {
                let kept = tempfile::tempdir().unwrap();
                (kept.path().to_path_buf(), Some(kept))
            }
        };
        fs::create_dir_all(config.join("plugins")).unwrap();
        fs::write(
            config.join("plugins/installed_plugins.json"),
            r#"{"version": 2, "plugins": {}}"#,
        )
        .unwrap();
        if kept.is_some() {
            let claude = stand_in::claude_code(outside.path(), "claude", "#!/bin/sh\nexit 0\n");
            fs::write(
                root.join(profiles::LOCAL_FILE),
                format!(
                    "[harness.work]\nkind = \"claude\"\ncommand = [{:?}]\nenv = {{ CLAUDE_CONFIG_DIR = {:?} }}\n",
                    claude.display().to_string(),
                    config.display().to_string()
                ),
            )
            .unwrap();
            let set = profiles::current(root);
            let profile = set.get("work").expect("the profile is declared");
            purlis_core::profiletrust::record_launched(
                root,
                "work",
                &purlis_core::profiletrust::fingerprint(profile),
            )
            .unwrap();
        }
        Some(Self {
            dir,
            _config: kept,
            _outside: outside,
        })
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn start(&self) -> Start {
        Start {
            profile: Some("work".to_owned()),
            name: "grade.1".to_owned(),
            cwd: Some(self.root().to_path_buf()),
            made: sandbox::Made::Window,
            ..Default::default()
        }
    }
}

#[test]
fn a_grades_chat_runs_on_the_model_picked_and_is_priced_from_it() {
    purlis_core::unsteered!();
    let Some(project) = Project::new(
        "a_grades_chat_runs_on_the_model_picked::a_grades_chat_runs_on_the_model_picked_and_is_priced_from_it",
    ) else {
        return;
    };
    let root = project.root();

    let own = grades::ready_to_grade(&project.start(), root).expect("a grade's chat starts");
    assert_eq!(
        grades::models_for(&own),
        ["opus", "sonnet", "haiku"],
        "the plan offers what Claude Code's --model takes"
    );
    assert!(
        !own.command.iter().any(|word| word == "--model"),
        "the profile's own model names none: {:?}",
        own.command
    );

    let picked = grades::ready_to_grade_on(&project.start(), root, Some("sonnet"))
        .expect("a grade's chat starts on the model picked");
    assert_eq!(
        picked.command[picked.command.len() - 2..],
        ["--model".to_owned(), "sonnet".to_owned()],
        "the model picked is the last the line names"
    );
    let priced = grades::priced(&picked);
    assert_eq!(priced.model.as_deref(), Some("sonnet"));
    assert_eq!(
        priced.from,
        purlis_core::skills::grade::PricedFrom::Model,
        "its limit is priced from the model picked"
    );
}

#[test]
fn a_model_the_harness_does_not_offer_is_refused() {
    purlis_core::unsteered!();
    let Some(project) = Project::new(
        "a_grades_chat_runs_on_the_model_picked::a_model_the_harness_does_not_offer_is_refused",
    ) else {
        return;
    };
    for model in ["gpt-5", "--dangerously-skip-permissions", "opus --x"] {
        let why = grades::ready_to_grade_on(&project.start(), project.root(), Some(model))
            .expect_err("only an offered model reaches the line");
        assert!(why.contains("opus, sonnet, haiku"), "{model}: {why}");
    }
}
