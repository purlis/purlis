// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat may reach, decided once** (spec #1661, #1664): the core decision module every
//! adapter asks, given a host and port and the layers a chat's sandbox was compiled with.
//!
//! A [`Reach`] is each host a chat's sandbox lists, with the layer that lists it ([`By`]): the
//! **Open hosts** (the presets in force and the project's own hosts), the **Persona hosts** the
//! person on this machine allowed for the persona the chat runs as, and the **Allowed hosts** at
//! the scope they were kept at (this project on this machine, or only this chat). Where two
//! layers list a host, the first decides it, in that order.
//!
//! [`Reach::decide`] answers one [`Decision`]: open, persona, allowed (with its scope), ask, or
//! refused (with why). It goes by host and port alone: nothing a connection carries after its
//! first line is read, so there is no TLS interception and no certificate of purlis's own.
//!
//! **The local-address check** is part of the decision. A chat never reaches this machine (its
//! loopback, the unspecified address and each of its own interface addresses), a link-local
//! address or a cloud metadata service ([`super::hosts::refused_address`]), unless that exact
//! address and port is listed. purlis's egress proxy asks [`Reach::lists_exactly`] of every
//! address a name resolves to as well, so a name pointed at one of them reaches nothing.
//!
//! **Ask** is a host nothing lists that a person could allow: the proxy holds the connection
//! while the person is asked ([`super::asks`], #1666), or, where policy turns that off, refuses
//! it and the chat's sandbox raises a Block with an Allow on it. A host policy pins as never
//! allowed is refused and never asked about ([`Refused::Policy`]).

use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};

use super::egress::{allows_on, entry_parts};
use super::hosts::{Host, refused_address};
use super::localport;

/// The layer that lists a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    /// A preset in force, or the project's own hosts: an **Open host**.
    Open,
    /// The persona the chat runs as, as the person on this machine allowed its list.
    Persona,
    /// An **Allowed host** kept for this project on this machine.
    You,
    /// An **Allowed host** kept for this chat alone.
    Chat,
    /// An **Allowed host** for everyone in the project, taken live by a running chat (#1709):
    /// carried as an Open host is, and kept in the record by the scope the person chose. From
    /// the chat's next start it is an Open host ([`By::Open`]), as a committed host is.
    Project,
}

/// The scope an Allow kept at `level` is carried at (#1666, #1709).
impl From<super::grant::Level> for By {
    fn from(level: super::grant::Level) -> Self {
        match level {
            super::grant::Level::Once | super::grant::Level::Chat => Self::Chat,
            super::grant::Level::You | super::grant::Level::Machine => Self::You,
            super::grant::Level::Project => Self::Project,
        }
    }
}

/// Why a connection is refused outright, never asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// What the connection named is not something a person could allow: a single word, a
    /// wildcard, a malformed name. The sentence says why.
    NeverAHost(String),
    /// This machine, a link-local address or a cloud metadata service, by address or by what a
    /// name resolved to.
    LocalAddress,
    /// A host an administrator's policy pins as never allowed (#1666): never asked about.
    Policy,
}

/// What a chat's connection to a host and port is answered with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Open,
    Persona,
    /// An Allowed host, at the scope it was kept at ([`By::You`], [`By::Chat`], or
    /// [`By::Project`] taken live).
    Allowed(By),
    /// Nothing lists it, and a person could allow it.
    Ask,
    Refused(Refused),
}

impl Decision {
    /// Whether the connection is carried.
    pub fn carries(&self) -> bool {
        matches!(self, Self::Open | Self::Persona | Self::Allowed(_))
    }

    /// The word the network record keeps it by.
    pub fn word(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Persona => "persona",
            Self::Allowed(By::You) => "you",
            Self::Allowed(By::Chat) => "chat",
            Self::Allowed(By::Project) => "project",
            // Never built: an Open or a Persona listing decides as itself.
            Self::Allowed(By::Open) => "open",
            Self::Allowed(By::Persona) => "persona",
            Self::Ask => "ask",
            Self::Refused(_) => "refused",
        }
    }

    /// Every word [`Self::word`] says.
    pub const WORDS: [&'static str; 7] = [
        "open", "persona", "you", "chat", "project", "ask", "refused",
    ];

    /// `word` as [`Self::word`] says it, where it is one: a decision's word read back from
    /// elsewhere (#1999).
    pub fn word_of(word: &str) -> Option<&'static str> {
        Self::WORDS.into_iter().find(|one| *one == word)
    }

    /// The decision a layer's listing makes: Open, Persona, or Allowed at its scope.
    pub fn by(by: By) -> Self {
        Self::of(by)
    }

    fn of(by: By) -> Self {
        match by {
            By::Open => Self::Open,
            By::Persona => Self::Persona,
            By::You | By::Chat | By::Project => Self::Allowed(by),
        }
    }
}

