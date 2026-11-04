// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A browser's reach proxy** (#2011, spec #2008): each browser's only route out, built from
//! the proxy a chat's sandbox runs ([`super::egress`]) and deciding by a lane of its chat's
//! reach ([`super::reach`]): the same grants, one Granted list.
//!
//! - **By host and port, without opening TLS.** A `CONNECT host:port` is decided by its target;
//!   a plain request by its absolute target, whose one `Host` must name the same host. The
//!   head is read as strictly as a chat's proxy reads it ([`super::egress`]).
//! - **Public addresses** ([`public_address`]), and besides them only what the person allowed by
//!   its literal address and port: never a link-local, metadata, multicast or reserved address,
//!   whatever the chat's reach lists.
//! - **A named local port by its literal address alone**: `127.0.0.1:N` or `[::1]:N`, where
//!   the chat's reach lists exactly that address and port (a port the person allowed). A name
//!   for this machine (`localhost`, a name of this Mac) is refused, and so is a name that ends
//!   in a number, which is an address written another way.
//! - **A private address by its literal address and port alone** (ruled 2026-10-15, #2131): a
//!   private, shared or unique local address the chat's reach lists exactly with its port,
//!   such as `10.0.0.5:6443`. Never by a name, a range, another port or another spelling.
//! - **It resolves names itself, once and within a limit**, and connects only at the public
//!   addresses it resolved ([`route`]), so the address it checked is the one it connects to.
//! - **Public sites are readable; a site new to the project asks.** An address the agent writes
//!   for a site new to the project, or a form it posts there, asks the person once (#2018); a
//!   link the page offered does not. By host and port the proxy cannot tell these apart, so it
//!   tells every new site to [`NewSites`], which the browser's broker answers
//!   (`crate::browsers::judge`): it carries a site new to the project only where the browser's
//!   driver judged a request to it to go. A browser started with no reach refuses every new
//!   site ([`NewSites::refusing`]).
//! - **A browser holding a login reaches its declared origin alone** (#2016): once purlis is
//!   about to fill a login in it, its proxy is bound ([`Bound`]) and carries nothing but that
//!   site, whatever lists anything else, for as long as the browser lives; every connection it
//!   carried before is ended at that moment. A login whose form is on another site (#4995) also
//!   reaches its form's site from then on, so the login's page's own redirects, frames and token
//!   refreshes to its sign-in provider go (the person's ruling of 2026-11-04, #5152). The agent
//!   acts on that site's documents only during a sign-in ([`Bound::kept_off`]); the browser's
//!   driver refuses the rest.
//! - **A browser holding a secret purlis filled reaches that site alone while the page holding
//!   it lives** (#5117; the person's ruling SF-3 and decision SF-9 on #5096): from just before the
//!   value goes in ([`ReachProxy::hold_for_fill`]) every connection to any other site is ended
//!   and a new one refused, until the browser tells that the page loaded another document in its
//!   main frame or was closed ([`ReachProxy::fill_released`]). It narrows a login's hold, its
//!   form's site among it, never widens it, and it holds the browser to one site at a time: a
//!   fill on another site while one holds is refused.
//! - **It fails closed.** The engine resolves no name and sends every request to this proxy
//!   (`purlis_browser_engine::launch`). When the proxy is dropped it stops listening and ends
//!   every connection it carries, tunnels included, so every load fails; where this machine's
//!   own addresses cannot be read, it carries nothing.

use std::collections::HashMap;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::egress::{
    LIMITS, Limits, PATIENCE, PLAIN_PORTS, Request, TUNNEL_PORTS, answer, read_head, same_host,
    splice,
};
use super::hosts::{carried_v4, ends_in_a_number, refused_address, tunnelled_v4};
use super::localport::is_a_name_of_this_mac;
use super::reach::{self, Decision, Reach};

/// **A browser's lane of its chat's reach**: the hosts the chat's sandbox lists, each with the
/// layer that lists it, judged by the browser's own rules ([`Lane::decide`]). The same grants,
/// read once more: nothing is listed for a browser that its chat's reach does not list.
#[derive(Debug, Clone)]
pub struct Lane {
    reach: Reach,
}

/// What a browser's connection to a host and port is answered with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Carried, by the layer that lists it. `at` is the one address it is connected to, where
    /// it was named by its address; a name is resolved at connect ([`route`]).
    Carry {
        by: Decision,
        at: Option<SocketAddr>,
    },
    /// A public site nothing lists, on HTTPS's or HTTP's port: whoever hears new sites decides
    /// ([`NewSites`]).
    Site {
        at: Option<SocketAddr>,
    },
    Refused(Refusal),
}

/// Why a browser's connection is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// This machine, by an address that is not a named local port's.
    ThisMachine,
    /// This machine by a name: a named local port is reached by its literal address alone.
    ThisMachineByName,
    /// A private, shared, link-local, metadata, multicast or reserved address.
    NotPublic,
    /// A name that ends in a number: an address written another way.
    SpelledAnotherWay,
    /// A port nothing lists for this host: a site nothing lists is reached on HTTPS's port by
    /// a tunnel and on HTTP's by a plain request.
    Port,
    /// Not a host anyone could allow, and why.
    NeverAHost(String),
    /// A host an administrator's policy pins as never allowed.
    Policy,
    /// This machine's own addresses could not be read, so no address can be told apart from
    /// them: nothing is carried.
    ThisMachineUnknown,
}

impl Lane {
    /// The browser's lane of `reach`, its chat's: what it lists, never the floor's reach of a
    /// chat without the sandbox ([`Reach::anything`]), so a browser reaches the same whatever its
    /// chat's sandbox: a public site nothing lists is a new site, and a private address is
    /// carried only where it is listed exactly.
    pub fn of(reach: Reach) -> Self {
        Self {
            reach: reach.listed_only(),
        }
    }

