// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! R-063: a run's listener is on `127.0.0.1` only, never `::1`, `0.0.0.0` or another
//! interface: a connect to `[::1]:<port>` and to this machine's non-loopback address on the
//! same port both fail, and `127.0.0.1:<port>` accepts.

use std::net::{IpAddr, Ipv6Addr, SocketAddr, TcpStream, UdpSocket};
use std::time::Duration;

use super::StartPath;
use super::start_path::{Desktop, provider, route_to};

/// This machine's address on its default route, never a loopback one: what a socket sending
/// toward a documentation address (RFC 5737, nothing is sent) would leave from.
fn non_loopback_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let address = socket.local_addr().ok()?.ip();
    (!address.is_loopback() && !address.is_unspecified()).then_some(address)
}

fn connects(to: SocketAddr) -> bool {
    TcpStream::connect_timeout(&to, Duration::from_secs(2)).is_ok()
}

pub(crate) async fn check(path: impl StartPath) {
    let provider = provider();
    let (port, _) = path.start("run-1", route_to(&provider)).await;

    assert!(
        connects(SocketAddr::from(([127, 0, 0, 1], port))),
        "127.0.0.1:{port} does not accept"
    );
    assert!(
        !connects(SocketAddr::from((Ipv6Addr::LOCALHOST, port))),
        "[::1]:{port} accepts: the listener is not on 127.0.0.1 alone"
    );
    match non_loopback_address() {
        Some(address) => assert!(
            !connects(SocketAddr::new(address, port)),
            "{address}:{port} accepts: the listener is not on 127.0.0.1 alone"
        ),
        None => eprintln!("this machine has no non-loopback address to try; checked ::1 alone"),
    }
}

#[tokio::test]
async fn the_listener_is_bound_to_ipv4_loopback_only() {
    check(Desktop::new().await).await;
}
