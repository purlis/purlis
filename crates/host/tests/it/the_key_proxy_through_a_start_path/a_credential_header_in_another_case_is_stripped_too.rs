// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-097: a credential header in other letters is stripped too. The harness writes
//! `AUTHORIZATION` (its token) and `X-Api-Key` (a key of its own) as raw bytes, on a route whose
//! declaration puts the key in `Authorization: Bearer`; upstream, no `x-api-key` arrives in any
//! case, and the one `authorization` carries the stand-in key, not the token.

use purlis_key_proxy::{Key, Reach, Route};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::StartPath;
use super::start_path::{
    Desktop, PATIENCE, STAND_IN_KEY, headers_named, plain_reach, provider, route_to,
};

pub(crate) async fn check<P: StartPath>(start: impl AsyncFn(Reach) -> P) {
    let path = start(plain_reach()).await;
    let provider = provider();
    let route = Route {
        key: Key::Bearer(STAND_IN_KEY.to_owned()),
        ..route_to(&provider)
    };
    let (port, token) = path.start("run-1", route).await;

    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("the listener accepts a connection");
    let request = format!(
        "POST /v1/messages HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAUTHORIZATION: Bearer {token}\r\n\
         X-Api-Key: the-harness-s-own\r\nContent-Type: application/json\r\n\
         Content-Length: 2\r\nConnection: close\r\n\r\n{{}}"
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = Vec::new();
    tokio::time::timeout(PATIENCE, stream.read_to_end(&mut answer))
        .await
        .expect("answered in time")
        .unwrap();
    let answer = String::from_utf8_lossy(&answer);
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");

    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    assert!(
        headers_named(&requests[0], "x-api-key").is_empty(),
        "the harness's X-Api-Key reached the provider: {:?}",
        requests[0].headers
    );
    assert_eq!(
        headers_named(&requests[0], "authorization"),
        [format!("Bearer {STAND_IN_KEY}").as_str()],
        "the token, or more than the key, reached the provider"
    );
}

#[tokio::test]
async fn a_credential_header_in_another_case_is_stripped_too() {
    check(Desktop::reaching).await;
}
