// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's own certificate authority (ADR 0128 D3a, D3c).
//!
//! `git` and `glab` reach a forge through `CONNECT`, and the proxy reads what they send by
//! terminating TLS for the forge's host. For that the chat's client must trust a certificate the
//! proxy made, and this is the least that can be:
//!
//! - **one per chat**, made when the listener starts, **its private key in memory only** and never
//!   written anywhere; only the certificate is handed out (the file the chat's client is pointed
//!   at is the caller's to write);
//! - with a **critical `nameConstraints`** that permits exactly the hosts [`ChatPolicy::terminable_hosts`]
//!   names (the chat's recorded forge hosts and their API hosts) and **excludes every IP address**,
//!   so a client that honours constraints refuses anything else the key might sign, and a path
//!   length of zero so it signs no other authority;
//! - **leaf certificates only for those hosts**: [`ChatCa::leaf_for`] is `None` for any other name.
//!
//! It is named in the tool's own variable (`http.<host>.sslCAInfo`, `GLAB_CA_CERT`), never added to
//! the system's or the keychain's trust.
//!
//! [`ChatPolicy::terminable_hosts`]: super::super::ChatPolicy::terminable_hosts

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use rcgen::{
    BasicConstraints, CertificateParams, CidrSubnet, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, GeneralSubtree, IsCa, Issuer, KeyPair, KeyUsagePurpose,
    NameConstraints, date_time_ymd,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;

/// How long a chat's authority and its leaves are valid, in days. A chat that outlives it starts
/// again (its listener, and so its authority, is made at every start and resume).
const VALID_DAYS: i64 = 30;

/// Why an authority or a leaf could not be made.
#[derive(Debug)]
pub struct CaError(String);

impl std::fmt::Display for CaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CaError {}

fn failed(what: &str, error: impl std::fmt::Display) -> CaError {
    CaError(format!("{what}: {error}"))
}

/// One chat's authority.
pub struct ChatCa {
    hosts: Vec<String>,
    issuer: Issuer<'static, KeyPair>,
    leaf_key: KeyPair,
    ca_pem: String,
    leaves: Mutex<HashMap<String, Arc<CertifiedKey>>>,
}

impl std::fmt::Debug for ChatCa {
    /// Names only: the keys are never printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatCa")
            .field("hosts", &self.hosts)
            .finish()
    }
}

impl ChatCa {
    /// Make an authority that may vouch for `hosts` and for nothing else.
    pub fn new(hosts: &[String]) -> Result<Self, CaError> {
        if hosts.is_empty() {
            return Err(CaError("a chat with no forge host has no authority".into()));
        }
        let key = KeyPair::generate().map_err(|e| failed("a key for the authority", e))?;
        let mut params =
            CertificateParams::new(Vec::<String>::new()).map_err(|e| failed("the authority", e))?;
        params.distinguished_name = common_name("purlis forge proxy (this chat only)");
        params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        set_validity(&mut params)?;
        // rcgen writes `nameConstraints` as a critical extension. A constraint on a name type
        // that is absent permits every name of it, so the one type a leaf could be made for
        // besides DNS names (an address) is excluded outright.
        params.name_constraints = Some(NameConstraints {
            permitted_subtrees: hosts
                .iter()
                .map(|host| GeneralSubtree::DnsName(host.clone()))
                .collect(),
            excluded_subtrees: vec![
                GeneralSubtree::IpAddress(CidrSubnet::from_v4_prefix([0; 4], 0)),
                GeneralSubtree::IpAddress(CidrSubnet::from_v6_prefix([0; 16], 0)),
            ],
        });
        let certificate = params
            .self_signed(&key)
            .map_err(|e| failed("the authority's certificate", e))?;
        let ca_pem = certificate.pem();
        let issuer = Issuer::new(params, key);
        Ok(Self {
            hosts: hosts.to_vec(),
            issuer,
            leaf_key: KeyPair::generate().map_err(|e| failed("a key for the leaves", e))?,
            ca_pem,
            leaves: Mutex::new(HashMap::new()),
        })
    }

