// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The SSH route to an engine on another machine** (#6266, ADR 0123 §2, §3 and §8, D-6257-1).
//!
//! A pool of kind `remote-docker` names its engine in Docker's own spelling,
//! `ssh://[user@]host[:port]/<socket>` ([`Route`]), and is pinned to the host key the person
//! confirmed ([`HostKey`]) and the engine's id ([`Pin`]). purlis reaches the engine with
//! OpenSSH, never with Docker's own `ssh://` helper: one `ssh -L` per pool in use, held by a pipe from purlis
//! ([`Forward`]) forwards the remote socket to `<data>/engine-links/<pool>/docker.sock`, and each
//! run of `docker` for the pool is pointed there.
//!
//! - **The options purlis passes are fixed** ([`pinned_options`]): host key checking strict,
//!   against a `known_hosts` of purlis's own that holds the pinned key alone, under an alias of
//!   the pool's (`HostKeyAlias`), so neither the person's `known_hosts`, nor DNS, nor a
//!   `KnownHostsCommand` can make another key trusted; no master connection; no prompt; the
//!   forward must succeed or ssh exits; and a keep-alive, so a lost connection is noticed within
//!   a minute. Everything else is the person's own SSH settings (aliases, users, ports, jump
//!   hosts, identity files).
//! - **Every argument list is built here, as data** ([`forward_args`], [`contact_args`],
//!   [`probe_args`], [`sign_in_args`]), and tested as `Made::connect_args` is.
//! - **Nothing ssh prints decides anything**: its exit and the files it writes do. What it says
//!   on standard error is kept only to be shown.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// The folder under `<data>` the forwards' local sockets are made in.
pub const DIR: &str = "engine-links";

/// The local end of a pool's forward, in its folder.
pub const SOCKET: &str = "docker.sock";

/// The file the process holding a pool's forward keeps locked.
const HELD: &str = "held.lock";

/// The scheme of a route.
const SCHEME: &str = "ssh://";

/// The longest a unix socket's path may be on every system purlis runs on (macOS's
/// `sun_path` is 104 bytes, with its terminating zero).
pub const SOCKET_PATH_MOST: usize = 103;

/// How long ssh waits for the machine to answer before it gives up.
pub const CONNECT_WITHIN: Duration = Duration::from_secs(10);

/// How long a contact, a probe or a sign-in may take as a whole.
pub const RUN_WITHIN: Duration = Duration::from_secs(30);

/// How long a forward is waited for to make its socket.
pub const FORWARD_WITHIN: Duration = Duration::from_secs(30);

/// The keep-alive: one ask every this many seconds, and the connection given up after
/// [`ALIVE_COUNT`] unanswered, so a lost connection is noticed within a minute.
pub const ALIVE_EVERY_SECS: u32 = 15;
const ALIVE_COUNT: u32 = 3;

/// The most of what ssh says on standard error that is kept, to be shown.
const MOST_SAID: u64 = 16 * 1024;

/// Where `ssh` is looked for: the system's own, in folders only root writes.
pub const SSH_INSTALLED: &[&str] = &["/usr/bin/ssh", "/usr/local/bin/ssh", "/bin/ssh"];

/// **Where an engine on another machine is** (ADR 0123 §2): Docker's and Podman's
/// `ssh://[user@]host[:port][/socket]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    user: Option<String>,
    host: String,
    port: Option<u16>,
    socket: Option<String>,
    /// The key file ssh signs in with, where a Podman connection names one (#6269): kept beside
    /// the route, never in Docker's spelling, which has no place for it.
    identity: Option<PathBuf>,
}

impl Route {
    /// **`text` read as a route**, or why not, in a sentence. Refused: another scheme (plain
    /// `tcp://` with the reason, ADR 0123 §2), a host or user that could be taken for an
    /// option of ssh's or that holds what no name holds, a port that is no port, and a socket
    /// path that is not absolute or holds a `:`, which a forward's spelling can't carry.
    pub fn parse(text: &str) -> Result<Route, String> {
        let text = text.trim();
        let Some(rest) = text.strip_prefix(SCHEME) else {
            if text.starts_with("tcp://") {
                return Err(PLAIN_TCP.to_owned());
            }
            return Err(format!(
                "a route starts with {SCHEME}, as in ssh://you@build-box"
            ));
        };
        let (address, socket) = match rest.find('/') {
            Some(at) => (&rest[..at], Some(&rest[at..])),
            None => (rest, None),
        };
        let (user, hostport) = match address.rsplit_once('@') {
            Some((user, hostport)) => (Some(user), hostport),
            None => (None, address),
        };
        if let Some(user) = user
            && !a_user(user)
        {
            return Err(format!(
                "{} isn't a user name ssh can sign in as",
                crate::shown::short(user)
            ));
        }
        let (host, port) = split_port(hostport)?;
        if !a_host(host) {
            return Err(if host.is_empty() {
                "the route names no machine".to_owned()
            } else {
                format!("{} isn't the name of a machine", crate::shown::short(host))
            });
        }
        let socket = match socket {
            None | Some("/") => None,
            Some(path) if a_socket_path(path) => Some(path.to_owned()),
            Some(path) => {
                return Err(format!(
                    "{} isn't a socket's path on the other machine: it must be absolute, with no \
                     space or colon",
                    crate::shown::short(path)
                ));
            }
        };
        Ok(Route {
            user: user.map(str::to_owned),
            host: host.to_owned(),
            port,
            socket,
            identity: None,
        })
    }

