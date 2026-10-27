// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A service on this Mac** (qw384): something listening on this machine's loopback interface
//! at one port, such as a local Kubernetes cluster, a database in a container or a dev server,
//! as a grant names it ([`super::grant::What::Port`]).
//!
//! A sandboxed chat reaches this machine's own services by nothing but its own proxy's two
//! ports and the app's hook socket. A program that never uses a proxy for this machine (every
//! Go client: kubectl, helm, gh) connects to the port directly, and the chat's sandbox refuses
//! it. That refusal is a Block naming the port ([`crate::sandboxblock`]), and its Notice offers
//! Allow for this chat, or for every chat of this project on this machine.
//!
//! - **One port, never a range or every port** ([`port`]): a number from 1 to 65535, typed or
//!   named whole.
//! - **Never purlis's own** ([`own`]): a port one of purlis's proxies or tunnels listens on is
//!   each chat's own way out, and no other chat's. Refused when it is allowed, and left out when
//!   a chat starts. A proxy or tunnel purlis opens later never takes
//!   a port a chat's sandbox was started with ([`still_ports`], [`bind_loopback`]).
//! - **This machine's alone**, as a socket is: kept for one chat in memory or in this machine's
//!   local file (`super::local`), never in the committed one.
//! - **What it reaches**: the chat's sandbox lets it connect to that port on the loopback
//!   interface, `localhost`, `127.0.0.1` and `::1` alike, and the chat's proxy carries a client
//!   that does use it there ([`reach_entries`]).

use std::collections::BTreeSet;
use std::net::{IpAddr, TcpListener};
use std::sync::{Mutex, PoisonError};

use super::grant::Refused;

/// **The name a chat reaches a service on this Mac by, through its proxy** (qw384b):
/// `this-mac.internal:<port>`, carried to that port of the loopback interface while the chat
/// holds it ([`super::reach::Reach::decide_by_name`]). A client that never sends this
/// machine's own addresses to a proxy (Claude Code's `NO_PROXY`, every Go client) still sends a
/// name. Under `.internal`, which no public name is ever given.
pub const THIS_MAC: &str = "this-mac.internal";

/// **The names of this Mac a chat's proxy carries a granted port by** (qw384b, #2097 review): a
/// fixed list, never looked up. [`THIS_MAC`], purlis's own; `kubernetes.docker.internal`, which
/// Docker Desktop's cluster is named by; and `host.docker.internal`, which a container names this
/// machine by. Each is this machine's loopback to the proxy whatever a resolver would say, and no
/// other name is ever sent to a resolver on this account.
pub const NAMES_OF_THIS_MAC: [&str; 3] = [
    THIS_MAC,
    "kubernetes.docker.internal",
    "host.docker.internal",
];

/// Whether `host` is one of [`NAMES_OF_THIS_MAC`], in any case, a trailing dot or none.
pub fn is_a_name_of_this_mac(host: &str) -> bool {
    let named = host.strip_suffix('.').unwrap_or(host);
    NAMES_OF_THIS_MAC
        .iter()
        .any(|name| named.eq_ignore_ascii_case(name))
}

/// What a chat whose commands send no address of this machine to a proxy (Claude Code's) is
/// told of `port` once it is allowed (qw384b): the name its proxy carries it by.
pub fn reached_by_name(port: u16) -> String {
    format!(
        "Its commands reach it through this chat's proxy as `{THIS_MAC}:{port}`, never as \
         127.0.0.1 or localhost: `curl https://{THIS_MAC}:{port}`. A program that uses no proxy \
         cannot reach it."
    )
}

/// What allowing a service on this Mac means, in one honest line.
pub const RISK: &str = "lets this chat connect to whatever listens on that port on this Mac, as \
                        you; a container engine's or a cluster's port can reach much of your \
                        machine";

/// **What allowing the service on `port` of the runner a chat runs on means** (#2510), named
/// `runner` where its name is known: for that chat alone, as long as it runs there, never this
/// machine's.
pub fn on_the_runner_said(port: u16, runner: Option<&str>) -> String {
    let runner = match runner {
        Some(name) => format!("the runner {name}"),
        None => "the runner this chat runs on".to_owned(),
    };
    format!(
        "Allowing it lets this chat alone connect to whatever listens on port {port} of {runner}, \
         while the chat runs there. It allows nothing on this Mac."
    )
}

/// **Which loopback ports are whose, in this process**, under one lock: the ports purlis itself
/// listens on (each chat's proxy and each tunnel), and the ports a chat's sandbox was started
/// with as a service on this Mac. A port is never in both: a start gives a chat only a port
/// purlis does not hold ([`still_ports`]), and purlis takes only a port no chat was given
/// ([`bind_loopback`]), each judged and marked in one hold of the lock.
#[derive(Debug, Default)]
pub(crate) struct Ports {
    own: BTreeSet<u16>,
    given: BTreeSet<u16>,
}