    /// [`Self::decide`], with `own` as it was read: refused outright where this machine's
    /// addresses could not be read (`None`) or the system listed none.
    pub fn decide_here(
        &self,
        host: &str,
        port: u16,
        tunnel: bool,
        own: Option<&[IpAddr]>,
    ) -> Verdict {
        match own {
            Some(own) if !own.is_empty() => self.decide(host, port, tunnel, own),
            _ => Verdict::Refused(Refusal::ThisMachineUnknown),
        }
    }

    /// **The verdict for a connection to `host` on `port`**, a tunnel (`CONNECT`) or a plain
    /// request, with `own` this machine's interface addresses. By host and port alone:
    ///
    /// - An address is judged as itself. This machine is carried only at `127.0.0.1` or `::1`
    ///   on a port the chat's reach lists exactly there (a named local port); a private address
    ///   ([`private_address`]) only where the chat's reach lists exactly it and that port; any
    ///   other address that is not public ([`public_address`]) is refused, whatever lists it.
    /// - A name of this machine (`localhost`, a name under it, a name of this Mac) is refused:
    ///   a named local port is reached by its literal address alone. A name that ends in a
    ///   number is an address written another way, and refused.
    /// - Any other host is carried where the chat's reach lists it on that port, and is a new
    ///   site where nothing does, on HTTPS's port by a tunnel or HTTP's by a plain request.
    ///
    /// Whatever a name resolves to is judged again at connect ([`route`]).
    pub fn decide(&self, host: &str, port: u16, tunnel: bool, own: &[IpAddr]) -> Verdict {
        let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
        if host.is_empty() {
            return Verdict::Refused(Refusal::NeverAHost("A host is named.".to_owned()));
        }
        let defaults: &[u16] = if tunnel { &TUNNEL_PORTS } else { &PLAIN_PORTS };
        let at = match host.parse::<IpAddr>() {
            Ok(ip) => {
                if let Some(refused) = self.local(ip, port, own) {
                    return refused;
                }
                Some(SocketAddr::new(ip, port))
            }
            Err(_) if host.contains([':', '%', '[', ']']) => {
                return Verdict::Refused(Refusal::NeverAHost(format!(
                    "{host} is not an address or a name."
                )));
            }
            Err(_) if host == "localhost" || host.ends_with(".localhost") => {
                return Verdict::Refused(Refusal::ThisMachineByName);
            }
            Err(_) if is_a_name_of_this_mac(&host) => {
                return Verdict::Refused(Refusal::ThisMachineByName);
            }
            Err(_) if ends_in_a_number(&host) => {
                return Verdict::Refused(Refusal::SpelledAnotherWay);
            }
            Err(_) => None,
        };
        match self.reach.decide(&host, port, defaults, own) {
            Decision::Ask if defaults.contains(&port) => Verdict::Site { at },
            Decision::Ask => Verdict::Refused(Refusal::Port),
            Decision::Refused(reach::Refused::NeverAHost(why)) => {
                Verdict::Refused(Refusal::NeverAHost(why))
            }
            Decision::Refused(reach::Refused::Policy) => Verdict::Refused(Refusal::Policy),
            // Never for a public address or a name of no local machine; refused all the same.
            Decision::Refused(reach::Refused::LocalAddress) => Verdict::Refused(Refusal::NotPublic),
            by => Verdict::Carry { by, at },
        }
    }

    /// The verdict on `ip` on `port` where it is not public, or none where it is.
    fn local(&self, ip: IpAddr, port: u16, own: &[IpAddr]) -> Option<Verdict> {
        if public_address(ip, own) {
            return None;
        }
        let named_loopback =
            ip == IpAddr::V4(Ipv4Addr::LOCALHOST) || ip == IpAddr::V6(Ipv6Addr::LOCALHOST);
        let named_private = private_address(ip, own);
        if named_loopback || named_private {
            // Carried only where the chat's reach lists exactly that address and port
            // ([`Reach::lists_exactly`]), at the layer that lists it, and (for a local port)
            // while it is not withdrawn: decided with no default port, so only an entry held to
            // this port can match.
            let by = self.reach.decide(&ip.to_string(), port, &[], own);
            if self.reach.lists_exactly(ip, port) && by.carries() {
                return Some(Verdict::Carry {
                    by,
                    at: Some(SocketAddr::new(ip, port)),
                });
            }
        }
        Some(Verdict::Refused(if this_machine(ip, own) {
            Refusal::ThisMachine
        } else {
            Refusal::NotPublic
        }))
    }
}

/// **Where a carried connection to `host` on `port` is made**: at `at`, the address it was named
/// by, never looked up; or, for a name, at the addresses `resolve` answers, asked once, less each
/// one that is not public ([`public_address`], with `own` this machine's interface addresses).
/// The addresses checked are the ones connected to, so a name rebound between a lookup and a
/// connect reaches nothing it was not checked for.
pub fn route(
    host: &str,
    port: u16,
    at: Option<SocketAddr>,
    own: &[IpAddr],
    resolve: &dyn Fn(&str, u16) -> Vec<SocketAddr>,
) -> Route {
    if let Some(at) = at {
        return Route::To(vec![at]);
    }
    let resolved = resolve(host, port);
    if resolved.is_empty() {
        return Route::NotFound;
    }
    let public: Vec<SocketAddr> = resolved
        .into_iter()
        .filter(|addr| public_address(addr.ip(), own))
        .collect();
    if public.is_empty() {
        Route::NotPublic
    } else {
        Route::To(public)
    }
}

