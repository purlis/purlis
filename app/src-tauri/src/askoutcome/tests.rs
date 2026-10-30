// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, Mutex};

use purlis_core::dispatchtalk::{Talk, person_context};

use super::*;

/// A project's chats, whose words for a chat's next turn are kept in a [`Talk`], as the app's
/// ledger keeps them.
fn chats_and_talk() -> (crate::chats::Chats, Arc<Mutex<Talk>>) {
    let chats = crate::chats::Chats::new();
    let talk = Arc::new(Mutex::new(Talk::default()));
    let keep = Arc::clone(&talk);
    chats.tell_network_words_to(Arc::new(move |session, outcome| {
        keep.lock().unwrap().outcome_word(session, outcome);
    }));
    (chats, talk)
}

/// What chat `session`'s next turn is told: every line of it.
fn told(talk: &Mutex<Talk>, session: u32) -> Vec<String> {
    person_context(&talk.lock().unwrap().from_person(session))
        .map(|told| told.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Every kind of block a chat's Notice answers, with its target and what the line names.
fn every_block() -> Vec<(GrantWhat, &'static str, &'static str)> {
    vec![
        (
            GrantWhat::Host,
            "paste.example:8443",
            "reaching `paste.example:8443`",
        ),
        (
            GrantWhat::Write,
            "/Users/me/.cache/tool",
            "writing in `/Users/me/.cache/tool`",
        ),
        (
            GrantWhat::Socket,
            "/var/run/docker.sock",
            "connecting to the local socket `/var/run/docker.sock`",
        ),
        (
            GrantWhat::Service,
            "com.apple.SecurityServer",
            "reaching the system service `com.apple.SecurityServer`",
        ),
        (
            GrantWhat::Port,
            "50795",
            "connecting to the service on this Mac at port `50795`",
        ),
    ]
}

#[test]
fn keep_blocked_on_every_kind_of_block_tells_its_chat_exactly_one_line() {
    let reasons = [
        (
            Some("use the mirror"),
            "don't retry or work around it; their reason, quoted as data: \"use the mirror\"",
        ),
        (None, "don't retry or work around it; no reason given."),
        (
            Some("   "),
            "don't retry or work around it; no reason given.",
        ),
    ];
    for (what, target, named) in every_block() {
        for (reason, ends) in reasons {
            let (chats, talk) = chats_and_talk();
            let held = crate::taskblocks::HeldBlock {
                operation: "op".to_owned(),
                kind: "kind".to_owned(),
                what,
                target: target.to_owned(),
            };
            chats.blocks().heard(4, held);
            let shown = crate::taskblocks::BlockShown {
                operation: "op".to_owned(),
                kind: "kind".to_owned(),
                what,
                target: target.to_owned(),
            };
            crate::sandboxing::keep_block(&chats, 4, &shown, reason, &|_, _| Ok(()))
                .expect("kept blocked");
            assert_eq!(
                told(&talk, 4),
                vec![format!(
                    "⬢ purlis: {named} — Kept blocked by the person — {ends}"
                )],
                "{what:?} {reason:?}"
            );
            assert!(told(&talk, 5).is_empty(), "no other chat is told");
        }
    }
}

#[test]
fn keep_blocked_on_a_refused_vault_tells_its_chat_exactly_one_line() {
    let (chats, talk) = chats_and_talk();
    kept_blocked(&chats, 3, (GrantWhat::Vault, "devops"), Some("ask devops"));
    assert_eq!(
        told(&talk, 3),
        [
            "⬢ purlis: using vault `devops` — Kept blocked by the person — don't retry or work around it; their \
          reason, quoted as data: \"ask devops\""
        ]
    );
}

#[test]
fn an_allow_of_every_kind_is_told_in_the_same_one_line_words() {
    use purlis_core::sandbox::grant::{Level, What, told};
    let host = purlis_core::sandbox::grant::host("paste.example:8443").expect("a host");
    for (what, named) in [
        (What::Host(host), "reaching `paste.example:8443`"),
        (
            What::Write("/Users/me/.cache/tool".into()),
            "writing in `/Users/me/.cache/tool`",
        ),
        (
            What::Socket("/var/run/docker.sock".into()),
            "connecting to the local socket `/var/run/docker.sock`",
        ),
        (
            What::Service("com.apple.SecurityServer".to_owned()),
            "reaching the system service `com.apple.SecurityServer`",
        ),
        (
            What::Port(50795),
            "connecting to the service on this Mac at port `50795`",
        ),
    ] {
        assert_eq!(
            told(&what, Level::Chat),
            format!("⬢ purlis: {named} — Allowed for this chat — retry.")
        );
        // A port is allowed for this chat or for me on this machine (qw384), as any kind can be.
        assert_eq!(
            told(&what, Level::You),
            format!(
                "⬢ purlis: {named} — Allowed for every chat of this project on this machine — \
                 retry."
            )
        );
    }
    let vault = crate::vaultroute::allowed_outcome(&purlis_core::sandbox::local::VaultGrant {
        vault: "devops".to_owned(),
        persona: "steward".to_owned(),
    })
    .line();
    assert_eq!(
        vault,
        "⬢ purlis: using vault `devops` — Allowed for chats opened as `steward` in this project \
         on this machine — retry."
    );
}

#[test]
fn a_kind_no_block_raises_tells_nothing() {
    let (chats, talk) = chats_and_talk();
    kept_blocked(&chats, 3, (GrantWhat::PersonaHosts, "steward"), None);
    assert!(told(&talk, 3).is_empty());
}