    /// The route as Docker spells it, as it is kept.
    #[must_use]
    pub fn spelled(&self) -> String {
        let mut out = SCHEME.to_owned();
        if let Some(user) = &self.user {
            out.push_str(user);
            out.push('@');
        }
        if self.host.contains(':') {
            out.push_str(&format!("[{}]", self.host));
        } else {
            out.push_str(&self.host);
        }
        if let Some(port) = self.port {
            out.push_str(&format!(":{port}"));
        }
        if let Some(socket) = &self.socket {
            out.push_str(socket);
        }
        out
    }

    /// **This route with the socket purlis found at Connect** written in, so a later
    /// connection never finds a different engine by itself. Refused for a path no route holds.
    pub fn with_socket(self, socket: &str) -> Result<Route, String> {
        if !a_socket_path(socket) {
            return Err(format!(
                "{} isn't a socket's path on the other machine",
                crate::shown::short(socket)
            ));
        }
        Ok(Route {
            socket: Some(socket.to_owned()),
            ..self
        })
    }

    /// **This route, signing in with the key file `identity`** (#6269): what a Podman connection
    /// names. Refused for a path that is not absolute, or that holds what ssh would read as one of
    /// its own tokens (`%`), an environment variable (`${`, which OpenSSH 8.4 and later expand in
    /// a key file's path) or a line of its settings.
    pub fn with_identity(self, identity: &Path) -> Result<Route, String> {
        let text = identity.to_str().unwrap_or_default();
        if !identity.is_absolute()
            || text.is_empty()
            || text.len() > 1024
            || text.chars().any(|c| c.is_control() || c == '%')
            || text.contains("${")
        {
            return Err(format!(
                "{} isn't a key file's path ssh can sign in with",
                crate::shown::short(&identity.display().to_string())
            ));
        }
        Ok(Route {
            identity: Some(identity.to_path_buf()),
            ..self
        })
    }

    /// The key file ssh signs in with, where one is named.
    #[must_use]
    pub fn identity(&self) -> Option<&Path> {
        self.identity.as_deref()
    }

    /// The machine, as the person named it (a host alias of their SSH settings, a name or an
    /// address).
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The user named, where one was.
    #[must_use]
    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    /// The port named, where one was.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// The engine's socket on the other machine, where it is known.
    #[must_use]
    pub fn socket(&self) -> Option<&str> {
        self.socket.as_deref()
    }

    /// What ssh is handed to reach the machine: `[user@]host`, after the options.
    fn destination(&self) -> String {
        match &self.user {
            Some(user) => format!("{user}@{}", self.host),
            None => self.host.clone(),
        }
    }

    /// The options that say where and as whom: the port, where one was named, and the key file
    /// to sign in with, where one is (beside the agent's keys and the person's own settings).
    fn where_options(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(port) = self.port {
            out.extend(["-p".to_owned(), port.to_string()]);
        }
        if let Some(identity) = &self.identity {
            out.extend(["-i".to_owned(), identity.display().to_string()]);
        }
        out
    }
}

impl std::fmt::Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.spelled())
    }
}

/// **The refusal of plain `tcp://`** (ADR 0123 §2), in the guide's one sentence and the two
/// ways forward.
pub const PLAIN_TCP: &str = "purlis doesn't connect to Docker over plain tcp://: anyone who can \
                             reach that port can run anything on that machine. Use ssh:// \
                             instead, or turn on TLS for Docker there.";

fn split_port(hostport: &str) -> Result<(&str, Option<u16>), String> {
    // An IPv6 address is written in brackets, as ssh:// URLs write it.
    if let Some(rest) = hostport.strip_prefix('[') {
        let Some((inside, after)) = rest.split_once(']') else {
            return Err("an address in brackets has no closing bracket".to_owned());
        };
        let port = match after.strip_prefix(':') {
            Some(port) => Some(port_of(port)?),
            None if after.is_empty() => None,
            None => return Err("an address in brackets is followed by something else".to_owned()),
        };
        return Ok((inside, port));
    }
    match hostport.rsplit_once(':') {
        Some((host, port)) => Ok((host, Some(port_of(port)?))),
        None => Ok((hostport, None)),
    }
}

