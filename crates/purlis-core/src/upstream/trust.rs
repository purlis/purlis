// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The trust store an upstream's certificate is verified against** (R-056, R-700): the
//! operating system's own, through `rustls-platform-verifier`, as the forge clients' `ureq`
//! verifies. The seam: with `test-support`, [`trusting_test_root`] adds one test root to it, so
//! the fake provider's certificate for a real origin's name verifies; nothing else can.

use std::sync::Arc;

use rustls::ClientConfig;

/// A TLS client that verifies against the operating system's trust store.
pub fn client_config() -> Result<ClientConfig, rustls::Error> {
    Err(rustls::Error::General("stub".into()))
}

/// A TLS client that verifies against the operating system's trust store and `root` beside it.
#[cfg(feature = "test-support")]
pub fn trusting_test_root(
    root: rustls::pki_types::CertificateDer<'static>,
) -> Result<ClientConfig, rustls::Error> {
    let _ = (root, Arc::<()>::default);
    Err(rustls::Error::General("stub".into()))
}