/// Where [`route`] connects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Each address to try, in order.
    To(Vec<SocketAddr>),
    /// The name resolved only to addresses that are not public: refused.
    NotPublic,
    /// The name resolved to nothing.
    NotFound,
}

/// **A site a browser goes to**: a host and the one port it is reached on, in one spelling.
/// What the proxy is told of a site nothing lists, what a browser asks the person about, and
/// what a browser site grant names (#2018). Written `host:port`, an IPv6 address in brackets.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Site {
    pub host: String,
    pub port: u16,
}

impl Site {
    /// `host` on `port` in one spelling: lower case, with no final dot, an IPv6 address
    /// without its brackets.
    pub fn of(host: &str, port: u16) -> Self {
        let host = host.strip_suffix('.').unwrap_or(host);
        let host = host
            .strip_prefix('[')
            .and_then(|inside| inside.strip_suffix(']'))
            .unwrap_or(host);
        Self {
            host: host.to_ascii_lowercase(),
            port,
        }
    }

    /// **A site as `host:port` writes it**, or why it is none: a host that is a name or an
    /// address, and a port from 1 to 65535. Nothing is resolved.
    pub fn parse(written: &str) -> Result<Self, String> {
        let refused = || format!("{written:?} is not a site, written host:port");
        let (host, port) = match written.strip_prefix('[') {
            Some(inside) => {
                let (host, after) = inside.split_once(']').ok_or_else(refused)?;
                host.parse::<Ipv6Addr>().map_err(|_| refused())?;
                (host, after.strip_prefix(':').ok_or_else(refused)?)
            }
            None => written.rsplit_once(':').ok_or_else(refused)?,
        };
        let port: u16 = port.parse().map_err(|_| refused())?;
        let fine = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_');
        if port == 0 || host.is_empty() || !host.chars().all(fine) {
            return Err(refused());
        }
        if host.contains(':') && host.parse::<Ipv6Addr>().is_err() {
            return Err(refused());
        }
        Ok(Self::of(host, port))
    }
}

impl std::fmt::Display for Site {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.host.contains(':') {
            write!(f, "[{}]:{}", self.host, self.port)
        } else {
            write!(f, "{}:{}", self.host, self.port)
        }
    }
}

/// **Whoever hears the browser's new sites**, and says whether each is carried: the seam the
/// browser's broker answers, carrying a link the page offered and asking the person once for
/// an address the agent wrote or a form it posts (#2018). What it carries is still connected
/// to only at public addresses ([`route`]).
#[derive(Clone)]
pub struct NewSites(Arc<dyn Fn(&Site) -> bool + Send + Sync>);

impl std::fmt::Debug for NewSites {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NewSites")
    }
}

impl NewSites {
    /// Told each new site; `true` carries it.
    pub fn new(hear: impl Fn(&Site) -> bool + Send + Sync + 'static) -> Self {
        Self(Arc::new(hear))
    }

    /// Refuses every new site: a browser started with no reach reaches only what its chat's
    /// reach lists.
    pub fn refusing() -> Self {
        Self::new(|_| false)
    }

    /// Carries every new site (#2013): a browser the person let reach the open web. What it
    /// carries is still connected to only at public addresses ([`route`]).
    pub fn carrying() -> Self {
        Self::new(|_| true)
    }

    /// Whether `site` is carried.
    pub fn carries(&self, site: &Site) -> bool {
        (self.0)(site)
    }
}

/// **The sites one login is held to** (#2016): its page's, and its form's where the form is on
/// another site (#4995).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginSites {
    pub page: Site,
    pub form: Option<Site>,
}

impl LoginSites {
    /// A login whose form is on its page's own site.
    pub fn page(page: Site) -> Self {
        Self { page, form: None }
    }

    /// Its sites, its form's first.
    pub fn all(&self) -> Vec<Site> {
        self.form
            .iter()
            .chain(std::iter::once(&self.page))
            .cloned()
            .collect()
    }
}

/// **The sites a browser holding a login reaches, and nothing else** (#2016): unbound until
/// purlis is about to fill a login in the browser, then bound to that login's page's site for as
/// long as the browser lives, and to its form's site where that is another (#4995, #5152). A
/// login whose form is on another site opens a sign-in, until it is over
/// ([`Self::sign_in_over`]): the agent acts on the form's site's documents only while one is
/// open ([`Self::kept_off`]). And while a page holding a secret purlis filled lives,
/// the browser reaches that secret's site alone (#5117, [`Self::hold_for_fill`]). Shared by the
/// browser's reach proxy, which refuses every other site, its judge, and its driver.
#[derive(Clone, Default)]
pub struct Bound(Arc<Mutex<Binding>>);

/// What a browser is bound to: a login, held for its life, and the fills held while their pages
/// live.
#[derive(Debug, Default)]
struct Binding {
    login: Option<Holding>,
    fills: Vec<FillHeld>,
    /// Fill holds made in this browser so far: each one's number.
    fills_made: u64,
}

/// **A page holding a secret purlis filled, while it lives** (#5117): the site the secret was
/// filled on, and its number.
#[derive(Debug, Clone)]
struct FillHeld {
    site: Site,
    number: u64,
}

/// What a bound browser holds: its login's page's site, its form's, and the sign-in open now, if
/// any.
#[derive(Debug, Clone)]
struct Holding {
    page: Site,
    sign_in: Option<SignIn>,
    /// The form's site its first sign-in reached, reached from then on: the only one a later
    /// sign-in may (#4995, #5152).
    form: Option<Site>,
    /// Sign-ins opened in this browser so far: each one's number.
    opened: u64,
}