    /// The authority's certificate, PEM: the only part of it that leaves memory.
    pub fn certificate_pem(&self) -> &str {
        &self.ca_pem
    }

    /// The hosts it may vouch for.
    pub fn hosts(&self) -> &[String] {
        &self.hosts
    }

    /// A certificate for `host`, made once and kept, or `None` when `host` is not one of the
    /// hosts this authority was made for (to the letter, never by suffix).
    pub fn leaf_for(&self, host: &str) -> Option<Arc<CertifiedKey>> {
        let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
        if !self.hosts.contains(&host) {
            return None;
        }
        let mut leaves = self.leaves.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(made) = leaves.get(&host) {
            return Some(Arc::clone(made));
        }
        let made = Arc::new(self.make_leaf(&host).ok()?);
        leaves.insert(host, Arc::clone(&made));
        Some(made)
    }

    fn make_leaf(&self, host: &str) -> Result<CertifiedKey, CaError> {
        let mut params = CertificateParams::new(vec![host.to_string()])
            .map_err(|e| failed("a leaf certificate", e))?;
        params.distinguished_name = common_name(host);
        params.is_ca = IsCa::NoCa;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        set_validity(&mut params)?;
        let certificate = params
            .signed_by(&self.leaf_key, &self.issuer)
            .map_err(|e| failed("a leaf certificate", e))?;
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.leaf_key.serialize_der()));
        let signing = rustls::crypto::ring::sign::any_supported_type(&key)
            .map_err(|e| failed("the leaf's signing key", e))?;
        Ok(CertifiedKey::new(
            vec![CertificateDer::from(certificate.der().to_vec())],
            signing,
        ))
    }

    /// The TLS server settings for a connection that asked for `host`: HTTP/1.1 only, and a
    /// certificate for that host and no other. A client whose server name (SNI) is not `host`
    /// gets no certificate, so no handshake: the name it asked the proxy for and the name it
    /// asks the server for are one.
    pub fn server_config_for(&self, host: &str) -> Option<Arc<rustls::ServerConfig>> {
        let leaf = self.leaf_for(host)?;
        let resolver = OneHost {
            host: host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase(),
            leaf,
        };
        let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .ok()?
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(resolver));
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Some(Arc::new(config))
    }
}

/// Serves one host's leaf to a client that names that host.
#[derive(Debug)]
struct OneHost {
    host: String,
    leaf: Arc<CertifiedKey>,
}

impl ResolvesServerCert for OneHost {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let asked = hello.server_name()?;
        let asked = asked.strip_suffix('.').unwrap_or(asked);
        asked
            .eq_ignore_ascii_case(&self.host)
            .then(|| Arc::clone(&self.leaf))
    }
}

fn common_name(name: &str) -> DistinguishedName {
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, name);
    dn
}

/// From two days ago (clocks differ) to [`VALID_DAYS`] ahead, in whole days.
fn set_validity(params: &mut CertificateParams) -> Result<(), CaError> {
    let now = jiff::Timestamp::now().to_zoned(jiff::tz::TimeZone::UTC);
    let day = |zoned: &jiff::Zoned| -> Result<(i32, u8, u8), CaError> {
        let date = zoned.date();
        Ok((
            i32::from(date.year()),
            u8::try_from(date.month()).map_err(|e| failed("a date", e))?,
            u8::try_from(date.day()).map_err(|e| failed("a date", e))?,
        ))
    };
    let before = now
        .checked_sub(jiff::Span::new().days(2))
        .map_err(|e| failed("a date", e))?;
    let after = now
        .checked_add(jiff::Span::new().days(VALID_DAYS))
        .map_err(|e| failed("a date", e))?;
    let (y, m, d) = day(&before)?;
    params.not_before = date_time_ymd(y, m, d);
    let (y, m, d) = day(&after)?;
    params.not_after = date_time_ymd(y, m, d);
    Ok(())
}
