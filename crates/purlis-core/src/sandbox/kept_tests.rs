// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat does not start when purlis cannot tell what to keep from it (ADR 0067 §5 as
//! amended): the list of what a chat is kept from is whole, or no sandbox is compiled from it.
//! And a project in an ordinary state starts as it always did. Every expected answer is
//! written out.

use std::path::{Path, PathBuf};

use super::*;

/// The sandbox on, without the package caches: a test machine has no data home to make them in.
const ON: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n";

const HARNESSES: [Harness; 3] = [Harness::ClaudeCode, Harness::Codex, Harness::Opencode];

/// A machine whose home is the folder beside the project at `root` ([`project`]).
fn machine(os: Os, root: &Path) -> Machine {
    Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(root.with_file_name("home")),
        os,
    }
}

/// A project's folder, as the kernel names it, with a home beside it and not in it.
fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a folder");
    let held = dir.path().canonicalize().expect("as the kernel names it");
    let root = held.join("project");
    std::fs::create_dir_all(&root).expect("a project");
    std::fs::create_dir_all(held.join("home")).expect("a home");
    (dir, root)
}

/// This machine's half of the vault registry, in the project at `root`.
fn local_half(root: &Path) -> PathBuf {
    crate::secrets::Ctx::new(root, crate::secrets::Env::of(&[])).local_registry()
}

/// What a chat of `harness` in the project at `root` starts under on `os`, the sandbox on.
fn started(harness: Harness, root: &Path, os: Os) -> Result<Applied, NotStarted> {
    let plane = Plane::of(Some(ON));
    let policy = plane.said().policy.expect("the sandbox is on");
    applied_for(harness, &policy, &plane, root, &machine(os, root))
}

/// What is asserted of a project whose list cannot be told whole: every harness is refused,
/// with `why` and the registry file to put right, before its compiler is asked, and the
/// window's list is refused the same.
fn refused_everywhere(root: &Path, why: &str, registry: Option<&Path>) {
    let untold = Untold {
        why: why.to_owned(),
        registry: registry.map(Path::to_path_buf),
    };
    assert_eq!(
        Denied::of(root, &machine(Os::MacOs, root)).untold,
        Some(untold.clone())
    );
    for os in [Os::MacOs, Os::Linux] {
        for harness in HARNESSES {
            let refused = started(harness, root, os)
                .map(|_| ())
                .expect_err("a chat started");
            assert_eq!(
                refused,
                NotStarted::KeptUntold(untold.clone()),
                "{harness:?} on {os:?}"
            );
        }
    }
    assert_eq!(
        kept_from_reading(root, &machine(Os::MacOs, root)),
        Err(untold)
    );
}

/// What Doctor is told of the list of the project at `root`.
fn noted(root: &Path) -> Vec<String> {
    let (untold, notes) = kept_said(root, &machine(Os::MacOs, root));
    assert_eq!(untold, None);
    notes.iter().map(ToString::to_string).collect()
}

/// The project's committed registry, with `text`.
fn committed(root: &Path, text: &str) -> PathBuf {
    let file = root.join("vaults.json");
    std::fs::write(&file, text).expect("the registry");
    file
}

/// This machine's half of the project's registry, with `text`.
fn on_this_machine(root: &Path, text: &str) -> PathBuf {
    let file = local_half(root);
    std::fs::create_dir_all(file.parent().expect("the state folder")).expect("a folder");
    std::fs::write(&file, text).expect("this machine's half");
    file
}

/// What is asserted of a project in an ordinary state: the list is whole, and a chat of every
/// harness starts.
fn starts_everywhere(root: &Path) {
    assert_eq!(Denied::of(root, &machine(Os::MacOs, root)).untold, None);
    for harness in HARNESSES {
        started(harness, root, Os::MacOs)
            .unwrap_or_else(|refused| panic!("{harness:?} was refused: {refused}"));
    }
    kept_from_reading(root, &machine(Os::MacOs, root)).expect("the list is told whole");
}

