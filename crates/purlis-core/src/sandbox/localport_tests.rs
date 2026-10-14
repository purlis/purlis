// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A service on this Mac (qw384): one port, never purlis's own, read from the refusals that
//! name one.

use super::*;

#[test]
fn a_port_is_one_whole_number_from_1_to_65535() {
    assert_eq!(port("50795"), Ok(50795));
    assert_eq!(port(" :8080 "), Ok(8080));
    for never in [
        "0",
        "65536",
        "80-90",
        "*",
        "",
        "localhost:80",
        "eighty",
        "-1",
    ] {
        assert!(
            matches!(port(never), Err(Refused::Not(_))),
            "{never:?} is no port of one service"
        );
    }
}

#[test]
fn purlis_own_listener_is_never_a_port_to_allow_and_is_left_out_at_a_start() {
    let listener = bind_loopback().expect("a loopback port");
    let own_port = listener.local_addr().expect("bound").port();
    assert!(is_own(own_port));
    assert!(matches!(
        port(&own_port.to_string()),
        Err(Refused::Never(_))
    ));
    assert_eq!(still_ports(&[own_port, 50795, 50795]), vec![50795]);
    release(own_port);
    assert!(!is_own(own_port));
}

#[test]
fn a_port_given_to_a_chat_is_never_taken_by_purlis_later() {
    // Take a port the system just handed out, give it to a chat, and free it: purlis's next
    // listeners never land on it.
    let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a loopback port");
    let taken = probe.local_addr().expect("bound").port();
    given(&[taken]);
    drop(probe);
    let mut ours = Vec::new();
    for _ in 0..8 {
        let listener = bind_loopback().expect("a loopback port");
        assert_ne!(listener.local_addr().expect("bound").port(), taken);
        ours.push(listener);
    }
    for listener in ours {
        release(listener.local_addr().expect("bound").port());
    }
}

#[test]
fn the_proxy_lists_this_machine_by_each_name_at_that_port_alone() {
    assert_eq!(
        reach_entries(50795),
        [
            "127.0.0.1:50795".to_owned(),
            "[::1]:50795".to_owned(),
            "localhost:50795".to_owned()
        ]
    );
    assert_eq!(
        seatbelt_rule(50795),
        "(allow network-outbound (remote ip \"localhost:50795\"))"
    );
}

#[test]
fn a_loopback_target_names_its_port_and_nothing_else_does() {
    assert_eq!(loopback_port("127.0.0.1:50795"), Some(50795));
    assert_eq!(loopback_port("127.0.0.2:6443"), Some(6443));
    assert_eq!(loopback_port("[::1]:50795"), Some(50795));
    assert_eq!(loopback_port("localhost:3000"), Some(3000));
    for never in [
        "10.0.0.1:443",
        "169.254.169.254:80",
        "example.com:443",
        "127.0.0.1",
        "127.0.0.1:0",
        "/var/run/docker.sock",
    ] {
        assert_eq!(loopback_port(never), None, "{never}");
    }
}

#[test]
fn go_and_node_refusals_of_this_machine_name_the_port() {
    assert_eq!(
        refused_in(
            "Unable to connect to the server: dial tcp 127.0.0.1:50795: connect: operation not \
             permitted"
        ),
        Some(50795)
    );
    assert_eq!(
        refused_in("E1014 dial tcp [::1]:6443: connect: operation not permitted"),
        Some(6443)
    );
    assert_eq!(
        refused_in("Error: connect EPERM 127.0.0.1:5432 - Local (0.0.0.0:0)"),
        Some(5432)
    );
    // Another host, a refused socket, a closed port: none of them a service on this Mac.
    for never in [
        "dial tcp 10.1.2.3:443: connect: operation not permitted",
        "dial unix /var/run/docker.sock: connect: operation not permitted",
        "dial tcp 127.0.0.1:50795: connect: connection refused",
    ] {
        assert_eq!(refused_in(never), None, "{never}");
    }
}
