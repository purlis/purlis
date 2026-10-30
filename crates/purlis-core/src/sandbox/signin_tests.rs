// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A profile's sign-in, kept from the agent's commands (#2037, ADR 0095 §5): each compiler, as
//! applied to a chat on a profile ([`super::Applied::on_profile`]).

use std::path::{Path, PathBuf};

use super::signin::{self, ProfileFolders};
use super::{Access, Applied, Class, Compiled, Form, Machine, Os, Plane, applied_of, compiler};
use crate::harness::Harness;

const SANDBOXED: &str = "[sandbox]\nmode = \"on\"\n";

fn machine(home: &Path) -> Machine {
    Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(home.to_path_buf()),
        os: Os::MacOs,
    }
}

/// A sandboxed project at `root`, compiled for `harness` on `machine`.
fn applied(harness: Harness, root: &Path, machine: &Machine) -> Applied {
    super::grant::TEST_HARNESS_TEMP.with(|roots| roots.set(&[]));
    let plane = Plane::of(Some(SANDBOXED));
    let policy = plane.said().policy.expect("on");
    let compiled = Compiled::of(&policy, &plane, root, machine, None);
    applied_of(
        harness,
        compiler(harness).expect("a compiler"),
        &compiled,
        root,
        machine,
    )
    .expect("compiles")
}

fn claude_settings(applied: &Applied) -> &super::claude::Settings {
    match applied.form() {
        Form::ClaudeCode(settings) => settings,
        other => panic!("not Claude Code's: {other:?}"),
    }
}

fn listed(settings: &super::claude::Settings, list: &str) -> Vec<String> {
    settings.sandbox["filesystem"][list]
        .as_array()
        .expect("a list")
        .iter()
        .map(|one| one.as_str().expect("text").to_owned())
        .collect()
}

#[test]
fn a_claude_code_chats_commands_cannot_read_or_write_its_profiles_sign_in_file() {
    let home = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let profile = home.path().join("profiles/work/config");
    let machine = machine(home.path());
    let mut applied = applied(Harness::ClaudeCode, root.path(), &machine);

    applied.on_profile(&ProfileFolders {
        config: Some(profile.clone()),
        data: None,
        sandbox_only: None,
        state: None,
    });

    let file = profile.join(".credentials.json").display().to_string();
    let settings = claude_settings(&applied);
    assert!(listed(settings, "denyRead").contains(&file), "{settings:?}");
    assert!(
        listed(settings, "denyWrite").contains(&file),
        "{settings:?}"
    );
    // Its Read and Edit tools run in its own process, which a deny rule holds.
    assert!(settings.deny.contains(&format!("Read(/{file})")));
    assert!(settings.deny.contains(&format!("Edit(/{file})")));
    // And a command purlis runs for the chat is held to it too.
    assert!(applied.confines().denied.iter().any(|denial| {
        denial.path == Path::new(&file)
            && denial.access == Access::ReadWrite
            && denial.class == Class::HumanPowers
    }));
}

#[test]
fn a_claude_code_chat_on_its_harnesss_own_folder_is_denied_the_sign_in_file_there() {
    let home = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let machine = machine(home.path());
    let mut applied = applied(Harness::ClaudeCode, root.path(), &machine);

    // No folder variable in the chat's environment: Claude Code's own default folder.
    applied.on_profile(&ProfileFolders::of(Harness::ClaudeCode, &[], &machine));

    let file = home.path().join(".claude/.credentials.json");
    assert!(
        listed(claude_settings(&applied), "denyRead").contains(&file.display().to_string()),
        "{:?}",
        claude_settings(&applied)
    );
}

#[test]
fn the_profiles_folders_are_the_ones_its_folder_variables_name_in_the_chats_environment() {
    let home = PathBuf::from("/home/op");
    let machine = machine(&home);
    let env = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    };
    assert_eq!(
        ProfileFolders::of(
            Harness::ClaudeCode,
            &env(&[("CLAUDE_CONFIG_DIR", "/p/claude")]),
            &machine
        )
        .config,
        Some(PathBuf::from("/p/claude"))
    );
    assert_eq!(
        ProfileFolders::of(Harness::Codex, &env(&[]), &machine).config,
        Some(home.join(".codex"))
    );
    let opencode = ProfileFolders::of(
        Harness::Opencode,
        &env(&[("XDG_DATA_HOME", "/p/data"), ("XDG_CONFIG_HOME", "rel")]),
        &machine,
    );
    assert_eq!(opencode.data, Some(PathBuf::from("/p/data")));
    // A relative folder is not one the harness can be pointed at: its default stands.
    assert_eq!(opencode.config, Some(home.join(".config")));
    assert_eq!(
        signin::credential_files(Harness::Opencode, &opencode),
        [
            PathBuf::from("/p/data/opencode/auth.json"),
            PathBuf::from("/p/data/opencode/mcp-auth.json")
        ]
    );
}

#[test]
fn a_wrapped_harness_reads_its_own_sign_in_so_its_wrap_is_left_as_compiled() {
    // Codex and opencode run whole inside purlis's wrap with the commands they run: the wrap
    // cannot deny what the harness itself reads, and their declarations answer no.
    let home = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let machine = machine(home.path());
    for harness in [Harness::Codex, Harness::Opencode] {
        let mut applied = applied(harness, root.path(), &machine);
        let before = applied.form().clone();
        let folders = ProfileFolders::of(harness, &[], &machine);

        applied.on_profile(&folders);

        assert_eq!(applied.form(), &before, "{harness:?}");
        for file in signin::credential_files(harness, &folders) {
            assert!(
                !applied
                    .confines()
                    .denied
                    .iter()
                    .any(|denial| denial.path == file && denial.access == Access::ReadWrite),
                "{harness:?}: {file:?}"
            );
        }
        assert!(
            !crate::harness_declaration::of(harness)
                .sign_in
                .kept_from_commands
                .holds()
        );
    }
}

#[test]
fn the_profile_page_says_plainly_where_commands_can_read_a_profiles_sign_in() {
    assert_eq!(
        signin::commands_can_read("codex").as_deref(),
        Some(
            "The commands this profile's chats run can read its sign-in: Codex and the commands \
             it runs are wrapped in one sandbox."
        )
    );
    assert_eq!(
        signin::commands_can_read("claude").as_deref(),
        Some(
            "The commands this profile's chats run can read its sign-in: on macOS and on a \
             runner purlis cannot keep Claude Code's sign-in from the commands it runs."
        )
    );
    // A harness purlis does not ship is not known to keep it from them.
    assert_eq!(
        signin::commands_can_read("aider").as_deref(),
        Some(
            "The commands this profile's chats run can read its sign-in: purlis does not know \
             that its harness keeps it from them."
        )
    );
}