// -------------------------------------------------------------------------------------
// Ordinary states start
// -------------------------------------------------------------------------------------

#[test]
fn a_project_with_no_vault_registry_and_no_state_folder_starts_its_chats() {
    let (_dir, root) = project();
    starts_everywhere(&root);
    assert_eq!(noted(&root), Vec::<String>::new());
}

#[test]
fn a_project_with_a_state_folder_and_no_registry_starts_its_chats() {
    let (_dir, root) = project();
    std::fs::create_dir_all(local_half(&root).parent().expect("the state folder"))
        .expect("the state folder");
    starts_everywhere(&root);
}

#[test]
fn a_registry_that_registers_nothing_starts_its_chats() {
    for text in ["{}", r#"{"vaults": {}}"#, r#"{"schema": 1}"#] {
        let (_dir, root) = project();
        committed(&root, text);
        on_this_machine(&root, text);
        starts_everywhere(&root);
        assert_eq!(noted(&root), Vec::<String>::new(), "{text}");
    }
}

#[test]
fn a_project_whose_vaults_are_in_the_keyring_starts_its_chats() {
    let (_dir, root) = project();
    committed(&root, r#"{"vaults": {"dev": {"provider": "keyring"}}}"#);
    starts_everywhere(&root);
}

#[test]
fn a_project_with_a_vault_of_every_provider_starts_its_chats_and_keeps_the_plain_file() {
    let (_dir, root) = project();
    committed(
        &root,
        r#"{"vaults": {
            "box": {"provider": "keyring"},
            "dev": {"provider": "plain-file", "config": {"file": "secrets/dev.json"}},
            "team": {"provider": "1password", "config": {"op-vault": "Team"}},
            "ref": {"provider": "reference"}
        }}"#,
    );
    starts_everywhere(&root);
    assert_eq!(noted(&root), Vec::<String>::new());
    assert!(keeps(&root, "secrets/dev.json"));
}

/// Whether a chat in the project at `root` neither reads nor writes `path`, as a vault's.
fn keeps(root: &Path, path: &str) -> bool {
    Denied::of(root, &machine(Os::MacOs, root))
        .paths
        .iter()
        .any(|it| {
            it.path == root.join(path)
                && it.access == Access::ReadWrite
                && it.class == Class::Vaults
        })
}

