// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

use purlis_browser_engine::drive::{Driver, Gate, Outbound, Why};
use purlis_host::protocol::browsing::BrowserReach;
use purlis_host::protocol::session::{Layer, Listed};

use super::{SiteJudge, site_of};
use crate::sandbox::browser_reach::Site;

/// This machine, as the tests say it: one address on a private network.
fn own() -> Vec<IpAddr> {
    vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20))]
}

fn reach(hosts: &[&str], open_web: bool) -> BrowserReach {
    BrowserReach {
        listed: hosts
            .iter()
            .map(|host| Listed {
                host: (*host).to_owned(),
                layer: Layer::Open,
            })
            .collect(),
        open_web,
        ..BrowserReach::default()
    }
}

fn sites(hosts: &[&str], open_web: bool) -> Arc<SiteJudge> {
    Arc::new(
        SiteJudge::of(&reach(hosts, open_web))
            .unwrap()
            .knowing_this_machine(own()),
    )
}

fn out(address: &str, why: Why) -> Outbound {
    Outbound {
        address: address.to_owned(),
        method: "GET".to_owned(),
        why,
        by: Driver::Agent,
    }
}

fn ask(site: &str) -> Gate {
    Gate::Ask {
        site: site.to_owned(),
    }
}

#[test]
fn a_site_the_chat_lists_goes_whatever_is_sent_to_it() {
    let sites = sites(&["docs.example.com"], false);
    for why in [Why::Reads, Why::Addressed, Why::Sends] {
        assert_eq!(
            sites.judge(&out("https://docs.example.com/guide?q=1", why)),
            Gate::Go
        );
    }
}

#[test]
fn a_new_site_the_page_reads_goes_and_its_proxy_carries_it() {
    let sites = sites(&["docs.example.com"], false);
    let carried = sites.new_sites();
    assert!(!carried.carries(&Site::of("cdn.example.net", 443)));
    assert_eq!(
        sites.judge(&out("https://cdn.example.net/logo.png", Why::Reads)),
        Gate::Go
    );
    assert!(carried.carries(&Site::of("cdn.example.net", 443)));
    assert!(
        !carried.carries(&Site::of("cdn.example.net", 80)),
        "by its host and the port it was read on"
    );
}

#[test]
fn an_address_the_agent_writes_to_a_new_site_asks_and_is_not_carried() {
    let sites = sites(&["docs.example.com"], false);
    assert_eq!(
        sites.judge(&out("https://news.example.org/?leak=1", Why::Addressed)),
        ask("news.example.org:443")
    );
    assert_eq!(
        sites.judge(&out("http://news.example.org:8080/", Why::Addressed)),
        ask("news.example.org:8080")
    );
    assert!(
        !sites
            .new_sites()
            .carries(&Site::of("news.example.org", 443))
    );
}

#[test]
fn sending_to_a_new_site_asks_though_the_page_read_it_before() {
    let sites = sites(&["docs.example.com"], false);
    assert_eq!(
        sites.judge(&out("https://forms.example.org/", Why::Reads)),
        Gate::Go
    );
    assert_eq!(
        sites.judge(&out("https://forms.example.org/submit", Why::Sends)),
        ask("forms.example.org:443")
    );
}

#[test]
fn what_the_person_does_while_they_drive_goes_and_is_carried() {
    let sites = sites(&[], false);
    let mut theirs = out("https://forms.example.org/submit", Why::Sends);
    theirs.by = Driver::Person;
    assert_eq!(sites.judge(&theirs), Gate::Go);
    assert!(
        sites
            .new_sites()
            .carries(&Site::of("forms.example.org", 443))
    );
}

#[test]
fn on_the_open_web_nothing_asks_and_every_public_site_is_carried() {
    let sites = sites(&[], true);
    for why in [Why::Reads, Why::Addressed, Why::Sends] {
        assert_eq!(
            sites.judge(&out("https://news.example.org/", why)),
            Gate::Go
        );
    }
    assert!(
        sites
            .new_sites()
            .carries(&Site::of("elsewhere.example.org", 443))
    );
}

#[test]
fn a_site_allowed_since_the_browser_started_goes_and_is_carried() {
    let sites = sites(&["docs.example.com"], false);
    sites
        .reach_now(&reach(&["docs.example.com", "news.example.org"], false))
        .unwrap();
    assert_eq!(
        sites.judge(&out("https://news.example.org/", Why::Addressed)),
        Gate::Go
    );
    assert!(
        sites
            .new_sites()
            .carries(&Site::of("news.example.org", 443))
    );
}

