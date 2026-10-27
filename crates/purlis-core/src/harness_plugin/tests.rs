// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The harness-plugin model (charter-app#274, ADR 0050): the precedence, each adapter's listing
//! from a fixture directory, and what a file may not say. Every expected answer is written out.

use super::*;
use crate::extension::project::Source;

fn plugin(harness: &'static str, id: &str) -> Plugin {
    Plugin {
        id: id.to_owned(),
        harness,
        name: id.split('@').next().unwrap_or(id).to_owned(),
        source: "fixture".to_owned(),
    }
}

fn claude(installed: &[&str], shared: &str, local: &str) -> Vec<Effective> {
    let installed: Vec<Plugin> = installed.iter().map(|id| plugin("claude", id)).collect();
    resolve(
        &CLAUDE_CODE,
        &installed,
        &Choices::from_text(Some(shared), Some(local)),
    )
}

fn row<'a>(all: &'a [Effective], id: &str) -> &'a Effective {
    all.iter()
        .find(|it| it.id == id)
        .unwrap_or_else(|| panic!("{id} is not listed: {all:?}"))
}

// -------------------------------------------------------------------------------------
// The precedence: Local over Shared, per plugin; neither is "not set"
// -------------------------------------------------------------------------------------

#[test]
fn an_installed_plugin_no_file_names_is_not_set_and_left_to_the_harness() {
    let all = claude(&["figma@official"], "", "");
    let it = row(&all, "figma@official");
    assert_eq!((it.wanted, it.source), (None, Source::Default));
    assert!(it.installed);
}

#[test]
fn shared_turns_a_plugin_on_or_off() {
    let all = claude(
        &["figma@official", "serena@official"],
        "[harness_plugins.claude]\n\"figma@official\" = false\n\"serena@official\" = true\n",
        "",
    );
    assert_eq!(
        (
            row(&all, "figma@official").wanted,
            row(&all, "figma@official").source
        ),
        (Some(false), Source::Shared)
    );
    assert_eq!(
        (
            row(&all, "serena@official").wanted,
            row(&all, "serena@official").source
        ),
        (Some(true), Source::Shared)
    );
}

#[test]
fn local_overrides_shared_in_both_directions() {
    let all = claude(
        &["figma@official", "serena@official"],
        "[harness_plugins.claude]\n\"figma@official\" = false\n\"serena@official\" = true\n",
        "[harness_plugins.claude]\n\"figma@official\" = true\n\"serena@official\" = false\n",
    );
    assert_eq!(
        (
            row(&all, "figma@official").wanted,
            row(&all, "figma@official").source
        ),
        (Some(true), Source::Local)
    );
    assert_eq!(
        (
            row(&all, "serena@official").wanted,
            row(&all, "serena@official").source
        ),
        (Some(false), Source::Local)
    );
}

#[test]
fn local_naming_one_plugin_leaves_shared_in_charge_of_the_others() {
    // Per plugin, as #272 is per key: Local saying something about one plugin is not Local
    // taking over the harness's whole table.
    let all = claude(
        &["figma@official", "serena@official"],
        "[harness_plugins.claude]\n\"figma@official\" = false\n",
        "[harness_plugins.claude]\n\"serena@official\" = true\n",
    );
    assert_eq!(
        (
            row(&all, "figma@official").wanted,
            row(&all, "figma@official").source
        ),
        (Some(false), Source::Shared)
    );
}

#[test]
fn a_choice_for_another_harness_does_not_reach_this_one() {
    let all = claude(
        &["figma@official"],
        "[harness_plugins.codex]\n\"figma@official\" = false\n",
        "",
    );
    assert_eq!(row(&all, "figma@official").wanted, None);
}

#[test]
fn a_plugin_a_file_names_that_is_not_installed_is_listed_and_handed_to_nothing() {
    let all = claude(&[], "[harness_plugins.claude]\n\"acme@corp\" = true\n", "");
    let it = row(&all, "acme@corp");
    assert!(!it.installed);
    assert_eq!((it.wanted, it.source), (Some(true), Source::Shared));
    assert_eq!(
        chosen(&CLAUDE_CODE, &all).plugins,
        pins(&CLAUDE_CODE).into_iter().collect::<BTreeMap<_, _>>()
    );
}

// -------------------------------------------------------------------------------------
// A workspace: the layer between Shared and Local (charter-app#282)
// -------------------------------------------------------------------------------------

/// [`claude`], in the workspace `alpha` whose `workspace.json` is `manifest`.
fn claude_in(installed: &[&str], shared: &str, manifest: &str, local: &str) -> Vec<Effective> {
    let installed: Vec<Plugin> = installed.iter().map(|id| plugin("claude", id)).collect();
    resolve(
        &CLAUDE_CODE,
        &installed,
        &Choices::from_text(Some(shared), Some(local)).in_workspace("alpha", Some(manifest)),
    )
}

/// A `workspace.json` whose settings turn each of Claude Code's plugins on or off.
fn ws(pairs: &[(&str, bool)]) -> String {
    let plugins: serde_json::Map<String, serde_json::Value> = pairs
        .iter()
        .map(|(id, on)| ((*id).to_owned(), serde_json::Value::Bool(*on)))
        .collect();
    serde_json::json!({"name": "alpha", "settings": {"harness_plugins": {"claude": plugins}}})
        .to_string()
}

#[test]
fn a_workspace_overrides_shared_and_local_overrides_the_workspace_per_plugin() {
    let all = claude_in(
        &["figma@official", "serena@official", "humanizer@h"],
        "[harness_plugins.claude]\n\"figma@official\" = true\n\"serena@official\" = true\n",
        &ws(&[("figma@official", false), ("serena@official", false)]),
        "[harness_plugins.claude]\n\"serena@official\" = true\n",
    );
    let at = |id| (row(&all, id).wanted, row(&all, id).source);
    assert_eq!(at("figma@official"), (Some(false), Source::Workspace));
    assert_eq!(at("serena@official"), (Some(true), Source::Local));
    assert_eq!(at("humanizer@h"), (None, Source::Default));
}

