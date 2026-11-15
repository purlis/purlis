// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which chats the key proxy carries**: the route resolver (design §3.1, §3.7, §3.8).

use crate::harness_declaration::Declaration;
use crate::model_provider::Format;
pub use crate::state::run::Level;

/// How a chat's profile reaches its model, as the resolver is asked it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// A key from your keys.
    YourKey,
    /// A subscription.
    Subscription,
    /// A key the harness stores itself.
    HarnessStoredKey,
    /// A key passed through the person's own settings.
    OwnSettingsKey,
}

/// What the resolver answers for a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Through the key proxy.
    Proxied { format: Format },
    /// The harness's own sign-in.
    OwnSignIn,
}

impl Route {
    pub fn starts_a_listener(self) -> bool {
        true
    }

    pub fn base_url_format(self) -> Option<Format> {
        Some(Format::AnthropicMessages)
    }
}

/// The route a chat on `declaration`, reaching its model `way`, at `level`, takes.
pub fn resolve(_declaration: &Declaration, _way: Way, _level: Level) -> Result<Route, String> {
    Ok(Route::Proxied {
        format: Format::AnthropicMessages,
    })
}

#[cfg(test)]
mod tests;
