// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-063: a run's token is 256 bits, 32 bytes from the operating system's generator, written as
//! lowercase hex, and never repeats: 1,000 starts give 1,000 distinct tokens.

use std::collections::HashSet;

use super::StartPath;
use super::start_path::{Desktop, provider, route_to};

const STARTS: usize = 1000;

/// `hex`'s bytes, if it is lowercase hex and nothing else.
fn decoded(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) || !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).ok())
        .collect()
}

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let mut seen = HashSet::new();
    for start in 0..STARTS {
        let run = format!("run-{start}");
        let (_, token) = path.start(&run, route_to(&provider)).await;
        path.stop(&run).await;
        let bytes = decoded(&token).unwrap_or_else(|| panic!("not lowercase hex: {token}"));
        assert_eq!(bytes.len(), 32, "a token of {} bytes: {token}", bytes.len());
        assert!(
            seen.insert(token.clone()),
            "start {start} repeated a token: {token}"
        );
    }
    assert_eq!(seen.len(), STARTS);
}

#[tokio::test]
async fn tokens_are_32_random_bytes_and_never_repeat() {
    check(Desktop::new().await).await;
}