fn port_of(port: &str) -> Result<u16, String> {
    port.parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| format!("{} isn't a port", crate::shown::short(port)))
}

/// A user name ssh can sign in as, and nothing it could take for an option.
fn a_user(user: &str) -> bool {
    !user.is_empty()
        && user.len() <= 64
        && !user.starts_with('-')
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// A machine's name, an alias of the person's SSH settings, or an address: letters, digits,
/// dots, dashes and underscores, or an IPv6 address other than the unspecified `::`; never one
/// ssh could take for an option.
fn a_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('-')
        && !host.starts_with('.')
        && (host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            || host
                .parse::<std::net::Ipv6Addr>()
                .is_ok_and(|address| !address.is_unspecified()))
}

/// `path`, owned, where it is one [`a_socket_path`] lets through: a path of the other machine's
/// that a pin keeps.
fn plain_path(path: &str) -> Option<String> {
    a_socket_path(path).then(|| path.to_owned())
}

/// An absolute path of the other machine's, with nothing a forward's `local:remote` spelling or
/// a shell word would split.
fn a_socket_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() > 1
        && path.len() <= 255
        && !path.contains("/../")
        && !path.ends_with("/..")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'))
}

/// **A machine's SSH host key**, as a `known_hosts` line spells it: `<algorithm> <base64>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKey {
    algorithm: String,
    key: String,
}

impl HostKey {
    /// `text`, `<algorithm> <base64>` with anything after (a comment) left out, read as a key,
    /// or why not.
    pub fn parse(text: &str) -> Result<HostKey, String> {
        let mut words = text.split_whitespace();
        let (Some(algorithm), Some(key)) = (words.next(), words.next()) else {
            return Err("a host key is its algorithm and the key, as ssh prints it".to_owned());
        };
        let algorithm_ok = algorithm.len() <= 64
            && !algorithm.is_empty()
            && algorithm
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '@'));
        let blob = base64_decode(key);
        // The key's own blob names its algorithm first: a key whose blob says otherwise is no
        // key of that algorithm.
        let named_inside = blob
            .as_deref()
            .and_then(|blob| blob.get(..4).map(|len| (blob, len)))
            .and_then(|(blob, len)| {
                let len = u32::from_be_bytes(len.try_into().ok()?) as usize;
                blob.get(4..4 + len)
            });
        if !algorithm_ok || named_inside != Some(algorithm.as_bytes()) {
            return Err(format!(
                "{} isn't an SSH host key",
                crate::shown::short(&format!("{algorithm} {key}"))
            ));
        }
        Ok(HostKey {
            algorithm: algorithm.to_owned(),
            key: key.to_owned(),
        })
    }

    /// The key as it is kept: `<algorithm> <base64>`.
    #[must_use]
    pub fn spelled(&self) -> String {
        format!("{} {}", self.algorithm, self.key)
    }

    /// Its algorithm (`ssh-ed25519`, `ecdsa-sha2-nistp256`, `ssh-rsa`).
    #[must_use]
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    /// **Its fingerprint as `ssh-keygen -lf` prints it**: `SHA256:` and the base64 of the
    /// SHA-256 of the key, without padding.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        use base64::Engine as _;
        use sha2::Digest as _;
        let blob = base64_decode(&self.key).unwrap_or_default();
        let digest = sha2::Sha256::digest(&blob);
        format!(
            "SHA256:{}",
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest)
        )
    }
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(text).ok()
}

/// **What a pool on another machine is pinned to** (ADR 0123 §3): the host key the person
/// confirmed, and the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub host_key: HostKey,
    pub engine: EnginePin,
}

/// **The engine part of a pin** (ADR 0123 §3): Docker's id, or, for a Podman, which prints no
/// id that holds (#6269), the socket's path and the store's graph root as `podman info` says
/// them. A different graph root under the same host key is a different engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnginePin {
    /// `docker info`'s `.ID`.
    Docker(String),
    /// `podman info`'s `.Host.RemoteSocket.Path` and `.Store.GraphRoot`.
    Podman { socket: String, graph_root: String },
}

