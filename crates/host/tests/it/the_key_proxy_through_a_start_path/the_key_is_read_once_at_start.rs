// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-078 (T1a-61): the key is read once, when the run starts, from your keys, and handed to the
//! key proxy in memory. The recording vault seam counts one read of the entry for a run of three
//! turns, and each turn reaches the provider with the key read.

use super::StartPath;
use super::start_path::{STAND_IN_KEY, Sources, header, provider, send};

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let sources = Sources::new(
        "https://gateway.example",
        &[("https://gateway.example", provider.port())],
    );
    let (port, token) = path.start_from("run-1", sources.clone()).await;
    for turn in 1..=3 {
        assert_eq!(
            send(port, Some(&token), "/v1/messages", b"{}").await.status,
            200,
            "turn {turn}"
        );
    }

    assert_eq!(
        sources.your_keys_reads(),
        1,
        "your keys were read once for the run's three turns"
    );
    let requests = provider.requests();
    assert_eq!(requests.len(), 3);
    for request in &requests {
        assert_eq!(header(request, "x-api-key"), Some(STAND_IN_KEY));
    }
}

#[tokio::test]
async fn the_key_is_read_once_at_start() {
    check(super::start_path::Desktop::new().await).await;
}
