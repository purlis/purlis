// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser's reach, decided without a socket: which addresses are public, what a lane carries
//! by host and port, and where a carried connection is made.

use std::cell::Cell;
use std::net::{IpAddr, SocketAddr};

use std::time::{Duration, Instant};

use super::{
    Lane, NewSites, Refusal, Route, Site, Verdict, lookup_bounded, lookup_within, public_address,
    route,
};
use crate::sandbox::reach::{By, Decision, Reach};

mod sockets;

fn ip(text: &str) -> IpAddr {
    text.parse()
        .unwrap_or_else(|_| panic!("{text} is an address"))
}

#[test]
fn only_a_public_unicast_address_is_public() {
    for (address, public) in [
        // Public sites, IPv4 and IPv6.
        ("93.184.215.14", true),
        ("1.1.1.1", true),
        ("8.8.8.8", true),
        ("2606:4700:4700::1111", true),
        ("2a00:1450:4001:82a::200e", true),
        // A public address carried inside IPv6 forms is judged as itself.
        ("::ffff:93.184.215.14", true),
        ("64:ff9b::5db8:d70e", true),
        // This machine.
        ("127.0.0.1", false),
        ("127.0.0.2", false),
        ("127.255.255.254", false),
        ("::1", false),
        ("0.0.0.0", false),
        ("0.1.2.3", false),
        ("::", false),
        // Private ranges.
        ("10.0.0.5", false),
        ("10.255.255.255", false),
        ("172.16.0.1", false),
        ("172.31.255.254", false),
        ("192.168.1.1", false),
        // The shared address space carriers put behind NAT (CGNAT).
        ("100.64.0.1", false),
        ("100.127.255.254", false),
        // Link-local, and the cloud metadata services.
        ("169.254.0.1", false),
        ("169.254.169.254", false),
        ("100.100.100.200", false),
        ("168.63.129.16", false),
        ("192.0.0.192", false),
        ("fe80::1", false),
        ("fd00:ec2::254", false),
        // IPv6 unique local and the old site-local.
        ("fc00::1", false),
        ("fd12:3456:789a::1", false),
        ("fec0::1", false),
        // Multicast and broadcast.
        ("224.0.0.1", false),
        ("239.255.255.250", false),
        ("255.255.255.255", false),
        ("ff02::1", false),
        // Reserved, documentation and benchmarking ranges: nobody's site.
        ("240.0.0.1", false),
        ("192.0.0.8", false),
        ("192.0.2.1", false),
        ("198.18.0.1", false),
        ("198.51.100.1", false),
        ("203.0.113.1", false),
        ("2001:db8::1", false),
        ("100::1", false),
        // Teredo hides the address it reaches, and NAT64's local-use prefix is a local network.
        ("2001::1", false),
        // So does Teredo under the prefix used before RFC 4380 (#2741).
        ("3ffe:831f:cb00:7107::39cc:9bf6", false),
        ("64:ff9b:1::a00:5", false),
        // A local address carried inside an IPv6 form is judged as the local one.
        ("::ffff:127.0.0.1", false),
        ("::ffff:7f00:1", false),
        ("::ffff:10.0.0.5", false),
        ("::ffff:169.254.169.254", false),
        ("::ffff:192.168.1.1", false),
        ("::ffff:0:a9fe:a9fe", false),
        ("::127.0.0.1", false),
        ("64:ff9b::a9fe:a9fe", false),
        ("64:ff9b::a00:5", false),
        ("2002:a00:5::1", false),
        ("2002:7f00:1::1", false),
        // An ISATAP address is judged as itself and as the IPv4 address it carries (#2678).
        ("2606:4700:1::5efe:a00:5", false),
        ("2606:4700:1:0:200:5efe:c0a8:101", false),
        ("2606:4700:1::5efe:101:101", true),
    ] {
        assert_eq!(public_address(ip(address), &[]), public, "{address}");
    }
}

