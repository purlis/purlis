// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one native transport a GitHub App's mint sends on** (#2053): to github.com's API alone,
//! each request carrying the App's JWT as its bearer, or, to revoke a minted token (#3808), that
//! token itself, and no sign-in token. Kept in a file of its
//! own, so the guard that lets only the account resolver build a native transport
//! (`tests/it/only_the_resolver_builds_the_native_transport.rs`) names this exception alone.

use std::sync::Arc;

use secrecy::SecretString;

use super::HOST;
use crate::forge::ForgeError;
use crate::forge::Kind;
use crate::forge::http::{ApiRoot, Http, TokenSource};

/// A JWT, or the token being revoked, handed out as the bearer of each request.
struct Bearer(SecretString);

impl TokenSource for Bearer {
    fn token(&self) -> Result<SecretString, ForgeError> {
        Ok(self.0.clone())
    }
}

/// The native transport to github.com's API, its requests sent with `bearer`: an App's JWT,
/// or the installation token a revoke is sent with.
pub fn to_github_com(bearer: &str) -> Result<Http, String> {
    Http::new(
        Kind::GitHub,
        ApiRoot::github(HOST),
        Arc::new(Bearer(SecretString::from(bearer))),
    )
    .map_err(|e| e.to_string())
}
