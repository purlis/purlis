// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The SSH route to an engine on another machine** (#6266, ADR 0123 §2, §3 and §8, D-6257-1).
//!
//! A pool of kind `remote-docker` names its engine in Docker's own spelling,
//! `ssh://[user@]host[:port]/<socket>` ([`Route`]), and is pinned to the host key the person
//! confirmed ([`HostKey`]) and the engine's id ([`Pin`]). purlis reaches the engine with
//! OpenSSH, never with Docker's own `ssh://` helper: one `ssh -N -L` per pool in use
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

    /// The options that say where: the port, where one was named.
    fn where_options(&self) -> Vec<String> {
        self.port
            .map(|port| vec!["-p".to_owned(), port.to_string()])
            .unwrap_or_default()
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
/// dots, dashes and underscores, or an IPv6 address's hex digits and colons; never one ssh could
/// take for an option.
fn a_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('-')
        && !host.starts_with('.')
        && (host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            || (host.contains(':') && host.chars().all(|c| c.is_ascii_hexdigit() || c == ':')))
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
/// confirmed, and the id `docker info` printed for the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub host_key: HostKey,
    pub engine: String,
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

/// **The forward's arguments** (D-6257-1): `ssh -N -L <local>:<remote socket>`, pinned, and
/// with the forward required to succeed. Refused for a route whose socket isn't known yet.
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
    let mut args = vec!["-N".to_owned()];
    args.extend(pinned_options(known_hosts, &alias(pool)));
    for option in [
        "ExitOnForwardFailure=yes",
        "StreamLocalBindUnlink=yes",
        "StreamLocalBindMask=0177",
    ] {
        args.push("-o".to_owned());
        args.push(option.to_owned());
    }
    args.extend(route.where_options());
    args.push("-L".to_owned());
    args.push(format!("{}:{remote}", local.display()));
    args.push("--".to_owned());
    args.push(route.destination());
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
        "PubkeyAuthentication=no".to_owned(),
        "GSSAPIAuthentication=no".to_owned(),
        "HostbasedAuthentication=no".to_owned(),
        "NumberOfPasswordPrompts=0".to_owned(),
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
    let mut command = std::process::Command::new(ssh);
    command.args(args);
    let ran =
        crate::boundedrun::run_within(&mut command, None, MOST_SAID, MOST_SAID, Some(RUN_WITHIN))
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

pub mod connect;
pub mod forward;
pub mod pool;

pub use forward::{Forward, Held};

#[cfg(test)]
mod tests;
