// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's browser logins on a pool's runners** (#2050, spec #2041 stories 22 and 23).
//!
//! A runner chat's browser runs on its runner, and a login purlis performs in it carries the
//! vault's values there for the time the browser fills them. So a login on a runner needs, each
//! checked by the window as it reads the values:
//!
//! - **A runner purlis can show is the person's own**: a Docker pool whose engine is a local
//!   socket on this machine ([`owned`]), so its runner is this machine, with no mark; or a
//!   cluster pool the person marked and whose claim is vouched for now
//!   ([`Bound::of_a_cluster_pool`], #4071, the person's ruling of 2026-10-30). A pool on any
//!   other engine (`tcp://`, `ssh://`) is refused in words, and so is an unmarked cluster pool.
//!   A runner not shown to be owned never receives a value.
//! - **The desktop connected**: the window is the one reading the values, so it is.
//! - **The person's opt-in for that persona's login on that pool**, standing for the pool and its
//!   runner as they are now. It is bound ([`Bound`]) to a digest of what defines the pool (its
//!   name, kind, image and limits, as `runners.json` keeps them), the pool's image digest, the
//!   runner the chat's link reaches (its device id, pinned link key and its own image), the
//!   engine it runs on, and the login's identity as declared and allowed. Any change asks again
//!   ([`Standing::Changed`]). A cluster runner has no image digest bound: its opt-in is bound to
//!   its pool's name, context, cluster, operator key, claim uid and pod, the pod's uid, its device
//!   and link key, and the engine the Ask names ([`Bound::of_cluster`]).
//! - **It is kept in memory, for this run of the app**: a restart asks again. Nothing of it is
//!   written, so no new store is made, and nothing of it reaches a runner. A lasting answer is
//!   #3299.
//!
//! No vault value is ever stored on a runner: the login's values go to its browser for the one
//! login, and the runner keeps none.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use crate::runners::{ClusterKept, ClusterPool, Kept, Pool};

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
    /// The uid of a cluster runner's pod as its cluster says now: a pod made again under its
    /// name, with its key, asks again. Unsigned, so it only ever asks again or refuses. Empty for
    /// a Docker runner.
    pub pod: String,
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
            pod: String::new(),
            engine: engine.to_owned(),
        }
    }

    /// The pool named `pool` as this machine keeps it under `config_root` now, with the runner a
    /// new chat of it reaches, where purlis can show that runner is the person's own
    /// ([`owned`]); or which way it is not ([`Unbound`]).
    pub fn read(config_root: &Path, pool: &str) -> Result<Bound, Unbound> {
        Self::read_with(
            crate::dockerpool::Docker::engine_on_this_machine(),
            config_root,
            pool,
        )
    }

    /// [`Bound::read`] on `engine`, the engine this machine's Docker pools run on.
    pub fn read_with(
        engine: Result<std::path::PathBuf, String>,
        config_root: &Path,
        pool: &str,
    ) -> Result<Bound, Unbound> {
        let engine = owned(engine).map_err(Unbound::NotYours)?;
        let kept = crate::runners::pools(config_root)
            .map_err(|why| Unbound::Unread(why.to_string()))?
            .into_iter()
            .find(|one| one.name() == pool)
            .ok_or(Unbound::NotHere)?;
        let runner = match crate::runners::runner_for(config_root, pool) {
            Ok(runner) => runner,
            // Read again for which way (#3814): a file that cannot be read says so, a pool gone
            // since is not here, and a draining runner starts no new chat (#2749).
            Err(_) => {
                let runners = crate::runners::read(config_root)
                    .map_err(|why| Unbound::Unread(why.to_string()))?;
                let still = crate::runners::pools(config_root)
                    .map_err(|why| Unbound::Unread(why.to_string()))?
                    .iter()
                    .any(|one| one.name() == pool);
                return Err(if !still {
                    Unbound::NotHere
                } else if runners
                    .iter()
                    .any(|kept| kept.pool() == pool && kept.draining)
                {
                    Unbound::Draining
                } else {
                    Unbound::NoRunnerOfItsImage
                });
            }
        };
        Ok(Bound::of(&kept, &runner, &engine))
    }

    /// `pool`, its runner `kept` in the pod of uid `pod`, as they are now (#4071): `definition`
    /// covers its context, cluster, operator key, claim uid and pod's name
    /// ([`ClusterPool::bound_digest`]), its device and key are bound as a Docker runner's are, and
    /// it has no image digest.
    pub fn of_cluster(pool: &ClusterPool, kept: &ClusterKept, pod: &str) -> Bound {
        Bound {
            pool: pool.name().to_owned(),
            definition: pool.bound_digest(kept),
            image: String::new(),
            runner: kept.pinned.device.clone(),
            runner_key: crate::runnerlink::hex_of(kept.pinned.key.as_bytes()),
            runner_image: String::new(),
            pod: pod.to_owned(),
            engine: format!(
                "cluster {}, context \"{}\"",
                pool.pin()
                    .map(|pin| pin.cluster.chars().take(8).collect::<String>())
                    .unwrap_or_default(),
                shortened(pool.context())
            ),
        }
    }

    /// Whether this is a cluster runner: it has no image digest bound, and purlis cannot show
    /// what it keeps.
    pub fn on_a_cluster(&self) -> bool {
        self.image.is_empty()
    }

    /// **The cluster pool `pool`, where one is called so, and may be sent a value now** (#4071,
    /// the person's ruling of 2026-10-30): `None` where no cluster pool is called `pool`. Both
    /// must hold, else it is refused in words:
    ///
    /// - **the person's mark**, read from this machine's own row and nothing else;
    /// - **`vouch`**, the operator vouching for its claim now, which asks the cluster, is asked
    ///   only of a marked pool, and answers the uid of the runner's pod. Its reasons stay in the
    ///   log: a chat is told none of the cluster's words.
    ///
    /// Both are read again once `vouch` has answered: the runner must be the one it vouched
    /// for and the mark must stand, and a pin `vouch` moved through a roll is what is bound.
    pub fn of_a_cluster_pool(
        config_root: &Path,
        pool: &str,
        reached: Option<&str>,
        vouch: &dyn Fn(&Path, &str) -> Result<String, crate::clusterpool::Refused>,
    ) -> Result<Option<Bound>, String> {
        let read = || crate::runners::cluster_runner_for(config_root, pool);
        let Some((held, kept)) = read().map_err(|why| why.to_string())? else {
            return Ok(None);
        };
        // The runner the values would go to is the link's: it must be the one judged here.
        if reached != Some(kept.reached().as_str()) {
            return Err(format!(
                "this chat's link does not reach the runner pool {pool} keeps now, so purlis sent \
                 it no vault value and no token: restart purlis, and start the chat again"
            ));
        }
        if !held.owned() {
            return Err(not_marked_said(pool));
        }
        if crate::runnertaint::pool_ran_unsandboxed(config_root, pool) {
            return Err(crate::runnertaint::sent_nothing_on(pool));
        }
        let pod = vouch(config_root, pool).map_err(|why| {
            tracing::info!("purlis: pool {pool}'s runner was not vouched for now: {why}");
            not_vouched_said(pool)
        })?;
        let (now, runner) = read()
            .map_err(|why| why.to_string())?
            .ok_or_else(|| not_vouched_said(pool))?;
        if runner != kept {
            return Err(not_vouched_said(pool));
        }
        if !now.owned() {
            return Err(not_marked_said(pool));
        }
        Ok(Some(Bound::of_cluster(&now, &runner, &pod)))
    }

    /// **The pool and runner a runner chat's link reached, as this machine keeps them now**
    /// (#3417), where purlis can show that runner is the person's own ([`owned`]): what the
    /// window judges a chat already on a runner by. `reached` is the runner the desktop's host
    /// said the link reached (`relay_to`'s answer, 1.48), as `<pool>/<runner>`: the pool's own
    /// runner, or one of its runners kept draining (ADR 0078 amended 2026-10-21). A host before
    /// 1.48 names none: then the pool's own runner is the one only while none of the pool's
    /// runners drains, since a link relayed before the pool moved on reaches the runner before,
    /// and with one draining it is refused, never judged by the pool's runner now. A new chat or
    /// a move goes by [`Bound::read`], which reaches no draining runner. A cluster pool goes by
    /// [`Bound::of_a_cluster_pool`], which asks its cluster: this may take as long as that does.
    pub fn over_the_link(
        config_root: &Path,
        pool: &str,
        reached: Option<&str>,
    ) -> Result<Bound, String> {
        if let Some(bound) =
            Self::of_a_cluster_pool(config_root, pool, reached, &crate::pools::vouched_now)?
        {
            return Ok(bound);
        }
        Self::over_the_link_with(
            crate::dockerpool::Docker::engine_on_this_machine(),
            config_root,
            pool,
            reached,
        )
    }

    /// [`Bound::over_the_link`] on `engine`, the engine this machine's Docker pools run on.
    pub fn over_the_link_with(
        engine: Result<std::path::PathBuf, String>,
        config_root: &Path,
        pool: &str,
        reached: Option<&str>,
    ) -> Result<Bound, String> {
        let engine = owned(engine)?;
        // Tools and tokens as browser logins: nothing goes to a pool a chat ran on unconfined.
        if crate::runnertaint::pool_ran_unsandboxed(config_root, pool) {
            return Err(crate::runnertaint::sent_nothing_on(pool));
        }
        let kept = crate::runners::pools(config_root)
            .map_err(|why| why.to_string())?
            .into_iter()
            .find(|one| one.name() == pool)
            .ok_or_else(|| format!("no pool named {pool} is on this machine"))?;
        let runner = match reached {
            Some(reached) => {
                let Some((of, name)) = reached.split_once('/').filter(|(of, _)| *of == pool) else {
                    return Err(format!(
                        "this chat's link reached {reached}, which is no runner of pool {pool}"
                    ));
                };
                crate::runners::read(config_root)
                    .map_err(|why| why.to_string())?
                    .into_iter()
                    .find(|kept| kept.pool() == of && kept.name() == name)
                    .ok_or_else(|| {
                        format!("runner {name} of pool {pool} is no longer on this machine")
                    })?
            }
            None => {
                let draining = crate::runners::draining(config_root)
                    .map_err(|why| why.to_string())?
                    .into_iter()
                    .any(|kept| kept.pool() == pool);
                if draining {
                    return Err(format!(
                        "pool {pool} moved to a new runner, and purlis's session host did not say \
                         which of its runners this chat's link reached: restart purlis to \
                         update it"
                    ));
                }
                crate::runners::runner_for(config_root, pool).map_err(|why| why.to_string())?
            }
        };
        Ok(Bound::of(&kept, &runner, &engine))
    }
}

