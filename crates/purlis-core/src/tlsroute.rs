// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Docker's TLS API, the second route to an engine on another machine** (#6267, ADR 0123 §2–§4).
//!
//! A pool of kind `remote-docker` may name its engine as `tcp://host:port` ([`Route`]) with the
//! files Docker's `--tlsverify` takes ([`Files`]): the CA the server is verified against, the
//! client certificate, and its key, either a file where it already is or a vault entry
//! ([`KeyFrom`]). The pool is pinned to the SHA-256 of the CA certificate the person confirmed
//! ([`Fingerprint`]) and to the engine's id ([`Pin`]).
//!
//! - **Plain `tcp://`, and TLS that does not verify the server, are refused** ([`Remote::new`]),
//!   with ADR 0123 §2's sentence and the two ways forward.
//! - **Before every run the CA file is checked against the pin** ([`Through::material`]): its
//!   one certificate's SHA-256 is compared with the pinned one, and the bytes found to match are
//!   the ones the run is handed (`ca.pem`), never the file read again.
//! - **A key from a vault is on disk for one run alone**: it is written into that run's own
//!   settings folder (`0600` in `0700`), which goes when the run ends, whether it succeeded or not
//!   ([`crate::dockerpool::settings::prepare`]).
//! - **What the doctor says is decided by a TLS handshake of purlis's own** ([`handshake`]):
//!   rustls, with the pool's CA as the only root and its client certificate, says which check
//!   failed as a typed error (a CA that didn't sign the server's certificate, one that expired,
//!   one for another name, a client certificate the server refused). Nothing Docker prints is
//!   read to tell them apart.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::Digest;
use zeroize::Zeroizing;

use crate::sshroute::connect::Finding;

/// The scheme of a route over Docker's TLS API.
pub const SCHEME: &str = "tcp://";

/// The port Docker's TLS API listens on by convention, written into a route given without one.
pub const DOCKER_TLS_PORT: u16 = 2376;

/// **The refusal of TLS that does not verify the server** (ADR 0123 §2): no CA named.
pub const NO_VERIFY: &str = "purlis doesn't connect to Docker over TLS that doesn't verify the \
                             server: anything that answers on that port could take its place. \
                             Name the CA Docker's certificates are signed with, or use ssh:// \
                             instead.";

/// The most of a CA, certificate or key file purlis reads.
const MOST_READ: u64 = 256 * 1024;

/// How long the handshake and the engine's first answer may take.
pub const HANDSHAKE_WITHIN: Duration = Duration::from_secs(10);

/// How long a key read from a vault is kept in this process's memory before it is read again.
pub const VAULT_KEY_KEPT_FOR: Duration = Duration::from_secs(600);

/// **Where an engine reached over TLS is**: `tcp://host:port`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    host: String,
    port: u16,
}

impl Route {
    /// **`text` read as a route over TLS**, or why not. `tcp://host[:port]`, with nothing after
    /// but an optional `/`; the port is [`DOCKER_TLS_PORT`] where none is named.
    pub fn parse(text: &str) -> Result<Route, String> {
        let text = text.trim();
        let Some(rest) = text.strip_prefix(SCHEME) else {
            return Err(format!(
                "a route over Docker's TLS API starts with {SCHEME}, as in tcp://build-box:2376"
            ));
        };
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        if rest.contains('/') || rest.contains('@') {
            return Err(format!(
                "{} isn't an address Docker's TLS API is reached at: it is tcp://host:port and \
                 nothing else",
                crate::shown::short(text)
            ));
        }
        let (host, port) = crate::sshroute::split_port(rest)?;
        if !crate::sshroute::a_host(host) {
            return Err(if host.is_empty() {
                "the route names no machine".to_owned()
            } else {
                format!("{} isn't the name of a machine", crate::shown::short(host))
            });
        }
        Ok(Route {
            host: host.to_owned(),
            port: port.unwrap_or(DOCKER_TLS_PORT),
        })
    }

