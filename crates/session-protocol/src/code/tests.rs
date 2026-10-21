// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[tokio::test]
async fn a_code_streams_header_is_its_kind_then_the_transfer_read_back_whole() {
    let header = Header {
        transfer: 0x0102_0304_0506_0708,
    };
    let bytes = header.to_bytes();
    assert_eq!(bytes[0], CODE);
    assert_ne!(
        CODE,
        crate::view::VIEW,
        "a code stream is never read as a view"
    );
    let mut rest = &bytes[1..];
    assert_eq!(Header::read_after_kind(&mut rest).await.unwrap(), header);
}

#[tokio::test]
async fn a_bundle_at_the_bound_is_taken_whole() {
    let bundle = vec![7u8; 64];
    let mut into = Vec::new();
    let copied = copy_held_to(&mut bundle.as_slice(), &mut into, 64)
        .await
        .unwrap();
    assert_eq!((copied, into), (64, bundle));
}

#[tokio::test]
async fn a_bundle_past_the_bound_is_refused_in_a_sentence() {
    let bundle = vec![7u8; 65];
    let mut into = Vec::new();
    let refused = copy_held_to(&mut bundle.as_slice(), &mut into, 64)
        .await
        .unwrap_err();
    assert!(
        matches!(refused, Unread::TooLarge { most: 64 }),
        "{refused:?}"
    );
    assert!(
        refused.to_string().contains("larger than 64 bytes"),
        "{refused}"
    );
}
