// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The resolver seam, a test build's alone** (T1.1-02, #6849): where a chat proxy and a
//! browser's reach proxy look a name up and dial an address in a scenario, so a scenario never
//! touches the internet and every lookup and dial is counted.
//!
//! It exists only with `cfg(any(test, feature = "test-support"))`, and `test-support` is turned
//! on only from dev-dependencies, so no shipped build has it (the scenario suite's
//! `the_resolver_seam_is_absent_without_test_support` proves it with `cargo tree`).
//!
//! **It never loosens a decision.** A proxy decides by name first ([`super::reach::Reach`]) and
//! consults the seam only to connect what it already carries, so a refused name is never looked
//! up, dialled or reached, whatever the map says. Its two kinds of entry:
//!
//! - **[`Names::origin`]**: an exact name and port, carried to a local test origin. The one entry
//!   exempt from the filter that keeps a chat off this machine's addresses, and only for that
//!   name and port.
//! - **[`Names::resolves_to`]**: scripted answers, one per lookup (the last repeats), which go
//!   through the production filter unchanged: a name rebound to the loopback is refused as
//!   [`super::egress::Connected::OnlyLocal`] as it is in production.
//!
//! A name with neither is **unmapped**: it answers nothing (never the system's resolver), is
//! recorded, and fails the scenario unless the cell lists it ([`Names::unlisted`]).

use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::egress::providers::Resolve;

/// What a lookup through the seam found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// An [`Names::origin`] entry: carried to the origin at this address.
    Origin(SocketAddr),
    /// A [`Names::resolves_to`] entry's answer for this lookup.
    Scripted(Vec<SocketAddr>),
    /// No entry: nothing answered, and nothing was asked of the system's resolver.
    Unmapped,
}

/// One lookup a proxy made through the seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lookup {
    pub name: String,
    pub port: u16,
    pub found: Found,
}

enum Entry {
    Origin(SocketAddr),
    /// The answers still to give, and how many were given.
    ResolvesTo(Vec<Vec<SocketAddr>>, usize),
}

#[derive(Default)]
struct Kept {
    entries: HashMap<(String, u16), Entry>,
    lookups: Vec<Lookup>,
    dials: Vec<SocketAddr>,
    origin_hits: Vec<(String, u16)>,
}

/// **A scenario's names, and the lookup and dial recorders** ([module docs](self)). Shared by
/// every proxy the scenario starts with it.
#[derive(Default)]
pub struct Names {
    kept: Mutex<Kept>,
}

impl std::fmt::Debug for Names {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Names").finish_non_exhaustive()
    }
}

/// `name` in one spelling: lower case, with no final dot and an IPv6 address without brackets.
fn spelled(name: &str) -> String {
    let name = name.strip_suffix('.').unwrap_or(name);
    let name = name
        .strip_prefix('[')
        .and_then(|inside| inside.strip_suffix(']'))
        .unwrap_or(name);
    name.to_ascii_lowercase()
}

impl Names {
    /// No names yet: every lookup is unmapped.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **`name` on `port` is the local test origin at `at`**: the one entry exempt from the
    /// filter that keeps a chat off this machine, and for that name and port alone.
    pub fn origin(&self, name: &str, port: u16, at: SocketAddr) -> &Self {
        self.kept()
            .entries
            .insert((spelled(name), port), Entry::Origin(at));
        self
    }

    /// **`name` on `port` answers `answers`, one per lookup**, the last again once they run out
    /// (none answers nothing). Each answer goes through the production filter unchanged.
    pub fn resolves_to(&self, name: &str, port: u16, answers: Vec<Vec<SocketAddr>>) -> &Self {
        self.kept()
            .entries
            .insert((spelled(name), port), Entry::ResolvesTo(answers, 0));
        self
    }

    /// Looks `name` up on `port`, and records the lookup.
    pub(crate) fn look_up(&self, name: &str, port: u16) -> Found {
        let mut kept = self.kept();
        let key = (spelled(name), port);
        let found = match kept.entries.get_mut(&key) {
            Some(Entry::Origin(at)) => Found::Origin(*at),
            Some(Entry::ResolvesTo(answers, given)) => {
                let answer = answers
                    .get(*given)
                    .or_else(|| answers.last())
                    .cloned()
                    .unwrap_or_default();
                *given += 1;
                Found::Scripted(answer)
            }
            None => Found::Unmapped,
        };
        kept.lookups.push(Lookup {
            name: key.0,
            port,
            found: found.clone(),
        });
        found
    }

    /// **Dials the origin `name` on `port` is carried to** (a [`Found::Origin`] lookup), within
    /// `patience`, recording the dial and the origin reached.
    pub(crate) fn dial_origin(
        &self,
        name: &str,
        port: u16,
        at: &SocketAddr,
        patience: Duration,
    ) -> io::Result<TcpStream> {
        self.kept().origin_hits.push((spelled(name), port));
        self.dial(at, patience)
    }

