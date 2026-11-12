// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Connect's checks of an SSH route** (ADR 0123 §3, §6 and §7), in the order a person meets
//! them: the name resolves, the machine answers, its key (met, and confirmed or pinned), the
//! sign-in, an engine at the socket this user may use, and the engine's id.
//!
//! Each check is read from what can be told without reading ssh's words: the person's own SSH
//! settings as `ssh -G` prints them, the system's resolver, a TCP connection, the key ssh wrote
//! as it met the machine, and ssh's and the probe's exits ([`super::PROBE`]). Each failure is a
//! [`Finding`], whose words are Pool setup's ([`crate::poolsetup`]).

use std::net::ToSocketAddrs as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{HostKey, Pin, Route};

/// **What a check of the route found wrong** (ADR 0123 §7, its SSH rows), each with what the
/// sentence names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// The name doesn't resolve.
    NoSuchName { host: String },
    /// Nothing answers on the port.
    NoAnswer { host: String, port: u16 },
    /// ssh met no key (it couldn't be run, or the machine said nothing ssh could read): what it
    /// said, to be shown.
    NotMet { host: String, said: String },
    /// The machine presents a key other than the one pinned or confirmed.
    KeyChanged {
        host: String,
        pinned: HostKey,
        now: HostKey,
    },
    /// The machine didn't let the person in.
    SignInRefused { host: String },
    /// No engine's socket is there.
    NoEngine { host: String, user: String },
    /// One is, and this user may not use it.
    NotAllowed {
        host: String,
        user: String,
        socket: String,
    },
    /// The engine at the socket didn't answer through the forward: what was said.
    EngineSilent { host: String, said: String },
    /// The engine answers with another id than the pinned one.
    EngineChanged {
        host: String,
        pinned: String,
        now: String,
    },
    /// Something else went wrong on this machine (no ssh, a folder that couldn't be made).
    Here(String),
}

/// **What ssh makes of the route, from the person's own settings** (`ssh -G`): the machine's
/// name or address it connects to, its port and user, whether a jump host or a command reaches
/// it, and the `known_hosts` files it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub hostname: String,
    pub port: u16,
    pub user: String,
    /// A `ProxyJump` or `ProxyCommand` reaches it, so this machine resolves and connects to
    /// nothing of it itself.
    pub proxied: bool,
    pub known_hosts: Vec<String>,
}

/// **`ssh -G`'s answer read** (`<key> <value>` lines, as ssh prints its settings). `None` where it
/// names no host.
#[must_use]
pub fn resolved_from(printed: &str) -> Option<Resolved> {
    let mut hostname = None;
    let mut port = 22;
    let mut user = String::new();
    let mut proxied = false;
    let mut known_hosts = Vec::new();
    for line in printed.lines() {
        let Some((key, value)) = line.split_once(' ') else {
            continue;
        };
        match key {
            "hostname" => hostname = Some(value.to_owned()),
            "port" => port = value.parse().unwrap_or(22),
            "user" => value.clone_into(&mut user),
            "proxyjump" | "proxycommand" if value != "none" => proxied = true,
            "userknownhostsfile" => {
                known_hosts.extend(value.split_whitespace().map(str::to_owned));
            }
            _ => {}
        }
    }
    Some(Resolved {
        hostname: hostname?,
        port,
        user,
        proxied,
        known_hosts,
    })
}

/// The arguments of `ssh -G` for `route`.
#[must_use]
pub fn settings_args(route: &Route) -> Vec<String> {
    let mut args = vec!["-G".to_owned()];
    args.extend(route.where_options());
    args.push("--".to_owned());
    args.push(route.destination());
    args
}

/// **What the person's own SSH settings say of the route** (`ssh -G`).
pub fn resolve(ssh: &Path, route: &Route) -> Result<Resolved, Finding> {
    let ran = super::run(ssh, &settings_args(route)).map_err(Finding::Here)?;
    if ran.code != Some(0) {
        return Err(Finding::Here(format!(
            "your SSH settings couldn't be read for {}: {}",
            route.host(),
            ran.said
        )));
    }
    resolved_from(&ran.printed).ok_or_else(|| {
        Finding::Here(format!(
            "your SSH settings name no machine for {}",
            route.host()
        ))
    })
}

/// How long the machine is given to answer on its port.
const ANSWER_WITHIN: Duration = Duration::from_secs(5);

/// **Whether the name resolves and the machine answers on its port**, where this machine reaches
/// it itself (no jump host or command).
pub fn reachable(route: &Route, resolved: &Resolved) -> Result<(), Finding> {
    if resolved.proxied {
        return Ok(());
    }
    let addresses: Vec<_> = (resolved.hostname.as_str(), resolved.port)
        .to_socket_addrs()
        .map_err(|_| Finding::NoSuchName {
            host: route.host().to_owned(),
        })?
        .collect();
    if addresses.is_empty() {
        return Err(Finding::NoSuchName {
            host: route.host().to_owned(),
        });
    }
    if addresses
        .iter()
        .any(|at| std::net::TcpStream::connect_timeout(at, ANSWER_WITHIN).is_ok())
    {
        return Ok(());
    }
    Err(Finding::NoAnswer {
        host: route.host().to_owned(),
        port: resolved.port,
    })
}

