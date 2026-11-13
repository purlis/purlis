// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Docker context, the third way to an engine on another machine** (#6268, ADR 0123 §2 and
//! §7).
//!
//! A context is read once, at Connect, into the SSH or TLS route its endpoint names
//! ([`Endpoint`]), with the paths of its TLS material where it has some. The pool keeps that
//! route. No run of `docker` follows a context (`DOCKER_CONTEXT` stays cleared,
//! [`crate::dockerpool::settings::CLEARED`]): the context's name is kept beside the route only so
//! that doctor can tell the person when the context now names somewhere else ([`moved`]).
//!
//! What a context holds is read from what `docker context ls` and `docker context inspect`
//! print ([`contexts_in`]), never from Docker's store on disk.

use std::path::{Path, PathBuf};

use crate::sshroute::connect::Finding;

/// **Where a context's endpoint is**, read once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    /// A socket on this machine (`unix://`, `npipe://`): the local Docker kind, not this one.
    Here,
    /// `ssh://`: the SSH route to that machine's engine.
    Ssh(crate::sshroute::Route),
    /// `tcp://` with the TLS material Docker's `--tlsverify` takes, kept by the context.
    Tls {
        route: crate::tlsroute::Route,
        files: crate::tlsroute::Files,
    },
    /// An endpoint purlis doesn't use, and why, in a sentence: plain `tcp://`, TLS that doesn't
    /// verify the server, or a scheme of another kind.
    Refused(String),
}

impl Endpoint {
    /// **The route the endpoint names**, as a pool keeps it: `None` for this machine or one
    /// refused.
    #[must_use]
    pub fn route(&self) -> Option<String> {
        match self {
            Endpoint::Ssh(route) => Some(route.spelled()),
            Endpoint::Tls { route, .. } => Some(route.spelled()),
            Endpoint::Here | Endpoint::Refused(_) => None,
        }
    }

    /// The TLS files, where the endpoint is reached over TLS.
    #[must_use]
    pub fn files(&self) -> Option<&crate::tlsroute::Files> {
        match self {
            Endpoint::Tls { files, .. } => Some(files),
            _ => None,
        }
    }
}

/// **A context of the person's Docker**: its name, whether it is the one their `docker` uses
/// now, and its endpoint, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerContext {
    pub name: String,
    pub current: bool,
    pub endpoint: Endpoint,
}

/// **The contexts Docker printed**: `listed`, what `docker context ls --format '{{json .}}'`
/// prints (one object a line, `Name` and `Current`), and `inspected`, what `docker context
/// inspect` prints for those names (one array). In the order `ls` lists them; one `inspect`
/// doesn't describe is left out.
#[must_use]
pub fn contexts_in(listed: &str, inspected: &str) -> Vec<DockerContext> {
    let Ok(serde_json::Value::Array(inspected)) = serde_json::from_str(inspected.trim()) else {
        return Vec::new();
    };
    let mut out: Vec<DockerContext> = Vec::new();
    for line in listed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(one) = serde_json::from_str::<serde_json::Value>(line) else {
            return Vec::new();
        };
        let Some(name) = one.get("Name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !a_name(name) || out.iter().any(|kept| kept.name == name) {
            continue;
        }
        let Some(described) = inspected.iter().find(|described| {
            described.get("Name").and_then(serde_json::Value::as_str) == Some(name)
        }) else {
            continue;
        };
        let Some(endpoint) = endpoint_of(described) else {
            continue;
        };
        out.push(DockerContext {
            name: name.to_owned(),
            current: one.get("Current").and_then(serde_json::Value::as_bool) == Some(true),
            endpoint,
        });
    }
    out
}

/// **A name a context can have**: Docker's own rule, `^[a-zA-Z0-9][a-zA-Z0-9_.+-]+$`, two
/// characters or more of letters, digits, `_`, `.`, `+` and `-`, starting with a letter or digit.
/// purlis holds it to that, and to 128 bytes at most.
pub fn a_name(name: &str) -> bool {
    (2..=128).contains(&name.len())
        && name
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|one| one.is_ascii_alphanumeric() || matches!(one, '.' | '_' | '+' | '-'))
}