impl Ports {
    const fn new() -> Self {
        Self {
            own: BTreeSet::new(),
            given: BTreeSet::new(),
        }
    }

    /// Whether `port` is purlis's own: one it listens on now, or a forge sign-in's redirect port,
    /// which it listens on while a sign-in runs.
    fn is_own(&self, port: u16) -> bool {
        self.own.contains(&port) || crate::forge::registration::REDIRECT_PORTS.contains(&port)
    }

    /// `ports`, less each that is purlis's own, each once, in order, and each kept marked as
    /// given: judged and marked together.
    pub(crate) fn admit(&mut self, ports: &[u16]) -> Vec<u16> {
        let mut out: Vec<u16> = Vec::new();
        for port in ports {
            if *port != 0 && !self.is_own(*port) && !out.contains(port) {
                out.push(*port);
            }
        }
        self.given.extend(out.iter().copied());
        out
    }

    /// Takes `port` as purlis's own unless a chat was given it: judged and marked together.
    pub(crate) fn claim(&mut self, port: u16) -> bool {
        if self.given.contains(&port) {
            return false;
        }
        self.own.insert(port);
        true
    }
}

/// This process's [`Ports`].
static PORTS: Mutex<Ports> = Mutex::new(Ports::new());

fn ports() -> std::sync::MutexGuard<'static, Ports> {
    PORTS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// **`typed` as the port of a service on this Mac**, or why not: one whole number from 1 to
/// 65535, never purlis's own.
pub fn port(typed: &str) -> Result<u16, Refused> {
    let typed = typed.trim();
    let number = typed
        .strip_prefix(':')
        .unwrap_or(typed)
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| {
            Refused::Not(
                "purlis allows a service on this Mac by one port, a number from 1 to 65535: \
                 never a range or every port."
                    .to_owned(),
            )
        })?;
    if is_own(number) {
        return Err(Refused::Never(OWN_SAID.to_owned()));
    }
    Ok(number)
}

/// Why purlis's own port is never allowed.
pub const OWN_SAID: &str = "That port is one purlis itself listens on, so no chat is allowed it. \
                            A service of your own on this Mac listens on a port of its own.";

/// What a Notice says of a port you allowed that purlis itself listens on now: the service you
/// allowed there is not running, purlis took the free port for a chat of its own, and the
/// service can take it back only once purlis lets it go.
pub const TAKEN_SAID: &str = "You allowed this port, but purlis itself listens on it now, for \
                              another chat's proxy or tunnel: the service you allowed there was \
                              not running when purlis needed a port, and no chat reaches it while \
                              purlis holds it. Your service cannot start on it until purlis lets \
                              it go: close the chat whose proxy or tunnel holds it, or quit and \
                              reopen purlis; then start your service, then restart this chat.";

/// Whether purlis listens on `port` now, or may while a forge sign-in runs.
pub fn is_own(port: u16) -> bool {
    ports().is_own(port)
}

/// `ports`, less each that is purlis's own now, each once, in order: what a start compiles in.
/// Each one kept is marked as given in the same hold of the lock, so no listener of purlis's
/// takes it in between.
pub fn still_ports(ports_asked: &[u16]) -> Vec<u16> {
    ports().admit(ports_asked)
}

/// How many times [`bind_loopback`] asks again for a port that was given to a chat.
const BIND_TRIES: usize = 32;

/// **A new port on the loopback interface for purlis's own listener**, never one a chat's
/// sandbox was given ([`still_ports`]), marked as purlis's own until [`release`].
pub fn bind_loopback() -> std::io::Result<TcpListener> {
    let mut held = Vec::new();
    for _ in 0..BIND_TRIES {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        if ports().claim(port) {
            return Ok(listener);
        }
        // Kept bound until a fit is found, so the system hands out another.
        held.push(listener);
    }
    Err(std::io::Error::other(
        "purlis found no loopback port free of the services chats may reach",
    ))
}

/// **`held` are ports a listener of purlis's in another process holds** (#1999): the session
/// host's proxy for a chat. Marked purlis's own here too, so no chat here is allowed one,
/// unless a chat here was given one, which is refused: judged and marked in one hold of the
/// lock. [`release`] each when it is let go of.
pub fn hold_elsewhere(held: &[u16]) -> bool {
    let mut ports = ports();
    if held.iter().any(|port| ports.given.contains(port)) {
        return false;
    }
    ports.own.extend(held.iter().copied());
    true
}

