// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-059 (D6640-C2 §1): the key proxy's parser is hyper 1's own `http1` server connection, with
//! no framing parser of purlis's own. What the verdict relies on, seen at the crate's seam: a
//! request whose two `Content-Length` headers disagree is refused by the connection, 400 and
//! closed, before the service ever runs; and one carrying both length headers is framed by
//! `Transfer-Encoding` alone, its connection closed after the answer.

use purlis_key_proxy::KeyProxy;

use crate::{Told, exchange, status};

#[tokio::test]
async fn the_crate_serves_on_hypers_own_http1_connection() {
    let proxy = KeyProxy::new().expect("its runtime starts");
    let told = Told::default();
    let started = proxy
        .start("run-1", None, told.heard())
        .expect("a listener starts");

    let answer = exchange(
        started.port,
        b"POST /v1/messages HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 2\r\n\
          Content-Length: 3\r\n\r\n{}",
    )
    .await;
    assert_eq!(status(&answer), 400, "answered {answer:?}");
    assert!(
        told.records.lock().unwrap().is_empty(),
        "the service ran for a request the connection should have refused"
    );

    // Both length headers: chunked wins, and the connection closes after the answer, so a
    // request pipelined behind it is never served.
    let answer = exchange(
        started.port,
        b"POST /v1/messages HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 2\r\n\
          Transfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n\
          GET /second HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    )
    .await;
    assert!(
        answer.to_ascii_lowercase().contains("connection: close"),
        "the connection is closed after a request with both length headers: {answer:?}"
    );
    let records = told.records.lock().unwrap().clone();
    let paths: Vec<_> = records.iter().map(|(r, _, _)| r.path.as_str()).collect();
    assert_eq!(paths, ["/v1/messages"], "the pipelined request was served");
}