#[test]
fn a_vault_this_machines_half_completes_starts_its_chats_and_keeps_its_file() {
    // The committed half names the vault; this machine's half says where its file is.
    let (_dir, root) = project();
    committed(&root, r#"{"vaults": {"dev": {"provider": "plain-file"}}}"#);
    on_this_machine(
        &root,
        r#"{"vaults": {"dev": {"config": {"file": "secrets/dev.json"}}}}"#,
    );
    starts_everywhere(&root);
    assert!(keeps(&root, "secrets/dev.json"));
    assert_eq!(noted(&root), Vec::<String>::new());
}

#[test]
fn a_vault_this_machine_has_not_set_up_yet_starts_its_chats_and_is_said_so() {
    // A teammate's fresh clone: the committed half names the vault, and nothing here says
    // where its file is, so there is no file of it on this machine to keep.
    for entry in [
        r#"{"provider": "plain-file"}"#,
        r#"{"provider": "plain-file", "config": {}}"#,
        r#"{"provider": "plain-file", "config": {"file": null}}"#,
    ] {
        let (_dir, root) = project();
        committed(&root, &format!(r#"{{"vaults": {{"dev": {entry}}}}}"#));
        starts_everywhere(&root);
        assert_eq!(
            noted(&root),
            ["'dev' is not set up on this machine: it does not say where its file is"],
            "{entry}"
        );
    }
}

#[test]
fn an_entry_left_over_on_this_machine_starts_its_chats_and_is_said_so() {
    // What this machine's half keeps of a vault the committed half no longer registers.
    let (_dir, root) = project();
    committed(&root, r#"{"vaults": {"box": {"provider": "keyring"}}}"#);
    on_this_machine(
        &root,
        r#"{"vaults": {"gone": {"config": {"account": "me@example.test"}}, "empty": {}}}"#,
    );
    starts_everywhere(&root);
    assert_eq!(
        noted(&root),
        [
            "'empty' is left over on this machine: it names no kind of vault",
            "'gone' is left over on this machine: it names no kind of vault",
        ]
    );
}

#[test]
fn an_entry_left_over_still_keeps_the_file_it_names_in_the_project() {
    let (_dir, root) = project();
    on_this_machine(
        &root,
        r#"{"vaults": {"gone": {"config": {"file": "secrets/gone.json"}}}}"#,
    );
    starts_everywhere(&root);
    assert!(keeps(&root, "secrets/gone.json"));
}

#[test]
fn a_vault_of_a_kind_this_purlis_does_not_know_starts_its_chats_and_keeps_what_it_names() {
    let (_dir, root) = project();
    committed(
        &root,
        r#"{"vaults": {
            "box": {"provider": "keyring"},
            "odd": {"provider": "carrier-pigeon", "path": "coop/notes",
                    "config": {"file": "coop/odd.json", "elsewhere": "coop/unseen"}},
            "far": {"provider": "carrier-pigeon", "config": {"file": "/somewhere/else.json",
                    "path": "../outside"}}
        }}"#,
    );
    starts_everywhere(&root);
    assert_eq!(
        noted(&root),
        [
            "'far' is a vault of a kind this purlis does not know (carrier-pigeon): update purlis",
            "'odd' is a vault of a kind this purlis does not know (carrier-pigeon): update purlis",
        ]
    );
    // The files it names are kept, wherever they are; nothing else is guessed at.
    assert!(keeps(&root, "coop/odd.json") && keeps(&root, "coop/notes"));
    assert!(!keeps(&root, "coop/unseen"));
    let kept = vault_places(&root);
    assert!(
        kept.contains(&PathBuf::from("/somewhere/else.json")),
        "{kept:?}"
    );
    assert!(kept.contains(&root.with_file_name("outside")), "{kept:?}");
}

/// Every place a chat in the project at `root` neither reads nor writes as a vault's.
fn vault_places(root: &Path) -> Vec<PathBuf> {
    Denied::of(root, &machine(Os::MacOs, root))
        .paths
        .into_iter()
        .filter(|it| it.class == Class::Vaults && it.access == Access::ReadWrite)
        .map(|it| it.path)
        .collect()
}

#[test]
fn an_entry_keeps_the_files_it_names_whatever_kind_it_says() {
    // The same entry, its kind typed right, typed wrong, of the wrong type, and left out.
    let outside = "/elsewhere/secrets/x.json";
    let stepped = "workspaces/../secrets/dev.json";
    for kind in [
        r#""provider": "plain-file","#,
        r#""provider": "plain_file","#,
        r#""provider": "Plain-File","#,
        r#""provider": 7,"#,
        "",
    ] {
        for file in [outside, stepped, "secrets/dev.json"] {
            let (_dir, root) = project();
            committed(
                &root,
                &format!(r#"{{"vaults": {{"dev": {{{kind} "config": {{"file": "{file}"}}}}}}}}"#),
            );
            starts_everywhere(&root);
            let kept = vault_places(&root);
            let written =
                crate::secrets::Ctx::new(&root, crate::secrets::Env::of(&[])).vault_file_path(file);
            assert!(kept.contains(&written), "{kind} {file}: {kept:?}");
            if file == stepped {
                assert!(
                    kept.contains(&real(&root.join("secrets/dev.json")))
                        || kept.contains(&root.join("secrets/dev.json")),
                    "{kind} {file}: {kept:?}"
                );
                // And the window leaves it out, as it does the same file typed right.
                let window = kept_from_reading(&root, &machine(Os::MacOs, &root)).expect("whole");
                assert!(window.contains(&written), "{kind}: {window:?}");
            }
        }
    }
}

#[test]
fn an_entry_that_is_not_a_vault_starts_its_chats_and_one_under_a_refused_name_is_still_judged() {
    let (_dir, root) = project();
    committed(
        &root,
        r#"{"vaults": {
            "s": "a string",
            "-flag": {"provider": "plain-file", "config": {"file": "secrets/flag.json"}},
            "a/../b": {"provider": "keyring"}
        }}"#,
    );
    starts_everywhere(&root);
    // Its file is kept though no command reads a vault by that name.
    assert!(keeps(&root, "secrets/flag.json"));
    let denied = Denied::of(&root, &machine(Os::MacOs, &root));
    assert_eq!(denied.services, [Service::CredentialStore]);
}

// -------------------------------------------------------------------------------------
// A list that cannot be told whole starts nothing
// -------------------------------------------------------------------------------------

fn unread(file: &Path) -> String {
    format!("the vault registry {} could not be read", file.display())
}

#[test]
fn no_chat_starts_where_the_vault_registry_does_not_read() {
    for text in ["{ not json", "[]", "\"vaults\""] {
        let (_dir, root) = project();
        let file = committed(&root, text);
        refused_everywhere(&root, &unread(&file), Some(&file));
    }
    // This machine's half alone.
    let (_dir, root) = project();
    let file = on_this_machine(&root, "{ not json");
    refused_everywhere(&root, &unread(&file), Some(&file));
    // A registry that is a folder is there, and does not read.
    let (_dir, root) = project();
    let file = root.join("vaults.json");
    std::fs::create_dir_all(&file).expect("a folder");
    refused_everywhere(&root, &unread(&file), Some(&file));
}

#[cfg(unix)]
#[test]
fn no_chat_starts_where_the_vault_registry_is_a_link() {
    let (_dir, root) = project();
    std::fs::write(root.join("elsewhere.json"), r#"{"vaults": {}}"#).expect("a file");
    let file = root.join("vaults.json");
    std::os::unix::fs::symlink(root.join("elsewhere.json"), &file).expect("a link");
    refused_everywhere(&root, &unread(&file), Some(&file));
}

#[test]
fn no_chat_starts_where_the_registrys_vaults_is_not_an_object() {
    for text in [
        r#"{"vaults": ["dev"]}"#,
        r#"{"vaults": null}"#,
        r#"{"vaults": "dev"}"#,
    ] {
        let (_dir, root) = project();
        let file = committed(&root, text);
        let why = format!(
            "'vaults' in the vault registry {} is not a list of vaults",
            file.display()
        );
        refused_everywhere(&root, &why, Some(&file));

        let (_dir, root) = project();
        let file = on_this_machine(&root, text);
        let why = format!(
            "'vaults' in the vault registry {} is not a list of vaults",
            file.display()
        );
        refused_everywhere(&root, &why, Some(&file));
    }
}

#[test]
fn no_chat_starts_where_a_plain_file_vault_says_where_its_file_is_in_a_way_purlis_cannot_use() {
    for entry in [
        r#"{"provider": "plain-file", "config": {"file": ""}}"#,
        r#"{"provider": "plain-file", "config": {"file": 7}}"#,
        r#"{"provider": "plain-file", "config": {"file": ["secrets/dev.json"]}}"#,
        r#"{"provider": "plain-file", "config": "secrets/dev.json"}"#,
    ] {
        let (_dir, root) = project();
        let file = committed(&root, &format!(r#"{{"vaults": {{"dev": {entry}}}}}"#));
        let why = format!(
            "the vault 'dev' in the vault registry {} does not say where its file is",
            file.display()
        );
        refused_everywhere(&root, &why, Some(&file));
    }
    // Said of the half that says it: this machine's, over a committed vault.
    let (_dir, root) = project();
    committed(&root, r#"{"vaults": {"dev": {"provider": "plain-file"}}}"#);
    let file = on_this_machine(&root, r#"{"vaults": {"dev": {"config": {"file": 7}}}}"#);
    let why = format!(
        "the vault 'dev' in the vault registry {} does not say where its file is",
        file.display()
    );
    refused_everywhere(&root, &why, Some(&file));
    // And under a name purlis refuses, judged where it is written.
    let (_dir, root) = project();
    let file = committed(
        &root,
        r#"{"vaults": {"-flag": {"provider": "plain-file", "config": {"file": 7}}}}"#,
    );
    let why = format!(
        "the vault '-flag' in the vault registry {} does not say where its file is",
        file.display()
    );
    refused_everywhere(&root, &why, Some(&file));
}

#[test]
fn a_name_from_the_registry_is_said_on_one_line() {
    let (_dir, root) = project();
    committed(
        &root,
        r#"{"vaults": {"two\nlines": {"provider": "plain-file", "config": {"file": 7}},
            "more\nlines": {"provider": "carrier\npigeon"}}}"#,
    );
    let (untold, notes) = kept_said(&root, &machine(Os::MacOs, &root));
    let why = untold.expect("the list is not whole").why;
    assert!(!why.contains('\n'), "{why:?}");
    for note in notes {
        assert!(!note.to_string().contains('\n'), "{note:?}");
    }
}

#[test]
fn the_refusal_is_a_sentence_that_names_the_file_to_put_right_and_doctor() {
    let refused = NotStarted::KeptUntold(Untold {
        why: "the vault registry /p/vaults.json could not be read".to_owned(),
        registry: Some(PathBuf::from("/p/vaults.json")),
    });
    assert_eq!(
        refused.to_string(),
        "this project runs every chat sandboxed, and the vault registry /p/vaults.json could \
         not be read, so purlis cannot tell what to keep from this chat, and nothing was \
         started. Doctor says what is wrong: put that file right, or remove the vault in its \
         view, and start the chat again."
    );
    let refused = NotStarted::KeptUntold(Untold {
        why: "the vault file /p/s/dev.json has more than one name, and this project holds \
              more files than purlis looks through for the others"
            .to_owned(),
        registry: None,
    });
    assert!(
        refused
            .to_string()
            .ends_with("Doctor says which file: leave it one name, and start the chat again."),
        "{refused}"
    );
}

#[test]
fn the_refusal_comes_before_any_compiler_is_asked() {
    // A compiler that would refuse for its own reason is never reached.
    let untold = Untold {
        why: "the vault registry /plane/vaults.json could not be read".to_owned(),
        registry: Some(PathBuf::from("/plane/vaults.json")),
    };
    let compiled = Compiled {
        denied: Denied {
            untold: Some(untold.clone()),
            services: vec![Service::CredentialStore],
            ..Denied::default()
        },
        hosts: Vec::new(),
        reach: reach::Reach::open(Vec::new()),
        writable: Vec::new(),
        os: Os::Linux,
        homes: Homes::default(),
        widened: Widened::default(),
    };
    for harness in HARNESSES {
        let compile = compiler(harness).expect("a compiler");
        let refused = compile_checked(
            compile,
            &compiled,
            Path::new("/plane"),
            &machine(Os::Linux, Path::new("/plane")),
        )
        .map(|_| ())
        .expect_err("refused");
        assert_eq!(
            refused,
            NotStarted::KeptUntold(untold.clone()),
            "{harness:?}"
        );
    }
}

// -------------------------------------------------------------------------------------
// A vault's file is kept under the other names purlis finds for it in the project
// -------------------------------------------------------------------------------------

/// A project with a plain-file vault at `secrets/dev.json` and a vault in the vaults folder.
#[cfg(unix)]
fn project_with_vault_files() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let (dir, root) = project();
    std::fs::create_dir_all(root.join("secrets")).expect("a folder");
    std::fs::write(root.join("secrets/dev.json"), "{}").expect("the vault's file");
    committed(
        &root,
        r#"{"vaults": {"dev": {"provider": "plain-file", "config": {"file": "secrets/dev.json"}}}}"#,
    );
    let vaults = crate::secrets::Ctx::new(&root, crate::secrets::Env::of(&[])).vaults_dir();
    std::fs::create_dir_all(&vaults).expect("the vaults folder");
    std::fs::write(vaults.join("forge.json"), "{}").expect("a vault");
    let plain = root.join("secrets/dev.json");
    let stored = vaults.join("forge.json");
    (dir, root, plain, stored)
}

#[cfg(unix)]
#[test]
fn another_name_of_a_vaults_file_is_denied_as_the_file_is() {
    let (_dir, root, plain, stored) = project_with_vault_files();
    std::fs::create_dir_all(root.join("notes/deep")).expect("a folder");
    let beside = root.join("notes/deep/copy.json");
    std::fs::hard_link(&plain, &beside).expect("a second name");
    let other = root.join("harmless.json");
    std::fs::hard_link(&stored, &other).expect("a second name");
    // In a workspace's clone and in a branch's folder, where chats work.
    std::fs::create_dir_all(root.join("workspaces/alpha/svc/.git")).expect("a clone");
    std::fs::create_dir_all(root.join("workspaces/alpha/.worktrees/svc/one")).expect("a folder");
    let cloned = root.join("workspaces/alpha/svc/kept.json");
    let branched = root.join("workspaces/alpha/.worktrees/svc/one/kept.json");
    std::fs::hard_link(&plain, &cloned).expect("a name");
    std::fs::hard_link(&plain, &branched).expect("a name");
    // Two names, neither a vault's.
    std::fs::write(root.join("README.md"), "read me").expect("a file");
    std::fs::hard_link(root.join("README.md"), root.join("README.again")).expect("a name");

    let denied = Denied::of(&root, &machine(Os::MacOs, &root));
    assert_eq!(denied.untold, None);
    let kept: Vec<&PathBuf> = denied
        .paths
        .iter()
        .filter(|it| it.class == Class::Vaults && it.access == Access::ReadWrite)
        .map(|it| &it.path)
        .collect();
    for name in [&beside, &other, &cloned, &branched, &plain] {
        assert_eq!(
            kept.iter().filter(|it| **it == name).count(),
            1,
            "{name:?} in {kept:?}"
        );
    }
    assert!(!kept.contains(&&root.join("README.again")), "{kept:?}");
    assert!(!kept.contains(&&stored), "named by its folder: {kept:?}");

    // The window's list is the same list.
    let window = kept_from_reading(&root, &machine(Os::MacOs, &root)).expect("told whole");
    assert!(
        window.contains(&beside) && window.contains(&other),
        "{window:?}"
    );

    // And each harness's sandbox is compiled with it.
    let claude = started(Harness::ClaudeCode, &root, Os::MacOs).expect("starts");
    let Form::ClaudeCode(settings) = claude.form() else {
        panic!("not Claude Code's");
    };
    let unread: Vec<&str> = settings.sandbox["filesystem"]["denyRead"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    for name in [&beside, &other] {
        assert!(
            unread.contains(&name.to_str().expect("a path")),
            "{unread:?}"
        );
    }
    for harness in [Harness::Codex, Harness::Opencode] {
        let applied = started(harness, &root, Os::MacOs).expect("starts");
        let wrapped = applied.form().denied().expect("a wrap's list");
        for name in [&beside, &other] {
            assert!(
                wrapped
                    .iter()
                    .any(|it| &it.path == name && it.access == Access::ReadWrite),
                "{harness:?}: {name:?}"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn a_project_whose_vault_files_have_one_name_is_not_looked_through() {
    let (_dir, _root, plain, stored) = project_with_vault_files();
    let several = several_names(&[plain, stored], 10).expect("looked at");
    assert_eq!(several, Vec::new());
}

#[cfg(unix)]
#[test]
fn the_refusal_past_the_most_looked_through_names_the_vaults_file() {
    let (_dir, root, plain, _stored) = project_with_vault_files();
    std::fs::hard_link(&plain, root.join("copy.json")).expect("a second name");
    let kept = [plain.clone()];
    let several = several_names(&kept, 10).expect("looked at");
    assert_eq!(several.len(), 1);
    assert_eq!(
        (several[0].path.clone(), several[0].names),
        (plain.clone(), 2)
    );

    // The project holds more entries than are looked through…
    assert_eq!(
        names_in(&root, &kept, &several, 1),
        Err(Untold {
            why: format!(
                "the vault file {} has more than one name, and this project holds more files \
                 than purlis looks through for the others",
                plain.display()
            ),
            registry: None,
        })
    );
    // …or a kept place holds more than are looked at.
    let secrets = root.join("secrets");
    assert_eq!(
        several_names(std::slice::from_ref(&secrets), 0),
        Err(Untold {
            why: format!(
                "{} holds more files than purlis looks at for a vault's file with more than \
                 one name",
                secrets.display()
            ),
            registry: None,
        })
    );
    // Within both, the name is found.
    assert_eq!(
        names_in(&root, &kept, &several, 100).map(|found| found.names),
        Ok(vec![root.join("copy.json")])
    );
}

#[cfg(unix)]
#[test]
fn what_git_and_builds_fill_is_not_looked_through() {
    let (_dir, root, plain, _stored) = project_with_vault_files();
    for folder in [
        ".git/objects",
        "target/debug",
        "web/node_modules/pkg",
        "src",
    ] {
        std::fs::create_dir_all(root.join(folder)).expect("a folder");
        for n in 0..5 {
            std::fs::write(root.join(folder).join(format!("f{n}")), "").expect("a file");
        }
    }
    std::fs::hard_link(&plain, root.join("src/copy.json")).expect("a second name");
    let kept = [plain];
    let several = several_names(&kept, 10).expect("looked at");
    // Thirty files lie in the folders left out; the bound is met without them.
    let found = names_in(&root, &kept, &several, 20).expect("looked through");
    assert_eq!(found.names, [root.join("src/copy.json")]);
}

#[cfg(unix)]
#[test]
fn a_folder_that_cannot_be_listed_is_not_looked_in_and_doctor_is_told() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_dir, root, plain, _stored) = project_with_vault_files();
    std::fs::hard_link(&plain, root.join("copy.json")).expect("a second name");
    let shut = root.join("shut");
    std::fs::create_dir_all(&shut).expect("a folder");
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000)).expect("shut");
    // Someone who may list any folder has nothing to be told.
    let cannot_list = std::fs::read_dir(&shut).is_err();

    let (untold, notes) = kept_said(&root, &machine(Os::MacOs, &root));
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o755)).expect("open");

    assert_eq!(untold, None);
    if cannot_list {
        assert_eq!(
            notes.iter().map(ToString::to_string).collect::<Vec<_>>(),
            [format!(
                "purlis could not look in {} for another name of a vault's file",
                shut.display()
            )]
        );
    }
    // The chat still starts, with the name that was found kept.
    assert!(keeps(&root, "copy.json"));
}

#[cfg(unix)]
#[test]
fn the_project_is_looked_through_once_for_the_same_files_and_again_when_a_name_is_added() {
    let (_dir, root, plain, _stored) = project_with_vault_files();
    std::fs::hard_link(&plain, root.join("copy.json")).expect("a second name");
    let kept = [plain.clone()];
    let first = second_names(&root, &kept).expect("looked through");
    assert_eq!(first.names, [root.join("copy.json")]);

    // Moved with the same number of names: the answer kept for these files is given again.
    std::fs::rename(root.join("copy.json"), root.join("moved.json")).expect("moved");
    assert_eq!(second_names(&root, &kept), Ok(first));

    // One more name: the files are not the same, and the project is looked through again.
    std::fs::hard_link(&plain, root.join("third.json")).expect("a third name");
    let again = second_names(&root, &kept).expect("looked through");
    assert_eq!(
        again.names,
        [root.join("moved.json"), root.join("third.json")]
    );
}

// -------------------------------------------------------------------------------------
// The start itself, from the project's own file
// -------------------------------------------------------------------------------------

/// A project that turned the sandbox on in its own file, with `registry` as its committed
/// vault registry.
fn sandboxed_project(registry: Option<&str>) -> (tempfile::TempDir, PathBuf) {
    let (dir, root) = project();
    std::fs::write(root.join("charter.toml"), ON).expect("charter.toml");
    if let Some(text) = registry {
        committed(&root, text);
    }
    (dir, root)
}

#[test]
fn a_start_is_refused_for_every_harness_where_the_list_cannot_be_told_whole() {
    let (_dir, root) = sandboxed_project(Some("{ not json"));
    let file = root.join("vaults.json");
    let untold = NotStarted::KeptUntold(Untold {
        why: unread(&file),
        registry: Some(file),
    });
    for harness in HARNESSES {
        let refused = for_start(harness, &root, &machine(Os::MacOs, &root), &|_| true)
            .map(|_| ())
            .expect_err("a chat started");
        // A harness purlis holds back is refused for that, before anything is compiled.
        match harness.adapter().sandbox_held_back() {
            Some(issue) => assert_eq!(refused, NotStarted::HeldBack(harness, issue)),
            None => assert_eq!(refused, untold, "{harness:?}"),
        }
        // A resumed or relaunched chat starts through the same decision.
        let again = decide(
            harness,
            &root,
            &machine(Os::MacOs, &root),
            &|_| true,
            None,
            None,
        )
        .map(|_| ())
        .expect_err("a chat started");
        assert_eq!(again, refused, "{harness:?}");
    }
}

#[test]
fn the_picker_says_why_before_anything_starts() {
    let (_dir, root) = sandboxed_project(Some(
        r#"{"vaults": {"dev": {"provider": "plain-file", "config": {"file": 7}}}}"#,
    ));
    let Ahead::Refused { why, install } = ahead(
        Harness::ClaudeCode,
        &root,
        &machine(Os::MacOs, &root),
        &|_| true,
        "",
        &|_| Ok(()),
    ) else {
        panic!("the picker offered a sandboxed start");
    };
    let file = root.join("vaults.json");
    assert!(
        why.contains(&format!(
            "the vault 'dev' in the vault registry {} does not say where its file is",
            file.display()
        )) && why.contains("purlis cannot tell what to keep from this chat")
            && why.contains("Doctor says what is wrong"),
        "{why}"
    );
    assert_eq!(install, None);
}

#[test]
fn a_sandboxed_project_in_an_ordinary_state_starts_from_its_own_file() {
    for registry in [
        None,
        Some("{}"),
        Some(r#"{"vaults": {"box": {"provider": "keyring"}}}"#),
        Some(
            r#"{"vaults": {"dev": {"provider": "plain-file", "config": {"file": "s/dev.json"}}}}"#,
        ),
        Some(r#"{"vaults": {"dev": {"provider": "plain-file"}}}"#),
        Some(r#"{"vaults": {"odd": {"provider": "carrier-pigeon"}, "gone": {}}}"#),
    ] {
        let (_dir, root) = sandboxed_project(registry);
        let started = for_start(
            Harness::ClaudeCode,
            &root,
            &machine(Os::MacOs, &root),
            &|_| true,
        );
        assert!(
            matches!(started, Ok(Some(_))),
            "{registry:?}: {:?}",
            started.err()
        );
    }
}

#[test]
fn a_project_that_has_not_turned_the_sandbox_on_starts_whatever_its_registry_says() {
    let (_dir, root) = project();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("charter.toml");
    committed(&root, "{ not json");
    for harness in HARNESSES {
        let started = for_start(harness, &root, &machine(Os::MacOs, &root), &|_| true);
        assert_eq!(started.map(|it| it.is_some()), Ok(false), "{harness:?}");
    }
    assert!(!in_force_at(&root));
}
