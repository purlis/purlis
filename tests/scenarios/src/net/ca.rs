// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The test certificate authority, and the bundle the suite's clients trust.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DnType, GeneralSubtree, IsCa, KeyPair,
    KeyUsagePurpose, NameConstraints,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// **The variables a client reads its trusted roots from**, each set to the one bundle
/// ([`TestCa::bundle`]): OpenSSL and Go on Linux, curl, Python's `requests`, and Node. Only the
/// scenario host sets them, and inside it they replace the `SSL_CERT_FILE` purlis sets for a
/// Codex chat (`purlis_core::sandbox::codex::ROOTS_ENV`): the host applies [`ca_env`] last.
///
/// **Go on macOS ignores `SSL_CERT_FILE`**: it verifies through the Security framework (Go's
/// `root_unix.go`, which reads the variable, is not built for darwin). Confirmed by one run on
/// 2026-11-16, go1.25.5 darwin/arm64: a leaf from a CA named by `SSL_CERT_FILE` was "not trusted"
/// by the system verifier, and verified against a pool read from the same file. So a Go TLS cell
/// runs `per_os: linux`, or reaches the plain-HTTP origin on macOS. Chrome reads none of them,
/// so a browser cell reaches the plain-HTTP origin.
pub const CA_VARIABLES: [&str; 4] = [
    "SSL_CERT_FILE",
    "CURL_CA_BUNDLE",
    "REQUESTS_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
];

/// Each of [`CA_VARIABLES`] set to `bundle`, for the scenario host to apply over a chat's
/// environment, last.
pub fn ca_env(bundle: &Path) -> Vec<(&'static str, OsString)> {
    CA_VARIABLES
        .iter()
        .map(|name| (*name, bundle.as_os_str().to_owned()))
        .collect()
}

/// **A test certificate authority**, new for each run: its key never leaves the process, and
/// it signs only `.test` names (an X.509 name constraint), so even a bundle that outlived its run
/// vouches for no real site.
pub struct TestCa {
    issuer: CertifiedIssuer<'static, KeyPair>,
}

impl std::fmt::Debug for TestCa {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestCa").finish_non_exhaustive()
    }
}

fn failed(err: rcgen::Error) -> io::Error {
    io::Error::other(format!("the test CA could not sign: {err}"))
}

impl TestCa {
    /// A new authority.
    pub fn new() -> io::Result<Self> {
        let mut params = CertificateParams::new(Vec::<String>::new()).map_err(failed)?;
        params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        params
            .distinguished_name
            .push(DnType::CommonName, "purlis scenario test CA");
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        params.name_constraints = Some(NameConstraints {
            permitted_subtrees: vec![GeneralSubtree::DnsName("test".into())],
            excluded_subtrees: Vec::new(),
        });
        let key = KeyPair::generate().map_err(failed)?;
        let issuer = CertifiedIssuer::self_signed(params, key).map_err(failed)?;
        Ok(Self { issuer })
    }

    /// The authority's certificate, PEM.
    pub fn pem(&self) -> String {
        self.issuer.pem()
    }

    /// The authority's certificate, DER: what a test's own TLS client trusts.
    pub fn der(&self) -> CertificateDer<'static> {
        self.issuer.der().clone()
    }

    /// A server certificate for `names` (each a `.test` name), and its key.
    pub fn leaf(
        &self,
        names: &[&str],
    ) -> io::Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
        let names: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
        let mut params = CertificateParams::new(names).map_err(failed)?;
        params
            .distinguished_name
            .push(DnType::CommonName, "purlis scenario origin");
        let key = KeyPair::generate().map_err(failed)?;
        let cert = params.signed_by(&key, &self.issuer).map_err(failed)?;
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
        Ok((vec![cert.der().clone(), self.der()], key))
    }

    /// **Writes the one bundle the suite's clients trust** into `dir`: this system's roots,
    /// then this authority. Its path is what [`ca_env`] sets.
    pub fn bundle(&self, dir: &Path) -> io::Result<PathBuf> {
        let found = rustls_native_certs::load_native_certs();
        if found.certs.is_empty() {
            return Err(io::Error::other(format!(
                "no system root certificate could be read: {:?}",
                found.errors
            )));
        }
        let mut out = String::new();
        for cert in &found.certs {
            out.push_str(&pem::encode(&pem::Pem::new("CERTIFICATE", cert.to_vec())));
        }
        out.push_str(&self.pem());
        let path = dir.join("scenario-ca-bundle.pem");
        std::fs::write(&path, out)?;
        Ok(path)
    }
}