/// `port` is no longer purlis's own: its listener closed.
pub fn release(port: u16) {
    ports().own.remove(&port);
}

/// **The entries a chat's proxy lists for `port`**: this machine by each name a client says it
/// by, each held to that one port, so the local-address check carries exactly that address and
/// port ([`super::reach::Reach::lists_exactly`]) and nothing else of this machine.
pub fn reach_entries(port: u16) -> [String; 3] {
    [
        format!("127.0.0.1:{port}"),
        format!("[::1]:{port}"),
        format!("localhost:{port}"),
    ]
}

/// The Seatbelt rule that lets a wrapped chat connect to `port` on the loopback interface
/// (`localhost` in a Seatbelt address is the loopback interface, IPv4 and IPv6).
pub fn seatbelt_rule(port: u16) -> String {
    format!("(allow network-outbound (remote ip \"localhost:{port}\"))")
}

/// **Whether `host` on `port` is that port of this machine's loopback**, by any name
/// [`loopback_port`] reads one by: what a service left out of a plan made elsewhere is asked
/// about and carried by (#2510).
pub fn names_loopback(host: &str, port: u16) -> bool {
    let target = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    loopback_port(&target) == Some(port)
}

/// **The port of a service on this Mac that `target` names**, where it names one on the
/// loopback interface as the profile and the proxy carry it: `127.0.0.1:50795`, `[::1]:50795`,
/// `localhost:50795`, and a name of this Mac such as `this-mac.internal:50795` (qw384b). None for
/// any other address, a name, or no port.
pub fn loopback_port(target: &str) -> Option<u16> {
    let target = target.trim().trim_end_matches(['.', ',', ';']);
    let (host, port) = if let Some(rest) = target.strip_prefix('[') {
        let (inside, after) = rest.split_once(']')?;
        (inside, after.strip_prefix(':')?)
    } else {
        target.rsplit_once(':')?
    };
    let port = port.parse::<u16>().ok().filter(|port| *port != 0)?;
    // Exactly the names the profile and the proxy carry ([`reach_entries`]): never another
    // address of the loopback block, which neither carries.
    let named = host.strip_suffix('.').unwrap_or(host);
    let loopback = named.eq_ignore_ascii_case("localhost")
        || is_a_name_of_this_mac(named)
        || host.parse::<IpAddr>().is_ok_and(|ip| {
            ip == IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
                || ip == IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
        });
    loopback.then_some(port)
}

/// **The port of this Mac a target the proxy refused as this machine names** (qw384b): what
/// [`loopback_port`] names, a name of this Mac ([`NAMES_OF_THIS_MAC`]) included. No name is
/// looked up.
pub fn named_port(target: &str) -> Option<u16> {
    loopback_port(target)
}

/// **The port a program's own refusal of a connection on this machine names** (qw384): Go's
/// `dial tcp 127.0.0.1:50795: connect: operation not permitted`, and Node's `connect EPERM
/// 127.0.0.1:50795`. None for any other line, and for an address that is not this machine's
/// loopback.
pub fn refused_in(line: &str) -> Option<u16> {
    let lower = line.to_ascii_lowercase();
    if let Some(at) = lower.find("dial tcp ")
        && lower.contains("connect: operation not permitted")
    {
        let rest = &line[at + "dial tcp ".len()..];
        // `dial tcp 127.0.0.1:50795: connect: …` and `dial tcp [::1]:50795: connect: …`.
        let address = rest.split(": ").next()?;
        return loopback_port(address);
    }
    if let Some(at) = lower.find("connect eperm ") {
        let rest = &line[at + "connect eperm ".len()..];
        return loopback_port(rest.split_whitespace().next()?);
    }
    None
}

/// **Where a chat's server on `port` listens** (#2019): the socket `<port>.sock` in `folder`,
/// the folder purlis made for that chat alone (`0700`), behind the door purlis listens on at
/// `port` ([`super::door`]).
pub fn serve_socket(folder: &std::path::Path, port: u16) -> std::path::PathBuf {
    folder.join(format!("{port}.sock"))
}

/// **The Seatbelt rules that let a wrapped chat serve on `socket`** (#2019): make that one
/// socket, bind it and accept on it, and take it away to start again. No TCP port: the door
/// listens on the chat's port, and the chat reaches its own server through the door.
pub fn serve_rules(socket: &std::path::Path) -> Result<[String; 3], &'static str> {
    let at = super::seatbelt::string(&socket.display().to_string())?;
    Ok([
        format!("(allow network-bind (local unix-socket (path-literal {at})))"),
        format!("(allow network-inbound (local unix-socket (path-literal {at})))"),
        format!("(allow file-write-create file-write-unlink (literal {at}))"),
    ])
}

