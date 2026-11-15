// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The trust store an upstream's certificate is verified against** (R-056, R-700): the
//! operating system's own, through `rustls-platform-verifier`, as the forge clients' `ureq`
//! verifies. The seam: with `test-support`, a second constructor adds one test root to it, so
//! the fake provider's certificate for a real origin's name verifies; nothing else can.

use std::sync::Arc;

use rustls::ClientConfig;
use rustls::client::danger::ServerCertVerifier;
use rustls::crypto::CryptoProvider;
use rustls_platform_verifier::Verifier;

/// A TLS client that verifies against the operating system's trust store.
pub fn client_config() -> Result<ClientConfig, rustls::Error> {
    let provider = provider();
    let verifier = Verifier::new(Arc::clone(&provider))?;
    configured(provider, Arc::new(verifier))
}

/// A TLS client that verifies against the operating system's trust store and `root` beside it.
#[cfg(feature = "test-support")]
pub fn trusting_test_root(
    root: rustls::pki_types::CertificateDer<'static>,
) -> Result<ClientConfig, rustls::Error> {
    let provider = provider();
    let verifier = Verifier::new_with_extra_roots([root], Arc::clone(&provider))?;
    configured(provider, Arc::new(verifier))
}

/// The `ring` provider rustls is built with here.
fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn configured(
    provider: Arc<CryptoProvider>,
    verifier: Arc<dyn ServerCertVerifier>,
) -> Result<ClientConfig, rustls::Error> {
    Ok(ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth())
}
