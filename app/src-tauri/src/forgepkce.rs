// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Sign in with GitLab**, in the browser (#730, ADR 0077 §1 and §3): the authorization code
//! flow with PKCE over a loopback redirect, carried to the window.
//!
//! The flow is the core's ([`purlis_core::forge::pkce`], with the listener of
//! [`purlis_core::forge::loopback`]). This file runs it where the human scope is, the window's
//! process, and gives it the three things only the app has: the person's browser, the forge's
//! token address over HTTPS, and the account store.
//!
//! **Nothing of a sign-in crosses to the window.** [`forge_pkce_sign_in`] takes the forge, the
//! host and a name for the try, and answers the account as Settings lists it. The address the
//! browser is sent to (which holds the `state`), the code and the token stay in this process;
//! the browser is opened from here, never by the page.
//!
//! **A try is named by the window, so a cancel always finds its own.** The window may send a
//! cancel before the try it names has started here (the two are separate calls), and may start
//! another try straight after. [`Tries`] keeps both straight: a cancel for a try not yet started
//! is remembered until it starts, and starting a try stops any other, so one listener at most is
//! ever open.
//!
//! Both commands are the window's alone (`WINDOW_ONLY`): no link starts or stops a sign-in.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use purlis_core::forge::accounts::{self, Method};
use purlis_core::forge::registration::{self, Flow};
use purlis_core::forge::{loopback, oauth, pkce};

use crate::forgeaccounts::{
    ForgeAccountRow, blocking, config_root, host_scope, kind_of, token_check,
};

/// How many cancels of tries that never started are remembered.
const MOST_STOPPED: usize = 16;

/// The sign-ins running in this process, and the ones stopped before they started.
#[derive(Default)]
pub(crate) struct Tries {
    running: HashMap<String, Arc<AtomicBool>>,
    stopped: VecDeque<String>,
}

impl Tries {
    /// Starts the try `name`: every other running try is stopped, and the flag that stops this
    /// one is answered, already set when the window stopped it before it started.
    pub(crate) fn begin(&mut self, name: &str) -> Result<Arc<AtomicBool>, String> {
        let named = (1..=64).contains(&name.len())
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        if !named || self.running.contains_key(name) {
            return Err(
                "That sign-in could not be started. Close the dialog and try again.".into(),
            );
        }
        for other in self.running.values() {
            other.store(true, Ordering::SeqCst);
        }
        let stopped_already = self.stopped.iter().position(|one| one == name);
        if let Some(at) = stopped_already {
            self.stopped.remove(at);
        }
        let flag = Arc::new(AtomicBool::new(stopped_already.is_some()));
        self.running.insert(name.to_string(), flag.clone());
        Ok(flag)
    }

    /// The try `name` is over, however it ended.
    pub(crate) fn end(&mut self, name: &str) {
        self.running.remove(name);
    }

    /// Stops the try `name`: now when it is running, as it starts when it has not yet.
    pub(crate) fn stop(&mut self, name: &str) {
        match self.running.get(name) {
            Some(flag) => flag.store(true, Ordering::SeqCst),
            None if name.len() <= 64 => {
                if self.stopped.len() == MOST_STOPPED {
                    self.stopped.pop_front();
                }
                self.stopped.push_back(name.to_string());
            }
            None => {}
        }
    }
}

static TRIES: Mutex<Option<Tries>> = Mutex::new(None);

pub(crate) fn tries<T>(with: impl FnOnce(&mut Tries) -> T) -> T {
    let mut held = TRIES.lock().unwrap_or_else(|e| e.into_inner());
    with(held.get_or_insert_with(Tries::default))
}

/// The flow itself: the page opened, its one redirect taken, the code exchanged, the token kept.
fn through_the_browser(
    scope: &purlis_core::forge::route::HostScope,
    root: &std::path::Path,
    kind: purlis_core::forge::Kind,
    registration: &'static registration::Registration,
    endpoints: &oauth::Endpoints,
    cancelled: &AtomicBool,
    open: &dyn Fn(&str) -> Result<(), String>,
) -> Result<ForgeAccountRow, String> {
    let endpoint = oauth::Https::shared();
    let granted = pkce::sign_in(
        scope,
        registration,
        cancelled,
        &pkce::Ways {
            bind: &loopback::bind,
            open,
            exchange: &pkce::exchange_at(endpoint.as_ref(), endpoints),
            clock: loopback::Clock::system(),
        },
    )?;
    let keyring = accounts::keyring_at(root);
    accounts::sign_in_with_grant(
        scope,
        root,
        keyring.as_ref(),
        &token_check,
        kind,
        registration.host,
        granted,
        Method::Pkce,
    )
    .map(ForgeAccountRow::from)
}