impl Pin {
    /// **The pin as `runners.json` and `pool-setup.json` keep it**: `{"hostKey", "engine"}` for
    /// a Docker, `{"hostKey", "podman": {"socket", "graphRoot"}}` for a Podman (#6269).
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        match &self.engine {
            EnginePin::Docker(id) => serde_json::json!({
                "hostKey": self.host_key.spelled(),
                "engine": id,
            }),
            EnginePin::Podman { socket, graph_root } => serde_json::json!({
                "hostKey": self.host_key.spelled(),
                "podman": {"socket": socket, "graphRoot": graph_root},
            }),
        }
    }

    /// **[`Pin::to_json`] read back**: exactly its two keys, each as purlis writes it, or `None`.
    #[must_use]
    pub fn from_json(pin: &serde_json::Map<String, serde_json::Value>) -> Option<Pin> {
        if pin.len() != 2 {
            return None;
        }
        let host_key = HostKey::parse(pin.get("hostKey")?.as_str()?).ok()?;
        let engine = match (pin.get("engine"), pin.get("podman")) {
            (Some(id), None) => {
                let id = id.as_str()?;
                crate::runners::usable_noted(id).then(|| EnginePin::Docker(id.to_owned()))?
            }
            (None, Some(podman)) => {
                let podman = podman.as_object()?;
                if podman.len() != 2 {
                    return None;
                }
                let path = |key: &str| plain_path(podman.get(key)?.as_str()?);
                EnginePin::Podman {
                    socket: path("socket")?,
                    graph_root: path("graphRoot")?,
                }
            }
            _ => return None,
        };
        Some(Pin { host_key, engine })
    }
}

impl EnginePin {
    /// **The pin in a sentence's words**: the id, or the Podman's store and socket.
    #[must_use]
    pub fn shown(&self) -> String {
        match self {
            EnginePin::Docker(id) => id.clone(),
            EnginePin::Podman { socket, graph_root } => {
                format!("Podman, its store at {graph_root}, at {socket}")
            }
        }
    }

    /// Whether the engine is a Podman, reached with Podman's own client.
    #[must_use]
    pub fn is_podman(&self) -> bool {
        matches!(self, EnginePin::Podman { .. })
    }

    /// **A Podman's pin, from what `podman info --format '{{json .}}'` printed** (`info`), or
    /// why not: both paths absolute and plain, as no other answer could be taken for them.
    /// `None` where `info` is not a Podman's (it names no store), as Docker's is not.
    #[must_use]
    pub fn podman_in(info: &serde_json::Value) -> Option<Result<EnginePin, String>> {
        let graph_root = info.pointer("/store/graphRoot")?;
        let plain = |value: Option<&serde_json::Value>, what: &str| {
            value
                .and_then(serde_json::Value::as_str)
                .map(|path| path.strip_prefix("unix://").unwrap_or(path))
                .and_then(plain_path)
                .ok_or_else(|| format!("Podman there didn't say its {what}"))
        };
        Some((|| {
            Ok(EnginePin::Podman {
                socket: plain(info.pointer("/host/remoteSocket/path"), "socket's path")?,
                graph_root: plain(Some(graph_root), "store's graph root")?,
            })
        })())
    }
}

/// **The name the forward calls the machine by, for its host key alone** (`HostKeyAlias`): the
/// pool's, so the key looked up is the one pinned for this pool, whatever the person's settings
/// call the machine or its port.
#[must_use]
pub fn alias(pool: &str) -> String {
    format!("purlis-pool-{pool}")
}

/// **What Connect calls the pool it reads a machine for**, before there is one: no pool's name
/// can start with a dot ([`crate::runners::usable_pool_name`]), so its forward's folder is no
/// pool's.
pub const CONNECTING: &str = ".connect";

/// **A name of [`CONNECTING`]'s for one Connect**: its own folder and forward, so two at once
/// (two windows, or a window and `purlis pool add --on`) never share one.
#[must_use]
pub fn connecting_name() -> String {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{CONNECTING}-{}-{n}", std::process::id())
}

/// The name a first contact calls the machine by, so what ssh writes is found by it alone.
const FIRST_CONTACT: &str = "purlis-first-contact";

/// **The `known_hosts` a forward is handed**: the pinned key alone, under the pool's alias.
#[must_use]
pub fn known_hosts(pool: &str, key: &HostKey) -> String {
    format!("{} {}\n", alias(pool), key.spelled())
}

/// **The options every ssh purlis runs for a pool is given, and that nothing overrides**
/// (ADR 0123 §2): its host key checked strictly against `known_hosts` alone, under `alias`; no
/// other place a key could come from; no master connection; no prompt; nothing forwarded to the
/// machine; and the keep-alive.
#[must_use]
pub fn pinned_options(known_hosts: &Path, alias: &str) -> Vec<String> {
    let mut out = common_options();
    for option in [
        "StrictHostKeyChecking=yes".to_owned(),
        format!("UserKnownHostsFile={}", known_hosts.display()),
        format!("HostKeyAlias={alias}"),
    ] {
        out.push("-o".to_owned());
        out.push(option);
    }
    out
}