#[test]
fn an_address_of_this_machine_is_never_public() {
    let own = [ip("93.184.215.14"), ip("2606:4700:4700::1111")];
    assert!(!public_address(own[0], &own));
    assert!(!public_address(own[1], &own));
    assert!(public_address(ip("1.1.1.1"), &own));
}

fn at(text: &str) -> SocketAddr {
    text.parse()
        .unwrap_or_else(|_| panic!("{text} is an address and port"))
}

/// A chat's reach as its sandbox lists it: an open host, a host allowed for this project, a
/// service on this Mac at port 5173 allowed for the chat (by each name its proxy carries it by),
/// a private cluster and a public address held to one port.
fn chats_reach() -> Reach {
    Reach::of(
        [
            ("example.com", By::Open),
            ("api.example.org", By::You),
            ("127.0.0.1:5173", By::Chat),
            ("[::1]:5173", By::Chat),
            ("localhost:5173", By::Chat),
            ("this-mac.internal:5173", By::Chat),
            ("10.0.0.5:6443", By::You),
            ("93.184.215.14:8443", By::You),
        ]
        .into_iter()
        .map(|(host, by)| (host.to_owned(), by))
        .collect(),
    )
}

fn decide(host: &str, port: u16, tunnel: bool) -> Verdict {
    Lane::of(chats_reach()).decide(host, port, tunnel, &[])
}

fn carried(by: Decision, at_: Option<&str>) -> Verdict {
    Verdict::Carry {
        by,
        at: at_.map(at),
    }
}

#[test]
fn a_host_the_chat_lists_is_carried_on_https_and_http_ports() {
    for (host, port, tunnel, verdict) in [
        ("example.com", 443, true, carried(Decision::Open, None)),
        ("EXAMPLE.com.", 443, true, carried(Decision::Open, None)),
        ("example.com", 80, false, carried(Decision::Open, None)),
        (
            "api.example.org",
            443,
            true,
            carried(Decision::Allowed(By::You), None),
        ),
        (
            "93.184.215.14",
            8443,
            true,
            carried(Decision::Allowed(By::You), Some("93.184.215.14:8443")),
        ),
    ] {
        assert_eq!(decide(host, port, tunnel), verdict, "{host}:{port}");
    }
}

#[test]
fn a_site_nothing_lists_is_a_new_site_on_https_and_http_ports_only() {
    assert_eq!(
        decide("new.example.net", 443, true),
        Verdict::Site { at: None }
    );
    assert_eq!(
        decide("new.example.net", 80, false),
        Verdict::Site { at: None }
    );
    assert_eq!(
        decide("93.184.215.14", 443, true),
        Verdict::Site {
            at: Some(at("93.184.215.14:443"))
        }
    );
    for (host, port, tunnel) in [
        ("new.example.net", 22, true),
        ("new.example.net", 443, false),
        ("new.example.net", 80, true),
        ("example.com", 8443, true),
        ("93.184.215.14", 25, true),
    ] {
        assert_eq!(
            decide(host, port, tunnel),
            Verdict::Refused(Refusal::Port),
            "{host}:{port}"
        );
    }
}

#[test]
fn a_named_local_port_is_carried_by_its_literal_address_alone() {
    for (host, tunnel, to) in [
        ("127.0.0.1", true, "127.0.0.1:5173"),
        ("127.0.0.1", false, "127.0.0.1:5173"),
        ("::1", true, "[::1]:5173"),
        ("::1", false, "[::1]:5173"),
    ] {
        assert_eq!(
            decide(host, 5173, tunnel),
            carried(Decision::Allowed(By::Chat), Some(to)),
            "{host}"
        );
    }
}

