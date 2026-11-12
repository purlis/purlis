// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

use purlis_browser_engine::drive::{Driver, Gate, Leaving, Outbound, Why};
use purlis_host::protocol::browsing::BrowserReach;
use purlis_host::protocol::session::{Layer, Listed};

use super::{SiteJudge, site_of};
use crate::sandbox::browser_reach::{LoginSites, Site};

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
    sites.logging_in(&[Site::of("shop.example.test", 443)]);
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
        .to(&LoginSites::page(Site::of("shop.example.test", 443)))
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

/// #5152: a browser holding a login whose form is on another site sends to its sign-in site
/// without asking, the agent driving or not: the person allowed it with the login. A page's token
/// refresh to its sign-in site goes once the sign-in is over. Its page's site is judged as before:
/// what the agent sends there asks where nothing lists it.
#[test]
fn a_browser_holding_a_login_sends_to_its_sign_in_site_without_asking() {
    let sites = sites(&[], false);
    let number = sites
        .bound()
        .to(&LoginSites {
            page: Site::of("app.example.test", 443),
            form: Some(Site::of("auth.example.test", 443)),
        })
        .unwrap()
        .expect("a sign-in opens");
    sites.bound().sign_in_over(number);
    for why in [Why::Sends, Why::Addressed, Why::Reads] {
        assert_eq!(
            sites.judge(&out("https://auth.example.test/token?grant=refresh", why)),
            Gate::Go,
            "{why:?}"
        );
    }
    assert_eq!(
        sites.judge(&out("https://app.example.test/api?q=1", Why::Sends)),
        ask("app.example.test:443")
    );
}

#[test]
fn a_browser_bound_to_one_login_is_not_bound_to_another() {
    let sites = sites(&[], false);
    let bound = sites.bound();
    assert!(
        bound.lets("anything.example", 443),
        "unbound, nothing is refused here"
    );
    let shop = LoginSites::page(Site::of("shop.example.test", 443));
    bound.to(&shop).unwrap();
    bound.to(&shop).unwrap();
    let refused = bound
        .to(&LoginSites::page(Site::of("mail.example.test", 443)))
        .unwrap_err();
    assert!(
        refused.contains("holds a login at shop.example.test:443"),
        "{refused}"
    );
    assert!(!bound.lets("mail.example.test", 443));
}

/// #5152, the person's ruling of 2026-11-04 (amending #4995's of 2026-11-03): a login whose form
/// is on another site reaches it from then on, and keeps the agent off it outside a sign-in. A
/// later login of the same page signs in anew; a sign-in over twice, or one another replaced,
/// changes nothing.
#[test]
fn a_login_whose_form_is_elsewhere_reaches_it_and_keeps_the_agent_off_it_outside_a_sign_in() {
    let sites = sites(&[], false);
    let bound = sites.bound();
    let app = Site::of("app.example.test", 443);
    let auth = Site::of("auth.example.test", 443);
    let login = LoginSites {
        page: app.clone(),
        form: Some(auth.clone()),
    };
    let first = bound.to(&login).unwrap().expect("a sign-in opens");
    assert!(bound.lets("auth.example.test", 443));
    assert!(bound.lets("app.example.test", 443));
    assert_eq!(bound.sites(), Some(vec![auth.clone(), app.clone()]));
    assert_eq!(bound.form(), Some(auth.clone()));
    assert_eq!(bound.kept_off("auth.example.test", 443), None, "signing in");
    assert_eq!(bound.sign_in_over(first), Some(auth.clone()));
    assert!(
        bound.lets("auth.example.test", 443),
        "over: the form's site is still reached"
    );
    assert_eq!(
        bound.kept_off("auth.example.test", 443),
        Some(auth.clone()),
        "over: the agent is kept off it"
    );
    assert_eq!(bound.kept_off("app.example.test", 443), None);
    assert_eq!(
        bound.kept_off("auth.example.test", 8443),
        None,
        "by its port too"
    );
    assert_eq!(
        bound.kept_off("auth.example.test", 80),
        Some(auth.clone()),
        "its plain web port is the same site"
    );
    assert_eq!(bound.sites(), Some(vec![auth.clone(), app.clone()]));
    assert_eq!(bound.sign_in_over(first), None, "over once");
    // A later login of the same page signs in anew, and only it ends its own.
    let second = bound.to(&login).unwrap().expect("a sign-in opens again");
    assert_ne!(second, first);
    assert_eq!(
        bound.sign_in_over(first),
        None,
        "an old sign-in ends no newer one"
    );
    assert_eq!(bound.kept_off("auth.example.test", 443), None);
    // Another form's site for the same page is refused: a browser signs in on one.
    let refused = bound
        .to(&LoginSites {
            page: app.clone(),
            form: Some(Site::of("login.other.test", 443)),
        })
        .unwrap_err();
    assert!(refused.contains("on auth.example.test:443"), "{refused}");
    assert!(!bound.lets("login.other.test", 443));
    let third = bound.to(&login).unwrap();
    assert_eq!(bound.sign_in_over(second), None, "replaced by the third");
    assert_eq!(bound.sign_in_over(third.unwrap()), Some(auth.clone()));
    // Another page's login is refused, whatever its form.
    let refused = bound
        .to(&LoginSites {
            page: auth.clone(),
            form: None,
        })
        .unwrap_err();
    assert!(
        refused.contains("holds a login at app.example.test:443"),
        "{refused}"
    );
    assert!(!bound.lets("other.example.test", 443));
}

