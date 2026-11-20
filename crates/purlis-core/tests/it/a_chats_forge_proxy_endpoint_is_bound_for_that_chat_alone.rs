// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A: a chat's endpoint is its own, and a connection's chat is the endpoint's,
//! never anything the connection says (ADR 0128 D2).

use purlis_core::forge::proxy::{
    ChatEndpoint, ChatId, EndpointKind, LoopbackEndpoint, pick_endpoint,
};

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixStream;

    use purlis_core::forge::proxy::UnixEndpoint;

    #[test]
    fn a_unix_socket_is_private_to_the_app_and_names_its_chat_by_the_endpoint() {
        purlis_core::unsteered!();
        let dir = tempfile::tempdir().unwrap();
        let sockets = dir.path().join("s");
        let a = UnixEndpoint::bind(&ChatId::new("chat-a"), &sockets).unwrap();
        let b = UnixEndpoint::bind(&ChatId::new("chat-b"), &sockets).unwrap();
        let (EndpointKind::UnixSocket(pa), EndpointKind::UnixSocket(pb)) = (a.kind(), b.kind())
        else {
            panic!("not unix sockets");
        };
        assert_ne!(pa, pb, "each chat has a socket of its own");
        assert_eq!(
            std::fs::metadata(&sockets).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&pa).unwrap().permissions().mode() & 0o777,
            0o600
        );

        // A client of chat-b's socket is chat-b, whatever it writes to it.
        let mut client = UnixStream::connect(&pb).unwrap();
        std::io::Write::write_all(&mut client, b"X-Chat: chat-a\r\n").unwrap();
        let (chat, _) = b.accept().unwrap();
        assert_eq!(chat, ChatId::new("chat-b"));
        let _client_a = UnixStream::connect(&pa).unwrap();
        assert_eq!(a.accept().unwrap().0, ChatId::new("chat-a"));
    }

    #[test]
    fn a_socket_is_removed_with_its_endpoint() {
        purlis_core::unsteered!();
        let dir = tempfile::tempdir().unwrap();
        let endpoint = UnixEndpoint::bind(&ChatId::new("c"), dir.path()).unwrap();
        let EndpointKind::UnixSocket(path) = endpoint.kind() else {
            panic!()
        };
        assert!(path.exists());
        drop(endpoint);
        assert!(!path.exists());
    }

    #[test]
    fn a_unix_socket_is_picked_where_the_client_can_use_one() {
        purlis_core::unsteered!();
        let dir = tempfile::tempdir().unwrap();
        let endpoint = pick_endpoint(&ChatId::new("c"), dir.path(), true).unwrap();
        assert!(matches!(endpoint.kind(), EndpointKind::UnixSocket(_)));
    }
}

#[test]
fn a_loopback_port_is_on_the_loopback_address_only_and_names_its_chat_by_the_endpoint() {
    purlis_core::unsteered!();
    let a = LoopbackEndpoint::bind(&ChatId::new("chat-a")).unwrap();
    let b = LoopbackEndpoint::bind(&ChatId::new("chat-b")).unwrap();
    let (EndpointKind::Loopback(addr_a), EndpointKind::Loopback(addr_b)) = (a.kind(), b.kind())
    else {
        panic!("not loopback");
    };
    assert!(addr_a.ip().is_loopback() && !addr_a.ip().is_unspecified());
    assert_ne!(
        addr_a.port(),
        addr_b.port(),
        "each chat has a port of its own"
    );
    let _client = std::net::TcpStream::connect(addr_b).unwrap();
    assert_eq!(b.accept().unwrap().0, ChatId::new("chat-b"));
}

#[test]
fn a_loopback_port_is_picked_where_the_client_cannot_use_a_unix_socket() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let endpoint = pick_endpoint(&ChatId::new("c"), dir.path(), false).unwrap();
    assert!(matches!(endpoint.kind(), EndpointKind::Loopback(_)));
    assert_eq!(endpoint.chat(), &ChatId::new("c"));
}
