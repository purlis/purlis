// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-056: with `HTTPS_PROXY` naming the recording outbound proxy, the connection is a `CONNECT`
//! to the upstream, and TLS inside it verifies against the declared name; with `NO_PROXY`
//! naming the upstream, it goes direct. The proxy variables are `purlisd`'s, read by hyper-util's
//! matcher (`Matcher::from_system()` in a build that ships); here the same matcher is built from
//! the values a test would have set, since a test can't set its own process's environment.

use purlis_key_proxy::{Matcher, Reach};

use super::StartPath;
use super::outbound_proxy::{Answers, OutboundProxy};
use super::start_path::{Desktop, UPSTREAM, reach, route_to_upstream, send, tls_provider};

pub(crate) async fn check<P: StartPath>(start: impl AsyncFn(Reach) -> P) {
    let (provider, root) = tls_provider();
    let proxy = OutboundProxy::start(Answers::TunnelTo(provider.address()));

    let proxied = Matcher::builder().https(proxy.url()).build();
    let path = start(reach(provider.address(), Some(&root), proxied)).await;
    let (port, token) = path.start("run-1", route_to_upstream()).await;
    let served = send(port, Some(&token), "/v1/messages", b"{}").await;
    assert_eq!(served.status, 200, "{served:?}");
    let heads = proxy.heads();
    assert_eq!(heads.len(), 1, "{heads:?}");
    assert!(
        heads[0].starts_with(&format!("CONNECT {UPSTREAM}:443 HTTP/1.1\r\n")),
        "not a CONNECT to the upstream: {heads:?}"
    );
    assert_eq!(provider.requests().len(), 1);

    let exempt = Matcher::builder().https(proxy.url()).no(UPSTREAM).build();
    let path = start(reach(provider.address(), Some(&root), exempt)).await;
    let (port, token) = path.start("run-2", route_to_upstream()).await;
    let served = send(port, Some(&token), "/v1/messages", b"{}").await;
    assert_eq!(served.status, 200, "{served:?}");
    assert_eq!(
        proxy.heads().len(),
        1,
        "NO_PROXY's upstream went through the proxy"
    );
    assert_eq!(provider.requests().len(), 2);
}

#[tokio::test]
async fn the_system_proxy_is_used_as_a_connect_tunnel() {
    check(Desktop::reaching).await;
}
