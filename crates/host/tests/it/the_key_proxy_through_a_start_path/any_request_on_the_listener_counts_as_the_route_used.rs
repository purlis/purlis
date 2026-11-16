// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-037, spec C-19 (U4): any request on the listener, token-less included, raises the "a request
//! arrived" signal T1a-42's flag reads: a token-less `HEAD /api/hello` alone raises it.

use super::StartPath;
use super::start_path::{provider, raw, route_to};

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let (port, _) = path.start("run-1", route_to(&provider)).await;
    assert_eq!(path.arrivals("run-1"), 0, "nothing has arrived yet");
    assert_eq!(raw(port, "HEAD", "/api/hello", None).await.status, 404);
    assert_eq!(
        path.arrivals("run-1"),
        1,
        "the hello did not count as the route used"
    );
    assert_eq!(raw(port, "GET", "/nowhere", None).await.status, 400);
    assert_eq!(path.arrivals("run-1"), 2, "a refused request did not count");
}

#[tokio::test]
async fn any_request_on_the_listener_counts_as_the_route_used() {
    check(super::start_path::Desktop::new().await).await;
}
