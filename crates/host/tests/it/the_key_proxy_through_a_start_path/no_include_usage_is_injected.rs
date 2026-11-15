// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-045: the key proxy never edits a body and never injects `stream_options.include_usage`: a
//! streamed Chat Completions request without `stream_options` reaches the provider without it.

use fake_harness::model::{Provider, Recording};

use super::StartPath;
use super::start_path::{Desktop, route_to, send};

const SENT: &[u8] =
    br#"{"model":"gpt-test","stream":true,"messages":[{"role":"user","content":"hi"}]}"#;

const CHUNKS: &str = "data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\ndata: [DONE]\n\n";

pub(crate) async fn check(path: impl StartPath) {
    let provider = Provider::builder()
        .answer("/v1/chat/completions", Recording::events(CHUNKS))
        .start()
        .expect("the fake provider serves");
    let (port, token) = path.start("run-1", route_to(&provider)).await;
    let answered = send(port, Some(&token), "/v1/chat/completions", SENT).await;
    assert_eq!(answered.status, 200, "{answered:?}");

    let requests = provider.requests();
    assert_eq!(requests.len(), 1, "the request did not reach the provider");
    let arrived: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("a JSON body arrived");
    assert!(
        arrived.get("stream_options").is_none(),
        "stream_options was added on the way: {arrived}"
    );
    assert_eq!(requests[0].body, SENT, "the body was edited on the way");
}

#[tokio::test]
async fn no_include_usage_is_injected() {
    check(Desktop::new().await).await;
}