/// #5152: a browser held to a login's page alone, as one opened from kept logins is, keeps the
/// agent off nothing, and reaches no other site, until a login of its page signs in elsewhere.
#[test]
fn a_browser_held_to_its_page_alone_keeps_the_agent_off_nothing() {
    let sites = sites(&[], false);
    let bound = sites.bound();
    assert_eq!(bound.kept_off("auth.example.test", 443), None, "unbound");
    bound
        .to(&LoginSites::page(Site::of("app.example.test", 443)))
        .unwrap();
    assert_eq!(bound.form(), None);
    assert_eq!(bound.kept_off("auth.example.test", 443), None);
    assert!(!bound.lets("auth.example.test", 443));
}

#[test]
fn a_logins_site_is_let_on_for_good_only_once_the_browser_is_bound_to_it() {
    let shop = Site::of("shop.example.test", 443);
    let sites = sites(&["docs.example.com"], false);
    sites.logging_in(std::slice::from_ref(&shop));
    assert!(sites.new_sites().carries(&shop), "while the login runs");
    sites.done_logging_in();
    assert!(
        !sites.new_sites().carries(&shop),
        "a login that never filled lets nothing on"
    );
    assert_eq!(
        sites.judge(&out("https://shop.example.test/?q=1", Why::Addressed)),
        ask("shop.example.test:443")
    );
    sites.logging_in(std::slice::from_ref(&shop));
    sites.bound_to(&shop);
    sites.done_logging_in();
    assert!(
        sites.new_sites().carries(&shop),
        "bound to it, the browser reaches it"
    );
}

/// #5230, SF-14: a site the browser was on continues by itself; any other asks once and stays
/// once asked; what the person allowed goes; never outside the usual reach.
#[test]
fn a_filled_page_leaves_for_a_site_it_was_on_and_asks_once_about_any_other() {
    let address = "https://app.example.com/?code=1";
    let site = "app.example.com:443";

    let judge = sites(&["app.example.com"], false);
    assert_eq!(judge.may_leave_for(address, true), Leaving::Go);

    // Listed, but not been on: it asks, once.
    let judge = sites(&["app.example.com"], false);
    assert_eq!(
        judge.may_leave_for(address, false),
        Leaving::Ask {
            site: site.to_owned()
        }
    );
    assert!(matches!(
        judge.may_leave_for(address, false),
        Leaving::No { .. }
    ));

    // The open web asks too, and goes once the person allowed it.
    let judge = sites(&[], true);
    assert!(matches!(
        judge.may_leave_for(address, false),
        Leaving::Ask { .. }
    ));
    let mut allowed = reach(&["app.example.com"], true);
    allowed.answered = vec![site.to_owned()];
    judge.reach_now(&allowed).unwrap();
    assert_eq!(judge.may_leave_for(address, false), Leaving::Go);

    // A site it was on, that the reach no longer carries, is asked about.
    let judge = sites(&[], false);
    assert!(matches!(
        judge.may_leave_for(address, true),
        Leaving::Ask { .. }
    ));

    // A private address is outside the reach, whoever says what.
    let judge = sites(&[], true);
    assert!(matches!(
        judge.may_leave_for("https://10.0.0.5/", true),
        Leaving::No { .. }
    ));
}

/// The person's ruling of 2026-11-06: only an answer to the ask about a filled page leaving
/// counts. An Allow of the site from before (an ordinary ask) is no answer to it, until the
/// window has seen the ask and the person allows the site again.
#[test]
fn a_filled_page_leaves_for_a_site_allowed_before_only_once_the_person_answers_its_ask() {
    let address = "https://app.example.com/?code=1";
    let site = "app.example.com:443";
    let mut allowed = reach(&["app.example.com"], false);
    allowed.answered = vec![site.to_owned()];
    let judge = Arc::new(SiteJudge::of(&allowed).unwrap().knowing_this_machine(own()));

    assert_eq!(
        judge.may_leave_for(address, false),
        Leaving::Ask {
            site: site.to_owned()
        },
        "allowed for an ordinary ask, it is asked about to leave"
    );
    // The next act's reach, from before the window saw the ask, holds the old Allow.
    judge.reach_now(&allowed).unwrap();
    assert!(matches!(
        judge.may_leave_for(address, false),
        Leaving::No { .. }
    ));
    // The window saw the ask, and the person has not answered it yet.
    judge
        .reach_now(&reach(&["app.example.com"], false))
        .unwrap();
    assert!(matches!(
        judge.may_leave_for(address, false),
        Leaving::No { .. }
    ));
    // They allow it now.
    judge.reach_now(&allowed).unwrap();
    assert_eq!(judge.may_leave_for(address, false), Leaving::Go);
}

