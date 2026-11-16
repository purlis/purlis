// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The resolver seam, the lookup and dial recorders, and the local origins** (T1.1-02, #6849):
//! a chat proxy and a browser's reach proxy, started with a scenario's [`Names`], reach a local
//! origin by name and nothing else, and the seam never loosens what they decide.
//!
//! These bind local ports, so they run in CI (`sandbox-scenarios-linux`, and the labelled macOS
//! run); a sandbox that refuses a bind fails them, never passes them.

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use purlis_core::sandbox::browser_reach::{Lane, NewSites, ReachProxy};
use purlis_core::sandbox::egress::{Proxy, Serving};
use purlis_core::sandbox::names::{self, Found, Lookup, Names};
use purlis_core::sandbox::reach::Reach;
use purlis_core::sandbox::timings::Timings;
use purlis_scenarios::net::{FORBIDDEN_BODY, ORIGIN_HEADER, Origin, Scheme, TestCa, Variant};

const WAIT: Duration = Duration::from_secs(20);

fn loopback() -> IpAddr {
    IpAddr::from([127, 0, 0, 1])
}

/// A chat proxy for `hosts`, looking names up and dialling through `names`.
fn proxy(hosts: &[&str], names: &Arc<Names>) -> Proxy {
    Proxy::serving(Serving {
        names: Some(Arc::clone(names)),
        ..Serving::of(
            Reach::open(hosts.iter().map(|host| (*host).to_owned()).collect()),
            &Timings::production(),
        )
    })
    .expect("the proxy starts")
}

fn connected(port: u16) -> TcpStream {
    let stream = TcpStream::connect(("127.0.0.1", port)).expect("the proxy answers");
    stream.set_read_timeout(Some(WAIT)).expect("a timeout");
    stream
}

/// A plain request for `http://host/path` through the HTTP proxy on `port`: the whole answer.
fn get(port: u16, host: &str, path: &str) -> String {
    let mut stream = connected(port);
    write!(
        stream,
        "GET http://{host}{path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .expect("the request is sent");
    let mut out = String::new();
    let _ = stream.read_to_string(&mut out);
    out
}

/// A `CONNECT host:443` through the HTTP proxy on `port`, then TLS to `host` trusting `ca`, and
/// a request for `path`: the proxy's answer to the CONNECT, and the origin's.
fn tunnel_tls(port: u16, host: &str, path: &str, ca: &TestCa) -> (String, String) {
    let mut stream = connected(port);
    write!(
        stream,
        "CONNECT {host}:443 HTTP/1.1\r\nHost: {host}:443\r\n\r\n"
    )
    .expect("the CONNECT is sent");
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head).into_owned();
    if !head.starts_with("HTTP/1.1 200") {
        let mut rest = String::new();
        let _ = stream.read_to_string(&mut rest);
        return (head + &rest, String::new());
    }
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca.der()).expect("the test CA is a root");
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("TLS versions")
    .with_root_certificates(roots)
    .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host.to_owned()).expect("a server name");
    let conn = rustls::ClientConnection::new(Arc::new(config), name).expect("a TLS client");
    let mut tls = rustls::StreamOwned::new(conn, stream);
    write!(
        tls,
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .expect("the request is sent over TLS");
    let mut out = String::new();
    let _ = tls.read_to_string(&mut out);
    (head, out)
}

/// A SOCKS5 connect to `host` on `port` through the SOCKS proxy on `socks`: the reply code, and
/// the stream where it succeeded.
fn socks(socks: u16, host: &str, port: u16) -> (u8, TcpStream) {
    let mut stream = connected(socks);
    stream.write_all(&[5, 1, 0]).expect("the greeting");
    let mut chosen = [0u8; 2];
    stream.read_exact(&mut chosen).expect("a method");
    assert_eq!(chosen, [5, 0], "no authentication");
    let mut request = vec![5, 1, 0, 3, u8::try_from(host.len()).expect("a short name")];
    request.extend_from_slice(host.as_bytes());
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).expect("the request");
    let mut reply = [0u8; 10];
    stream.read_exact(&mut reply).expect("a reply");
    (reply[1], stream)
}

fn status(answer: &str) -> &str {
    answer.lines().next().unwrap_or_default()
}

/// Whether `answer` is purlis's own refusal: a 403 with no origin's header.
fn purlis_refused(answer: &str) -> bool {
    status(answer).starts_with("HTTP/1.1 403")
        && !answer.to_ascii_lowercase().contains(ORIGIN_HEADER)
}

