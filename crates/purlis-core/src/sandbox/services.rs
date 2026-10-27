// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A local socket or a system service, as a grant names it** (D-sock-1, amending ruling
//! "purlis grants no local socket", ADR 0067 §2; ADR 0093).
//!
//! A sandboxed chat reaches no local socket but the app's own hook socket, and no system service
//! by name but the few every program needs. A block on one no longer dead-ends: its Notice
//! offers Allow for this chat and Always allow (this project on this machine), as a host's and a
//! folder's do, and Keep blocked. Nothing is granted by default.
//!
//! - **A socket** ([`socket`]) is named by the path the kernel names it by, links resolved, so a
//!   link a chat puts in its own folder later widens nothing. It must be a socket now, and
//!   never one under a path the sandbox denies by class: purlis's own state, a vault.
//! - **A service** ([`service`]) is a macOS `mach-lookup` name, reverse-DNS, whole: never a
//!   prefix. The keychain's services are never granted (ADR 0067 §5 class 1).
//!
//! Either is this machine's alone: kept for one chat in memory, or in this machine's local file
//! (`super::local`), **never in the committed one**, where a repository could grant itself a
//! socket (D-sock-2).
//!
//! **What allowing one means is said, honestly, per known kind** ([`risk`]): a container
//! engine's socket is about the same as no sandbox, an ssh agent lends its keys, a database's
//! socket logs in as the person. Anything else is said to be a service purlis can't tell.

use std::path::{Path, PathBuf};

use super::Denial;
use super::grant::{Refused, What};

/// The certificate check's service (#1337): what Go programs on macOS ask to verify a host's
/// certificate. Claude Code grants it as `enableWeakerNetworkIsolation`, a wrapped chat's profile
/// as [`super::seatbelt::TRUST`]. **Never a grant** since the operator's ruling R1 of
/// 2026-10-13: every sandboxed chat has it unless the project's `certificate-checks` is `false`
/// or policy locks it ([`super::CertificateChecks`]), so it is neither declared nor allowed by
/// name.
pub const TRUSTD: &str = "com.apple.trustd.agent";

/// What allowing a container engine's socket means.
pub const RISK_CONTAINERS: &str = "lets this chat control containers, which can reach your \
                                   whole machine: about the same as no sandbox";
/// What allowing an ssh agent's socket means.
pub const RISK_SSH_AGENT: &str = "lets this chat use your SSH keys";
/// What allowing a database's socket means.
pub const RISK_DATABASE: &str = "lets this chat use that database as you";
/// What allowing the certificate check's service means.
pub const RISK_TRUSTD: &str = "lets Go and similar tools check certificates; that service can \
                               make its own network requests";
/// What allowing anything else means.
pub const RISK_UNKNOWN: &str = "a service on this machine; purlis can't tell what it does";

/// The kind of local thing a grant names, as far as purlis can tell from its name alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Known {
    Containers,
    SshAgent,
    Database,
    CertificateCheck,
    Unknown,
}

impl Known {
    /// What allowing it means, in one honest line.
    pub fn risk(self) -> &'static str {
        match self {
            Self::Containers => RISK_CONTAINERS,
            Self::SshAgent => RISK_SSH_AGENT,
            Self::Database => RISK_DATABASE,
            Self::CertificateCheck => RISK_TRUSTD,
            Self::Unknown => RISK_UNKNOWN,
        }
    }
}

/// What `what` is, as far as its name tells: a socket by its path, a service by its name. A
/// host or a folder is [`Known::Unknown`]: their Notices say their own words.
pub fn known(what: &What) -> Known {
    match what {
        What::Socket(path) => known_socket(path, std::env::var_os("SSH_AUTH_SOCK").as_deref()),
        What::Service(name) if name == TRUSTD => Known::CertificateCheck,
        _ => Known::Unknown,
    }
}

/// What allowing `what` means ([`Known::risk`]).
pub fn risk(what: &What) -> &'static str {
    known(what).risk()
}