/// **A sign-in open now** (#4995): the form's site the agent may act on, and its number.
#[derive(Debug, Clone)]
struct SignIn {
    form: Site,
    number: u64,
}

impl std::fmt::Debug for Bound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Bound")
            .field(&self.sites())
            .field(&self.held_for_fill())
            .finish()
    }
}

impl Bound {
    fn lock(&self) -> std::sync::MutexGuard<'_, Binding> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **Holds the browser to `site` while a page holding a secret purlis filled there lives**
    /// (#5117): answered is the hold's number, for [`Self::fill_released`]. Refused where a page
    /// holding a secret filled on another site lives: the browser is held to one site at a time.
    pub fn hold_for_fill(&self, site: &Site) -> Result<u64, String> {
        let mut bound = self.lock();
        if let Some(other) = bound.fills.iter().find(|held| held.site != *site) {
            return Err(format!(
                "this browser is held to {} while a page holding a secret purlis filled there \
                 is open, so it takes no secret for {site}",
                other.site
            ));
        }
        bound.fills_made += 1;
        let number = bound.fills_made;
        bound.fills.push(FillHeld {
            site: site.clone(),
            number,
        });
        Ok(number)
    }

    /// **Fill hold `number` is over** (#5117): its page loaded another document in its main frame,
    /// or was closed. Whether it was still held.
    pub fn fill_released(&self, number: u64) -> bool {
        let mut bound = self.lock();
        let before = bound.fills.len();
        bound.fills.retain(|held| held.number != number);
        bound.fills.len() != before
    }

    /// The site a page holding a secret purlis filled holds the browser to, while one does.
    pub fn held_for_fill(&self) -> Option<Site> {
        self.lock().fills.first().map(|held| held.site.clone())
    }

    /// **Why a connection to `host` on `port` may not go**, as the browser is told, or `None`
    /// where it may ([`Self::lets`]).
    pub fn refuses(&self, host: &str, port: u16) -> Option<String> {
        let site = Site::of(host, port);
        let bound = self.lock();
        if let Some(held) = bound.fills.iter().find(|held| held.site != site) {
            return Some(fill_held_said(&site, &held.site));
        }
        let login_lets = bound
            .login
            .as_ref()
            .is_none_or(|holding| holding.page == site || holding.form.as_ref() == Some(&site));
        (!login_lets).then(|| bound_said(host, port))
    }

    /// **Binds the browser to `at`**, one login's sites, or why it may not be: one already bound
    /// to another login's page stays bound to that, since a browser holds one login's session
    /// at a time. Binding again to the same page changes nothing of it. Where the login's form
    /// is on another site, the browser reaches it from now on, and a sign-in opens, in place of
    /// any open before (#4995, #5152): what is answered is its number. A browser signs in on one
    /// form's site: a login of its page whose form is on another is refused.
    pub fn to(&self, at: &LoginSites) -> Result<Option<u64>, String> {
        let mut bound = self.lock();
        let bound = &mut bound.login;
        let holding = match bound.as_mut() {
            None => bound.insert(Holding {
                page: at.page.clone(),
                sign_in: None,
                form: None,
                opened: 0,
            }),
            Some(holding) if holding.page == at.page => holding,
            Some(holding) => {
                return Err(format!(
                    "this browser holds a login at {}, and reaches nothing else",
                    holding.page
                ));
            }
        };
        let Some(form) = &at.form else {
            return Ok(None);
        };
        // A browser signs in on one form's site, whichever login of its page asks.
        if let Some(signed_in_on) = &holding.form
            && signed_in_on != form
        {
            return Err(format!(
                "this browser signs in to {} on {signed_in_on}, and on no other site",
                holding.page
            ));
        }
        holding.form = Some(form.clone());
        holding.opened += 1;
        let number = holding.opened;
        holding.sign_in = Some(SignIn {
            form: form.clone(),
            number,
        });
        Ok(Some(number))
    }

    /// **Sign-in `number` is over** (#4995, #5152): the agent acts on the form's site's
    /// documents no more, though the browser still reaches it. Answers that site, where that
    /// sign-in was still open; a sign-in already over, or one another replaced, changes nothing.
    pub fn sign_in_over(&self, number: u64) -> Option<Site> {
        let mut bound = self.lock();
        let holding = bound.login.as_mut()?;
        if holding.sign_in.as_ref()?.number != number {
            return None;
        }
        holding.sign_in.take().map(|over| over.form)
    }

    /// The form's site sign-in `number` reaches, while it is open (#4995).
    pub fn signing_in(&self, number: u64) -> Option<Site> {
        let bound = self.lock();
        let open = bound.login.as_ref()?.sign_in.as_ref()?;
        (open.number == number).then(|| open.form.clone())
    }

    /// The site of the login's page it is bound to, where it is.
    pub fn page(&self) -> Option<Site> {
        self.lock()
            .login
            .as_ref()
            .map(|holding| holding.page.clone())
    }

    /// The sites it is bound to, where it is: its form's first, where it signed in on another.
    pub fn sites(&self) -> Option<Vec<Site>> {
        self.lock().login.as_ref().map(|holding| {
            holding
                .form
                .iter()
                .cloned()
                .chain(std::iter::once(holding.page.clone()))
                .collect()
        })
    }

    /// The site of the form a login of its page signs in on, where that is another (#5152).
    pub fn form(&self) -> Option<Site> {
        self.lock().login.as_ref()?.form.clone()
    }

    /// Whether a connection to `host` on `port` may go: always while unbound; once bound to a
    /// login, only to its page's site, or the site its login signs in on (#5152); and while a
    /// page holding a secret purlis filled lives, only to that secret's site (#5117).
    pub fn lets(&self, host: &str, port: u16) -> bool {
        self.refuses(host, port).is_none()
    }