/// What every ssh purlis runs is given, pinned or a first contact.
fn common_options() -> Vec<String> {
    let options = [
        // The keys of the person's own files, DNS and a command are never trusted for a pool.
        "GlobalKnownHostsFile=/dev/null".to_owned(),
        "KnownHostsCommand=none".to_owned(),
        "VerifyHostKeyDNS=no".to_owned(),
        "UpdateHostKeys=no".to_owned(),
        "CheckHostIP=no".to_owned(),
        // One connection of its own, sharing none.
        "ControlMaster=no".to_owned(),
        "ControlPath=none".to_owned(),
        // Never a prompt: signing is the agent's or a key the person's settings name.
        "BatchMode=yes".to_owned(),
        "PasswordAuthentication=no".to_owned(),
        "KbdInteractiveAuthentication=no".to_owned(),
        // Nothing of this machine's is handed to the other one.
        "ForwardAgent=no".to_owned(),
        "ForwardX11=no".to_owned(),
        "PermitLocalCommand=no".to_owned(),
        "RequestTTY=no".to_owned(),
        // Only the command purlis names runs there, and no tunnel opens.
        "RemoteCommand=none".to_owned(),
        "Tunnel=no".to_owned(),
        format!("ConnectTimeout={}", CONNECT_WITHIN.as_secs()),
        format!("ServerAliveInterval={ALIVE_EVERY_SECS}"),
        format!("ServerAliveCountMax={ALIVE_COUNT}"),
        "LogLevel=ERROR".to_owned(),
    ];
    options
        .into_iter()
        .flat_map(|option| ["-o".to_owned(), option])
        .collect()
}

/// **What the forward's session runs on the other machine**: it waits on a line from purlis, which
/// never sends one. When the process holding the forward goes, however it goes, its end of the
/// pipe closes and `read` returns. It then ends the session's own sshd process, its parent once
/// `exec` has replaced the login shell, so the whole connection ends with every channel on it
/// (a `docker exec` still open too, which would otherwise keep ssh here waiting), and ssh here
/// ends with it. No forward outlives what opened it (`-N` would: nothing but a kill ends it).
/// POSIX `sh`, `bash`, `zsh` and `fish` read it alike; ADR 0123's amendment of 2026-11-12.
pub const HELD_BY: &str = "exec sh -c 'read line; [ \"$PPID\" -gt 1 ] && kill -HUP \"$PPID\"'";

/// **The forward's arguments** (D-6257-1): `ssh -T -L <local>:<remote socket> -- <machine>
/// <`[`HELD_BY`]`>`, pinned, with the forward required to succeed, held by a pipe from purlis,
/// and nothing else forwarded. Refused for a route whose socket isn't known yet.
pub fn forward_args(
    route: &Route,
    pool: &str,
    known_hosts: &Path,
    local: &Path,
) -> Result<Vec<String>, String> {
    let Some(remote) = route.socket() else {
        return Err(
            "the pool's route names no socket on the other machine: connect it again".to_owned(),
        );
    };
    let mut args = vec!["-T".to_owned()];
    args.extend(pinned_options(known_hosts, &alias(pool)));
    for option in [
        "ExitOnForwardFailure=yes",
        "StreamLocalBindUnlink=yes",
        "StreamLocalBindMask=0177",
        // Not `yes`, which clears this `-L` too: a `ClearAllForwardings yes` of the person's
        // settings never takes it away. What those settings would add is refused before ssh runs
        // ([`connect::not_carried`]).
        "ClearAllForwardings=no",
    ] {
        args.push("-o".to_owned());
        args.push(option.to_owned());
    }
    args.extend(route.where_options());
    args.push("-L".to_owned());
    args.push(format!("{}:{remote}", local.display()));
    args.push("--".to_owned());
    args.push(route.destination());
    args.push(HELD_BY.to_owned());
    Ok(args)
}

/// **A first contact's arguments** (ADR 0123 §3): ssh meets the machine and writes the key it
/// presents into `scratch`, a `known_hosts` of the run's own, and signs in with nothing, so it
/// learns the key and is let in nowhere. Nothing is pinned by it.
#[must_use]
pub fn contact_args(route: &Route, scratch: &Path) -> Vec<String> {
    let mut args = common_options();
    for option in [
        "StrictHostKeyChecking=accept-new".to_owned(),
        format!("UserKnownHostsFile={}", scratch.display()),
        format!("HostKeyAlias={FIRST_CONTACT}"),
        // Written plainly whatever the system's ssh_config says, so it is found by its alias.
        "HashKnownHosts=no".to_owned(),
        "PubkeyAuthentication=no".to_owned(),
        "GSSAPIAuthentication=no".to_owned(),
        "HostbasedAuthentication=no".to_owned(),
        "NumberOfPasswordPrompts=0".to_owned(),
        // It forwards nothing, whatever the person's settings add.
        "ClearAllForwardings=yes".to_owned(),
    ] {
        args.push("-o".to_owned());
        args.push(option);
    }
    args.extend(route.where_options());
    args.push("--".to_owned());
    args.push(route.destination());
    args.push("true".to_owned());
    args
}

