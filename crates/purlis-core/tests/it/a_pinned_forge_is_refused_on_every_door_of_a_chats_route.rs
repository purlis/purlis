// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice K4 (ADR 0128 D3g): an administrator's never-allow pin on a forge host refuses
//! that forge **entirely** for a chat, the route's listener included, and the pin is read on every
//! request. Run on socket pairs, so no port is bound.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::forge::pin::Pin;
use purlis_core::forge::proxy::{ConnectVerdict, Refusal};

use crate::support::forge_listener::{Answer, World, gh_get, plain};

/// The pin as policy answers it, with the hosts pinned changeable while a chat is open.
#[derive(Default)]
struct Pinned(Mutex<HashSet<String>>);

impl Pinned {
    fn set(&self, hosts: &[&str]) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) =
            hosts.iter().map(|h| (*h).to_owned()).collect();
    }
}

impl Pin for Pinned {
    fn pinned(&self, host: &str, _: u16) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(host)
            .then(|| {
                format!(
                    "Policy never allows {host}. Locked by policy, set by root in \
                     /etc/purlis/policy.toml. An administrator pinned this forge, so no chat \
                     reaches it and signing in does not change that."
                )
            })
    }
}

fn pinned(hosts: &[&str]) -> (World, Arc<Pinned>) {
    let pin = Arc::new(Pinned::default());
    pin.set(hosts);
    (World::with_pin(pin.clone()), pin)
}

#[test]
fn gh_over_the_socket_is_refused_with_the_pin_sentence_when_github_com_is_pinned() {
    purlis_core::unsteered!();
    let (w, _) = pinned(&["github.com"]);
    w.fake.answer(Answer::ok(r#"{"login":"octocat"}"#));
    let reply = plain(&w, &gh_get(&w, "/user"));
    assert_eq!(reply.status, 403, "{}", reply.text());
    let said = reply.text();
    assert!(said.contains("set by root"), "{said}");
    assert!(said.contains("signing in does not change"), "{said}");
    assert!(w.fake.requests().is_empty(), "a pinned forge was reached");
}

#[test]
fn a_raw_request_naming_a_recorded_host_is_refused_when_that_forge_is_pinned() {
    purlis_core::unsteered!();
    let (w, _) = pinned(&["github.com"]);
    for host in ["github.com", "api.github.com"] {
        let request = format!(
            "GET /repos/acme/app HTTP/1.1\r\nHost: {host}\r\nAuthorization: token {}\r\n\r\n",
            w.sentinel.expose()
        );
        let reply = plain(&w, request.as_bytes());
        assert_eq!(reply.status, 403, "{host}: {}", reply.text());
        assert!(
            reply.text().contains("signing in does not change"),
            "{host}: {}",
            reply.text()
        );
    }
    assert!(w.fake.requests().is_empty(), "a pinned forge was reached");
}

#[test]
fn the_pin_is_read_on_every_request_so_one_set_while_a_chat_is_open_reaches_it() {
    purlis_core::unsteered!();
    let (w, pin) = pinned(&[]);
    w.fake.answer(Answer::ok(r#"{"login":"octocat"}"#));
    assert_eq!(plain(&w, &gh_get(&w, "/user")).status, 200);
    pin.set(&["github.com"]);
    assert_eq!(plain(&w, &gh_get(&w, "/user")).status, 403);
    pin.set(&[]);
    w.fake.answer(Answer::ok(r#"{"login":"octocat"}"#));
    assert_eq!(plain(&w, &gh_get(&w, "/user")).status, 200);
}

#[test]
fn a_connect_to_a_pinned_forge_or_its_api_host_is_refused_and_a_gitlab_by_its_own_name() {
    purlis_core::unsteered!();
    let (w, pin) = pinned(&["github.com", "gitlab.com"]);
    for host in ["github.com", "api.github.com", "codeload.github.com", "gitlab.com"] {
        assert_eq!(
            w.proxy.connect(&w.chat, host, 443),
            ConnectVerdict::Refuse(Refusal::Pinned),
            "{host}"
        );
    }
    // Pinning one forge leaves the other.
    pin.set(&["github.com"]);
    assert!(matches!(
        w.proxy.connect(&w.chat, "gitlab.com", 443),
        ConnectVerdict::Terminate { .. }
    ));
}

#[test]
fn a_pinned_github_enterprise_server_is_refused_by_its_one_host() {
    purlis_core::unsteered!();
    use purlis_core::forge::proxy::{AllowedRepo, Kind};
    let pin = Arc::new(Pinned::default());
    pin.set(&["ghe.corp.example"]);
    let w = World::with_repos_and_pin(
        vec![AllowedRepo::new(Kind::GitHub, "ghe.corp.example", 443, "acme/app", "main").unwrap()],
        pin,
    );
    assert_eq!(
        w.proxy.connect(&w.chat, "ghe.corp.example", 443),
        ConnectVerdict::Refuse(Refusal::Pinned)
    );
    let alias = w.proxy.github_alias(&w.chat, "github.localhost");
    assert_eq!(alias.expect("an alias").unwrap_err(), Refusal::Pinned);
}
