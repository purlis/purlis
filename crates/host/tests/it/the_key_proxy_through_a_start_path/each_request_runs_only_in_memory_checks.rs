// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-479: a run keeps the route it resolved at its start, and only in-memory checks run on each
//! request: the declaration store and your keys are read at the start and never per request.

use std::sync::atomic::Ordering;

use super::StartPath;
use super::start_path::{Sources, provider, raw, send};

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let sources = Sources::new(
        "https://gateway.example",
        &[("https://gateway.example", provider.address())],
    );
    let (port, token) = path.start_from("run-1", sources.clone()).await;
    let at_start = (
        sources.declaration_reads.load(Ordering::SeqCst),
        sources.key_reads.load(Ordering::SeqCst),
    );
    assert_eq!(at_start, (1, 1), "the start reads each once");

    assert_eq!(send(port, Some(&token), "/v1/messages", b"{}").await.status, 200);
    assert_eq!(send(port, Some(&token), "/v1/messages?beta=true", b"{}").await.status, 200);
    assert_eq!(raw(port, "POST", "/v1/files", Some(&token)).await.status, 400);
    assert_eq!(send(port, None, "/v1/messages", b"{}").await.status, 400);
    assert_eq!(raw(port, "HEAD", "/api/hello", None).await.status, 404);

    assert_eq!(
        (
            sources.declaration_reads.load(Ordering::SeqCst),
            sources.key_reads.load(Ordering::SeqCst),
        ),
        at_start,
        "a request read the declaration store or your keys"
    );
}

#[tokio::test]
async fn each_request_runs_only_in_memory_checks() {
    check(super::start_path::Desktop::new().await).await;
}