/// [`known`] for a socket at `path`, with `ssh_auth_sock` the person's own agent's, where set.
pub fn known_socket(path: &Path, ssh_auth_sock: Option<&std::ffi::OsStr>) -> Known {
    if ssh_auth_sock
        .map(Path::new)
        .is_some_and(|agent| agent == path || super::real(agent) == path)
    {
        return Known::SshAgent;
    }
    let whole = path.to_string_lossy().to_lowercase();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let containers = [
        "docker",
        "podman",
        "colima",
        "orbstack",
        "containerd",
        "/.rd/",
    ];
    // Lima by a folder of its own name, never a word inside another ("climate").
    let lima = path.components().any(|part| {
        let part = part.as_os_str().to_string_lossy().to_lowercase();
        part == "lima" || part == ".lima"
    });
    if lima || containers.iter().any(|word| whole.contains(word)) {
        return Known::Containers;
    }
    let agents = [
        "ssh-agent",
        "ssh_agent",
        "gpg-agent.ssh",
        "agent.sock",
        "/listeners",
    ];
    if agents.iter().any(|word| whole.contains(word))
        // OpenSSH's own agent: `/tmp/ssh-XXXX/agent.<pid>`.
        || (name.starts_with("agent.") && whole.contains("/ssh-"))
    {
        return Known::SshAgent;
    }
    let databases = [
        ".s.pgsql.",
        "mysql",
        "mysqld",
        "mariadb",
        "redis",
        "mongodb",
        "valkey",
    ];
    if databases.iter().any(|word| name.contains(word)) {
        return Known::Database;
    }
    Known::Unknown
}

/// A place a known service keeps its socket: under the home folder (`~/`), or from `/`.
const KNOWN_PLACES: [&str; 14] = [
    // Container engines.
    "~/.docker/run/docker.sock",
    "/var/run/docker.sock",
    "~/.colima/default/docker.sock",
    "~/.orbstack/run/docker.sock",
    "~/.rd/docker.sock",
    "~/.lima/default/sock/docker.sock",
    "~/.lima/docker/sock/docker.sock",
    "~/.local/share/containers/podman/machine/podman.sock",
    "~/.local/share/containers/podman/machine/podman-machine-default/podman.sock",
    "~/.local/share/containers/podman/machine/qemu/podman.sock",
    // An ssh agent that is not the environment's.
    "~/.gnupg/S.gpg-agent.ssh",
    // Databases that keep their socket where only they write.
    "/var/run/postgresql/.s.PGSQL.5432",
    "/var/run/mysqld/mysqld.sock",
    "/var/run/redis/redis.sock",
];

/// **Where the local sockets purlis can recognise would be on this machine** (ADR 0093, amended
/// 2026-10-13): what a person is offered to choose from when a refusal named no socket. A fixed,
/// audited table of the places known services keep theirs ([`KNOWN_PLACES`]), and what the
/// app's own environment names, as the person's shell would: `DOCKER_HOST` and `CONTAINER_HOST`
/// (a `unix://` one only), `SSH_AUTH_SOCK`, `PGHOST` (a folder, with `PGPORT`),
/// `MYSQL_UNIX_PORT`, and the engines' sockets under `XDG_RUNTIME_DIR`.
///
/// **Nothing a chat wrote is read**: no file is opened, no program is asked, and `env` is the
/// app's own process environment, never a chat's. The order is this function's, container
/// engines first, then agents, then databases, and nothing else moves it. Each is only a place
/// to look: [`discovered`] keeps the ones that are a socket a grant may name now. A database
/// that keeps its socket in `/tmp` is not listed, since no grant may name it there.
pub fn candidates(
    home: Option<&Path>,
    env: &dyn Fn(&str) -> Option<std::ffi::OsString>,
) -> Vec<PathBuf> {
    let named = |key: &str| env(key).filter(|value| !value.is_empty());
    // A `unix://` endpoint, or a bare path; never another scheme (`tcp://`, `ssh://`).
    let endpoint = |key: &str| {
        let value = named(key)?.to_string_lossy().into_owned();
        let path = value.strip_prefix("unix://").unwrap_or(&value);
        (path.starts_with('/')).then(|| PathBuf::from(path))
    };
    let runtime = named("XDG_RUNTIME_DIR").map(PathBuf::from);
    let table = |from: usize, to: usize| {
        KNOWN_PLACES[from..to]
            .iter()
            .filter_map(|place| match place.strip_prefix("~/") {
                Some(rest) => home.map(|home| home.join(rest)),
                None => Some(PathBuf::from(place)),
            })
            .collect::<Vec<_>>()
    };
    let mut out: Vec<PathBuf> = Vec::new();
    out.extend(endpoint("DOCKER_HOST"));
    out.extend(endpoint("CONTAINER_HOST"));
    out.extend(table(0, 10));
    if let Some(runtime) = &runtime {
        out.push(runtime.join("docker.sock"));
        out.push(runtime.join("podman/podman.sock"));
    }
    out.extend(endpoint("SSH_AUTH_SOCK"));
    out.extend(table(10, 11));
    if let Some(folder) = endpoint("PGHOST") {
        let port = named("PGPORT")
            .map(|port| port.to_string_lossy().into_owned())
            .filter(|port| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or_else(|| "5432".to_owned());
        out.push(folder.join(format!(".s.PGSQL.{port}")));
    }
    out.extend(endpoint("MYSQL_UNIX_PORT"));
    out.extend(table(11, KNOWN_PLACES.len()));
    out.into_iter().fold(Vec::new(), |mut once, path| {
        if !once.contains(&path) {
            once.push(path);
        }
        once
    })
}

/// A socket purlis found on this machine that a grant may name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// As a grant names it: the path the kernel names it by, links resolved.
    pub path: PathBuf,
    pub known: Known,
}

