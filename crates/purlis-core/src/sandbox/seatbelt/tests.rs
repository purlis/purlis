// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #1425: a start whose words and environment together pass what the system lets a program
//! start with is refused in plain words before the system answers `E2BIG`.

use super::{Exec, TOO_LONG, exec_size, fits_exec, not_started};

const MAC: Exec = Exec {
    total: 1024 * 1024,
    word: None,
};

const LINUX: Exec = Exec {
    total: 2 * 1024 * 1024,
    word: Some(128 * 1024),
};

fn words(first_message: usize) -> Vec<String> {
    vec![
        "-p".to_owned(),
        "(version 1)\n(deny default)\n".to_owned(),
        "/usr/local/bin/codex".to_owned(),
        "exec".to_owned(),
        "x".repeat(first_message),
    ]
}

fn env(bytes: usize) -> Vec<(String, String)> {
    vec![
        ("HOME".to_owned(), "/Users/dev".to_owned()),
        ("BIG".to_owned(), "y".repeat(bytes)),
    ]
}

#[test]
fn an_ordinary_start_fits() {
    let (args, env) = (words(2_000), env(10_000));
    assert_eq!(fits_exec("/usr/bin/sandbox-exec", &args, &env, MAC), Ok(()));
    assert_eq!(
        fits_exec("/usr/bin/sandbox-exec", &args, &env, LINUX),
        Ok(())
    );
}

#[test]
fn a_first_message_past_what_the_system_takes_is_refused_before_it_starts() {
    let args = words(900 * 1024);
    let env = env(200 * 1024);
    assert_eq!(
        fits_exec("/usr/bin/sandbox-exec", &args, &env, MAC),
        Err(TOO_LONG)
    );
    // The same words with a small environment fit: the environment is counted.
    assert_eq!(
        fits_exec("/usr/bin/sandbox-exec", &args, &self::env(10), MAC),
        Ok(())
    );
}

#[test]
fn every_string_is_counted_with_its_end_and_its_pointer() {
    let args = vec!["ab".to_owned()];
    let env = vec![("K".to_owned(), "v".to_owned())];
    // "prog\0", "ab\0", "K=v\0", and a pointer for each, plus each list's closing null.
    let want = 5 + 3 + 4 + 8 * 3 + 8 * 2;
    assert_eq!(exec_size("prog", &args, &env), want);
}

#[test]
fn one_word_past_what_linux_takes_alone_is_refused_however_short_the_rest() {
    let args = words(129 * 1024);
    assert_eq!(
        fits_exec("/usr/bin/bwrap", &args, &env(10), LINUX),
        Err(TOO_LONG)
    );
    assert_eq!(
        fits_exec("/usr/bin/sandbox-exec", &args, &env(10), MAC),
        Ok(())
    );
    // An environment value is one such string too.
    assert_eq!(
        fits_exec("/usr/bin/bwrap", &words(10), &env(129 * 1024), LINUX),
        Err(TOO_LONG)
    );
}

#[test]
fn the_refusal_says_what_happened_and_the_way_on_in_plain_words() {
    let said = not_started(
        "this project runs every chat sandboxed, and",
        TOO_LONG,
        true,
    );
    assert!(said.contains("so nothing was started"), "{said}");
    assert!(said.contains("first message"), "{said}");
    assert!(!said.contains("E2BIG"), "{said}");
    // Never an opt-out: the sandbox is not what is too long.
    assert!(!said.contains("without the sandbox"), "{said}");
}

