// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A token signed in through the account store reaches no chat** (#1814, FI3, ADR 0077 §7).
//!
//! A canary token is signed in for an account, the forge answering from a recording, and a
//! second account is signed in as a browser sign-in is (#729): what the device flow granted, an
//! access token and a refresh token, each a canary of its own. Then everything a chat is given
//! or can ask for is searched for all three:
//!
//! - the environment a chat is started with, composed from this process's own as the app
//!   composes it;
//! - the answer of every tool purlis's MCP server offers a chat, each called in the chat's
//!   workspace;
//! - every file under the project and the config home that a chat could be handed, the accounts
//!   file among them (only the keyring item may hold it);
//! - and the keyring item itself is the chat sandbox's to deny: the keychain files are denied to
//!   every chat, and the `purlis` binary a chat runs never claims the host's scope that signing
//!   in or reading an account's token takes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use purlis_core::active::Place;
use purlis_core::chatenv::{Starting, compose};
use purlis_core::forge::accounts::{self, Pasted};
use purlis_core::forge::http::TokenSource;
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::route::HostScope;
use purlis_core::forge::transport::Transport;
use purlis_core::forge::{Forge, ForgeError, Kind};

/// Not shaped like any forge's token, so only its value can find it.
const CANARY: &str = "canaryFORGEsignin41c7e";

/// What a browser sign-in keeps: the access token it was granted, and its refresh token.
const CANARY_ACCESS: &str = "canaryDEVICEaccess77b2d";
const CANARY_REFRESH: &str = "canaryDEVICErefresh9e03a";

/// Whether `text` holds any of the canaries.
fn holds_a_canary(text: &str) -> bool {
    [CANARY, CANARY_ACCESS, CANARY_REFRESH]
        .iter()
        .any(|canary| text.contains(canary))
}

/// Signs in to github.com as the device flow does, under the config home `config`: the grant
/// kept through the account store, its pair in the account's own keyring item.
fn signed_in_through_the_browser(config: &Path) {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/forge_contract/accounts/github/token_check.json"),
    )
    .unwrap();
    let recorded = Arc::new(Recorded::parse(&text).unwrap());
    let reach = |_: &Forge, _: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
        Ok(recorded.clone() as Arc<dyn Transport>)
    };
    let granted = purlis_core::forge::oauth::Answer::of(
        200,
        &format!(
            r#"{{"access_token": "{CANARY_ACCESS}", "scope": "repo,read:org",
                 "expires_in": 28800, "refresh_token": "{CANARY_REFRESH}"}}"#
        ),
    )
    .granted()
    .unwrap();
    let keyring = accounts::keyring_at(config);
    let account = accounts::sign_in_with_grant(
        &HostScope::for_a_test(),
        config,
        keyring.as_ref(),
        &reach,
        Kind::GitHub,
        "github.com",
        granted,
        accounts::Method::Device,
    )
    .unwrap();
    // The control: both ARE kept, in the account's own keyring item.
    let item = keyring
        .get(&accounts::service(&account), "token")
        .unwrap()
        .map(|s| s.into_inner())
        .unwrap();
    assert!(item.contains(CANARY_ACCESS) && item.contains(CANARY_REFRESH));
}

/// A project with one workspace, as the MCP tool tests make one.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    let found = purlis_core::workspaces::Plane::open(dir.path())
        .workspace("alpha")
        .unwrap();
    std::fs::create_dir_all(found.dir()).unwrap();
    found.scaffold_charter().unwrap();
    dir
}

/// Signs the canary in on gitlab.com, under the config home `config`.
fn signed_in(config: &Path) {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/forge_contract/accounts/gitlab/token_check.json"),
    )
    .unwrap();
    let recorded = Arc::new(Recorded::parse(&text).unwrap());
    let reach = |_: &Forge, _: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
        Ok(recorded.clone() as Arc<dyn Transport>)
    };
    let keyring = accounts::keyring_at(config);
    let account = accounts::sign_in_with_token(
        &HostScope::for_a_test(),
        config,
        keyring.as_ref(),
        &reach,
        Kind::GitLab,
        "gitlab.com",
        Pasted::new(CANARY.to_string()),
    )
    .unwrap();
    // The control: the token IS kept, in its own keyring item.
    assert_eq!(
        keyring
            .get(&accounts::service(&account), "token")
            .unwrap()
            .map(|s| s.into_inner())
            .as_deref(),
        Some(CANARY)
    );
}

/// Every file under `root` but the test build's keyring stub, which stands in for the keychain.
fn files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n != "keyring-stub.json") {
                found.push(path);
            }
        }
    }
    found
}

/// A value for each argument a tool's schema requires: a plain word, so a tool that runs gets as
/// far as it can.
fn arguments(schema: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    let mut args = serde_json::Map::new();
    for name in schema["required"].as_array().into_iter().flatten() {
        if let Some(name) = name.as_str() {
            let kind = schema["properties"][name]["type"]
                .as_str()
                .unwrap_or("string");
            let value = match kind {
                "integer" | "number" => serde_json::json!(1),
                "boolean" => serde_json::json!(false),
                "array" => serde_json::json!(["word"]),
                _ => serde_json::json!("word"),
            };
            args.insert(name.to_string(), value);
        }
    }
    args
}