/// **The sockets among `candidates` a grant may name now**, each as [`socket`] judges it on
/// `ground`, in the order given and once each by the path it resolves to (`/var/run/docker.sock`
/// is often a link to the one under the home folder). One that is not there, is not a socket,
/// or that `ground` refuses is left out: what is listed is what an Allow could keep.
pub fn discovered(candidates: &[PathBuf], ground: &SocketGround) -> Vec<Found> {
    let agent = std::env::var_os("SSH_AUTH_SOCK");
    discovered_by(candidates, agent.as_deref(), &|typed| socket(typed, ground))
}

/// [`discovered`], each candidate judged by `judge` and the person's agent at `ssh_auth_sock`.
fn discovered_by(
    candidates: &[PathBuf],
    ssh_auth_sock: Option<&std::ffi::OsStr>,
    judge: &dyn Fn(&str) -> Result<PathBuf, Refused>,
) -> Vec<Found> {
    let mut out: Vec<Found> = Vec::new();
    for candidate in candidates {
        let Some(typed) = candidate.to_str() else {
            continue;
        };
        if let Ok(path) = judge(typed)
            && !out.iter().any(|found| found.path == path)
        {
            let known = known_socket(&path, ssh_auth_sock);
            out.push(Found { path, known });
        }
    }
    out
}

/// **The system services purlis knows how to allow**, by their whole names: what a person is
/// offered to choose from when a refusal named no service. A service has no path to check, so
/// none is typed: one that is not listed here is not chosen in the window.
///
/// **None today.** The certificate check was the one listed, and it is no grant any more (R1,
/// 2026-10-13): every sandboxed chat has it unless the project or policy turns it off, so
/// there is nothing of it to choose or allow. A service is listed here only once purlis can
/// say what allowing it means.
///
/// **The window's service chooser is kept for that service** (#1927), not removed: the list,
/// the commands and the form (`sandboxchoose`, `SandboxChoose.tsx`) serve sockets and services
/// alike, and their service half is tested. Listing one here is not enough to offer it: the
/// app's `hooks::choosable` offers a service to choose only once it is given back its arm,
/// with the sentence its Notice says in place of "nothing to allow".
pub const GRANTABLE_SERVICES: &[&str] = &[];

impl Known {
    /// What a person reads a known kind as, in a list to choose from.
    pub fn named(self) -> &'static str {
        match self {
            Self::Containers => "Container engine",
            Self::SshAgent => "SSH agent",
            Self::Database => "Database",
            Self::CertificateCheck => "Certificate check",
            Self::Unknown => "Local service",
        }
    }
}

/// **Where a socket a grant names may not be** (MF-3 of #1808's review): every folder a chat of
/// the project may write, so a chat can neither put a socket of its choosing where a grant
/// would name it, nor link one it may not reach there under a name purlis would read as
/// harmless. And what the sandbox denies by class.
#[derive(Debug, Clone, Default)]
pub struct SocketGround {
    pub denied: Vec<Denial>,
    /// Folders a chat may write, each as the kernel names it, from what the compilers let a chat
    /// write: the project tree, the system's temp folder for this user, Claude Code's own temp
    /// root, each harness's own home and temp folders, the project's package-cache home, and
    /// every folder a person let chats write.
    pub writable: Vec<PathBuf>,
    /// The names a harness's own temp root starts with in a system temp folder (`claude-`).
    pub harness_temp: Vec<String>,
}

impl SocketGround {
    /// The ground for a socket grant in the project at `root` on `machine`, with `writes`, the
    /// folders a person let this chat write besides.
    pub fn of(root: &Path, machine: &super::Machine, writes: &[PathBuf]) -> Self {
        let ground = super::grant::Ground::of(root, root, machine);
        let mut writable = vec![super::real(root), super::real(&std::env::temp_dir())];
        // Claude Code's own temp root, which its sandbox lets every chat of this user write.
        for temp in ["/tmp/claude", "/private/tmp/claude"] {
            writable.push(super::real(Path::new(temp)));
        }
        writable.extend(ground.refused.iter().map(|path| super::real(path)));
        writable.extend(writes.iter().cloned());
        // The project's package-cache home, which the toolchains preset lets a chat write.
        let denied = super::Denied::of(root, machine);
        if let Some(caches) = super::caches::home_of(machine, root, &denied) {
            writable.push(super::real(&caches.root));
            writable.extend(
                caches
                    .trees
                    .iter()
                    .chain(&caches.bare)
                    .chain(&caches.files)
                    .cloned(),
            );
        }
        writable.extend(super::local::granted_writes(root));
        writable.extend(super::local::grantable_folders(root));
        Self {
            denied: ground.denied,
            writable,
            harness_temp: ground
                .harness_temp
                .iter()
                .map(|one| (*one).to_owned())
                .collect(),
        }
    }

