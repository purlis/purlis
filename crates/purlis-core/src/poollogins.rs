// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's browser logins on a pool's runners** (#2050, spec #2041 stories 22 and 23).
//!
//! A runner chat's browser runs on its runner, and a login purlis performs in it carries the
//! vault's values there for the time the browser fills them. So a login on a runner needs
//! three things, each checked by the window as it reads the values: the desktop connected (the
//! window is the one reading them, so it is), the person's opt-in for that persona on that pool,
//! and the opt-in still standing for the pool as it is now.
//!
//! - **An opt-in is bound to the pool's definition and its image** ([`Bound`]): a digest of
//!   what defines the pool (its name, kind, image and limits, as `runners.json` keeps them), and
//!   the image's own digest. A pool whose definition or image changed since is a pool the person
//!   has not opted in to: they are asked again ([`Standing::Changed`]).
//! - **It is the person's, for one persona on one pool**: another persona's chats, or another
//!   pool's, are asked of apart.
//! - **It is kept in memory, for this run of the app**: a restart asks again. Nothing of it is
//!   written, so no new store is made, and nothing of it reaches a runner.
//!
//! No vault value is ever stored on a runner: the login's values go to its browser for the one
//! login, and the runner keeps none.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use crate::runners::Pool;

/// **What an opt-in is bound to**: the pool, the digest of how it is defined, and its image's
/// digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    pub pool: String,
    pub definition: String,
    pub image: String,
}

impl Bound {
    /// `pool` as it is defined now.
    pub fn of(pool: &Pool) -> Bound {
        Bound {
            pool: pool.name().to_owned(),
            definition: pool.definition_digest(),
            image: pool.image().digest().to_owned(),
        }
    }

    /// The pool named `pool` as this machine keeps it under `config_root` now, or why it cannot
    /// be read.
    pub fn read(config_root: &Path, pool: &str) -> Result<Bound, String> {
        crate::runners::pools(config_root)
            .map_err(|why| why.to_string())?
            .iter()
            .find(|one| one.name() == pool)
            .map(Bound::of)
            .ok_or_else(|| format!("no pool named {pool} is on this machine"))
    }
}

/// Where a persona's logins stand on a pool as it is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// The person opted the persona in for the pool as it is now.
    Allowed,
    /// Nobody was asked.
    Unasked,
    /// The person opted it in for the pool as it was: its definition or its image changed since.
    Changed,
    /// The person kept its logins off on this pool, as it is now.
    KeptOff,
}

/// The opt-ins the person gave in this run of the app, by persona and pool.
#[derive(Debug, Default)]
pub struct OptIns {
    held: Mutex<HashMap<(String, String), (Bound, bool)>>,
}

impl OptIns {
    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<(String, String), (Bound, bool)>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The person opts `persona`'s browser logins in for the pool as `bound` says it is.
    pub fn allow(&self, persona: &str, bound: Bound) {
        self.held()
            .insert((persona.to_owned(), bound.pool.clone()), (bound, true));
    }

    /// The person keeps `persona`'s browser logins off on the pool as `bound` says it is.
    pub fn keep_off(&self, persona: &str, bound: Bound) {
        self.held()
            .insert((persona.to_owned(), bound.pool.clone()), (bound, false));
    }

    /// Where `persona`'s logins stand on the pool as `now` says it is.
    pub fn standing(&self, persona: &str, now: &Bound) -> Standing {
        match self.held().get(&(persona.to_owned(), now.pool.clone())) {
            None => Standing::Unasked,
            Some((then, _)) if then != now => Standing::Changed,
            Some((_, true)) => Standing::Allowed,
            Some((_, false)) => Standing::KeptOff,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bound(pool: &str, definition: &str, image: &str) -> Bound {
        Bound {
            pool: pool.into(),
            definition: definition.into(),
            image: image.into(),
        }
    }

    #[test]
    fn nobody_is_opted_in_until_the_person_says() {
        let opt_ins = OptIns::default();
        assert_eq!(
            opt_ins.standing("steward", &bound("builds", "d1", "i1")),
            Standing::Unasked
        );
    }

    #[test]
    fn an_opt_in_holds_for_its_persona_on_its_pool_as_it_was() {
        let opt_ins = OptIns::default();
        opt_ins.allow("steward", bound("builds", "d1", "i1"));

        assert_eq!(
            opt_ins.standing("steward", &bound("builds", "d1", "i1")),
            Standing::Allowed
        );
        assert_eq!(
            opt_ins.standing("reviewer", &bound("builds", "d1", "i1")),
            Standing::Unasked,
            "another persona is asked apart"
        );
        assert_eq!(
            opt_ins.standing("steward", &bound("tests", "d1", "i1")),
            Standing::Unasked,
            "another pool is asked apart"
        );
    }

    #[test]
    fn a_changed_image_or_definition_asks_again() {
        let opt_ins = OptIns::default();
        opt_ins.allow("steward", bound("builds", "d1", "i1"));

        assert_eq!(
            opt_ins.standing("steward", &bound("builds", "d1", "i2")),
            Standing::Changed
        );
        assert_eq!(
            opt_ins.standing("steward", &bound("builds", "d2", "i1")),
            Standing::Changed
        );
    }

    #[test]
    fn kept_off_stands_until_the_pool_changes() {
        let opt_ins = OptIns::default();
        opt_ins.keep_off("steward", bound("builds", "d1", "i1"));

        assert_eq!(
            opt_ins.standing("steward", &bound("builds", "d1", "i1")),
            Standing::KeptOff
        );
        assert_eq!(
            opt_ins.standing("steward", &bound("builds", "d1", "i2")),
            Standing::Changed
        );
    }

    #[test]
    fn a_pools_bound_follows_its_image_and_its_limits() {
        use crate::dockerpool::Image;
        use crate::runners::Set;
        let image = |hex: char| {
            Image::pinned(&format!(
                "ghcr.io/purlis/purlis-runner@sha256:{}",
                hex.to_string().repeat(64)
            ))
            .unwrap()
        };
        let one = Pool::new("builds", image('a'), Set::default()).unwrap();
        let moved = Pool::new("builds", image('b'), Set::default()).unwrap();
        let limited = Pool::new(
            "builds",
            image('a'),
            Set {
                memory_mib: Some(8192),
                ..Set::default()
            },
        )
        .unwrap();

        let (a, b, c) = (Bound::of(&one), Bound::of(&moved), Bound::of(&limited));
        assert_eq!(a, Bound::of(&one), "the same pool, the same bound");
        assert_eq!(a.image, "a".repeat(64));
        assert_ne!(a.image, b.image);
        assert_ne!(a.definition, b.definition, "its image is part of its definition");
        assert_eq!(a.image, c.image);
        assert_ne!(a.definition, c.definition, "and so are its limits");
    }
}
