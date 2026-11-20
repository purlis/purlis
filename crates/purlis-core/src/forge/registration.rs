// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis's own forge registrations, compiled in (ADR 0077 §1, as amended 2026-10-12).
//!
//! A **forge registration** is what a host knows purlis by when a person signs in: an OAuth app
//! on github.com, an OAuth application on gitlab.com. Each is named by its **client id**, which
//! is public. The operator registered both on 2026-10-13 (issue 726), under the purlis
//! organisation and the purlis group.
//!
//! **Nothing here is secret, and nothing secret belongs here.** Neither registration has a
//! client secret purlis uses: GitHub's device flow needs none and none was generated, and the
//! gitlab.com application is not confidential and signs in with PKCE. A changed client id is a
//! release.
//!
//! **A registration exists on one host only.** [`on`] answers for the exact forge and host a
//! sign-in names, so purlis's github.com client id is never sent to a GitHub Enterprise server,
//! nor its gitlab.com one to a self-managed GitLab. Those hosts take a token (ADR 0077 §4).

use super::Kind;

/// How a person signs in through a registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// The OAuth device flow: a code shown in the window, confirmed in the browser.
    Device,
    /// The authorization code flow with PKCE and a loopback redirect.
    Pkce,
}

/// One registration: the forge, the one host it exists on, its public client id, and its flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registration {
    pub kind: Kind,
    /// The host the registration was made on, lower-case.
    pub host: &'static str,
    /// The public client id the host knows purlis by.
    pub client_id: &'static str,
    pub flow: Flow,
}

impl Registration {
    /// The scopes a sign-in through this registration asks for: exactly what a pasted token
    /// needs on the same forge ([`super::accounts::needed`]), `repo read:org` on GitHub and
    /// `api` on GitLab.
    pub fn scopes(&self) -> &'static [&'static str] {
        super::accounts::needed(self.kind)
    }
}

/// Every registration purlis ships with.
pub const TABLE: [Registration; 2] = [
    Registration {
        kind: Kind::GitHub,
        host: "github.com",
        client_id: "Ov23litEn94I5HXrpdsX",
        flow: Flow::Device,
    },
    Registration {
        kind: Kind::GitLab,
        host: "gitlab.com",
        client_id: "2a9cd8aa4db16d865e0e7cfa31a71bf04cf019e3d40be52c924bd6add0bc6c89",
        flow: Flow::Pkce,
    },
];

/// The loopback ports the gitlab.com application registered its redirect on. The listener binds
/// the first that is unused (ADR 0077 §1).
pub const REDIRECT_PORTS: [u16; 3] = [47823, 47824, 47825];

/// The path of the loopback redirect, as registered (issue 726, corrected 2026-10-13).
pub const REDIRECT_PATH: &str = "/purlis/signin";

/// The loopback redirect address on `port`, exactly as the application registered it.
pub fn redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}{REDIRECT_PATH}")
}

/// purlis's registration for `kind` on `host`, or `None` when it has none there: the host is
/// compared whole, without regard to case, and never by its ending.
pub fn on(kind: Kind, host: &str) -> Option<&'static Registration> {
    TABLE
        .iter()
        .find(|r| r.kind == kind && r.host.eq_ignore_ascii_case(host))
}

/// How a store of pinned client ids answers: the client id kept for `kind` on `host`, if any.
pub type PinLookup = fn(Kind, &str) -> Option<String>;

static PINS: std::sync::RwLock<Option<PinLookup>> = std::sync::RwLock::new(None);

/// Give the pinned client ids' store to this process: the registration of a self-hosted server,
/// kept per host. The store is slice G's of one login, which calls this as it starts; with none
/// given, no host has a pin.
pub fn use_pins(lookup: PinLookup) {
    *PINS.write().unwrap_or_else(|e| e.into_inner()) = Some(lookup);
}

/// The client id the person pinned for `kind` on `host`, from the store [`use_pins`] gave. The
/// readiness check asks it, so a self-hosted server with a pinned client id is not told it has
/// no registration.
pub fn pinned_client_id(kind: Kind, host: &str) -> Option<String> {
    let lookup = (*PINS.read().unwrap_or_else(|e| e.into_inner()))?;
    lookup(kind, host)
}

/// Whether purlis can sign in on `host`: it ships a registration there ([`on`]) or the person
/// pinned a client id for it ([`pinned_client_id`]).
pub fn known_on(kind: Kind, host: &str) -> bool {
    on(kind, host).is_some() || pinned_client_id(kind, host).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_com_signs_in_by_the_device_flow_and_gitlab_com_by_pkce() {
        let github = on(Kind::GitHub, "github.com").unwrap();
        assert_eq!(github.flow, Flow::Device);
        assert_eq!(github.client_id, "Ov23litEn94I5HXrpdsX");
        assert_eq!(github.scopes(), ["repo", "read:org"]);
        let gitlab = on(Kind::GitLab, "GitLab.com").unwrap();
        assert_eq!(gitlab.flow, Flow::Pkce);
        assert_eq!(gitlab.client_id.len(), 64);
        assert_eq!(gitlab.scopes(), ["api"]);
    }

    #[test]
    fn a_registration_is_never_used_on_another_host_or_for_the_other_forge() {
        for (kind, host) in [
            (Kind::GitHub, "ghe.example.com"),
            (Kind::GitHub, "evil-github.com"),
            (Kind::GitHub, "github.com.example.com"),
            (Kind::GitHub, "api.github.com"),
            (Kind::GitHub, "gitlab.com"),
            (Kind::GitLab, "github.com"),
            (Kind::GitLab, "gitlab.example.com"),
            (Kind::GitHub, ""),
        ] {
            assert_eq!(on(kind, host), None, "{kind:?} on {host:?}");
        }
    }

    #[test]
    fn each_host_has_one_registration_and_a_client_id_with_nothing_but_letters_and_digits() {
        for (at, r) in TABLE.iter().enumerate() {
            assert!(r.client_id.bytes().all(|b| b.is_ascii_alphanumeric()));
            assert_eq!(r.host, r.host.to_ascii_lowercase());
            assert!(
                TABLE[..at]
                    .iter()
                    .all(|before| (before.kind, before.host) != (r.kind, r.host))
            );
        }
    }

    #[test]
    fn the_loopback_redirect_is_spelled_as_it_was_registered() {
        assert_eq!(
            REDIRECT_PORTS.map(redirect_uri),
            [
                "http://127.0.0.1:47823/purlis/signin",
                "http://127.0.0.1:47824/purlis/signin",
                "http://127.0.0.1:47825/purlis/signin",
            ]
        );
    }
}
