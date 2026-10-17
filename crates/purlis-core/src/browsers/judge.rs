// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The browser's judge: where a chat's browser may go, and what it asks** (#2018, spec #2008):
//! told every http and https request its pages are about to send, and the page or frame the
//! agent is about to type on, and shared with the browser's reach proxy
//! ([`crate::sandbox::browser_reach`]).
//!
//! The reach proxy decides by host and port, so it cannot tell a link a page offered from an
//! address the agent wrote, or a read from a form posted. The driver can: it holds every request
//! until it is judged and says what it is ([`Outbound`]). So the rules of the 2026-10-14 ruling
//! are kept here, by the browser's lane of its chat's reach ([`Lane`]):
//!
//! - **A site the browser's lane lists goes**, whatever is sent to it: a host its chat's reach
//!   lists, or a browser site the person allowed (a grant for the browser alone, ruled
//!   2026-10-17; never read by the chat's own commands).
//! - **A public site new to the project is readable**: what a page reads there (a link it
//!   offered, an image, a script) goes, and its proxy then carries that site.
//! - **An address the agent writes for a site new to the project asks** ([`Gate::Ask`]), and so
//!   does a form posted or a request with a body sent there, and the agent's typing on a page of
//!   it. Nothing is sent until the person allows the site for the browser.
//! - **The open web asks nothing**: every public site is carried, as the person allowed.
//! - **What the person does while they drive is theirs**, and goes.
//! - **What is never a site** (this machine, a private or reserved address, an address of no
//!   web scheme) asks nothing: the proxy refuses or carries it by its own rules.
//!
//! The proxy carries a site new to the project only where a request to it was judged here to
//! go, or the person allowed it: what the driver does not hold reaches no other new site.

use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, PoisonError};

use purlis_browser_engine::drive::{Driver, Gate, Outbound, Why};
use purlis_host::protocol::browsing::BrowserReach;

use crate::sandbox::browser_reach::{Lane, NewSites, Refusal, Site, Verdict};
use crate::sandbox::reach::Reach;

/// The most sites new to the project one browser's proxy carries: past them a new site is
/// refused there, so its memory stays bounded.
const MOST_LET_ON: usize = 4096;

/// **One browser's judge, and the sites new to the project it let on** (module docs).
pub struct SiteJudge {
    now: Mutex<Now>,
    /// This machine's addresses, where a test says them; read at each judgement otherwise.
    own: Option<Vec<IpAddr>>,
}

struct Now {
    lane: Lane,
    open_web: bool,
    let_on: HashSet<Site>,
}

impl std::fmt::Debug for SiteJudge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SiteJudge").finish_non_exhaustive()
    }
}

impl SiteJudge {
    /// The judge of a browser that reaches `reach`. A listing that does not read is refused.
    pub fn of(reach: &BrowserReach) -> Result<Self, String> {
        Ok(Self {
            now: Mutex::new(Now {
                lane: Lane::of(read(reach)?),
                open_web: reach.open_web,
                let_on: HashSet::new(),
            }),
            own: None,
        })
    }

    /// This judge, taking `own` as this machine's addresses.
    #[cfg(test)]
    #[must_use]
    pub fn knowing_this_machine(mut self, own: Vec<IpAddr>) -> Self {
        self.own = Some(own);
        self
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Now> {
        self.now.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn own(&self) -> Option<Vec<IpAddr>> {
        self.own
            .clone()
            .or_else(crate::sandbox::hosts::own_addresses_read)
    }

    /// **What the browser reaches from now on** (#2018): its chat's reach as the window says it
    /// is now. One that does not read is refused, and the reach before stands. What it let on
    /// of sites new to the project stays let on.
    pub fn reach_now(&self, reach: &BrowserReach) -> Result<(), String> {
        let lane = Lane::of(read(reach)?);
        let mut now = self.lock();
        now.lane = lane;
        now.open_web = reach.open_web;
        Ok(())
    }

    /// **Whether `out` goes**, or the site the person is to be asked about (module docs).
    pub fn judge(&self, out: &Outbound) -> Gate {
        let Some((host, port)) = site_of(&out.address) else {
            return Gate::Go;
        };
        let tunnel = out
            .address
            .get(..6)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https:"));
        let own = self.own();
        let mut now = self.lock();
        let new = match now.lane.decide_here(&host, port, tunnel, own.as_deref()) {
            Verdict::Carry { .. } => return Gate::Go,
            Verdict::Site { .. } => true,
            // A public site on a port nothing lists: the proxy refuses it, and only an Allow
            // of that host and port would carry it, so the agent's address asks.
            Verdict::Refused(Refusal::Port) => false,
            Verdict::Refused(_) => return Gate::Go,
        };
        let theirs = out.by == Driver::Person;
        if now.open_web || theirs || out.why == Why::Reads {
            if new && now.let_on.len() < MOST_LET_ON {
                now.let_on.insert(Site::of(&host, port));
            }
            Gate::Go
        } else {
            Gate::Ask {
                site: Site::of(&host, port).to_string(),
            }
        }
    }

    /// **What the browser's reach proxy carries of a site nothing lists**: on the open web,
    /// every one; otherwise one this judge let on, or one the chat's reach lists now that did
    /// not when the browser started.
    pub fn new_sites(self: &Arc<Self>) -> NewSites {
        let sites = Arc::clone(self);
        NewSites::new(move |site: &Site| sites.carries(site))
    }

    fn carries(&self, site: &Site) -> bool {
        let own = self.own();
        let now = self.lock();
        now.open_web
            || now.let_on.contains(site)
            || [true, false].into_iter().any(|tunnel| {
                matches!(
                    now.lane
                        .decide_here(&site.host, site.port, tunnel, own.as_deref()),
                    Verdict::Carry { .. }
                )
            })
    }
}

/// `reach`'s listing, read as the chat's own reach reads it (`crate::browsers::reach_of`).
fn read(reach: &BrowserReach) -> Result<Reach, String> {
    super::reach_of(Some(reach)).map(|(reach, _)| reach)
}

/// **The host and port an http or https address goes to**, as the engine writes a request's
/// address: lower case, no final dot, an IPv6 address without its brackets, and the scheme's
/// port where none is written. None for any other address, or one with no readable host.
pub fn site_of(address: &str) -> Option<(String, u16)> {
    let (scheme, rest) = address.split_once("://")?;
    let default = match scheme.to_ascii_lowercase().as_str() {
        "https" => 443,
        "http" => 80,
        _ => return None,
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let (host, port) = if let Some(inside) = authority.strip_prefix('[') {
        let (host, after) = inside.split_once(']')?;
        (host, after.strip_prefix(':'))
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    let port = match port {
        Some(port) => port.parse().ok()?,
        None => default,
    };
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    (!host.is_empty()).then_some((host, port))
}

#[cfg(test)]
mod tests;
