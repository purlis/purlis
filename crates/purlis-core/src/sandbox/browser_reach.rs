// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A browser's reach proxy** (#2011, spec #2008): each browser's only route out, built from
//! the proxy a chat's sandbox runs ([`super::egress`]) and deciding by a lane of its chat's
//! reach ([`super::reach`]): the same grants, one Granted list.
//!
//! - **By host and port, without opening TLS.** A `CONNECT host:port` is decided by its target;
//!   a plain request by its absolute target, whose one `Host` must name the same host. The
//!   head is read as strictly as a chat's proxy reads it ([`super::egress`]).
//! - **Public addresses only** ([`public_address`]): never this machine, a private, shared,
//!   link-local, metadata, multicast or reserved address, whatever the chat's reach lists.
//! - **A named local port by its literal address alone**: `127.0.0.1:N` or `[::1]:N`, where
//!   the chat's reach lists exactly that address and port (a port the person allowed). A name
//!   for this machine (`localhost`, a name of this Mac) is refused, and so is a name that ends
//!   in a number, which is an address written another way.
//! - **It resolves names itself, once**, and connects only at the public addresses it resolved
//!   ([`route`]), so the address it checked is the one it connects to.
//! - **A new site** (public, and listed by nothing) is told to [`NewSites`], which refuses it
//!   until a browser's Ask answers there.
//! - **It fails closed.** The engine resolves no name and sends every request to this proxy
//!   (`purlis_browser_engine::launch`); when the proxy is dropped it stops listening, and every
//!   load fails.

use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use super::egress::{
    LIMITS, Limits, PATIENCE, PLAIN_PORTS, Request, TUNNEL_PORTS, answer, read_head, same_host,
    splice,
};
use super::hosts::{embedded_v4, ends_in_a_number, refused_address};
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
    NewSite {
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
}

impl Lane {
    /// The browser's lane of `reach`, its chat's.
    pub fn of(reach: Reach) -> Self {
        Self { reach }
    }

    /// **The verdict for a connection to `host` on `port`**, a tunnel (`CONNECT`) or a plain
    /// request, with `own` this machine's interface addresses. By host and port alone:
    ///
    /// - An address is judged as itself. This machine is carried only at `127.0.0.1` or `::1`
    ///   on a port the chat's reach lists exactly there (a named local port); any other address
    ///   that is not public ([`public_address`]) is refused, whatever lists it.
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
            Decision::Ask if defaults.contains(&port) => Verdict::NewSite { at },
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
        if ip == IpAddr::V4(Ipv4Addr::LOCALHOST) || ip == IpAddr::V6(Ipv6Addr::LOCALHOST) {
            // Carried where the chat's reach lists exactly that address and port, at the layer
            // that lists it ([`Reach::lists_exactly`]), and while it is not withdrawn.
            let by = self.reach.decide(&ip.to_string(), port, &[], own);
            if by.carries() {
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

/// A public site nothing in the browser's lane lists, as the connection named it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSite {
    pub host: String,
    pub port: u16,
}

/// **Whoever hears the browser's new sites**, and says whether each is carried. The seam a
/// browser's Ask meets; until it does, every new site is refused ([`NewSites::refusing`]), so
/// a browser reaches only what its chat's reach lists. What it carries is still connected to
/// only at public addresses ([`route`]).
#[derive(Clone)]
pub struct NewSites(Arc<dyn Fn(&NewSite) -> bool + Send + Sync>);

impl std::fmt::Debug for NewSites {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NewSites")
    }
}

impl NewSites {
    /// Told each new site; `true` carries it.
    pub fn new(hear: impl Fn(&NewSite) -> bool + Send + Sync + 'static) -> Self {
        Self(Arc::new(hear))
    }

    /// Refuses every new site.
    pub fn refusing() -> Self {
        Self::new(|_| false)
    }

    /// Whether `site` is carried.
    pub fn carries(&self, site: &NewSite) -> bool {
        (self.0)(site)
    }
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
    limits: Limits,
}

impl ReachProxy {
    /// A reach proxy for `lane`, telling `new_sites` each public site nothing lists, within a
    /// chat proxy's [`LIMITS`].
    pub fn start(lane: Lane, new_sites: NewSites) -> io::Result<Self> {
        Self::within(lane, new_sites, LIMITS)
    }

    /// [`Self::start`], within `limits`.
    pub fn within(lane: Lane, new_sites: NewSites, limits: Limits) -> io::Result<Self> {
        // Never a port a chat may be allowed, and kept as purlis's own while it listens.
        let listener = super::localport::bind_loopback()?;
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let served = Arc::new(Served {
            lane,
            new_sites,
            limits,
        });
        let open = Arc::new(AtomicUsize::new(0));
        let stopping = Arc::clone(&stop);
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
                    let served = Arc::clone(&served);
                    let _ = std::thread::Builder::new()
                        .name("purlis-reach-conn".into())
                        .spawn(move || {
                            let _held = held;
                            serve(stream, &served);
                        });
                }
            });
        if let Err(why) = spawned {
            super::localport::release(address.port());
            return Err(why);
        }
        Ok(Self { address, stop })
    }

    /// The address the browser's engine is told to send everything to.
    pub fn address(&self) -> SocketAddr {
        self.address
    }
}

impl Drop for ReachProxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes the accept, which then sees the stop and closes the listener.
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_secs(1));
        super::localport::release(self.address.port());
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
fn serve(mut client: TcpStream, served: &Served) {
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
    let own = super::hosts::own_addresses();
    let tunnel = request.forward.is_none();
    let at = match served.lane.decide(host, port, tunnel, &own) {
        Verdict::Carry { at, .. } => at,
        Verdict::NewSite { at } => {
            let site = NewSite {
                host: host.to_owned(),
                port,
            };
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
    let resolve = |host: &str, port: u16| -> Vec<SocketAddr> {
        (host, port)
            .to_socket_addrs()
            .map(Iterator::collect)
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
                &format!("purlis's reach proxy found no address for {host}"),
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

/// Why a new site was refused, as the browser is told.
fn new_site_said(host: &str, port: u16) -> String {
    format!(
        "purlis's reach proxy does not carry this browser to {host}:{port}: it is a site new to \
         this project, and it is not allowed for this browser"
    )
}

/// Why a verdict refused, as the browser is told.
fn refusal_said(host: &str, port: u16, why: &Refusal) -> String {
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
        Refusal::NotPublic => {
            "it is a private, link-local, metadata or reserved address".to_owned()
        }
        Refusal::SpelledAnotherWay => {
            "it is an address written another way: write an IPv4 address as four numbers".to_owned()
        }
        Refusal::Port => "a site is reached on port 443 for https and 80 for http, unless its \
                          port is allowed"
            .to_owned(),
        Refusal::NeverAHost(why) => why.clone(),
        Refusal::Policy => "an administrator's policy pins it as never allowed".to_owned(),
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
        IpAddr::V6(ip) => match embedded_v4(ip) {
            Some(carried) => this_machine(IpAddr::V4(carried), own),
            None => ip.is_loopback() || ip.is_unspecified(),
        },
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
/// (`2001::/23`, Teredo among them).
pub fn public_address(ip: IpAddr, own: &[IpAddr]) -> bool {
    if refused_address(ip, own) {
        return false;
    }
    match ip {
        IpAddr::V4(ip) => public_v4(ip),
        IpAddr::V6(ip) => match embedded_v4(ip) {
            Some(carried) => public_address(IpAddr::V4(carried), own),
            None => public_v6(ip),
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
    global_unicast && !documentation && !ietf_assignments
}

#[cfg(test)]
mod tests;
