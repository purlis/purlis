// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's browser logins on a pool's runners** (#2050, spec #2041 stories 22 and 23).
//!
//! A runner chat's browser runs on its runner, and a login purlis performs in it carries the
//! vault's values there for the time the browser fills them. So a login on a runner needs, each
//! checked by the window as it reads the values:
//!
//! - **A runner purlis can show is the person's own** ([`owned`]): until a pool can be marked
//!   owned (#2561), that is a Docker pool whose engine is a local socket on this machine, so its
//!   runner is this machine. A pool on any other engine (`tcp://`, `ssh://`), and every other kind
//!   of pool, is refused in words. A runner not shown to be owned never receives a value.
//! - **The desktop connected**: the window is the one reading the values, so it is.
//! - **The person's opt-in for that persona's login on that pool**, standing for the pool and its
//!   runner as they are now. It is bound ([`Bound`]) to a digest of what defines the pool (its
//!   name, kind, image and limits, as `runners.json` keeps them), the pool's image digest, the
//!   runner the chat's link reaches (its device id, pinned link key and its own image), the
//!   engine it runs on, and the login's identity as declared and allowed. Any change asks again
//!   ([`Standing::Changed`]).
//! - **It is kept in memory, for this run of the app**: a restart asks again. Nothing of it is
//!   written, so no new store is made, and nothing of it reaches a runner. A lasting answer is
//!   #3299.
//!
//! No vault value is ever stored on a runner: the login's values go to its browser for the one
//! login, and the runner keeps none.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use crate::runners::{Kept, Pool};

/// **What an opt-in is bound to**: the pool, the digest of how it is defined, its image's digest,
/// the runner that receives the values, and the engine that runner runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    pub pool: String,
    pub definition: String,
    pub image: String,
    /// The receiving runner's device id, pinned link key (hex) and image, as this machine keeps
    /// them.
    pub runner: String,
    pub runner_key: String,
    pub runner_image: String,
    /// The engine the pool runs on, as the Ask names it.
    pub engine: String,
}

impl Bound {
    /// `pool`, its runner `runner`, on `engine`, as they are now.
    pub fn of(pool: &Pool, runner: &Kept, engine: &str) -> Bound {
        Bound {
            pool: pool.name().to_owned(),
            definition: pool.definition_digest(),
            image: pool.image().digest().to_owned(),
            runner: runner.pinned.device.clone(),
            runner_key: crate::runnerlink::hex_of(runner.pinned.key.as_bytes()),
            runner_image: runner.made.runner.image().digest().to_owned(),
            engine: engine.to_owned(),
        }
    }

    /// The pool named `pool` as this machine keeps it under `config_root` now, with the runner a
    /// new chat of it reaches, where purlis can show that runner is the person's own
    /// ([`owned`]); or why not, in words.
    pub fn read(config_root: &Path, pool: &str) -> Result<Bound, String> {
        let engine = owned(crate::dockerpool::Docker::engine_on_this_machine())?;
        let kept = crate::runners::pools(config_root)
            .map_err(|why| why.to_string())?
            .into_iter()
            .find(|one| one.name() == pool)
            .ok_or_else(|| format!("no pool named {pool} is on this machine"))?;
        let runner =
            crate::runners::runner_for(config_root, pool).map_err(|why| why.to_string())?;
        Ok(Bound::of(&kept, &runner, &engine))
    }
}

/// **Whether a Docker pool's runner is shown to be the person's own** (#2050): its engine is a
/// local socket on this machine, as `engine` says; named for the Ask as this machine's Docker.
/// Refused, in words that say a login is never filled there, otherwise.
pub fn owned(engine: Result<std::path::PathBuf, String>) -> Result<String, String> {
    match engine {
        Ok(_) => Ok("this machine's Docker".to_owned()),
        Err(why) => Err(format!(
            "{why}, so its runners are not known to be yours, and purlis fills no browser login \
             there until a pool can be marked as yours"
        )),
    }
}

/// Where a persona's login stands on a pool as it is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// The person opted the login in for the pool as it is now.
    Allowed,
    /// Nobody was asked.
    Unasked,
    /// The person answered for the pool as it was: it or its runner changed since.
    Changed,
    /// The person kept the login off on this pool, as it is now.
    KeptOff,
}

/// Which opt-in: a persona's login, by its identity, on a pool.
type Key = (String, String, String);

/// The opt-ins the person gave in this run of the app, by persona, login and pool.
#[derive(Debug, Default)]
pub struct OptIns {
    held: Mutex<HashMap<Key, (Bound, bool)>>,
}

impl OptIns {
    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<Key, (Bound, bool)>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn key(persona: &str, login: &str, bound: &Bound) -> Key {
        (persona.to_owned(), login.to_owned(), bound.pool.clone())
    }

    /// The person opts `persona`'s login `login` (its identity) in for the pool as `bound` says.
    pub fn allow(&self, persona: &str, login: &str, bound: Bound) {
        self.held()
            .insert(Self::key(persona, login, &bound), (bound, true));
    }

    /// The person keeps `persona`'s login `login` off on the pool as `bound` says it is.
    pub fn keep_off(&self, persona: &str, login: &str, bound: Bound) {
        self.held()
            .insert(Self::key(persona, login, &bound), (bound, false));
    }

    /// Where `persona`'s login `login` stands on the pool as `now` says it is.
    pub fn standing(&self, persona: &str, login: &str, now: &Bound) -> Standing {
        match self.held().get(&Self::key(persona, login, now)) {
            None => Standing::Unasked,
            Some((then, _)) if then != now => Standing::Changed,
            Some((_, true)) => Standing::Allowed,
            Some((_, false)) => Standing::KeptOff,
        }
    }
}

#[cfg(test)]
mod tests;
