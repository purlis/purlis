// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Inside a chat's process tree, the host's scope is refused, so no forge sign-in is handed
//! over and no call can take the native transport** (#1816, ADR 0070 §4, ADR 0077 §3).
//!
//! The app hands its sign-ins to its own forge calls with
//! [`signin::hand_over_unless_in_a_chat`], which claims [`HostScope`] first. This binary runs a
//! child of itself with the variables a chat's environment carries, a signed-in account on disk
//! all the same, and asks it to hand over: it must refuse, and hold no sign-in. A child with
//! neither variable is the control, and hands over.
//!
//! Each child is its own process, so the sign-ins one hands over never reach another case.

use std::ffi::OsStr;
use std::sync::Arc;

use purlis_core::forge::accounts::{self, Pasted};
use purlis_core::forge::http::TokenSource;
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::route::HostScope;
use purlis_core::forge::signin;
use purlis_core::forge::transport::Transport;
use purlis_core::forge::{Forge, ForgeError, Kind};

/// What the child is to find: `refused` or `handed`. Set only on a child this binary started.
const EXPECT: &str = "HAND_OVER_EXPECTED";

#[test]
fn inside_a_chats_process_tree_no_sign_in_is_handed_over() {
    purlis_core::unsteered!();
    let child = "a_chats_process_tree_is_handed_no_forge_sign_in::a_child_signed_in_on_disk_hands_over_only_outside_a_chat";
    for chat_mark in ["PURLIS_CHAT", "PURLIS_SESSION_ID"] {
        purlis_core::testrun::rerun(
            &[child],
            &[
                (chat_mark, OsStr::new("7")),
                (EXPECT, OsStr::new("refused")),
            ],
        );
    }
    purlis_core::testrun::rerun(&[child], &[(EXPECT, OsStr::new("handed"))]);
}

/// The child: in a run [`inside_a_chats_process_tree_no_sign_in_is_handed_over`] started, it signs
/// in an account on disk, then hands over as the app does; in any other run it does nothing.
#[test]
fn a_child_signed_in_on_disk_hands_over_only_outside_a_chat() {
    purlis_core::unsteered!();
    let Ok(expected) = std::env::var(EXPECT) else {
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let check = Arc::new(
        Recorded::parse(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/forge_contract/accounts/github/token_check.json"),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let reach =
        move |_: &Forge, _: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
            Ok(check.clone() as Arc<dyn Transport>)
        };
    let keyring = accounts::keyring_at(home.path());
    accounts::sign_in_with_token(
        &HostScope::for_a_test(),
        home.path(),
        keyring.as_ref(),
        &reach,
        Kind::GitHub,
        "github.com",
        Pasted::new("pasted-for-the-recorded-forge".to_string()),
    )
    .expect("signed in");

    let in_a_chat = expected == "refused";
    assert_eq!(HostScope::claim().is_none(), in_a_chat, "the host's scope");
    assert_eq!(
        signin::hand_over_unless_in_a_chat(home.path()),
        !in_a_chat,
        "handed over"
    );
    assert_eq!(
        signin::holds_a_sign_in(),
        !in_a_chat,
        "a sign-in the window's calls could use"
    );
}
