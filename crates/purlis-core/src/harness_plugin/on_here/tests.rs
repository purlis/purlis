// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Each state measured on Claude Code 2.1.316 (`on_here`'s head), from files in a temporary folder.

use std::path::{Path, PathBuf};

use super::super::{Adapter, CLAUDE_CODE, Env};
use super::*;

struct Machine {
    _tmp: tempfile::TempDir,
    config: PathBuf,
    cwd: PathBuf,
    chat: Vec<(String, String)>,
}

impl Machine {
    /// A machine whose record holds `plugins`, and whose marketplace `market` is known.
    fn new(plugins: serde_json::Value) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("cfg");
        let cwd = tmp.path().join("here");
        std::fs::create_dir_all(config.join("plugins")).unwrap();
        std::fs::create_dir_all(cwd.join(".claude")).unwrap();
        std::fs::write(
            config.join("plugins/installed_plugins.json"),
            serde_json::json!({"version": 2, "plugins": plugins}).to_string(),
        )
        .unwrap();
        std::fs::write(
            config.join("plugins/known_marketplaces.json"),
            r#"{"market":{}}"#,
        )
        .unwrap();
        let chat = vec![("CLAUDE_CONFIG_DIR".to_owned(), config.display().to_string())];
        Self {
            _tmp: tmp,
            config,
            cwd,
            chat,
        }
    }

    fn user(&self, text: &str) -> &Self {
        std::fs::write(self.config.join("settings.json"), text).unwrap();
        self
    }

    fn project(&self, file: &str, text: &str) -> &Self {
        std::fs::write(self.cwd.join(".claude").join(file), text).unwrap();
        self
    }

    fn read(&self, reads_project: bool) -> Plugins {
        CLAUDE_CODE
            .plugins_here(
                &Env {
                    chat: &self.chat,
                    home: None,
                    process: false,
                },
                &ChatFolder {
                    cwd: Some(self.cwd.clone()),
                    reads_project,
                },
            )
            .unwrap()
    }
}

const ON: &str = r#"{"enabledPlugins":{"superpowers@market":true}}"#;
const OFF: &str = r#"{"enabledPlugins":{"superpowers@market":false}}"#;

fn at_user() -> Machine {
    Machine::new(serde_json::json!({"superpowers@market": [{"scope": "user"}]}))
}

fn why(plugins: &Plugins) -> Option<&OffWhy> {
    plugins.off.first().map(|one| &one.why)
}

#[test]
fn a_plugin_the_persons_settings_turn_on_is_the_chats() {
    let machine = at_user();
    machine.user(ON);
    let plugins = machine.read(true);
    assert_eq!(plugins.on, ["superpowers"]);
    assert!(plugins.off.is_empty());
}

#[test]
fn a_plugin_nothing_names_or_named_false_is_off() {
    let machine = at_user();
    assert_eq!(why(&machine.read(true)), Some(&OffWhy::TurnedOff));
    machine.user(OFF);
    assert_eq!(why(&machine.read(true)), Some(&OffWhy::TurnedOff));
}