/// Sign in to `kind` on `host` in the person's browser, as the try `attempt`: the forge's
/// authorization page is opened there, its one redirect is taken on a loopback port, and the
/// code is exchanged for a token kept in the account's own keyring item. The account as Settings
/// lists it, or a sentence saying why not. Neither ever holds the code or the token.
///
/// Only a host purlis has a PKCE registration on signs in this way (gitlab.com); any other host
/// is told to use a token.
#[tauri::command]
#[specta::specta]
pub async fn forge_pkce_sign_in(
    app: tauri::AppHandle,
    kind: String,
    host: String,
    attempt: String,
) -> Result<ForgeAccountRow, String> {
    let kind = kind_of(&kind)?;
    let cancelled = tries(|all| all.begin(&attempt))?;
    let done = blocking(move || {
        let scope = host_scope()?;
        // A scenario run's fake forge plays the browser and the OAuth side (#4090's proof): the
        // shipped flow, listener and exchange, against the host the run names.
        #[cfg(feature = "e2e")]
        if let Some(forge) = crate::e2eforge::browser_forge() {
            let (registration, endpoints) = forge.registration();
            let open = |page: &str| forge.open(page);
            return through_the_browser(
                &scope,
                &config_root()?,
                kind,
                registration,
                &endpoints,
                &cancelled,
                &open,
            );
        }
        let registration = registration::on(kind, host.trim())
            .filter(|found| found.flow == Flow::Pkce)
            .ok_or_else(|| {
                "purlis has no browser sign-in on that host. Sign in with a token instead."
                    .to_string()
            })?;
        // A test build (the e2e build among them) has no person at a browser, and must never
        // reach the real forge.
        if purlis_core::fence::FENCED {
            return Err(
                "A test build of purlis does not sign in through the browser. Sign in with a \
                 token instead."
                    .to_string(),
            );
        }
        let root = config_root()?;
        let open = |page: &str| {
            use tauri_plugin_opener::OpenerExt as _;
            // The opener's own words could repeat the address, which holds the `state`.
            app.opener()
                .open_url(page, None::<&str>)
                .map_err(|_| "the browser did not open".to_string())
        };
        through_the_browser(
            &scope,
            &root,
            kind,
            registration,
            &oauth::Endpoints::of(registration),
            &cancelled,
            &open,
        )
    })
    .await;
    tries(|all| all.end(&attempt));
    done
}

/// Stop the browser sign-in `attempt`: its listener closes at once and nothing is signed in.
/// One that has not started yet is stopped as it starts; one that is over is left as it ended.
#[tauri::command]
#[specta::specta]
pub fn forge_pkce_sign_in_cancel(attempt: String) {
    tries(|all| all.stop(&attempt));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stopped(flag: &AtomicBool) -> bool {
        flag.load(Ordering::SeqCst)
    }

    #[test]
    fn a_cancel_stops_the_try_it_names_and_no_other() {
        let mut tries = Tries::default();
        let first = tries.begin("try-1").unwrap();
        tries.stop("try-2");
        assert!(!stopped(&first));
        tries.stop("try-1");
        assert!(stopped(&first));
    }

    #[test]
    fn a_cancel_that_arrives_before_its_try_stops_it_as_it_starts() {
        let mut tries = Tries::default();
        tries.stop("try-1");
        assert!(stopped(&tries.begin("try-1").unwrap()));
        // And only once: the name is free again when that try is over.
        tries.end("try-1");
        assert!(!stopped(&tries.begin("try-1").unwrap()));
    }

    #[test]
    fn starting_a_try_stops_the_one_before_so_one_listener_is_open_at_most() {
        let mut tries = Tries::default();
        let first = tries.begin("try-1").unwrap();
        let second = tries.begin("try-2").unwrap();
        assert!(stopped(&first));
        assert!(!stopped(&second));
        // The first try's late cancel does not reach the second.
        tries.stop("try-1");
        tries.end("try-1");
        assert!(!stopped(&second));
    }

    #[test]
    fn a_try_needs_a_plain_name_that_is_not_already_running() {
        let mut tries = Tries::default();
        for name in ["", "has space", "a/b", &"x".repeat(65)] {
            assert!(tries.begin(name).is_err(), "{name:?}");
        }
        tries.begin("3f0c9c2e-7a51-4c58-9d0a-0d6a1c5f2b11").unwrap();
        assert!(tries.begin("3f0c9c2e-7a51-4c58-9d0a-0d6a1c5f2b11").is_err());
    }

    #[test]
    fn only_so_many_cancels_of_tries_that_never_start_are_remembered() {
        let mut tries = Tries::default();
        for n in 0..=MOST_STOPPED {
            tries.stop(&format!("never-{n}"));
        }
        tries.stop(&"x".repeat(65));
        assert_eq!(tries.stopped.len(), MOST_STOPPED);
        assert!(!stopped(&tries.begin("never-0").unwrap()));
        assert!(stopped(&tries.begin("never-1").unwrap()));
    }
}
