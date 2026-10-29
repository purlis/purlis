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

/// #2019: a server's own refusal to listen names the port it asked for, on this Mac's loopback
/// or on every interface, and nothing else does.
#[test]
fn a_refused_listen_names_the_port_the_server_asked_for() {
    for (line, port) in [
        (
            "Error: listen EPERM: operation not permitted 127.0.0.1:3000",
            3000,
        ),
        ("Error: listen EPERM: operation not permitted :::5173", 5173),
        (
            "Error: listen EPERM: operation not permitted ::1:8080",
            8080,
        ),
        ("listen EPERM: operation not permitted 0.0.0.0:4000", 4000),
        (
            "listen tcp 127.0.0.1:8080: bind: operation not permitted",
            8080,
        ),
        ("listen tcp :9000: bind: operation not permitted", 9000),
        ("listen tcp [::1]:9001: bind: operation not permitted", 9001),
        (
            "listen tcp4 0.0.0.0:9002: bind: operation not permitted",
            9002,
        ),
    ] {
        assert_eq!(listen_refused_in(line), Some(port), "{line}");
    }
    for never in [
        // Another machine's address, a port already taken, a privileged port's own error, a
        // connect, and a port of zero.
        "listen tcp 192.168.1.5:3000: bind: operation not permitted",
        "Error: listen EADDRINUSE: address already in use 127.0.0.1:3000",
        "listen tcp 127.0.0.1:80: bind: permission denied",
        "dial tcp 127.0.0.1:3000: connect: operation not permitted",
        "listen tcp 127.0.0.1:0: bind: operation not permitted",
        "Error: listen EPERM: operation not permitted",
    ] {
        assert_eq!(listen_refused_in(never), None, "{never}");
    }
}

/// #2019: a chat allowed to serve on port N listens on a socket of its own in the folder purlis
/// made for it, and on no TCP port: that socket alone is made, bound and accepted on.
#[test]
fn a_chats_server_listens_on_its_socket_in_its_folder_and_on_no_port() {
    let folder = std::path::Path::new("/data/serve/chat-1");
    assert_eq!(
        serve_socket(folder, 3000),
        std::path::PathBuf::from("/data/serve/chat-1/3000.sock")
    );
    assert_eq!(
        serve_rules(&serve_socket(folder, 3000)).expect("rules"),
        [
            "(allow network-bind (local unix-socket (path-literal \"/data/serve/chat-1/3000.sock\")))",
            "(allow network-inbound (local unix-socket (path-literal \"/data/serve/chat-1/3000.sock\")))",
            "(allow file-write-create file-write-unlink (literal \"/data/serve/chat-1/3000.sock\"))",
        ]
    );
    assert!(
        !serve_rules(&serve_socket(folder, 3000))
            .expect("rules")
            .iter()
            .any(|rule| rule.contains("tcp") || rule.contains("ip "))
    );
}

/// #2019: the Ask says what a page a chat's server sends can do in the person's own browser, and
/// claims no more of the door than it holds.
#[test]
fn a_servers_risk_says_what_its_page_can_do_in_your_browser() {
    for said in [listen_risk(5173), listen_risk_unnamed()] {
        assert!(
            said.contains("runs in your browser as a site of this Mac"),
            "{said}"
        );
        assert!(said.contains("cookies that are not HttpOnly"), "{said}");
        assert!(
            said.contains("send what it reads to the internet"),
            "{said}"
        );
        assert!(
            said.contains("send requests to those apps as you"),
            "{said}"
        );
        assert!(said.contains("Open it in the chat's own browser"), "{said}");
        assert!(!said.contains("cookies for other sites"), "{said}");
        assert!(!said.contains("reaches it"), "{said}");
    }
}

