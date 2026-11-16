// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A key chat is started without what would take its calls around its route** (T1a-15 fix
//! round): Claude Code reads `ANTHROPIC_UNIX_SOCKET` from its launch environment alone, and a
//! socket path there sends every model call and the run's token to the socket, around the
//! listener, whatever `--settings` `env` says. A key chat's process is composed as every chat's
//! is (`chatenv::compose`), started without what `hoststart::started_without` names, so it never
//! carries it; an unproxied chat keeps the person's value (R-252).
//!
//! And **its loopback bypass is written in both spellings from whichever the person set**
//! (R-232): `NO_PROXY` and `no_proxy`, merged, with `127.0.0.1` added once, so an upper-case list
//! purlis writes never outranks a lower-case one the person keeps.

use std::ffi::OsString;
use std::path::Path;

use purlis_core::harness::{Harness, Kit, StateHooks};
use purlis_core::keyroute::{KeyRoute, Listener};

/// A key route through the run's listener, in a chat whose environment holds `env`.
fn a_key_route(env: &[(&str, &str)]) -> KeyRoute {
    let env: Vec<(String, String)> = env
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    KeyRoute::new(
        Listener {
            port: 47_555,
            token: "run-token-for-the-strip-6b2e".to_owned(),
        },
        None,
        move |asked| {
            env.iter()
                .find(|(name, _)| name == asked)
                .map(|(_, value)| value.clone())
        },
    )
}

/// What Claude Code is handed for a chat on `route`: its arguments and its environment.
fn armed(route: Option<&KeyRoute>) -> (Vec<String>, Vec<(String, String)>) {
    let StateHooks::ThisSessionOnly { args, env, .. } = Harness::ClaudeCode.state_hooks(
        Kit {
            binary: Path::new("/bin/purlis"),
            plugin: Some(Path::new("/app/plugin")),
            persona: None,
            skills: None,
            graded: false,
            judging: false,
            writes_its_piece: false,
            runs_a_shell: false,
            own: &[],
            laid_out: None,
            key_route: route,
        },
        None,
        &purlis_core::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("Claude Code is armed per session");
    };
    (args, env)
}

/// The process environment of a Claude Code chat on `route`, as its start composes it: its app
/// has `ANTHROPIC_UNIX_SOCKET` set and the operator passes it, its profile sets it too, and it is
/// armed by its adapter.
fn a_claude_chats_env(root: &Path, route: Option<&KeyRoute>) -> Vec<(String, String)> {
    let app: Vec<(OsString, OsString)> = [
        ("PATH", "/usr/bin"),
        ("ANTHROPIC_UNIX_SOCKET", "/tmp/the-persons.sock"),
    ]
    .iter()
    .map(|(name, value)| ((*name).into(), (*value).into()))
    .collect();
    let strip = purlis_core::hoststart::started_without(root, Some(Harness::ClaudeCode), route);
    let mut set = vec![(
        "ANTHROPIC_UNIX_SOCKET".to_owned(),
        "/tmp/the-profiles.sock".to_owned(),
    )];
    set.extend(armed(route).1);
    purlis_core::chatenv::compose(
        app,
        &purlis_core::chatenv::Starting {
            harness: Some(Harness::ClaudeCode),
            operator: &["ANTHROPIC_UNIX_SOCKET".to_owned()],
            strip: &strip,
            set: &set,
            git_hooks: None,
            declared: &[],
        },
    )
    .into_iter()
    .map(|(name, value)| {
        (
            name.to_string_lossy().into_owned(),
            value.to_string_lossy().into_owned(),
        )
    })
    .collect()
}

/// The values `env` holds for `name`, in order: a later one wins where the program is spawned.
fn values_of<'a>(env: &'a [(String, String)], name: &str) -> Vec<&'a str> {
    env.iter()
        .filter(|(held, _)| held == name)
        .map(|(_, value)| value.as_str())
        .collect()
}