/// Set on the child [`a_signed_in_token_is_in_no_chats_environment_tool_answer_or_file`] runs
/// its body in: the config home the canary is signed in to.
const CANARY_HOME: &str = "FI3_CANARY_CONFIG_HOME";

#[test]
fn a_signed_in_token_is_in_no_chats_environment_tool_answer_or_file() {
    purlis_core::unsteered!();
    // The tools find the account store and the keyring through this process's config home. So
    // the canary is signed in at that home, or a tool that read either could not be caught:
    // the body runs in a child whose `PURLIS_CONFIG_HOME` is this test's own directory.
    let Some(config) = std::env::var_os(CANARY_HOME).map(PathBuf::from) else {
        let home = tempfile::tempdir().unwrap();
        purlis_core::testrun::rerun_steered(
            &[
                "a_signed_in_forge_token_never_reaches_a_chat::a_signed_in_token_is_in_no_chats_environment_tool_answer_or_file",
            ],
            &[
                (purlis_core::machine::HOME_VAR, home.path().as_os_str()),
                (CANARY_HOME, home.path().as_os_str()),
            ],
        );
        return;
    };
    assert_eq!(
        purlis_core::machine::config_root().as_deref(),
        Some(config.as_path()),
        "the tools read the config home the canary is signed in at"
    );
    let plane = project();
    signed_in(&config);
    signed_in_through_the_browser(&config);

    // 1. The environment a chat starts with.
    let env = compose(
        std::env::vars_os(),
        &Starting {
            harness: None,
            operator: &["*".to_string()],
            strip: &[],
            set: &[],
            git_hooks: None,
            declared: &[],
        },
    );
    for (name, value) in &env {
        assert!(
            !holds_a_canary(&value.to_string_lossy()),
            "{} holds the token",
            name.to_string_lossy()
        );
    }

    // 2. Every tool purlis's MCP server offers a chat.
    let now = chrono::NaiveDate::from_ymd_opt(2026, 10, 12)
        .unwrap()
        .and_hms_opt(9, 0, 0)
        .unwrap();
    let place = Place::Workspace("alpha".into());
    let mut called = 0;
    for tool in &purlis_core::chattools::TOOLS {
        let said = purlis_core::chattools::call(
            plane.path(),
            &place,
            tool.name,
            &arguments(&(tool.schema)()),
            now,
        );
        let said = match said {
            Ok(said) | Err(said) => said,
        };
        assert!(
            !holds_a_canary(&said),
            "{} answered with the token",
            tool.name
        );
        called += 1;
    }
    assert_eq!(called, purlis_core::chattools::TOOLS.len());

    // 3. Every file a chat could be handed: the project's, and the config home's but the
    // keyring's own.
    let mut searched = 0;
    for path in files(plane.path()).into_iter().chain(files(&config)) {
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        assert!(!holds_a_canary(&text), "{} holds the token", path.display());
        searched += 1;
    }
    assert!(
        files(&config).iter().any(|p| p == &accounts::path(&config)),
        "the accounts file was searched"
    );
    assert!(searched > 1);
}

#[test]
fn the_keychain_holding_the_token_is_denied_to_every_chat() {
    purlis_core::unsteered!();
    let home = Path::new("/Users/op");
    let denied = purlis_core::sandbox::seatbelt::keychains(Some(home)).unwrap();
    assert_eq!(denied.path, home.join("Library/Keychains"));
    assert_eq!(denied.access, purlis_core::sandbox::Access::ReadWrite);
}

/// Signing in and reading the account store's tokens take the host's scope, and the `purlis`
/// binary, which a chat can run, never names the account store's sign-in or sign-out, nor
/// purlis's git credential helper (#752): it neither serves it nor names it for its own git. Nor
/// does it import a forge CLI's login (#1376), nor sign in through the browser or refresh what
/// that granted (#729).
#[test]
fn the_purlis_binary_never_signs_in_or_out_of_a_forge_account() {
    purlis_core::unsteered!();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../purlis-cli/src");
    let mut read = 0;
    for path in files(&src) {
        if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            read += 1;
            for word in [
                "sign_in_with_token",
                "accounts::sign_out",
                "keyring_at",
                "gitcredential",
                "as_the_window",
                // Importing a forge CLI's login (#1376) is the window's too.
                "cliimport",
                // And so are the browser sign-in, what it grants, and its refresh (#729).
                "sign_in_with_grant",
                "forge::device",
                "forge::oauth",
                "forge::refresh",
                "Refresher",
            ] {
                assert!(
                    !text.contains(word),
                    "{} names {word}: the purlis binary holds no forge sign-in",
                    path.display()
                );
            }
        }
    }
    assert!(read > 0, "no purlis-cli source was read");
}