/// **The endpoint one context `inspect` described names**, or `None` where it describes no
/// Docker endpoint at all.
fn endpoint_of(described: &serde_json::Value) -> Option<Endpoint> {
    let docker = described.get("Endpoints")?.get("docker")?;
    let host = docker.get("Host")?.as_str()?.trim();
    if host.starts_with("unix://") || host.starts_with("npipe://") {
        return Some(Endpoint::Here);
    }
    if host.starts_with("ssh://") {
        return Some(match crate::sshroute::Route::parse(host) {
            Ok(route) => Endpoint::Ssh(route),
            Err(why) => Endpoint::Refused(why),
        });
    }
    if !crate::tlsroute::is_tcp(host) {
        return Some(Endpoint::Refused(format!(
            "{} isn't an address purlis reaches Docker at: it reaches another machine at an \
             ssh:// or a tcp:// address with TLS",
            crate::shown::short(host)
        )));
    }
    let material: Vec<&str> = described
        .get("TLSMaterial")
        .and_then(|material| material.get("docker"))
        .and_then(serde_json::Value::as_array)
        .map(|files| files.iter().filter_map(serde_json::Value::as_str).collect())
        .unwrap_or_default();
    if docker
        .get("SkipTLSVerify")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        return Some(Endpoint::Refused(crate::tlsroute::NO_VERIFY.to_owned()));
    }
    if material.is_empty() {
        return Some(Endpoint::Refused(crate::sshroute::PLAIN_TCP.to_owned()));
    }
    if !material.contains(&crate::tlsroute::CA_PEM) {
        return Some(Endpoint::Refused(crate::tlsroute::NO_VERIFY.to_owned()));
    }
    if !material.contains(&crate::tlsroute::CERT_PEM)
        || !material.contains(&crate::tlsroute::KEY_PEM)
    {
        return Some(Endpoint::Refused(
            "purlis reaches Docker over TLS with a client certificate and its key, and this \
             context keeps none. Make the context again with them, or use ssh:// instead."
                .to_owned(),
        ));
    }
    let route = match crate::tlsroute::Route::parse(host) {
        Ok(route) => route,
        Err(why) => return Some(Endpoint::Refused(why)),
    };
    let folder = described
        .get("Storage")
        .and_then(|storage| storage.get("TLSPath"))
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .filter(|folder| folder.is_absolute());
    let Some(folder) = folder else {
        return Some(Endpoint::Refused(
            "Docker didn't say where this context keeps its TLS files".to_owned(),
        ));
    };
    Some(
        match crate::tlsroute::Files::new(
            tls_file(&folder, crate::tlsroute::CA_PEM),
            tls_file(&folder, crate::tlsroute::CERT_PEM),
            crate::tlsroute::KeyFrom::File(tls_file(&folder, crate::tlsroute::KEY_PEM)),
        ) {
            Ok(files) => Endpoint::Tls { route, files },
            Err(why) => Endpoint::Refused(why),
        },
    )
}

/// **Whether the context the pool's route was read from now names somewhere else** (ADR 0123
/// §7): the route it names now, where that is a route purlis uses and isn't `kept`. A context
/// that is gone, or now names this machine or something refused, has nothing to follow, and
/// the pool keeps its route as it always does.
#[must_use]
pub fn moved(contexts: &[DockerContext], name: &str, kept: &str) -> Option<String> {
    let now = contexts
        .iter()
        .find(|one| one.name == name)?
        .endpoint
        .route()?;
    (!same_route(&now, kept)).then_some(now)
}

/// **Whether the route a context names is the one kept**: the same machine, user and port, and,
/// over SSH, the same socket where the context names one (the pool's route has the socket found
/// at Connect written into it).
pub fn same_route(now: &str, kept: &str) -> bool {
    if crate::tlsroute::is_tcp(now) {
        return crate::tlsroute::Route::parse(now).ok() == crate::tlsroute::Route::parse(kept).ok();
    }
    let (Ok(now), Ok(kept)) = (
        crate::sshroute::Route::parse(now),
        crate::sshroute::Route::parse(kept),
    ) else {
        return false;
    };
    now.host() == kept.host()
        && now.user() == kept.user()
        && now.port() == kept.port()
        && now
            .socket()
            .is_none_or(|socket| kept.socket() == Some(socket))
}

/// The most `docker context` may print, and say, and how long it may take.
const MOST_PRINTED: u64 = 256 * 1024;
const MOST_SAID: u64 = 16 * 1024;
const RUN_WITHIN: std::time::Duration = std::time::Duration::from_secs(10);

/// **The person's Docker contexts** ([`contexts_in`]), as this machine's `docker` lists and
/// describes them: `docker context ls`, then `docker context inspect` of the names it listed.
/// None where `docker` lists none or can't. Each run reads the person's own Docker settings
/// (where the contexts are), with no context and no `DOCKER_HOST` handed on (so `Current` is
/// the context their settings choose) and Docker's plugin hooks off. It only reads.
#[must_use]
pub fn contexts_here(docker: &Path) -> Vec<DockerContext> {
    contexts_with(docker, None)
}

/// [`contexts_here`], with Docker's settings the folder `settings` (`--config`) where one is
/// named, as a test makes its own contexts in.
#[must_use]
pub fn contexts_with(docker: &Path, settings: Option<&Path>) -> Vec<DockerContext> {
    let run = |args: &[&str]| {
        let mut command = std::process::Command::new(docker);
        if let Some(settings) = settings {
            command.arg("--config").arg(settings);
        }
        command
            .args(args)
            .env_remove("DOCKER_CONTEXT")
            .env_remove("DOCKER_HOST")
            .env("DOCKER_CLI_HOOKS", "false");
        crate::boundedrun::run_within(
            &mut command,
            None,
            MOST_PRINTED,
            MOST_SAID,
            Some(RUN_WITHIN),
        )
        .ok()
        .filter(|ran| ran.status.success())
        .map(|ran| String::from_utf8_lossy(&ran.printed).into_owned())
    };
    let Some(listed) = run(&["context", "ls", "--format", "{{json .}}"]) else {
        return Vec::new();
    };
    let names: Vec<String> = contexts_listed(&listed);
    if names.is_empty() {
        return Vec::new();
    }
    let mut args = vec!["context", "inspect", "--"];
    args.extend(names.iter().map(String::as_str));
    run(&args).map_or_else(Vec::new, |inspected| contexts_in(&listed, &inspected))
}