/// #2019: a port given for a listen grant alone is connected to only where its door opened, and
/// a chat no door opens for (Claude Code's) is given nothing for the grant.
#[test]
fn a_port_given_for_a_listen_grant_alone_is_reached_only_where_its_door_opened() {
    use super::super::{Widened, seatbelt};
    let widened = Widened {
        ports: vec![5173, 8080],
        listens: vec![5173],
        listen_only: vec![5173],
        ..Widened::default()
    };
    let mut own = seatbelt::Own::default();
    seatbelt::widen(&mut own, &widened).expect("widened");
    assert!(!own.after.contains(&seatbelt_rule(5173)), "{:?}", own.after);
    assert!(own.after.contains(&seatbelt_rule(8080)), "{:?}", own.after);
    seatbelt::served(
        &mut own,
        std::path::Path::new("/run/user/1000/purlis-serve/0123456789abcdef"),
        &[5173],
    )
    .expect("served");
    assert!(own.after.contains(&seatbelt_rule(5173)), "{:?}", own.after);
    let unserving = widened.unserving();
    assert_eq!(unserving.ports, vec![8080]);
    assert!(unserving.listens.is_empty());
}

/// **Under Seatbelt, for real** (#2019): a chat allowed to serve on a port serves on its own
/// socket and on no TCP port, and is reached through the door alone. Small Perl programs inside
/// the chat's own compiled profile try each thing and print `yes`, or the step refused.
#[cfg(target_os = "macos")]
mod under_seatbelt {
    use std::io::{Read, Write};
    use std::os::unix::fs::DirBuilderExt;

    use super::super::super::{Widened, backend, door, seatbelt};
    use super::serve_socket;

    /// A TCP socket of family `4` or `6` bound to `address` on `port`, then listened on.
    const TCP: &str = r#"
use Socket qw(:all);
my ($family, $address, $port) = @ARGV;
my $af = $family eq "6" ? AF_INET6 : AF_INET;
my $at = $af == AF_INET6 ? pack_sockaddr_in6($port, inet_pton(AF_INET6, $address))
                         : pack_sockaddr_in($port, inet_aton($address));
socket(my $s, $af, SOCK_STREAM, 0) or do { print "socket\n"; exit };
bind($s, $at) or do { print "bind\n"; exit };
listen($s, 4) or do { print "listen\n"; exit };
print "yes\n";
"#;

    /// A unix socket at a path, bound and listened on; with `serve`, answering each request
    /// with `ok` until it is killed.
    const UNIX: &str = r#"
use Socket qw(:all);
$| = 1;
my ($path, $mode) = @ARGV;
unlink $path;
socket(my $s, AF_UNIX, SOCK_STREAM, 0) or do { print "socket\n"; exit };
bind($s, pack_sockaddr_un($path)) or do { print "bind\n"; exit };
listen($s, 4) or do { print "listen\n"; exit };
print "yes\n";
exit unless $mode eq "serve";
while (accept(my $c, $s)) {
  while (my $line = <$c>) { last if $line eq "\r\n"; }
  print $c "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
  close $c;
}
"#;

    fn can_apply() -> bool {
        crate::forklock::status(std::process::Command::new(backend::SANDBOX_EXEC).args([
            "-p",
            "(version 1)(allow default)",
            "/usr/bin/true",
        ]))
        .is_ok_and(|status| status.success())
    }

    fn free_port() -> u16 {
        std::net::TcpListener::bind(("127.0.0.1", 0))
            .and_then(|listener| listener.local_addr())
            .expect("a free port")
            .port()
    }