#[test]
fn a_mapped_origin_is_decided_by_name_and_carried_to_the_local_origin() {
    let ca = TestCa::new().expect("a test CA");
    let plain = Origin::http().expect("a plain origin");
    let tls = Origin::start(
        Scheme::Https {
            ca: &ca,
            names: &["api.example.test"],
        },
        Variant::Plain,
        loopback(),
    )
    .expect("an HTTPS origin");
    let names = Names::new();
    names
        .origin("api.example.test", 80, plain.addr())
        .origin("api.example.test", 443, tls.addr());
    let proxy = proxy(&["api.example.test"], &names);

    // A plain request, by the HTTP proxy.
    let answer = get(proxy.port(), "api.example.test", "/hello");
    assert!(status(&answer).starts_with("HTTP/1.1 200"), "{answer}");
    let seen = plain.requests();
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].host.as_deref(), Some("api.example.test"));
    assert_eq!(seen[0].path, "/hello");

    // A tunnel, by CONNECT, TLS end to end under the test CA.
    let (head, answer) = tunnel_tls(proxy.port(), "api.example.test", "/secure", &ca);
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(status(&answer).starts_with("HTTP/1.1 200"), "{answer}");
    let seen = tls.requests();
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].sni.as_deref(), Some("api.example.test"));
    assert_eq!(seen[0].path, "/secure");

    // A tunnel by SOCKS, to the same origin.
    let (code, _stream) = socks(proxy.socks_port(), "api.example.test", 80);
    assert_eq!(code, 0, "SOCKS succeeded");

    assert_eq!(
        names.origin_hits(),
        vec![
            ("api.example.test".to_owned(), 80),
            ("api.example.test".to_owned(), 443),
            ("api.example.test".to_owned(), 80),
        ]
    );
    assert_eq!(
        names.dials(),
        vec![plain.addr(), tls.addr(), plain.addr()],
        "each carried connection dialled its origin, and nothing else"
    );
}

#[test]
fn a_refused_name_is_never_looked_up_dialled_or_reached() {
    let origin = Origin::http().expect("an origin");
    let names = Names::new();
    // Mapped every way the seam can map a name: the map never makes a refused name carried.
    names
        .origin("blocked.test", 80, origin.addr())
        .origin("blocked.test", 443, origin.addr())
        .resolves_to("other.test", 80, vec![vec![origin.addr()]]);
    let proxy = proxy(&["allowed.test"], &names);

    let answer = get(proxy.port(), "blocked.test", "/");
    assert!(purlis_refused(&answer), "{answer}");
    let answer = get(proxy.port(), "other.test", "/");
    assert!(purlis_refused(&answer), "{answer}");
    let (head, _) = tunnel_tls(
        proxy.port(),
        "blocked.test",
        "/",
        &TestCa::new().expect("a CA"),
    );
    assert!(purlis_refused(&head), "{head}");
    let (code, _) = socks(proxy.socks_port(), "blocked.test", 80);
    assert_ne!(code, 0, "SOCKS refused it");

    assert_eq!(names.lookups(), Vec::<Lookup>::new(), "no lookup");
    assert_eq!(names.dials(), Vec::<SocketAddr>::new(), "no dial");
    assert_eq!(names.origin_hits(), Vec::new(), "no origin reached");
    assert!(names.saw_nothing());
    assert!(origin.requests().is_empty(), "{:?}", origin.requests());
}

#[test]
fn a_resolves_to_answer_of_loopback_is_refused_as_only_local() {
    let origin = Origin::http().expect("an origin on the loopback");
    let names = Names::new();
    names.resolves_to("rebound.test", 80, vec![vec![origin.addr()]]);
    let proxy = proxy(&["rebound.test"], &names);

    let answer = get(proxy.port(), "rebound.test", "/");
    assert!(purlis_refused(&answer), "{answer}");
    assert!(
        answer.contains("this machine") || answer.contains("local"),
        "refused as a local address: {answer}"
    );
    assert_eq!(
        names.lookups(),
        vec![Lookup {
            name: "rebound.test".into(),
            port: 80,
            found: Found::Scripted(vec![origin.addr()]),
        }]
    );
    assert_eq!(names.dials(), Vec::<SocketAddr>::new(), "never dialled");
    assert!(origin.requests().is_empty(), "{:?}", origin.requests());
}

#[test]
fn a_rebinding_answer_on_the_second_lookup_is_refused() {
    // The first answer is an address the chat may reach (listed exactly, on this machine's
    // non-loopback interface); the second, for the same name, is the loopback.
    let ip = purlis_scenarios::net::interface_address().expect("a non-loopback interface");
    let first = Origin::start(Scheme::Http, Variant::Plain, ip).expect("an origin on it");
    let second = Origin::http().expect("an origin on the loopback");
    let names = Names::new();
    names.resolves_to(
        "rebind.test",
        80,
        vec![vec![first.addr()], vec![second.addr()]],
    );
    let exact = first.addr().to_string();
    let proxy = proxy(&["rebind.test", &exact], &names);

    let answer = get(proxy.port(), "rebind.test", "/first");
    assert!(status(&answer).starts_with("HTTP/1.1 200"), "{answer}");
    let answer = get(proxy.port(), "rebind.test", "/second");
    assert!(purlis_refused(&answer), "{answer}");

    assert_eq!(names.lookups().len(), 2, "{:?}", names.lookups());
    assert_eq!(
        names.dials(),
        vec![first.addr()],
        "the rebound answer is never dialled"
    );
    assert_eq!(first.requests().len(), 1);
    assert!(second.requests().is_empty(), "{:?}", second.requests());
}