/// #1408: a wrapped chat (opencode, Codex) connects to its hook socket and never writes the
/// folder that holds it, as Claude Code's settings hold it.
#[test]
fn a_wrapped_chat_never_writes_the_hook_socket_s_folder() {
    use crate::sandbox::tests::writes;
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().canonicalize().expect("resolved");
    let cwd = root.join("workspaces/alpha/api");
    let tmp = root.join("tmp");
    let granted = root.join("granted");
    for made in [&cwd, &tmp, &granted] {
        std::fs::create_dir_all(made).expect("a folder");
    }
    let machine = crate::sandbox::Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(std::path::PathBuf::from("/home/op")),
        os: crate::sandbox::Os::MacOs,
    };
    let denied = crate::sandbox::Denied::of(&root, &machine).paths;
    let mut with_grant = super::Own::default();
    super::granted(&mut with_grant, std::slice::from_ref(&granted)).expect("granted");
    let state = crate::names::state(&root);
    for (what, chat, own, socket) in [
        (
            "the state folder, a chat at the project root",
            root.clone(),
            super::Own::default(),
            state.join("app/hooks.sock"),
        ),
        (
            "a fallback folder inside a granted folder",
            cwd.clone(),
            with_grant,
            granted.join("purlis-op-0123/hooks.sock"),
        ),
        (
            "a folder inside the chat's own",
            cwd.clone(),
            super::Own::default(),
            cwd.join(".sockets/hooks.sock"),
        ),
    ] {
        let profile =
            super::profile(&denied, &own, &chat, &tmp, &[4040], Some(&socket)).expect("a profile");
        let folder = socket.parent().expect("a folder");
        assert!(
            writes(&profile, &chat.join("notes.txt")),
            "{what}: {profile}"
        );
        for held in [
            socket.clone(),
            folder.to_path_buf(),
            folder.join("hooks.sock.new"),
        ] {
            assert!(
                !writes(&profile, &held),
                "{what}: {} is writable:\n{profile}",
                held.display()
            );
        }
        let sockets: Vec<&str> = profile
            .lines()
            .filter(|line| line.contains("unix-socket"))
            .collect();
        assert_eq!(
            sockets,
            [format!(
                "(allow network-outbound (remote unix-socket (path-literal \"{}\")))",
                socket.display()
            )],
            "{what}"
        );
    }
}

/// #951: where a project's path is too long for a unix socket, its hook channel lives in a
/// per-user fallback folder among the temporary folders a wrapped chat writes. The wrap denies
/// that folder to writes wherever it is, inside the chat's own temporary folder too.
#[test]
fn a_hook_socket_in_the_per_user_fallback_folder_is_never_the_wrapped_chat_s_to_write() {
    use crate::sandbox::tests::writes;
    let dir = tempfile::tempdir().expect("a directory");
    let base = dir.path().canonicalize().expect("resolved");
    let root = base.join("a-project-whose-path-is-long");
    let tmp = base.join("tmp");
    for made in [&root, &tmp] {
        std::fs::create_dir_all(made).expect("a folder");
    }
    let machine = crate::sandbox::Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(std::path::PathBuf::from("/home/op")),
        os: crate::sandbox::Os::MacOs,
    };
    let denied = crate::sandbox::Denied::of(&root, &machine).paths;
    let folder = tmp.join("charter-op-0123456789abcdef");
    let socket = folder.join("hooks.sock");
    let profile = super::profile(
        &denied,
        &super::Own::default(),
        &root,
        &tmp,
        &[4040],
        Some(&socket),
    )
    .expect("a profile");
    // The chat writes its temporary folder, and not the fallback folder in it.
    assert!(writes(&profile, &tmp.join("scratch.txt")), "{profile}");
    for held in [
        socket.clone(),
        folder.clone(),
        folder.join("hooks.sock.new"),
    ] {
        assert!(
            !writes(&profile, &held),
            "{} is writable:\n{profile}",
            held.display()
        );
    }
}

/// `tools/seatbelt-case.sh` measures the rules purlis writes (#2071): each `cased_` rule in it is
/// one the core generates, word for word, so the measurement is of the rules that ship.
#[test]
fn the_case_measurement_applies_the_rules_purlis_writes() {
    let script = include_str!("../../../../../tools/seatbelt-case.sh");
    let generated: Vec<String> = crate::sandbox::seatbelt::vaults_everywhere_rules()
        .expect("rules")
        .into_iter()
        .chain(crate::sandbox::seatbelt::floor_planted_rules().expect("rules"))
        .chain(crate::sandbox::seatbelt::every_project_rules().expect("rules"))
        .collect();
    let mut checked = 0;
    for line in script.lines() {
        let Some(rest) = line.strip_prefix("cased_") else {
            continue;
        };
        let (name, value) = rest.split_once("='").expect("a quoted value");
        let value = value.strip_suffix('\'').expect("one quoted value");
        let regex = format!("(regex \"{value}\")");
        assert!(
            generated.iter().any(|rule| rule.contains(&regex)),
            "cased_{name} is not a rule the core writes: {regex}"
        );
        checked += 1;
    }
    assert_eq!(checked, 5, "every cased rule of the script is checked");
}