    /// The route as it is kept and as Docker's `DOCKER_HOST` takes it.
    #[must_use]
    pub fn spelled(&self) -> String {
        if self.host.contains(':') {
            format!("{SCHEME}[{}]:{}", self.host, self.port)
        } else {
            format!("{SCHEME}{}:{}", self.host, self.port)
        }
    }

    /// The machine, as the person named it.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Its port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl std::fmt::Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.spelled())
    }
}

/// **Whether `text` names an engine over Docker's TCP API**: `tcp://`, with or without TLS.
#[must_use]
pub fn is_tcp(text: &str) -> bool {
    text.trim().starts_with(SCHEME)
}

/// **The SHA-256 of a CA certificate's DER**: what a TLS pool is pinned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// The fingerprint of the certificate `der`.
    #[must_use]
    pub fn of_der(der: &[u8]) -> Fingerprint {
        Fingerprint(sha2::Sha256::digest(der).into())
    }

    /// `text`, `sha256:` and 64 hex digits, as it is kept, or the colon-separated pairs
    /// `openssl x509 -fingerprint -sha256` prints, read as a fingerprint.
    pub fn parse(text: &str) -> Result<Fingerprint, String> {
        let text = text.trim();
        let hex: String = text
            .strip_prefix("sha256:")
            .or_else(|| text.strip_prefix("SHA256:"))
            .or_else(|| text.strip_prefix("sha256 Fingerprint="))
            .or_else(|| text.strip_prefix("SHA256 Fingerprint="))
            .unwrap_or(text)
            .chars()
            .filter(|c| *c != ':')
            .collect();
        let refused = || {
            format!(
                "{} isn't a certificate's SHA-256 fingerprint",
                crate::shown::short(text)
            )
        };
        if hex.len() != 64 {
            return Err(refused());
        }
        let bytes = data_encoding::HEXLOWER_PERMISSIVE
            .decode(hex.as_bytes())
            .map_err(|_| refused())?;
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| refused())?;
        Ok(Fingerprint(bytes))
    }

    /// As it is kept: `sha256:` and 64 lower-case hex.
    #[must_use]
    pub fn spelled(&self) -> String {
        format!("sha256:{}", data_encoding::HEXLOWER.encode(&self.0))
    }

    /// As a person compares it with what `openssl x509 -noout -fingerprint -sha256` prints:
    /// upper-case pairs joined by colons.
    #[must_use]
    pub fn shown(&self) -> String {
        self.0
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }
}

/// **Where a TLS pool's client key comes from**: a file where it already is, or a vault entry
/// of a project's, written to disk for one run alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyFrom {
    File(PathBuf),
    Vault {
        /// The absolute root of the project whose vault holds it.
        plane: PathBuf,
        vault: String,
        key: String,
    },
}

/// **The files Docker's `--tlsverify` takes**, each where the person keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Files {
    pub ca: PathBuf,
    pub cert: PathBuf,
    pub key: KeyFrom,
}

impl Files {
    /// **The files as given**, or why not: each path absolute, and a vault entry's names words
    /// a vault and a key can be.
    pub fn new(ca: PathBuf, cert: PathBuf, key: KeyFrom) -> Result<Files, String> {
        for (what, path) in [("the CA", &ca), ("the client certificate", &cert)] {
            if !path.is_absolute() {
                return Err(format!(
                    "{what} is named as {}, which isn't an absolute path",
                    path.display()
                ));
            }
        }
        match &key {
            KeyFrom::File(path) if !path.is_absolute() => {
                return Err(format!(
                    "the client key is named as {}, which isn't an absolute path",
                    path.display()
                ));
            }
            KeyFrom::Vault { plane, vault, key } => {
                if !plane.is_absolute() {
                    return Err("the vault's project isn't named by an absolute path".to_owned());
                }
                if !a_name(vault) || !a_name(key) {
                    return Err(format!(
                        "{} isn't a vault entry purlis can read the client key from",
                        crate::shown::short(&format!("{vault}/{key}"))
                    ));
                }
            }
            KeyFrom::File(_) => {}
        }
        Ok(Files { ca, cert, key })
    }