/// The names `docker context ls` listed that a context can have, but for one it lists with an
/// error, which `docker context inspect` would refuse together with all the others.
fn contexts_listed(listed: &str) -> Vec<String> {
    listed
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line.trim()).ok())
        .filter(|one| {
            one.get("Error")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|error| error.trim().is_empty())
        })
        .filter_map(|one| one.get("Name")?.as_str().map(str::to_owned))
        .filter(|name| a_name(name))
        .collect()
}

/// **Where purlis keeps its own copies of the TLS files read from Docker contexts** (#6268):
/// [`crate::machine::DOCKER_CONTEXT_TLS_DIR`], a folder of this user's alone that no chat reads
/// or writes, one folder in it for each set of files kept.
#[must_use]
pub fn kept_tls_dir(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(crate::machine::DOCKER_CONTEXT_TLS_DIR)
}

/// A context's TLS files, read, and the folder of [`kept_tls_dir`] they are kept in: named by
/// what they hold, so the same files are kept once and others never over them.
struct ToKeep {
    ca: Vec<u8>,
    cert: Vec<u8>,
    key: zeroize::Zeroizing<Vec<u8>>,
    folder: PathBuf,
}

fn to_keep(
    config_root: &Path,
    files: &crate::tlsroute::Files,
    host: &str,
) -> Result<ToKeep, Finding> {
    use sha2::Digest;
    let Some(key) = files.key_file() else {
        return Err(Finding::Here(
            "a Docker context keeps its client key in a file, and this one names none".to_owned(),
        ));
    };
    let ca = crate::tlsroute::read_file(&files.ca, host)?;
    let cert = crate::tlsroute::read_file(&files.cert, host)?;
    let key = zeroize::Zeroizing::new(crate::tlsroute::read_file(key, host)?);
    let mut digest = sha2::Sha256::new();
    for part in [ca.as_slice(), cert.as_slice(), key.as_slice()] {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part);
    }
    let name: String = digest
        .finalize()
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(ToKeep {
        ca,
        cert,
        key,
        folder: kept_tls_dir(config_root).join(name),
    })
}

/// The files kept in `folder`, by the names Docker gives them.
fn kept_in(folder: &Path) -> crate::tlsroute::Files {
    crate::tlsroute::Files {
        ca: folder.join(crate::tlsroute::CA_PEM),
        cert: folder.join(crate::tlsroute::CERT_PEM),
        key: crate::tlsroute::KeyFrom::File(folder.join(crate::tlsroute::KEY_PEM)),
    }
}

/// **Where [`keep_tls`] would keep `files`**, read now, with nothing written: for Connect to see
/// that what it trusted earlier is these same files.
pub fn tls_copies_at(
    config_root: &Path,
    files: &crate::tlsroute::Files,
    host: &str,
) -> Result<crate::tlsroute::Files, Finding> {
    Ok(kept_in(&to_keep(config_root, files, host)?.folder))
}

/// **Copies a context's TLS files into purlis's own store, once** (#6268, the 2026-11-14
/// amendment of ADR 0123): the files at the paths `docker context inspect` named are Docker's,
/// which `docker context update` rewrites in place and `docker context rm` deletes, so a pool
/// runs on copies of its own instead. Read once, at Connect's trust step, and kept in a folder
/// of [`kept_tls_dir`] named by what they hold, the key `0600`; the files kept, which the trust
/// step then checks and the pool names. Nothing is kept where one can't be read.
pub fn keep_tls(
    config_root: &Path,
    files: &crate::tlsroute::Files,
    host: &str,
) -> Result<crate::tlsroute::Files, Finding> {
    let read = to_keep(config_root, files, host)?;
    let could_not = |why: std::io::Error| {
        Finding::Here(format!(
            "purlis could not keep its own copy of the TLS files for {host}: {why}"
        ))
    };
    purlis_same_user::PrivateDirectory::open(&kept_tls_dir(config_root)).map_err(could_not)?;
    let folder = purlis_same_user::PrivateDirectory::open(&read.folder).map_err(could_not)?;
    for (name, bytes) in [
        (crate::tlsroute::CA_PEM, read.ca.as_slice()),
        (crate::tlsroute::CERT_PEM, read.cert.as_slice()),
        (crate::tlsroute::KEY_PEM, read.key.as_slice()),
    ] {
        folder.replace_file(name, bytes).map_err(could_not)?;
    }
    Ok(kept_in(&read.folder))
}

/// The file `name` in a context's TLS folder for Docker's endpoint.
fn tls_file(folder: &Path, name: &str) -> PathBuf {
    folder.join("docker").join(name)
}

#[cfg(test)]
mod tests;