/// **A sign-in's arguments**: ssh, pinned, signs in and runs `true` there, so its exit says
/// whether the machine let the person in.
#[must_use]
pub fn sign_in_args(route: &Route, pool: &str, known_hosts: &Path) -> Vec<String> {
    let mut args = pinned_options(known_hosts, &alias(pool));
    // It forwards nothing, whatever the person's settings add.
    args.extend(["-o".to_owned(), "ClearAllForwardings=yes".to_owned()]);
    args.extend(route.where_options());
    args.push("--".to_owned());
    args.push(route.destination());
    args.push("true".to_owned());
    args
}

/// **What finds the engine's socket on the other machine** (ADR 0123 §2), run there by the
/// person's login shell, which must take POSIX `sh`: the system's Docker, rootless Docker's,
/// then Podman's rootful and rootless sockets. It prints the first that is there and this user
/// may use, and exits [`PROBE_NONE`] where none is there or [`PROBE_NOT_ALLOWED`] where one is
/// and this user may not use it, printing that one.
pub const PROBE: &str = "r=\"${XDG_RUNTIME_DIR:-/run/user/$(id -u)}\"; f=; \
for s in /var/run/docker.sock \"$r/docker.sock\" /run/podman/podman.sock \"$r/podman/podman.sock\"; do \
if [ -S \"$s\" ]; then if [ -r \"$s\" ] && [ -w \"$s\" ]; then echo \"$s\"; exit 0; fi; \
[ -z \"$f\" ] && f=\"$s\"; fi; done; \
if [ -n \"$f\" ]; then echo \"$f\"; exit 4; fi; exit 3";

/// The probe's exit where no engine's socket is there.
pub const PROBE_NONE: i32 = 3;

/// The probe's exit where one is there and this user may not use it.
pub const PROBE_NOT_ALLOWED: i32 = 4;

/// **The probe's arguments**: [`sign_in_args`]'s, running [`PROBE`] in place of `true`, and for
/// a route whose socket is named, the same checks of that socket alone.
#[must_use]
pub fn probe_args(route: &Route, pool: &str, known_hosts: &Path) -> Vec<String> {
    let mut args = sign_in_args(route, pool, known_hosts);
    args.pop();
    args.push(match route.socket() {
        // Checked only: the path is one [`a_socket_path`] lets through, so it is a plain word.
        Some(socket) => format!(
            "s={socket}; if [ -S \"$s\" ]; then if [ -r \"$s\" ] && [ -w \"$s\" ]; then echo \
             \"$s\"; exit 0; fi; echo \"$s\"; exit {PROBE_NOT_ALLOWED}; fi; exit {PROBE_NONE}"
        ),
        None => PROBE.to_owned(),
    });
    args
}

/// **A file read on the other machine** (#6269): [`sign_in_args`]'s, printing `path` there where
/// it is a plain file, and failing where it is not, so nothing waits on a FIFO in its place.
/// Refused for a path that isn't plain, which no Podman's own file would be.
pub fn read_args(
    route: &Route,
    pool: &str,
    known_hosts: &Path,
    path: &str,
) -> Result<Vec<String>, String> {
    if !a_socket_path(path) {
        return Err(format!(
            "{} isn't a file's path purlis reads on the other machine",
            crate::shown::short(path)
        ));
    }
    let mut args = sign_in_args(route, pool, known_hosts);
    args.pop();
    // Checked only: the path is one [`a_socket_path`] lets through, so it is a plain word.
    args.push(format!("test -f {path} && exec cat -- {path}"));
    Ok(args)
}

/// **Whether a file is there, on the other machine** (#6269): [`sign_in_args`]'s, succeeding
/// only where `path` is a regular file there. Refused for a path that isn't plain, as
/// [`read_args`] refuses it.
pub fn is_file_args(
    route: &Route,
    pool: &str,
    known_hosts: &Path,
    path: &str,
) -> Result<Vec<String>, String> {
    if !a_socket_path(path) {
        return Err(format!(
            "{} isn't a file's path purlis looks for on the other machine",
            crate::shown::short(path)
        ));
    }
    let mut args = sign_in_args(route, pool, known_hosts);
    args.pop();
    // Checked only: the path is one [`a_socket_path`] lets through, so it is a plain word.
    args.push(format!("test -f {path}"));
    Ok(args)
}

/// **The `ssh` this machine runs**: the system's own, in a folder only root writes, never one a
/// `PATH` names. Refused where there is none.
pub fn ssh_here() -> Result<PathBuf, String> {
    SSH_INSTALLED
        .iter()
        .map(PathBuf::from)
        .find(|at| {
            at.parent()
                .is_some_and(crate::dockerpool::settings::only_root_writes)
                && crate::secrets::run::which(&at.display().to_string(), None).is_some()
        })
        .ok_or_else(|| {
            "purlis found no ssh on this machine in a folder only root writes; install \
             OpenSSH's client"
                .to_owned()
        })
}