    /// The client key, where it is named as a file; `None` where it is in a vault.
    #[must_use]
    pub fn key_file(&self) -> Option<&Path> {
        match &self.key {
            KeyFrom::File(path) => Some(path),
            KeyFrom::Vault { .. } => None,
        }
    }
}

/// A vault's or a key's name: nothing that could name a path or an option.
fn a_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('-')
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// **What a TLS pool is pinned to**: the CA the person confirmed, and the engine's id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub ca: Fingerprint,
    pub engine: String,
}

/// **A pool's engine over Docker's TLS API**: where it is, the files that reach it, and its pin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub route: Route,
    pub files: Files,
    pub pin: Pin,
}

/// **`text` with `files`, as Connect and `purlis pool add` take a route over TCP**, or why not:
/// plain `tcp://` (no files at all) and TLS that verifies no server (no CA) are refused with
/// ADR 0123 §2's words.
pub fn route_with(text: &str, files: Option<&Files>) -> Result<Route, String> {
    let route = Route::parse(text)?;
    match files {
        None => Err(crate::sshroute::PLAIN_TCP.to_owned()),
        Some(_) => Ok(route),
    }
}

/// **What each run of a TLS pool's `docker` is given to reach its engine**: the route, the files
/// and the pinned CA they are checked against first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Through {
    pub route: Route,
    pub files: Files,
    pub ca: Fingerprint,
    /// The engine's id it is pinned to, once the person trusted it; `None` while Connect is
    /// still meeting the machine, which signs nothing in.
    pub engine: Option<String>,
}

impl Through {
    /// What reaches `remote`'s engine.
    #[must_use]
    pub fn of(remote: &Remote) -> Through {
        Through {
            route: remote.route.clone(),
            files: remote.files.clone(),
            ca: remote.pin.ca,
            engine: Some(remote.pin.engine.clone()),
        }
    }

    /// **The files, read and checked, for one run** (ADR 0123 §3): the CA file's one certificate
    /// is the pinned one, the client certificate reads as one, and the key is read from its file
    /// or its vault. Refused, naming the file, where any isn't.
    pub fn material(&self) -> Result<Material, Finding> {
        let host = self.route.host();
        let ca = read_ca(&self.files.ca, host)?;
        if ca.fingerprint != self.ca {
            return Err(Finding::CaChanged {
                host: host.to_owned(),
                pinned: self.ca.shown(),
                now: ca.fingerprint.shown(),
                file: self.files.ca.display().to_string(),
            });
        }
        let cert = read_file(&self.files.cert, host)?;
        certificates_in(&cert, &self.files.cert, host)?;
        let key = read_key(&self.files.key, host)?;
        Ok(Material {
            ca: ca.pem,
            cert,
            key,
        })
    }
}

/// **The files one run is handed**, read and checked: `ca.pem`, `cert.pem` and `key.pem`.
pub struct Material {
    pub ca: Vec<u8>,
    pub cert: Vec<u8>,
    pub key: Zeroizing<Vec<u8>>,
}

/// The names Docker reads in `DOCKER_CERT_PATH`.
pub const CA_PEM: &str = "ca.pem";
pub const CERT_PEM: &str = "cert.pem";
pub const KEY_PEM: &str = "key.pem";

/// **A CA file read**: its bytes, its one certificate's DER and fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ca {
    pub pem: Vec<u8>,
    pub der: Vec<u8>,
    pub fingerprint: Fingerprint,
}

impl Ca {
    /// **Its subject**, as RFC 4514 spells it, to show beside its fingerprint at Connect. `None`
    /// where the certificate doesn't say one purlis can read.
    #[must_use]
    pub fn subject(&self) -> Option<String> {
        use x509_cert::der::Decode;
        x509_cert::Certificate::from_der(&self.der)
            .ok()
            .map(|cert| cert.tbs_certificate.subject.to_string())
            .filter(|subject| !subject.is_empty())
    }
}