/// Every host a chat's sandbox lists, each with the layer that lists it, in the order the
/// layers decide.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reach {
    listed: Vec<(String, By)>,
    /// The hosts policy pins as never allowed (#1666): refused, never asked about.
    never: Vec<Host>,
    /// The ports of this machine whose grant was removed while the chat runs ([`Self::withdraw`]):
    /// shared by every copy, so the proxy's copy hears it.
    withdrawn: Withdrawn,
    /// At the floor (#2071): every host is carried as an Open host, and none is asked about.
    /// This machine, a link-local address and a metadata service stay refused, and so does a
    /// host policy pins as never allowed.
    anything: bool,
}

/// **A [`Reach`] written down** (#1999): what the session host starts a chat's proxy from,
/// read back with [`Reach::from_written`] into the same reach, so the proxy it starts decides
/// as one started in the window would.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// Each host listed, with its layer, in the order the layers decide.
    pub listed: Vec<(String, By)>,
    /// The hosts policy pins as never allowed, each as a host is written.
    pub never: Vec<String>,
    /// The ports of this machine withdrawn while the chat runs.
    pub withdrawn: Vec<u16>,
    /// The floor: every host carried.
    pub anything: bool,
}

/// A layer the session protocol names, as the core names it.
pub fn by_of(layer: purlis_session_protocol::session::Layer) -> By {
    use purlis_session_protocol::session::Layer;
    match layer {
        Layer::Open => By::Open,
        Layer::Persona => By::Persona,
        Layer::You => By::You,
        Layer::Chat => By::Chat,
        Layer::Project => By::Project,
    }
}

/// A layer as the session protocol names it.
pub fn layer_of(by: By) -> purlis_session_protocol::session::Layer {
    use purlis_session_protocol::session::Layer;
    match by {
        By::Open => Layer::Open,
        By::Persona => Layer::Persona,
        By::You => Layer::You,
        By::Chat => Layer::Chat,
        By::Project => Layer::Project,
    }
}

/// [`Reach::withdrawn`]: one set, shared by every copy of a reach.
#[derive(Debug, Clone, Default)]
struct Withdrawn(Arc<Mutex<BTreeSet<u16>>>);

impl Withdrawn {
    fn holds(&self, port: u16) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&port)
    }

    fn ports(&self) -> BTreeSet<u16> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Two reaches are alike where they withdrew the same ports, whether or not they share them.
impl PartialEq for Withdrawn {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.ports() == other.ports()
    }
}

impl Eq for Withdrawn {}

impl Reach {
    /// `listed`: each entry a host as [`super::egress::allows_on`] reads one, put in the order
    /// the layers decide (Open, Persona, then Allowed for this project on this machine, then
    /// for this chat), each layer's entries as written.
    pub fn of(mut listed: Vec<(String, By)>) -> Self {
        listed.sort_by_key(|(_, by)| match by {
            By::Open | By::Project => 0,
            By::Persona => 1,
            By::You => 2,
            By::Chat => 3,
        });
        Self {
            listed,
            never: Vec::new(),
            withdrawn: Withdrawn::default(),
            anything: false,
        }
    }

    /// **The floor's reach** (#2071): every host, but what [`Self::decide`] refuses whatever is
    /// listed.
    pub fn anything() -> Self {
        Self {
            anything: true,
            ..Self::default()
        }
    }

    /// Whether every host is carried ([`Self::anything`]).
    pub fn is_anything(&self) -> bool {
        self.anything
    }

    /// This reach without the floor ([`Self::anything`]): only what it lists, and what policy
    /// pins, with the same withdrawn ports. What a browser's lane reads (#2011).
    #[must_use]
    pub fn listed_only(&self) -> Self {
        Self {
            anything: false,
            ..self.clone()
        }
    }

    /// This, refusing `hosts` outright where nothing lists them: the hosts an administrator's
    /// policy pins as never allowed (#1666), which no person is asked about.
    #[must_use]
    pub fn never(self, hosts: Vec<Host>) -> Self {
        Self {
            never: hosts,
            ..self
        }
    }

    /// This, also listing each of `more` at its layer, where it is not listed there already,
    /// in the order the layers decide.
    #[must_use]
    pub fn and(self, more: impl IntoIterator<Item = (String, By)>) -> Self {
        let Self {
            mut listed,
            never,
            withdrawn,
            anything,
        } = self;
        for one in more {
            if !listed.contains(&one) {
                listed.push(one);
            }
        }
        Self {
            never,
            withdrawn,
            anything,
            ..Self::of(listed)
        }
    }

    /// This reach written down ([`Written`]), for the session host (#1999).
    pub fn written(&self) -> Written {
        Written {
            listed: self.listed.clone(),
            never: self.never.iter().map(ToString::to_string).collect(),
            withdrawn: self.withdrawn.ports().into_iter().collect(),
            anything: self.anything,
        }
    }

    /// **The reach `written` names** (#1999), exactly: its entries in the order written, never
    /// sorted again, and a withdrawn set of its own. Refused where a host policy pins does not
    /// read as a host, rather than read without it.
    pub fn from_written(written: &Written) -> Result<Self, String> {
        let never = written
            .never
            .iter()
            .map(|typed| Host::parse(typed))
            .collect::<Result<Vec<Host>, String>>()?;
        Ok(Self {
            listed: written.listed.clone(),
            never,
            withdrawn: Withdrawn(Arc::new(Mutex::new(
                written.withdrawn.iter().copied().collect(),
            ))),
            anything: written.anything,
        })
    }