    /// Why a socket at `resolved` may not be granted here, or none.
    fn refuses(&self, resolved: &Path) -> Option<Refused> {
        if let Some(denial) = self.denied.iter().find(|denial| {
            let held = |path: &Path| match denial.reach {
                super::Reach::AndBelow => resolved.starts_with(path),
                // An entry held alone holds nothing below it (#2752).
                super::Reach::Itself => resolved == path,
            };
            held(&denial.path) || held(&super::real(&denial.path))
        }) {
            return Some(Refused::Never(
                super::grant::brokered_route(denial.class).to_owned(),
            ));
        }
        let in_a_harness_temp =
            ["/private/tmp", "/tmp", "/private/var/folders"]
                .iter()
                .any(|temp| {
                    resolved.strip_prefix(temp).is_ok_and(|rest| {
                        rest.components().any(|part| {
                            let name = part.as_os_str().to_string_lossy();
                            self.harness_temp
                                .iter()
                                .any(|prefix| name.starts_with(prefix.as_str()))
                        })
                    })
                });
        let writable = in_a_harness_temp
            || self
                .writable
                .iter()
                .any(|folder| !folder.as_os_str().is_empty() && resolved.starts_with(folder));
        writable.then(|| {
            Refused::Outside(
                "purlis will not allow a socket in a folder a chat may write, such as the temp \
                 folder: a chat could put any socket there under a harmless name. Have the \
                 service keep its socket in a folder of yours that chats cannot write (for \
                 Podman, a `podman machine` socket under ~/.local/share/containers), then \
                 allow that one."
                    .to_owned(),
            )
        })
    }
}

/// **The local sockets of the system's name lookup** (#2657): Linux's
/// ([`crate::sandboxblock::RESOLVER_SOCKETS`], which also sorts a refusal at one as a lookup)
/// and the Mac's own. No socket grant names one.
pub const NAME_LOOKUP_SOCKETS: [&str; 8] = [
    "/run/systemd/resolve/io.systemd.Resolve",
    "/run/nscd/socket",
    "/var/run/nscd/socket",
    "/var/lib/sss/pipes/nss",
    "/run/avahi-daemon/socket",
    "/var/run/avahi-daemon/socket",
    "/run/systemd/resolve/io.systemd.Resolve.Monitor",
    "/var/run/mDNSResponder",
];

/// **The system services of the Mac's own name lookup** (#2657): no service grant names one.
pub const NAME_LOOKUP_SERVICES: [&str; 1] = ["com.apple.dnssd.service"];

/// What purlis says where an Allow names the system's name lookup (#2657): the rule, then why,
/// one sentence each.
pub const NAME_LOOKUP_REFUSED: &str = "purlis never allows the system's name lookup. It would let \
                                       a chat look up names its sandbox does not let it reach.";

/// **Whether `path` is a socket of the system's name lookup** ([`NAME_LOOKUP_SOCKETS`], #2657),
/// by any of its names: as written, with `.` and `..` read as written, as the path it leads to,
/// across a firmlink, or as the same file: no grant names it, at any level, so what a Notice
/// offers for it and what a grant keeps agree.
pub fn is_name_lookup(path: &Path) -> bool {
    let lexical = lexically(path);
    let names = names_of(&lexical);
    let file = same_file_key(&lexical);
    NAME_LOOKUP_SOCKETS.iter().any(|lookup| {
        let lookup = Path::new(lookup);
        names_of(lookup).iter().any(|name| names.contains(name))
            || file.is_some() && file == same_file_key(lookup)
    })
}

/// Every name `path` goes by: as written, as the kernel names it, and across a firmlink
/// ([`super::kernel_names`]).
fn names_of(path: &Path) -> Vec<PathBuf> {
    let mut names = super::kernel_names(path);
    names.push(path.to_path_buf());
    names
}

/// The device and file `path` is, where something is there: two names of one file share it.
#[cfg(unix)]
fn same_file_key(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path)
        .ok()
        .map(|meta| (meta.dev(), meta.ino()))
}

