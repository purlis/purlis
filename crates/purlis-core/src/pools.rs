// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Docker pool, added and removed from the window** (spec #2041, #2044, #2679).
//!
//! - **Adding a pool makes its one runner and pairs it at once** ([`add`]): the pool is recorded
//!   ([`crate::runners::record_pool`]), its runner's container is made from the pool's image,
//!   confined and held to its limits ([`crate::dockerpool::Docker::make`]), paired through the
//!   connector ([`crate::dockerpool::Docker::pair`]), and its device id and key kept once
//!   ([`crate::runners::keep`]). Anything that fails takes back what this add made, the pool
//!   included, and says what it could not.
//! - **Removing a pool removes its runner whole first** ([`remove`],
//!   [`crate::runners::remove`]), and forgets the pool once its runner is gone, so a removal
//!   that leaves something can be asked again.
//!
//! A pool has one runner, called by the pool's name. Chats in a workspace that runs on the pool
//! all run on it.

use std::path::Path;

use purlis_session_protocol::hostlink::Pinned;

use crate::dockerpool::{Docker, Runner};
use crate::runners::{self, Kept, Pool, Refused, Removed, Unpair};

/// **Adds `pool` on this machine**, with `docker`: records it, makes and checks its runner,
/// pairs it with `desktop` (this desktop's device id and public link key) and keeps what the
/// runner answered, once. The runner it kept.
///
/// What fails takes back what this add made: the runner's container and home volume, and the
/// pool's record. A part that could not be taken back is said with the refusal, with the
/// command that removes it.
pub fn add(
    config_root: &Path,
    docker: &Docker,
    pool: &Pool,
    desktop: &Pinned,
) -> Result<Kept, Refused> {
    runners::record_pool(config_root, pool)?;
    let made = docker
        .declared(pool.image())
        .and_then(|(user, home)| Runner::new(pool.name(), pool.image().clone(), user, &home))
        .map(|runner| runner.limited(pool.limits()))
        .and_then(|runner| docker.make(&runner));
    let made = match made {
        Ok(made) => made,
        Err(why) => return Err(forgotten(config_root, pool, Refused::Other(why.0))),
    };
    let kept = docker
        .pair(&made, desktop)
        .map_err(|why| Refused::Other(why.0))
        .and_then(|answered| runners::keep(config_root, &made, &answered, desktop));
    match kept {
        Ok(kept) => Ok(kept),
        Err(why) => {
            let why = match docker.remove(&made) {
                Ok(()) => why,
                Err(left) => Refused::Other(format!(
                    "{why}; and its runner could not all be removed ({left}): remove the \
                     container {} and the volume {} with docker",
                    made.container.as_str(),
                    made.runner.volume()
                )),
            };
            Err(forgotten(config_root, pool, why))
        }
    }
}

/// `why`, once `pool`'s record is forgotten; a record that could not be is said with it.
fn forgotten(config_root: &Path, pool: &Pool, why: Refused) -> Refused {
    match runners::forget_pool(config_root, pool.name()) {
        Ok(()) => why,
        Err(left) => Refused::Other(format!(
            "{why}; and the pool could not be forgotten ({left})"
        )),
    }
}

/// **Removes the pool `name`**: its runner whole ([`runners::remove`]), with `unpair` taking
/// this desktop out of its peers when its link is up, then the pool's record once its runner is
/// gone. A pool whose runner was never kept is forgotten at once. What [`runners::remove`] did,
/// where it ran.
pub fn remove(
    config_root: &Path,
    docker: &Docker,
    name: &str,
    unpair: Option<&Unpair<'_>>,
) -> Result<Option<Removed>, Refused> {
    if !runners::pools(config_root)?
        .iter()
        .any(|pool| pool.name() == name)
    {
        return Err(Refused::Other(format!(
            "no pool named {name} is on this machine"
        )));
    }
    let removed = match runners::named(config_root, name)? {
        Some(_) => Some(runners::remove(config_root, docker, name, unpair)?),
        None => None,
    };
    if removed.as_ref().is_none_or(|removed| removed.forgotten) {
        runners::forget_pool(config_root, name)?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests;