    /// `hosts`, every one an Open host.
    pub fn open(hosts: Vec<String>) -> Self {
        Self::of(hosts.into_iter().map(|host| (host, By::Open)).collect())
    }

    /// Each host listed, once, in the order the layers decide.
    pub fn hosts(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (host, _) in &self.listed {
            if !out.contains(host) {
                out.push(host.clone());
            }
        }
        out
    }

    /// **The decision** for a connection to `host` on `port`, where a host listed without a
    /// port is carried on `defaults`, and `own` is this machine's interface addresses.
    pub fn decide(&self, host: &str, port: u16, defaults: &[u16], own: &[IpAddr]) -> Decision {
        let host = host.strip_suffix('.').unwrap_or(host);
        let literal = host.parse::<IpAddr>().ok();
        if let Some(ip) = literal
            && refused_address(ip, own)
        {
            return match self.exactly(ip, port) {
                Some(by) => Decision::of(by),
                None => Decision::Refused(Refused::LocalAddress),
            };
        }
        if let Some((_, by)) = self
            .listed
            .iter()
            .find(|(entry, _)| allows_on(std::slice::from_ref(entry), host, port, defaults))
        {
            return Decision::of(*by);
        }
        let named = super::egress::host_and_port(host, port);
        match Host::parse(&named) {
            Err(why) => Decision::Refused(Refused::NeverAHost(why)),
            Ok(parsed) if parsed.to_string().starts_with("*.") => Decision::Refused(
                Refused::NeverAHost(format!("{named} names many hosts, not one.")),
            ),
            Ok(parsed) if self.never.iter().any(|pinned| pinned.covers(&parsed)) => {
                Decision::Refused(Refused::Policy)
            }
            Ok(_) if self.anything => Decision::Open,
            Ok(_) => Decision::Ask,
        }
    }

    /// [`Self::decide`], where a host listed without a port is carried on every port: what
    /// Claude Code's own proxy did, by name alone, so a Claude Code chat reaches through purlis's
    /// proxy what it reached before (#1665). The local-address check is the same.
    pub fn decide_on_any_port(&self, host: &str, port: u16, own: &[IpAddr]) -> Decision {
        self.decide(host, port, &[port], own)
    }

    /// Whether `ip` on `port` is listed as exactly that address and that port: the one way a
    /// local address is ever carried.
    pub fn lists_exactly(&self, ip: IpAddr, port: u16) -> bool {
        self.exactly(ip, port).is_some()
    }

    /// **[`Self::decide`], with a service on this Mac reached by name** (qw384b): what the
    /// chat's proxy asks. A client that never sends this machine's own addresses to a proxy
    /// (Claude Code's `NO_PROXY`, every Go client's built-in bypass) still sends a name, so a
    /// name of this Mac ([`localport::NAMES_OF_THIS_MAC`], a fixed list, never looked up) on a
    /// port is that port of this machine's loopback: carried at the layer that granted it, and
    /// refused as this machine where none did, never asked about as a host. Any other name is
    /// decided as [`Self::decide`] decides it, and no name is resolved here.
    ///
    /// Only while the port stands: one [`Self::withdraw`]n is carried no longer.
    pub fn decide_by_name(
        &self,
        host: &str,
        port: u16,
        defaults: &[u16],
        own: &[IpAddr],
    ) -> Decision {
        if localport::is_a_name_of_this_mac(host) {
            return self.on_this_mac(port);
        }
        self.decide(host, port, defaults, own)
    }

    /// **Where the proxy connects for `host` on `port`, never looking it up**: this machine's
    /// loopback, for a name of this Mac on a port the chat holds. Pinned, so no lookup at
    /// connect can send such a connection anywhere else (#2097 review).
    pub fn address_of(&self, host: &str, port: u16) -> Option<SocketAddr> {
        (localport::is_a_name_of_this_mac(host) && self.on_this_mac(port).carries())
            .then(|| SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
    }

    /// **`port` of this machine is carried no longer**, by any copy of this reach (the chat's
    /// proxy holds one): its grant was removed while the chat runs (qw384b).
    pub fn withdraw(&self, port: u16) {
        self.withdrawn
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(port);
    }

    /// The decision for `port` of this machine's loopback, by whichever name it was reached.
    fn on_this_mac(&self, port: u16) -> Decision {
        match self
            .exactly(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
            .or_else(|| self.exactly(IpAddr::V6(Ipv6Addr::LOCALHOST), port))
        {
            Some(by) => Decision::of(by),
            None => Decision::Refused(Refused::LocalAddress),
        }
    }

    fn exactly(&self, ip: IpAddr, port: u16) -> Option<By> {
        if ip.is_loopback() && self.withdrawn.holds(port) {
            return None;
        }
        self.listed.iter().find_map(|(entry, by)| {
            let (name, held) = entry_parts(entry);
            (held == Some(port) && name.parse::<IpAddr>().is_ok_and(|listed| listed == ip))
                .then_some(*by)
        })
    }
}