#[test]
fn an_unmapped_name_never_reaches_the_system_resolver_and_fails_the_scenario() {
    let unmapped = format!("unmapped-{}.test", std::process::id());
    let names = Names::new();
    let proxy = proxy(&[&unmapped], &names);

    let answer = get(proxy.port(), &unmapped, "/");
    assert!(
        status(&answer).starts_with("HTTP/1.1 502"),
        "carried nowhere: {answer}"
    );
    assert_eq!(
        names.lookups(),
        vec![Lookup {
            name: unmapped.clone(),
            port: 80,
            found: Found::Unmapped,
        }],
        "recorded"
    );
    assert_eq!(names.dials(), Vec::<SocketAddr>::new());
    assert!(
        !names::asked_the_system(&unmapped),
        "never asked of the system's resolver"
    );
    assert_eq!(
        names.unlisted(&[]),
        vec![(unmapped.clone(), 80)],
        "fails the scenario"
    );
    assert!(
        names.unlisted(&[(&unmapped, 80)]).is_empty(),
        "unless the cell lists it"
    );

    // The control: a proxy with no seam asks the system's resolver, and the recorder sees it.
    let control = format!("control-{}.invalid", std::process::id());
    let open = Proxy::start(vec![control.clone()]).expect("a proxy with no seam");
    let _ = get(open.port(), &control, "/");
    assert!(names::asked_the_system(&control), "the recorder records");
}

#[test]
fn the_browser_reach_proxy_resolves_through_the_seam_and_is_recorded() {
    let origin = Origin::http().expect("an origin");
    let names = Names::new();
    names.origin("site.test", 80, origin.addr()).resolves_to(
        "rebound.test",
        80,
        vec![vec![origin.addr()]],
    );
    let lane = Lane::of(Reach::open(vec![
        "site.test".into(),
        "rebound.test".into(),
        "unmapped.test".into(),
    ]));
    let reach = ReachProxy::named(
        lane,
        NewSites::refusing(),
        &Timings::production(),
        Arc::clone(&names),
    )
    .expect("the reach proxy starts");
    let port = reach.address().port();

    let answer = get(port, "site.test", "/page");
    assert!(status(&answer).starts_with("HTTP/1.1 200"), "{answer}");
    assert_eq!(origin.requests().len(), 1);
    assert_eq!(origin.requests()[0].path, "/page");
    assert_eq!(names.origin_hits(), vec![("site.test".to_owned(), 80)]);
    assert_eq!(names.dials(), vec![origin.addr()]);

    // A scripted loopback answer is not public: refused, never dialled.
    let answer = get(port, "rebound.test", "/");
    assert!(purlis_refused(&answer), "{answer}");
    // An unmapped name answers nothing: carried nowhere, and recorded.
    let answer = get(port, "unmapped.test", "/");
    assert!(status(&answer).starts_with("HTTP/1.1 502"), "{answer}");
    // A site nothing lists is refused before any lookup.
    let answer = get(port, "elsewhere.test", "/");
    assert!(purlis_refused(&answer), "{answer}");

    let looked: Vec<(String, Found)> = names
        .lookups()
        .into_iter()
        .map(|lookup| (lookup.name, lookup.found))
        .collect();
    assert_eq!(
        looked,
        vec![
            ("site.test".to_owned(), Found::Origin(origin.addr())),
            (
                "rebound.test".to_owned(),
                Found::Scripted(vec![origin.addr()])
            ),
            ("unmapped.test".to_owned(), Found::Unmapped),
        ]
    );
    assert_eq!(
        names.dials(),
        vec![origin.addr()],
        "only the origin was dialled"
    );
    assert_eq!(origin.requests().len(), 1);
    assert!(!names::asked_the_system("unmapped.test"));
}

#[test]
fn an_origins_own_403_is_told_from_purliss() {
    let forbidden =
        Origin::start(Scheme::Http, Variant::Forbidden, loopback()).expect("a 403 origin");
    let names = Names::new();
    names.origin("refuses.test", 80, forbidden.addr());
    let proxy = proxy(&["refuses.test"], &names);

    let answer = get(proxy.port(), "refuses.test", "/");
    assert!(status(&answer).starts_with("HTTP/1.1 403"), "{answer}");
    assert!(!purlis_refused(&answer), "the site's own: {answer}");
    assert!(answer.contains(FORBIDDEN_BODY), "{answer}");
    assert_eq!(forbidden.requests().len(), 1);
}

