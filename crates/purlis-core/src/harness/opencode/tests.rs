// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_reference_is_written_as_opencodes_own_mention() {
    use crate::reference::{Lines, Reference};
    let r = |path, lines, folder| ADAPTER.reference(&Reference::for_tests(path, lines, folder));
    assert_eq!(r("src/main.rs", None, false), "@src/main.rs");
    assert_eq!(r("src", None, true), "@src/");
    assert_eq!(
        r(
            "src/main.rs",
            Some(Lines {
                first: 10,
                last: 20
            }),
            false
        ),
        "@src/main.rs#10-20"
    );
    assert_eq!(
        r("src/main.rs", Some(Lines::one(7)), false),
        "@src/main.rs#7"
    );
    assert_eq!(
        r("my dir/note one.txt", None, false),
        "\"my dir/note one.txt\""
    );
}

/// opencode reads `!…` as a shell command and `/…` as one of its own: the clipboard is
/// never handed a reference beginning with either, its plain-words fallback included.
#[test]
fn a_reference_never_begins_as_a_shell_or_slash_command() {
    use crate::reference::{Reference, starts_safely};
    let r = |path| ADAPTER.reference(&Reference::for_tests(path, None, false));
    assert_eq!(r("!cmd"), "@./!cmd");
    assert_eq!(r("!a b"), "\"./!a b\"");
    for path in ["!x", "!x#y", "/abs/x", "/abs/a b", "#x"] {
        assert!(starts_safely(&r(path)), "{path} -> {}", r(path));
    }
}

#[test]
fn an_opencode_chat_is_handed_charter_s_mcp_server_in_its_session_config() {
    // HP-7: opencode merges the config in OPENCODE_CONFIG_CONTENT key by key, so the server
    // sits beside the operator's own and nothing is written.
    let bundle = tempfile::tempdir().expect("a bundle");
    let shim = crate::opencode::shim_in(bundle.path());
    std::fs::create_dir_all(shim.parent().expect("a parent")).expect("dirs");
    std::fs::write(&shim, "").expect("the shim");
    let StateHooks::ThisSessionOnly { env, .. } = ADAPTER.arm(
        Kit {
            skills: None,
            laid_out: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(bundle.path()),
            persona: None,
            graded: false,
            judging: false,
            writes_its_piece: false,
        },
        None,
        &crate::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("armed per session");
    };
    let config = env
        .iter()
        .find(|(name, _)| name == crate::opencode::CONFIG_ENV)
        .map(|(_, value)| value)
        .expect("the session config");
    let config: serde_json::Value = serde_json::from_str(config).expect("JSON");
    assert_eq!(
        config["mcp"]["purlis"],
        serde_json::json!({"type": "local", "command": ["/bin/charter", "mcp"], "enabled": true})
    );
    assert!(config["plugin"].is_array(), "{config}");
    // #3794: and each dispatch tool runs on purlis's own dispatch grant, by the name opencode
    // asks its permission under (`<server>_<tool>`, 1.18.33), never the server as a whole, but
    // where a rule of this machine's operator names it (`opencode::tests` holds those).
    let operator = crate::opencode::OperatorRules::read(&crate::opencode::Layers::here(None));
    let mut wanted: Vec<String> = crate::chattools::run_on_the_dispatch_grant()
        .map(|tool| format!("purlis_{tool}"))
        .filter(|name| operator.leaves_unasked(name))
        .collect();
    wanted.sort();
    let mut allowed: Vec<String> = config
        .get("permission")
        .and_then(serde_json::Value::as_object)
        .map(|rules| rules.keys().cloned().collect())
        .unwrap_or_default();
    allowed.sort();
    assert_eq!(allowed, wanted);
    assert_eq!(crate::chattools::run_on_the_dispatch_grant().count(), 11);
}

#[test]
fn an_opencode_chat_denies_each_skill_turned_off_except_a_built_in_skills_name() {
    // SK-6 (#2769): opencode's `permission.skill` goes by the name alone, so a built-in
    // skill's name is never denied, whatever a file says. A skill of the person's or the clone's
    // named `secrets` is another, and is denied.
    let bundle = tempfile::tempdir().expect("a bundle");
    let shim = crate::opencode::shim_in(bundle.path());
    std::fs::create_dir_all(shim.parent().expect("a parent")).expect("dirs");
    std::fs::write(&shim, "").expect("the shim");
    let secrets = bundle.path().join("skills/purlis-secrets");
    std::fs::create_dir_all(&secrets).expect("a built-in skill");
    std::fs::write(
        secrets.join("SKILL.md"),
        "---\nname: purlis-secrets\ndescription: Keeps secrets.\n---\n",
    )
    .expect("its SKILL.md");
    let chosen = crate::harness_plugin::Chosen {
        plugins: std::collections::BTreeMap::new(),
        skills_off: [
            "review".to_owned(),
            "secrets".to_owned(),
            "purlis-secrets".to_owned(),
        ]
        .into(),
    };
    let StateHooks::ThisSessionOnly { env, .. } = ADAPTER.arm(
        Kit {
            skills: None,
            laid_out: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(bundle.path()),
            persona: None,
            graded: false,
            judging: false,
            writes_its_piece: false,
        },
        None,
        &chosen,
        None,
    ) else {
        panic!("armed per session");
    };
    let config = env
        .iter()
        .find(|(name, _)| name == crate::opencode::CONFIG_ENV)
        .map(|(_, value)| value)
        .expect("the session config");
    let config: serde_json::Value = serde_json::from_str(config).expect("JSON");
    assert_eq!(
        config["permission"]["skill"],
        serde_json::json!({ "review": "deny", "secrets": "deny" })
    );
}

/// The shim is handed the set's folder the host laid out, which holds the built-in skills
/// as `purlis-<name>`, and never the bundle's own `skills/`, whose `secrets` is a bare name.
#[test]
fn an_opencode_chat_is_handed_the_bundles_skills_and_nothing_else_it_has_no_folder_for() {
    let bundle = tempfile::tempdir().expect("a bundle");
    let shim = crate::opencode::shim_in(bundle.path());
    std::fs::create_dir_all(shim.parent().expect("a parent")).expect("dirs");
    std::fs::write(&shim, "").expect("the shim");
    let secrets = bundle.path().join("skills/secrets");
    std::fs::create_dir_all(&secrets).expect("a built-in skill");
    std::fs::write(
        secrets.join("SKILL.md"),
        "---\nname: secrets\ndescription: Keeps secrets.\n---\n",
    )
    .expect("its SKILL.md");
    let folder = bundle.path().join("set");
    let handed = |laid_out| {
        let StateHooks::ThisSessionOnly { env, .. } = ADAPTER.arm(
            Kit {
                skills: None,
                laid_out,
                binary: std::path::Path::new("/bin/charter"),
                plugin: Some(bundle.path()),
                persona: None,
                graded: false,
                judging: false,
                writes_its_piece: false,
            },
            None,
            &crate::harness_plugin::Chosen::new(),
            None,
        ) else {
            panic!("armed per session");
        };
        let config = env
            .iter()
            .find(|(name, _)| name == crate::opencode::CONFIG_ENV)
            .map(|(_, value)| value.clone())
            .expect("the session config");
        let config: serde_json::Value = serde_json::from_str(&config).expect("JSON");
        config["plugin"][0].clone()
    };

    let bundled = bundle.path().join("skills").display().to_string();
    assert_eq!(handed(Some(folder.as_path()))[1]["skills"], bundled);
    assert_eq!(handed(None)[1]["skills"], bundled);
}
