// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The addresses a model is served at, as its hosts resolve** (ADR 0101 §3, #3094): what a
//! confined extension's proxy never connects to, whatever name the extension asked for.
//!
//! A confined extension names its own hosts, and a name is the extension's to point anywhere
//! (its own DNS can answer with a provider's address). So its proxy looks up the hosts that
//! serve a model, and refuses any address one of them resolved to: looked up again at most every
//! [`ANSWERS_STAND`], and an address kept for [`REFUSED_FOR`] after the last answer that held it, so a provider
//! whose answers rotate is refused at each address it answered with lately. A lookup that is due
//! is made before the connect it holds up, never after, so no connect is let through on an
//! answer still pending. Where no provider name answered and none answered lately, nothing is
//! known of where a model is served, so every address is refused until one answers.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// How long the provider hosts' answers stand before they are looked up again.
pub const ANSWERS_STAND: Duration = Duration::from_secs(30);

/// How long an address a provider host answered with is refused after the last answer that
/// held it.
pub const REFUSED_FOR: Duration = Duration::from_secs(10 * 60);

/// The port a provider host is looked up for: the one a tunnel is carried to.
const PORT: u16 = 443;

/// At most this many lookups run at once.
const LOOKUPS_AT_ONCE: usize = 8;

/// How a name is looked up: the system's resolver, or a test's.
pub type Resolve = Arc<dyn Fn(&str, u16) -> Vec<SocketAddr> + Send + Sync>;

/// **The addresses the hosts that serve a model resolve to** ([module docs](self)).
pub struct ProviderAddresses {
    names: Vec<String>,
    resolve: Resolve,
    seen: Mutex<Seen>,
}

impl std::fmt::Debug for ProviderAddresses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderAddresses")
            .field("names", &self.names)
            .finish_non_exhaustive()
    }
}

/// What the last lookups answered.
#[derive(Default)]
struct Seen {
    /// When the names were last looked up.
    looked: Option<Instant>,
    /// Each address answered, with when it was last answered.
    at: HashMap<IpAddr, Instant>,
}

impl ProviderAddresses {
    /// The addresses `names` resolve to, looked up by the system's resolver.
    pub fn of(names: Vec<String>) -> Self {
        let resolve: Resolve = Arc::new(|name: &str, port: u16| {
            std::net::ToSocketAddrs::to_socket_addrs(&(name, port))
                .map(Iterator::collect)
                .unwrap_or_default()
        });
        Self::resolving(names, resolve)
    }

    /// The addresses `names` resolve to, looked up by `resolve`.
    pub fn resolving(names: Vec<String>, resolve: Resolve) -> Self {
        Self {
            names,
            resolve,
            seen: Mutex::new(Seen::default()),
        }
    }

    /// Looks `name` up on `port` as these addresses are looked up: what a confined proxy resolves
    /// the names it is asked for by, so a test's resolver answers both.
    pub fn resolve(&self, name: &str, port: u16) -> Vec<SocketAddr> {
        (self.resolve)(name, port)
    }

    /// **Whether a model is served at `ip`**, as the provider hosts answer now: also where none
    /// of them answers, so a resolver that fails never lets a connect through.
    pub fn serves(&self, ip: IpAddr) -> bool {
        self.serves_at(ip, Instant::now())
    }

    /// [`Self::serves`], at `now`.
    pub(crate) fn serves_at(&self, ip: IpAddr, now: Instant) -> bool {
        // Held while a due lookup runs, so a connect waits for its answer rather than pass.
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        if seen
            .looked
            .is_none_or(|looked| now.saturating_duration_since(looked) >= ANSWERS_STAND)
        {
            let found = self.look_up();
            // Nothing answered: looked up again at the next connect, not after the answers stand.
            if !found.is_empty() {
                seen.looked = Some(now);
            }
            for at in found {
                seen.at.insert(at.ip().to_canonical(), now);
            }
        }
        seen.at
            .retain(|_, answered| now.saturating_duration_since(*answered) < REFUSED_FOR);
        // Nothing known of where a model is served: every address may be one.
        seen.at.is_empty() || seen.at.contains_key(&ip.to_canonical())
    }