    /// **The site the agent is kept off at `host` on `port`, now** (#5152, the person's ruling
    /// of 2026-11-04): the site its login signs in on, where that is `host` on `port`, while no
    /// sign-in is open. The browser loads that site's documents (the page's own redirects,
    /// frames and token refreshes); the agent neither reads them nor acts on them.
    ///
    /// The web's two default ports of the form's host are one site here: an `http://` address
    /// the browser upgrades reaches the `https://` one.
    pub fn kept_off(&self, host: &str, port: u16) -> Option<Site> {
        let site = Site::of(host, port);
        let bound = self.lock();
        let holding = bound.login.as_ref()?;
        let form = holding.form.as_ref()?;
        let web = |port: u16| matches!(port, 80 | 443);
        let same = *form == site || (form.host == site.host && web(form.port) && web(site.port));
        (same && holding.sign_in.is_none()).then(|| form.clone())
    }
}

/// How long a name's lookup may take before the connection is given up on.
pub const LOOKUP_PATIENCE: Duration = Duration::from_secs(10);

/// The most lookups running at once across every reach proxy: one more is refused at once, so
/// a resolver that never answers holds a bounded number of threads.
const LOOKUPS_AT_ONCE: usize = 32;

/// Lookups running now ([`LOOKUPS_AT_ONCE`]).
static LOOKUPS: AtomicUsize = AtomicUsize::new(0);

/// **`resolve` run on a thread of its own, waited for at most `within`**: what it answered, or
/// `None` where it took longer or too many lookups are running. A lookup given up on runs
/// to its end on its own thread and its answer is dropped.
pub fn lookup_within<F>(within: Duration, resolve: F) -> Option<Vec<SocketAddr>>
where
    F: FnOnce() -> Vec<SocketAddr> + Send + 'static,
{
    lookup_bounded(&LOOKUPS, LOOKUPS_AT_ONCE, within, resolve)
}

/// [`lookup_within`], counting the lookups running in `running` and refusing one past
/// `at_once` at once.
pub fn lookup_bounded<F>(
    running: &'static AtomicUsize,
    at_once: usize,
    within: Duration,
    resolve: F,
) -> Option<Vec<SocketAddr>>
where
    F: FnOnce() -> Vec<SocketAddr> + Send + 'static,
{
    if running.fetch_add(1, Ordering::SeqCst) >= at_once {
        running.fetch_sub(1, Ordering::SeqCst);
        return None;
    }
    let (send, answered) = std::sync::mpsc::sync_channel(1);
    let spawned = std::thread::Builder::new()
        .name("purlis-reach-lookup".into())
        .spawn(move || {
            let found = resolve();
            running.fetch_sub(1, Ordering::SeqCst);
            let _ = send.send(found);
        });
    if spawned.is_err() {
        running.fetch_sub(1, Ordering::SeqCst);
        return None;
    }
    answered.recv_timeout(within).ok()
}

/// How long a failing accept waits before it tries again.
const BACKOFF: Duration = Duration::from_millis(100);

/// **A browser's reach proxy**: one HTTP proxy port on the loopback interface, the browser's
/// only route out, deciding each connection by its [`Lane`]. It carries a `CONNECT` tunnel and
/// a plain absolute-form request, read as strictly as a chat's proxy reads them, and nothing
/// else: no SOCKS, no TLS of its own. It stops listening when dropped, so a browser whose proxy
/// is gone loads nothing.
pub struct ReachProxy {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    live: Live,
    bound: Bound,
    accepting: Option<std::thread::JoinHandle<()>>,
}

/// How long a dropped proxy waits for its accept loop to end.
const STOP_PATIENCE: Duration = Duration::from_secs(2);

/// **Every connection a proxy carries now**, each by its sockets (the browser's, and the
/// site's once connected), so a proxy that stops ends them all. Shared by its connections.
#[derive(Clone, Default)]
struct Live(Arc<Mutex<Connections>>);

#[derive(Default)]
struct Connections {
    /// Set once the proxy stops: a socket added after is shut at once.
    closed: bool,
    next: u64,
    open: HashMap<u64, Vec<TcpStream>>,
    /// The site each connection asked for, once it was read: a fill hold ends those to any
    /// other (#5117).
    to: HashMap<u64, Site>,
}

/// One connection's place in [`Live`], given up when the connection ends.
struct Entry {
    live: Live,
    id: u64,
}

impl Live {
    fn lock(&self) -> std::sync::MutexGuard<'_, Connections> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A new connection's entry, holding `client`.
    fn enter(&self, client: &TcpStream) -> Entry {
        let id = {
            let mut connections = self.lock();
            let id = connections.next;
            connections.next += 1;
            id
        };
        let entry = Entry {
            live: self.clone(),
            id,
        };
        entry.hold(client);
        entry
    }

    /// Ends every connection open now but those to `site` (#5117), and those whose site is not
    /// read yet, which are judged once it is; one that starts later goes on.
    fn end_open_but(&self, site: &Site) {
        let connections = self.lock();
        for (id, sockets) in &connections.open {
            if connections.to.get(id).is_some_and(|to| to != site) {
                for socket in sockets {
                    let _ = socket.shutdown(Shutdown::Both);
                }
            }
        }
    }

    /// Ends every connection open now; one that starts later goes on.
    fn end_open(&self) {
        let connections = self.lock();
        for socket in connections.open.values().flatten() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }

