// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-056: no proxy variables (a PAC-only machine reads as none), and the direct connect refused.
//! The harness gets a 502 whose `purlis:` message names the connect failure.

use purlis_key_proxy::{Matcher, Reach};

use super::StartPath;
use super::start_path::{Desktop, UPSTREAM, message, reach, route_to_upstream, send};

pub(crate) async fn check<P: StartPath>(start: impl AsyncFn(Reach) -> P) {
    let closed = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    };
    let path = start(reach(closed, None, Matcher::builder().build())).await;
    let (port, token) = path.start("run-1", route_to_upstream()).await;
    let refused = send(port, Some(&token), "/v1/messages", b"{}").await;
    assert_eq!(refused.status, 502, "{refused:?}");
    assert_eq!(
        message(&refused),
        format!("purlis: purlis couldn't connect to {UPSTREAM}.")
    );
}

#[tokio::test]
async fn no_proxy_and_a_refused_direct_connect_gives_a_502() {
    check(Desktop::reaching).await;
}