    /// Every address the names resolve to now, [`LOOKUPS_AT_ONCE`] lookups at a time.
    fn look_up(&self) -> Vec<SocketAddr> {
        let per = self.names.len().div_ceil(LOOKUPS_AT_ONCE).max(1);
        std::thread::scope(|scope| {
            let running: Vec<_> = self
                .names
                .chunks(per)
                .map(|names| {
                    scope.spawn(move || {
                        names
                            .iter()
                            .flat_map(|name| (self.resolve)(name, PORT))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            running
                .into_iter()
                .flat_map(|one| one.join().unwrap_or_default())
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// Provider addresses whose one name answers `answers[n]` at its `n`th lookup (the last one
    /// after), and the count of lookups made.
    fn rotating(answers: &'static [&'static str]) -> (ProviderAddresses, Arc<AtomicUsize>) {
        let looked = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&looked);
        let resolve: Resolve = Arc::new(move |_: &str, port: u16| {
            let n = counted
                .fetch_add(1, Ordering::SeqCst)
                .min(answers.len() - 1);
            vec![SocketAddr::new(
                answers[n].parse().expect("an address"),
                port,
            )]
        });
        (
            ProviderAddresses::resolving(vec!["api.provider.example".to_owned()], resolve),
            looked,
        )
    }

    fn ip(text: &str) -> IpAddr {
        text.parse().expect("an address")
    }

    #[test]
    fn the_names_are_looked_up_again_only_once_their_answers_are_due() {
        let (providers, looked) = rotating(&["104.18.32.47", "104.18.33.47"]);
        let start = Instant::now();
        assert!(providers.serves_at(ip("104.18.32.47"), start));
        assert!(!providers.serves_at(ip("104.18.33.47"), start + ANSWERS_STAND / 2));
        assert_eq!(looked.load(Ordering::SeqCst), 1, "looked up before due");
        assert!(providers.serves_at(ip("104.18.33.47"), start + ANSWERS_STAND));
        assert_eq!(looked.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn an_address_a_provider_answered_lately_is_refused_until_it_is_kept_no_longer() {
        let (providers, _) = rotating(&["104.18.32.47", "104.18.33.47"]);
        let start = Instant::now();
        assert!(providers.serves_at(ip("104.18.32.47"), start));
        // Its answer moved on, and the address it gave before is still refused for a while.
        assert!(providers.serves_at(ip("104.18.32.47"), start + ANSWERS_STAND));
        assert!(!providers.serves_at(ip("104.18.32.47"), start + REFUSED_FOR));
        assert!(providers.serves_at(ip("104.18.33.47"), start + REFUSED_FOR));
    }

    #[test]
    fn while_no_provider_name_answers_every_address_is_refused_and_looked_up_again() {
        let looked = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&looked);
        let resolve: Resolve = Arc::new(move |_: &str, _: u16| {
            counted.fetch_add(1, Ordering::SeqCst);
            Vec::new()
        });
        let providers =
            ProviderAddresses::resolving(vec!["api.provider.example".to_owned()], resolve);
        let start = Instant::now();
        assert!(providers.serves_at(ip("151.101.1.1"), start));
        assert!(providers.serves_at(ip("151.101.1.1"), start));
        assert_eq!(looked.load(Ordering::SeqCst), 2, "not looked up again");
    }

    #[test]
    fn an_address_is_the_same_in_either_spelling() {
        let (providers, _) = rotating(&["::ffff:104.18.32.47"]);
        assert!(providers.serves(ip("104.18.32.47")));
        assert!(providers.serves(ip("::ffff:104.18.32.47")));
        assert!(!providers.serves(ip("151.101.1.1")));
    }
}
