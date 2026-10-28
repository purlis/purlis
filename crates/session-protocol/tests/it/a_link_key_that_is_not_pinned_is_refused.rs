// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Pinning fails closed (ADR 0078 §3, #2042): a host links only with a host whose link key was
//! pinned when the two were paired, and a runner with the one desktop it is paired with. A kept
//! key whose halves do not match is refused before it is ever pinned. Each refusal says which key was offered by its fingerprint,
//! and never prints a key.

use purlis_session_protocol::hostlink::{
    self, Handshake, LinkRefused, Liveness, Pinned, PublicKey,
};
use tokio::io::duplex;

use crate::common;
use common::hosts::{HOST_B, a_as_pinned, b_as_pinned, host_a, host_b, identity, pinned};

fn hex(key: &PublicKey) -> String {
    key.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

/// Host A dials, as `dialer`, a host B that answers as `answerer` and admits `peers`, its one
/// desktop; each
/// end's outcome.
async fn link(
    dialer: hostlink::Identity,
    expecting: Pinned,
    answerer: hostlink::Identity,
    peers: Pinned,
) -> (Result<(), LinkRefused>, Result<(), LinkRefused>) {
    let (a_end, b_end) = duplex(1 << 16);
    let answered = tokio::spawn(async move {
        hostlink::answer(
            b_end,
            Handshake::THIS_BUILD,
            &answerer,
            &peers,
            Liveness::default(),
        )
        .await
        .map(|_| ())
    });
    let dialed = hostlink::connect(
        a_end,
        Handshake::THIS_BUILD,
        &dialer,
        &expecting,
        Liveness::default(),
    )
    .await
    .map(|_| ());
    (dialed, answered.await.unwrap())
}

#[tokio::test]
async fn a_host_that_is_not_the_paired_desktop_is_refused_and_told_why() {
    let stranger = identity("01JC0STRANGER0000000000000", 0xC3);
    let offered = stranger.key.public();

    let (dialed, answered) = link(stranger, b_as_pinned(), host_b(), a_as_pinned()).await;

    let answered = answered.unwrap_err();
    assert!(
        matches!(&answered, LinkRefused::UnknownPeer { offered: fp } if *fp == offered.fingerprint()),
        "{answered}"
    );
    let dialed = dialed.unwrap_err();
    assert!(matches!(dialed, LinkRefused::ByThePeer(_)), "{dialed}");
    for words in [answered.to_string(), dialed.to_string()] {
        assert!(words.contains(&offered.fingerprint()), "{words}");
        assert!(!words.contains(&hex(&offered)), "no key in {words}");
    }
}

#[tokio::test]
async fn a_host_that_answers_with_another_key_than_its_pin_is_refused_before_the_dialer_finishes() {
    let impostor = identity(HOST_B, 0xD4);
    let offered = impostor.key.public();

    let (dialed, answered) = link(host_a(), b_as_pinned(), impostor, a_as_pinned()).await;

    let dialed = dialed.unwrap_err();
    assert!(
        matches!(&dialed, LinkRefused::NotThePinnedHost { device, offered: fp, .. }
            if device == HOST_B && *fp == offered.fingerprint()),
        "{dialed}"
    );
    assert!(!dialed.to_string().contains(&hex(&offered)));
    assert!(answered.is_err(), "the impostor never admits the dialer");
}

#[tokio::test]
async fn a_connector_that_reaches_another_host_than_the_paired_one_fails_the_handshake() {
    // The other host holds host B's key, and would admit host A, but is not host B.
    let elsewhere = identity("01JE0ELSEWHERE000000000000", 0xB2);
    let peers = pinned(&host_a());

    let (dialed, answered) = link(host_a(), b_as_pinned(), elsewhere, peers).await;

    assert!(
        matches!(dialed, Err(LinkRefused::HandshakeFailed(_))),
        "{dialed:?}"
    );
    assert!(answered.is_err());
}

#[tokio::test]
async fn a_key_prints_only_as_its_fingerprint() {
    let a = host_a();

    let printed = format!("{a:?}");

    assert!(printed.contains(&a.key.public().fingerprint()), "{printed}");
    assert!(!printed.contains(&hex(&a.key.public())), "{printed}");
}

#[test]
fn a_kept_key_is_loaded_only_when_its_halves_match() {
    let kept = || hostlink::Zeroizing::new([7u8; 32]);
    let public = hostlink::LinkKey::from_private(kept()).unwrap().public();

    let loaded = hostlink::LinkKey::from_parts(kept(), public).expect("its own halves");
    let mismatched = hostlink::LinkKey::from_parts(kept(), host_a().key.public());

    assert_eq!(loaded.public(), public);
    assert!(
        matches!(mismatched, Err(LinkRefused::HandshakeFailed(_))),
        "{mismatched:?}"
    );
}