/// **Where pool `pool`'s forward has its socket**: `<engine links>/<pool>/docker.sock`. Refused
/// where the path would be longer than a socket's may be.
pub fn socket_of(engine_links: &Path, pool: &str) -> Result<PathBuf, String> {
    let socket = engine_links.join(pool).join(SOCKET);
    if socket.as_os_str().len() > SOCKET_PATH_MOST {
        return Err(format!(
            "the pool's name is too long for a socket in {}: choose a name of at most {} letters",
            engine_links.display(),
            SOCKET_PATH_MOST
                .saturating_sub(engine_links.join(SOCKET).as_os_str().len() + 2)
                .min(40)
        ));
    }
    Ok(socket)
}

/// What one ssh run ended with: its exit code (none where a signal ended it) and what it printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub code: Option<i32>,
    pub printed: String,
    /// What it said on standard error, kept to be shown and never read to decide anything.
    pub said: String,
}

/// **Runs `ssh` with `args`**, bounded in time and in what is read, and answers how it ended.
pub fn run(ssh: &Path, args: &[String]) -> Result<Ran, String> {
    run_reading(ssh, args, MOST_SAID)
}

/// [`run`], reading up to `most` bytes of what it prints: a file read there ([`read_args`]).
pub fn run_reading(ssh: &Path, args: &[String], most: u64) -> Result<Ran, String> {
    let mut command = std::process::Command::new(ssh);
    command.args(args);
    let ran = crate::boundedrun::run_within(&mut command, None, most, MOST_SAID, Some(RUN_WITHIN))
        .map_err(|why| match why {
            crate::boundedrun::Unanswered::NotStarted(why) => {
                format!("{} could not be run: {why}", ssh.display())
            }
            crate::boundedrun::Unanswered::PrintedTooMuch => {
                "ssh printed more than purlis reads of it".to_owned()
            }
            crate::boundedrun::Unanswered::NotFinished(_) => {
                format!("ssh did not finish within {} seconds", RUN_WITHIN.as_secs())
            }
        })?;
    Ok(Ran {
        code: ran.status.code(),
        printed: String::from_utf8_lossy(&ran.printed).into_owned(),
        said: crate::shown::short(String::from_utf8_lossy(&ran.said).trim()),
    })
}

/// **The key a first contact's `known_hosts` holds**: what ssh wrote as it met the machine,
/// under [`FIRST_CONTACT`], the first key of the line. `None` where it wrote none.
#[must_use]
pub fn key_met(scratch: &str) -> Option<HostKey> {
    scratch.lines().find_map(|line| {
        let rest = line.strip_prefix(FIRST_CONTACT)?.strip_prefix(' ')?;
        HostKey::parse(rest).ok()
    })
}

/// **The machines the person's SSH settings name** (`Host` lines of `~/.ssh/config`), each
/// once, in order: what Connect offers to pick from (#6265, choice 3). A pattern (`*`, `?`, `!`)
/// names no one machine and is left out, and so is a name no route could hold.
#[must_use]
pub fn hosts_in(config: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in config.lines() {
        let line = line.trim();
        let Some((key, rest)) = line.split_once(|c: char| c.is_whitespace() || c == '=') else {
            continue;
        };
        if !key.eq_ignore_ascii_case("host") {
            continue;
        }
        for name in rest.split_whitespace() {
            let name = name.trim_matches('"');
            if name.contains(['*', '?', '!'])
                || !a_host(name)
                || out.iter().any(|kept| kept == name)
            {
                continue;
            }
            out.push(name.to_owned());
        }
    }
    out
}

/// [`hosts_in`] the person's `~/.ssh/config`, read up to 256 KiB; none where there is none.
#[must_use]
pub fn hosts_here(home: Option<&Path>) -> Vec<String> {
    use std::io::Read as _;
    let Some(home) = home else {
        return Vec::new();
    };
    let Ok(file) = std::fs::File::open(home.join(".ssh/config")) else {
        return Vec::new();
    };
    let mut text = String::new();
    if file.take(256 * 1024).read_to_string(&mut text).is_err() {
        return Vec::new();
    }
    hosts_in(&text)
}

/// **How a file of the other machine's is read** (#6269): over ssh, pinned to the pool's key, as
/// a sign-in is ([`read_args`]). What a rootless Podman's check reads its runtime config with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reach {
    pub ssh: PathBuf,
    pub route: Route,
    pub pool: String,
    pub key: HostKey,
    /// `<data>/docker-settings`, where the pinned `known_hosts` is kept for the read.
    pub settings_in: PathBuf,
}

impl Reach {
    /// The pinned `known_hosts` for one of this reach's runs, in a run folder of its own.
    fn known_hosts(&self) -> Result<(crate::dockerpool::settings::RunFolder, PathBuf), String> {
        let folder =
            crate::dockerpool::settings::run_folder(&self.settings_in).map_err(|why| why.0)?;
        let known_hosts = folder
            .keep("known_hosts", known_hosts(&self.pool, &self.key).as_bytes())
            .map_err(|why| format!("purlis could not write the host key: {why}"))?;
        Ok((folder, known_hosts))
    }