    fn ask(port: u16, host: &str) -> String {
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).expect("the door");
        write!(client, "GET / HTTP/1.1\r\nHost: {host}\r\n\r\n").expect("sent");
        let mut answer = String::new();
        let _ = client.read_to_string(&mut answer);
        answer
    }

    #[test]
    fn a_chats_server_serves_on_its_socket_alone_and_is_reached_through_its_door() {
        if !can_apply() {
            let _ = writeln!(
                std::io::stderr(),
                "skipped: this process is already sandboxed, so no second profile applies"
            );
            return;
        }
        let port = free_port();
        let base = tempfile::tempdir().expect("a folder");
        let folder = base.path().canonicalize().expect("real").join("serve");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&folder)
            .expect("the chat's folder");
        let socket = serve_socket(&folder, port);
        let cwd = tempfile::tempdir().expect("a folder");
        let tmp = tempfile::tempdir().expect("a temp folder");
        let mut own = seatbelt::Own::default();
        seatbelt::widen(
            &mut own,
            &Widened {
                ports: vec![port],
                listens: vec![port],
                listen_only: vec![port],
                ..Widened::default()
            },
        )
        .expect("widens");
        seatbelt::served(&mut own, &folder, &[port]).expect("served");
        let profile =
            seatbelt::profile(&[], &own, cwd.path(), tmp.path(), &[], None).expect("a profile");
        let tried = |script: &str, words: &[&str]| -> String {
            let out = crate::forklock::output(
                std::process::Command::new(backend::SANDBOX_EXEC)
                    .args(["-p", &profile, "/usr/bin/perl", "-e", script])
                    .args(words)
                    .current_dir(cwd.path()),
            )
            .expect("it runs");
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        };
        let port_word = port.to_string();
        // Its own socket, and nothing else: no TCP port at all.
        assert_eq!(tried(UNIX, &[&socket.display().to_string(), "once"]), "yes");
        assert_eq!(
            tried(
                UNIX,
                &[&folder.join("other.sock").display().to_string(), "once"]
            ),
            "bind"
        );
        for (family, address) in [
            ("4", "127.0.0.1"),
            ("6", "::1"),
            ("4", "0.0.0.0"),
            ("6", "::"),
        ] {
            assert_eq!(
                tried(TCP, &[family, address, &port_word]),
                "bind",
                "{address}"
            );
        }
        // Served through the door: the chat's server answers there, and a request that names
        // another host is refused before it reaches it.
        let mut server = crate::forklock::spawn(
            std::process::Command::new(backend::SANDBOX_EXEC)
                .args(["-p", &profile, "/usr/bin/perl", "-e", UNIX])
                .arg(socket.display().to_string())
                .arg("serve")
                .current_dir(cwd.path())
                .stdout(std::process::Stdio::piped()),
        )
        .expect("the server starts");
        let mut ready = [0u8; 4];
        let _ = server
            .stdout
            .take()
            .expect("its output")
            .read_exact(&mut ready);
        let door = door::Door::open(port, &socket).expect("the door opens");
        let carried = ask(port, &format!("127.0.0.1:{port}"));
        let refused = ask(port, "evil.example");
        drop(door);
        let _ = server.kill();
        let _ = server.wait();
        assert!(carried.starts_with("HTTP/1.1 200 OK\r\n"), "{carried}");
        assert!(carried.ends_with("ok"), "{carried}");
        assert!(refused.starts_with("HTTP/1.1 421 "), "{refused}");
        // No other user reaches the socket: its folder is this user's alone.
        let other_user = crate::forklock::output(std::process::Command::new("/usr/bin/sudo").args([
            "-n",
            "-u",
            "nobody",
            "/usr/bin/perl",
            "-MSocket",
            "-e",
            "socket(my $s, AF_UNIX, SOCK_STREAM, 0); print(connect($s, pack_sockaddr_un($ARGV[0])) ? qq(yes) : qq(no))",
            &socket.display().to_string(),
        ]));
        if let Ok(out) = other_user
            && out.status.success()
        {
            assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "no");
        }
    }
    /// **Claude Code's rules for a socket it is allowed** (#3316), as its macOS profile builder
    /// writes each `network.allowUnixSockets` path (read out of Claude Code 2.1.312): the socket
    /// domain, `network-bind` and `network-outbound` on the path as a `subpath`, and no
    /// `network-inbound`. With the file's own make and remove allowed, as its `allowWrite` would.
    /// What a server there gets is printed: `yes`, or the step refused.
    #[test]
    fn claude_codes_rules_for_an_allowed_socket_and_a_server_there() {
        if !can_apply() {
            let _ = writeln!(
                std::io::stderr(),
                "skipped: this process is already sandboxed, so no second profile applies"
            );
            return;
        }
        let base = tempfile::tempdir().expect("a folder");
        let folder = base.path().canonicalize().expect("real").join("serve");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&folder)
            .expect("the chat's folder");
        let socket = serve_socket(&folder, 4000);
        let at = seatbelt::string(&socket.display().to_string()).expect("a string");
        let cwd = tempfile::tempdir().expect("a folder");
        let tmp = tempfile::tempdir().expect("a temp folder");
        let mut own = seatbelt::Own::default();
        own.after.extend([
            "(allow system-socket (socket-domain AF_UNIX))".to_owned(),
            format!("(allow network-bind (local unix-socket (subpath {at})))"),
            format!("(allow network-outbound (remote unix-socket (subpath {at})))"),
            format!("(allow file-write-create file-write-unlink (literal {at}))"),
        ]);
        let profile =
            seatbelt::profile(&[], &own, cwd.path(), tmp.path(), &[], None).expect("a profile");
        let out = crate::forklock::output(
            std::process::Command::new(backend::SANDBOX_EXEC)
                .args(["-p", &profile, "/usr/bin/perl", "-e", UNIX])
                .arg(socket.display().to_string())
                .arg("once")
                .current_dir(cwd.path()),
        )
        .expect("it runs");
        let got = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        let _ = writeln!(std::io::stderr(), "PROBE-3316 claude code socket: {got}");
        assert_eq!(got, "yes", "PROBE-3316");
    }

    /// **A port the system picks** (#3396): a bind of `127.0.0.1:0` allowed by a Seatbelt rule
    /// on port 0, with listening allowed on every loopback port. What a server gets is printed.
    #[test]
    fn a_port_the_system_picks_under_a_rule_for_port_zero() {
        if !can_apply() {
            let _ = writeln!(
                std::io::stderr(),
                "skipped: this process is already sandboxed, so no second profile applies"
            );
            return;
        }
        let cwd = tempfile::tempdir().expect("a folder");
        let tmp = tempfile::tempdir().expect("a temp folder");
        let mut own = seatbelt::Own::default();
        own.after.extend([
            r#"(allow network-bind (local ip "localhost:0"))"#.to_owned(),
            r#"(allow network-inbound (local ip "localhost:*"))"#.to_owned(),
        ]);
        let Ok(profile) = seatbelt::profile(&[], &own, cwd.path(), tmp.path(), &[], None) else {
            panic!("PROBE-3396 no profile");
        };
        let tried = |words: &[&str]| -> String {
            let out = crate::forklock::output(
                std::process::Command::new(backend::SANDBOX_EXEC)
                    .args(["-p", &profile, "/usr/bin/perl", "-e", TCP])
                    .args(words)
                    .current_dir(cwd.path()),
            )
            .expect("it runs");
            format!(
                "{} {}",
                String::from_utf8_lossy(&out.stdout).trim(),
                String::from_utf8_lossy(&out.stderr).trim()
            )
        };
        let zero = tried(&["4", "127.0.0.1", "0"]);
        let named = tried(&["4", "127.0.0.1", &free_port().to_string()]);
        let every = tried(&["4", "0.0.0.0", "0"]);
        let _ = writeln!(
            std::io::stderr(),
            "PROBE-3396 zero: {zero} | named: {named} | every: {every}"
        );
        assert!(
            zero.starts_with("yes"),
            "PROBE-3396 {zero} | {named} | {every}"
        );
    }

    /// A server the person starts on every interface after the door opened is refused its
    /// bind, as on Linux, and the port is free on every interface once the door is let go.
    #[test]
    fn a_door_holds_its_port_on_every_interface_against_a_later_server() {
        let port = free_port();
        let base = tempfile::tempdir().expect("a folder");
        let folder = base.path().canonicalize().expect("real").join("serve");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&folder)
            .expect("the chat's folder");
        let door = door::Door::open(port, &serve_socket(&folder, port)).expect("the door opens");
        // As a dev server binds, with the address shared (Rust's listener sets it, as Node's).
        for address in ["0.0.0.0", "::"] {
            assert!(
                std::net::TcpListener::bind((address, port)).is_err(),
                "{address}:{port} was let bind beside the door"
            );
        }
        drop(door);
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(std::net::TcpListener::bind(("0.0.0.0", port)).is_ok());
    }
}