#[test]
fn a_name_for_a_named_local_port_is_refused() {
    for host in [
        "localhost",
        "LOCALHOST",
        "localhost.",
        "app.localhost",
        "this-mac.internal",
        "host.docker.internal",
        "kubernetes.docker.internal",
    ] {
        for tunnel in [true, false] {
            assert_eq!(
                decide(host, 5173, tunnel),
                Verdict::Refused(Refusal::ThisMachineByName),
                "{host}"
            );
        }
    }
}

#[test]
fn this_machine_on_a_port_nobody_named_is_refused() {
    for (host, port) in [
        ("127.0.0.1", 5174),
        ("127.0.0.1", 443),
        ("::1", 22),
        ("127.0.0.2", 5173),
        ("0.0.0.0", 5173),
        ("::", 5173),
        ("::ffff:127.0.0.1", 5173),
        ("::127.0.0.1", 5173),
    ] {
        assert_eq!(
            decide(host, port, true),
            Verdict::Refused(Refusal::ThisMachine),
            "{host}:{port}"
        );
    }
}

#[test]
fn a_withdrawn_local_port_is_refused() {
    let reach = chats_reach();
    let lane = Lane::of(reach.clone());
    reach.withdraw(5173);
    assert_eq!(
        lane.decide("127.0.0.1", 5173, true, &[]),
        Verdict::Refused(Refusal::ThisMachine)
    );
}

#[test]
fn a_private_link_local_or_metadata_address_is_refused_unless_named_exactly() {
    for (host, port) in [
        ("10.0.0.5", 443),
        ("172.16.0.1", 443),
        ("192.168.1.1", 80),
        ("100.64.0.1", 443),
        ("169.254.169.254", 80),
        ("fd00:ec2::254", 80),
        ("fe80::1", 443),
        ("fd12:3456:789a::1", 443),
        ("::ffff:10.0.0.5", 6443),
        ("::ffff:169.254.169.254", 80),
        ("224.0.0.1", 443),
    ] {
        assert_eq!(
            decide(host, port, port != 80),
            Verdict::Refused(Refusal::NotPublic),
            "{host}:{port}"
        );
    }
}

#[test]
fn an_address_spelled_another_way_is_refused() {
    for host in [
        "2130706433",
        "0x7f000001",
        "0x7f.1",
        "0177.0.0.1",
        "127.1",
        "127.0.1",
        "017700000001",
        "127.0.0.01",
        "0x7F.0.0.1",
        "3627733262",
        "a.b.0x10",
        "example.123",
    ] {
        assert_eq!(
            decide(host, 5173, true),
            Verdict::Refused(Refusal::SpelledAnotherWay),
            "{host}"
        );
    }
}

#[test]
fn what_is_no_host_is_refused() {
    for host in [
        "fe80::1%en0",
        "intranet",
        "*.example.com",
        "exa mple.com",
        "",
    ] {
        assert!(
            matches!(
                decide(host, 443, true),
                Verdict::Refused(Refusal::NeverAHost(_))
            ),
            "{host}: {:?}",
            decide(host, 443, true)
        );
    }
}

/// A resolver that answers `answer` for every name, counting how often it is asked.
fn resolving(answer: &[&str], asked: &Cell<usize>) -> impl Fn(&str, u16) -> Vec<SocketAddr> {
    let answer: Vec<SocketAddr> = answer.iter().map(|it| at(it)).collect();
    move |_: &str, _: u16| {
        asked.set(asked.get() + 1);
        answer.clone()
    }
}

#[test]
fn a_name_is_connected_to_only_at_the_public_addresses_it_resolved_to_once() {
    let asked = Cell::new(0);
    let resolve = resolving(
        &[
            "10.0.0.5:443",
            "93.184.215.14:443",
            "127.0.0.1:443",
            "[2606:4700:4700::1111]:443",
            "[fd00:ec2::254]:443",
        ],
        &asked,
    );
    assert_eq!(
        route("example.com", 443, None, &[], &resolve),
        Route::To(vec![
            at("93.184.215.14:443"),
            at("[2606:4700:4700::1111]:443")
        ])
    );
    assert_eq!(
        asked.get(),
        1,
        "looked up once, and connected where it was checked"
    );
}

