// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat's confinement is started from** (#1999): its proxy's reach and ports, and what
//! runs beside it, written down as data, apart from who hears what the proxy does.
//!
//! The window compiles a chat's sandbox ([`super::Applied`]) and plans its confinement from it
//! ([`super::Applied::plan`]). Where the chat runs in the window's own process, the plan is
//! started there; where it runs in the session host, the plan is handed to the host, read back
//! exactly ([`super::reach::Reach::from_written`]) and started there by this same code. So a
//! chat's proxy decides alike wherever it runs: one compile, one plan, one start.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::sync::Arc;

use purlis_session_protocol::session;

use super::asks::Asks;
use super::egress;
use super::internal::Internal;
use super::localport;
use super::reach::{Reach, Written};
use super::{Confinement, tunnel};

/// **A chat's confinement, planned** (module docs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Every host the chat's sandbox lists, with its layer ([`Reach`]).
    pub reach: Written,
    /// The ports a tunnel is carried to for a host listed without one.
    pub tunnel_ports: Vec<u16>,
    /// The ports a plain request is carried to for a host listed without one.
    pub plain_ports: Vec<u16>,
    /// Whether a host listed without a port is carried on every port (#1665).
    pub any_port: bool,
    /// What runs beside the proxy.
    pub beside: Beside,
    /// The services on this Mac the chat was given, each by its port (qw384): never a port a
    /// proxy of purlis's listens on, which [`Plan::start`] holds to again where it starts.
    pub given: Vec<u16>,
}

/// What runs beside a chat's proxy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Beside {
    /// A harness purlis wraps whole: a temp folder of the chat's own, with its ssh route.
    Wrapped,
    /// A Claude Code chat (#1699): the proxy alone, and its ssh route in a folder of its own
    /// inside `routes`, where no chat writes (#1667).
    ProxyOnly { routes: PathBuf },
}

/// **What a plan made on another machine is held to on the runner that starts it** (#2445):
/// the runner's own folder for the project's ssh routes, the services on the runner its
/// sandbox for the project gives, the networks its interfaces are on, the internal services its
/// policy lists, and how it resolves a name.
#[derive(Clone, Copy)]
pub struct Here<'a> {
    pub routes: &'a std::path::Path,
    pub given: &'a [u16],
    pub own: &'a [super::hosts::Network],
    pub internal: &'a Internal,
    pub resolve: &'a dyn Fn(&str, u16) -> Vec<IpAddr>,
}

/// Who hears what a chat's proxy does: what it refused, what it carried, and its board of live
/// asks (#1663, #1664, #1666). Nobody, by default.
#[derive(Default)]
pub struct Telling {
    pub refusals: egress::Refusals,
    pub reached: Option<egress::Reached>,
    pub asks: Option<Arc<Asks>>,
}

/// **Who hears what a chat's proxy does, before its board is made** (#1999): what the window
/// hands whatever starts the chat's confinement. Started here, it becomes the proxy's
/// [`Telling`] ([`Self::here`]); started in the session host, the host's proxy tells the window,
/// and the window tells these, so a refusal, a connection and an ask reach the window by the
/// same closures wherever the proxy runs.
#[derive(Default)]
pub struct Hearing {
    /// Told each host refused, once heard ([`egress::Refusals::telling_once_heard`]).
    pub refusals: egress::Refusals,
    /// Told the count of hosts refused past the ones named.
    pub more: Option<egress::MoreTold>,
    /// Told each connection carried, coalesced.
    pub reached: Option<egress::Reached>,
    /// Its board of live asks' hearing, where its proxy asks live.
    pub asks: Option<Asking>,
}

/// What a chat's board of live asks is made of (#1666): who hears an ask, when one may be
/// raised, and where a host already allowed is found.
pub struct Asking {
    pub tell: super::asks::Tell,
    pub askable: super::asks::Askable,
    pub known: super::asks::Known,
}

impl Asking {
    /// The board these make.
    pub fn board(self) -> Arc<Asks> {
        Arc::new(
            Asks::new(self.tell)
                .asking_while(self.askable)
                .knowing(self.known),
        )
    }
}

