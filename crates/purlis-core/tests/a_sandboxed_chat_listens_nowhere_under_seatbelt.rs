// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A wrapped chat on macOS starts no server, under its own compiled profile, for real.** A
//! small Perl program inside the profile tries to bind a port of this Mac and to listen with no
//! bind of its own, and prints `yes` where every step worked, or the step that was refused.

#![cfg(target_os = "macos")]

use purlis_core::sandbox::seatbelt;

const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// A socket of family `4` or `6` and type `stream` or `dgram`, bound to `address` on `port`
/// (or not bound at all, for `listen-only`), then listened on.
const TRY: &str = r#"
use Socket qw(:all);
my ($family, $type, $address, $port, $mode) = @ARGV;
my $af = $family eq "6" ? AF_INET6 : AF_INET;
my $st = $type eq "dgram" ? SOCK_DGRAM : SOCK_STREAM;
my $at = $af == AF_INET6 ? pack_sockaddr_in6($port, inet_pton(AF_INET6, $address))
                         : pack_sockaddr_in($port, inet_aton($address));
socket(my $s, $af, $st, 0) or do { print "socket\n"; exit };
if ($mode ne "listen-only") { bind($s, $at) or do { print "bind\n"; exit } }
if ($st == SOCK_DGRAM) { print "yes\n"; exit }
listen($s, 4) or do { print "listen\n"; exit };
print "yes\n";
"#;

/// Whether this process can apply a profile at all: not where it is already sandboxed.
fn can_apply() -> bool {
    std::process::Command::new(SANDBOX_EXEC)
        .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
        .status()
        .is_ok_and(|status| status.success())
}

/// A port nothing listens on now.
fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .expect("a free port")
        .port()
}

#[test]
fn a_wrapped_chat_binds_no_port_and_listens_nowhere() {
    if !can_apply() {
        eprintln!("skipped: this process is already sandboxed, so no second profile applies");
        return;
    }
    let cwd = tempfile::tempdir().expect("a folder");
    let tmp = tempfile::tempdir().expect("a temp folder");
    let profile = seatbelt::profile(
        &[],
        &seatbelt::Own::default(),
        cwd.path(),
        tmp.path(),
        &[],
        None,
    )
    .expect("a profile");
    let port = free_port();
    let tried = |family: &str, kind: &str, address: &str, mode: &str| -> String {
        let out = std::process::Command::new(SANDBOX_EXEC)
            .args([
                "-p",
                &profile,
                "/usr/bin/perl",
                "-e",
                TRY,
                family,
                kind,
                address,
            ])
            .arg(port.to_string())
            .arg(mode)
            .current_dir(cwd.path())
            .output()
            .expect("it runs");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    assert_eq!(tried("4", "stream", "127.0.0.1", "serve"), "bind");
    assert_eq!(tried("6", "stream", "::1", "serve"), "bind");
    assert_eq!(tried("4", "dgram", "127.0.0.1", "serve"), "bind");
    assert_eq!(tried("4", "stream", "127.0.0.1", "listen-only"), "listen");
    assert_eq!(tried("6", "stream", "::1", "listen-only"), "listen");
}