#[test]
fn a_name_that_resolves_only_to_this_machine_or_a_private_network_is_refused() {
    for answer in [
        &["127.0.0.1:5173"][..],
        &["[::1]:5173"],
        &["10.0.0.5:5173", "192.168.1.1:5173"],
        &["169.254.169.254:5173"],
        &["[::ffff:127.0.0.1]:5173"],
    ] {
        let asked = Cell::new(0);
        assert_eq!(
            route(
                "rebound.example.com",
                5173,
                None,
                &[],
                &resolving(answer, &asked)
            ),
            Route::NotPublic,
            "{answer:?}"
        );
    }
}

#[test]
fn a_name_that_resolves_to_nothing_is_not_found() {
    let asked = Cell::new(0);
    assert_eq!(
        route(
            "nowhere.example.com",
            443,
            None,
            &[],
            &resolving(&[], &asked)
        ),
        Route::NotFound
    );
}

#[test]
fn an_address_is_connected_to_as_itself_and_never_looked_up() {
    let asked = Cell::new(0);
    let resolve = resolving(&["93.184.215.14:443"], &asked);
    assert_eq!(
        route("127.0.0.1", 5173, Some(at("127.0.0.1:5173")), &[], &resolve),
        Route::To(vec![at("127.0.0.1:5173")])
    );
    assert_eq!(asked.get(), 0);
}

#[test]
fn a_new_site_is_told_to_whoever_hears_new_sites_and_refused_by_default() {
    let site = Site {
        host: "new.example.net".to_owned(),
        port: 443,
    };
    assert!(!NewSites::refusing().carries(&site));
    let heard = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let hearing = std::sync::Arc::clone(&heard);
    let sites = NewSites::new(move |site: &Site| {
        hearing.lock().unwrap().push(site.clone());
        site.host == "new.example.net"
    });
    assert!(sites.carries(&site));
    assert!(!sites.carries(&Site {
        host: "other.example.net".to_owned(),
        port: 443
    }));
    assert_eq!(heard.lock().unwrap().len(), 2);
}

/// A chat's reach naming private addresses each way a list could: exactly an address and port,
/// an address with no port, ranges, and link-local and metadata addresses exactly.
fn private_reach() -> Reach {
    Reach::of(
        [
            ("10.0.0.5:6443", By::You),
            ("[fd12:3456:789a::1]:8443", By::Chat),
            ("100.64.0.7:22", By::Chat),
            ("10.0.0.6", By::You),
            ("10.0.0.0/8", By::You),
            ("10.0.0.0/8:443", By::You),
            ("169.254.169.254:80", By::You),
            ("[fd00:ec2::254]:80", By::You),
            ("[::ffff:10.0.0.9]:443", By::You),
            ("cluster.example.com:6443", By::You),
        ]
        .into_iter()
        .map(|(host, by)| (host.to_owned(), by))
        .collect(),
    )
}

#[test]
fn a_private_address_is_carried_by_the_literal_address_and_port_the_person_allowed() {
    let lane = Lane::of(private_reach());
    for (host, port, by, to) in [
        ("10.0.0.5", 6443, By::You, "10.0.0.5:6443"),
        (
            "fd12:3456:789a::1",
            8443,
            By::Chat,
            "[fd12:3456:789a::1]:8443",
        ),
        ("100.64.0.7", 22, By::Chat, "100.64.0.7:22"),
    ] {
        for tunnel in [true, false] {
            assert_eq!(
                lane.decide(host, port, tunnel, &[]),
                carried(Decision::Allowed(by), Some(to)),
                "{host}:{port}"
            );
        }
    }
}