#[test]
fn a_workspace_that_names_nothing_leaves_the_project_answer_as_it_was() {
    let shared = "[harness_plugins.claude]\n\"figma@official\" = false\n";
    assert_eq!(
        claude_in(&["figma@official"], shared, r#"{"name": "alpha"}"#, ""),
        claude(&["figma@official"], shared, "")
    );
    assert_eq!(
        claude_in(&["figma@official"], shared, "not json", ""),
        claude(&["figma@official"], shared, "")
    );
}

#[test]
fn a_workspace_that_turns_a_pin_the_wrong_way_is_ignored_by_its_own_file_and_key() {
    let all = claude_in(&[], "", &ws(&[("purlis@inline", false)]), "");
    let own = row(&all, "purlis@inline");
    assert_eq!((own.wanted, own.source), (Some(true), Source::Default));
    assert_eq!(
        own.ignored,
        [Ignored {
            source: Source::Workspace,
            why: "workspaces/alpha/workspace.json sets \
                  settings.harness_plugins.claude.\"purlis@inline\" to false, and \
                  purlis@inline is always on: it is purlis's own plugin, and it carries \
                  purlis's hooks and the Bash guard"
                .to_owned(),
        }]
    );
}

#[test]
fn a_workspaces_choice_for_a_harness_that_cannot_apply_is_said_ignored_and_handed_nothing() {
    let installed = [plugin("opencode", "acme-plugin")];
    let manifest = r#"{"settings": {"harness_plugins": {"opencode": {"acme-plugin": false}}}}"#;
    let all = resolve(
        &OPENCODE,
        &installed,
        &Choices::default().in_workspace("alpha", Some(manifest)),
    );
    assert_eq!(chosen(&OPENCODE, &all), Chosen::new());
    let it = row(&all, "acme-plugin");
    assert_eq!(it.ignored.len(), 1, "{:?}", it.ignored);
    assert_eq!(it.ignored[0].source, Source::Workspace);
    assert!(
        it.ignored[0]
            .why
            .contains("plugins for opencode are not supported yet"),
        "{}",
        it.ignored[0].why
    );
}

// -------------------------------------------------------------------------------------
// What no file moves
// -------------------------------------------------------------------------------------

fn pins(adapter: &dyn Adapter) -> Vec<(String, bool)> {
    adapter
        .pinned()
        .iter()
        .map(|pin| (pin.id.to_owned(), pin.on))
        .collect()
}

#[test]
fn charters_own_plugin_is_always_on_and_the_old_one_always_off() {
    let all = claude(&["charter@charter"], "", "");
    assert_eq!(row(&all, "purlis@inline").wanted, Some(true));
    assert!(row(&all, "purlis@inline").pinned.is_some());
    assert_eq!(row(&all, "charter@charter").wanted, Some(false));
    assert!(row(&all, "charter@charter").pinned.is_some());
}

#[test]
fn no_file_can_turn_charters_own_plugin_off_or_the_old_one_on() {
    let all = claude(
        &["charter@charter"],
        "[harness_plugins.claude]\n\"purlis@inline\" = false\n",
        "[harness_plugins.claude]\n\"charter@charter\" = true\n",
    );
    let own = row(&all, "purlis@inline");
    assert_eq!((own.wanted, own.source), (Some(true), Source::Default));
    assert_eq!(
        own.ignored,
        [Ignored {
            source: Source::Shared,
            why: "charter.toml sets harness_plugins.claude.\"purlis@inline\" to false, and \
                  purlis@inline is always on: it is purlis's own plugin, and it carries \
                  purlis's hooks and the Bash guard"
                .to_owned(),
        }]
    );
    let old = row(&all, "charter@charter");
    assert_eq!((old.wanted, old.source), (Some(false), Source::Default));
    assert_eq!(old.ignored.len(), 1, "{:?}", old.ignored);
    assert_eq!(old.ignored[0].source, Source::Local);

    let chosen = chosen(&CLAUDE_CODE, &all);
    assert_eq!(chosen.plugins.get("purlis@inline"), Some(&true));
    assert_eq!(chosen.plugins.get("charter@charter"), Some(&false));
}

#[test]
fn claude_codes_pins_are_charters_own_on_and_the_two_it_replaced_off() {
    assert_eq!(
        pins(&CLAUDE_CODE).into_iter().collect::<BTreeMap<_, _>>(),
        BTreeMap::from([
            ("purlis@inline".to_owned(), true),
            ("charter@charter".to_owned(), false),
            ("charter-app@inline".to_owned(), false),
            ("charter@inline".to_owned(), false),
            ("charter@charter-app".to_owned(), false),
        ])
    );
    assert_eq!(pins(&CLAUDE_CODE).len(), 5);
}

#[test]
fn every_id_the_plugin_had_before_purlis_is_pinned_off_with_the_id_that_replaced_it() {
    // #1266 (V93m), ADR 0056's FORMERLY: the old plugins are named `charter`, the new one
    // `purlis`, so `--plugin-dir` no longer wins the name over them; each is turned off by its
    // whole id, and a file that turns one on is told the id it is now.
    let all = claude(
        &["charter@charter-app"],
        "[harness_plugins.claude]\n\"charter@inline\" = true\n",
        "[harness_plugins.claude]\n\"charter@charter-app\" = true\n",
    );
    let inline = row(&all, "charter@inline");
    assert_eq!(
        (inline.wanted, inline.source),
        (Some(false), Source::Default)
    );
    assert_eq!(
        inline.ignored,
        [Ignored {
            source: Source::Shared,
            why: "charter.toml sets harness_plugins.claude.\"charter@inline\" to true, and \
                  charter@inline is always off: it is an id purlis's own plugin had before it \
                  was renamed, and purlis's own plugin is purlis@inline"
                .to_owned(),
        }]
    );
    let copy = row(&all, "charter@charter-app");
    assert_eq!((copy.wanted, copy.source), (Some(false), Source::Default));
    assert_eq!(copy.ignored.len(), 1, "{:?}", copy.ignored);
    assert!(
        copy.ignored[0].why.ends_with(
            "charter@charter-app is always off: it is the id the copy `purlis plugin install` \
             wrote had before the plugin was renamed; that copy is purlis@purlis-app now, and a \
             chat the app starts loads purlis@inline"
        ),
        "{}",
        copy.ignored[0].why
    );
    let chosen = chosen(&CLAUDE_CODE, &all);
    assert_eq!(chosen.plugins.get("purlis@inline"), Some(&true));
    for old in [
        "charter@inline",
        "charter-app@inline",
        "charter@charter-app",
    ] {
        assert_eq!(chosen.plugins.get(old), Some(&false), "{old}");
    }
}

#[test]
fn turning_the_python_plugin_off_never_turns_charters_own_off() {
    // #406: both plugins are *named* `charter`. Only the marketplace tells them apart, and
    // a file that turns `charter@charter` off, or a machine that has it installed, must leave
    // `purlis@inline` on.
    let all = claude(
        &["charter@charter"],
        "[harness_plugins.claude]\n\"charter@charter\" = false\n",
        "[harness_plugins.claude]\n\"charter@charter\" = false\n",
    );
    let own = row(&all, "purlis@inline");
    assert_eq!((own.wanted, own.source), (Some(true), Source::Default));
    assert!(own.ignored.is_empty(), "{:?}", own.ignored);
    let old = row(&all, "charter@charter");
    assert_eq!(old.wanted, Some(false));
    assert!(old.ignored.is_empty(), "{:?}", old.ignored);
    let chosen = chosen(&CLAUDE_CODE, &all);
    assert_eq!(chosen.plugins.get("purlis@inline"), Some(&true));
    assert_eq!(chosen.plugins.get("charter@charter"), Some(&false));
    assert_eq!(
        refusals(
            "[harness_plugins.claude]\n\"charter@charter\" = false\n",
            "charter.toml"
        ),
        Vec::<String>::new()
    );
}

#[test]
fn turning_charters_own_plugin_on_never_turns_the_python_one_on() {
    let all = claude(
        &["charter@charter"],
        "[harness_plugins.claude]\n\"purlis@inline\" = true\n",
        "",
    );
    let old = row(&all, "charter@charter");
    assert_eq!((old.wanted, old.source), (Some(false), Source::Default));
    assert!(old.ignored.is_empty(), "{:?}", old.ignored);
    assert_eq!(
        chosen(&CLAUDE_CODE, &all).plugins.get("charter@charter"),
        Some(&false)
    );
    assert_eq!(
        refusals(
            "[harness_plugins.claude]\n\"purlis@inline\" = true\n",
            "charter.toml"
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_file_that_still_names_the_plugin_by_its_old_id_is_told_its_new_one() {
    // #406: the plugin was `charter-app@inline` until it was renamed. A file that turns the old
    // id on turns nothing on, and says so; one that turns it off agrees with charter.
    let all = claude(
        &[],
        "[harness_plugins.claude]\n\"charter-app@inline\" = true\n",
        "",
    );
    let former = row(&all, "charter-app@inline");
    assert_eq!(
        (former.wanted, former.source),
        (Some(false), Source::Default)
    );
    assert_eq!(
        former.ignored,
        [Ignored {
            source: Source::Shared,
            why: "charter.toml sets harness_plugins.claude.\"charter-app@inline\" to true, and \
                  charter-app@inline is always off: it is an id purlis's own plugin had before \
                  it was renamed, and purlis's own plugin is purlis@inline"
                .to_owned(),
        }]
    );
    let own = row(&all, "purlis@inline");
    assert_eq!(own.wanted, Some(true));
    assert_eq!(
        refusals(
            "[harness_plugins.claude]\n\"charter-app@inline\" = true\n",
            "charter.toml"
        ),
        [
            "harness_plugins.claude.\"charter-app@inline\" in charter.toml cannot be true: \
          charter-app@inline is always off: it is an id purlis's own plugin had before it was \
          renamed, and purlis's own plugin is purlis@inline"
                .to_owned()
        ]
    );
    assert_eq!(
        refusals(
            "[harness_plugins.claude]\n\"charter-app@inline\" = false\n",
            "charter.toml"
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_chat_gets_exactly_the_installed_plugins_a_file_decided_and_the_pins() {
    let all = claude(
        &["figma@official", "serena@official", "humanizer@h"],
        "[harness_plugins.claude]\n\"figma@official\" = false\n\"serena@official\" = true\n",
        "[harness_plugins.claude]\n\"serena@official\" = false\n",
    );
    assert_eq!(
        chosen(&CLAUDE_CODE, &all).plugins,
        BTreeMap::from([
            ("charter-app@inline".to_owned(), false),
            ("charter@inline".to_owned(), false),
            ("charter@charter-app".to_owned(), false),
            ("purlis@inline".to_owned(), true),
            ("charter@charter".to_owned(), false),
            ("figma@official".to_owned(), false),
            ("serena@official".to_owned(), false),
        ])
    );
}

// -------------------------------------------------------------------------------------
// A harness whose adapter cannot apply says so
// -------------------------------------------------------------------------------------

#[test]
fn every_harness_charter_knows_has_an_adapter_and_says_whether_it_applies() {
    let words: Vec<&str> = adapters().map(|it| it.harness()).collect();
    assert_eq!(words, ["claude", "opencode", "codex"]);
    assert!(matches!(CLAUDE_CODE.support(), Support::PerChat));
    assert!(matches!(CODEX.support(), Support::OffOnly(why) if !why.is_empty()));
    assert!(matches!(OPENCODE.support(), Support::NotYet(why) if !why.is_empty()));
}

#[test]
fn a_harness_that_cannot_apply_says_plugins_are_not_supported_yet() {
    assert_eq!(
        not_supported(&OPENCODE).expect("opencode cannot apply"),
        format!(
            "plugins for opencode are not supported yet — {}",
            match OPENCODE.support() {
                Support::NotYet(why) => why,
                Support::PerChat | Support::OffOnly(_) => unreachable!(),
            }
        )
    );
    assert_eq!(not_supported(&CLAUDE_CODE), None);
    assert_eq!(
        not_supported(&CODEX),
        None,
        "Codex turns a plugin off per chat"
    );
}

#[test]
fn a_chat_on_a_harness_that_cannot_apply_is_handed_nothing_and_its_choices_are_said_ignored() {
    let installed = [plugin("opencode", "acme-plugin")];
    let all = resolve(
        &OPENCODE,
        &installed,
        &Choices::from_text(
            Some("[harness_plugins.opencode]\n\"acme-plugin\" = false\n"),
            None,
        ),
    );
    assert_eq!(chosen(&OPENCODE, &all).plugins, BTreeMap::new());
    let it = row(&all, "acme-plugin");
    assert_eq!(it.ignored.len(), 1, "{:?}", it.ignored);
    assert!(
        it.ignored[0]
            .why
            .contains("plugins for opencode are not supported yet"),
        "{}",
        it.ignored[0].why
    );
}

#[test]
fn a_codex_chat_is_handed_each_installed_plugin_the_project_turned_off_and_none_turned_on() {
    // Measured on codex-cli 0.163.0 (SK-6): `-c plugins."<id>".enabled=false` turns a plugin off
    // for one session, and `=true` over a config.toml that has it off does nothing.
    let installed = [
        plugin("codex", "figma@official"),
        plugin("codex", "serena@official"),
    ];
    let all = resolve(
        &CODEX,
        &installed,
        &Choices::from_text(
            Some(
                "[harness_plugins.codex]\n\"figma@official\" = false\n\"serena@official\" = true\n\
                 \"acme@corp\" = false\n",
            ),
            None,
        ),
    );
    assert_eq!(
        chosen(&CODEX, &all).plugins,
        BTreeMap::from([("figma@official".to_owned(), false)]),
        "one on is the harness's own to decide, and one not installed is handed nothing"
    );
    assert!(row(&all, "figma@official").ignored.is_empty());
    assert!(
        only_off(&CODEX).is_some_and(|said| said.starts_with(
            "Codex is handed a plugin only to \
                                                             turn it off — "
        )),
        "{:?}",
        only_off(&CODEX)
    );
    assert_eq!(only_off(&CLAUDE_CODE), None);
}

// -------------------------------------------------------------------------------------
// The adapters' listings, from fixtures
// -------------------------------------------------------------------------------------

fn env_of(pairs: &[(&str, &std::path::Path)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, path)| ((*name).to_owned(), path.display().to_string()))
        .collect()
}

#[test]
fn claude_code_lists_every_plugin_its_install_record_holds_once() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("plugins")).unwrap();
    std::fs::write(
        home.path().join("plugins/installed_plugins.json"),
        r#"{"version": 2, "plugins": {
            "figma@claude-plugins-official": [{"scope": "user", "installPath": "/x", "version": "2.2.1"}],
            "superpowers@claude-plugins-official": [
                {"scope": "project", "projectPath": "/a", "installPath": "/y", "version": "5.0.0"},
                {"scope": "project", "projectPath": "/c", "installPath": "/y", "version": "5.0.0"},
                {"scope": "local", "projectPath": "/b", "installPath": "/y", "version": "5.0.0"}
            ]
        }}"#,
    )
    .unwrap();
    let env = env_of(&[("CLAUDE_CONFIG_DIR", home.path())]);

    let got = CLAUDE_CODE.installed(&Env::of(&env)).expect("it reads");

    assert_eq!(
        got,
        [
            Plugin {
                id: "figma@claude-plugins-official".to_owned(),
                harness: "claude",
                name: "figma".to_owned(),
                source: "claude-plugins-official, user".to_owned(),
            },
            Plugin {
                id: "superpowers@claude-plugins-official".to_owned(),
                harness: "claude",
                name: "superpowers".to_owned(),
                source: "claude-plugins-official, project, local".to_owned(),
            },
        ]
    );
}

#[test]
fn claude_code_with_nothing_installed_lists_nothing() {
    let home = tempfile::tempdir().unwrap();
    let env = env_of(&[("CLAUDE_CONFIG_DIR", home.path())]);
    assert_eq!(CLAUDE_CODE.installed(&Env::of(&env)), Ok(Vec::new()));
}

#[test]
fn claude_code_says_so_when_its_install_record_is_not_json() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("plugins")).unwrap();
    std::fs::write(home.path().join("plugins/installed_plugins.json"), "{").unwrap();
    let env = env_of(&[("CLAUDE_CONFIG_DIR", home.path())]);

    let why = CLAUDE_CODE
        .installed(&Env::of(&env))
        .expect_err("it refuses");
    assert!(why.contains("installed_plugins.json"), "{why}");
}

