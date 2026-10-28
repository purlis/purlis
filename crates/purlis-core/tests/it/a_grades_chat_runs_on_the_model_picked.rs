// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A grade's chats run on the model the person picked (SK-60, #3240; spec #2762 decision 22, "on
//! a chosen harness and model"): one of those the profile's harness offers, started by its
//! adapter's own words after the profile's, and its dollar limit priced from it
//! ([`purlis_core::skills::grades::ready_to_grade_on`]). A model the harness does not offer is
//! refused, so nothing but a name it takes reaches the line.

use std::fs;
use std::path::Path;

use purlis_core::profiles;
use purlis_core::sandbox;
use purlis_core::skills::grades;
use purlis_core::start::Start;

/// A project whose chats run in the full sandbox, on a Claude Code profile `work`.
struct Project {
    dir: tempfile::TempDir,
    _config: tempfile::TempDir,
    _outside: stand_in::NoChatWrites,
}

impl Project {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").unwrap();
        let outside = stand_in::NoChatWrites::new();
        let claude = stand_in::claude_code(outside.path(), "claude", "#!/bin/sh\nexit 0\n");
        let config = tempfile::tempdir().unwrap();
        fs::create_dir_all(config.path().join("plugins")).unwrap();
        fs::write(
            config.path().join("plugins/installed_plugins.json"),
            r#"{"version": 2, "plugins": {}}"#,
        )
        .unwrap();
        fs::write(
            root.join(profiles::LOCAL_FILE),
            format!(
                "[harness.work]\nkind = \"claude\"\ncommand = [{:?}]\nenv = {{ CLAUDE_CONFIG_DIR = {:?} }}\n",
                claude.display().to_string(),
                config.path().display().to_string()
            ),
        )
        .unwrap();
        let project = Self {
            dir,
            _config: config,
            _outside: outside,
        };
        let set = profiles::current(project.root());
        let profile = set.get("work").expect("the profile is declared");
        purlis_core::profiletrust::record_launched(
            project.root(),
            "work",
            &purlis_core::profiletrust::fingerprint(profile),
        )
        .unwrap();
        project
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
    let project = Project::new();
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
    let project = Project::new();
    for model in ["gpt-5", "--dangerously-skip-permissions", "opus --x"] {
        let why = grades::ready_to_grade_on(&project.start(), project.root(), Some(model))
            .expect_err("only an offered model reaches the line");
        assert!(why.contains("opus, sonnet, haiku"), "{model}: {why}");
    }
}
