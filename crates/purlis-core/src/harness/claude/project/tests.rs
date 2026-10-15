// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// What a chat in `cwd` finds, on a machine where the person installed `tidy@team` alone.
fn screen(cwd: Option<&Path>) -> Screened {
    screen_with(cwd, Some(&vec!["tidy@team".to_owned()]))
}

fn write(dir: &Path, file: &str, text: &str) {
    let path = dir.join(file);
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("folders");
    std::fs::write(path, text).expect("written");
}

#[test]
fn a_project_with_no_settings_files_is_loaded_as_it_is() {
    let dir = tempfile::tempdir().expect("a folder");
    let screened = screen(Some(dir.path()));
    assert!(screened.loads_the_project(), "{screened:?}");
    assert!(screened.setting_sources().is_empty());
    assert_eq!(screened.note(), None);
}

#[test]
fn a_project_holding_only_kept_keys_is_loaded_as_it_is() {
    let dir = tempfile::tempdir().expect("a folder");
    write(
        dir.path(),
        ".claude/settings.json",
        r#"{"$schema": "https://example.invalid/s.json",
            "permissions": {"deny": ["Bash(rm *)"], "ask": ["Bash(git push *)"]},
            "enabledPlugins": {"tidy@team": true, "old@team": false},
            "model": "sonnet", "outputStyle": "Explanatory", "theme": "dark"}"#,
    );
    write(
        dir.path(),
        ".claude/settings.local.json",
        r#"{"permissions": {"deny": ["Read(./secrets/**)"]}}"#,
    );
    let screened = screen(Some(dir.path()));
    assert!(screened.loads_the_project(), "{screened:?}");
    assert_eq!(screened.deny, ["Bash(rm *)", "Read(./secrets/**)"]);
    assert_eq!(screened.ask, ["Bash(git push *)"]);
}

#[test]
fn any_other_key_keeps_the_project_s_files_from_loading_and_is_named() {
    let dir = tempfile::tempdir().expect("a folder");
    write(
        dir.path(),
        ".claude/settings.json",
        r#"{"permissions": {"allow": ["Bash"], "deny": ["Bash(rm *)"], "ask": ["Edit"],
                            "defaultMode": "plan"},
            "hooks": {"Stop": []}, "sandbox": {}, "env": {"PROBE": "1"}}"#,
    );
    let screened = screen(Some(dir.path()));
    assert!(!screened.loads_the_project());
    assert_eq!(
        screened.dropped,
        [
            "env",
            "hooks",
            "permissions.allow",
            "permissions.defaultMode",
            "sandbox"
        ]
    );
    assert_eq!(screened.setting_sources(), ["--setting-sources", "user"]);
    // Its deny and ask rules are still what it holds, for purlis to carry.
    assert_eq!(screened.deny, ["Bash(rm *)"]);
    assert_eq!(screened.ask, ["Edit"]);
    let note = screened.note().expect("a note");
    assert!(note.contains("env, hooks, permissions.allow"), "{note}");
    assert!(note.contains("CLAUDE.md"), "{note}");
}

#[test]
fn a_kept_key_of_the_wrong_shape_is_not_kept() {
    let dir = tempfile::tempdir().expect("a folder");
    write(
        dir.path(),
        ".claude/settings.local.json",
        r#"{"permissions": {"deny": "Bash"}, "enabledPlugins": {"a@b": "yes"}, "model": 3}"#,
    );
    let screened = screen(Some(dir.path()));
    assert_eq!(
        screened.dropped,
        ["enabledPlugins", "model", "permissions.deny"]
    );
}

#[test]
fn a_file_that_cannot_be_read_as_settings_keeps_the_project_s_files_from_loading() {
    for text in ["{ not json", "[]", "\"a string\""] {
        let dir = tempfile::tempdir().expect("a folder");
        write(dir.path(), ".claude/settings.json", text);
        let screened = screen(Some(dir.path()));
        assert_eq!(screened.unreadable, [".claude/settings.json"], "{text}");
        assert!(!screened.loads_the_project());
        assert!(
            screened
                .note()
                .expect("a note")
                .contains("could not be read")
        );
    }
    // A folder where the file would be.
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::create_dir_all(dir.path().join(".claude/settings.local.json")).expect("a folder");
    assert!(!screen(Some(dir.path())).loads_the_project());
}

