// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Claude Code's startup, as the spike recorded it** (T1a-02's spike 1a-startup, 2026-11-16,
//! at 2.1.325, whose strings the CI pin 2.1.300's match): what the pinned Claude Code sends its
//! base URL at startup and for one turn, replayed against a run's key proxy (spec C-19, R-712).
//!
//! - **Plain:** `HEAD /api/hello` with no credential (`user-agent: Bun/1.4.4`), then one
//!   `POST /v1/messages?beta=true` per model round trip, with the token. Nothing else: no
//!   `count_tokens`, no `/v1/models`, no `/inference-profiles`.
//! - **Through a proxy:** with a proxy variable set (and `NO_PROXY` carrying `127.0.0.1`, so the
//!   call stays direct), Claude Code skips `HEAD /api/hello`.
//! - **Discovery on**, by the person's own setting: one `GET /v1/models?limit=1000` at startup,
//!   with the credential.

use super::start_path::{Answered, raw, send};

/// One request Claude Code sent its base URL.
#[derive(Debug, Clone, Copy)]
pub struct Call {
    pub method: &'static str,
    pub target: &'static str,
    /// Whether it carried the run's token (`ANTHROPIC_AUTH_TOKEN`).
    pub with_token: bool,
}

/// `HEAD /api/hello`, with no credential.
pub const HELLO: Call = Call {
    method: "HEAD",
    target: "/api/hello",
    with_token: false,
};

/// One model round trip.
pub const TURN: Call = Call {
    method: "POST",
    target: "/v1/messages?beta=true",
    with_token: true,
};

/// Discovery's one call, only when the person switched it on.
pub const DISCOVERY: Call = Call {
    method: "GET",
    target: "/v1/models?limit=1000",
    with_token: true,
};

/// The startup and one turn, with no proxy variable set.
pub const PLAIN: &[Call] = &[HELLO, TURN];

/// The startup and one turn with `HTTPS_PROXY` set and `NO_PROXY` carrying `127.0.0.1`.
pub const THROUGH_A_PROXY: &[Call] = &[TURN];

/// A turn's body, as Claude Code sends it (its fields cut to what the fake provider reads).
pub const TURN_BODY: &[u8] =
    br#"{"model":"claude-test","max_tokens":16,"messages":[{"role":"user","content":"hi"}]}"#;

/// Sends `call` to `port`, carrying `token` where the recording says it did.
pub async fn replay_one(port: u16, token: &str, call: Call) -> Answered {
    let token = call.with_token.then_some(token);
    if call.method == "POST" {
        send(port, token, call.target, TURN_BODY).await
    } else {
        raw(port, call.method, call.target, token).await
    }
}

/// Sends each of `calls` to `port` in order: what each was answered.
pub async fn replay(port: u16, token: &str, calls: &[Call]) -> Vec<Answered> {
    let mut answered = Vec::new();
    for call in calls {
        answered.push(replay_one(port, token, *call).await);
    }
    answered
}