#[test]
fn claude_code_reads_under_home_when_the_chat_names_no_config_dir() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".claude/plugins")).unwrap();
    std::fs::write(
        home.path().join(".claude/plugins/installed_plugins.json"),
        r#"{"version": 2, "plugins": {"a@m": [{"scope": "user"}]}}"#,
    )
    .unwrap();
    let env = Env {
        chat: &[],
        home: Some(home.path().to_path_buf()),
        process: false,
    };

    let got = CLAUDE_CODE.installed(&env).expect("it reads");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].id, "a@m");
}

#[test]
fn codex_lists_the_plugins_its_config_names() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join("config.toml"),
        "model = \"o3\"\n\n[plugins.\"charter@charter\"]\nenabled = true\n\n\
         [plugins.\"deep-research@openai-curated\"]\nenabled = false\n",
    )
    .unwrap();
    let env = env_of(&[("CODEX_HOME", home.path())]);

    let got = CODEX.installed(&Env::of(&env)).expect("it reads");

    assert_eq!(
        got,
        [
            Plugin {
                id: "charter@charter".to_owned(),
                harness: "codex",
                name: "charter".to_owned(),
                source: "charter, on in config.toml".to_owned(),
            },
            Plugin {
                id: "deep-research@openai-curated".to_owned(),
                harness: "codex",
                name: "deep-research".to_owned(),
                source: "openai-curated, off in config.toml".to_owned(),
            },
        ]
    );
}

#[test]
fn opencode_lists_its_config_plugins_and_its_plugin_files() {
    let config = tempfile::tempdir().unwrap();
    let dir = config.path().join("opencode");
    std::fs::create_dir_all(dir.join("plugin")).unwrap();
    std::fs::create_dir_all(dir.join("plugins")).unwrap();
    std::fs::write(
        dir.join("opencode.json"),
        r#"{"plugin": ["opencode-wakatime", "@org/custom@1.2.0"]}"#,
    )
    .unwrap();
    std::fs::write(dir.join("plugin/charter.ts"), "export default {}").unwrap();
    std::fs::write(dir.join("plugins/notes.js"), "export default {}").unwrap();
    std::fs::write(dir.join("plugins/README.md"), "not a plugin").unwrap();
    let env = env_of(&[("XDG_CONFIG_HOME", config.path())]);
    // An empty home: the machine running the test has its own `~/.opencode`.
    let home = tempfile::tempdir().unwrap();
    let env = Env {
        chat: &env,
        home: Some(home.path().to_path_buf()),
        process: false,
    };

    let got = OPENCODE.installed(&env).expect("it reads");
    let ids: Vec<(&str, &str)> = got
        .iter()
        .map(|it| (it.id.as_str(), it.source.as_str()))
        .collect();

    assert_eq!(
        ids,
        [
            ("@org/custom@1.2.0", "npm, in opencode.json"),
            ("opencode-wakatime", "npm, in opencode.json"),
            ("plugin/charter.ts", "a file in the plugin directory"),
            ("plugins/notes.js", "a file in the plugin directory"),
        ]
    );
}

#[test]
fn opencode_lists_a_projects_own_plugin_files_by_their_place_in_the_project() {
    let project = tempfile::tempdir().unwrap();
    let dir = project.path().join(".opencode");
    std::fs::create_dir_all(dir.join("plugin")).unwrap();
    std::fs::create_dir_all(dir.join("plugins")).unwrap();
    std::fs::write(dir.join("plugin/guard.ts"), "export default {}").unwrap();
    std::fs::write(dir.join("plugins/notes.mjs"), "export default {}").unwrap();
    std::fs::write(dir.join("plugins/README.md"), "not a plugin").unwrap();

    let got: Vec<(String, String)> = OPENCODE
        .in_project(project.path())
        .into_iter()
        .map(|it| (it.id, it.source))
        .collect();

    let source = "a file in this project's .opencode plugin directory".to_owned();
    assert_eq!(
        got,
        [
            (".opencode/plugin/guard.ts".to_owned(), source.clone()),
            (".opencode/plugins/notes.mjs".to_owned(), source),
        ]
    );
}

