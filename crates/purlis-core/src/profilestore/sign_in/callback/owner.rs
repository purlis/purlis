// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which of the loopback's addresses a login's own group listens on at a port**, by the socket's
//! owner and not by who answers a connection: a chat on the same runner may listen on the other
//! address, or be there before the login is. Where the system says nothing of owners (`None`), the
//! caller keeps to the address the login was found on.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::net::IpAddr;

#[cfg(target_os = "linux")]
use super::LOOPBACK;

/// The state `/proc/net/tcp` writes for a listening socket.
const LISTENING: &str = "0A";

/// The socket inodes listening at `port` in a `/proc/net/tcp` or `tcp6` table, with the address
/// each is bound to, as the table writes it.
fn listeners_in(table: &str, port: u16) -> Vec<(&str, u64)> {
    let port = format!("{port:04X}");
    table
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (address, at) = fields.get(1)?.split_once(':')?;
            let inode = fields.get(9)?.parse().ok()?;
            (at == port && *fields.get(3)? == LISTENING).then_some((address, inode))
        })
        .collect()
}

/// The inode of the socket a connection to `at` reaches of `listeners`: the one bound to `at`,
/// which the system prefers, else one on every address of its family.
fn reached_by(at: IpAddr, v4: &[(&str, u64)], v6: &[(&str, u64)]) -> Option<u64> {
    const ANY_V4: &str = "00000000";
    const ANY_V6: &str = "00000000000000000000000000000000";
    const V4_HERE: &str = "0100007F";
    const V6_HERE: &str = "00000000000000000000000001000000";
    let bound = |table: &[(&str, u64)], address: &str| {
        table
            .iter()
            .find(|(bound, _)| *bound == address)
            .map(|(_, inode)| *inode)
    };
    if at.is_ipv4() {
        bound(v4, V4_HERE)
            .or_else(|| bound(v4, ANY_V4))
            .or_else(|| bound(v6, ANY_V6))
    } else {
        bound(v6, V6_HERE).or_else(|| bound(v6, ANY_V6))
    }
}

/// The process group a `/proc/<pid>/stat` line says, which is read after the program's name, the
/// one part of the line a program chooses.
fn group_in_stat(stat: &str) -> Option<u32> {
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(2)?
        .parse()
        .ok()
}

/// Every socket inode a process of `group` holds, from `/proc`.
#[cfg(target_os = "linux")]
fn sockets_of_group(group: u32) -> Option<std::collections::HashSet<u64>> {
    let mut sockets = std::collections::HashSet::new();
    for process in std::fs::read_dir("/proc").ok()?.flatten() {
        let path = process.path();
        let in_group = std::fs::read_to_string(path.join("stat"))
            .ok()
            .and_then(|stat| group_in_stat(&stat))
            == Some(group);
        if !in_group {
            continue;
        }
        let held = match std::fs::read_dir(path.join("fd")) {
            Ok(held) => held,
            // It ended since, and holds nothing.
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => continue,
            // The system won't say what it holds, so this says nothing of owners.
            Err(_) => return None,
        };
        for fd in held.flatten() {
            let Ok(link) = std::fs::read_link(fd.path()) else {
                continue;
            };
            let inode = link
                .to_str()
                .and_then(|link| link.strip_prefix("socket:["))
                .and_then(|rest| rest.strip_suffix(']'))
                .and_then(|inode| inode.parse::<u64>().ok());
            sockets.extend(inode);
        }
    }
    Some(sockets)
}

/// **The loopback addresses at `port` a process of `group` is listening on**, in the order they
/// are asked: empty where it holds neither. `None` where the system's tables can't be read.
#[cfg(target_os = "linux")]
pub(super) fn own_addresses(port: u16, group: u32) -> Option<Vec<IpAddr>> {
    let v4 = std::fs::read_to_string("/proc/net/tcp").ok()?;
    // A machine with IPv6 off has no such table: nothing listens there.
    let v6 = match std::fs::read_to_string("/proc/net/tcp6") {
        Ok(table) => table,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return None,
    };
    let (v4, v6) = (listeners_in(&v4, port), listeners_in(&v6, port));
    let sockets = sockets_of_group(group)?;
    Some(
        LOOPBACK
            .into_iter()
            .filter(|at| reached_by(*at, &v4, &v6).is_some_and(|inode| sockets.contains(&inode)))
            .collect(),
    )
}

/// No other system's tables are read here. A runner is Linux.
#[cfg(not(target_os = "linux"))]
pub(super) fn own_addresses(_port: u16, _group: u32) -> Option<Vec<IpAddr>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    const TABLE: &str =
        "  sl  local_address rem_address   st tx:rx tr:when retrnsmt   uid  timeout inode
   0: 0100007F:05AF 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 111 1 0
   1: 0100007F:05AF 0100007F:9000 01 00000000:00000000 00:00000000 00000000  1000        0 222 1 0
   2: 00000000:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 333 1 0
";

    #[test]
    fn a_table_gives_only_what_listens_at_the_port() {
        assert_eq!(listeners_in(TABLE, 1455), vec![("0100007F", 111)]);
        assert_eq!(listeners_in(TABLE, 8080), vec![("00000000", 333)]);
        assert!(listeners_in(TABLE, 1).is_empty());
    }

    #[test]
    fn a_connection_reaches_the_socket_bound_to_its_address_before_one_on_every_address() {
        let here: IpAddr = Ipv4Addr::LOCALHOST.into();
        let here6: IpAddr = Ipv6Addr::LOCALHOST.into();
        let v4 = [("00000000", 1), ("0100007F", 2)];
        let v6 = [
            ("00000000000000000000000000000000", 3),
            ("00000000000000000000000001000000", 4),
        ];
        assert_eq!(reached_by(here, &v4, &v6), Some(2));
        assert_eq!(reached_by(here6, &v4, &v6), Some(4));
        assert_eq!(reached_by(here, &v4[..1], &v6), Some(1));
        assert_eq!(reached_by(here, &[], &v6), Some(3));
        assert_eq!(reached_by(here6, &v4, &[]), None);
    }

    #[test]
    fn a_programs_own_name_does_not_say_its_group() {
        assert_eq!(group_in_stat("12 (a) R 1 77 77 0"), Some(77));
        assert_eq!(group_in_stat("12 (a) R 9 9 (b) R 1 77 77 0"), Some(77));
        assert_eq!(group_in_stat("no name here"), None);
    }
}
