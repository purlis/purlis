// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A recorded Messages answer, and the usage it reports.

use serde_json::Value;

/// **One recorded answer**: its status, its headers as recorded, and its body as the chunks it
/// arrived in. A streamed answer is sent chunked, one HTTP chunk per recorded chunk; any other is
/// sent whole with its length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recording {
    /// The status code.
    pub status: u16,
    /// The headers, in the order recorded, as `(name, value)`; framing headers are the
    /// provider's own and never listed here.
    pub headers: Vec<(String, String)>,
    /// The body, as the chunks it was recorded in.
    pub chunks: Vec<Vec<u8>>,
    /// Whether it is sent chunked, chunk by chunk.
    pub streamed: bool,
    /// Whether it is sent gzip-encoded (`Content-Encoding: gzip`).
    pub gzip: bool,
}

/// **The usage an answer recorded**: Messages' `usage` block, as the vendor reported it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

impl Recording {
    /// A whole, non-streamed `200` answer of `content_type` with `body`.
    pub fn whole(content_type: &str, body: &[u8]) -> Recording {
        Recording {
            status: 200,
            headers: vec![("content-type".to_owned(), content_type.to_owned())],
            chunks: vec![body.to_vec()],
            streamed: false,
            gzip: false,
        }
    }

    /// A non-streamed Messages answer: the message object `json`.
    pub fn message(json: &str) -> Recording {
        Recording::whole("application/json", json.as_bytes())
    }

    /// A streamed Messages answer: the server-sent events in `sse`, one chunk per event.
    pub fn events(sse: &str) -> Recording {
        let chunks = sse
            .split_inclusive("\n\n")
            .map(|event| event.as_bytes().to_vec())
            .collect();
        Recording::chunked("text/event-stream", chunks)
    }

    /// A streamed `200` answer of `content_type`, sent as `chunks`.
    pub fn chunked(content_type: &str, chunks: Vec<Vec<u8>>) -> Recording {
        Recording {
            streamed: true,
            chunks,
            ..Recording::whole(content_type, b"")
        }
    }

    /// This answer, sent gzip-encoded.
    pub fn gzipped(self) -> Recording {
        Recording { gzip: true, ..self }
    }

    /// Its body, whole.
    pub fn body(&self) -> Vec<u8> {
        self.chunks.concat()
    }

    /// The usage it recorded: a message's `usage`, or a stream's `message_start` usage with the
    /// last `message_delta`'s counts over it. None recorded reads as zero.
    pub fn usage(&self) -> Usage {
        let mut usage = Usage::default();
        let body = self.body();
        if !self.streamed {
            if let Ok(message) = serde_json::from_slice::<Value>(&body) {
                usage_of(&message["usage"], &mut usage);
            }
            return usage;
        }
        for data in String::from_utf8_lossy(&body)
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
        {
            let Ok(event) = serde_json::from_str::<Value>(data.trim()) else {
                continue;
            };
            match event["type"].as_str() {
                Some("message_start") => usage_of(&event["message"]["usage"], &mut usage),
                Some("message_delta") => usage_of(&event["usage"], &mut usage),
                _ => {}
            }
        }
        usage
    }
}

/// Each count `block` states, over what `into` held.
fn usage_of(block: &Value, into: &mut Usage) {
    let take = |name: &str, field: &mut u64| {
        if let Some(count) = block[name].as_u64() {
            *field = count;
        }
    };
    take("input_tokens", &mut into.input_tokens);
    take("output_tokens", &mut into.output_tokens);
    take(
        "cache_creation_input_tokens",
        &mut into.cache_creation_input_tokens,
    );
    take("cache_read_input_tokens", &mut into.cache_read_input_tokens);
}