/// #1590: opencode also loads the npm plugins a project's own config names, from each of the
/// four files it reads there (measured, opencode 1.18.33). Each is listed by the file that
/// names it, so it never collides with the global entry of the same package.
#[test]
fn opencode_lists_the_npm_plugins_a_project_s_own_configs_name() {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join(".opencode")).unwrap();
    std::fs::write(
        project.path().join("opencode.json"),
        r#"{"plugin": ["opencode-wakatime", "@org/custom@1.2.0"]}"#,
    )
    .unwrap();
    std::fs::write(
        project.path().join(".opencode/opencode.json"),
        r#"{"plugin": ["opencode-wakatime"], "model": "x"}"#,
    )
    .unwrap();

    let got: Vec<(String, String, String)> = OPENCODE
        .in_project(project.path())
        .into_iter()
        .map(|it| (it.id, it.name, it.source))
        .collect();

    let at =
        |id: &str, name: &str, source: &str| (id.to_owned(), name.to_owned(), source.to_owned());
    assert_eq!(
        got,
        [
            at(
                "opencode.json:@org/custom@1.2.0",
                "@org/custom@1.2.0",
                "npm, in this project's opencode.json"
            ),
            at(
                "opencode.json:opencode-wakatime",
                "opencode-wakatime",
                "npm, in this project's opencode.json"
            ),
            at(
                ".opencode/opencode.json:opencode-wakatime",
                "opencode-wakatime",
                "npm, in this project's .opencode/opencode.json"
            ),
        ]
    );
}

/// #1590: a project config purlis cannot read (JSONC, or not JSON at all) may still name
/// plugins opencode loads, so it is named itself rather than read as naming none.
#[test]
fn a_project_s_opencode_config_purlis_cannot_read_is_named_not_skipped() {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join(".opencode")).unwrap();
    std::fs::write(
        project.path().join("opencode.json"),
        "{\n  // a comment\n  \"plugin\": [\"x\"]\n}\n",
    )
    .unwrap();
    std::fs::write(
        project.path().join(".opencode/opencode.jsonc"),
        "{ \"plugin\": [\"y\"], }",
    )
    .unwrap();

    let got: Vec<(String, String)> = OPENCODE
        .in_project(project.path())
        .into_iter()
        .map(|it| (it.id, it.source))
        .collect();

    let unread = "a config purlis cannot read: the plugins it names are not listed".to_owned();
    assert_eq!(
        got,
        [
            ("opencode.json".to_owned(), unread.clone()),
            (".opencode/opencode.jsonc".to_owned(), unread),
        ]
    );
}

/// #1590: opencode follows a symbolic link in a plugin directory (its scan is `symlink: true`,
/// measured on 1.18.33), so a linked script is listed like a file, in the project and in the
/// global directory alike. A link to a directory is not a script.
#[cfg(unix)]
#[test]
fn a_linked_script_in_a_plugin_directory_is_listed_like_a_file() {
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(elsewhere.path().join("real.ts"), "export default {}").unwrap();
    std::fs::create_dir_all(elsewhere.path().join("dir.ts")).unwrap();
    let project = tempfile::tempdir().unwrap();
    let plugin = project.path().join(".opencode/plugin");
    std::fs::create_dir_all(&plugin).unwrap();
    std::os::unix::fs::symlink(elsewhere.path().join("real.ts"), plugin.join("linked.ts")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path().join("gone.ts"), plugin.join("dangling.ts"))
        .unwrap();
    std::os::unix::fs::symlink(elsewhere.path().join("dir.ts"), plugin.join("folder.ts")).unwrap();

    let got: Vec<String> = OPENCODE
        .in_project(project.path())
        .into_iter()
        .map(|it| it.id)
        .collect();

    // A link whose target is not there yet is listed too: what it points at may be there by the
    // time opencode starts.
    assert_eq!(
        got,
        [".opencode/plugin/dangling.ts", ".opencode/plugin/linked.ts"]
    );
}

/// A project with the workspace `smart`, whose `.opencode` folders and configs are `files`
/// (each a path under the project, and its text), and a clone `app` in it.
fn project_with_a_workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    let root = project.path();
    std::fs::create_dir_all(root.join("workspaces/smart/app/.git")).unwrap();
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    project
}

fn ids_and_sources(plugins: Vec<Plugin>) -> Vec<(String, String)> {
    plugins.into_iter().map(|it| (it.id, it.source)).collect()
}

fn pair(id: &str, source: &str) -> (String, String) {
    (id.to_owned(), source.to_owned())
}

/// #1590: a chat in a workspace has opencode walk from the workspace's folder up to the git top
/// (read from opencode 1.18.33's source), and a chat in a clone reads the clone's own. Each is
/// listed by its place in the project and says whose it is; a folder that is not a clone, and
/// the project's own, are not the workspace's.
#[test]
fn a_workspace_s_own_opencode_and_its_clones_are_listed_by_their_place() {
    let project = project_with_a_workspace(&[
        (".opencode/plugin/root.ts", ""),
        (
            "workspaces/smart/opencode.json",
            r#"{"plugin": ["ws-pkg"]}"#,
        ),
        ("workspaces/smart/.opencode/plugin/guard.ts", ""),
        ("workspaces/.opencode/plugins/between.js", ""),
        (
            "workspaces/smart/app/opencode.json",
            r#"{"plugin": ["clone-pkg"]}"#,
        ),
        ("workspaces/smart/app/.opencode/plugin/clone.ts", ""),
        ("workspaces/smart/notes/.opencode/plugin/no.ts", ""),
        ("workspaces/other/.opencode/plugin/theirs.ts", ""),
    ]);
    let root = project.path().canonicalize().unwrap();

    assert_eq!(
        ids_and_sources(OPENCODE.in_workspace(&root, "smart")),
        [
            pair(
                "workspaces/smart/opencode.json:ws-pkg",
                "npm, in workspace smart's opencode.json"
            ),
            pair(
                "workspaces/smart/.opencode/plugin/guard.ts",
                "a file in workspace smart's .opencode plugin directory"
            ),
            pair(
                "workspaces/.opencode/plugins/between.js",
                "a file in the workspaces folder's .opencode plugin directory"
            ),
            pair(
                "workspaces/smart/app/opencode.json:clone-pkg",
                "npm, in clone app's opencode.json"
            ),
            pair(
                "workspaces/smart/app/.opencode/plugin/clone.ts",
                "a file in clone app's .opencode plugin directory"
            ),
        ]
    );
    assert!(OPENCODE.in_workspace(&root, "nowhere").is_empty());
    assert!(OPENCODE.in_workspace(&root, "../..").is_empty());
    assert!(CLAUDE_CODE.in_workspace(&root, "smart").is_empty());
    assert!(CODEX.in_workspace(&root, "smart").is_empty());
}

/// #1590: opencode stops at the git top, so a workspace that is a repository of its own is
/// where a chat in it stops looking.
#[test]
fn a_workspace_that_is_its_own_git_top_is_where_the_walk_stops() {
    let project = project_with_a_workspace(&[
        ("workspaces/smart/.git/HEAD", ""),
        ("workspaces/smart/.opencode/plugin/guard.ts", ""),
        ("workspaces/.opencode/plugin/between.ts", ""),
    ]);
    let root = project.path().canonicalize().unwrap();
    assert_eq!(
        ids_and_sources(OPENCODE.in_workspace(&root, "smart")),
        [pair(
            "workspaces/smart/.opencode/plugin/guard.ts",
            "a file in workspace smart's .opencode plugin directory"
        )]
    );
}

/// #1828: opencode stops at the top git resolves, and git takes a `.git` only where it is a
/// folder holding `HEAD` or a file that starts `gitdir:`. Any other `.git` (an empty folder, a
/// stray file) is no repository to git, so opencode walks on past it, and so does the list.
#[test]
fn a_git_entry_that_is_no_repository_does_not_stop_the_walk() {
    let between = pair(
        "workspaces/.opencode/plugin/between.ts",
        "a file in the workspaces folder's .opencode plugin directory",
    );
    let guard = pair(
        "workspaces/smart/.opencode/plugin/guard.ts",
        "a file in workspace smart's .opencode plugin directory",
    );
    for (planted, stops) in [
        (None, false),
        (Some("not a pointer"), false),
        (Some("gitdir: ../../.git/worktrees/smart\n"), true),
    ] {
        let project = project_with_a_workspace(&[
            ("workspaces/smart/.opencode/plugin/guard.ts", ""),
            ("workspaces/.opencode/plugin/between.ts", ""),
        ]);
        let root = project.path().canonicalize().unwrap();
        let git = root.join("workspaces/smart/.git");
        match planted {
            None => std::fs::create_dir_all(&git).unwrap(),
            Some(text) => std::fs::write(&git, text).unwrap(),
        }
        let want = if stops {
            vec![guard.clone()]
        } else {
            vec![guard.clone(), between.clone()]
        };
        assert_eq!(
            ids_and_sources(OPENCODE.in_workspace(&root, "smart")),
            want,
            "a .git of {planted:?}"
        );
    }
}

/// A folder made under `under`, with `files` (each a path under it, empty).
fn folder_with(under: &std::path::Path, files: &[&str]) {
    for path in files {
        let path = under.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }
}