/// **The CA file at `path`, read**: refused, naming it, where it can't be read or holds anything
/// but exactly one certificate.
pub fn read_ca(path: &Path, host: &str) -> Result<Ca, Finding> {
    let pem = read_file(path, host)?;
    let certificates = certificates_in(&pem, path, host)?;
    let [der] = certificates.as_slice() else {
        return Err(tls_file(
            host,
            path,
            "holds more than one certificate: name the file of the one CA Docker's certificates \
             are signed with",
        ));
    };
    Ok(Ca {
        fingerprint: Fingerprint::of_der(der),
        der: der.clone(),
        pem,
    })
}

fn tls_file(host: &str, path: &Path, why: &str) -> Finding {
    Finding::TlsFile {
        host: host.to_owned(),
        file: path.display().to_string(),
        why: why.to_owned(),
    }
}

/// The file at `path`, read up to [`MOST_READ`], where it is a regular file.
fn read_file(path: &Path, host: &str) -> Result<Vec<u8>, Finding> {
    let file = std::fs::File::open(path).map_err(|why| {
        tls_file(
            host,
            path,
            &match why.kind() {
                std::io::ErrorKind::NotFound => "isn't there".to_owned(),
                std::io::ErrorKind::PermissionDenied => "can't be read by your account".to_owned(),
                _ => format!("can't be read ({why})"),
            },
        )
    })?;
    if !file.metadata().is_ok_and(|meta| meta.is_file()) {
        return Err(tls_file(host, path, "isn't a file"));
    }
    let mut bytes = Vec::new();
    file.take(MOST_READ + 1)
        .read_to_end(&mut bytes)
        .map_err(|why| tls_file(host, path, &format!("can't be read ({why})")))?;
    if bytes.len() as u64 > MOST_READ {
        return Err(tls_file(host, path, "is larger than any certificate"));
    }
    Ok(bytes)
}

/// The certificates' DER in the PEM `bytes` of `path`: at least one, or refused.
fn certificates_in(bytes: &[u8], path: &Path, host: &str) -> Result<Vec<Vec<u8>>, Finding> {
    use rustls::pki_types::CertificateDer;
    use rustls::pki_types::pem::PemObject;
    let read: Result<Vec<CertificateDer<'static>>, _> =
        CertificateDer::pem_slice_iter(bytes).collect();
    match read {
        Ok(certificates) if !certificates.is_empty() => Ok(certificates
            .into_iter()
            .map(|der| der.as_ref().to_vec())
            .collect()),
        _ => Err(tls_file(host, path, "holds no certificate in PEM")),
    }
}

/// The client key from `from`, as PEM, checked to read as a private key.
fn read_key(from: &KeyFrom, host: &str) -> Result<Zeroizing<Vec<u8>>, Finding> {
    let (bytes, named) = match from {
        KeyFrom::File(path) => (
            Zeroizing::new(read_file(path, host)?),
            path.display().to_string(),
        ),
        KeyFrom::Vault { plane, vault, key } => (
            vault_key(plane, vault, key).map_err(|why| Finding::TlsFile {
                host: host.to_owned(),
                file: format!("{key} in vault {vault}"),
                why,
            })?,
            format!("{key} in vault {vault}"),
        ),
    };
    if private_key_of(&bytes).is_none() {
        return Err(Finding::TlsFile {
            host: host.to_owned(),
            file: named,
            why: "holds no private key in PEM".to_owned(),
        });
    }
    Ok(bytes)
}

fn private_key_of(pem: &[u8]) -> Option<rustls::pki_types::PrivateKeyDer<'static>> {
    use rustls::pki_types::pem::PemObject;
    rustls::pki_types::PrivateKeyDer::from_pem_slice(pem).ok()
}

/// Each vault key read lately, by `(plane, vault, key)`: its value and when it was read.
type KeptKeys =
    std::collections::BTreeMap<(PathBuf, String, String), (Zeroizing<Vec<u8>>, Instant)>;
