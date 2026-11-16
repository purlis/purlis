// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-077: a run's listener is bound to its route at its start and keeps it to its end. The
//! declaration edited mid-run leaves the run on the base it started with; the next run takes the
//! edited one.

use super::StartPath;
use super::start_path::{Sources, provider, send};

pub(crate) async fn check(path: impl StartPath) {
    let (one, two) = (provider(), provider());
    let sources = Sources::new(
        "https://one.example",
        &[
            ("https://one.example", one.port()),
            ("https://two.example", two.port()),
        ],
    );
    let (port, token) = path.start_from("run-1", sources.clone()).await;

    sources.edit("https://two.example");
    let answered = send(port, Some(&token), "/v1/messages", b"{}").await;
    assert_eq!(answered.status, 200, "{answered:?}");
    assert_eq!(one.requests().len(), 1, "the run left its starting base");
    assert!(two.requests().is_empty(), "the edit reached a running run");

    let (port, token) = path.start_from("run-2", sources).await;
    let answered = send(port, Some(&token), "/v1/messages", b"{}").await;
    assert_eq!(answered.status, 200, "{answered:?}");
    assert_eq!(
        two.requests().len(),
        1,
        "the next run did not take the edit"
    );
    assert_eq!(one.requests().len(), 1);
}

#[tokio::test]
async fn a_declaration_edited_mid_run_changes_nothing_for_that_run() {
    check(super::start_path::Desktop::new().await).await;
}
