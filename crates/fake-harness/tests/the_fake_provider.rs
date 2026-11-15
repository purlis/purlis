// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The fake provider every key-proxy contract row tests against (R-695, R-696, R-700, design
//! §5.4): recordings over plain HTTP and TLS, the usage they recorded, the outbound trait's
//! log, a built-in's `https` origin on loopback, and the load and edge behaviour the budgets
//! read. Every wait on time is on the injected clock, but the one test of the monotonic clock.

mod support;

use std::io::Read;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use fake_harness::model::{
    ErrorFixture, PeerClose, Provenance, Provider, Recording, TestRoot, Usage, error_fixtures,
};
use flate2::read::GzDecoder;
use purlis_core::clock::{Clock, InjectedClock, Monotonic};
use purlis_core::upstream::{Destination, Direct, Outbound, RecordingOutbound, TestReach, trust};
use serde_json::Value;
use support::{Connection, until};

/// A streamed Messages answer as Anthropic sends it, usage in `message_start` and the final
/// output count in `message_delta`.
const STREAMED: &str = concat!(
    "event: message_start\n",
    "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_rec\",\"type\":\"message\",",
    "\"role\":\"assistant\",\"model\":\"claude-sonnet-4-5\",\"content\":[],\"stop_reason\":null,",
    "\"stop_sequence\":null,\"usage\":{\"input_tokens\":12,\"output_tokens\":1}}}\n\n",
    "event: content_block_start\n",
    "data: {\"type\":\"content_block_start\",\"index\":0,",
    "\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":0,",
    "\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
    "event: content_block_stop\n",
    "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: message_delta\n",
    "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",",
    "\"stop_sequence\":null},\"usage\":{\"output_tokens\":34}}\n\n",
    "event: message_stop\n",
    "data: {\"type\":\"message_stop\"}\n\n",
);

/// The same answer, not streamed.
const WHOLE: &str = concat!(
    "{\"id\":\"msg_rec\",\"type\":\"message\",\"role\":\"assistant\",",
    "\"model\":\"claude-sonnet-4-5\",\"content\":[{\"type\":\"text\",\"text\":\"Hello\"}],",
    "\"stop_reason\":\"end_turn\",\"stop_sequence\":null,",
    "\"usage\":{\"input_tokens\":12,\"output_tokens\":34}}",
);

const ASKED: &[u8] = br#"{"model":"claude-sonnet-4-5","max_tokens":64,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#;

/// `count` chunks of `size` bytes each, every chunk's bytes its own index.
fn chunks(count: usize, size: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|i| vec![b'a' + (i % 26) as u8; size])
        .collect()
}

fn injected() -> (Arc<InjectedClock>, Arc<dyn Clock>) {
    let clock = Arc::new(InjectedClock::new());
    let shared: Arc<dyn Clock> = clock.clone();
    (clock, shared)
}

fn fixture(name: &str) -> ErrorFixture {
    error_fixtures()
        .into_iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no error fixture `{name}` in fixtures/errors.json"))
}