    /// **What a proxy resolves by in a scenario**: a [`Names::resolves_to`] entry's answer, and
    /// nothing for an unmapped name or an origin (whose address is reached only by
    /// [`Self::dial_origin`], so it never passes the filter by another path). Recorded either
    /// way.
    pub fn resolve(&self, name: &str, port: u16) -> Vec<SocketAddr> {
        match self.look_up(name, port) {
            Found::Scripted(answer) => answer,
            Found::Origin(_) | Found::Unmapped => Vec::new(),
        }
    }

    /// [`Self::resolve`], as a [`super::egress::ProviderAddresses`] resolver.
    pub fn resolver(self: &Arc<Self>) -> Resolve {
        let names = Arc::clone(self);
        Arc::new(move |name: &str, port: u16| names.resolve(name, port))
    }

    /// Dials `at`, within `patience`, and records the dial.
    pub(crate) fn dial(&self, at: &SocketAddr, patience: Duration) -> io::Result<TcpStream> {
        self.kept().dials.push(*at);
        TcpStream::connect_timeout(at, patience)
    }

    /// Every lookup made so far, in order.
    pub fn lookups(&self) -> Vec<Lookup> {
        self.kept().lookups.clone()
    }

    /// Every address dialled so far, in order.
    pub fn dials(&self) -> Vec<SocketAddr> {
        self.kept().dials.clone()
    }

    /// Every connection carried to an origin so far, by name and port.
    pub fn origin_hits(&self) -> Vec<(String, u16)> {
        self.kept().origin_hits.clone()
    }

    /// **"Saw nothing"**: no lookup, no dial and no origin reached.
    pub fn saw_nothing(&self) -> bool {
        let kept = self.kept();
        kept.lookups.is_empty() && kept.dials.is_empty() && kept.origin_hits.is_empty()
    }

    /// **What fails the scenario**: each name looked up with no entry that `listed` does not
    /// name, once each, in the order first seen. A cell lists the names a real binary's own
    /// start-up asks for.
    pub fn unlisted(&self, listed: &[(&str, u16)]) -> Vec<(String, u16)> {
        let listed: Vec<(String, u16)> = listed
            .iter()
            .map(|(name, port)| (spelled(name), *port))
            .collect();
        let mut out: Vec<(String, u16)> = Vec::new();
        for lookup in &self.kept().lookups {
            let key = (lookup.name.clone(), lookup.port);
            if lookup.found == Found::Unmapped && !listed.contains(&key) && !out.contains(&key) {
                out.push(key);
            }
        }
        out
    }
}

/// Every name a test build asked of the system's resolver, so a scenario test can show a name
/// never reached it ([`asked_the_system`]).
static ASKED_THE_SYSTEM: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Records that `name` was asked of the system's resolver (called from the one place each proxy
/// does that, in a test build alone).
pub(crate) fn asking_the_system(name: &str) {
    ASKED_THE_SYSTEM
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(spelled(name));
}

/// Whether this process has asked the system's resolver for `name`.
pub fn asked_the_system(name: &str) -> bool {
    ASKED_THE_SYSTEM
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains(&spelled(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(last: u8, port: u16) -> SocketAddr {
        SocketAddr::from(([10, 0, 0, last], port))
    }

    #[test]
    fn a_resolves_to_entry_answers_each_lookup_in_turn_and_repeats_its_last() {
        let names = Names::new();
        names.resolves_to(
            "Rebind.Test.",
            443,
            vec![vec![at(1, 443)], vec![at(2, 443)]],
        );
        assert_eq!(names.resolve("rebind.test", 443), vec![at(1, 443)]);
        assert_eq!(names.resolve("REBIND.test", 443), vec![at(2, 443)]);
        assert_eq!(names.resolve("rebind.test", 443), vec![at(2, 443)]);
        assert_eq!(names.lookups().len(), 3);
        assert!(names.dials().is_empty());
    }

    #[test]
    fn an_origins_address_is_never_handed_out_as_a_resolved_answer() {
        let names = Names::new();
        names.origin("site.test", 80, at(9, 8080));
        assert!(names.resolve("site.test", 80).is_empty());
        assert_eq!(names.look_up("site.test", 80), Found::Origin(at(9, 8080)));
        // Only that exact port.
        assert_eq!(names.look_up("site.test", 443), Found::Unmapped);
    }

    #[test]
    fn an_unmapped_lookup_is_recorded_and_unlisted_until_a_cell_lists_it() {
        let names = Names::new();
        assert!(names.saw_nothing());
        assert!(names.resolve("startup.example.test", 443).is_empty());
        assert!(names.resolve("startup.example.test", 443).is_empty());
        assert!(!names.saw_nothing());
        assert_eq!(
            names.unlisted(&[]),
            vec![("startup.example.test".to_owned(), 443)],
            "once, however often it was asked"
        );
        assert!(names.unlisted(&[("Startup.Example.Test", 443)]).is_empty());
        assert_eq!(names.unlisted(&[("startup.example.test", 80)]).len(), 1);
    }
}
