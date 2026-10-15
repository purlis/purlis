// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The only addresses a browser opens: http and https.

use std::fmt;

/// An address a browser may be sent to: `http://` or `https://` with a host, as written.
///
/// The engine's navigation takes this type and nothing else, so an address of another scheme
/// (`file:`, `chrome:`, `javascript:`, `data:`, an app's own scheme) never reaches the engine.
/// The check is deliberately narrow and refuses what it cannot read plainly: whitespace,
/// control characters and backslashes are refused rather than cleaned, because the engine
/// strips or rewrites each of them, and then what it opens is not what was checked.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct WebAddress(String);

/// Why an address is not one a browser opens.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotWebAddress {
    /// The address names a scheme other than http or https.
    #[error("a browser opens only http and https addresses, and this one is `{0}`")]
    Scheme(String),
    /// The address has no scheme at all.
    #[error("a browser opens only http and https addresses, and this one names no scheme")]
    NoScheme,
    /// `http://` or `https://` with nothing after it.
    #[error("the address names no host")]
    NoHost,
    /// A name or password before the host, which hides which host is meant.
    #[error("the address carries a name or password before its host")]
    UserInfo,
    /// Whitespace, a control character or a backslash, which the engine would rewrite.
    #[error("the address carries a space, a control character or a backslash")]
    Unreadable,
}

impl WebAddress {
    /// Accepts `address` when it is an http or https address with a host, unchanged.
    pub fn parse(address: &str) -> Result<Self, NotWebAddress> {
        if address
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
        {
            return Err(NotWebAddress::Unreadable);
        }
        let Some((scheme, rest)) = address.split_once(':') else {
            return Err(NotWebAddress::NoScheme);
        };
        if scheme.is_empty() {
            return Err(NotWebAddress::NoScheme);
        }
        let lowered = scheme.to_ascii_lowercase();
        if lowered != "http" && lowered != "https" {
            return Err(NotWebAddress::Scheme(scheme.to_owned()));
        }
        let Some(after) = rest.strip_prefix("//") else {
            return Err(NotWebAddress::NoHost);
        };
        let authority = after.split(['/', '?', '#']).next().unwrap_or_default();
        if authority.contains('@') {
            return Err(NotWebAddress::UserInfo);
        }
        let host = authority;
        if host.is_empty() || host.starts_with(':') {
            return Err(NotWebAddress::NoHost);
        }
        Ok(Self(address.to_owned()))
    }

    /// The address, exactly as it was accepted.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for WebAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("WebAddress").field(&self.0).finish()
    }
}

impl fmt::Display for WebAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
