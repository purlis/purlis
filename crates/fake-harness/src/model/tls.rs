// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The test root certificate the fake provider serves TLS under.

use std::sync::Arc;

use rustls::ServerConfig;
use rustls::pki_types::CertificateDer;

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
    pub fn generate(names: &[&str]) -> TestRoot {
        let _ = names;
        unimplemented!("stub")
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