/// Finding 1: a key chat's process never carries `ANTHROPIC_UNIX_SOCKET`: not from the app,
/// though the operator passes it, nor from its profile. Its token still reaches it.
#[test]
fn a_key_chats_process_environment_never_carries_the_unix_socket() {
    let root = tempfile::tempdir().expect("a plane");
    let route = a_key_route(&[]);

    let env = a_claude_chats_env(root.path(), Some(&route));

    assert_eq!(
        values_of(&env, "ANTHROPIC_UNIX_SOCKET"),
        Vec::<&str>::new(),
        "a key chat carries the socket"
    );
    assert_eq!(
        values_of(&env, "ANTHROPIC_AUTH_TOKEN"),
        ["run-token-for-the-strip-6b2e"]
    );
    assert_eq!(values_of(&env, "PATH"), ["/usr/bin"]);
}

/// Finding 1 (R-252): an unproxied chat keeps the person's `ANTHROPIC_UNIX_SOCKET`, the
/// profile's set over the app's.
#[test]
fn an_unproxied_chat_keeps_the_persons_unix_socket() {
    let root = tempfile::tempdir().expect("a plane");

    let env = a_claude_chats_env(root.path(), None);

    assert_eq!(
        values_of(&env, "ANTHROPIC_UNIX_SOCKET"),
        ["/tmp/the-persons.sock", "/tmp/the-profiles.sock"],
        "an unproxied chat lost the person's socket"
    );
}

/// The `env` object of the one `--settings` value in `args`.
fn settings_env_of(args: &[String]) -> serde_json::Map<String, serde_json::Value> {
    let at = args
        .iter()
        .position(|arg| arg == "--settings")
        .expect("a --settings flag");
    let settings: serde_json::Value =
        serde_json::from_str(&args[at + 1]).expect("--settings is JSON");
    settings["env"]
        .as_object()
        .cloned()
        .expect("--settings carries an env object")
}

/// The two spellings of the bypass a key chat on a route read from `env` is written with.
fn bypass_written_for(env: &[(&str, &str)]) -> [String; 2] {
    let route = a_key_route(env);
    let written = settings_env_of(&armed(Some(&route)).0);
    ["NO_PROXY", "no_proxy"].map(|spelling| {
        written[spelling]
            .as_str()
            .unwrap_or_else(|| panic!("{spelling} is written as text"))
            .to_owned()
    })
}

/// Finding 3: a person with only `no_proxy` gets it in both spellings, with the loopback added
/// once.
#[test]
fn adapter_seeds_both_bypass_spellings_from_a_lower_case_no_proxy() {
    assert_eq!(
        bypass_written_for(&[("no_proxy", "corp.example")]),
        ["corp.example,127.0.0.1", "corp.example,127.0.0.1"]
    );
}

/// Finding 3: a person with only `NO_PROXY` gets it in both spellings too.
#[test]
fn adapter_seeds_both_bypass_spellings_from_an_upper_case_no_proxy() {
    assert_eq!(
        bypass_written_for(&[("NO_PROXY", "corp.example")]),
        ["corp.example,127.0.0.1", "corp.example,127.0.0.1"]
    );
}

/// Finding 3: two different lists are merged, each host once, the loopback added once at the
/// end where neither names it, and kept where it already is.
#[test]
fn adapter_merges_both_bypass_spellings() {
    assert_eq!(
        bypass_written_for(&[
            ("NO_PROXY", "corp.example, .internal"),
            ("no_proxy", ".internal,localhost"),
        ]),
        [
            "corp.example,.internal,localhost,127.0.0.1",
            "corp.example,.internal,localhost,127.0.0.1"
        ]
    );
    assert_eq!(
        bypass_written_for(&[
            ("NO_PROXY", "127.0.0.1"),
            ("no_proxy", "localhost, 127.0.0.1")
        ]),
        ["127.0.0.1,localhost", "127.0.0.1,localhost"]
    );
}