static KEPT: std::sync::Mutex<KeptKeys> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// **The value of `key` in `vault` of the project at `plane`**, held in memory for
/// [`VAULT_KEY_KEPT_FOR`] so a link's several runs read the vault once, never on disk here.
fn vault_key(plane: &Path, vault: &str, key: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    let at = (plane.to_path_buf(), vault.to_owned(), key.to_owned());
    {
        let kept = KEPT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((value, read)) = kept.get(&at)
            && read.elapsed() < VAULT_KEY_KEPT_FOR
        {
            return Ok(value.clone());
        }
    }
    vault_key_in(plane, vault, key, crate::secrets::Env::from_process())
}

/// [`vault_key`], with the project's vaults read as `env` says where they are.
pub(crate) fn vault_key_in(
    plane: &Path,
    vault: &str,
    key: &str,
    env: crate::secrets::Env,
) -> Result<Zeroizing<Vec<u8>>, String> {
    let at = (plane.to_path_buf(), vault.to_owned(), key.to_owned());
    let ctx = crate::secrets::Ctx::new(plane, env);
    let provider = crate::secrets::cmd::provider(&ctx, vault)
        .map_err(|why| format!("can't be read: {}", why.message))?;
    let value = Zeroizing::new(
        crate::secrets::cmd::get_value(&ctx, &provider, key)
            .map_err(|why| format!("can't be read: {}", why.message))?,
    );
    let value = Zeroizing::new(value.as_bytes().to_vec());
    KEPT.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(at, (value.clone(), Instant::now()));
    Ok(value)
}

/// **Every vault key held in memory is let go of**: the person pressed Check again, or connected
/// the pool again.
pub fn forget_vault_keys() {
    KEPT.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

/// **Whether purlis can hold `files` from every chat** (#6267), as Connect asks before it trusts
/// them: each chat's sandbox denies the files a pool names, reading included, by their paths. A
/// file with another name on disk (a hard link) is reached by that name, which no rule holds; a
/// file in a folder chats write (`chats_write`, a temporary folder or a project) can be moved
/// away with that folder and read from where it went. Either is refused, naming the file; a key
/// is pointed to a vault.
pub fn held_from_chats(files: &Files, chats_write: &[PathBuf], host: &str) -> Result<(), Finding> {
    let written: Vec<PathBuf> = chats_write.iter().map(|folder| real_of(folder)).collect();
    let named = [(files.ca.as_path(), false), (files.cert.as_path(), false)]
        .into_iter()
        .chain(files.key_file().map(|key| (key, true)));
    for (file, is_key) in named {
        let instead = if is_key {
            "keep the key in a vault instead (--tls-key-in VAULT/KEY), which purlis hands each run \
             alone"
        } else {
            "move it to a folder only you write, such as ~/.docker, and name it there"
        };
        let refused = |why: &str| {
            Err(Finding::TlsFile {
                host: host.to_owned(),
                file: file.display().to_string(),
                why: format!("{why}, so purlis can't keep chats from reading it: {instead}"),
            })
        };
        if links_to(file) > 1 {
            return refused("has another name on disk (a hard link)");
        }
        let real = real_of(file);
        if written.iter().any(|folder| real.starts_with(folder)) {
            return refused("is in a folder chats write");
        }
    }
    Ok(())
}

/// **The folders chats write, for [`held_from_chats`]**: the temporary folders and every project
/// this machine, its store under `config_root`, has opened.
#[must_use]
pub fn folders_chats_write(config_root: &Path) -> Vec<PathBuf> {
    let mut folders = vec![
        std::env::temp_dir(),
        PathBuf::from("/tmp"),
        PathBuf::from("/var/tmp"),
    ];
    folders.extend(
        crate::machine::read(config_root)
            .store
            .recents
            .into_iter()
            .map(|recent| recent.plane),
    );
    folders
}

/// How many names the file at `path` has on disk; 1 where it can't be read.
fn links_to(path: &Path) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path).map_or(1, |meta| meta.nlink())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        1
    }
}