/// No file key where there is no unix: the names alone are compared.
#[cfg(not(unix))]
fn same_file_key(_: &Path) -> Option<(u64, u64)> {
    None
}

/// **Whether `name` is a service of the system's name lookup** ([`NAME_LOOKUP_SERVICES`],
/// #2657), in any case.
pub fn is_name_lookup_service(name: &str) -> bool {
    NAME_LOOKUP_SERVICES
        .iter()
        .any(|lookup| lookup.eq_ignore_ascii_case(name.trim()))
}

/// [`NAME_LOOKUP_REFUSED`] where `what` is a socket or a service of the system's name lookup.
pub fn name_lookup_refused(what: &What) -> Option<&'static str> {
    match what {
        What::Socket(path) if is_name_lookup(path) => Some(NAME_LOOKUP_REFUSED),
        What::Service(name) if is_name_lookup_service(name) => Some(NAME_LOOKUP_REFUSED),
        _ => None,
    }
}

/// `path` with `.` dropped and `..` taking off the name before it, read from the words alone.
fn lexically(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// **`typed` as a local socket a grant names**, or why not: the path the kernel names it by,
/// links resolved ([`super::real`]). It must be absolute, a socket now with one name alone (a
/// second hard link could be one a chat made), outside every folder a chat may write, and
/// under no path the sandbox denies by class ([`SocketGround`]).
pub fn socket(typed: &str, ground: &SocketGround) -> Result<PathBuf, Refused> {
    let typed = typed.strip_prefix("unix://").unwrap_or(typed);
    let path = Path::new(typed);
    if !path.is_absolute() {
        return Err(Refused::Not(
            "purlis allows a local socket by its whole path, from /.".to_owned(),
        ));
    }
    if is_name_lookup(path) {
        return Err(Refused::Outside(NAME_LOOKUP_REFUSED.to_owned()));
    }
    let resolved = super::real(path);
    match shape(&resolved) {
        Shape::Socket => {}
        Shape::Linked => {
            return Err(Refused::Outside(
                "That socket has more than one name, so purlis cannot tell which service it is."
                    .to_owned(),
            ));
        }
        Shape::Foreign => {
            return Err(Refused::Outside(
                "That socket belongs to another user, so purlis cannot tell who made it."
                    .to_owned(),
            ));
        }
        Shape::Exposed => {
            return Err(Refused::Outside(
                "That socket is in a folder others may write, such as /tmp, so purlis cannot \
                 tell who put it there. A service that keeps its socket in a folder of yours can \
                 be allowed."
                    .to_owned(),
            ));
        }
        Shape::Other | Shape::Gone => {
            return Err(Refused::Not(
                "That is not a local socket on this machine now, so there is nothing to allow."
                    .to_owned(),
            ));
        }
    }
    if let Some(refused) = ground.refuses(&resolved) {
        return Err(refused);
    }
    Ok(resolved)
}

/// What is at a path now, as a socket grant asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// A socket with one name.
    Socket,
    /// A socket with more than one name: a hard link.
    Linked,
    /// A socket owned by someone other than this user or the system: anyone could have made it.
    Foreign,
    /// A socket in a folder others may write that this user does not own (`/tmp`): anyone
    /// could have put it there, or put another in its place.
    Exposed,
    /// Anything else: a folder, a file.
    Other,
    /// Nothing.
    Gone,
}

/// What one socket's file and its folder say, as [`shape_of`] judges them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Seen {
    socket: bool,
    links: u64,
    owner: u32,
    /// Its folder's mode bits and owner, where the folder is there.
    folder: Option<(u32, u32)>,
}

/// **The shape a socket is in, judged from what was `seen` of it**, for the user `me`. A pure
/// function, so each rule is tested apart from any socket on the machine:
/// - a socket with a second name is [`Shape::Linked`];
/// - one neither `me` nor the system (uid 0) owns is [`Shape::Foreign`];
/// - one whose folder others may write is [`Shape::Exposed`], unless the folder is `me`'s, or
///   the system's and others may only write it as a system group (`/private/var/run`,
///   root:daemon 0775). A folder anyone may write (`/tmp`) is exposed whoever owns it: anyone
///   could have put a socket there, or put another in its place.
fn shape_of(seen: Seen, me: u32) -> Shape {
    if !seen.socket {
        return Shape::Other;
    }
    if seen.links != 1 {
        return Shape::Linked;
    }
    if seen.owner != 0 && seen.owner != me {
        return Shape::Foreign;
    }
    if seen.folder.is_some_and(|folder| folder_exposed(folder, me)) {
        return Shape::Exposed;
    }
    Shape::Socket
}

