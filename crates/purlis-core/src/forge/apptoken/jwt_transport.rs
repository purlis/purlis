// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one native transport a GitHub App's mint sends on** (#2053): to github.com's API alone,
//! each request carrying the App's JWT as its bearer, and no sign-in token. Kept in a file of its
//! own, so the guard that lets only the account resolver build a native transport
//! (`tests/it/only_the_resolver_builds_the_native_transport.rs`) names this exception alone.

use std::sync::Arc;

use secrecy::SecretString;

use super::HOST;
use crate::forge::ForgeError;
use crate::forge::Kind;
use crate::forge::http::{ApiRoot, Http, TokenSource};

/// A JWT, handed out as the bearer of each request of one mint.
struct Bearer(SecretString);

impl TokenSource for Bearer {
    fn token(&self) -> Result<SecretString, ForgeError> {
        Ok(self.0.clone())
    }
}

/// The native transport to github.com's API, its requests sent with `jwt`.
pub fn to_github_com(jwt: &str) -> Result<Http, String> {
    Http::new(
        Kind::GitHub,
        ApiRoot::github(HOST),
        Arc::new(Bearer(SecretString::from(jwt))),
    )
    .map_err(|e| e.to_string())
}