/// #1590: outside a repository opencode walks from the chat's folder up to `/`, and in a
/// project nested in a larger repository, up to that repository's top. So the list goes on
/// above a project that is not a git top of its own, each folder above it named by its path,
/// and stops at the first git top.
#[test]
fn a_project_that_is_no_git_top_lists_the_folders_above_it_up_to_the_git_top() {
    let outer = tempfile::tempdir().unwrap();
    let top = outer.path().canonicalize().unwrap().join("repo");
    folder_with(
        &top,
        &[
            ".git/HEAD",
            ".opencode/plugin/top.ts",
            "mid/.opencode/plugins/mid.js",
            "mid/project/.opencode/plugin/own.ts",
        ],
    );
    std::fs::write(
        outer.path().join("repo/mid/opencode.json"),
        r#"{"plugin": ["mid-pkg"]}"#,
    )
    .unwrap();
    folder_with(outer.path(), &[".opencode/plugin/above-the-top.ts"]);
    let project = top.join("mid/project");
    let (mid, top_name) = (
        top.join("mid").display().to_string(),
        top.display().to_string(),
    );

    assert_eq!(
        ids_and_sources(OPENCODE.in_project(&project)),
        [
            pair(
                ".opencode/plugin/own.ts",
                "a file in this project's .opencode plugin directory"
            ),
            pair(
                &format!("{mid}/opencode.json:mid-pkg"),
                &format!("npm, in the {mid} folder's opencode.json")
            ),
            pair(
                &format!("{mid}/.opencode/plugins/mid.js"),
                &format!("a file in the {mid} folder's .opencode plugin directory")
            ),
            pair(
                &format!("{top_name}/.opencode/plugin/top.ts"),
                &format!("a file in the {top_name} folder's .opencode plugin directory")
            ),
        ]
    );

    // A project that is a git top of its own is where the walk stops.
    folder_with(&project, &[".git/HEAD"]);
    assert_eq!(
        ids_and_sources(OPENCODE.in_project(&project)),
        [pair(
            ".opencode/plugin/own.ts",
            "a file in this project's .opencode plugin directory"
        )]
    );
}

/// #1590: beside its global directory opencode reads `~/.opencode/`: its two configs and its
/// `plugin/` and `plugins/` scripts. It never reads a config at the top of the home folder, so
/// `~/opencode.json` is not listed.
#[test]
fn opencode_lists_the_home_folder_s_own_opencode() {
    let config = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    folder_with(
        home.path(),
        &[".opencode/plugins/home.ts", ".opencode/plugin/no.md"],
    );
    std::fs::write(
        home.path().join(".opencode/opencode.json"),
        r#"{"plugin": ["home-pkg"]}"#,
    )
    .unwrap();
    std::fs::write(
        home.path().join(".opencode/opencode.jsonc"),
        "{ // a comment\n}",
    )
    .unwrap();
    std::fs::write(
        home.path().join("opencode.json"),
        r#"{"plugin": ["never"]}"#,
    )
    .unwrap();
    let chat = env_of(&[("XDG_CONFIG_HOME", config.path())]);
    let env = Env {
        chat: &chat,
        home: Some(home.path().to_path_buf()),
        process: false,
    };

    assert_eq!(
        ids_and_sources(OPENCODE.installed(&env).expect("it reads")),
        [
            pair(
                "~/.opencode/opencode.json:home-pkg",
                "npm, in the home folder's .opencode/opencode.json"
            ),
            pair(
                "~/.opencode/opencode.jsonc",
                "a config purlis cannot read: the plugins it names are not listed"
            ),
            pair(
                "~/.opencode/plugins/home.ts",
                "a file in the home folder's .opencode plugin directory"
            ),
        ]
    );
}

/// #1828: a `.opencode` is read through the one folder handle purlis opened, so a link that
/// takes its place after it was looked at is never read through.
#[cfg(unix)]
#[test]
fn a_held_opencode_folder_is_read_as_opened_when_a_link_takes_its_place() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    folder_with(&root, &[".opencode/plugin/own.ts"]);
    let elsewhere = tempfile::tempdir().unwrap();
    folder_with(elsewhere.path(), &["plugin/away.ts", "plugins/far.ts"]);
    std::fs::write(
        elsewhere.path().join("opencode.json"),
        r#"{"plugin": ["away"]}"#,
    )
    .unwrap();

    let level = Folder::open(&root, Follow::Link).expect("the project opens");
    let Opened::Folder(dot) = level.child(".opencode", Follow::NoLink) else {
        panic!(".opencode is a folder")
    };
    std::fs::rename(root.join(".opencode"), root.join("moved")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), root.join(".opencode")).unwrap();

    let listed = opencode_dot(
        OPENCODE.harness(),
        &dot,
        "",
        "this project's",
        true,
        HardLinks::Refused,
    );
    assert_eq!(
        ids_and_sources(listed),
        [pair(
            ".opencode/plugin/own.ts",
            "a file in this project's .opencode plugin directory"
        )]
    );
}

/// #1590: a chat in a linked worktree (`workspaces/<ws>/.worktrees/<repo>/<piece>`) has opencode
/// read that worktree, its own git top. Each is listed by its place; a worktree folder that is
/// a link is named and not followed. Needs git: first run on CI.
#[cfg(unix)]
#[test]
fn a_workspace_s_linked_worktrees_are_listed_by_their_place() {
    let git = |at: &std::path::Path, argv: &[&str]| {
        let run = crate::testgit::run(at, argv);
        assert!(run.ok(), "git {argv:?} failed: {}", run.err);
    };
    let project = tempfile::tempdir().unwrap();
    let root = project.path().canonicalize().unwrap();
    let clone = root.join("workspaces/smart/app");
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q", "-b", "main"]);
    git(&clone, &["config", "user.email", "t@example.com"]);
    git(&clone, &["config", "user.name", "T"]);
    std::fs::write(clone.join("README.md"), "hi\n").unwrap();
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-qm", "first"]);
    let pieces = crate::worktree::root_of(&root, "smart").join("app");
    let piece = pieces.join("fix-login");
    git(
        &clone,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "fix/login",
            piece.to_str().unwrap(),
        ],
    );
    folder_with(&piece, &[".opencode/plugin/piece.ts"]);
    let elsewhere = tempfile::tempdir().unwrap();
    folder_with(elsewhere.path(), &[".opencode/plugin/away.ts"]);
    std::os::unix::fs::symlink(elsewhere.path(), pieces.join("linked")).unwrap();

    assert_eq!(
        ids_and_sources(OPENCODE.in_workspace(&root, "smart")),
        [
            pair(
                "workspaces/smart/.worktrees/app/fix-login/.opencode/plugin/piece.ts",
                "a file in the app/fix-login branch folder's .opencode plugin directory"
            ),
            pair(
                "workspaces/smart/.worktrees/app/linked",
                "a link purlis does not follow: the plugins opencode loads through it are \
                 not listed"
            ),
        ]
    );
}

/// #1590: a chat can write a workspace's, a clone's and the project's `.opencode`, so purlis
/// never reads through a link there. A `.opencode`, `plugin` or `plugins` that is a link is
/// named, since opencode follows it, and what is behind it is not listed. A workspace that is
/// itself a link lists nothing.
#[cfg(unix)]
#[test]
fn a_linked_opencode_folder_is_named_and_never_followed() {
    let elsewhere = tempfile::tempdir().unwrap();
    let away = elsewhere.path().canonicalize().unwrap();
    std::fs::create_dir_all(away.join("plugin")).unwrap();
    std::fs::write(away.join("plugin/secret.ts"), "").unwrap();
    std::fs::write(away.join("opencode.json"), r#"{"plugin": ["away"]}"#).unwrap();
    let project = project_with_a_workspace(&[("workspaces/smart/app/.opencode/plugins/ok.ts", "")]);
    let root = project.path().canonicalize().unwrap();
    std::os::unix::fs::symlink(&away, root.join(".opencode")).unwrap();
    std::os::unix::fs::symlink(&away, root.join("workspaces/smart/.opencode")).unwrap();
    std::os::unix::fs::symlink(
        away.join("plugin"),
        root.join("workspaces/smart/app/.opencode/plugin"),
    )
    .unwrap();

    let unfollowed = "a link purlis does not follow: the plugins opencode loads through it are \
                      not listed";
    assert_eq!(
        ids_and_sources(OPENCODE.in_project(&root)),
        [pair(".opencode", unfollowed)]
    );
    assert_eq!(
        ids_and_sources(OPENCODE.in_workspace(&root, "smart")),
        [
            pair("workspaces/smart/.opencode", unfollowed),
            pair("workspaces/smart/app/.opencode/plugin", unfollowed),
            pair(
                "workspaces/smart/app/.opencode/plugins/ok.ts",
                "a file in clone app's .opencode plugin directory"
            ),
        ]
    );

    std::fs::create_dir_all(away.join("ws/.opencode/plugin")).unwrap();
    std::fs::write(away.join("ws/.opencode/plugin/far.ts"), "").unwrap();
    std::os::unix::fs::symlink(away.join("ws"), root.join("workspaces/linked")).unwrap();
    assert!(OPENCODE.in_workspace(&root, "linked").is_empty());
}

/// #1590: the Settings tab at the Workspace level lists what the workspace holds of its own
/// beside the project's; at the Project level, only the project's.
#[test]
fn the_survey_in_a_workspace_lists_its_own_opencode_beside_the_project_s() {
    let project = project_with_a_workspace(&[
        (".opencode/plugin/root.ts", ""),
        ("workspaces/smart/.opencode/plugin/guard.ts", ""),
    ]);
    let root = project.path().canonicalize().unwrap();
    let env = Env {
        chat: &[],
        home: None,
        process: false,
    };
    let listed = |choices: &Choices| -> Vec<String> {
        survey(choices, &env)
            .into_iter()
            .find(|it| it.adapter.harness() == "opencode")
            .expect("opencode is a group")
            .plugins
            .into_iter()
            .map(|it| it.id)
            .collect()
    };
    assert_eq!(
        listed(&Choices::read_in(&root, Some("smart"))),
        [
            ".opencode/plugin/root.ts",
            "workspaces/smart/.opencode/plugin/guard.ts"
        ]
    );
    assert_eq!(listed(&Choices::read(&root)), [".opencode/plugin/root.ts"]);
}

#[test]
fn a_harness_that_loads_nothing_from_a_project_lists_nothing_of_it() {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join(".opencode/plugin")).unwrap();
    std::fs::write(project.path().join(".opencode/plugin/a.ts"), "").unwrap();
    assert!(CLAUDE_CODE.in_project(project.path()).is_empty());
    assert!(CODEX.in_project(project.path()).is_empty());
}

