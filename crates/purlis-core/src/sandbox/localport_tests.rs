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
    assert_eq!(still_ports(&[taken]), vec![taken]);
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

/// A start and a new listener of purlis's judge and mark one table in one hold of its lock, so
/// a port is either a chat's or purlis's, never both, whichever comes first.
#[test]
fn a_port_is_a_chats_or_purlis_s_never_both_whichever_comes_first() {
    let mut first_a_chat = Ports::default();
    assert_eq!(first_a_chat.admit(&[50795]), vec![50795]);
    assert!(
        !first_a_chat.claim(50795),
        "purlis never takes a port a chat was given"
    );

    let mut first_purlis = Ports::default();
    assert!(first_purlis.claim(50795));
    assert_eq!(
        first_purlis.admit(&[50795, 6443]),
        vec![6443],
        "a chat is never given a port purlis holds"
    );

    // Concurrently, through the one table: every port ends up with one side alone.
    let shared = std::sync::Arc::new(Mutex::new(Ports::default()));
    let mut handles = Vec::new();
    for at in 0..8u16 {
        let shared = std::sync::Arc::clone(&shared);
        handles.push(std::thread::spawn(move || {
            for port in 40000..40200u16 {
                if (port + at) % 2 == 0 {
                    shared.lock().unwrap().admit(&[port]);
                } else {
                    shared.lock().unwrap().claim(port);
                }
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    let table = shared.lock().unwrap();
    assert!(table.own.is_disjoint(&table.given));
}

#[test]
fn a_forge_sign_ins_redirect_ports_are_purlis_s_own_always() {
    for redirect in crate::forge::registration::REDIRECT_PORTS {
        assert!(is_own(redirect));
        assert!(matches!(
            port(&redirect.to_string()),
            Err(Refused::Never(_))
        ));
        assert!(still_ports(&[redirect]).is_empty());
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
    assert_eq!(loopback_port("[::1]:50795"), Some(50795));
    assert_eq!(loopback_port("localhost:3000"), Some(3000));
    for never in [
        // Only the addresses the profile and the proxy carry.
        "127.0.0.2:6443",
        "[::ffff:127.0.0.1]:6443",
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

/// Review of #1966: a port purlis holds cannot take your service until purlis lets it go, so
/// the sentence says to free it first, then start the service, then restart this chat.
#[test]
fn a_port_purlis_took_is_freed_before_the_service_is_started_again() {
    let free = TAKEN_SAID
        .find("close the chat")
        .expect("says how to free the port");
    let quit = TAKEN_SAID
        .find("quit and reopen purlis")
        .expect("or to reopen purlis");
    let start = TAKEN_SAID
        .find("start your service")
        .expect("then to start the service");
    let restart = TAKEN_SAID
        .find("restart this chat")
        .expect("then to restart this chat");
    assert!(
        free < start && quit < start && start < restart,
        "{TAKEN_SAID}"
    );
}

/// qw384b: a name of this Mac names its port too, from the fixed list alone; no name is looked
/// up (#2097 review).
#[test]
fn a_name_of_this_mac_names_its_port_and_no_other_name_does() {
    assert_eq!(loopback_port("this-mac.internal:50795"), Some(50795));
    assert_eq!(loopback_port("THIS-MAC.internal.:6443"), Some(6443));
    assert_eq!(named_port("kubernetes.docker.internal:6443"), Some(6443));
    assert_eq!(named_port("host.docker.internal:8080"), Some(8080));
    assert_eq!(named_port("this-mac.internal:1"), Some(1));
    assert_eq!(named_port("127.0.0.1:2"), Some(2));
    for never in [
        "example.com:6443",
        "evil.example:6443",
        "kubernetes.docker.internal.example.com:6443",
        "kubernetes.docker.internal",
        "kubernetes.docker.internal:0",
    ] {
        assert_eq!(named_port(never), None, "{never}");
    }
}
