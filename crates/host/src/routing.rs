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
/// to it ([`Keep`]), and the runner the name resolved to, as `<pool>/<runner>` (#2865); or why
/// there is none. Called off the runtime, as each `relay_to` is taken.
pub type RelaysTo = Arc<dyn Fn(&str) -> Result<(Keep, String), String> + Send + Sync>;

/// **Asks for the named runner to run again, where it is suspended for being idle** (#3980): only
/// for the window's own steps towards a chat there: relaying a link, holding a project and starting
/// a program. A link that redials, a check or a view never does, so a runner's own link does not
/// undo its suspension.
///
/// **Answers how long a start waits for that runner** (#4403), where this word brings it back;
/// `None` where it does not. A start waits out a wake another link asked for too.
pub type RunsAgain = Arc<dyn Fn(&str) -> Option<std::time::Duration> + Send + Sync>;

/// **The `<pool>/<runner>` a link's `relay_to` answer names** (#4071): a cluster runner's answer
/// goes on to name the link key it was pinned with, `<pool>/<runner>@<key>`, for the window alone.
/// Everything here goes by the name.
fn runner_named(resolved: &str) -> &str {
    resolved
        .split_once('@')
        .map_or(resolved, |(named, _)| named)
}

/// **Whether the runner `<pool>/<runner>` it is handed drains** (#2865): one replaced by a runner
/// of the desktop's version, which keeps its chats until they end. A link relayed to it starts
/// no new program there, asked again at each start, so a link relayed before a follow drains
/// with its runner. Called off the runtime. The binary's, or nothing, and no runner drains.
pub type DrainsTo = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// One link, served by this host or by a relay to a runner, as its first command decides: what
/// the host serves each link it admits as (`crate::serve`).
pub struct Routed {
    hosting: Hosting,
    route: Mutex<Route>,
    /// The window's `name_window` (1.31), while the link is undecided: handed to whichever
    /// serves the link once it is decided, before anything else.
    named: Mutex<Option<Command>>,
    /// The runner the link is relayed to, as `<pool>/<runner>` (#2865): asked at each start
    /// whether it drains.
    runner: Mutex<Option<String>>,
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
            runner: Mutex::new(None),
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
        let (relay, reported) = match relayed {
            Ok((keep, resolved)) => {
                let named = runner_named(&resolved).to_owned();
                *lock(&self.runner) = Some(named.clone());
                let relay = Relay::kept(keep)
                    .claiming(self.hosting.claimed(), &named)
                    .stopping_first(self.hosting.stop_first(named));
                (relay, resolved)
            }
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
        *lock(&self.route) = Route::There(relay.clone());
        // The window relays a link as a chat starts or is taken back there, and a link to a
        // suspended runner gets no further without it (#3980).
        let _ = self.run_again().await;
        // A link that ended while this was taken was ended as this host's: its relay is told
        // too, so the name it holds there goes with it.
        if peer.has_ended() {
            session::Host::link_ended(&relay, peer).await;
        }
        // The window is told which runner its link reached (1.48, #3417): a draining one is
        // kept as the link's own until it ends, apart from the pool's runner now.
        Ok(if peer.version() >= session::KNOWS_RELAYED_TO {
            Answer::RelayedTo { runner: reported }
        } else {
            Answer::Done
        })
    }
}

impl Routed {
    /// Whether the runner this link is relayed to drains now, read off the runtime.
    async fn drains(&self) -> bool {
        let (Some(drains), Some(runner)) = (self.hosting.drains(), lock(&self.runner).clone())
        else {
            return false;
        };
        // A look that could not be made refuses the start rather than let it through.
        tokio::task::spawn_blocking(move || drains(&runner))
            .await
            .unwrap_or(true)
    }

    /// Asks for the runner this link is relayed to to run again, off the runtime: how long a
    /// start waits for it, where it is on its way back. A stopped host wakes nothing.
    async fn run_again(&self) -> Option<std::time::Duration> {
        let (Some(runs_again), Some(runner)) =
            (self.hosting.runs_again(), lock(&self.runner).clone())
        else {
            return None;
        };
        if self.hosting.is_stopped() {
            return None;
        }
        let asking = runner.clone();
        let asked = tokio::task::spawn_blocking(move || runs_again(&asking))
            .await
            .ok()
            .flatten();
        self.hosting.waits_for(&runner, asked)
    }

    /// Whether the runner this link is relayed to was asked to run again, and is not back yet.
    fn its_runner_comes_back(&self) -> bool {
        !self.hosting.is_stopped()
            && lock(&self.runner)
                .as_deref()
                .is_some_and(|runner| self.hosting.waits_for(runner, None).is_some())
    }

    /// `peer`'s hold or start waits for its runner where that is on its way back (#4403).
    async fn waits_for_the_runner(&self, relay: &Relay, peer: &Peer) -> bool {
        let Some(patience) = self.run_again().await else {
            return false;
        };
        if relay.waits_for_its_runner(peer, patience).await
            && let Some(runner) = lock(&self.runner).clone()
        {
            self.hosting.came_back(&runner);
        }
        true
    }

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
            Goes::There(relay) => {
                // A runner that drains keeps its chats, and starts no new one (#2865): asked at
                // each start, so a link relayed before its runner was replaced drains with it.
                if matches!(command, Command::StartProgram(_)) && self.drains().await {
                    return Err(not_allowed(session::A_DRAINING_RUNNER_STARTS_NONE));
                }
                // Before anything that needs the runner: a start holds its project first, and a
                // relay that is down refuses that hold (#3980), so it waits for a runner on its way
                // back (#4403).
                if matches!(
                    command,
                    Command::HoldProject { .. } | Command::StartProgram(_)
                ) && self.waits_for_the_runner(&relay, peer).await
                    && matches!(command, Command::StartProgram(_))
                    && self.drains().await
                {
                    // Replaced while it waited: the rule above holds at the end of the wait too.
                    return Err(not_allowed(session::A_DRAINING_RUNNER_STARTS_NONE));
                }
                session::Host::call(&relay, command, peer).await
            }
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
            Goes::There(relay) if self.its_runner_comes_back() => {
                relay.subscribes_as_its_runner_comes_back(since, peer).await
            }
            Goes::There(relay) => session::Host::subscribe(&relay, since, peer).await,
            Goes::Refused(refusal) => Err(refusal),
        }
    }
}

#[cfg(test)]
mod tests;
