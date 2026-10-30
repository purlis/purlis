// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::net::Ipv4Addr;

use super::*;

/// The kernel's `/proc/net/tcp` writes an IPv4 address as the 32-bit number it holds, in this
/// machine's byte order: 127.0.0.1 is `0100007F` on a little-endian machine.
fn hex(ip: Ipv4Addr) -> String {
    format!("{:08X}", u32::from_ne_bytes(ip.octets()))
}

/// A `/proc/net/tcp` table with each `(address, port, state, inode)` on a line of its own.
fn table(rows: &[(Ipv4Addr, u16, &str, u64)]) -> String {
    let mut out = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   \
                   uid  timeout inode\n"
        .to_owned();
    for (n, (ip, port, state, inode)) in rows.iter().enumerate() {
        out.push_str(&format!(
            "   {n}: {}:{port:04X} 00000000:0000 {state} 00000000:00000000 00:00000000 00000000  \
             1000        0 {inode} 1 0000000000000000 100 0 0 10 0\n",
            hex(*ip)
        ));
    }
    out
}

const LISTEN: &str = "0A";
const ESTABLISHED: &str = "01";

#[test]
fn only_a_port_the_system_picks_on_127_0_0_1_is_picked_on_the_loopback() {
    for yes in ["127.0.0.1:0"] {
        assert!(
            picks_on_the_loopback(yes.parse().expect("an address")),
            "{yes}"
        );
    }
    for no in [
        "127.0.0.1:3000",
        "0.0.0.0:0",
        "[::]:0",
        "[::1]:0",
        "127.0.0.2:0",
        "[::ffff:127.0.0.1]:0",
        "192.168.1.5:0",
    ] {
        assert!(
            !picks_on_the_loopback(no.parse().expect("an address")),
            "{no}"
        );
    }
}

#[test]
fn the_sockets_listening_at_an_address_are_read_from_the_kernel_s_table() {
    let lo = Ipv4Addr::LOCALHOST;
    let rows = table(&[
        (lo, 40_001, LISTEN, 11),
        (lo, 40_001, ESTABLISHED, 12),
        (lo, 40_002, LISTEN, 13),
        (Ipv4Addr::UNSPECIFIED, 40_001, LISTEN, 14),
        (lo, 40_001, LISTEN, 15),
    ]);
    assert_eq!(listening_at(&rows, lo, 40_001), vec![11, 15]);
    assert_eq!(listening_at(&rows, lo, 40_002), vec![13]);
    assert_eq!(listening_at(&rows, lo, 40_003), Vec::<u64>::new());
    assert_eq!(listening_at("", lo, 40_001), Vec::<u64>::new());
    assert_eq!(
        listening_at("garbage\n1: x y z\n", lo, 40_001),
        Vec::<u64>::new()
    );
}

#[test]
fn a_port_is_the_chat_s_own_only_while_the_socket_made_for_it_alone_listens_there() {
    let lo = Ipv4Addr::LOCALHOST;
    let picked = Picked::new(vec![40_001, 40_002]);
    // Never made for the chat: another service's, though it listens on a picked port.
    assert!(!picked.owns(40_002, &table(&[(lo, 40_002, LISTEN, 21)])));
    picked.own(40_001, 11);
    assert!(picked.owns(40_001, &table(&[(lo, 40_001, LISTEN, 11)])));
    // Not listening yet, or no longer: nothing to reach.
    assert!(!picked.owns(40_001, &table(&[(lo, 40_001, ESTABLISHED, 11)])));
    assert!(!picked.owns(40_001, &table(&[])));
    // Closed, and the port taken since by another socket.
    assert!(!picked.owns(40_001, &table(&[(lo, 40_001, LISTEN, 99)])));
    // Shared with another listener (a reuse-port group): a connect could reach either.
    assert!(!picked.owns(
        40_001,
        &table(&[(lo, 40_001, LISTEN, 11), (lo, 40_001, LISTEN, 99)])
    ));
    // A port outside the pool is never the chat's, whatever was recorded.
    picked.own(5432, 31);
    assert!(!picked.owns(5432, &table(&[(lo, 5432, LISTEN, 31)])));
}

#[test]
fn each_bind_tries_every_port_of_the_pool_starting_after_the_last_one_tried() {
    let picked = Picked::new(vec![1, 2, 3]);
    assert_eq!(picked.ports(), [1, 2, 3]);
    assert_eq!(picked.turn(), vec![1, 2, 3]);
    assert_eq!(picked.turn(), vec![2, 3, 1]);
    assert_eq!(picked.turn(), vec![3, 1, 2]);
    assert_eq!(picked.turn(), vec![1, 2, 3]);
    assert_eq!(Picked::new(Vec::new()).turn(), Vec::<u16>::new());
}
