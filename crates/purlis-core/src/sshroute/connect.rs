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

use super::{EnginePin, HostKey, Pin, Route};

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
    /// The machine's SSH server signs the person in and refuses to forward the engine's socket
    /// (#6349): `AllowTcpForwarding no`, `AllowStreamLocalForwarding no`, `DisableForwarding`,
    /// or a key's `no-port-forwarding`. Found by [`forwarding_refused`], never from what ssh said.
    ForwardingRefused { host: String },
    /// The engine answers with another id than the pinned one, or, for a Podman, another socket
    /// or graph root (#6269).
    EngineChanged {
        host: String,
        pinned: String,
        now: String,
        podman: bool,
    },
    /// The engine there is a Podman, and this machine has no `podman` to reach it with (#6269).
    NeedsPodman { host: String },
    /// **Over Docker's TLS API** (#6267, ADR 0123 §7): a file the pool names can't be used.
    TlsFile {
        host: String,
        file: String,
        why: String,
    },
    /// The CA file no longer holds the certificate the pool is pinned to: both fingerprints, as
    /// a person compares them.
    CaChanged {
        host: String,
        pinned: String,
        now: String,
        file: String,
    },
    /// The server's certificate isn't signed by the CA named.
    NotSignedByCa { host: String, ca: String },
    /// The server's certificate has expired.
    ServerExpired { host: String },
    /// The server's certificate is for other names than the one connected to: those names.
    WrongName { host: String, name: String },
    /// The server refused the client certificate.
    ClientRefused { host: String, cert: String },
    /// The server says the client certificate has expired.
    ClientExpired { host: String, cert: String },
    /// **Doctor's checks of the engine** (#6272, ADR 0123 §7): it doesn't give containers its
    /// default seccomp profile, as the local kind's daemon check holds it to.
    Unconfined { host: String, podman: bool },
    /// Its platform (`docker info`'s OS and architecture) has no runner image: what it is, and
    /// what the image is built for.
    Architecture {
        host: String,
        platform: String,
        built_for: Vec<String>,
    },
    /// Docker's disk there is too full to make a runner on (#6205): what is free, where `df`
    /// said.
    DiskFull { host: String, free: Option<u64> },
    /// It couldn't download the runner image from its registry.
    ImageUnreachable { host: String, registry: String },
    /// **The Docker context the pool's route was read from now names another route** (#6268).
    ContextMoved { name: String },
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

/// **The settings of the person's that would ride on purlis's forward**, from what `ssh -G`
/// printed, each named once as ssh_config spells it.
#[must_use]
pub fn not_carried(printed: &str) -> Vec<&'static str> {
    let mut named = Vec::new();
    for (key, setting, harmless) in [
        ("localforward", "LocalForward", None),
        ("remoteforward", "RemoteForward", None),
        ("dynamicforward", "DynamicForward", None),
        ("tunnel", "Tunnel", Some("false")),
        ("sessiontype", "SessionType", Some("default")),
        ("stdinnull", "StdinNull", Some("no")),
        (
            "forkafterauthentication",
            "ForkAfterAuthentication",
            Some("no"),
        ),
    ] {
        let set = printed.lines().any(|line| {
            let (at, value) = line.split_once(' ').unwrap_or((line, ""));
            at == key && Some(value.trim()) != harmless
        });
        if set {
            named.push(setting);
        }
    }
    named
}

/// **Why a route whose SSH settings add what [`not_carried`] names opens no forward**, in Pool
/// setup's words.
#[must_use]
pub fn carried_refused(host: &str, settings: &[&str]) -> String {
    format!(
        "Your SSH settings for {host} add {}, which purlis won't carry on its connection to \
         Docker there. Give the machine a Host entry without {}.",
        settings.join(", "),
        if settings.len() == 1 { "it" } else { "them" },
    )
}

