// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A grade's chat has only purlis's plugin on (#3468, the operator's ruling P8 of 2026-10-27 on
//! spec #2762): every plugin Claude Code has installed is handed off for that chat alone, so a
//! grade measures the skill alone, while an ordinary chat leaves them to Claude Code and nothing
//! of the person's own settings is written
//! ([`purlis_core::skills::grades::ready_to_grade`]).

use std::fs;
use std::path::Path;

use purlis_core::profiles;
use purlis_core::sandbox;
use purlis_core::skills::grades;
use purlis_core::start::{self, Start};

const INSTALLED: &str = r#"{"version": 2, "plugins": {
    "figma@claude-plugins-official": [{"scope": "user", "installPath": "/x"}],
    "superpowers@claude-plugins-official": [{"scope": "project", "installPath": "/y"}]
}}"#;

/// A project whose chats run in the full sandbox, on a Claude Code profile `work` whose own
/// settings folder holds `installed` as its record of installed plugins.
struct Project {
    dir: tempfile::TempDir,
    config: tempfile::TempDir,
    _outside: stand_in::NoChatWrites,
}

impl Project {
    fn new(installed: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").unwrap();
        let outside = stand_in::NoChatWrites::new();
        let claude = stand_in::claude_code(outside.path(), "claude", "#!/bin/sh\nexit 0\n");
        let config = tempfile::tempdir().unwrap();
        fs::create_dir_all(config.path().join("plugins")).unwrap();
        fs::write(
            config.path().join("plugins/installed_plugins.json"),
            installed,
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
            config,
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

    fn record(&self) -> String {
        fs::read_to_string(self.config.path().join("plugins/installed_plugins.json")).unwrap()
    }

    fn start(&self) -> Start {
        Start {
            profile: Some("work".to_owned()),
            persona: None,
            name: "grade.1".to_owned(),
            cwd: Some(self.root().to_path_buf()),
            resume: None,
            show_footer: false,
            resuming: None,
            without_sandbox: None,
            held: None,
            grants: Default::default(),
            made: sandbox::Made::Window,
        }
    }
}

#[test]
fn a_grades_chat_is_handed_every_installed_plugin_off_and_purlis_s_own_on() {
    purlis_core::unsteered!();
    let project = Project::new(INSTALLED);
    let root = project.root();

    // Not vacuous: an ordinary chat names neither plugin, so Claude Code decides them.
    let ordinary = start::ready(&project.start(), root).expect("an ordinary chat starts");
    assert!(
        !ordinary
            .plugins
            .plugins
            .contains_key("figma@claude-plugins-official"),
        "{:?}",
        ordinary.plugins
    );

    let graded = grades::ready_to_grade(&project.start(), root).expect("a grade's chat starts");
    let plugins = &graded.plugins.plugins;
    assert_eq!(plugins.get("figma@claude-plugins-official"), Some(&false));
    assert_eq!(
        plugins.get("superpowers@claude-plugins-official"),
        Some(&false)
    );
    assert_eq!(
        plugins.get(purlis_core::plugin::LOADED_AS),
        Some(&true),
        "purlis's own plugin stays on"
    );
    assert_eq!(
        grades::plugins_off(&graded),
        [
            "figma@claude-plugins-official",
            "superpowers@claude-plugins-official"
        ],
        "what the grade's record names"
    );
    assert_eq!(
        project.record(),
        INSTALLED,
        "nothing of the person's is written"
    );

    // Held only for the grade's chat: the next ordinary chat leaves them to Claude Code again.
    let after = start::ready(&project.start(), root).expect("an ordinary chat starts");
    assert_eq!(after.plugins, ordinary.plugins);
}

#[test]
fn a_grade_whose_plugin_record_cannot_be_read_does_not_start() {
    purlis_core::unsteered!();
    let project = Project::new("{");
    let why = grades::ready_to_grade(&project.start(), project.root())
        .expect_err("a grade that could not say which plugins were off is refused");
    assert!(why.contains("every plugin but purlis's own off"), "{why}");
}
