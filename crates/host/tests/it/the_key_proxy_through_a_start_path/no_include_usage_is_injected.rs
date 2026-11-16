// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-045: the key proxy never edits a body and never injects `stream_options.include_usage`: a
//! streamed request without `stream_options` reaches the provider without it. It is a Messages
//! request on the Anthropic built-in's route, the one format whose allowlist 1a lists (T1a-08);
//! the Chat Completions form returns with that format's list in 1b.

use fake_harness::model::{Provider, Recording};

use super::StartPath;
use super::start_path::{Desktop, route_to, send};

const SENT: &[u8] = br#"{"model":"claude-test","max_tokens":16,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#;

const CHUNKS: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-test\",\"usage\":{\"input_tokens\":3,\"output_tokens\":0}}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

pub(crate) async fn check(path: impl StartPath) {
    let provider = Provider::builder()
        .answer("/v1/messages", Recording::events(CHUNKS))
        .start()
        .expect("the fake provider serves");
    let (port, token) = path.start("run-1", route_to(&provider)).await;
    let answered = send(port, Some(&token), "/v1/messages", SENT).await;
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
