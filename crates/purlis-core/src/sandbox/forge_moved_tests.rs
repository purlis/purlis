// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! gh's and glab's folders where the person's environment moves them by a relative path: gh and
//! glab read such a variable against the folder they run in, so a chat's own runs there load
//! what is in it next time. Held against the chat's own folder, at every level.

use std::path::{Path, PathBuf};

use super::*;

fn real(path: &Path) -> PathBuf {
    path.canonicalize().expect("a real path")
}

/// A project saying `toml`, a home, and a chat's folder in the project.
fn started(toml: &str, harness: Harness, env: &[(&str, &str)]) -> (tempfile::TempDir, tempfile::TempDir, PathBuf, Applied) {
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join("charter.toml"), toml).expect("charter.toml");
    let root = real(plane.path());
    let cwd = root.join("w");
    std::fs::create_dir_all(&cwd).expect("a chat's folder");
    let home = tempfile::tempdir().expect("a home");
    let machine = Machine {
        env: crate::secrets::Env::of(env),
        home: Some(real(home.path())),
        os: Os::MacOs,
    };
    let applied = for_start(harness, &root, &machine, &|_| true)
        .expect("starts")
        .expect("sandboxed");
    (plane, home, cwd, applied)
}

fn profile_in(applied: &Applied, cwd: &Path) -> String {
    let tmp = Path::new("/private/var/folders/xx/chat-tmp");
    match applied.form_in(Some(cwd)) {
        Form::Opencode(wrap) if applied.level() == Level::Floor => {
            opencode::floor_profile(&wrap, &[4040], None)
        }
        Form::Codex(wrap) if applied.level() == Level::Floor => {
            codex::floor_profile(&wrap, &[4040], None)
        }
        Form::Opencode(wrap) => opencode::profile(&wrap, cwd, tmp, &[4040], None),
        Form::Codex(wrap) => codex::profile(&wrap, cwd, tmp, &[4040], None),
        other => panic!("not a wrap: {other:?}"),
    }
    .expect("a profile")
}

#[test]
fn a_forge_programs_folder_moved_by_a_relative_variable_is_held_in_the_chat_s_own_folder() {
    use super::tests::writes;
    let env = [
        ("GH_CONFIG_DIR", ".gh"),
        ("GLAB_CONFIG_DIR", "tools/../glab"),
    ];
    for toml in [
        "schema = 1\n",
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n",
    ] {
        for harness in [Harness::Codex, Harness::Opencode] {
            let (_plane, _home, cwd, applied) = started(toml, harness, &env);
            let profile = profile_in(&applied, &cwd);
            let level = applied.level();
            for not in [cwd.join(".gh/hosts.yml"), cwd.join("glab/aliases.yml")] {
                assert!(
                    !writes(&profile, &not),
                    "{harness:?} at {level:?}: {} is writable",
                    not.display()
                );
            }
            assert!(
                writes(&profile, &cwd.join("src/main.rs")),
                "{harness:?} at {level:?}: the chat's own work is not writable"
            );
        }
    }
}

#[test]
fn a_relative_variable_naming_the_chat_s_folder_or_above_holds_nothing() {
    use super::tests::writes;
    for value in [".", "..", "./"] {
        let env = [("GH_CONFIG_DIR", value)];
        let (_plane, _home, cwd, applied) = started("schema = 1\n", Harness::Codex, &env);
        let profile = profile_in(&applied, &cwd);
        assert!(
            writes(&profile, &cwd.join("src/main.rs")),
            "{value}: the chat's own work is held"
        );
    }
}
