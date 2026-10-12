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
/// as [`super::seatbelt::TRUST`].
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
        "lima",
    ];
    if containers.iter().any(|word| whole.contains(word)) {
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

/// **`typed` as a local socket a grant names**, or why not: the path the kernel names it by,
/// links resolved ([`super::real`]). It must be absolute, a socket now, and under no path
/// `denied` denies by class.
pub fn socket(typed: &str, denied: &[Denial]) -> Result<PathBuf, Refused> {
    let typed = typed.strip_prefix("unix://").unwrap_or(typed);
    let path = Path::new(typed);
    if !path.is_absolute() {
        return Err(Refused::Not(
            "purlis allows a local socket by its whole path, from /.".to_owned(),
        ));
    }
    let resolved = super::real(path);
    if !is_socket(&resolved) {
        return Err(Refused::Not(
            "That is not a local socket on this machine now, so there is nothing to allow."
                .to_owned(),
        ));
    }
    if let Some(denial) = denied.iter().find(|denial| {
        resolved.starts_with(&denial.path) || resolved.starts_with(super::real(&denial.path))
    }) {
        return Err(Refused::Never(
            super::grant::brokered_route(denial.class).to_owned(),
        ));
    }
    Ok(resolved)
}

/// Whether `path` is a socket now.
fn is_socket(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        std::fs::metadata(path).is_ok_and(|meta| meta.file_type().is_socket())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
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
    let lower = name.to_ascii_lowercase();
    if NEVER_SERVICES.iter().any(|part| lower.contains(part)) {
        return Err(Refused::Never(
            super::grant::brokered_route(super::Class::Vaults).to_owned(),
        ));
    }
    Ok(name.to_owned())
}

/// `sockets`, kept grants, less each that no longer resolves to itself (it, or a folder above
/// it, was swapped for a link since): what a start compiles in. One that is not there now is
/// kept: a container engine that is not running has no socket until it starts.
pub fn still_sockets(sockets: &[PathBuf], denied: &[Denial]) -> Vec<PathBuf> {
    sockets
        .iter()
        .filter(|path| path.is_absolute() && super::real(path) == **path)
        .filter(|path| {
            !denied.iter().any(|denial| {
                path.starts_with(&denial.path) || path.starts_with(super::real(&denial.path))
            })
        })
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
            What::Host(_) | What::Write(_) => false,
        }
    }
}

/// What the committed `[sandbox] expects` in `top`, a whole `charter.toml`, declares. Anything
/// that is neither a whole socket path nor a service name is left out: a declaration says
/// nothing it cannot say plainly.
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
        if one.starts_with('/') {
            let path = PathBuf::from(one);
            if !out.sockets.contains(&path) {
                out.sockets.push(path);
            }
        } else if let Ok(name) = service(one)
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

#[cfg(test)]
#[path = "services_tests.rs"]
mod tests;
