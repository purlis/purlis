// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where a link's commands go: this host, or a runner it relays the link to** (1.33, #2679,
//! ADR 0078 §4: *"The window reaches a runner through its own `local-ui` connection to its own
//! host, which relays"*).
//!
//! - **A window's link names its runner with its first command** (`relay_to`), before it
//!   subscribes, after only the window's own name (`name_window`, 1.31), which is handed to
//!   whichever serves the link: from then on every command on it goes to a [`Relay`] kept to that runner, and
//!   the runner's views, events and asks come back on it. A link whose first command is any
//!   other is this host's, and a later `relay_to` on it is refused, so a link never changes
//!   hands with what it holds.
//! - **Only the window asks** (`local-ui`, [`purlis_session_protocol::grants::Power::RelayTo`]),
//!   and only for a runner the binary's [`RelaysTo`] answers: the one this machine paired and
//!   keeps, reached through its connector, checked before every connection, and linked with
//!   the key pinned at its pairing. Anything else is refused, and the link is this host's
//!   still undecided, so the window may ask again or use it here.

use std::sync::{Arc, Mutex};

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::session::{self, Answer, Command, Peer, Pushed, Refusal, Since};
use tokio::sync::mpsc;

use crate::hosting::Hosting;
use crate::relay::{Keep, Relay};

/// What links a window's link to the runner this machine keeps under a name: how to keep a link
/// to it ([`Keep`]), or why there is none. Called off the runtime, as each `relay_to` is taken.
pub type RelaysTo = Arc<dyn Fn(&str) -> Result<Keep, String> + Send + Sync>;

/// One link, served by this host or by a relay to a runner, as its first command decides: what
/// the host serves each link it admits as (`crate::serve`).
pub struct Routed {
    hosting: Hosting,
    route: Mutex<Route>,
    /// The window's `name_window` (1.31), while the link is undecided: handed to whichever
    /// serves the link once it is decided, before anything else.
    named: Mutex<Option<Command>>,
}

#[derive(Clone)]
enum Route {
    /// Nothing has been sent on the link yet.
    Undecided,
    /// A `relay_to` is being taken: what else comes meanwhile is refused.
    Relaying,
    /// This host's.
    Here,
    /// Relayed to a runner.
    There(Relay),
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Where a command that is not `relay_to` goes.
enum Goes {
    Here,
    There(Relay),
    Refused(Refusal),
}

impl Routed {
    pub fn new(hosting: Hosting) -> Routed {
        Routed {
            hosting,
            route: Mutex::new(Route::Undecided),
            named: Mutex::new(None),
        }
    }

    /// Where the link's next command goes; an undecided link is this host's from now on.
    fn goes(&self) -> Goes {
        let mut route = lock(&self.route);
        match &*route {
            Route::Undecided | Route::Here => {
                *route = Route::Here;
                Goes::Here
            }
            Route::Relaying => Goes::Refused(Refusal {
                code: session::NOT_STARTED.into(),
                why: "this link is being relayed to a runner; send it once the relay answers"
                    .into(),
            }),
            Route::There(relay) => Goes::There(relay.clone()),
        }
    }

    async fn relay_to(&self, runner: String, peer: &Peer) -> Result<Answer, Refusal> {
        // `serve` asked the scope's grants already; this is the same rule, said here too.
        if peer.scope() != Scope::LocalUi {
            return Err(not_allowed(format!(
                "`relay_to` is purlis's window's alone, and this link is `{}`",
                peer.scope()
            )));
        }
        {
            let mut route = lock(&self.route);
            if !matches!(*route, Route::Undecided) {
                return Err(not_allowed(
                    "`relay_to` is a link's first command, and this link has sent another",
                ));
            }
            *route = Route::Relaying;
        }
        let relayed = match self.hosting.relays() {
            None => Err("this host relays to no runner".to_owned()),
            Some(relays) => tokio::task::spawn_blocking(move || relays(&runner))
                .await
                .unwrap_or_else(|why| Err(format!("the runner could not be looked up: {why}"))),
        };
        let relay = match relayed {
            Ok(keep) => Relay::kept(keep).claiming(self.hosting.claimed()),
            Err(why) => {
                *lock(&self.route) = Route::Undecided;
                return Err(Refusal {
                    code: session::NOT_STARTED.into(),
                    why,
                });
            }
        };
        // The name the window gave this link goes to the relay first, which names the window
        // by it to the runner.
        let named = lock(&self.named).take();
        if let Some(named) = named
            && let Err(refused) = session::Host::call(&relay, named.clone(), peer).await
        {
            // Kept again, so a `relay_to` sent again still carries it.
            *lock(&self.named) = Some(named);
            *lock(&self.route) = Route::Undecided;
            return Err(refused);
        }
        *lock(&self.route) = Route::There(relay);
        Ok(Answer::Done)
    }
}

impl Routed {
    /// The window's name, kept while the link was undecided, handed to this host now that the
    /// link is its own.
    async fn named_here(&self, peer: &Peer) {
        let named = lock(&self.named).take();
        if let Some(named) = named {
            let _ = session::Host::call(&self.hosting, named, peer).await;
        }
    }
}

fn not_allowed(why: impl Into<String>) -> Refusal {
    Refusal {
        code: session::NOT_ALLOWED.into(),
        why: why.into(),
    }
}

impl session::Host for Routed {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        if let Command::RelayTo { runner } = command {
            return self.relay_to(runner, peer).await;
        }
        // The window names itself first (1.31) and may still relay the link after: its name is
        // kept until the link is decided.
        if let Command::NameWindow { .. } = &command {
            let route = lock(&self.route);
            if matches!(*route, Route::Undecided) {
                let mut named = lock(&self.named);
                if named.is_some() {
                    return Err(Refusal {
                        code: session::MALFORMED.into(),
                        why: "a window names itself as its link's first command, and this link \
                              has sent one before"
                            .into(),
                    });
                }
                *named = Some(command);
                return Ok(Answer::Done);
            }
            drop(route);
        }
        match self.goes() {
            Goes::Here => {
                self.named_here(peer).await;
                session::Host::call(&self.hosting, command, peer).await
            }
            Goes::There(relay) => session::Host::call(&relay, command, peer).await,
            Goes::Refused(refusal) => Err(refusal),
        }
    }

    async fn elicits_a_secret(&self, chat: &str, ask: &str) -> bool {
        let route = lock(&self.route).clone();
        match route {
            Route::There(relay) => session::Host::elicits_a_secret(&relay, chat, ask).await,
            _ => session::Host::elicits_a_secret(&self.hosting, chat, ask).await,
        }
    }

    async fn link_ended(&self, peer: &Peer) {
        let route = lock(&self.route).clone();
        match route {
            Route::There(relay) => session::Host::link_ended(&relay, peer).await,
            _ => session::Host::link_ended(&self.hosting, peer).await,
        }
    }

    async fn subscribe(
        &self,
        since: Since,
        peer: &Peer,
    ) -> Result<mpsc::Receiver<Pushed>, Refusal> {
        match self.goes() {
            Goes::Here => {
                self.named_here(peer).await;
                session::Host::subscribe(&self.hosting, since, peer).await
            }
            Goes::There(relay) => session::Host::subscribe(&relay, since, peer).await,
            Goes::Refused(refusal) => Err(refusal),
        }
    }
}

#[cfg(test)]
mod tests;