    /// Ends every connection, and every one that starts from now.
    fn close(&self) {
        let mut connections = self.lock();
        connections.closed = true;
        for socket in connections.open.values().flatten() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
}

impl Entry {
    /// Holds `socket` with this connection, or shuts it where the proxy has stopped. Whether
    /// the connection may go on.
    fn hold(&self, socket: &TcpStream) -> bool {
        let mut connections = self.live.lock();
        let Ok(copy) = socket.try_clone() else {
            let _ = socket.shutdown(Shutdown::Both);
            return false;
        };
        if connections.closed {
            let _ = socket.shutdown(Shutdown::Both);
            return false;
        }
        connections.open.entry(self.id).or_default().push(copy);
        true
    }
}

impl Entry {
    /// This connection goes to `site`: a fill hold on another ends it (#5117).
    fn to(&self, site: Site) {
        self.live.lock().to.insert(self.id, site);
    }
}

impl Drop for Entry {
    fn drop(&mut self) {
        let mut connections = self.live.lock();
        connections.open.remove(&self.id);
        connections.to.remove(&self.id);
    }
}

impl std::fmt::Debug for ReachProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReachProxy")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

/// What one reach proxy serves.
struct Served {
    lane: Lane,
    new_sites: NewSites,
    bound: Bound,
    limits: Limits,
}

impl ReachProxy {
    /// A reach proxy for `lane`, telling `new_sites` each public site nothing lists, within a
    /// chat proxy's [`LIMITS`].
    pub fn start(lane: Lane, new_sites: NewSites) -> io::Result<Self> {
        Self::within(lane, new_sites, LIMITS)
    }

    /// [`Self::start`], carrying nothing but what `bound` lets go once it is bound (#2016).
    pub fn bound_by(lane: Lane, new_sites: NewSites, bound: Bound) -> io::Result<Self> {
        Self::serving(lane, new_sites, bound, LIMITS)
    }

    /// [`Self::start`], within `limits`.
    pub fn within(lane: Lane, new_sites: NewSites, limits: Limits) -> io::Result<Self> {
        Self::serving(lane, new_sites, Bound::default(), limits)
    }

    /// **Binds the browser to `at`** ([`Bound::to`]), and ends every connection it carries
    /// now: from this moment it carries the login's page's site alone, for as long as it runs,
    /// and its form's where that is another (#4995, #5152). Answered is the number of the
    /// sign-in this opens, where it opens one, for [`Self::sign_in_over`].
    pub fn bind(&self, at: &LoginSites) -> Result<Option<u64>, String> {
        let sign_in = self.bound.to(at)?;
        self.live.end_open();
        Ok(sign_in)
    }

    /// **Opens the sign-in of a login of the page the browser is bound to already** (#4995,
    /// [`Bound::to`]), its connections left as they are: the login's page may send it to its
    /// form before anything is filled. Answers the sign-in's number, where the login opens one.
    pub fn sign_in(&self, at: &LoginSites) -> Result<Option<u64>, String> {
        self.bound.to(at)
    }

    /// **Sign-in `number` is over** (#4995, #5152, [`Bound::sign_in_over`]): the agent acts on
    /// the form's site's documents no more. The browser still reaches it, and no connection is
    /// ended. Whether that sign-in was still open.
    pub fn sign_in_over(&self, number: u64) -> bool {
        self.bound.sign_in_over(number).is_some()
    }

    /// **Holds the browser to `site` while a page holding a secret purlis filled there lives**
    /// (#5117, [`Bound::hold_for_fill`]), and ends every connection it carries now to any other
    /// site: from this moment it carries `site` alone, until [`Self::fill_released`]. Answered is
    /// the hold's number.
    pub fn hold_for_fill(&self, site: &Site) -> Result<u64, String> {
        let number = self.bound.hold_for_fill(site)?;
        self.live.end_open_but(site);
        Ok(number)
    }

    /// What it carries once bound, shared with the browser's judge.
    pub fn bound(&self) -> &Bound {
        &self.bound
    }

    /// **Fill hold `number` is over** (#5117, [`Bound::fill_released`]): its page is gone, and the
    /// browser reaches as it did before, unless another such page holds it. Whether it was held.
    pub fn fill_released(&self, number: u64) -> bool {
        self.bound.fill_released(number)
    }

    fn serving(lane: Lane, new_sites: NewSites, bound: Bound, limits: Limits) -> io::Result<Self> {
        // Never a port a chat may be allowed, and kept as purlis's own while it listens.
        let listener = super::localport::bind_loopback()?;
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let served = Arc::new(Served {
            lane,
            new_sites,
            bound: bound.clone(),
            limits,
        });
        let open = Arc::new(AtomicUsize::new(0));
        let stopping = Arc::clone(&stop);
        let live = Live::default();
        let living = live.clone();
        let spawned = std::thread::Builder::new()
            .name("purlis-reach".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if stopping.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(mut stream) = stream else {
                        std::thread::sleep(BACKOFF);
                        continue;
                    };
                    if open.fetch_add(1, Ordering::SeqCst) >= served.limits.connections {
                        open.fetch_sub(1, Ordering::SeqCst);
                        let _ = stream.set_write_timeout(Some(BACKOFF));
                        answer(
                            &mut stream,
                            "503 Service Unavailable",
                            "purlis's reach proxy is carrying all the connections it takes",
                        );
                        continue;
                    }
                    let held = Counted(Arc::clone(&open));
                    let entry = living.enter(&stream);
                    let served = Arc::clone(&served);
                    let _ = std::thread::Builder::new()
                        .name("purlis-reach-conn".into())
                        .spawn(move || {
                            let _held = held;
                            serve(stream, &served, &entry);
                        });
                }
                // The listener closes here, and only then is its port let go: never while
                // anything could still connect to it.
                drop(listener);
                super::localport::release(address.port());
            });
        let accepting = match spawned {
            Ok(accepting) => accepting,
            Err(why) => {
                super::localport::release(address.port());
                return Err(why);
            }
        };
        Ok(Self {
            address,
            stop,
            live,
            bound,
            accepting: Some(accepting),
        })
    }

    /// The address the browser's engine is told to send everything to.
    pub fn address(&self) -> SocketAddr {
        self.address
    }
}