/// `path` with every link on the way resolved, where it is there.
fn real_of(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// **The vault key `from` names is let go of**, where this process holds it: its pool is gone.
pub fn forget_vault_key(from: &KeyFrom) {
    if let KeyFrom::Vault { plane, vault, key } = from {
        KEPT.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&(plane.clone(), vault.clone(), key.clone()));
    }
}

/// Whether this process holds the vault key `from` names.
#[cfg(test)]
pub(crate) fn kept_in_memory(from: &KeyFrom) -> bool {
    let KeyFrom::Vault { plane, vault, key } = from else {
        return false;
    };
    KEPT.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains_key(&(plane.clone(), vault.clone(), key.clone()))
}

/// **What the engine at `route` says to a TLS handshake of purlis's own** (ADR 0123 §7), with
/// `material`'s CA as the only root and its client certificate, then Docker's `/_ping` asked
/// over it: `Ok` where the server verifies and accepts the client, else the finding that names
/// which check failed and the file to check. Nothing the server prints decides it but the
/// handshake's own typed errors and the HTTP status of the ping.
pub fn handshake(route: &Route, files: &Files, material: &Material) -> Result<(), Finding> {
    let config = client_config(route.host(), files, material)?;
    let mut tcp = connect_tcp(route)?;
    handshake_over(&mut tcp, route, files, config)
}

/// [`handshake`] over `tcp`, a connection to the engine already open.
fn handshake_over(
    tcp: &mut (impl Read + Write),
    route: &Route,
    files: &Files,
    config: rustls::ClientConfig,
) -> Result<(), Finding> {
    let host = route.host().to_owned();
    let ca_file = files.ca.display().to_string();
    let cert_file = files.cert.display().to_string();
    let name = rustls::pki_types::ServerName::try_from(route.host().to_owned())
        .map_err(|_| Finding::NoSuchName { host: host.clone() })?;
    let mut tls = rustls::ClientConnection::new(Arc::new(config), name)
        .map_err(|why| Finding::Here(format!("purlis couldn't start TLS to {host}: {why}")))?;
    let mut stream = rustls::Stream::new(&mut tls, tcp);
    let ping = format!(
        "GET /_ping HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        route.host()
    );
    let mut answer = Vec::new();
    let went = stream
        .write_all(ping.as_bytes())
        .and_then(|()| stream.flush())
        .and_then(|()| (&mut stream).take(4096).read_to_end(&mut answer).map(drop));
    if let Err(why) = went {
        // A peer that closes once it answered is a finished answer, not a failure.
        let closed = why.kind() == std::io::ErrorKind::UnexpectedEof && !answer.is_empty();
        if !closed {
            return Err(finding_of(&why, &host, &ca_file, &cert_file));
        }
    }
    let status_ok = answer.starts_with(b"HTTP/1.1 200") || answer.starts_with(b"HTTP/1.0 200");
    if status_ok {
        Ok(())
    } else {
        Err(Finding::EngineSilent {
            host,
            said: String::from_utf8_lossy(answer.split(|b| *b == b'\n').next().unwrap_or(&[]))
                .trim()
                .to_owned(),
        })
    }
}

fn client_config(
    host: &str,
    files: &Files,
    material: &Material,
) -> Result<rustls::ClientConfig, Finding> {
    use rustls::pki_types::CertificateDer;
    use rustls::pki_types::pem::PemObject;
    let mut roots = rustls::RootCertStore::empty();
    for der in certificates_in(&material.ca, &files.ca, host)? {
        roots.add(CertificateDer::from(der)).map_err(|why| {
            tls_file(
                host,
                &files.ca,
                &format!("isn't a CA purlis can use ({why})"),
            )
        })?;
    }
    let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(&material.cert)
        .collect::<Result<_, _>>()
        .map_err(|_| tls_file(host, &files.cert, "holds no certificate in PEM"))?;
    let key = private_key_of(&material.key).ok_or_else(|| Finding::TlsFile {
        host: host.to_owned(),
        file: key_named(files),
        why: "holds no private key in PEM".to_owned(),
    })?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|why| Finding::Here(format!("purlis couldn't set up TLS: {why}")))?
        .with_root_certificates(roots)
        .with_client_auth_cert(chain, key)
        .map_err(|why| Finding::TlsFile {
            host: host.to_owned(),
            file: key_named(files),
            why: format!(
                "isn't the key of the client certificate {} ({why})",
                files.cert.display()
            ),
        })
}