#[test]
fn only_a_literal_true_turns_a_plugin_on() {
    let machine = at_user();
    machine.user(r#"{"enabledPlugins":{"superpowers@market":["1.0.0"]}}"#);
    assert_eq!(why(&machine.read(true)), Some(&OffWhy::TurnedOff));
}

#[test]
fn this_folders_settings_win_over_the_persons_while_the_chat_reads_them() {
    let machine = at_user();
    machine.user(ON).project("settings.json", OFF);
    assert_eq!(why(&machine.read(true)), Some(&OffWhy::ProjectOff));
    assert_eq!(machine.read(false).on, ["superpowers"]);
}

#[test]
fn a_plugin_only_the_project_turns_on_is_off_for_a_chat_that_does_not_read_it() {
    let machine = at_user();
    machine.user(OFF).project("settings.json", ON);
    assert_eq!(machine.read(true).on, ["superpowers"]);
    assert_eq!(why(&machine.read(false)), Some(&OffWhy::ProjectUnread));
}

#[test]
fn the_local_file_wins_over_the_project_file() {
    let machine = at_user();
    machine
        .project("settings.json", ON)
        .project("settings.local.json", OFF);
    assert_eq!(why(&machine.read(true)), Some(&OffWhy::ProjectOff));
}

#[test]
fn a_plugin_installed_for_another_folder_says_which() {
    let machine = Machine::new(serde_json::json!({
        "superpowers@market": [{"scope": "project", "projectPath": "/work/site"}],
    }));
    assert_eq!(
        why(&machine.read(true)),
        Some(&OffWhy::ElsewhereOnly(
            Path::new("/work/site").to_path_buf()
        ))
    );
}

#[test]
fn a_plugin_installed_elsewhere_that_the_person_turns_on_is_the_chats() {
    let machine = Machine::new(serde_json::json!({
        "superpowers@market": [{"scope": "local", "projectPath": "/work/site"}],
    }));
    machine.user(ON);
    assert_eq!(machine.read(true).on, ["superpowers"]);
}

#[test]
fn an_enabled_plugin_whose_marketplace_nothing_declares_is_not_loaded() {
    let machine = at_user();
    machine.user(ON);
    std::fs::remove_file(machine.config.join("plugins/known_marketplaces.json")).unwrap();
    assert_eq!(
        why(&machine.read(true)),
        Some(&OffWhy::MarketplaceGone("market".to_owned()))
    );
}

#[test]
fn a_marketplace_declared_in_settings_alone_is_known() {
    let machine = at_user();
    machine.user(
        r#"{"enabledPlugins":{"superpowers@market":true},"extraKnownMarketplaces":{"market":{}}}"#,
    );
    std::fs::remove_file(machine.config.join("plugins/known_marketplaces.json")).unwrap();
    assert_eq!(machine.read(true).on, ["superpowers"]);
}

#[test]
fn one_install_on_gives_the_chat_that_names_skills_whichever_marketplace() {
    let machine = Machine::new(serde_json::json!({
        "superpowers@market": [{"scope": "user"}],
        "superpowers@other": [{"scope": "user"}],
    }));
    machine.user(ON);
    let plugins = machine.read(true);
    assert_eq!(plugins.on, ["superpowers"]);
    assert!(!plugins.is_off("superpowers"));
}

#[test]
fn a_marketplace_gone_offers_no_command_and_turned_off_offers_enable() {
    let gone = PluginOff {
        id: "superpowers@market".to_owned(),
        name: "superpowers".to_owned(),
        why: OffWhy::MarketplaceGone("market".to_owned()),
    };
    assert_eq!(gone.command(), None);
    let off = PluginOff {
        why: OffWhy::TurnedOff,
        ..gone
    };
    assert_eq!(
        off.command().as_deref(),
        Some("claude plugin enable superpowers@market")
    );
}

#[test]
fn the_local_file_at_the_top_of_the_repository_wins_for_a_chat_in_a_subfolder() {
    let machine = at_user();
    machine.user(OFF).project("settings.json", ON);
    assert_eq!(machine.read(true).on, ["superpowers"]);
    std::fs::create_dir(machine.cwd.join(".git")).unwrap();
    let top = machine.cwd.join("sub");
    std::fs::create_dir_all(top.join(".claude")).unwrap();
    std::fs::write(top.join(".claude/settings.json"), ON).unwrap();
    machine.project("settings.local.json", OFF);
    let plugins = CLAUDE_CODE
        .plugins_here(
            &Env {
                chat: &machine.chat,
                home: None,
                process: false,
            },
            &ChatFolder {
                cwd: Some(top),
                reads_project: true,
            },
        )
        .unwrap();
    assert_eq!(why(&plugins), Some(&OffWhy::ProjectOff));
}

#[test]
fn a_record_that_is_not_there_is_nothing_installed_and_one_unreadable_says_nothing() {
    let machine = at_user();
    let record = machine.config.join("plugins/installed_plugins.json");
    let env = Env {
        chat: &machine.chat,
        home: None,
        process: false,
    };
    std::fs::write(&record, "not json").unwrap();
    assert_eq!(CLAUDE_CODE.plugins_here(&env, &ChatFolder::default()), None);
    std::fs::remove_file(&record).unwrap();
    assert_eq!(
        CLAUDE_CODE.plugins_here(&env, &ChatFolder::default()),
        Some(Plugins::default())
    );
}

#[test]
fn a_plugin_purlis_pins_off_is_off_and_its_own_is_not_listed() {
    let machine = Machine::new(serde_json::json!({
        "charter@charter": [{"scope": "user"}],
        "purlis@inline": [{"scope": "user"}],
    }));
    let plugins = machine.read(true);
    assert_eq!(why(&plugins), Some(&OffWhy::PurlisOff));
    assert_eq!(plugins.off.len(), 1);
    assert!(plugins.on.is_empty());
}

#[test]
fn a_chat_is_told_to_restart_and_a_new_one_is_told_to_start() {
    let off = PluginOff {
        id: "p@m".to_owned(),
        name: "p".to_owned(),
        why: OffWhy::TurnedOff,
    };
    assert!(
        off.said("Claude Code", true)
            .ends_with("then restart this chat.")
    );
    assert!(
        off.said("Claude Code", false)
            .ends_with("then start a new chat.")
    );
}

#[test]
fn a_harness_that_does_not_read_this_says_nothing() {
    let env = Env {
        chat: &[],
        home: None,
        process: false,
    };
    assert_eq!(
        crate::harness_plugin::OPENCODE.plugins_here(&env, &ChatFolder::default()),
        None
    );
}

// Codex, as measured on codex-cli 0.163.0 (ADR 0050 as amended 2026-10-21): a plugin whose
// `config.toml` table says `enabled = false` does not reach the model.

/// A Codex home whose `config.toml` is `text`, and what a chat there has of its plugins.
fn codex_with(text: Option<&str>) -> (tempfile::TempDir, Option<Plugins>) {
    let home = tempfile::tempdir().unwrap();
    if let Some(text) = text {
        std::fs::write(home.path().join("config.toml"), text).unwrap();
    }
    let chat = [("CODEX_HOME".to_owned(), home.path().display().to_string())];
    let got = crate::harness_plugin::CODEX.plugins_here(
        &Env {
            chat: &chat,
            home: None,
            process: false,
        },
        &ChatFolder::default(),
    );
    (home, got)
}

#[test]
fn a_codex_plugin_its_config_turns_off_is_not_the_chats_and_says_where() {
    let (home, got) = codex_with(Some(
        "[plugins.\"superpowers@market\"]\nenabled = false\n\n\
         [plugins.\"deep-research@openai-curated\"]\nenabled = true\n",
    ));
    let plugins = got.expect("it reads");
    assert_eq!(plugins.on, ["deep-research"]);
    assert!(plugins.is_off("superpowers"));
    assert_eq!(
        why(&plugins),
        Some(&OffWhy::OffInConfig(home.path().join("config.toml")))
    );
}

#[test]
fn a_codex_plugin_whose_table_does_not_say_enabled_is_on() {
    let (_home, got) = codex_with(Some("[plugins.\"superpowers@market\"]\n"));
    let plugins = got.expect("it reads");
    assert_eq!(plugins.on, ["superpowers"]);
    assert!(plugins.off.is_empty());
}

#[test]
fn a_codex_plugin_off_in_config_names_the_file_and_offers_no_claude_command() {
    let off = PluginOff {
        id: "superpowers@market".to_owned(),
        name: "superpowers".to_owned(),
        why: OffWhy::OffInConfig(PathBuf::from("/h/.codex/config.toml")),
    };
    assert_eq!(off.command(), None);
    assert_eq!(off.line("Codex"), "Off in Codex");
    let said = off.said("Codex", true);
    assert!(said.contains("/h/.codex/config.toml"), "{said}");
    assert!(said.contains("enabled = false"), "{said}");
    assert!(said.ends_with("then restart this chat."), "{said}");
}

#[test]
fn a_codex_config_that_is_not_there_is_nothing_installed_and_one_unreadable_says_nothing() {
    assert_eq!(codex_with(None).1, Some(Plugins::default()));
    assert_eq!(codex_with(Some("not = [toml")).1, None);
}