impl Hearing {
    /// What a proxy started in this process tells.
    pub fn here(self) -> Telling {
        Telling {
            refusals: match self.more {
                Some(more) => self.refusals.telling_more(more),
                None => self.refusals,
            },
            reached: self.reached,
            asks: self.asks.map(Asking::board),
        }
    }
}

impl Plan {
    /// **This plan, as the session protocol carries it to the host that starts it** (#1999).
    pub fn wired(&self) -> session::ConfinePlan {
        session::ConfinePlan {
            listed: self
                .reach
                .listed
                .iter()
                .map(|(host, by)| session::Listed {
                    host: host.clone(),
                    layer: super::reach::layer_of(*by),
                })
                .collect(),
            never: self.reach.never.clone(),
            withdrawn: self.reach.withdrawn.clone(),
            anything: self.reach.anything,
            tunnel_ports: self.tunnel_ports.clone(),
            plain_ports: self.plain_ports.clone(),
            any_port: self.any_port,
            beside: match &self.beside {
                Beside::Wrapped => session::Beside::Wrapped,
                Beside::ProxyOnly { routes } => session::Beside::ProxyOnly {
                    routes: routes.display().to_string(),
                },
            },
            given: self.given.clone(),
        }
    }

    /// **The plan a host was handed, read back exactly**, or why it does not read: a host
    /// policy pins that is not a host is refused, never dropped.
    pub fn from_wire(wired: &session::ConfinePlan) -> Result<Plan, String> {
        let reach = Written {
            listed: wired
                .listed
                .iter()
                .map(|listed| (listed.host.clone(), super::reach::by_of(listed.layer)))
                .collect(),
            never: wired.never.clone(),
            withdrawn: wired.withdrawn.clone(),
            anything: wired.anything,
        };
        // Read once here, so a plan that does not read is refused before anything starts.
        Reach::from_written(&reach)?;
        Ok(Plan {
            reach,
            tunnel_ports: wired.tunnel_ports.clone(),
            plain_ports: wired.plain_ports.clone(),
            any_port: wired.any_port,
            beside: match &wired.beside {
                session::Beside::Wrapped => Beside::Wrapped,
                session::Beside::ProxyOnly { routes } => Beside::ProxyOnly {
                    routes: PathBuf::from(routes),
                },
            },
            given: wired.given.clone(),
        })
    }

    /// **Whether this plan, made on another machine, fits the runner that is to start it**
    /// (#2445, ruled 2026-10-17), or why not, in a sentence:
    ///
    /// - a Claude Code chat's ssh route goes in the runner's own folder for the project;
    /// - every service on the runner it gives the chat is one the runner's sandbox for the
    ///   project gives;
    /// - the runner's loopback is listed only on such a port;
    /// - an address of a private network, a cluster's or a link's, or of the runner itself, is
    ///   listed only where the runner's policy lists it ([`super::internal`]), and so is a name
    ///   that resolves to one there. A name under `*.` resolves to nothing here, and is held to
    ///   the same rule at every connect.
    pub fn fits(&self, here: &Here<'_>) -> Result<(), String> {
        if let Beside::ProxyOnly { routes } = &self.beside
            && routes != here.routes
        {
            return Err(format!(
                "it puts the chat's ssh route in {}, which is not the runner's folder for it ({})",
                routes.display(),
                here.routes.display()
            ));
        }
        if let Some(port) = self.given.iter().find(|port| !here.given.contains(port)) {
            return Err(format!(
                "it gives the chat the service on port {port} of the runner, which the runner's \
                 sandbox for the project does not give"
            ));
        }
        self.reach
            .listed
            .iter()
            .try_for_each(|(entry, _)| fits_here(entry, here))
    }

    /// What the proxy serves: this plan, told to `telling`.
    pub fn serving(&self, reach: Reach, telling: Telling) -> egress::Serving {
        egress::Serving {
            tunnel_ports: self.tunnel_ports.clone(),
            plain_ports: self.plain_ports.clone(),
            any_port: self.any_port,
            refusals: telling.refusals,
            reached: telling.reached,
            asks: telling.asks,
            ..egress::Serving::of(reach)
        }
    }