/// ADR 0058's visibility: the settings tab's list names the project's own scripts beside the
/// global ones (#1371).
#[test]
fn the_survey_of_a_project_lists_its_own_opencode_scripts_beside_the_global_ones() {
    let config = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(config.path().join("opencode/plugin")).unwrap();
    std::fs::write(config.path().join("opencode/plugin/global.js"), "").unwrap();
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join(".opencode/plugin")).unwrap();
    std::fs::write(project.path().join(".opencode/plugin/local.ts"), "").unwrap();
    let chat = env_of(&[("XDG_CONFIG_HOME", config.path())]);
    let env = Env {
        chat: &chat,
        home: None,
        process: false,
    };

    let groups = survey(&Choices::read(project.path()), &env);
    let opencode = groups
        .iter()
        .find(|it| it.adapter.harness() == "opencode")
        .expect("opencode is a group");
    let listed: Vec<(&str, bool)> = opencode
        .plugins
        .iter()
        .map(|it| (it.id.as_str(), it.installed))
        .collect();
    assert_eq!(
        listed,
        [
            (".opencode/plugin/local.ts", true),
            ("plugin/global.js", true)
        ]
    );
    // Choices made from text name no project, and list none of one.
    let groups = survey(&Choices::default(), &env);
    let opencode = groups
        .iter()
        .find(|it| it.adapter.harness() == "opencode")
        .unwrap();
    assert_eq!(opencode.plugins.len(), 1);
}

// -------------------------------------------------------------------------------------
// What a file may not say
// -------------------------------------------------------------------------------------

#[test]
fn a_well_formed_table_is_refused_nothing() {
    assert_eq!(
        refusals(
            "[harness_plugins.claude]\n\"figma@official\" = true\n\n[harness_plugins.codex]\n\"a@b\" = false\n",
            "charter.toml"
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_file_is_refused_a_harness_charter_does_not_know_a_value_that_is_not_bool_and_a_pin() {
    let got = refusals(
        "[harness_plugins.vim]\n\"a@b\" = true\n\n[harness_plugins.claude]\n\"figma@official\" = \"yes\"\n\"purlis@inline\" = false\n",
        "charter.local.toml",
    );
    assert_eq!(
        got,
        [
            "[harness_plugins.vim] in charter.local.toml is not a harness purlis knows — one of: \
             claude, opencode, codex"
                .to_owned(),
            "harness_plugins.claude.\"figma@official\" in charter.local.toml is not true or false"
                .to_owned(),
            "harness_plugins.claude.\"purlis@inline\" in charter.local.toml cannot be false: \
             purlis@inline is always on: it is purlis's own plugin, and it carries \
             purlis's hooks and the Bash guard"
                .to_owned(),
        ]
    );
}

#[test]
fn a_harness_plugins_that_is_not_a_table_is_refused() {
    assert_eq!(
        refusals("harness_plugins = 3\n", "charter.toml"),
        [
            "harness_plugins in charter.toml is not a table — each harness is \
          [harness_plugins.<harness>], holding \"<plugin id>\" = true or false"
                .to_owned()
        ]
    );
}

#[test]
fn a_harness_skills_table_is_refused_what_its_harness_cannot_switch_alone() {
    let got = refusals(
        "[harness_skills.claude]\nlangfuse = false\n\"figma:frame\" = false\n\n\
         [harness_skills.codex]\n\"figma:frame\" = false\nreview = \"off\"\n\n\
         [harness_skills.vim]\nx = false\n",
        "charter.toml",
    );
    assert_eq!(
        got,
        [
            "harness_skills.claude.\"figma:frame\" in charter.toml cannot be set: Claude Code \
             turns a plugin's skill off only with the whole plugin, figma"
                .to_owned(),
            "harness_skills.codex.review in charter.toml is not true or false".to_owned(),
            "[harness_skills.vim] in charter.toml is not a harness purlis knows — one of: \
             claude, opencode, codex"
                .to_owned(),
        ]
    );
}

#[test]
fn a_skill_the_harness_knows_is_on_or_off_by_the_layer_with_the_last_word() {
    let choices = Choices::from_text(
        Some("[harness_skills.opencode]\nreview = false\nlint = false\n"),
        Some("[harness_skills.opencode]\nlint = true\n"),
    )
    .in_workspace(
        "alpha",
        Some(r#"{"settings": {"harness_skills": {"opencode": {"deploy": false}}}}"#),
    );
    assert_eq!(
        choices.skill("opencode", "review"),
        (Some(false), Source::Shared)
    );
    assert_eq!(
        choices.skill("opencode", "lint"),
        (Some(true), Source::Local)
    );
    assert_eq!(
        choices.skill("opencode", "deploy"),
        (Some(false), Source::Workspace)
    );
    assert_eq!(choices.skill("codex", "review"), (None, Source::Default));
    assert_eq!(
        choices.skills_off("opencode"),
        BTreeSet::from(["deploy".to_owned(), "review".to_owned()])
    );
}

// -------------------------------------------------------------------------------------
// At a chat's start
// -------------------------------------------------------------------------------------

/// A plane whose `charter.toml` is `shared`, and a Claude Code config dir whose install record
/// is `record` — read only through the chat's own `CLAUDE_CONFIG_DIR`.
fn start_with(shared: &str, record: &str) -> BTreeMap<String, bool> {
    start_in(shared, None, record)
}

/// [`start_with`], for a chat in the workspace `alpha` whose `workspace.json` is `manifest`,
/// when there is one.
fn start_in(shared: &str, manifest: Option<&str>, record: &str) -> BTreeMap<String, bool> {
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(plane.path().join("charter.toml"), shared).unwrap();
    if let Some(manifest) = manifest {
        let alpha = plane.path().join("workspaces/alpha");
        std::fs::create_dir_all(&alpha).unwrap();
        std::fs::write(alpha.join("workspace.json"), manifest).unwrap();
    }
    let config = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(config.path().join("plugins")).unwrap();
    std::fs::write(config.path().join("plugins/installed_plugins.json"), record).unwrap();
    let chat = env_of(&[("CLAUDE_CONFIG_DIR", config.path())]);
    let env = Env {
        chat: &chat,
        home: None,
        process: false,
    };
    for_start("claude", plane.path(), manifest.map(|_| "alpha"), &env).plugins
}

#[test]
fn a_workspace_that_chooses_in_a_project_that_does_not_is_handed_its_choice_at_the_start() {
    // The record is read because the WORKSPACE names a plugin, though neither project file does.
    let got = start_in(
        "",
        Some(&ws(&[("figma@official", false)])),
        r#"{"version": 2, "plugins": {"figma@official": [{"scope": "user"}]}}"#,
    );
    let mut want = pins(&CLAUDE_CODE).into_iter().collect::<BTreeMap<_, _>>();
    want.insert("figma@official".to_owned(), false);
    assert_eq!(got, want);
}

#[test]
fn a_project_that_chooses_nothing_starts_its_chats_with_the_pins_and_reads_no_record() {
    // The record here is not JSON: had it been read, the listing would have failed. It was not,
    // and the answer is the pins, as every chat before this carried.
    assert_eq!(
        start_with("", "{"),
        pins(&CLAUDE_CODE).into_iter().collect::<BTreeMap<_, _>>()
    );
}

#[test]
fn a_project_that_chooses_is_handed_what_the_chats_own_record_has_installed() {
    let got = start_with(
        "[harness_plugins.claude]\n\"figma@official\" = false\n\"acme@corp\" = true\n",
        r#"{"version": 2, "plugins": {"figma@official": [{"scope": "user"}]}}"#,
    );
    let mut want = pins(&CLAUDE_CODE).into_iter().collect::<BTreeMap<_, _>>();
    want.insert("figma@official".to_owned(), false);
    assert_eq!(got, want);
}

#[test]
fn a_chat_on_a_harness_that_cannot_apply_is_handed_nothing_at_its_start() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(
        plane.path().join("charter.toml"),
        "[harness_plugins.opencode]\n\"acme-plugin\" = false\n",
    )
    .unwrap();
    let env = Env {
        chat: &[],
        home: None,
        process: false,
    };
    assert_eq!(
        for_start_as(true, "opencode", plane.path(), None, &env),
        Chosen::new()
    );
}

#[test]
fn a_chat_is_handed_the_harness_skills_the_project_turned_off_only_while_the_studio_is_on() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(
        plane.path().join("charter.toml"),
        "[harness_skills.codex]\nlangfuse = false\nreview = false\n",
    )
    .unwrap();
    std::fs::write(
        plane.path().join("charter.local.toml"),
        "[harness_skills.codex]\nreview = true\n",
    )
    .unwrap();
    let env = Env {
        chat: &[],
        home: None,
        process: false,
    };
    assert_eq!(
        for_start_as(true, "codex", plane.path(), None, &env).skills_off,
        BTreeSet::from(["langfuse".to_owned()]),
        "Local turned review back on, over Shared"
    );
    assert_eq!(
        for_start_as(false, "codex", plane.path(), None, &env),
        Chosen::new(),
        "behind the Skill studio's flag"
    );
}

#[cfg(unix)]
#[test]
fn a_project_that_chooses_nothing_never_opens_the_harnesss_record() {
    // The test above cannot tell a read from none: a record that fails to parse is the pins
    // too. A fifo can — opening it for reading waits for a writer, so the writer here learns
    // whether anything opened it.
    use std::io::Write;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let plane = tempfile::tempdir().unwrap();
    std::fs::write(plane.path().join("charter.toml"), "").unwrap();
    let config = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(config.path().join("plugins")).unwrap();
    let record = config.path().join("plugins/installed_plugins.json");
    let made_it = crate::forklock::status(std::process::Command::new("mkfifo").arg(&record))
        .expect("mkfifo runs");
    assert!(made_it.success(), "the test needs a fifo to plant");

    let opened = Arc::new(AtomicBool::new(false));
    let writer = {
        let opened = Arc::clone(&opened);
        let record = record.clone();
        std::thread::spawn(move || {
            let mut pipe = std::fs::OpenOptions::new()
                .write(true)
                .open(&record)
                .expect("the fifo's write end");
            opened.store(true, Ordering::SeqCst);
            let _ = pipe.write_all(b"{}");
        })
    };
    let chat = env_of(&[("CLAUDE_CONFIG_DIR", config.path())]);
    let env = Env {
        chat: &chat,
        home: None,
        process: false,
    };

    let got = for_start("claude", plane.path(), None, &env);
    let read_it = opened.load(Ordering::SeqCst);
    if !read_it {
        // Nothing opened it, so the writer is still waiting for a reader: be one.
        drop(std::fs::File::open(&record).expect("the fifo's read end"));
    }
    writer.join().expect("the writer");

    assert!(
        !read_it,
        "the record was opened for a project that chooses nothing"
    );
    assert_eq!(
        got.plugins,
        pins(&CLAUDE_CODE).into_iter().collect::<BTreeMap<_, _>>()
    );
}

#[test]
fn claude_code_says_so_when_its_install_record_cannot_be_read() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("plugins/installed_plugins.json")).unwrap();
    let env = env_of(&[("CLAUDE_CONFIG_DIR", home.path())]);

    let why = CLAUDE_CODE
        .installed(&Env::of(&env))
        .expect_err("a record that is a directory read as no record");
    assert!(why.starts_with("purlis could not read "), "{why}");
}