/// `context` as an Ask names it: its words, at most 40 characters of them. Bound whole.
fn shortened(context: &str) -> String {
    let mut said: String = context.chars().take(40).collect();
    if context.chars().count() > 40 {
        said.push('…');
    }
    said
}

/// **Which way a pool is not ready for a new chat of it, or a move to or from it** (#3646):
/// [`Bound::read`]'s refusal, each said in its own words by whoever asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unbound {
    /// Its runners are not shown to be the person's own ([`owned`]), and why.
    NotYours(String),
    /// No pool of that name is on this machine.
    NotHere,
    /// The pool has no runner of its image: it is to be removed and added again.
    NoRunnerOfItsImage,
    /// Its only runner of its image is draining (#2749), and it starts nothing new.
    Draining,
    /// What this machine keeps of its pools could not be read, and why.
    Unread(String),
}

/// **Whether a Docker pool's runner is shown to be the person's own** (#2050): its engine is a
/// local socket on this machine, as `engine` says; named for the Ask as this machine's Docker.
/// Refused, in words that say no vault value, a browser login's or a tool's (#2047), is ever
/// sent there, and no profile signed in there (#2049), otherwise.
pub fn owned(engine: Result<std::path::PathBuf, String>) -> Result<String, String> {
    match engine {
        Ok(_) => Ok("this machine's Docker".to_owned()),
        Err(why) => Err(format!(
            "{why}, so its runners are not known to be yours, and purlis sends no vault value \
             there, for a browser login or a tool, and signs no profile in there"
        )),
    }
}

/// What an unmarked cluster pool is refused with (#4071): how to mark it, and only the person can.
pub fn not_marked_said(pool: &str) -> String {
    format!(
        "pool {pool} is not marked as your own, so purlis sends its runner no vault value and no \
         token. Only you can mark it: in Settings, under Pools, turn on This is my own on its row"
    )
}

/// What a marked cluster pool whose claim is not vouched for now is refused with (#4071).
fn not_vouched_said(pool: &str) -> String {
    format!(
        "purlis could not confirm pool {pool}'s runner as yours just now, so it sent no vault \
         value and no token. Try again; if it stays so, look at the pool in Settings, under Pools"
    )
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

    /// Every answer given about pool `pool`: its mark was taken back (#4071).
    pub fn forget_pool(&self, pool: &str) {
        self.held().retain(|(_, _, of), _| of != pool);
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