/// **What allowing a chat to serve on `port` means**, as the Ask says it (#2019): who reaches
/// the server, what the door holds back, and what a page it serves can do in the person's own
/// browser, where it runs as a site of this Mac.
pub fn listen_risk(port: u16) -> String {
    format!(
        "lets this chat run a server that its browser, its commands and your own browser reach \
         at http://127.0.0.1:{port}, through purlis on this Mac. {LISTEN_RISK_REST}"
    )
}

/// [`listen_risk`] where the port is not known yet: the person types it.
pub fn listen_risk_unnamed() -> String {
    format!(
        "lets this chat run a server that its browser, its commands and your own browser reach \
         at http://127.0.0.1 on the port you type, through purlis on this Mac. {LISTEN_RISK_REST}"
    )
}

/// What [`listen_risk`] says after who reaches the server.
const LISTEN_RISK_REST: &str = "purlis checks each request's address, and keeps your browser's \
     cookies from the server. A page the server sends runs in your browser as a site of this \
     Mac: it can read your other local apps' cookies that are not HttpOnly, send requests to \
     those apps, and send what it reads to the internet. Open it in the chat's own browser \
     unless you trust it. Nothing on another machine connects to it";

/// What a chat is told to start a server on `port` with, once it may (#2019).
pub fn listen_said(port: u16) -> String {
    format!(
        "start it with `purlis listen {port} -- <command>`, or on the socket `PURLIS_SERVE_SOCKET` \
         names; its browser opens `http://127.0.0.1:{port}`"
    )
}

/// The lowest port a chat may serve on: a port below it is a system service's.
pub const FIRST_SERVED: u16 = 1024;

/// **`typed` as a port a chat may serve on** (#2019), or why not: a port as [`port`] takes
/// one, from [`FIRST_SERVED`], and none that anything on this Mac listens on now
/// ([`super::door::in_use`]): that is a service to connect to, asked about as one.
pub fn serve_port(typed: &str) -> Result<u16, Refused> {
    let number = port(typed)?;
    if number < FIRST_SERVED {
        return Err(Refused::Not(format!(
            "purlis lets a chat serve on a port from {FIRST_SERVED} to 65535: a port below \
             {FIRST_SERVED} is a system service's."
        )));
    }
    if super::door::in_use(number) {
        return Err(Refused::Not(in_use_said(number)));
    }
    Ok(number)
}

/// Why a chat is not let serve on `port`: something on this Mac listens there now.
pub fn in_use_said(port: u16) -> String {
    format!(
        "Something on this Mac already listens on port {port}, so this chat cannot run a server \
         there. Allow it as a service on this Mac to connect to, or serve on another port."
    )
}

/// **The port a server's own refusal to listen names** (#2019): Node's `listen EPERM:
/// operation not permitted 127.0.0.1:3000` and Go's `listen tcp 127.0.0.1:3000: bind: operation
/// not permitted`, on this Mac's loopback or on every interface (`0.0.0.0`, `::`, no address).
/// None for any other line, another machine's address, or a port of zero.
pub fn listen_refused_in(line: &str) -> Option<u16> {
    let lower = line.to_ascii_lowercase();
    let address = if let Some(at) = lower.find("listen eperm: operation not permitted ") {
        line[at + "listen eperm: operation not permitted ".len()..]
            .split_whitespace()
            .next()?
    } else if let Some(at) = lower.find("listen tcp")
        && lower.contains(": bind: operation not permitted")
    {
        // `listen tcp 127.0.0.1:3000: bind: …`, `listen tcp4 :3000: bind: …`.
        let rest = line[at + "listen tcp".len()..].trim_start_matches(['4', '6']);
        rest.strip_prefix(' ')?.split(": ").next()?
    } else {
        return None;
    };
    listened_port(address)
}

/// **The port of `address` where it is this Mac's loopback or every interface** (#2019):
/// `127.0.0.1:N`, `[::1]:N`, `::1:N`, `localhost:N`, and `0.0.0.0:N`, `:::N`, `[::]:N`, `:N` or
/// Seatbelt's `*:N`. None for another machine's address or a port of zero.
pub fn listened_port(address: &str) -> Option<u16> {
    let address = address.trim().trim_end_matches(['.', ',', ';']);
    let (host, port) = address.rsplit_once(':')?;
    let port = port.parse::<u16>().ok().filter(|port| *port != 0)?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let here = host.is_empty()
        || host == "*"
        || host == "::"
        || host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| {
            // Exactly the loopback addresses the profile carries, never another of 127/8.
            ip.is_unspecified()
                || ip == IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
                || ip == IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
        });
    here.then_some(port)
}

#[cfg(test)]
#[path = "localport/tests.rs"]
mod tests;
