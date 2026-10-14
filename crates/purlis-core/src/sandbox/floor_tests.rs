// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The floor (#2071, the operator's ruling of 2026-10-14): the default for a project that has
//! not turned the sandbox on. No network limits, no folder limits, no Allow asks; still no vault
//! read, nothing a later program runs written, and no folder on the person's `PATH` written.

use super::*;
use serde_json::Value;

/// A project that has not turned the sandbox on, with a plain-file vault registered.
fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a project");
    let root = dir.path().canonicalize().expect("a real folder");
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("charter.toml");
    (dir, root)
}

/// A macOS machine whose `PATH` holds `bin`, a folder outside the project.
fn machine_with(bin: &Path) -> Machine {
    Machine {
        env: crate::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(PathBuf::from("/home/op")),
        os: Os::MacOs,
    }
}

fn decided(harness: Harness, root: &Path, machine: &Machine) -> Option<Decided> {
    decide(harness, root, machine, &|_| true, None, None).expect("decided")
}

fn floor_of(decided: Option<Decided>) -> Applied {
    match decided {
        Some(Decided::Sandboxed(applied)) => applied,
        other => panic!("not the floor: {other:?}"),
    }
}

fn settings_of(applied: &Applied) -> claude::Settings {
    match applied.form() {
        Form::ClaudeCode(settings) => settings.clone(),
        other => panic!("not Claude Code's: {other:?}"),
    }
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_project_that_has_not_turned_the_sandbox_on_starts_its_chats_at_the_floor() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let applied = floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_with(bin.path()),
    ));
    assert_eq!(applied.level(), Level::Floor);
    // No proxy, so no Allow asks and no host list.
    assert!(!applied.through_purlis_proxy());
    assert!(applied.confines().hosts.is_empty());
    let settings = settings_of(&applied);
    let network = &settings.sandbox["network"];
    // Nothing limits the network: Claude Code restricts a command's only while domains are set.
    assert!(network.get("allowedDomains").is_none(), "{network}");
    assert!(network.get("strictAllowlist").is_none(), "{network}");
    assert!(network.get("httpProxyPort").is_none(), "{network}");
    assert_eq!(network["allowAllUnixSockets"], Value::Bool(false));
    // Nothing limits the folders written but what it holds.
    assert_eq!(
        strings(&settings.sandbox["filesystem"]["allowWrite"]),
        ["/"]
    );
    // Its web tools are its own: not denied.
    assert!(!settings.deny.iter().any(|rule| rule == "WebFetch"));
}

#[test]
fn the_floor_still_denies_the_vaults_and_what_a_later_program_runs() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let machine = machine_with(bin.path());
    let applied = floor_of(decided(Harness::ClaudeCode, &root, &machine));
    let settings = settings_of(&applied);
    let deny_read = strings(&settings.sandbox["filesystem"]["denyRead"]);
    let deny_write = strings(&settings.sandbox["filesystem"]["denyWrite"]);
    // The same classes the full sandbox denies, path for path.
    let full = Denied::of(&root, &machine);
    for denial in full.paths.iter().filter(|d| d.access == Access::ReadWrite) {
        let path = denial.path.display().to_string();
        assert!(deny_read.contains(&path), "{path} in {deny_read:?}");
    }
    for name in [".git/hooks", ".claude/settings.json"] {
        assert!(
            deny_write.iter().any(|rule| rule.ends_with(name)),
            "{name} in {deny_write:?}"
        );
    }
    assert!(
        settings.deny.iter().any(|rule| rule.starts_with("Read(")),
        "its tools are held to the vaults too"
    );
}

#[test]
fn the_floor_keeps_a_chat_from_writing_a_folder_on_the_persons_path() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let held = real(bin.path());
    let machine = machine_with(bin.path());
    let applied = floor_of(decided(Harness::ClaudeCode, &root, &machine));
    assert!(
        applied.confines().held.contains(&held),
        "{:?}",
        applied.confines().held
    );
    let deny_write = strings(&settings_of(&applied).sandbox["filesystem"]["denyWrite"]);
    assert!(
        deny_write.contains(&held.display().to_string()),
        "{held:?} in {deny_write:?}"
    );
    // A folder a person let the chat write (its Allow) is not held.
    assert!(!floor_held(&root, &machine, std::slice::from_ref(&held)).contains(&held));
    // And without the sandbox (a person's opt-out), nothing is held.
    let off = decide(
        Harness::ClaudeCode,
        &root,
        &machine,
        &|_| true,
        Some(&OptOut::default()),
        None,
    )
    .expect("decided");
    assert!(matches!(off, Some(Decided::Unsandboxed(_))), "{off:?}");
}