/// #5230: a login's hold outlasts a fill's: a filled page leaves for no site it keeps off.
#[test]
fn a_filled_page_leaves_for_no_site_a_login_keeps_the_browser_off() {
    let judge = sites(&["app.example.com"], false);
    judge
        .bound()
        .to(&LoginSites {
            page: Site::of("shop.example.com", 443),
            form: None,
        })
        .unwrap();
    assert!(matches!(
        judge.may_leave_for("https://app.example.com/", true),
        Leaving::No { .. }
    ));
}

/// The person's ruling of 2026-11-06: during an open sign-in, a page held to the login's form's
/// site follows its server's redirect back to the login's page's site by itself, though the chain
/// began elsewhere; to any other site, from any other site, or once the sign-in is over, it asks
/// as before.
#[test]
fn during_a_sign_in_a_page_held_to_its_form_follows_its_redirect_back_to_the_login_page() {
    let app = "https://app.example.com/callback?code=1";
    let auth = Site::of("auth.example.com", 443);
    let judge = sites(&["app.example.com"], false);
    judge
        .bound()
        .to(&LoginSites {
            page: Site::of("app.example.com", 443),
            form: Some(auth.clone()),
        })
        .unwrap()
        .expect("a sign-in");
    judge.bound_to(&Site::of("app.example.com", 443));

    assert_eq!(judge.may_leave_from(&auth, app, false), Leaving::Go);
    // Not from the form's site: asked about as before.
    let other = Site::of("pay.example.com", 443);
    assert!(matches!(
        judge.may_leave_from(&other, app, false),
        Leaving::Ask { .. }
    ));

    // Once the sign-in is over, the way back asks again (it was asked about once already).
    let judge = sites(&["app.example.com", "auth.example.com"], false);
    let number_now = judge
        .bound()
        .to(&LoginSites {
            page: Site::of("app.example.com", 443),
            form: Some(auth.clone()),
        })
        .unwrap()
        .expect("a sign-in");
    judge.bound().sign_in_over(number_now).expect("it was open");
    assert!(matches!(
        judge.may_leave_from(&auth, app, false),
        Leaving::Ask { .. }
    ));
}

/// The person's ruling of 2026-11-06: the way back goes to the login's page's site alone.
#[test]
fn during_a_sign_in_a_page_held_to_its_form_still_asks_before_any_site_but_the_login_page() {
    let auth = Site::of("auth.example.com", 443);
    let judge = sites(&["app.example.com", "elsewhere.example.com"], false);
    judge
        .bound()
        .to(&LoginSites {
            page: Site::of("app.example.com", 443),
            form: Some(auth.clone()),
        })
        .unwrap()
        .expect("a sign-in");
    assert!(matches!(
        judge.may_leave_from(&auth, "https://elsewhere.example.com/", false),
        Leaving::No { .. }
    ));
}

/// #6291 (the operator's ruling of 2026-11-12): a redirect refused during a sign-in is said by
/// its site alone. The address, which may hold the code or the token the identity provider
/// handed back, never reaches the chat in the refusal, whichever way it is refused.
#[test]
fn a_refused_redirect_names_its_site_and_never_its_address() {
    let code = "c0de-from-the-provider";
    let auth = Site::of("auth.example.com", 443);
    let elsewhere = format!("https://elsewhere.example.com/cb?code={code}&state=s");
    let said = |leaving: Leaving| match leaving {
        Leaving::No { why } => why,
        Leaving::Ask { site } => site,
        Leaving::Go => panic!("it went"),
    };

    // Kept off by the login: the browser reaches its sites alone.
    let judge = sites(&["app.example.com", "elsewhere.example.com"], false);
    judge
        .bound()
        .to(&LoginSites {
            page: Site::of("app.example.com", 443),
            form: Some(auth.clone()),
        })
        .unwrap();
    let why = said(judge.may_leave_from(&auth, &elsewhere, false));
    assert!(!why.contains(code) && !why.contains("/cb"), "{why}");

    // Asked about, then refused once asked.
    let judge = sites(&["elsewhere.example.com"], false);
    for _ in 0..2 {
        let why = said(judge.may_leave_for(&elsewhere, false));
        assert!(!why.contains(code) && !why.contains("/cb"), "{why}");
    }

    // Outside what the browser reaches at all.
    let judge = sites(&[], false);
    let private = format!("https://10.0.0.5/cb?code={code}");
    let why = said(judge.may_leave_for(&private, true));
    assert!(!why.contains(code), "{why}");
}