#[test]
fn a_chat_whose_folder_is_not_known_does_not_load_project_files() {
    let screened = screen(None);
    assert!(!screened.loads_the_project());
    assert_eq!(screened.setting_sources(), ["--setting-sources", "user"]);
    assert!(screened.note().is_some());
}

#[test]
fn the_local_file_at_the_top_of_the_repository_is_read_too() {
    let dir = tempfile::tempdir().expect("a folder");
    let repo = dir.path().join("repo");
    let chat = repo.join("src/deep");
    std::fs::create_dir_all(repo.join(".git")).expect("a repository");
    std::fs::create_dir_all(&chat).expect("the chat's folder");
    write(
        &repo,
        ".claude/settings.local.json",
        r#"{"statusLine": {}}"#,
    );
    // The repository's shared file is not one Claude Code loads for a chat below its top.
    write(&repo, ".claude/settings.json", r#"{"other": 1}"#);
    let screened = screen(Some(&chat));
    assert_eq!(screened.dropped, ["statusLine"]);
}

#[test]
fn a_linked_worktree_s_main_checkout_local_file_is_read_too() {
    let dir = tempfile::tempdir().expect("a folder");
    let main = dir.path().join("main");
    let gitdir = main.join(".git/worktrees/piece");
    std::fs::create_dir_all(&gitdir).expect("the worktree's git folder");
    std::fs::write(gitdir.join("commondir"), "../..\n").expect("commondir");
    let piece = dir.path().join("piece");
    std::fs::create_dir_all(&piece).expect("the worktree");
    std::fs::write(
        piece.join(".git"),
        format!("gitdir: {}\n", gitdir.display()),
    )
    .expect(".git");
    write(
        &main,
        ".claude/settings.local.json",
        r#"{"env": {"PROBE": "1"}}"#,
    );
    let screened = screen(Some(&piece));
    assert_eq!(screened.dropped, ["env"]);
}

#[test]
fn a_name_from_the_project_is_shown_printable_and_short() {
    let dir = tempfile::tempdir().expect("a folder");
    let long = "k".repeat(200);
    write(
        dir.path(),
        ".claude/settings.json",
        &format!(r#"{{"a\nb\u001b[31m": 1, "{long}": 2}}"#),
    );
    let note = screen(Some(dir.path())).note().expect("a note");
    assert!(!note.contains('\n') && !note.contains('\u{1b}'), "{note}");
    assert!(!note.contains(&long), "{note}");
}

#[test]
fn what_purlis_s_own_init_writes_is_kept() {
    // The harness variable and the guard hook, as `init` writes them into a plane's file.
    let dir = tempfile::tempdir().expect("a folder");
    let file = serde_json::json!({
        "env": {"CHARTER_HARNESS": "claude-code"},
        "permissions": {"ask": ["Bash(charter report *--yes*)"]},
        "hooks": {"PreToolUse": [crate::scaffold::settings::guard_hook()]},
    });
    write(dir.path(), ".claude/settings.json", &file.to_string());
    let screened = screen(Some(dir.path()));
    assert!(screened.loads_the_project(), "{screened:?}");
    for spelling in ["PURLIS_HARNESS", "CHARTER_HARNESS"] {
        let file = serde_json::json!({"env": {spelling: "claude-code"}});
        write(dir.path(), ".claude/settings.json", &file.to_string());
        assert!(screen(Some(dir.path())).loads_the_project(), "{spelling}");
    }
}

#[test]
fn anything_else_under_purlis_s_own_keys_is_not_kept() {
    let mut other_hook = crate::scaffold::settings::guard_hook();
    other_hook["hooks"][0]["command"] = serde_json::json!("true");
    for file in [
        serde_json::json!({"env": {"CHARTER_HARNESS": "claude-code", "OTHER": "1"}}),
        serde_json::json!({"env": {"CHARTER_HARNESS": 1}}),
        serde_json::json!({"hooks": {"PreToolUse": [other_hook]}}),
        serde_json::json!({"hooks": {"Stop": [crate::scaffold::settings::guard_hook()]}}),
        serde_json::json!({"hooks": {"PreToolUse": [crate::scaffold::settings::guard_hook(), {}]}}),
    ] {
        let dir = tempfile::tempdir().expect("a folder");
        write(dir.path(), ".claude/settings.json", &file.to_string());
        let screened = screen(Some(dir.path()));
        assert!(!screened.loads_the_project(), "{file}");
    }
}

#[test]
fn the_harness_variable_is_kept_only_with_the_value_init_gives_it() {
    let dir = tempfile::tempdir().expect("a folder");
    for (value, kept) in [("claude-code", true), ("codex", false), ("", false)] {
        let file = serde_json::json!({"env": {"PURLIS_HARNESS": value}});
        write(dir.path(), ".claude/settings.json", &file.to_string());
        let screened = screen(Some(dir.path()));
        assert_eq!(screened.loads_the_project(), kept, "{value:?}");
        if !kept {
            assert_eq!(screened.dropped, ["env"]);
        }
    }
}

#[cfg(unix)]
#[test]
fn a_folder_reached_through_a_link_still_has_its_repository_s_file_read() {
    let dir = tempfile::tempdir().expect("a folder");
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect("a repository");
    std::fs::create_dir_all(repo.join("sub")).expect("a folder in it");
    write(
        &repo,
        ".claude/settings.local.json",
        r#"{"statusLine": {}}"#,
    );
    let link = dir.path().join("elsewhere");
    std::fs::create_dir_all(&link).expect("a folder outside it");
    std::os::unix::fs::symlink(repo.join("sub"), link.join("link")).expect("a link");
    let screened = screen(Some(&link.join("link")));
    assert_eq!(screened.dropped, ["statusLine"], "{screened:?}");
}

#[test]
fn plugins_are_kept_only_where_they_choose_among_what_the_person_has() {
    use crate::harness_plugin::Adapter as _;
    let pinned_on = crate::harness_plugin::CLAUDE_CODE
        .pinned()
        .iter()
        .find(|pin| pin.on)
        .expect("a plugin purlis pins on")
        .id;
    let one = |id: &str, on: bool| {
        let mut plugins = serde_json::Map::new();
        plugins.insert(id.to_owned(), serde_json::Value::Bool(on));
        serde_json::Value::Object(plugins)
    };
    for (plugins, kept) in [
        (
            serde_json::json!({"tidy@team": true, "other@team": false}),
            true,
        ),
        (one(pinned_on, true), true),
        (serde_json::json!({"never@installed": true}), false),
        (one(pinned_on, false), false),
    ] {
        let dir = tempfile::tempdir().expect("a folder");
        let file = serde_json::json!({ "enabledPlugins": plugins });
        write(dir.path(), ".claude/settings.json", &file.to_string());
        let screened = screen(Some(dir.path()));
        assert_eq!(screened.loads_the_project(), kept, "{plugins}");
    }
    // Where the record of what the person installed cannot be read, nothing turned on is kept.
    let dir = tempfile::tempdir().expect("a folder");
    write(
        dir.path(),
        ".claude/settings.json",
        r#"{"enabledPlugins": {"tidy@team": true}}"#,
    );
    assert!(!screen_with(Some(dir.path()), None).loads_the_project());
}

#[test]
fn the_person_s_plugins_are_read_from_claude_code_s_record_at_user_scope() {
    let dir = tempfile::tempdir().expect("a folder");
    let record = dir.path().join("installed_plugins.json");
    std::fs::write(
        &record,
        r#"{"version": 2, "plugins": {
            "mine@team": [{"scope": "user"}],
            "theirs@team": [{"scope": "project"}],
            "both@team": [{"scope": "local"}, {"scope": "user"}]
        }}"#,
    )
    .expect("the record");
    let mut ids = installed_for_the_person(Some(&record)).expect("read");
    ids.sort();
    assert_eq!(ids, ["both@team", "mine@team"]);
    assert_eq!(
        installed_for_the_person(Some(&dir.path().join("none"))),
        None
    );
    assert_eq!(installed_for_the_person(None), None);
}