#[test]
fn a_private_address_on_another_port_by_a_range_or_in_another_form_is_refused() {
    let lane = Lane::of(private_reach());
    for (host, port) in [
        ("10.0.0.5", 443),
        ("10.0.0.5", 6444),
        ("10.0.0.6", 443),
        ("10.0.0.6", 80),
        ("10.0.0.1", 443),
        ("10.0.0.0", 443),
        ("::ffff:10.0.0.5", 6443),
        ("::ffff:a00:5", 6443),
        ("64:ff9b::a00:5", 6443),
        ("2002:a00:5::1", 6443),
        ("::ffff:10.0.0.9", 443),
        ("10.0.0.9", 443),
        ("169.254.169.254", 80),
        ("fd00:ec2::254", 80),
        ("::ffff:169.254.169.254", 80),
    ] {
        assert_eq!(
            lane.decide(host, port, port != 80, &[]),
            Verdict::Refused(Refusal::NotPublic),
            "{host}:{port}"
        );
    }
}

#[test]
fn a_private_address_of_this_machine_is_refused_though_the_person_allowed_it() {
    let lane = Lane::of(private_reach());
    assert_eq!(
        lane.decide("10.0.0.5", 6443, true, &[ip("10.0.0.5")]),
        Verdict::Refused(Refusal::ThisMachine)
    );
}

#[test]
fn a_name_for_an_allowed_private_address_is_refused() {
    let lane = Lane::of(private_reach());
    // Listed by name with its port, it is decided as a name ...
    let Verdict::Carry { at: None, .. } = lane.decide("cluster.example.com", 6443, true, &[])
    else {
        panic!("a listed name is carried to what it resolves to, judged then");
    };
    // ... and what it resolves to is judged as a name's: a private address is never reached
    // by a name, even one the person allowed by its address.
    let asked = Cell::new(0);
    assert_eq!(
        route(
            "cluster.example.com",
            6443,
            None,
            &[],
            &resolving(&["10.0.0.5:6443"], &asked)
        ),
        Route::NotPublic
    );
}

#[test]
fn nothing_is_carried_where_this_machines_addresses_cannot_be_read() {
    let lane = Lane::of(chats_reach());
    for own in [None, Some(&[][..])] {
        for (host, port) in [
            ("example.com", 443),
            ("127.0.0.1", 5173),
            ("new.example.net", 443),
        ] {
            assert_eq!(
                lane.decide_here(host, port, true, own),
                Verdict::Refused(Refusal::ThisMachineUnknown),
                "{host}:{port}"
            );
        }
    }
    assert_eq!(
        lane.decide_here("example.com", 443, true, Some(&[ip("127.0.0.1")])),
        carried(Decision::Open, None)
    );
}

#[test]
fn a_lookup_that_takes_too_long_is_given_up_on() {
    let started = Instant::now();
    let found = lookup_within(Duration::from_millis(100), || {
        std::thread::sleep(Duration::from_secs(3));
        vec![at("93.184.215.14:443")]
    });
    assert_eq!(found, None);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(
        lookup_within(Duration::from_secs(5), || vec![at("93.184.215.14:443")]),
        Some(vec![at("93.184.215.14:443")])
    );
}

#[test]
fn a_new_site_is_named_as_one_spelling() {
    assert_eq!(
        Site::of("New.Example.NET.", 443),
        Site {
            host: "new.example.net".to_owned(),
            port: 443
        }
    );
}

#[test]
fn a_browser_never_takes_the_floors_reach_of_a_chat_without_the_sandbox() {
    let lane = Lane::of(Reach::anything());
    for (host, port) in [
        ("10.0.0.5", 6443),
        ("10.0.0.5", 22),
        ("192.168.1.1", 80),
        ("100.64.0.7", 22),
        ("100.100.1.2", 443),
        ("fd12:3456:789a::1", 8443),
    ] {
        assert_eq!(
            lane.decide(host, port, port != 80, &[]),
            Verdict::Refused(Refusal::NotPublic),
            "{host}:{port}"
        );
    }
    assert_eq!(
        lane.decide("127.0.0.1", 5173, true, &[]),
        Verdict::Refused(Refusal::ThisMachine)
    );
    for (host, port, tunnel) in [("new.example.net", 443, true), ("example.com", 80, false)] {
        assert_eq!(
            lane.decide(host, port, tunnel, &[]),
            Verdict::Site { at: None },
            "{host}"
        );
    }
}