/// The client key as a sentence names it.
fn key_named(files: &Files) -> String {
    match &files.key {
        KeyFrom::File(path) => path.display().to_string(),
        KeyFrom::Vault { vault, key, .. } => format!("{key} in vault {vault}"),
    }
}

fn connect_tcp(route: &Route) -> Result<std::net::TcpStream, Finding> {
    use std::net::ToSocketAddrs;
    let host = route.host().to_owned();
    let addresses: Vec<_> = (route.host(), route.port())
        .to_socket_addrs()
        .map_err(|_| Finding::NoSuchName { host: host.clone() })?
        .collect();
    let stream = addresses
        .iter()
        .find_map(|at| std::net::TcpStream::connect_timeout(at, HANDSHAKE_WITHIN).ok())
        .ok_or_else(|| {
            if addresses.is_empty() {
                Finding::NoSuchName { host: host.clone() }
            } else {
                Finding::NoAnswer {
                    host: host.clone(),
                    port: route.port(),
                }
            }
        })?;
    let _ = stream.set_read_timeout(Some(HANDSHAKE_WITHIN));
    let _ = stream.set_write_timeout(Some(HANDSHAKE_WITHIN));
    Ok(stream)
}

/// **The finding a failed handshake or ping says**, from rustls's typed error inside `why`.
#[must_use]
pub fn finding_of(why: &std::io::Error, host: &str, ca: &str, cert: &str) -> Finding {
    let inner = why
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<rustls::Error>());
    match inner {
        Some(error) => finding_of_tls(error, host, ca, cert),
        None => Finding::EngineSilent {
            host: host.to_owned(),
            said: why.to_string(),
        },
    }
}

/// [`finding_of`], from rustls's error itself.
#[must_use]
pub fn finding_of_tls(error: &rustls::Error, host: &str, ca: &str, cert: &str) -> Finding {
    use rustls::{AlertDescription as Alert, CertificateError as Cert, Error};
    let host = host.to_owned();
    match error {
        Error::InvalidCertificate(Cert::Expired | Cert::ExpiredContext { .. }) => {
            Finding::ServerExpired { host }
        }
        Error::InvalidCertificate(Cert::NotValidForName) => Finding::WrongName {
            host,
            name: String::new(),
        },
        Error::InvalidCertificate(Cert::NotValidForNameContext { presented, .. }) => {
            Finding::WrongName {
                host,
                name: presented.join(", "),
            }
        }
        Error::InvalidCertificate(Cert::UnknownIssuer | Cert::BadSignature) => {
            Finding::NotSignedByCa {
                host,
                ca: ca.to_owned(),
            }
        }
        Error::AlertReceived(Alert::CertificateExpired) => Finding::ClientExpired {
            host,
            cert: cert.to_owned(),
        },
        Error::AlertReceived(
            Alert::BadCertificate
            | Alert::UnknownCA
            | Alert::CertificateUnknown
            | Alert::CertificateRequired
            | Alert::AccessDenied
            | Alert::UnsupportedCertificate
            | Alert::CertificateRevoked,
        ) => Finding::ClientRefused {
            host,
            cert: cert.to_owned(),
        },
        other => Finding::EngineSilent {
            host,
            said: other.to_string(),
        },
    }
}

/// **Whether `pin`'s engine is the one `docker` reaches over TLS**: the check made before any
/// runner is made, checked or linked, as for SSH (ADR 0123 §3).
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
        Ok(())
    } else {
        Err(Finding::EngineChanged {
            host: route.host().to_owned(),
            pinned: pin.engine.clone(),
            now,
            // Docker's API over TLS is Docker's: a Podman is reached over SSH alone (#6269).
            podman: false,
        })
    }
}

pub mod pool;

#[cfg(test)]
mod tests;