/// Whether a folder of `(mode, owner)` lets others than `me` put a socket in it.
fn folder_exposed((mode, owner): (u32, u32), me: u32) -> bool {
    let anyone = mode & 0o002 != 0;
    let group = mode & 0o020 != 0;
    owner != me && (anyone || (group && owner != 0))
}

/// Whether the folder `path` is in lets others put a socket there ([`folder_exposed`]). A folder
/// that is not there holds nothing anyone put there yet.
#[cfg(unix)]
fn exposed(path: &Path) -> bool {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let Some(folder) = path.parent() else {
        return true;
    };
    let Ok(meta) = std::fs::metadata(folder) else {
        return false;
    };
    folder_exposed(
        (meta.permissions().mode(), meta.uid()),
        rustix::process::getuid().as_raw(),
    )
}

#[cfg(not(unix))]
fn exposed(_path: &Path) -> bool {
    true
}

fn shape(path: &Path) -> Shape {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
        let Ok(meta) = std::fs::symlink_metadata(path) else {
            return Shape::Gone;
        };
        let folder = path
            .parent()
            .and_then(|folder| std::fs::metadata(folder).ok())
            .map(|folder| (folder.permissions().mode(), folder.uid()));
        shape_of(
            Seen {
                socket: meta.file_type().is_socket(),
                links: meta.nlink(),
                owner: meta.uid(),
                folder,
            },
            rustix::process::getuid().as_raw(),
        )
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Shape::Gone
    }
}

/// The name parts of a keychain's services, which no person grants a chat (class 1).
const NEVER_SERVICES: [&str; 4] = ["securityserver", "securityd", "secd", "keychain"];

/// **`typed` as a system service a grant names**, or why not: a reverse-DNS name, whole, never
/// a pattern; never a keychain's.
pub fn service(typed: &str) -> Result<String, Refused> {
    let name = typed.trim();
    let shaped = name.contains('.')
        && name.len() <= 200
        && !name.starts_with('.')
        && !name.ends_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if !shaped {
        return Err(Refused::Not(
            "purlis allows a system service by its whole name, such as com.apple.trustd.agent."
                .to_owned(),
        ));
    }
    if is_name_lookup_service(name) {
        return Err(Refused::Outside(NAME_LOOKUP_REFUSED.to_owned()));
    }
    let lower = name.to_ascii_lowercase();
    if NEVER_SERVICES.iter().any(|part| lower.contains(part)) {
        return Err(Refused::Never(
            super::grant::brokered_route(super::Class::Vaults).to_owned(),
        ));
    }
    Ok(name.to_owned())
}

/// `sockets`, kept grants, less each that no longer resolves to itself (it, or a folder above
/// it, was swapped for a link since), that is something other than a socket of one name now, or
/// that `ground` refuses: what a start compiles in. One that is not there now is kept: a
/// container engine that is not running has no socket until it starts, and it makes the socket
/// anew each time, so a grant is of the path, held to these rules at every start.
pub fn still_sockets(sockets: &[PathBuf], ground: &SocketGround) -> Vec<PathBuf> {
    sockets
        .iter()
        .filter(|path| path.is_absolute() && super::real(path) == **path)
        .filter(|path| !is_name_lookup(path))
        .filter(|path| matches!(shape(path), Shape::Socket | Shape::Gone))
        .filter(|path| !exposed(path))
        .filter(|path| ground.refuses(path).is_none())
        .fold(Vec::new(), |mut out, path| {
            if !out.contains(path) {
                out.push(path.clone());
            }
            out
        })
}

/// `services`, kept grants, each still one a grant may name.
pub fn still_services(services: &[String]) -> Vec<String> {
    services
        .iter()
        .filter_map(|name| service(name).ok())
        .fold(Vec::new(), |mut out, name| {
            if !out.contains(&name) {
                out.push(name);
            }
            out
        })
}

/// The key in the committed `[sandbox]` table that **declares** the local sockets and system
/// services a project expects (D-sock-4, the operator's ruling of 2026-10-12): socket paths
/// (from `/`) and service names, as a Flatpak manifest declares what an app needs. It **never
/// grants anything**: it shapes what an ask says ("this project expects it") and what Settings
/// lists, and each person still allows on their own machine, in their local file.
pub const EXPECTS: &str = "expects";

/// What a project's committed file declares it expects: never a grant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Declared {
    pub sockets: Vec<PathBuf>,
    pub services: Vec<String>,
}