#[test]
fn a_reach_that_does_not_read_is_refused_and_the_reach_before_stands() {
    let sites = sites(&["docs.example.com"], false);
    let unreadable = BrowserReach {
        never: vec!["not a host at all".into()],
        ..BrowserReach::default()
    };
    assert!(sites.reach_now(&unreadable).is_err());
    assert_eq!(
        sites.judge(&out("https://docs.example.com/", Why::Sends)),
        Gate::Go
    );
}

#[test]
fn what_is_never_a_site_asks_nothing_and_is_carried_by_nothing_it_did() {
    let sites = sites(&[], false);
    // Its proxy refuses each: there is nothing to ask the person about.
    for address in [
        "http://127.0.0.1:5173/",
        "http://10.0.0.5/",
        "http://localhost:3000/",
        "about:blank",
        "data:text/plain,hello",
    ] {
        assert_eq!(sites.judge(&out(address, Why::Addressed)), Gate::Go);
    }
    assert!(!sites.new_sites().carries(&Site::of("10.0.0.5", 80)));
}

#[test]
fn a_site_is_its_host_and_port_as_the_engine_writes_the_address() {
    assert_eq!(
        site_of("https://News.Example.org./a#b"),
        Some(("news.example.org".to_owned(), 443))
    );
    assert_eq!(
        site_of("http://example.org:8080?x"),
        Some(("example.org".to_owned(), 8080))
    );
    assert_eq!(
        site_of("https://user@example.org/"),
        Some(("example.org".to_owned(), 443))
    );
    assert_eq!(
        site_of("https://[2606:4700::1111]/"),
        Some(("2606:4700::1111".to_owned(), 443))
    );
    assert_eq!(site_of("about:blank"), None);
    assert_eq!(site_of("https:///nohost"), None);
    assert_eq!(site_of("https://example.org:notaport/"), None);
}

// ---- A login purlis performs (#2016) ----

fn by(address: &str, why: Why, by: Driver) -> Outbound {
    Outbound {
        by,
        ..out(address, why)
    }
}

#[test]
fn what_purlis_sends_to_its_logins_site_goes_and_nothing_else_it_sends_does() {
    let sites = sites(&["docs.example.com"], false);
    sites.logging_in(&Site::of("shop.example.test", 443));
    for why in [Why::Addressed, Why::Sends] {
        assert_eq!(
            sites.judge(&by("https://shop.example.test/login", why, Driver::Purlis)),
            Gate::Go
        );
    }
    assert!(
        sites
            .new_sites()
            .carries(&Site::of("shop.example.test", 443))
    );
    // A page purlis drives that sends elsewhere is judged as the agent's would be.
    assert_eq!(
        sites.judge(&by(
            "https://news.example.org/?leak=1",
            Why::Sends,
            Driver::Purlis
        )),
        ask("news.example.org:443")
    );
    // The agent's own address to the login's site, before anything is bound, is the agent's.
    assert_eq!(
        sites.judge(&by(
            "https://shop.example.test/?q=1",
            Why::Addressed,
            Driver::Agent
        )),
        ask("shop.example.test:443")
    );
}

#[test]
fn a_browser_holding_a_login_asks_about_no_other_site() {
    let sites = sites(&["docs.example.com"], false);
    sites
        .bound()
        .to(&Site::of("shop.example.test", 443))
        .unwrap();
    // Refused at the proxy, in words; never an Ask an Allow could answer.
    assert_eq!(
        sites.judge(&out("https://news.example.org/?leak=1", Why::Addressed)),
        Gate::Go
    );
    assert!(!sites.bound().lets("news.example.org", 443));
    assert!(!sites.bound().lets("docs.example.com", 443));
    assert!(sites.bound().lets("shop.example.test", 443));
    assert!(sites.bound().lets("Shop.Example.Test.", 443));
    assert!(!sites.bound().lets("shop.example.test", 80));
}

#[test]
fn a_browser_bound_to_one_login_is_not_bound_to_another() {
    let sites = sites(&[], false);
    let bound = sites.bound();
    assert!(
        bound.lets("anything.example", 443),
        "unbound, nothing is refused here"
    );
    bound.to(&Site::of("shop.example.test", 443)).unwrap();
    bound.to(&Site::of("shop.example.test", 443)).unwrap();
    let refused = bound.to(&Site::of("mail.example.test", 443)).unwrap_err();
    assert!(
        refused.contains("holds a login at shop.example.test:443"),
        "{refused}"
    );
    assert!(!bound.lets("mail.example.test", 443));
}
