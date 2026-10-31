// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::*;

/// The kernel's `/proc/net/tcp` and `/proc/net/tcp6` write an address as the 32-bit numbers it
/// holds, each in this machine's byte order: 127.0.0.1 is `0100007F` on a little-endian machine.
fn hex(ip: IpAddr) -> String {
    let octets = match ip {
        IpAddr::V4(v4) => v4.octets().to_vec(),
        IpAddr::V6(v6) => v6.octets().to_vec(),
    };
    octets
        .chunks(4)
        .map(|word| {
            format!(
                "{:08X}",
                u32::from_ne_bytes([word[0], word[1], word[2], word[3]])
            )
        })
        .collect()
}

/// A table as the kernel writes one, with each `(address, port, state, inode)` on a line of its
/// own.
fn table(rows: &[(IpAddr, u16, &str, u64)]) -> String {
    let mut out = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   \
                   uid  timeout inode\n"
        .to_owned();
    for (n, (ip, port, state, inode)) in rows.iter().enumerate() {
        let remote = "0".repeat(hex(*ip).len());
        out.push_str(&format!(
            "   {n}: {}:{port:04X} {remote}:0000 {state} 00000000:00000000 00:00000000 00000000  \
             1000        0 {inode} 1 0000000000000000 100 0 0 10 0\n",
            hex(*ip)
        ));
    }
    out
}

const LISTEN: &str = "0A";
const ESTABLISHED: &str = "01";
const LO: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const EVERY: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
const EVERY6: IpAddr = IpAddr::V6(Ipv6Addr::UNSPECIFIED);

fn mapped_lo() -> IpAddr {
    IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped())
}

#[test]
fn only_a_port_the_system_picks_on_127_0_0_1_is_picked_on_the_loopback() {
    assert!(picks_on_the_loopback(
        "127.0.0.1:0".parse().expect("an address")
    ));
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
fn the_sockets_listening_on_a_port_are_read_from_both_of_the_kernel_s_tables() {
    let v4 = table(&[
        (LO, 40_001, LISTEN, 11),
        (LO, 40_001, ESTABLISHED, 12),
        (LO, 40_002, LISTEN, 13),
        (EVERY, 40_001, LISTEN, 14),
    ]);
    let v6 = table(&[
        (mapped_lo(), 40_001, LISTEN, 15),
        (EVERY6, 40_002, ESTABLISHED, 16),
    ]);
    let both = format!("{v4}{v6}");
    assert_eq!(
        listening(&both, 40_001),
        vec![(LO, 11), (EVERY, 14), (mapped_lo(), 15)]
    );
    assert_eq!(listening(&both, 40_002), vec![(LO, 13)]);
    assert_eq!(listening(&both, 40_003), Vec::new());
    assert_eq!(listening("", 40_001), Vec::new());
    assert_eq!(listening("garbage\n1: x y z\n", 40_001), Vec::new());
}

#[test]
fn a_port_is_the_chat_s_own_only_while_the_socket_made_for_it_alone_listens_there() {
    let picked = Picked::new(vec![40_001, 40_002]);
    // A port outside the pool is no business of the pool's, whatever listens there.
    assert_eq!(
        picked.reach(5432, &table(&[(LO, 5432, LISTEN, 31)])),
        PoolPort::NotPicked
    );
    // Never made for the chat: another service's, though it listens on a picked port.
    assert_eq!(
        picked.reach(40_002, &table(&[(LO, 40_002, LISTEN, 21)])),
        PoolPort::Taken
    );
    picked.own(40_001, 11);
    assert_eq!(
        picked.reach(40_001, &table(&[(LO, 40_001, LISTEN, 11)])),
        PoolPort::Own
    );
    // Not listening yet, or no longer: nothing listens, as the kernel would answer a connect.
    assert_eq!(
        picked.reach(40_001, &table(&[(LO, 40_001, ESTABLISHED, 11)])),
        PoolPort::Free
    );
    assert_eq!(picked.reach(40_001, &table(&[])), PoolPort::Free);
    // Closed, and the port taken since by another socket.
    assert_eq!(
        picked.reach(40_001, &table(&[(LO, 40_001, LISTEN, 99)])),
        PoolPort::Taken
    );
    // Shared with another listener, on the same address, on every interface, or through
    // IPv6's mapped address or every IPv6 interface: a connect could reach either.
    for other in [LO, EVERY, mapped_lo(), EVERY6] {
        let both = format!(
            "{}{}",
            table(&[(LO, 40_001, LISTEN, 11)]),
            table(&[(other, 40_001, LISTEN, 99)])
        );
        assert_eq!(picked.reach(40_001, &both), PoolPort::Taken, "{other}");
    }
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