#[test]
fn claude_code_is_called_claude_code() {
    assert_eq!(CLAUDE_CODE.title(), "Claude Code");
}

/// A title is written once, in the harness's built-in declaration (qw312, qw323): each plugin
/// adapter is called what the harness of its `kind` is.
#[test]
fn every_plugin_adapter_is_called_what_its_harnesss_declaration_calls_it() {
    for word in ["claude", "codex", "opencode"] {
        let adapter = adapter(word).expect("an adapter for every harness purlis ships");
        let harness = crate::harness::Harness::of_kind(adapter.harness()).expect("a harness");
        assert_eq!(adapter.title(), harness.declared().title, "{word}");
    }
}

#[test]
fn a_plugin_id_is_one_short_line_with_nothing_in_it_that_draws_as_nothing() {
    for good in ["figma@official", "a", &"a".repeat(MOST_ID_BYTES)] {
        assert!(id_ok(good), "{good}");
    }
    for bad in [
        "",
        "   ",
        &"a".repeat(MOST_ID_BYTES + 1),
        "a\nb",
        "a\u{200b}b",
    ] {
        assert!(!id_ok(bad), "{bad:?}");
    }
}

#[test]
fn the_settings_tab_lists_every_harness_with_what_it_has_installed() {
    let config = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(config.path().join("plugins")).unwrap();
    std::fs::write(
        config.path().join("plugins/installed_plugins.json"),
        r#"{"version": 2, "plugins": {"figma@official": [{"scope": "user"}]}}"#,
    )
    .unwrap();
    let chat = env_of(&[("CLAUDE_CONFIG_DIR", config.path())]);
    let env = Env {
        chat: &chat,
        home: None,
        process: false,
    };

    let groups = survey(&Choices::default(), &env);
    let harnesses: Vec<&str> = groups.iter().map(|it| it.adapter.harness()).collect();
    assert_eq!(harnesses, ["claude", "opencode", "codex"]);
    assert_eq!(
        groups[0].record,
        Some(config.path().join("plugins/installed_plugins.json"))
    );
    assert!(groups[0].trouble.is_none());
    assert!(
        groups[0].plugins.iter().any(|it| it.id == "figma@official"),
        "{:?}",
        groups[0]
            .plugins
            .iter()
            .map(|it| &it.id)
            .collect::<Vec<_>>()
    );
}