impl Declared {
    /// Whether `what` is one the project declares it expects: a socket as written or as the
    /// kernel names it, a service by its whole name.
    pub fn expects(&self, what: &What) -> bool {
        match what {
            What::Socket(path) => self
                .sockets
                .iter()
                .any(|one| one == path || super::real(one) == *path),
            What::Service(name) => self.services.iter().any(|one| one == name),
            What::Host(_) | What::Write(_) | What::Port(_) | What::Listen(_) => false,
        }
    }
}

/// The longest socket path a declaration names: far past any path a socket can be bound at.
const MOST_DECLARED_PATH: usize = 1024;

/// **The most a project declares**, sockets and services together: far past what a project
/// needs, and few enough to draw whole in one question and to look at on this machine each
/// time the declaration is read. What a file lists past it declares nothing.
pub const MOST_DECLARED: usize = 32;

/// Whether `path`, a socket's as a repository wrote it, can be said plainly: no longer than a
/// path is, and with no character that draws as nothing (a control, a zero-width or
/// direction-changing one: [`purlis_unicode::invisible`]). It is drawn to a person and named in
/// a question, so one that cannot be is no declaration.
fn said_plainly(path: &str) -> bool {
    path.len() <= MOST_DECLARED_PATH
        && path
            .chars()
            .all(|ch| ch == ' ' || !purlis_unicode::invisible(ch))
}

/// What the committed `[sandbox] expects` in `top`, a whole `charter.toml`, declares. Anything
/// that is neither a whole socket path nor a service name is left out: a declaration says
/// nothing it cannot say plainly ([`said_plainly`], [`service`]). And no more than [`MOST_DECLARED`] of
/// them, in the order the file lists them.
pub fn declared_in(top: Option<&toml::Table>) -> Declared {
    let mut out = Declared::default();
    let Some(listed) = top
        .and_then(|top| top.get(super::TABLE))
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get(EXPECTS))
        .and_then(toml::Value::as_array)
    else {
        return out;
    };
    for one in listed.iter().filter_map(toml::Value::as_str) {
        if out.sockets.len() + out.services.len() >= MOST_DECLARED {
            break;
        }
        if one.starts_with('/') {
            let path = PathBuf::from(one);
            if said_plainly(one) && !out.sockets.contains(&path) {
                out.sockets.push(path);
            }
        } else if let Ok(name) = service(one)
            // The certificate check is the project's own setting, never a grant (R1): a
            // declaration of it would ask a person for what every chat has already.
            && name != TRUSTD
            && !out.services.contains(&name)
        {
            out.services.push(name);
        }
    }
    out
}

/// What the project at `root` declares it expects, from its committed file.
pub fn declared(root: &Path) -> Declared {
    let text = std::fs::read_to_string(crate::names::manifest(root)).ok();
    let top = text.and_then(|text| text.parse::<toml::Table>().ok());
    declared_in(top.as_ref())
}

/// What an ask about `what` says it means: [`risk`], led by "This project expects it" where
/// the project's committed file declares it ([`Declared::expects`]). The declaration changes the
/// words, and grants nothing.
pub fn said_of(what: &What, declared: &Declared) -> String {
    let meaning = match known(what) {
        Known::Unknown => format!("It is {RISK_UNKNOWN}."),
        known => format!("Allowing it {}.", known.risk()),
    };
    if declared.expects(what) {
        format!("This project expects it. {meaning}")
    } else {
        meaning
    }
}

/// One local socket or system service the project declares it expects and nobody has allowed
/// on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Awaited {
    /// The socket, as the kernel names it where it can be allowed and as the file writes it
    /// otherwise; or the service by its whole name.
    pub what: What,
    /// As the committed file writes it: what an answer is recorded by.
    pub written: String,
    /// What allowing it means, in the words its Notice would say ([`said_of`]).
    pub said: String,
    /// Why it cannot be allowed on this machine now, where it cannot: the socket is not
    /// there, or is somewhere no grant names.
    pub refused: Option<String>,
}

/// **What the project declares it expects and this machine has not allowed** (ADR 0093, ruling
/// 2: a committed file declares and never grants; each person allows on their own machine).
/// Asked about up front, once, where the declaration is read, so nobody learns of it from a
/// tool that failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Awaiting {
    pub items: Vec<Awaited>,
    /// Why none of it can be allowed here, where an administrator's policy forbids it (every
    /// socket and service grant, or allowing one for yourself on this machine): said with who
    /// set it, and nothing is offered.
    pub locked: Option<String>,
    /// Whether the person here is still to be asked: an item is declared that they have
    /// neither allowed nor answered "Not now" to. Settings offers the Allow either way.
    pub asked: bool,
}

