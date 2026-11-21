// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An administrator's never-allow pin, as the forge route reads it** (ADR 0128 D3g).
//!
//! A forge host that policy pins as never allowed is refused for a chat **entirely**: the chat's
//! egress hand-off, the route's `CONNECT` door, `gh` on the route's unix socket and the app's own
//! git for the chat. Each asks [`Pin`] **on every connection or request**, so a pin an
//! administrator sets while a chat is open reaches it at once, and none keeps a copy.
//!
//! **The pin names a forge, not a spelling.** [`forge_host_of`] maps the names a route serves to
//! the forge they belong to (`api.github.com` and the download hosts are `github.com`), and
//! [`pinned_forge`] asks the pin about the name and about its forge, so a pin on `github.com`
//! covers the API host. A GitHub Enterprise Server and a GitLab are served at one host for git
//! and API alike, so the name is the forge there.
//!
//! **Fail closed:** a policy file purlis refuses (untrusted, malformed, or with a key this
//! version does not know) pins **every** forge, as [`Locks`] locks everything such a file could
//! lock; a pin is never read as absent because it could not be asked.
//!
//! **Tier** (ADR 0069): nothing is stored here; the pin is read from policy, in memory, per ask.

use std::path::Path;

use crate::sandbox::egress::host_and_port;
use crate::sandbox::hosts::Host;
use crate::sandbox::policy::Locks;

/// What says whether policy pins a host as never allowed.
pub trait Pin: Send + Sync {
    /// The sentence to refuse `host:port` with where policy pins it as never allowed, or `None`.
    /// `host` is one name; [`pinned_forge`] asks for each name a forge goes by.
    fn pinned(&self, host: &str, port: u16) -> Option<String>;
}

/// Nothing is pinned: for a run with no policy to read (a test, a stub).
pub struct NoPin;

impl Pin for NoPin {
    fn pinned(&self, _: &str, _: u16) -> Option<String> {
        None
    }
}

/// The policy's pin, read afresh on each ask ([`Locks::of`]).
pub struct PolicyPin {
    read: Box<dyn Fn() -> Locks + Send + Sync>,
}

impl PolicyPin {
    /// The pin of the project at `root`, with this machine's policy beside it.
    pub fn of(root: &Path) -> Self {
        let root = root.to_path_buf();
        Self::reading(move || Locks::of(&root))
    }

    /// The pin of whatever `read` answers each time.
    pub fn reading(read: impl Fn() -> Locks + Send + Sync + 'static) -> Self {
        Self {
            read: Box::new(read),
        }
    }
}

impl Pin for PolicyPin {
    fn pinned(&self, host: &str, port: u16) -> Option<String> {
        let locks = (self.read)();
        // A refused policy file locks everything it could lock, so it pins every forge: the
        // route is handed over before the egress layers that would otherwise lock the host.
        if locks.refused_because().is_some() {
            return Some(locks.led_by(
                "This forge is refused while the administrator's policy file is refused",
            ));
        }
        let host = host.strip_suffix('.').unwrap_or(host);
        let parsed = Host::parse(&host_and_port(host, port)).ok()?;
        // Who set it, and that nothing the person does here changes it.
        locks.never_allows(&parsed).map(|said| {
            format!(
                "{said} An administrator pinned this forge, so no chat reaches it and signing in \
                 does not change that."
            )
        })
    }
}

/// The forge a name belongs to, in lower case with no trailing dot: GitHub's API host and its
/// download hosts are `github.com`; any other name is itself. **The one place** a name is mapped
/// to its forge for the pin; the route's credential lookup maps the same names.
pub fn forge_host_of(host: &str) -> String {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    match host.as_str() {
        "api.github.com" => "github.com".to_owned(),
        other if crate::forgewire::DOWNLOAD_ONLY.contains(&other) => "github.com".to_owned(),
        _ => host,
    }
}

/// Whether `pin` pins `host:port` **or the forge it belongs to**, with its sentence.
pub fn pinned_forge(pin: &dyn Pin, host: &str, port: u16) -> Option<String> {
    let named = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let forge = forge_host_of(&named);
    pin.pinned(&named, port)
        .or_else(|| (forge != named).then(|| pin.pinned(&forge, port)).flatten())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pinning(hosts: &[&str]) -> PolicyPin {
        let list: Vec<String> = hosts.iter().map(|h| format!("\"{h}\"")).collect();
        let text = format!(
            "{{\"owner\": \"it-security\", \"sandbox\": {{\"never-hosts\": [{}]}}}}",
            list.join(", ")
        );
        PolicyPin::reading(move || Locks::parse(&text, Path::new("/etc/purlis/policy.json")))
    }

    #[test]
    fn a_pin_on_github_com_covers_the_api_host_and_the_download_hosts() {
        let pin = pinning(&["github.com"]);
        for host in [
            "github.com",
            "GitHub.com.",
            "api.github.com",
            "codeload.github.com",
            "objects.githubusercontent.com",
        ] {
            assert!(pinned_forge(&pin, host, 443).is_some(), "{host}");
        }
        assert!(pinned_forge(&pin, "gitlab.com", 443).is_none());
    }

    #[test]
    fn a_pin_on_a_server_or_gitlab_covers_that_host_and_no_other() {
        let pin = pinning(&["ghe.corp.example", "gitlab.com"]);
        assert!(pinned_forge(&pin, "ghe.corp.example", 443).is_some());
        assert!(pinned_forge(&pin, "GitLab.com", 443).is_some());
        assert!(pinned_forge(&pin, "github.com", 443).is_none());
        assert!(pinned_forge(&pin, "other.corp.example", 443).is_none());
    }

    #[test]
    fn the_sentence_says_who_set_the_pin_and_that_signing_in_does_not_change_it() {
        let said = pinned_forge(&pinning(&["github.com"]), "api.github.com", 443)
            .expect("api.github.com is pinned with github.com");
        assert!(said.contains("set by it-security"), "{said}");
        assert!(said.contains("/etc/purlis/policy.json"), "{said}");
        assert!(said.contains("signing in does not change"), "{said}");
    }

    #[test]
    fn a_refused_policy_file_pins_every_forge() {
        let pin = PolicyPin::reading(|| {
            Locks::parse(
                "{\"sandbox\": {\"never-hosts\": [\"github.com\"]}, \"a-key-no-version-knows\": 1}",
                Path::new("/etc/purlis/policy.json"),
            )
        });
        assert!(
            pin.pinned("github.com", 443).is_some(),
            "a refused file must not read as no pin"
        );
        for host in ["github.com", "api.github.com", "gitlab.com", "ghe.corp.example"] {
            assert!(pinned_forge(&pin, host, 443).is_some(), "{host}");
        }
    }

    #[test]
    fn a_pin_is_read_on_every_ask() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let on = std::sync::Arc::new(AtomicBool::new(false));
        let seen = on.clone();
        let pin = PolicyPin::reading(move || {
            if seen.load(Ordering::SeqCst) {
                Locks::parse(
                    "{\"sandbox\": {\"never-hosts\": [\"github.com\"]}}",
                    Path::new("/etc/purlis/policy.json"),
                )
            } else {
                Locks::none()
            }
        });
        assert!(pinned_forge(&pin, "github.com", 443).is_none());
        on.store(true, Ordering::SeqCst);
        assert!(pinned_forge(&pin, "github.com", 443).is_some());
    }
}