/// Train 77 review: a project's own config is chat-written, so it is read only where it is a
/// plain file of its own and not too big; a link (to a file elsewhere, or to one that never
/// ends) is never followed, and is named as a config purlis cannot read.
#[cfg(unix)]
#[test]
fn a_chat_written_config_is_read_only_as_a_plain_file_of_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain.json");
    std::fs::write(&plain, r#"{"plugin":["a"]}"#).unwrap();
    let elsewhere = dir.path().join("elsewhere.json");
    std::os::unix::fs::symlink(&plain, &elsewhere).unwrap();
    std::os::unix::fs::symlink("/dev/zero", dir.path().join("endless.json")).unwrap();
    std::fs::write(dir.path().join("big.json"), vec![b' '; 1024 * 1024 + 1]).unwrap();
    std::fs::create_dir(dir.path().join("folder.json")).unwrap();
    // SK-52 (#3205): a config with a second name reaches past its folder as a link does, and
    // is refused as a skill's own file is.
    std::fs::hard_link(&plain, dir.path().join("twinned.json")).unwrap();
    std::fs::write(dir.path().join("own.json"), r#"{"plugin":["b"]}"#).unwrap();
    let folder = Folder::open(dir.path(), Follow::NoLink).expect("the folder opens");

    assert_eq!(
        read_config(&folder, "own.json", HardLinks::Refused),
        Ok(Some(r#"{"plugin":["b"]}"#.to_owned()))
    );
    assert_eq!(
        read_config(&folder, "missing.json", HardLinks::Refused),
        Ok(None)
    );
    assert_eq!(
        read_config(&folder, "elsewhere.json", HardLinks::Refused),
        Err(())
    );
    assert_eq!(
        read_config(&folder, "endless.json", HardLinks::Refused),
        Err(())
    );
    assert_eq!(
        read_config(&folder, "big.json", HardLinks::Refused),
        Err(())
    );
    assert_eq!(
        read_config(&folder, "folder.json", HardLinks::Refused),
        Err(())
    );
    assert_eq!(
        read_config(&folder, "twinned.json", HardLinks::Refused),
        Err(())
    );
    assert_eq!(
        read_config(&folder, "plain.json", HardLinks::Refused),
        Err(()),
        "its twin is a hard link too"
    );
    assert_eq!(
        read_config(&folder, "twinned.json", HardLinks::Read),
        Ok(Some(r#"{"plugin":["a"]}"#.to_owned())),
        "the person's own folders read it"
    );
}

// -------------------------------------------------------------------------------------
// The Skill studio's switch (SK-6, #2769)
// -------------------------------------------------------------------------------------

/// The built-in skills' names, as the window hands them over.
fn built_in() -> Vec<String> {
    vec!["secrets".to_owned(), "handoff".to_owned()]
}

#[test]
fn a_built_in_skills_name_is_refused_in_harness_skills_on_every_harness() {
    // SK-6 (#2769): opencode's and Claude Code's switches go by the name alone, so a key named as
    // a built-in skill would turn purlis's own off.
    let top: toml::Table = "[harness_skills.opencode]\nsecrets = false\nreview = false\n\n\
                            [harness_skills.codex]\nhandoff = true\n"
        .parse()
        .unwrap();
    let got: Vec<String> = built_in_refused(&top, "charter.toml", "", &built_in())
        .into_iter()
        .map(|one| one.why)
        .collect();
    assert_eq!(
        got,
        [
            "harness_skills.opencode.secrets in charter.toml cannot be set: secrets is one of \
             purlis's built-in skills, which no project turns off"
                .to_owned(),
            "harness_skills.codex.handoff in charter.toml cannot be set: handoff is one of \
             purlis's built-in skills, which no project turns off"
                .to_owned(),
        ]
    );
}

fn text_of(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn the_studio_turns_a_harness_skill_off_at_the_root_in_this_machines_own_file_and_back_by_removing_it()
 {
    let plane = tempfile::tempdir().unwrap();
    let local = plane.path().join("charter.local.toml");

    switch_in_the_studio(
        plane.path(),
        None,
        crate::harness::Harness::Codex,
        Switched::Skill,
        "langfuse",
        false,
        &built_in(),
    )
    .expect("turned off");
    assert_eq!(
        Choices::read(plane.path()).skill("codex", "langfuse"),
        (Some(false), Source::Local)
    );
    assert!(
        text_of(&local).contains("langfuse = false"),
        "{}",
        text_of(&local)
    );

    switch_in_the_studio(
        plane.path(),
        None,
        crate::harness::Harness::Codex,
        Switched::Skill,
        "langfuse",
        true,
        &built_in(),
    )
    .expect("turned on");
    assert_eq!(
        Choices::read(plane.path()).skill("codex", "langfuse"),
        (None, Source::Default),
        "nothing else named it, so the harness decides again"
    );
    assert!(!text_of(&local).contains("langfuse"), "{}", text_of(&local));
}

#[test]
fn the_studio_turns_one_on_over_a_shared_off_by_writing_it_on() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(
        plane.path().join("charter.toml"),
        "[harness_plugins.claude]\n\"figma@official\" = false\n",
    )
    .unwrap();

    switch_in_the_studio(
        plane.path(),
        None,
        crate::harness::Harness::ClaudeCode,
        Switched::Plugin,
        "figma@official",
        true,
        &built_in(),
    )
    .expect("turned on");

    let all = resolve(
        &CLAUDE_CODE,
        &[plugin("claude", "figma@official")],
        &Choices::read(plane.path()),
    );
    let it = row(&all, "figma@official");
    assert_eq!((it.wanted, it.source), (Some(true), Source::Local));
}

#[test]
fn the_studio_switches_for_a_workspace_in_its_own_settings() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join("workspaces/alpha")).unwrap();
    // A manifest purlis wrote and stamped, as a workspace's is.
    std::fs::write(
        plane.path().join("workspaces/alpha/workspace.json"),
        include_str!("../../../../tests/fixtures/planes/daily/workspaces/alpha/workspace.json"),
    )
    .unwrap();

    switch_in_the_studio(
        plane.path(),
        Some("alpha"),
        crate::harness::Harness::Opencode,
        Switched::Skill,
        "review",
        false,
        &built_in(),
    )
    .expect("turned off");

    assert_eq!(
        Choices::read_in(plane.path(), Some("alpha")).skill("opencode", "review"),
        (Some(false), Source::Workspace)
    );
    assert_eq!(
        Choices::read_in(plane.path(), None).skill("opencode", "review"),
        (None, Source::Default),
        "the project's root is untouched"
    );

    switch_in_the_studio(
        plane.path(),
        Some("alpha"),
        crate::harness::Harness::Opencode,
        Switched::Skill,
        "review",
        true,
        &built_in(),
    )
    .expect("turned back on");
    assert_eq!(
        Choices::read_in(plane.path(), Some("alpha")).skill("opencode", "review"),
        (None, Source::Default),
        "its key is taken out again"
    );
    let manifest = text_of(&plane.path().join("workspaces/alpha/workspace.json"));
    assert!(!manifest.contains("review"), "{manifest}");
}

#[test]
fn the_studio_refuses_what_a_harness_cannot_switch_and_writes_nothing() {
    let plane = tempfile::tempdir().unwrap();
    let refused = [
        (
            switch_in_the_studio(
                plane.path(),
                None,
                crate::harness::Harness::ClaudeCode,
                Switched::Skill,
                "figma:frame",
                false,
                &built_in(),
            ),
            "Claude Code turns a plugin's skill off only with the whole plugin, figma, so nothing \
             was changed",
        ),
        (
            switch_in_the_studio(
                plane.path(),
                None,
                crate::harness::Harness::Opencode,
                Switched::Plugin,
                "acme",
                false,
                &built_in(),
            ),
            "plugins for opencode are not supported yet",
        ),
        (
            switch_in_the_studio(
                plane.path(),
                None,
                crate::harness::Harness::ClaudeCode,
                Switched::Plugin,
                "purlis@inline",
                false,
                &built_in(),
            ),
            "purlis@inline is always on",
        ),
        (
            switch_in_the_studio(
                plane.path(),
                None,
                crate::harness::Harness::Opencode,
                Switched::Skill,
                "secrets",
                false,
                &built_in(),
            ),
            "secrets is one of purlis's built-in skills, which no project turns off, so nothing \
             was changed",
        ),
    ];
    for (got, said) in refused {
        let why = got.expect_err("refused");
        assert!(why.starts_with(said), "{why}");
    }
    assert!(!plane.path().join("charter.local.toml").exists());
}

#[test]
fn opencode_lists_a_plugin_folder_of_more_names_than_purlis_reads_as_one_it_did_not_read() {
    // SK-44 (#3133): a project's plugin folder is a chat's to fill; past the most names purlis
    // reads of one folder, none is read, and the folder is named, saying why.
    let project = tempfile::tempdir().unwrap();
    let dir = project.path().join(".opencode/plugin");
    std::fs::create_dir_all(&dir).unwrap();
    for n in 0..=MOST_NAMES {
        std::fs::write(dir.join(format!("p{n}.ts")), "export default {}").unwrap();
    }

    let got: Vec<(String, String)> = OPENCODE
        .in_project(project.path())
        .into_iter()
        .filter(|it| it.id.starts_with(".opencode/"))
        .map(|it| (it.id, it.source))
        .collect();

    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].0, ".opencode/plugin");
    assert!(
        got[0]
            .1
            .starts_with("a folder of more than 2000 names, the most purlis reads of one"),
        "{}",
        got[0].1
    );
}

#[test]
fn a_clones_worktree_folder_of_more_names_than_purlis_reads_is_named_and_not_read() {
    // SK-44 (#3133): the worktree folder of a clone is a chat's to fill too.
    let project = project_with_a_workspace(&[]);
    let root = project.path().canonicalize().unwrap();
    let pieces = crate::worktree::root_of(&root, "smart").join("app");
    std::fs::create_dir_all(&pieces).unwrap();
    for n in 0..=MOST_NAMES {
        std::fs::write(pieces.join(format!("w{n}")), "").unwrap();
    }

    let got: Vec<(String, String)> = ids_and_sources(OPENCODE.in_workspace(&root, "smart"))
        .into_iter()
        .filter(|(id, _)| id.contains(".worktrees"))
        .collect();

    assert_eq!(
        got,
        [pair(
            "workspaces/smart/.worktrees/app",
            "a folder of more than 2000 names, the most purlis reads of one: the plugins \
             opencode loads from it are not listed"
        )]
    );
}

#[test]
fn a_chats_start_reads_the_harnesss_own_skill_index_where_its_environment_finds_its_skills() {
    // #3320: the arming turns the harness's own `skill-index` off so the chat gets purlis's index
    // alone, and what it knows of that copy is read here, at the start, with the chat's
    // environment, never with this machine's.
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(plane.path().join("charter.toml"), "schema = 1\n").unwrap();
    let config = tempfile::tempdir().unwrap();
    let index = config.path().join("skills/skill-index");
    std::fs::create_dir_all(&index).unwrap();
    std::fs::write(
        index.join("SKILL.md"),
        "---\nname: skill-index\ndescription: The person's own.\n---\n",
    )
    .unwrap();
    let dir = config.path().display().to_string();
    let none = Env {
        chat: &[],
        home: None,
        process: false,
    };
    for (kind, variable) in [("claude", "CLAUDE_CONFIG_DIR"), ("codex", "CODEX_HOME")] {
        let chat = [(variable.to_owned(), dir.clone())];
        let env = Env {
            chat: &chat,
            home: None,
            process: false,
        };
        assert!(
            for_start_as(true, kind, plane.path(), None, &env).own_index,
            "{kind}"
        );
        assert!(
            !for_start_as(true, kind, plane.path(), None, &none).own_index,
            "{kind}"
        );
        assert!(
            !for_start_as(false, kind, plane.path(), None, &env).own_index,
            "{kind}: behind the Skill studio's flag"
        );
    }
    // opencode knows purlis's index as `purlis-skill-index`: its own `skill-index` is another
    // skill, so it is not even looked for.
    let chat = [("OPENCODE_CONFIG_DIR".to_owned(), dir)];
    let env = Env {
        chat: &chat,
        home: None,
        process: false,
    };
    assert!(!for_start_as(true, "opencode", plane.path(), None, &env).own_index);
}