/// **The `ssh-keygen` on `path`** (the person's `PATH`, where `None`).
#[must_use]
pub fn keygen_on(path: Option<&str>) -> Option<PathBuf> {
    let from_env = std::env::var("PATH").ok();
    crate::secrets::run::which("ssh-keygen", path.or(from_env.as_deref()))
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

/// **What the person's own SSH settings say of the route** (`ssh -G`). Refused where they add
/// what purlis won't carry ([`not_carried`]).
pub fn resolve(ssh: &Path, route: &Route) -> Result<Resolved, Finding> {
    let ran = super::run(ssh, &settings_args(route)).map_err(Finding::Here)?;
    if ran.code != Some(0) {
        return Err(Finding::Here(format!(
            "your SSH settings couldn't be read for {}: {}",
            route.host(),
            ran.said
        )));
    }
    // Refused before anything meets the machine: nothing of these rides on purlis's forward.
    let carried = not_carried(&ran.printed);
    if !carried.is_empty() {
        return Err(Finding::Here(carried_refused(route.host(), &carried)));
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
    // Only read: the person's own known hosts, by their own ssh-keygen.
    let keygen = keygen_on(None)?;
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
            &keygen,
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

/// **The engine's pin, read through a forward opened against `key`** (ADR 0123 §3): Docker's id,
/// or a Podman's socket and graph root, read with this machine's `podman` where Docker's client
/// finds a Podman there (#6269). The forward is ended once read; the pool holds its own once it
/// is added.
pub fn engine_pin(
    docker: &crate::dockerpool::Docker,
    opening: &super::forward::Opening<'_>,
) -> Result<EnginePin, Finding> {
    let host = || opening.route.host().to_owned();
    let held = super::forward::open(opening)
        .map_err(|said| Finding::EngineSilent { host: host(), said })?;
    let through = docker.clone().through(held.socket());
    let read = match through.engine_pin_said() {
        Ok(crate::dockerpool::EngineSaid::PodmanOverDocker) => match super::podman_here() {
            Some(podman) => through.as_program(podman).engine_pin_said(),
            None => return Err(Finding::NeedsPodman { host: host() }),
        },
        read => read,
    };
    drop(held);
    match read {
        Ok(crate::dockerpool::EngineSaid::Pin(pin)) => Ok(pin),
        // Podman's own client answers as a Podman, never as Docker's over it.
        Ok(crate::dockerpool::EngineSaid::PodmanOverDocker) => {
            Err(Finding::NeedsPodman { host: host() })
        }
        Err(why) => Err(silent_or_refused(opening, why.0)),
    }
}

/// **Why an engine didn't answer through a forward that opened** (#6349): the machine's SSH
/// server refusing to forward its socket, where [`forwarding_refused`] finds it does, else the
/// engine itself, with what was said.
pub fn silent_or_refused(opening: &super::forward::Opening<'_>, said: String) -> Finding {
    let host = opening.route.host().to_owned();
    if forwarding_refused(opening) {
        Finding::ForwardingRefused { host }
    } else {
        Finding::EngineSilent { host, said }
    }
}

/// **Whether the machine's SSH server refuses to forward the engine's socket** (#6349), by a check
/// of its own. A local forward opens on this machine alone, so a server that refuses it is found
/// only by asking it for a channel: ssh asks for one to the socket ([`super::forward_check_args`])
/// and ends with 255 where it isn't opened. ssh ends with 255 too for a sign-in refused, a lost
/// connection, and a channel the server opened to a socket that took no connection, so a 255 is
/// taken as a refused forward only where the probe, run right after, signs in and finds the
/// socket there and usable ([`super::probe_args`]). What ssh said is never read.
///
/// What it can't tell apart: a socket left behind by an engine that ended without removing it,
/// on a machine that forwards. That is said as a refused forward, and Check again asks again.
#[must_use]
pub fn forwarding_refused(opening: &super::forward::Opening<'_>) -> bool {
    let (route, pool) = (opening.route, opening.pool);
    let Ok(folder) = crate::dockerpool::settings::run_folder(opening.settings_in) else {
        return false;
    };
    let Ok(known_hosts) = folder.keep(
        "known_hosts",
        super::known_hosts(pool, opening.key).as_bytes(),
    ) else {
        return false;
    };
    let Ok(args) = super::forward_check_args(route, pool, &known_hosts) else {
        return false;
    };
    if !matches!(
        super::run(opening.ssh, &args),
        Ok(super::Ran {
            code: Some(255),
            ..
        })
    ) {
        return false;
    }
    super::run(opening.ssh, &super::probe_args(route, pool, &known_hosts))
        .is_ok_and(|probed| probed.code == Some(0))
}

/// **Whether `pin` still holds for the engine `docker` reaches through the pool's forward**: the
/// check made before every runner is made, checked or linked (ADR 0123 §3). The host key is
/// ssh's to refuse as the forward opens. A Podman's is its socket and graph root: another graph
/// root under the same key is another engine (#6269).
pub fn engine_holds(
    docker: &crate::dockerpool::Docker,
    route: &Route,
    pin: &Pin,
) -> Result<(), Finding> {
    let silent = |why: crate::dockerpool::Refused| Finding::EngineSilent {
        host: route.host().to_owned(),
        said: why.0,
    };
    let now = match &pin.engine {
        EnginePin::Docker(_) => EnginePin::Docker(docker.engine_id_said().map_err(silent)?),
        EnginePin::Podman { .. } => match docker.engine_pin_said().map_err(silent)? {
            crate::dockerpool::EngineSaid::Pin(now) => now,
            crate::dockerpool::EngineSaid::PodmanOverDocker => {
                return Err(Finding::NeedsPodman {
                    host: route.host().to_owned(),
                });
            }
        },
    };
    if now == pin.engine {
        return Ok(());
    }
    Err(Finding::EngineChanged {
        host: route.host().to_owned(),
        pinned: pin.engine.shown(),
        now: now.shown(),
        podman: pin.engine.is_podman(),
    })
}