/// **The key the machine presents** (ADR 0123 §3), met by a first contact that signs in with
/// nothing ([`super::contact_args`]) in a run folder of its own under `settings_in`.
pub fn meet(ssh: &Path, route: &Route, settings_in: &Path) -> Result<HostKey, Finding> {
    let folder =
        crate::dockerpool::settings::run_folder(settings_in).map_err(|why| Finding::Here(why.0))?;
    let scratch = folder
        .keep("known_hosts", b"")
        .map_err(|why| Finding::Here(format!("purlis could not meet the machine: {why}")))?;
    let ran = super::run(ssh, &super::contact_args(route, &scratch)).map_err(Finding::Here)?;
    let written = std::fs::read_to_string(&scratch).unwrap_or_default();
    super::key_met(&written).ok_or_else(|| Finding::NotMet {
        host: route.host().to_owned(),
        said: ran.said,
    })
}

/// **The key the person's own `known_hosts` holds for the machine**, where it holds one: what
/// Connect shows as the one to confirm (ADR 0123 §3). Read by `ssh-keygen -F`, which reads
/// hashed names too.
#[must_use]
pub fn known_to_the_person(resolved: &Resolved, home: Option<&Path>) -> Option<HostKey> {
    let keygen = Path::new("/usr/bin/ssh-keygen");
    let name = if resolved.port == 22 {
        resolved.hostname.clone()
    } else {
        format!("[{}]:{}", resolved.hostname, resolved.port)
    };
    resolved.known_hosts.iter().find_map(|file| {
        let file = match (file.strip_prefix("~/"), home) {
            (Some(rest), Some(home)) => home.join(rest),
            (Some(_), None) => return None,
            (None, _) => PathBuf::from(file),
        };
        let ran = super::run(
            keygen,
            &[
                "-F".to_owned(),
                name.clone(),
                "-f".to_owned(),
                file.display().to_string(),
            ],
        )
        .ok()?;
        ran.printed.lines().find_map(|line| {
            if line.starts_with('#') {
                return None;
            }
            let (_, key) = line.split_once(' ')?;
            HostKey::parse(key).ok()
        })
    })
}

/// **The sign-in and the engine's socket**, against `key`: the route with the socket found
/// written in. A machine presenting another key is met again to say which ([`Finding::KeyChanged`]).
pub fn probe(
    ssh: &Path,
    route: &Route,
    pool: &str,
    key: &HostKey,
    settings_in: &Path,
    user: &str,
) -> Result<Route, Finding> {
    let host = || route.host().to_owned();
    let folder =
        crate::dockerpool::settings::run_folder(settings_in).map_err(|why| Finding::Here(why.0))?;
    let known_hosts = folder
        .keep("known_hosts", super::known_hosts(pool, key).as_bytes())
        .map_err(|why| Finding::Here(format!("purlis could not write the host key: {why}")))?;
    let signed_in =
        super::run(ssh, &super::sign_in_args(route, pool, &known_hosts)).map_err(Finding::Here)?;
    if signed_in.code != Some(0) {
        // Met again with nothing pinned: a key other than this one is said as such.
        return Err(match meet(ssh, route, settings_in) {
            Ok(now) if now != *key => Finding::KeyChanged {
                host: host(),
                pinned: key.clone(),
                now,
            },
            _ => Finding::SignInRefused { host: host() },
        });
    }
    let probed =
        super::run(ssh, &super::probe_args(route, pool, &known_hosts)).map_err(Finding::Here)?;
    let found = probed.printed.lines().next().unwrap_or_default().trim();
    match probed.code {
        Some(0) => route.clone().with_socket(found).map_err(Finding::Here),
        Some(super::PROBE_NOT_ALLOWED) => Err(Finding::NotAllowed {
            host: host(),
            user: user.to_owned(),
            socket: crate::shown::short(found),
        }),
        Some(super::PROBE_NONE) => Err(Finding::NoEngine {
            host: host(),
            user: user.to_owned(),
        }),
        _ => Err(Finding::Here(format!(
            "purlis couldn't look for Docker on {}: {}",
            route.host(),
            probed.said
        ))),
    }
}

/// **The engine's id, read through a forward opened against `key`** (`docker info`'s `.ID`): what
/// the pin keeps. The forward is ended once read; the pool holds its own once it is added.
pub fn engine_id(
    docker: &crate::dockerpool::Docker,
    opening: &super::forward::Opening<'_>,
) -> Result<String, Finding> {
    let host = || opening.route.host().to_owned();
    let held = super::forward::open(opening)
        .map_err(|said| Finding::EngineSilent { host: host(), said })?;
    let read = docker.clone().through(held.socket()).engine_id_said();
    drop(held);
    read.map_err(|why| Finding::EngineSilent {
        host: host(),
        said: why.0,
    })
}

/// **Whether `pin` still holds for the engine `docker` reaches through the pool's forward**: the
/// check made before every runner is made, checked or linked (ADR 0123 §3). The host key is
/// ssh's to refuse as the forward opens.
pub fn engine_holds(
    docker: &crate::dockerpool::Docker,
    route: &Route,
    pin: &Pin,
) -> Result<(), Finding> {
    let now = docker
        .engine_id_said()
        .map_err(|why| Finding::EngineSilent {
            host: route.host().to_owned(),
            said: why.0,
        })?;
    if now == pin.engine {
        return Ok(());
    }
    Err(Finding::EngineChanged {
        host: route.host().to_owned(),
        pinned: pin.engine.clone(),
        now,
    })
}
