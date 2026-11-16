// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which chats the key proxy carries**: the route resolver (design §3.1, §3.7, §3.8).
//!
//! Every route purlis holds the credential for goes through the key proxy, and nothing else
//! does. A chat goes through it only when all of these hold:
//!
//! - **The key is one purlis holds**, from your keys. A subscription, a key the harness stores
//!   itself and a key passed through the person's own settings never do (R-036): each is the
//!   harness's own sign-in, purlis starts no listener for it and sets no base URL, and its
//!   sandbox keeps the `model-providers` preset (R-038). A subscription in particular is used
//!   only through the harness's own sign-in, talking to its own vendor, and never carried to
//!   another harness (R-260).
//! - **The harness takes a base URL**: its built-in declaration states the wire formats it
//!   speaks (`[route] formats`, R-227). One that states none takes its own sign-in only, and no
//!   key is set in its environment instead (R-257): the environment path is dropped. Only a
//!   built-in may state one (`harness_declaration::parse`, H-19).
//! - **The chat runs at level 1 or 2** (R-225): level 3 (opencode's ACP) takes no proxied route
//!   until it is measured.
//!
//! The variables a route sets, the strip floor and the pin are the adapter's, never this
//! module's or a declaration's (R-222).

use crate::harness_declaration::Declaration;
use crate::model_provider::Format;
pub use crate::state::run::Level;

/// How a chat's profile reaches its model, as the resolver is asked it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// A key from your keys: purlis holds it, and the key proxy swaps it in.
    YourKey,
    /// A subscription, signed in through the harness's own flow.
    Subscription,
    /// A key the harness stores itself, through its own sign-in.
    HarnessStoredKey,
    /// A key passed through the person's own settings (`[chat_env] pass`, a profile's `env`).
    OwnSettingsKey,
}

/// What the resolver answers for a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Through the key proxy: a listener is started for the run, and the harness is pointed at
    /// it in `format`, the first its declaration states.
    Proxied { format: Format },
    /// The harness's own sign-in, with purlis never on the path: no listener, no base URL, and
    /// the `model-providers` preset in its sandbox.
    OwnSignIn,
}

impl Route {
    /// Whether the run starts a key-proxy listener.
    pub fn starts_a_listener(self) -> bool {
        matches!(self, Self::Proxied { .. })
    }

    /// The wire format of the base URL the adapter sets, or none where it sets none.
    pub fn base_url_format(self) -> Option<Format> {
        match self {
            Self::Proxied { format } => Some(format),
            Self::OwnSignIn => None,
        }
    }
}

/// The route a chat on `declaration`, reaching its model `way`, takes at `level`, or why a key
/// from your keys can't reach it, in words.
pub fn resolve(declaration: &Declaration, way: Way, level: Level) -> Result<Route, String> {
    match way {
        Way::Subscription | Way::HarnessStoredKey | Way::OwnSettingsKey => Ok(Route::OwnSignIn),
        Way::YourKey => {
            let Some(&format) = declaration.route.formats.first() else {
                let why = own_sign_in_only(declaration).unwrap_or_default();
                return Err(format!("{why}. A key from your keys can't reach it."));
            };
            if level == Level::Three {
                return Err(format!(
                    "A key from your keys runs {} at levels 1 and 2 only, and this chat starts it \
                     at level 3, over ACP, which purlis has not measured with the key proxy yet. \
                     Start it at level 1 or 2.",
                    declaration.title
                ));
            }
            Ok(Route::Proxied { format })
        }
    }
}

/// **Whether a chat on `declaration`, reaching its model `way` at `level`, starts under this
/// machine's policy `locks`**: its route, or why not, in words (R-128, R-430, R-714). A policy
/// file that can't be read refuses a proxied start; a `min-version` above this build, or a key
/// inside `models`, refuses every chat that reaches a model, a subscription chat included.
pub fn start(
    declaration: &Declaration,
    way: Way,
    level: Level,
    locks: &crate::sandbox::policy::Locks,
) -> Result<Route, String> {
    let route = resolve(declaration, way, level)?;
    match locks.refuses_model_start(route) {
        Some(why) => Err(why),
        None => Ok(route),
    }
}

/// Why `declaration`'s harness takes its own sign-in only, or none where it takes a base URL:
/// it states no wire format it speaks (R-227, R-257).
pub fn own_sign_in_only(declaration: &Declaration) -> Option<String> {
    declaration.route.formats.is_empty().then(|| {
        format!(
            "{} states no wire format it speaks, so it takes no base URL from purlis and signs in \
             with its own sign-in only",
            declaration.title
        )
    })
}

/// **The wire format a pair speaks**: the first of `declaration`'s `[route] formats` that
/// `provider` serves an endpoint in, or none where they share none, so the pair takes no proxied
/// route. The one place a pair's format is worked out: the shipped table reads it rather than
/// restating it.
pub fn pair_format(
    declaration: &Declaration,
    provider: &crate::model_provider::Declaration,
) -> Option<Format> {
    declaration.route.formats.iter().copied().find(|&format| {
        provider
            .endpoints
            .iter()
            .any(|endpoint| endpoint.format == format)
    })
}

#[cfg(test)]
mod tests;