#[test]
fn a_browser_of_a_chat_without_the_sandbox_takes_only_what_the_person_allowed() {
    let lane = Lane::of(
        Reach::anything().and(
            [
                ("10.0.0.5:6443", By::You),
                ("127.0.0.1:5173", By::Chat),
                ("example.com", By::Open),
            ]
            .into_iter()
            .map(|(host, by)| (host.to_owned(), by)),
        ),
    );
    assert_eq!(
        lane.decide("10.0.0.5", 6443, true, &[]),
        carried(Decision::Allowed(By::You), Some("10.0.0.5:6443"))
    );
    assert_eq!(
        lane.decide("127.0.0.1", 5173, true, &[]),
        carried(Decision::Allowed(By::Chat), Some("127.0.0.1:5173"))
    );
    assert_eq!(
        lane.decide("example.com", 443, true, &[]),
        carried(Decision::Open, None)
    );
    for (host, port) in [("10.0.0.5", 22), ("10.0.0.6", 6443), ("192.168.1.1", 80)] {
        assert_eq!(
            lane.decide(host, port, port != 80, &[]),
            Verdict::Refused(Refusal::NotPublic),
            "{host}:{port}"
        );
    }
    assert_eq!(
        lane.decide("new.example.net", 443, true, &[]),
        Verdict::Site { at: None }
    );
}

#[test]
fn a_lookup_past_the_number_running_at_once_is_refused_at_once() {
    static RUNNING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let release = std::sync::Arc::new(std::sync::Barrier::new(3));
    for _ in 0..2 {
        let waiting = std::sync::Arc::clone(&release);
        let held = lookup_bounded(&RUNNING, 2, Duration::from_millis(10), move || {
            waiting.wait();
            Vec::new()
        });
        assert_eq!(held, None, "given up on, still running");
    }
    let started = Instant::now();
    assert_eq!(
        lookup_bounded(&RUNNING, 2, Duration::from_secs(5), || vec![at(
            "93.184.215.14:443"
        )]),
        None
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    release.wait();
    let until = Instant::now() + Duration::from_secs(5);
    while RUNNING.load(std::sync::atomic::Ordering::SeqCst) > 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        lookup_bounded(&RUNNING, 2, Duration::from_secs(5), || vec![at(
            "93.184.215.14:443"
        )]),
        Some(vec![at("93.184.215.14:443")])
    );
}

#[test]
fn a_site_is_written_host_colon_port_and_read_back_the_same() {
    for (written, host, port) in [
        ("news.example.org:443", "news.example.org", 443),
        ("News.Example.ORG.:8080", "news.example.org", 8080),
        ("[2606:4700::1111]:443", "2606:4700::1111", 443),
        ("10.0.0.5:6443", "10.0.0.5", 6443),
    ] {
        let site = Site::parse(written).unwrap_or_else(|why| panic!("{written}: {why}"));
        assert_eq!((site.host.as_str(), site.port), (host, port), "{written}");
        assert_eq!(Site::parse(&site.to_string()), Ok(site));
    }
    assert_eq!(
        Site::parse("[2606:4700::1111]:443").unwrap().to_string(),
        "[2606:4700::1111]:443"
    );
    for refused in [
        "news.example.org",
        "news.example.org:0",
        "news.example.org:70000",
        ":443",
        "a b.example:443",
        "2606:4700::1111:443x",
        "[not-v6]:443",
    ] {
        assert!(Site::parse(refused).is_err(), "{refused}");
    }
}
