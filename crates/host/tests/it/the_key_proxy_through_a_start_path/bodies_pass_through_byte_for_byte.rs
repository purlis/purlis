// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-042: purlis never translates wire formats. A request's body reaches the fake provider as
//! the harness sent it, and the answer's body reaches the harness as the provider sent it,
//! byte for byte, for a non-streamed and a streamed call.

use fake_harness::model::{Provider, Recording};

use super::StartPath;
use super::start_path::{Desktop, MESSAGE, route_to, send};

/// A Messages request, its spacing, key order and escapes as a harness might write them.
const NOT_STREAMED: &str = r#"{ "model":"claude-test","max_tokens":16,
  "messages":[{"role":"user","content":"hé \"there\""}], "metadata":{}}"#;

const STREAMED: &str = r#"{"stream":true,"model":"claude-test","max_tokens":16,"messages":[{"role":"user","content":"hé"}]}"#;

const EVENTS: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-test\",\"usage\":{\"input_tokens\":3,\"output_tokens\":1}}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hé\"}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":1}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

pub(crate) async fn check(path: impl StartPath) {
    for (run, recording, sent) in [
        (
            "not-streamed",
            Recording::message(MESSAGE),
            NOT_STREAMED.as_bytes(),
        ),
        ("streamed", Recording::events(EVENTS), STREAMED.as_bytes()),
    ] {
        let provider = Provider::builder()
            .answer("/v1/messages", recording.clone())
            .start()
            .expect("the fake provider serves");
        let (port, token) = path.start(run, route_to(&provider)).await;
        let answered = send(port, Some(&token), "/v1/messages?beta=true", sent).await;
        assert_eq!(answered.status, 200, "{run}: {answered:?}");
        let requests = provider.requests();
        assert_eq!(
            requests.len(),
            1,
            "{run}: the request did not reach the provider"
        );
        assert_eq!(
            requests[0].body, sent,
            "{run}: the request's body was changed on its way to the provider"
        );
        assert_eq!(
            answered.headers.get("content-type").map(|v| v.as_bytes()),
            Some(recording.headers[0].1.as_bytes()),
            "{run}: the answer's content type was changed"
        );
        assert_eq!(
            answered.body,
            recording.body(),
            "{run}: the answer's body was changed on its way to the harness"
        );
    }
}

#[tokio::test]
async fn bodies_pass_through_byte_for_byte() {
    check(Desktop::new().await).await;
}