impl Drop for ReachProxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Every connection it carries ends now, tunnels included.
        self.live.close();
        // Wakes the accept, which then sees the stop, closes the listener and lets its port go.
        let Some(accepting) = self.accepting.take() else {
            return;
        };
        let until = Instant::now() + STOP_PATIENCE;
        while !accepting.is_finished() && Instant::now() < until {
            let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(200));
            std::thread::sleep(Duration::from_millis(10));
        }
        if accepting.is_finished() {
            let _ = accepting.join();
        }
    }
}

/// One connection being served, counted until it ends.
struct Counted(Arc<AtomicUsize>);

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// One browser connection: its request read, decided, and carried or refused.
fn serve(mut client: TcpStream, served: &Served, entry: &Entry) {
    let idle = served.limits.idle;
    let _ = client.set_write_timeout(Some(idle.min(PATIENCE)));
    let Some((head, rest)) = read_head(&mut client, served.limits.head) else {
        answer(
            &mut client,
            "400 Bad Request",
            "purlis's reach proxy: no request",
        );
        return;
    };
    let Some(request) = Request::parse(&head) else {
        answer(
            &mut client,
            "400 Bad Request",
            "purlis's reach proxy carries CONNECT, and plain absolute-form requests whose body \
             states its length, only",
        );
        return;
    };
    let (host, port) = (request.host.as_str(), request.port);
    if let Some(named) = &request.host_header
        && !same_host(named, host, port)
    {
        answer(
            &mut client,
            "403 Forbidden",
            &format!(
                "purlis's reach proxy carries a request to {host} only when its Host names it"
            ),
        );
        return;
    }
    // Told before it is judged: a fill hold from here ends it, and the check after the connect
    // below refuses it where one held before (#5117).
    entry.to(Site::of(host, port));
    if let Some(said) = served.bound.refuses(host, port) {
        answer(&mut client, "403 Forbidden", &said);
        return;
    }
    let own = super::hosts::own_addresses_read();
    let tunnel = request.forward.is_none();
    let at = match served.lane.decide_here(host, port, tunnel, own.as_deref()) {
        Verdict::Carry { at, .. } => at,
        Verdict::Site { at } => {
            let site = Site::of(host, port);
            if !served.new_sites.carries(&site) {
                answer(&mut client, "403 Forbidden", &new_site_said(host, port));
                return;
            }
            at
        }
        Verdict::Refused(why) => {
            answer(
                &mut client,
                "403 Forbidden",
                &refusal_said(host, port, &why),
            );
            return;
        }
    };
    let own = own.unwrap_or_default();
    let resolve = |host: &str, port: u16| -> Vec<SocketAddr> {
        let host = host.to_owned();
        lookup_within(LOOKUP_PATIENCE, move || {
            (host.as_str(), port)
                .to_socket_addrs()
                .map(Iterator::collect)
                .unwrap_or_default()
        })
        .unwrap_or_default()
    };
    let addresses = match route(host, port, at, &own, &resolve) {
        Route::To(addresses) => addresses,
        Route::NotPublic => {
            answer(
                &mut client,
                "403 Forbidden",
                &refusal_said(host, port, &Refusal::NotPublic),
            );
            return;
        }
        Route::NotFound => {
            answer(
                &mut client,
                "502 Bad Gateway",
                &format!("purlis's reach proxy found no address for {host} in time"),
            );
            return;
        }
    };
    let Some(mut upstream) = addresses
        .iter()
        .find_map(|addr| TcpStream::connect_timeout(addr, PATIENCE).ok())
    else {
        answer(
            &mut client,
            "502 Bad Gateway",
            &format!("purlis's reach proxy could not reach {host}"),
        );
        return;
    };
    // Bound while it connected (#2016): every connection open then was ended, and so is this.
    if !entry.hold(&upstream) || !served.bound.lets(host, port) {
        return;
    }
    let _ = client.set_write_timeout(Some(idle));
    let _ = upstream.set_write_timeout(Some(idle));
    let sent = match &request.forward {
        None => client.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n"),
        Some(head) => upstream.write_all(head.as_bytes()),
    };
    if sent.is_err() {
        return;
    }
    // A plain request's body, and nothing after it; a tunnel's bytes, all of them.
    let up_to = request.body.unwrap_or(u64::MAX);
    let rest = &rest[..rest.len().min(usize::try_from(up_to).unwrap_or(usize::MAX))];
    if upstream.write_all(rest).is_err() {
        return;
    }
    splice(client, upstream, up_to - rest.len() as u64, idle);
}

/// Why a site was refused to a browser holding a login (#2016), as the browser is told.
fn bound_said(host: &str, port: u16) -> String {
    format!(
        "purlis's reach proxy does not carry this browser to {host}:{port}: the browser holds a \
         login, and reaches only the site it was logged in to"
    )
}

/// Why a site was refused to a browser held while a page holding a secret purlis filled lives
/// (#5117), as the browser is told.
fn fill_held_said(site: &Site, held: &Site) -> String {
    format!(
        "purlis's reach proxy does not carry this browser to {site}: a page in it holds a secret \
         purlis filled on {held}, and until that page loads another document or is closed the \
         browser reaches only {held}"
    )
}