fn error_type(body: &[u8]) -> String {
    let body: Value = serde_json::from_slice(body).expect("a JSON error body");
    assert_eq!(body["type"], "error", "{body}");
    body["error"]["type"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn the_fake_provider_replays_a_streamed_recording_over_tls_under_a_test_root() {
    let root = TestRoot::generate(&["localhost"]);
    let provider = Provider::builder()
        .answer("/v1/messages", Recording::events(STREAMED))
        .tls(&root)
        .start()
        .unwrap();
    let stream = TcpStream::connect(provider.address()).unwrap();
    let config = trust::trusting_test_root(root.certificate()).unwrap();
    let mut connection = Connection::tls(stream, config, "localhost").unwrap();

    let answer = connection
        .post(
            "/v1/messages?beta=true",
            &[
                ("x-api-key", "sk-ant-stand-in"),
                ("anthropic-version", "2023-06-01"),
            ],
            ASKED,
        )
        .unwrap();

    assert_eq!(answer.head.status, 200);
    assert_eq!(
        answer.head.header("content-type"),
        Some("text/event-stream")
    );
    assert_eq!(String::from_utf8(answer.body).unwrap(), STREAMED);
    let requests = provider.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    let request = &requests[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/messages?beta=true");
    assert!(
        request
            .headers
            .contains(&("x-api-key".to_owned(), "sk-ant-stand-in".to_owned())),
        "{:?}",
        request.headers
    );
    assert_eq!(request.body, ASKED);
}

#[test]
fn the_fake_provider_replays_a_whole_recording_over_plain_http() {
    let provider = Provider::builder()
        .answer("/v1/messages", Recording::message(WHOLE))
        .start()
        .unwrap();
    let mut connection = Connection::plain(provider.address()).unwrap();

    let answer = connection.post("/v1/messages", &[], b"{}").unwrap();

    assert_eq!(answer.head.status, 200);
    assert_eq!(answer.head.header("content-type"), Some("application/json"));
    assert_eq!(answer.body, WHOLE.as_bytes());
    assert_eq!(provider.requests()[0].body, b"{}");
}

#[test]
fn the_fake_provider_reports_the_recorded_usage() {
    let recorded = Usage {
        input_tokens: 12,
        output_tokens: 34,
        ..Usage::default()
    };
    assert_eq!(Recording::events(STREAMED).usage(), recorded);
    assert_eq!(Recording::message(WHOLE).usage(), recorded);

    let provider = Provider::builder()
        .answer("/v1/messages", Recording::events(STREAMED))
        .answer("/v1/messages/whole", Recording::message(WHOLE))
        .start()
        .unwrap();
    let mut connection = Connection::plain(provider.address()).unwrap();
    connection.post("/v1/messages", &[], ASKED).unwrap();
    connection.post("/v1/messages/whole", &[], ASKED).unwrap();

    assert_eq!(provider.usage(), [recorded, recorded]);
}

#[test]
fn the_recording_outbound_trait_logs_each_destination() {
    let one = Provider::builder().start().unwrap();
    let two = Provider::builder().start().unwrap();
    let outbound = RecordingOutbound::new(Direct::new());
    let proxy = two.address().to_string();

    let direct = Destination::direct("127.0.0.1", one.port());
    let proxied = Destination::through("api.anthropic.com", 443, &proxy);
    let _first = outbound.connect(&direct).unwrap();
    let _second = outbound.connect(&proxied).unwrap();

    assert_eq!(outbound.destinations(), [direct, proxied]);
    assert_eq!(outbound.destinations()[0].proxy, None);
    assert_eq!(outbound.destinations()[1].proxy.as_deref(), Some(&*proxy));
    until("each origin to take its connection", || {
        one.accepts() == 1 && two.accepts() == 1
    });
}

#[test]
fn the_anthropic_origin_is_served_on_loopback_under_the_test_root() {
    let root = TestRoot::generate(&["api.anthropic.com"]);
    let provider = Provider::builder()
        .answer("/v1/messages", Recording::message(WHOLE))
        .tls(&root)
        .start()
        .unwrap();
    let outbound = Direct::with_test_reach(TestReach::new("api.anthropic.com", provider.address()));
    let anthropic = Destination::direct("api.anthropic.com", 443);

    let stream = outbound.connect(&anthropic).unwrap();
    let config = trust::trusting_test_root(root.certificate()).unwrap();
    let mut connection = Connection::tls(stream, config, "api.anthropic.com").unwrap();
    let answer = connection.post("/v1/messages", &[], ASKED).unwrap();
    assert_eq!(answer.body, WHOLE.as_bytes());
    assert_eq!(provider.requests().len(), 1);

    let stream = outbound.connect(&anthropic).unwrap();
    let refused = Connection::tls(stream, trust::client_config().unwrap(), "api.anthropic.com")
        .err()
        .expect("the handshake fails without the test root");
    assert!(
        refused.to_string().to_lowercase().contains("certificate"),
        "the handshake failed for another reason than verification: {refused}"
    );
    assert_eq!(
        provider.requests().len(),
        1,
        "nothing was sent without the root"
    );
}

#[test]
fn the_accept_counter_counts_each_new_connection() {
    let provider = Provider::builder()
        .answer("/v1/messages", Recording::message(WHOLE))
        .start()
        .unwrap();
    let mut kept = Connection::plain(provider.address()).unwrap();
    for _ in 0..3 {
        kept.post("/v1/messages", &[], ASKED).unwrap();
    }
    assert_eq!(provider.accepts(), 1);

    let mut fresh = Connection::plain(provider.address()).unwrap();
    fresh.post("/v1/messages", &[], ASKED).unwrap();
    assert_eq!(provider.accepts(), 2);
    assert_eq!(provider.requests().len(), 4);
}

#[test]
fn paced_streaming_records_each_chunk_send_time() {
    const CONNECTIONS: usize = 50;
    const CHUNKS: usize = 100;
    const SIZE: usize = 200;
    let (clock, shared) = injected();
    let provider = Provider::builder()
        .answer(
            "/v1/messages",
            Recording::chunked("text/event-stream", chunks(CHUNKS, SIZE)),
        )
        .clock(shared)
        .paced(100)
        .start()
        .unwrap();

    let clients: Vec<_> = (0..CONNECTIONS)
        .map(|_| {
            let address = provider.address();
            std::thread::spawn(move || {
                let mut connection = Connection::plain(address).unwrap();
                connection.send("POST", "/v1/messages", &[], ASKED).unwrap();
                let head = connection.head().unwrap();
                assert_eq!(head.header("transfer-encoding"), Some("chunked"));
                let mut got = Vec::new();
                while let Some(chunk) = connection.chunk().unwrap() {
                    got.push(chunk);
                }
                got
            })
        })
        .collect();
    for _ in 1..CHUNKS {
        until("every connection to wait for its next chunk", || {
            clock.sleepers() == CONNECTIONS
        });
        clock.advance(Duration::from_millis(10));
    }
    for client in clients {
        assert_eq!(client.join().unwrap(), chunks(CHUNKS, SIZE));
    }

    let sends = provider.sends();
    assert_eq!(sends.len(), CONNECTIONS * CHUNKS);
    for connection in 1..=CONNECTIONS {
        let mine: Vec<_> = sends
            .iter()
            .filter(|s| s.connection == connection)
            .collect();
        assert_eq!(mine.len(), CHUNKS, "connection {connection}");
        let first = mine[0].at;
        for (i, send) in mine.iter().enumerate() {
            assert_eq!(send.index, i, "connection {connection} sent out of order");
            assert_eq!(
                send.at - first,
                Duration::from_millis(10) * i as u32,
                "connection {connection}'s chunk {i} off the pace"
            );
        }
    }
}

#[test]
fn the_fake_provider_can_answer_gzip() {
    let provider = Provider::builder()
        .answer("/v1/messages", Recording::message(WHOLE).gzipped())
        .answer(
            "/v1/messages/streamed",
            Recording::events(STREAMED).gzipped(),
        )
        .start()
        .unwrap();
    let mut connection = Connection::plain(provider.address()).unwrap();

    for (path, recorded) in [("/v1/messages", WHOLE), ("/v1/messages/streamed", STREAMED)] {
        let answer = connection.post(path, &[], ASKED).unwrap();
        assert_eq!(
            answer.head.header("content-encoding"),
            Some("gzip"),
            "{path}"
        );
        assert_ne!(
            answer.body,
            recorded.as_bytes(),
            "{path} was sent as it was"
        );
        let mut inflated = String::new();
        GzDecoder::new(&answer.body[..])
            .read_to_string(&mut inflated)
            .unwrap();
        assert_eq!(inflated, recorded, "{path}");
    }
}

#[test]
fn a_slow_accept_delays_the_handshake_by_the_injected_amount() {
    let (clock, shared) = injected();
    let root = TestRoot::generate(&["localhost"]);
    let provider = Provider::builder()
        .tls(&root)
        .clock(shared)
        .accept_delay(Duration::from_secs(2))
        .start()
        .unwrap();
    let (done, finished) = mpsc::channel();
    let address = provider.address();
    let certificate = root.certificate();
    std::thread::spawn(move || {
        let stream = TcpStream::connect(address).unwrap();
        let config = trust::trusting_test_root(certificate).unwrap();
        let connected = Connection::tls(stream, config, "localhost").is_ok();
        let _ = done.send(connected);
    });

    until("the provider to hold the connection", || {
        provider.accepts() == 1 && clock.sleepers() == 1
    });
    clock.advance(Duration::from_millis(1_999));
    assert!(
        finished.recv_timeout(Duration::from_millis(200)).is_err(),
        "the handshake finished before the clock passed 2 s"
    );
    clock.advance(Duration::from_millis(1));
    assert_eq!(finished.recv_timeout(Duration::from_secs(30)), Ok(true));
}

#[test]
fn the_fake_provider_records_a_peer_close_mid_stream() {
    let (clock, shared) = injected();
    let provider = Provider::builder()
        .answer(
            "/v1/messages",
            Recording::chunked("text/event-stream", chunks(100, 200)),
        )
        .clock(shared)
        .paced(100)
        .start()
        .unwrap();
    let (closed, client_gone) = mpsc::channel();
    let address = provider.address();
    std::thread::spawn(move || {
        let mut connection = Connection::plain(address).unwrap();
        connection.send("POST", "/v1/messages", &[], ASKED).unwrap();
        connection.head().unwrap();
        for _ in 0..10 {
            connection.chunk().unwrap().expect("a chunk");
        }
        drop(connection);
        let _ = closed.send(());
    });

    for _ in 1..10 {
        until("the provider to wait for its next chunk", || {
            clock.sleepers() == 1
        });
        clock.advance(Duration::from_millis(10));
    }
    client_gone.recv_timeout(Duration::from_secs(30)).unwrap();
    for _ in 10..100 {
        until("the provider to wait or to see the close", || {
            clock.sleepers() == 1 || !provider.peer_closes().is_empty()
        });
        if !provider.peer_closes().is_empty() {
            break;
        }
        clock.advance(Duration::from_millis(10));
    }

    let closes = provider.peer_closes();
    assert_eq!(closes.len(), 1, "{closes:?}");
    let PeerClose { connection, sent } = closes[0];
    assert_eq!(connection, 1);
    assert!(
        (10..100).contains(&sent),
        "saw the close after {sent} chunks"
    );
    assert_eq!(
        provider.sends().len(),
        sent,
        "it kept sending after the close"
    );
}

#[test]
fn the_401_fixture_carries_the_recorded_real_headers() {
    let unauthorised = fixture("401");
    let Provenance::Recorded { source } = &unauthorised.provenance else {
        panic!("the 401 fixture is not marked recorded");
    };
    assert!(!source.is_empty(), "the 401 fixture names no recording");
    // Anthropic's real 401 as T1a-02 recorded it at api.anthropic.com (spike 1a-startup,
    // 2026-11-15), in the order received. Identifiers stay redacted as the record has them;
    // content-length is the provider's own, and content-type is the body's type.
    let recorded = [
        ("content-type", "application/json"),
        ("date", "Sun, 15 Nov 2026 20:52:05 GMT"),
        ("server-timing", "x-originResponse;dur="),
        ("cache-control", "no-store"),
        ("request-id", "req_<redacted>"),
        ("x-should-retry", "false"),
        ("server", "cloudflare"),
        ("cf-cache-status", "DYNAMIC"),
        ("x-robots-tag", "none"),
        (
            "content-security-policy",
            "default-src 'none'; frame-ancestors 'none'",
        ),
        ("cf-ray", "<redacted>-SOF"),
    ];
    let carried: Vec<(&str, &str)> = unauthorised
        .recording
        .headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    assert_eq!(
        carried, recorded,
        "the 401 fixture is not the recorded header set"
    );
    assert!(
        !source.contains("waits"),
        "the 401 fixture still says its headers wait for a record: {source}"
    );
    let provider = Provider::builder()
        .answer("/v1/messages", unauthorised.recording.clone())
        .start()
        .unwrap();

    let answer = Connection::plain(provider.address())
        .unwrap()
        .post("/v1/messages", &[], ASKED)
        .unwrap();

    assert_eq!(answer.head.status, 401);
    assert_eq!(answer.head.header("x-should-retry"), Some("false"));
    for (name, value) in &unauthorised.recording.headers {
        let line = format!("\r\n{name}: {value}\r\n");
        assert!(
            answer.head.raw.contains(&line),
            "the recorded header `{name}: {value}` was not sent byte for byte:\n{}",
            answer.head.raw
        );
    }
    assert_eq!(error_type(&answer.body), "authentication_error");

    // The shapes no one has recorded yet say so, until a recording stands beside them.
    for name in ["403", "429", "529"] {
        match fixture(name).provenance {
            Provenance::Unrecorded => {}
            Provenance::Recorded { source } if !source.is_empty() => {}
            other => panic!("the {name} fixture is {other:?}: mark it unrecorded or record it"),
        }
    }
}

#[test]
fn the_documented_fixtures_are_marked_unrecorded() {
    let expected = [
        ("400-spend-limit", 400, Some("invalid_request_error")),
        ("402", 402, Some("billing_error")),
        ("429-spend-cap", 429, Some("rate_limit_error")),
        ("413", 413, None),
    ];
    let mut builder = Provider::builder();
    for (name, _, _) in expected {
        let documented = fixture(name);
        assert_eq!(
            documented.provenance,
            Provenance::FromTheDocs,
            "the {name} fixture is not marked \"from the docs, unrecorded\""
        );
        builder = builder.answer(&format!("/{name}"), documented.recording);
    }
    let provider = builder.start().unwrap();
    let mut connection = Connection::plain(provider.address()).unwrap();

    for (name, status, kind) in expected {
        let answer = connection.post(&format!("/{name}"), &[], ASKED).unwrap();
        assert_eq!(answer.head.status, status, "{name}");
        match kind {
            Some(kind) => assert_eq!(error_type(&answer.body), kind, "{name}"),
            None => {
                assert_eq!(
                    answer.head.header("content-type"),
                    Some("text/plain"),
                    "{name}"
                );
                assert!(
                    serde_json::from_slice::<Value>(&answer.body).is_err(),
                    "the {name} body is JSON"
                );
            }
        }
    }
    let spend_cap = connection.post("/429-spend-cap", &[], ASKED).unwrap();
    assert_eq!(spend_cap.head.header("retry-after"), None);
    let cap: Value = serde_json::from_slice(&spend_cap.body).unwrap();
    assert_eq!(
        cap["error"]["details"]["error_code"], "enforced_spend_limit_reached",
        "{cap}"
    );
    assert!(
        cap["error"]["message"]
            .as_str()
            .is_some_and(|m| m.starts_with("You have reached your API usage limits:")),
        "{cap}"
    );
    let spend_limit = connection.post("/400-spend-limit", &[], ASKED).unwrap();
    let message: Value = serde_json::from_slice(&spend_limit.body).unwrap();
    assert!(
        message["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("usage limits")),
        "{message}"
    );
}

#[test]
fn paced_streaming_takes_either_clock() {
    let recording = Recording::chunked("text/event-stream", chunks(10, 200));
    let run = |clock: Arc<dyn Clock>, step: &dyn Fn()| {
        let provider = Provider::builder()
            .answer("/v1/messages", recording.clone())
            .clock(clock)
            .paced(100)
            .start()
            .unwrap();
        let address = provider.address();
        let client = std::thread::spawn(move || {
            Connection::plain(address)
                .unwrap()
                .post("/v1/messages", &[], ASKED)
                .unwrap()
        });
        step();
        client.join().unwrap();
        provider.sends()
    };

    let (clock, shared) = injected();
    let sends = run(shared, &|| {
        for _ in 1..10 {
            until("the provider to wait for its next chunk", || {
                clock.sleepers() == 1
            });
            clock.advance(Duration::from_millis(10));
        }
    });
    assert_eq!(sends.len(), 10);
    for (i, send) in sends.iter().enumerate() {
        assert_eq!(send.at - sends[0].at, Duration::from_millis(10) * i as u32);
    }

    let sends = run(Arc::new(Monotonic), &|| {});
    assert_eq!(sends.len(), 10);
    for (i, send) in sends.iter().enumerate() {
        let since = send.at - sends[0].at;
        let paced = Duration::from_millis(10) * i as u32;
        // Wall time: never early, and late by at most a wide margin.
        assert!(since >= paced, "chunk {i} went early: {since:?}");
        assert!(
            since < paced + Duration::from_secs(2),
            "chunk {i} went late: {since:?}"
        );
    }
}