    /// **Whether `path` is a regular file there**, or why that could not be asked; at most
    /// `most` bytes of what ssh says are read.
    pub fn is_file(&self, path: &Path, most: u64) -> Result<bool, String> {
        let (_folder, known_hosts) = self.known_hosts()?;
        let path = path.to_str().unwrap_or_default();
        let ran = run_reading(
            &self.ssh,
            &is_file_args(&self.route, &self.pool, &known_hosts, path)?,
            most,
        )?;
        match ran.code {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(if ran.said.is_empty() {
                format!("{} did not say", self.route.host())
            } else {
                ran.said
            }),
        }
    }

    /// **The file at `path` there**, up to `most` bytes, or why not.
    pub fn read(&self, path: &Path, most: u64) -> Result<String, String> {
        let (_folder, known_hosts) = self.known_hosts()?;
        let path = path.to_str().unwrap_or_default();
        let ran = run_reading(
            &self.ssh,
            &read_args(&self.route, &self.pool, &known_hosts, path)?,
            most,
        )?;
        if ran.code != Some(0) {
            return Err(if ran.said.is_empty() {
                format!("{} has no such file", self.route.host())
            } else {
                ran.said
            });
        }
        Ok(ran.printed)
    }
}

/// **A connection of Podman's own** (`podman system connection list`, #6269): its name and the
/// SSH route to that machine's Podman socket, with the key file it signs in with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodmanConnection {
    pub name: String,
    pub route: Route,
}

/// **The connections `podman system connection list --format json` printed**, each read once
/// into its route (ADR 0123 §2): a connection to a Podman machine of this machine's own
/// (`IsMachine`) is this machine, and one that is no `ssh://` route (a `unix://` socket here),
/// or whose key file isn't a path ssh can take, is left out. In the order Podman lists them.
#[must_use]
pub fn podman_connections_in(printed: &str) -> Vec<PodmanConnection> {
    let Ok(serde_json::Value::Array(listed)) = serde_json::from_str(printed.trim()) else {
        return Vec::new();
    };
    let field = |one: &serde_json::Value, key: &str| {
        one.get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let mut out: Vec<PodmanConnection> = Vec::new();
    for one in &listed {
        if one.get("IsMachine").and_then(serde_json::Value::as_bool) == Some(true) {
            continue;
        }
        let (Some(name), Some(uri)) = (field(one, "Name"), field(one, "URI")) else {
            continue;
        };
        if name.len() > 128
            || name.chars().any(char::is_control)
            || out.iter().any(|kept| kept.name == name)
        {
            continue;
        }
        let Ok(route) = Route::parse(&uri) else {
            continue;
        };
        let route = match field(one, "Identity") {
            Some(identity) => match route.with_identity(Path::new(&identity)) {
                Ok(route) => route,
                Err(_) => continue,
            },
            None => route,
        };
        out.push(PodmanConnection { name, route });
    }
    out
}

/// **Where this machine's `podman` is looked for**: the places Podman's installers put it
/// (Podman's own for macOS, Homebrew's, `/usr/local`'s and the system's), as `docker` is looked
/// for in Docker's ([`crate::dockerpool::Docker::on_this_machine`]), never a `PATH`'s.
pub const PODMAN_INSTALLED: &[&str] = &[
    "/opt/podman/bin/podman",
    "/opt/homebrew/bin/podman",
    "/usr/local/bin/podman",
    "/usr/bin/podman",
];

/// **This machine's `podman`**: [`PODMAN_INSTALLED`]'s first that is there.
#[must_use]
pub fn podman_here() -> Option<PathBuf> {
    PODMAN_INSTALLED
        .iter()
        .map(PathBuf::from)
        .find(|at| crate::secrets::run::which(&at.display().to_string(), None).is_some())
}

/// **The person's Podman connections** ([`podman_connections_in`]), as this machine's `podman`
/// lists them: none where there is no `podman`, or it lists none.
#[must_use]
pub fn podman_connections_here() -> Vec<PodmanConnection> {
    let Some(podman) = podman_here() else {
        return Vec::new();
    };
    let mut command = std::process::Command::new(podman);
    command.args(["system", "connection", "list", "--format", "json"]);
    crate::boundedrun::run_within(
        &mut command,
        None,
        MOST_SAID * 16,
        MOST_SAID,
        Some(RUN_WITHIN),
    )
    .ok()
    .filter(|ran| ran.status.success())
    .map(|ran| podman_connections_in(&String::from_utf8_lossy(&ran.printed)))
    .unwrap_or_default()
}

pub mod connect;
pub mod forward;
pub mod pool;

pub use forward::{Forward, Held};

#[cfg(test)]
mod tests;
