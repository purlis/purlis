// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The link protocol carries its own version, apart from the session protocol's (#2042): hosts
//! within one major version link, at the lower minor, and hosts of two majors are refused in
//! words that say what to do.

use purlis_session_protocol::hostlink::{self, Handshake, LinkRefused, Liveness};
use serde_json::json;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, duplex};

use crate::common;
use common::hosts::{a_as_pinned, b_as_pinned, host_a, host_b};

async fn write_hello(io: &mut (impl AsyncWrite + Unpin), json: &serde_json::Value) {
    let body = json.to_string();
    io.write_all(hostlink::MAGIC).await.unwrap();
    io.write_all(&u16::try_from(body.len()).unwrap().to_be_bytes())
        .await
        .unwrap();
    io.write_all(body.as_bytes()).await.unwrap();
}

async fn read_hello(io: &mut (impl AsyncRead + Unpin)) -> serde_json::Value {
    let mut magic = [0u8; 8];
    io.read_exact(&mut magic).await.unwrap();
    assert_eq!(&magic, hostlink::MAGIC);
    let mut body = vec![0u8; usize::from(io.read_u16().await.unwrap())];
    io.read_exact(&mut body).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn the_link_has_a_version_of_its_own() {
    assert_eq!(
        hostlink::VERSION,
        purlis_session_protocol::version::Version { major: 1, minor: 0 }
    );
    assert_eq!(hostlink::speaks().majors(), [1]);
    assert_ne!(hostlink::MAGIC, purlis_session_protocol::version::MAGIC);
}

#[tokio::test]
async fn a_host_of_a_later_minor_links_at_the_lower_one() {
    let (mut newer, b_end) = duplex(1 << 16);
    tokio::spawn(async move {
        let _ = hostlink::answer(
            b_end,
            Handshake::THIS_BUILD,
            &host_b(),
            &a_as_pinned(),
            Liveness::default(),
        )
        .await;
    });

    write_hello(&mut newer, &json!({"versions": [{"major": 1, "minor": 7}]})).await;

    assert_eq!(
        read_hello(&mut newer).await,
        json!({"accept": {"major": 1, "minor": 0}})
    );
}

#[tokio::test]
async fn a_host_of_another_major_is_refused_and_told_which_major_this_one_links_with() {
    let (mut later, b_end) = duplex(1 << 16);
    let answered = tokio::spawn(async move {
        hostlink::answer(
            b_end,
            Handshake::THIS_BUILD,
            &host_b(),
            &a_as_pinned(),
            Liveness::default(),
        )
        .await
        .map(|_| ())
    });

    write_hello(&mut later, &json!({"versions": [{"major": 2, "minor": 0}]})).await;

    assert_eq!(
        read_hello(&mut later).await,
        json!({"refuse": {"speaks": [1]}})
    );
    let refused = answered.await.unwrap().unwrap_err();
    assert!(
        matches!(&refused, LinkRefused::Majors { ours, theirs } if *ours == [1] && *theirs == [2]),
        "{refused}"
    );
    assert!(refused.to_string().contains("update purlis"), "{refused}");
}

#[tokio::test]
async fn a_dialer_refused_for_its_major_says_so_in_words() {
    let (a_end, mut later) = duplex(1 << 16);
    tokio::spawn(async move {
        let _ = read_hello(&mut later).await;
        write_hello(&mut later, &json!({"refuse": {"speaks": [2]}})).await;
    });

    let refused = hostlink::connect(
        a_end,
        Handshake::THIS_BUILD,
        &host_a(),
        &b_as_pinned(),
        Liveness::default(),
    )
    .await
    .map(|_| ())
    .unwrap_err();

    assert!(
        matches!(&refused, LinkRefused::Majors { ours, theirs } if *ours == [1] && *theirs == [2]),
        "{refused}"
    );
    assert!(
        refused.to_string().contains("one major version"),
        "{refused}"
    );
}