/// Why a new site was refused, as the browser is told.
fn new_site_said(host: &str, port: u16) -> String {
    format!(
        "purlis's reach proxy does not carry this browser to {host}:{port}: it is a site new to \
         this project, and it is not allowed for this browser"
    )
}

/// Why a verdict refused, as the browser is told.
pub(crate) fn refusal_said(host: &str, port: u16, why: &Refusal) -> String {
    let because = match why {
        Refusal::ThisMachine => {
            "it is this machine, which a browser reaches only on a port the person allowed, at \
             127.0.0.1 or [::1]"
                .to_owned()
        }
        Refusal::ThisMachineByName => format!(
            "a port of this machine is reached by its address, 127.0.0.1:{port} or [::1]:{port}, \
             never by a name"
        ),
        Refusal::NotPublic => "it is a private, link-local, metadata or reserved address, and a \
                               private one is reached only by the address and port the person \
                               allowed"
            .to_owned(),
        Refusal::SpelledAnotherWay => {
            "it is an address written another way: write an IPv4 address as four numbers".to_owned()
        }
        Refusal::Port => "a site is reached on port 443 for https and 80 for http, unless its \
                          port is allowed"
            .to_owned(),
        Refusal::NeverAHost(why) => why.clone(),
        Refusal::Policy => "an administrator's policy pins it as never allowed".to_owned(),
        Refusal::ThisMachineUnknown => {
            "this machine's own addresses cannot be read, so no address is carried".to_owned()
        }
    };
    format!("purlis's reach proxy does not carry this browser to {host}:{port}: {because}")
}

/// Whether `ip` is this machine: one of `own`, a loopback or unspecified address, `0.0.0.0/8`,
/// or an IPv6 form carrying one of them.
fn this_machine(ip: IpAddr, own: &[IpAddr]) -> bool {
    if own.contains(&ip) {
        return true;
    }
    match ip {
        IpAddr::V4(ip) => ip.is_loopback() || ip.octets()[0] == 0,
        IpAddr::V6(ip) => {
            let as_itself = match carried_v4(ip).as_slice() {
                [] => ip.is_loopback() || ip.is_unspecified(),
                carried => carried
                    .iter()
                    .any(|one| this_machine(IpAddr::V4(*one), own)),
            };
            as_itself
                || tunnelled_v4(ip)
                    .into_iter()
                    .any(|one| this_machine(IpAddr::V4(one), own))
        }
    }
}

/// **Whether `ip` is a private address a person may allow a browser by its literal address and
/// port** (ruled 2026-10-15, #2131): a private IPv4 range, the carriers' shared space
/// (`100.64.0.0/10`) or an IPv6 unique local address (`fc00::/7`), written as itself (never an
/// IPv6 form carrying an IPv4 one), and none of this machine's (`own`), a link-local or a cloud
/// metadata address ([`refused_address`]).
pub fn private_address(ip: IpAddr, own: &[IpAddr]) -> bool {
    if refused_address(ip, own) {
        return false;
    }
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, ..] = ip.octets();
            ip.is_private() || (a == 100 && (64..128).contains(&b))
        }
        IpAddr::V6(ip) => {
            carried_v4(ip).is_empty()
                && tunnelled_v4(ip).is_empty()
                && ip.segments()[0] & 0xfe00 == 0xfc00
        }
    }
}

/// **Whether a browser may be carried to `ip`**: a public unicast address, and none of this
/// machine's (`own`, its interface addresses). Everything a chat is never carried to
/// ([`refused_address`]: this machine, link-local, multicast, broadcast, the cloud metadata
/// services) is refused, and besides it every private range, the carriers' shared space
/// (`100.64.0.0/10`), IPv6 unique local and site-local addresses, and the reserved,
/// documentation and benchmarking ranges. An IPv6 address that carries an IPv4 one (mapped,
/// SIIT, NAT64, IPv4-compatible, 6to4) is judged as the IPv4 one; outside those, only global
/// unicast (`2000::/3`) is public, less documentation and the IETF's own assignments
/// (`2001::/23`, Teredo among them) and Teredo's prefix before RFC 4380 (`3ffe:831f::/32`), and
/// an ISATAP address only where the IPv4 address it
/// carries is public too (#2678).
pub fn public_address(ip: IpAddr, own: &[IpAddr]) -> bool {
    if refused_address(ip, own) {
        return false;
    }
    match ip {
        IpAddr::V4(ip) => public_v4(ip),
        IpAddr::V6(ip) => match carried_v4(ip).as_slice() {
            [] => {
                public_v6(ip)
                    && tunnelled_v4(ip)
                        .into_iter()
                        .all(|one| public_address(IpAddr::V4(one), own))
            }
            carried => carried
                .iter()
                .all(|one| public_address(IpAddr::V4(*one), own)),
        },
    }
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    let shared = a == 100 && (64..128).contains(&b);
    let protocol_assignments = a == 192 && b == 0 && c == 0;
    let benchmarking = a == 198 && (b == 18 || b == 19);
    let reserved = a >= 240;
    !(ip.is_private()
        || ip.is_loopback()
        || a == 0
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || shared
        || protocol_assignments
        || benchmarking
        || reserved)
}

fn public_v6(ip: Ipv6Addr) -> bool {
    let [first, second, ..] = ip.segments();
    let global_unicast = first & 0xe000 == 0x2000;
    let documentation =
        (first == 0x2001 && second == 0x0db8) || (first == 0x3fff && second < 0x1000);
    let ietf_assignments = first == 0x2001 && second < 0x0200;
    let pre_rfc_teredo = first == 0x3ffe && second == 0x831f;
    global_unicast && !documentation && !ietf_assignments && !pre_rfc_teredo
}

#[cfg(test)]
mod tests;
