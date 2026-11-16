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
/// from your keys can't reach it, in words: the copy module's W133 and W134.
pub fn resolve(declaration: &Declaration, way: Way, level: Level) -> Result<Route, String> {
    use crate::window_copy::{fill, line};
    match way {
        Way::Subscription | Way::HarnessStoredKey | Way::OwnSettingsKey => Ok(Route::OwnSignIn),
        Way::YourKey => {
            let Some(&format) = declaration.route.formats.first() else {
                return Err(own_sign_in_only(declaration).unwrap_or_default());
            };
            if level == Level::Three {
                return Err(fill(line::W133, &[("harness", &declaration.title)]));
            }
            Ok(Route::Proxied { format })
        }
    }
}

/// Why `declaration`'s harness takes its own sign-in only, or none where it takes a base URL:
/// it states no wire format it speaks (R-227, R-257). The copy module's W134.
pub fn own_sign_in_only(declaration: &Declaration) -> Option<String> {
    use crate::window_copy::{fill, line};
    declaration
        .route
        .formats
        .is_empty()
        .then(|| fill(line::W134, &[("harness", &declaration.title)]))
}

/// **A run's listener, as its start was handed it** (T1a-15): its port on `127.0.0.1` and the
/// run's token. The token never shows in `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Listener {
    pub port: u16,
    pub token: String,
}

impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener")
            .field("port", &self.port)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// **The route resolver at a chat's start** (T1a-15, with T1a-17): the route `declaration` takes
/// for `way` at `level`, and for a proxied one the listener `listen` starts for the run. The
/// unproxied ways start none: `listen` is never called for them (R-036), nor for a start refused
/// in words (R-225, R-227).
pub fn at_start(
    declaration: &Declaration,
    way: Way,
    level: Level,
    listen: impl FnOnce() -> Result<Listener, String>,
) -> Result<Option<Listener>, String> {
    match resolve(declaration, way, level)? {
        Route::OwnSignIn => Ok(None),
        Route::Proxied { .. } => listen().map(Some),
    }
}

/// **What a proxied chat's adapter is handed** (T1a-15, R-232): the run's listener, the
/// background model of the provider it reaches (`[models] background`, R-293), and the person's
/// own loopback bypass, `NO_PROXY` and `no_proxy` as the chat's environment holds them, which the
/// adapter keeps and adds `127.0.0.1` to. Which variables carry each is the adapter's, never this
/// module's (R-222). The token never shows in `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyRoute {
    listener: Listener,
    background_model: Option<String>,
    no_proxy: Option<String>,
    no_proxy_lower: Option<String>,
}

impl KeyRoute {
    /// The route through `listener`, with `background_model` and the person's bypass read from
    /// `env`, the environment the chat starts with.
    pub fn new(
        listener: Listener,
        background_model: Option<&str>,
        env: impl Fn(&str) -> Option<String>,
    ) -> KeyRoute {
        KeyRoute {
            listener,
            background_model: background_model.map(str::to_owned),
            no_proxy: env("NO_PROXY"),
            no_proxy_lower: env("no_proxy"),
        }
    }

    /// The listener's URL, on `127.0.0.1` (never `localhost`, which a bypass of `127.0.0.1`
    /// doesn't cover, T1a-02).
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.listener.port)
    }

    /// The run's token: what the harness alone is handed, in its process environment (R-066).
    pub fn token(&self) -> &str {
        &self.listener.token
    }

    /// The model for the background slot, where the provider names one.
    pub fn background_model(&self) -> Option<&str> {
        self.background_model.as_deref()
    }

    /// The person's own `NO_PROXY`, then `no_proxy`, as the chat's environment holds them.
    pub fn bypass(&self) -> [Option<&str>; 2] {
        [self.no_proxy.as_deref(), self.no_proxy_lower.as_deref()]
    }
}

impl std::fmt::Debug for KeyRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyRoute")
            .field("listener", &self.listener)
            .field("background_model", &self.background_model)
            .finish_non_exhaustive()
    }
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