#[test]
fn the_redirect_and_slow_origins_answer_as_named() {
    let names = Names::new();
    let redirect = Origin::start(
        Scheme::Http,
        Variant::RedirectTo("http://other.test/landed".into()),
        loopback(),
    )
    .expect("a redirecting origin");
    let slow = Origin::start(
        Scheme::Http,
        Variant::Slow(Duration::from_millis(300)),
        loopback(),
    )
    .expect("a slow origin");
    names
        .origin("moves.test", 80, redirect.addr())
        .origin("slow.test", 80, slow.addr());
    let proxy = proxy(&["moves.test", "slow.test"], &names);

    let answer = get(proxy.port(), "moves.test", "/");
    assert!(status(&answer).starts_with("HTTP/1.1 302"), "{answer}");
    assert!(
        answer.contains("location: http://other.test/landed"),
        "{answer}"
    );
    let started = std::time::Instant::now();
    let answer = get(proxy.port(), "slow.test", "/");
    assert!(status(&answer).starts_with("HTTP/1.1 200"), "{answer}");
    assert!(started.elapsed() >= Duration::from_millis(300));
}

#[test]
fn the_ca_bundle_holds_the_system_roots_and_the_test_ca() {
    let ca = TestCa::new().expect("a test CA");
    let dir = tempfile::tempdir().expect("a temp dir");
    let bundle = ca.bundle(dir.path()).expect("the bundle is written");
    let text = std::fs::read_to_string(&bundle).expect("the bundle reads");
    assert!(text.ends_with(&ca.pem()), "the test CA is in it");
    assert!(
        text.matches("BEGIN CERTIFICATE").count() > 1,
        "and the system's roots"
    );
    let env = purlis_scenarios::net::ca_env(&bundle);
    let named: Vec<&str> = env.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        named,
        [
            "SSL_CERT_FILE",
            "CURL_CA_BUNDLE",
            "REQUESTS_CA_BUNDLE",
            "NODE_EXTRA_CA_CERTS"
        ]
    );
    assert!(env.iter().all(|(_, value)| value == bundle.as_os_str()));
}

#[test]
fn the_direct_target_listens_off_the_loopback_and_records_what_reaches_it() {
    let target = purlis_scenarios::net::DirectTarget::start().expect("the direct target");
    assert!(!target.tcp().ip().is_loopback());
    assert!(!target.udp().ip().is_loopback());
    assert!(target.saw_nothing());

    // The positive control: a direct connection and a direct lookup are both seen.
    drop(TcpStream::connect_timeout(&target.tcp(), WAIT).expect("it answers"));
    let probe = std::net::UdpSocket::bind((target.udp().ip(), 0)).expect("a probe");
    probe.set_read_timeout(Some(WAIT)).expect("a timeout");
    let mut query = vec![0xab, 0xcd, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in ["direct", "test"] {
        query.push(u8::try_from(label.len()).expect("short"));
        query.extend_from_slice(label.as_bytes());
    }
    query.extend_from_slice(&[0, 0, 1, 0, 1]);
    probe
        .send_to(&query, target.udp())
        .expect("the query is sent");
    let mut reply = [0u8; 512];
    let (len, _) = probe.recv_from(&mut reply).expect("an answer");
    assert!(len > 12 && reply[3] & 0x0F == 5, "REFUSED");
    let deadline = std::time::Instant::now() + WAIT;
    while target.connections().is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(target.connections().len(), 1);
    assert_eq!(target.queries(), vec!["direct.test".to_owned()]);
}

/// The packages a shipped build is made of resolve purlis-core without `test-support`, so the
/// resolver seam is not in it: `cargo tree` over every workspace member's normal and build edges
/// (what `cargo build --workspace` and each `-p` release build resolve), with the dev edges as
/// the positive control that shows the feature is found where it is on.
#[test]
fn the_resolver_seam_is_absent_without_test_support() {
    let features = |edges: &str| -> String {
        let out = std::process::Command::new(env!("CARGO"))
            .args([
                "tree",
                "--locked",
                "--workspace",
                "-e",
                edges,
                "-i",
                "purlis-core",
            ])
            .current_dir(purlis_scenarios::root())
            .output()
            .expect("cargo tree runs");
        assert!(
            out.status.success(),
            "cargo tree: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let shipped = features("features,normal,build");
    assert!(
        shipped.contains("purlis-core"),
        "the tree names purlis-core: {shipped}"
    );
    assert!(
        !shipped.contains("feature \"test-support\""),
        "a shipped build turns purlis-core's test-support on:\n{shipped}"
    );
    let tested = features("features,normal,build,dev");
    assert!(
        tested.contains("purlis-core feature \"test-support\""),
        "the control: the test build has it:\n{tested}"
    );
}
