// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-064: a request without the run's token is refused and never ends the run: the next one,
//! with the token, reaches the fake provider, the stand-in key in the token's place.

use super::StartPath;
use super::start_path::{Desktop, STAND_IN_KEY, header, provider, route_to, send};

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let (port, token) = path.start("run-1", route_to(&provider)).await;

    let refused = send(port, None, "/v1/messages", b"{}").await;
    assert_eq!(
        refused.status, 400,
        "a request without the token was served"
    );
    assert!(
        provider.requests().is_empty(),
        "a request without the token reached the provider"
    );

    let served = send(port, Some(&token), "/v1/messages", b"{}").await;
    assert_eq!(served.status, 200, "the run did not go on: {served:?}");
    let requests = provider.requests();
    assert_eq!(
        requests.len(),
        1,
        "the request with the token did not reach the provider"
    );
    assert_eq!(header(&requests[0], "x-api-key"), Some(STAND_IN_KEY));
    assert!(
        requests[0]
            .headers
            .iter()
            .all(|(_, value)| !value.contains(&token)),
        "the token reached the provider: {:?}",
        requests[0].headers
    );
}

#[tokio::test]
async fn a_tokenless_request_is_refused_and_the_run_goes_on() {
    check(Desktop::new().await).await;
}