    /// **Starts this plan here**: the proxy, and what runs beside it, told to `telling`.
    /// Refused where a port the chat was given is one purlis listens on now in this process,
    /// so no proxy and no chat ever share a port, and where the reach does not read back.
    pub fn start(&self, telling: Telling) -> std::io::Result<Confinement> {
        self.starting(telling, None)
    }

    /// [`Self::start`], for a plan made on another machine (#2445): its proxy carries an
    /// internal address only where `internal`, the runner's policy, lists it, checked at every
    /// connect ([`super::internal`]).
    pub fn start_held(&self, telling: Telling, internal: Internal) -> std::io::Result<Confinement> {
        self.starting(telling, Some(internal))
    }

    fn starting(
        &self,
        telling: Telling,
        internal: Option<Internal>,
    ) -> std::io::Result<Confinement> {
        let admitted = localport::still_ports(&self.given);
        if self.given.iter().any(|port| !admitted.contains(port)) {
            return Err(std::io::Error::other(
                "a service this chat was given is on a port purlis listens on now",
            ));
        }
        let reach = Reach::from_written(&self.reach).map_err(std::io::Error::other)?;
        let serving = egress::Serving {
            internal,
            ..self.serving(reach, telling)
        };
        match &self.beside {
            Beside::Wrapped => Confinement::serving(serving),
            Beside::ProxyOnly { routes } => Confinement::proxy_only_routed(serving, routes),
        }
    }
}

/// Whether listed `entry` fits the runner `here` describes ([`Plan::fits`]), or why not.
fn fits_here(entry: &str, here: &Here<'_>) -> Result<(), String> {
    let (name, port) = egress::entry_parts(entry);
    if name.is_empty() || name.starts_with("*.") {
        return Ok(());
    }
    let Ok(ip) = name.parse::<IpAddr>() else {
        // A name: what it resolves to on the runner, held as the address would be.
        let listed = here.internal.lists_entry(entry, Some(here.own));
        return match (here.resolve)(name, port.unwrap_or(HTTPS))
            .into_iter()
            .find(|ip| {
                here.internal.holds(*ip, Some(here.own))
                    && (!listed
                        || here.internal.metadata(*ip)
                        || here.internal.is_own(*ip, Some(here.own)))
            }) {
            Some(ip) => Err(not_taken_said(
                format!(
                    "it lets the chat reach {entry}, which the runner resolves to {ip}, an \
                     internal address its policy does not list"
                ),
                here,
            )),
            None => Ok(()),
        };
    };
    // `127.0.0.1` and `::1` alone: the rest of the loopback network is internal, below.
    if ip == Ipv4Addr::LOCALHOST || ip == Ipv6Addr::LOCALHOST {
        return match port {
            Some(port) if here.given.contains(&port) => Ok(()),
            _ => Err(format!(
                "it lets the chat reach {entry}, a port of the runner's own that the runner's \
                 sandbox for the project does not give"
            )),
        };
    }
    if here.internal.holds(ip, Some(here.own)) && !here.internal.lists_entry(entry, Some(here.own))
    {
        return Err(not_taken_said(
            format!(
                "it lets the chat reach {entry}, an internal address the runner's policy does not \
                 list"
            ),
            here,
        ));
    }
    Ok(())
}

/// `why`, and each entry of the runner's `relayed-internal` it does not take, each in its own
/// sentence (#2530).
fn not_taken_said(why: String, here: &Here<'_>) -> String {
    let not_taken = here.internal.not_taken();
    if not_taken.is_empty() {
        return why;
    }
    format!(
        "{why}. Some entries in the runner's policy's relayed-internal are not taken, so relayed \
         chats reach nothing through them: {}",
        not_taken.join(" ")
    )
}

/// The port a listed name without one is resolved on: the name's addresses are the same on any.
const HTTPS: u16 = 443;

/// The plan's routes folder for a Claude Code chat of the plane at `root`.
pub(super) fn routes(root: &std::path::Path) -> PathBuf {
    tunnel::routes_folder(root)
}

#[cfg(test)]
mod tests;