#[test]
fn the_floor_never_holds_the_project_or_a_folder_above_the_home() {
    let (_dir, root) = project();
    let inside = root.join("node_modules/.bin");
    std::fs::create_dir_all(&inside).expect("a folder in the project");
    let machine = Machine {
        env: crate::secrets::Env::of(&[(
            "PATH",
            &format!("{}:{}:/", inside.display(), root.display()),
        )]),
        home: Some(PathBuf::from("/home/op")),
        os: Os::MacOs,
    };
    let held = floor_held(&root, &machine, &[]);
    assert!(
        !held
            .iter()
            .any(|dir| dir.starts_with(&root) || root.starts_with(dir)),
        "{held:?}"
    );
}

#[test]
fn the_floor_holds_the_program_its_chat_starts_and_the_full_sandbox_leaves_it_to_the_check() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let elsewhere = tempfile::tempdir().expect("an install folder");
    let program = real(elsewhere.path()).join("claude");
    std::fs::write(&program, "").expect("a program");
    let mut applied = floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_with(bin.path()),
    ));
    applied.hold_programs(&[program.display().to_string()]);
    let folder = real(elsewhere.path());
    assert!(applied.confines().held.contains(&folder));
    let deny_write = strings(&settings_of(&applied).sandbox["filesystem"]["denyWrite"]);
    assert!(
        deny_write.contains(&folder.display().to_string()),
        "{deny_write:?}"
    );
}

#[test]
fn a_person_may_still_start_a_chat_without_the_floor_and_it_is_audited() {
    let (_dir, root) = project();
    let decided = decide(
        Harness::ClaudeCode,
        &root,
        &machine_with(Path::new("/opt/none")),
        &|_| true,
        Some(&OptOut {
            reason: Some("why not".to_owned()),
        }),
        None,
    );
    assert_eq!(
        decided,
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::Person,
            reason: Some("why not".to_owned()),
        })))
    );
}

#[test]
fn where_the_floor_cannot_be_applied_the_chat_starts_as_before_and_says_why() {
    let (_dir, root) = project();
    // Codex is wrapped on macOS only (#1040).
    let linux = Machine {
        os: Os::Linux,
        ..machine_with(Path::new("/opt/none"))
    };
    match decide(Harness::Codex, &root, &linux, &|_| true, None, None) {
        Ok(Some(Decided::Floorless(why))) => assert!(why.contains("no sandbox"), "{why}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_project_that_turned_the_sandbox_on_keeps_the_full_sandbox() {
    let (_dir, root) = project();
    std::fs::write(
        root.join("charter.toml"),
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n",
    )
    .expect("charter.toml");
    let applied = floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_with(Path::new("/opt/none")),
    ));
    assert_eq!(applied.level(), Level::Full);
    let settings = settings_of(&applied);
    assert!(settings.sandbox["network"].get("allowedDomains").is_some());
    assert!(settings.deny.iter().any(|rule| rule == "WebFetch"));
}

#[test]
fn a_wrapped_harness_at_the_floor_allows_everything_but_what_the_floor_denies() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let held = real(bin.path());
    let machine = machine_with(bin.path());
    for harness in [Harness::Codex, Harness::Opencode] {
        let compiled = Compiled::floor(&root, &machine, &grant::Grants::default());
        let compile = compiler(harness).expect("a compiler");
        let profile = match compile(&compiled).expect("compiled") {
            Form::Codex(wrap) => codex::floor_profile(&wrap),
            Form::Opencode(wrap) => opencode::floor_profile(&wrap),
            Form::ClaudeCode(_) => unreachable!(),
        }
        .expect("a profile");
        assert!(
            profile.starts_with("(version 1)\n(allow default)\n"),
            "{harness:?}"
        );
        assert!(
            profile.contains(&format!(
                "(deny file-write* (subpath \"{}\"))",
                held.display()
            )),
            "{harness:?}: the PATH folder is held"
        );
        for denial in compiled
            .denied
            .paths
            .iter()
            .filter(|d| d.access == Access::ReadWrite)
        {
            let path = real(&denial.path).display().to_string();
            assert!(
                profile.contains(&format!(
                    "(deny file-read* file-write* (subpath \"{path}\"))"
                )),
                "{harness:?}: {path}"
            );
        }
        assert!(profile.contains("\\.git/"), "{harness:?}: later-code names");
        assert!(
            !profile.contains("localhost:"),
            "{harness:?}: no proxy port"
        );
    }
}
