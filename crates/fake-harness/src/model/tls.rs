// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The test root certificate the fake provider serves TLS under.

use std::sync::Arc;

use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair, KeyUsagePurpose};
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// **A test root and a certificate it signed** for the names given, `localhost` and
/// `127.0.0.1` always among them. Generated fresh each time: nothing trusts it but a client
/// handed [`TestRoot::certificate`].
#[derive(Debug, Clone)]
pub struct TestRoot {
    root: CertificateDer<'static>,
    server: Arc<ServerConfig>,
}

impl TestRoot {
    /// A new root, and a server certificate for `names` signed by it.
    ///
    /// # Panics
    /// When the certificates cannot be made, which is a fault of this test seam, not of a test.
    pub fn generate(names: &[&str]) -> TestRoot {
        let root_key = KeyPair::generate().expect("a root key");
        let mut root = CertificateParams::new(Vec::<String>::new()).expect("root parameters");
        root.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        root.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        root.distinguished_name
            .push(rcgen::DnType::CommonName, "purlis fake provider test root");
        let root_certificate = root.self_signed(&root_key).expect("a root certificate");
        let issuer = Issuer::new(root, root_key);

        let mut all: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
        all.extend(["localhost".to_owned(), "127.0.0.1".to_owned()]);
        let leaf_key = KeyPair::generate().expect("a server key");
        let leaf = CertificateParams::new(all)
            .expect("server parameters")
            .signed_by(&leaf_key, &issuer)
            .expect("a server certificate");

        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der()));
        let server =
            ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .expect("TLS versions")
                .with_no_client_auth()
                .with_single_cert(vec![leaf.der().clone()], key)
                .expect("the server certificate fits its key");
        TestRoot {
            root: root_certificate.der().clone(),
            server: Arc::new(server),
        }
    }

    /// The root, for a client's trust store.
    pub fn certificate(&self) -> CertificateDer<'static> {
        self.root.clone()
    }

    /// The server side, for the provider.
    pub(crate) fn server_config(&self) -> Arc<ServerConfig> {
        Arc::clone(&self.server)
    }
}