impl Awaiting {
    /// The declaration as the question shows it, for the answer to record.
    pub fn shown(&self) -> Vec<String> {
        self.items.iter().map(|item| item.written.clone()).collect()
    }

    /// Each item as the file writes it with the name it is shown under now: what an answer
    /// to the question as it stands records ([`super::local::acknowledge_expects`]).
    pub fn answered_as_named(&self) -> Vec<(String, String)> {
        self.items.iter().map(Awaited::as_named).collect()
    }
}

impl Awaited {
    /// As the file writes it, and the name it is shown under now: the socket an Allow would
    /// keep, links resolved, or the path as written where it cannot be allowed; a service's
    /// own name.
    pub fn as_named(&self) -> (String, String) {
        (self.written.clone(), self.what.target())
    }
}

/// **Whether the person is still to be asked about any of `items`** (#1927): one they have
/// not answered under the name it has now. An answer is to what was shown, so a declared path
/// that names another socket since is asked about again, and one that names a socket already
/// answered is not.
fn still_asked(items: &[Awaited], answered: &[super::local::Answered]) -> bool {
    items.iter().any(|item| {
        let named = item.what.target();
        !answered
            .iter()
            .any(|one| one.written == item.written && one.named.contains(&named))
    })
}

/// [`Awaiting`] for the project at `root` on `machine`, or none: where its chats are not
/// sandboxed, where it declares nothing, and where everything it declares is allowed here
/// already, for this project or for every project.
pub fn awaiting(root: &Path, machine: &super::Machine) -> Option<Awaiting> {
    let locks = super::policy::Locks::of(root);
    super::Plane::read(root).in_force(&locks)?;
    let declared = declared(root);
    let ground = SocketGround::of(root, machine, &[]);
    // Allowed already only where policy leaves the scope it was kept at open (#1927): one
    // held by policy reaches no chat, so it waits again and is asked about.
    let (sockets, services) = super::kept_in_scope(root, &locks);
    let none = Declared::default();
    let mut items = Vec::new();
    for written in &declared.sockets {
        let spelled = written.display().to_string();
        let (what, refused) = match socket(&spelled, &ground) {
            Ok(resolved) => (What::Socket(resolved), None),
            Err(why) => (What::Socket(written.clone()), Some(why.to_string())),
        };
        let What::Socket(path) = &what else { continue };
        if sockets
            .iter()
            .any(|one| one == path || one == written || super::real(one) == *path)
        {
            continue;
        }
        items.push(Awaited {
            said: said_of(&what, &none),
            what,
            written: spelled,
            refused,
        });
    }
    for name in &declared.services {
        if services.contains(name) {
            continue;
        }
        let what = What::Service(name.clone());
        items.push(Awaited {
            said: said_of(&what, &none),
            what,
            written: name.clone(),
            refused: None,
        });
    }
    if items.is_empty() {
        return None;
    }
    let answered = super::local::expects_answered(root);
    Some(Awaiting {
        asked: still_asked(&items, &answered),
        // The level the Allow keeps at, so what is offered is what would be kept.
        locked: items
            .first()
            .and_then(|item| locks.refuses_grant(&item.what, super::grant::Level::You)),
        items,
    })
}

/// **What a chat's pane says from its start** where the project expects something this
/// machine has not allowed and the chat does not hold (`has`, what its sandbox was compiled
/// with): one line, so the first sign is never a tool that failed. None once the person
/// answered "Not now", where policy locks it (the project's Notice says that once), and for a
/// chat that has everything declared.
///
/// **The sentence is purlis's own, whole**: it counts what is missing and never names it. A
/// repository wrote those names, and one set inside a sentence of purlis's could be read as
/// purlis speaking. The project's Notice and Settings name each, drawn as data.
pub fn start_note(root: &Path, machine: &super::Machine, has: &super::Widened) -> Option<String> {
    let awaiting = awaiting(root, machine)?;
    if !awaiting.asked || awaiting.locked.is_some() {
        return None;
    }
    let missing = awaiting
        .items
        .iter()
        .filter(|item| match &item.what {
            What::Socket(path) => !has.sockets.contains(path),
            What::Service(name) => !has.services.contains(name),
            What::Host(_) | What::Write(_) | What::Port(_) | What::Listen(_) => false,
        })
        .count();
    let (what, it) = match missing {
        0 => return None,
        1 => ("a local socket or system service".to_owned(), "it"),
        many => (format!("{many} local sockets or system services"), "them"),
    };
    Some(format!(
        "This project expects {what} this machine has not allowed, so this chat cannot reach \
         {it}. See and allow {it} on the project's notice or in Settings › Sandbox; this chat \
         takes {it} when it restarts."
    ))
}

#[cfg(test)]
mod tests;
