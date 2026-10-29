// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Docker pool, added and removed from the window** (spec #2041, #2044, #2679).
//!
//! - **Adding a pool makes its one runner and pairs it at once** ([`add`]): the pool is recorded
//!   ([`crate::runners::record_pool`]), its runner's container is made from the pool's image,
//!   suspending itself after the pool's time without a link (#3471), confined and held to its
//!   limits ([`crate::dockerpool::Docker::make`]), paired through the
//!   connector ([`crate::dockerpool::Docker::pair`]), and its device id and key kept once
//!   ([`crate::runners::keep`]). Anything that fails takes back what this add made, the pool
//!   included, and says what it could not.
//! - **Removing a pool removes its runner whole first** ([`remove`],
//!   [`crate::runners::remove`]), and forgets the pool once its runner is gone, so a removal
//!   that leaves something can be asked again.
//!
//! - **After an update, a pool of purlis's own image follows the desktop's version, side by
//!   side** ([`follow`], #2749, ruled 2026-10-20): a runner of the image of the desktop's
//!   version is made, checked and paired beside the pool's, kept as the pool's runner, and the
//!   one it replaces is kept draining: it keeps its chats and starts no new one, and is removed
//!   once its last chat has ended ([`drain`]). A pool of a team's own image is never moved.
//!
//! A pool's chats start on its one runner that is not draining: the one called by the pool's
//! name, or the one made beside it after an update.
//!
//! - **A cluster pool is added with its runner claimed and paired at once** ([`add_cluster`],
//!   #3292): the pool is recorded with the cluster operator key the person checked, a kept
//!   runner is claimed from the cluster's Pool as who the kubeconfig is, waited on until the
//!   operator vouches for its key, paired over this desktop's own `kubectl exec` and kept. What
//!   fails deletes the claim and forgets the pool. Removing it deletes its claim, and with it its
//!   runner ([`remove_cluster`]).

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
    // Held across the whole add, so a removal of this pool, or another add of it, waits.
    let locked = runners::lock(config_root);
    runners::record_pool_locked(config_root, pool, &locked)?;
    let made = docker
        .declared(pool.image())
        .and_then(|(user, home)| Runner::new(pool.name(), pool.image().clone(), user, &home))
        .map(|runner| {
            runner
                .limited(pool.limits())
                .suspending_after(pool.suspend_after())
        })
        .and_then(|runner| docker.make_for(&runner, Some(&runners::desktop_label(config_root))));
    let made = match made {
        Ok(made) => made,
        Err(why) => {
            let why = Refused::Other(own_time_said(pool, why.0));
            return Err(forgotten(config_root, pool, why, &locked));
        }
    };
    let kept = docker
        .pair(&made, desktop)
        .map_err(|why| Refused::Other(why.0))
        .and_then(|answered| {
            runners::keep_for_its_pool(config_root, &made, &answered, desktop, &locked)
        });
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
            Err(forgotten(config_root, pool, why, &locked))
        }
    }
}

/// `why` its runner could not be made, with the likely cause where the pool has a time of its
/// own before its runner suspends (#3471): a runner image of a purlis before that refuses
/// `--suspends-unlinked=<duration>` and exits as it starts.
fn own_time_said(pool: &Pool, why: String) -> String {
    if pool.set().suspend_after.is_none() {
        return why;
    }
    format!(
        "{why}. This pool has its own Suspends after, which its runner is started with; a runner \
         image whose purlis is older than that setting refuses it and stops as it starts, so \
         build the image from a current purlis or leave Suspends after empty"
    )
}

/// `why`, once `pool`'s record is forgotten; a record that could not be is said with it.
fn forgotten(config_root: &Path, pool: &Pool, why: Refused, locked: &runners::Locked) -> Refused {
    match runners::forget_pool_locked(config_root, pool.name(), locked) {
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
    let locked = runners::lock(config_root);
    if !runners::pools(config_root)?
        .iter()
        .any(|pool| pool.name() == name)
    {
        return Err(Refused::Other(format!(
            "no pool named {name} is on this machine"
        )));
    }
    // Every runner of the pool, a draining one too (#2749), the pool's current one last.
    let mut of_it: Vec<runners::Kept> = runners::read(config_root)?
        .into_iter()
        .filter(|kept| kept.pool() == name)
        .collect();
    of_it.sort_by_key(|kept| !kept.draining);
    let mut removed = None;
    let mut all_gone = true;
    for kept in of_it {
        let one = runners::remove_locked(config_root, docker, kept.name(), unpair, &locked)?;
        all_gone &= one.forgotten;
        removed = Some(one);
    }
    if all_gone {
        runners::forget_pool_locked(config_root, name, &locked)?;
        // Every workspace that ran its chats on it runs them on this machine again.
        crate::machine::update(config_root, |store| {
            for recent in &mut store.recents {
                recent.chats_run_on.retain(|_, pool| pool != name);
            }
        })
        .map_err(|why| {
            Refused::Other(format!(
                "pool {name} was removed, but the workspaces that ran their chats on it could \
                 not be set back to this machine: {why}"
            ))
        })?;
    }
    Ok(removed)
}

/// Where the person removes a pool, as Settings names it.
pub const SETTING: &str = "Settings › This machine › Pools";

/// **Which pool `workspace` of the plane at `plane` runs its chats on, on this machine**
/// (#2679, #2046): `None` for this machine, the pool where this machine's store names one it
/// keeps with its runner, and refused, in words, where it names one this machine does not keep:
/// such a chat starts nowhere, and is never started here in its place.
pub fn runs_on(
    config_root: &Path,
    plane: &Path,
    workspace: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(workspace) = workspace else {
        return Ok(None);
    };
    let loaded = crate::machine::read(config_root);
    let Some(pool) = loaded.store.chats_run_on(plane, workspace) else {
        return Ok(None);
    };
    if let Err(missing) = runners::runner_of(config_root, pool) {
        return Err(format!(
            "Workspace {workspace} runs its chats on pool {pool}: {missing}. Choose where its \
             chats run in its settings, or add the pool again in {SETTING}."
        ));
    }
    Ok(Some(pool.to_owned()))
}

/// **Every pool a workspace of the plane at `plane` runs its chats on**, with those
/// workspaces, as this machine's store names them (#2046). Whether each pool is kept is asked
/// as each is reached ([`runs_on`]).
pub fn workspaces_on_pools(
    config_root: &Path,
    plane: &Path,
) -> std::collections::BTreeMap<String, Vec<String>> {
    let loaded = crate::machine::read(config_root);
    let mut on: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for (workspace, pool) in loaded.store.every_chats_run_on(plane) {
        on.entry(pool.to_owned())
            .or_default()
            .push(workspace.to_owned());
    }
    on
}

/// **Moves the pool `name` to `image`, the runner image of the desktop's version** (#2749): a
/// runner of `image` is made beside the pool's, held to the pool's limits, checked, paired with
/// `desktop` and kept as the pool's runner, and every runner of the pool before it is kept
/// draining ([`runners::succeed_locked`]). The runner kept, or `None` where the pool is of
/// `image` already.
///
/// Refused, with nothing changed, for a pool of a team's own image, which is never moved, and
/// for an `image` that is not purlis's own. What fails takes back the runner this made; the
/// pool and its runner stay as they were.
pub fn follow(
    config_root: &Path,
    docker: &Docker,
    name: &str,
    image: &crate::dockerpool::Image,
    desktop: &Pinned,
) -> Result<Option<Kept>, Refused> {
    let locked = runners::lock(config_root);
    let pool = runners::pools(config_root)?
        .into_iter()
        .find(|pool| pool.name() == name)
        .ok_or_else(|| Refused::Other(format!("no pool named {name} is on this machine")))?;
    if pool.image() == image {
        return Ok(None);
    }
    if !pool.image().is_purlis_own() || !image.is_purlis_own() {
        return Err(Refused::Other(format!(
            "pool {name} runs {}, which is not purlis's own runner image, so purlis does not \
             move it to another",
            pool.image().as_str()
        )));
    }
    let successor = crate::dockerpool::successor_name(name, image);
    // What a follow that stopped short of keeping its runner left goes first (#2869).
    clear_leftovers_locked(config_root, docker, &successor, &locked)?;
    let made = docker
        .declared(image)
        .and_then(|(user, home)| Runner::new(&successor, image.clone(), user, &home))
        .map(|runner| {
            runner
                .limited(pool.limits())
                .suspending_after(pool.suspend_after())
        })
        .and_then(|runner| docker.make_for(&runner, Some(&runners::desktop_label(config_root))))
        .map_err(|why| Refused::Other(why.0))?;
    let kept = docker
        .pair(&made, desktop)
        .map_err(|why| Refused::Other(why.0))
        .and_then(|answered| {
            runners::succeed_locked(config_root, name, &made, &answered, desktop, &locked)
        });
    match kept {
        Ok(kept) => Ok(Some(kept)),
        Err(why) => Err(match docker.remove(&made) {
            Ok(()) => why,
            Err(left) => Refused::Other(format!(
                "{why}; and the runner made for it could not all be removed ({left}): remove \
                 the container {} and the volume {} with docker",
                made.container.as_str(),
                made.runner.volume()
            )),
        }),
    }
}

/// **Removes what a make of the runner `name` left behind, where nothing kept claims it**
/// (#2869): a container and home volume of its name, made by a follow that stopped before
/// `runners.json` kept the runner. A name a kept runner or a pool holds is left alone. What was
/// removed, in a sentence; `None` where nothing was.
pub fn clear_leftovers_of(
    config_root: &Path,
    docker: &Docker,
    name: &str,
) -> Result<Option<String>, Refused> {
    clear_leftovers_locked(config_root, docker, name, &runners::lock(config_root))
}

fn clear_leftovers_locked(
    config_root: &Path,
    docker: &Docker,
    name: &str,
    _locked: &runners::Locked,
) -> Result<Option<String>, Refused> {
    let claimed = runners::named(config_root, name)?.is_some()
        || runners::pools(config_root)?
            .iter()
            .any(|pool| pool.name() == name);
    if claimed {
        return Ok(None);
    }
    let removed = docker
        .remove_leftovers_of(name, &runners::desktop_label(config_root))
        .map_err(|why| Refused::Other(why.0))?;
    Ok((!removed.is_empty()).then(|| {
        format!(
            "removed {} that a move of the pool to a new runner left behind",
            removed.join(" and ")
        )
    }))
}

/// **Removes the draining runner `name`** (#2749), once its last chat has ended, as a runner is
/// removed ([`runners::remove`]). Refused for a runner that is not draining.
pub fn drain(
    config_root: &Path,
    docker: &Docker,
    name: &str,
    unpair: Option<&Unpair<'_>>,
) -> Result<Removed, Refused> {
    let locked = runners::lock(config_root);
    match runners::named(config_root, name)? {
        Some(kept) if kept.draining => {
            runners::remove_locked(config_root, docker, name, unpair, &locked)
        }
        Some(_) => Err(Refused::Other(format!(
            "runner {name} is its pool's runner, not one draining, so it was not removed"
        ))),
        None => Err(Refused::Other(format!(
            "no runner named {name} is kept on this machine"
        ))),
    }
}

/// **The claim a cluster pool's runner is made by on this desktop** (#3292): the pool's name and
/// this desktop's label ([`runners::desktop_label`]), so two desktops of one person never take
/// each other's claim, within the 63 characters a kept claim's pod is named in.
pub fn claim_name(config_root: &Path, pool: &str) -> String {
    format!("{pool}-{}", runners::desktop_label(config_root))
}

/// How long adding a cluster pool waits for the cluster operator to vouch for its runner.
pub const VOUCHED_WITHIN: std::time::Duration = std::time::Duration::from_secs(300);

/// How often it asks.
pub const ASKED_EVERY: std::time::Duration = std::time::Duration::from_secs(2);

/// **Adds the cluster pool `pool` on this machine** (#3292), through `kubectl` (found on this
/// machine, pointed at the pool's kubeconfig and context here): records it with the operator key
/// the person checked, once the cluster's Pool still names that key, and **its own copy of the
/// context** ([`runners::keep_kubeconfig`]), which every run of its kubectl is pointed at from
/// then on, this add's included; claims a kept runner in its namespace, waits at most `within`
/// for the operator to vouch for it, pairs it with `desktop` and keeps what it answered. What
/// fails deletes the claim, the copy and the pool, and says so.
pub fn add_cluster(
    config_root: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    pool: &runners::ClusterPool,
    desktop: &Pinned,
    within: std::time::Duration,
) -> Result<runners::ClusterKept, Refused> {
    let taken_from = pool.kubectl(kubectl);
    let locked = runners::lock(config_root);
    let pin = pool.pin()?;
    let named = taken_from
        .pool_pin(pool.pool())
        .map_err(|why| Refused::Other(why.to_string()))?;
    if named != pin {
        return Err(Refused::Other(format!(
            "the cluster's pool {} names another cluster operator key, or another cluster, than \
             the one you checked, so nothing was added; check the key again",
            pool.pool()
        )));
    }
    let context = taken_from
        .context_kept()
        .map_err(|why| Refused::Other(why.to_string()))?;
    // Recorded pointing at its own copy, which is kept only once its name is the pool's.
    let pool = &pool.kept_at(&runners::kubeconfig_kept_at(config_root, pool.name()));
    runners::record_cluster_pool_locked(config_root, pool, &locked)?;
    if let Err(why) = runners::keep_kubeconfig(config_root, pool.name(), &context) {
        return Err(forgotten_cluster(config_root, pool.name(), why, &locked));
    }
    let kubectl = pool.kubectl(kubectl);
    let name = claim_name(config_root, pool.name());
    let paired = crate::clusterpool::claim_and_pair(
        &kubectl,
        &pin,
        pool.pool(),
        pool.namespace(),
        &name,
        desktop,
        within,
        ASKED_EVERY,
    )
    .map_err(|why| Refused::Other(why.to_string()));
    let kept = paired.and_then(|paired| {
        let kept = runners::ClusterKept {
            name: pool.name().to_owned(),
            claimed: paired.claimed,
            pod: paired.vouched.pod,
            pinned: paired.pinned,
            paired: runners::now(),
        };
        match runners::keep_cluster_runner_locked(config_root, &kept, desktop, &locked) {
            Ok(()) => Ok(kept),
            Err(why) => Err(match kubectl.delete_claim(pool.namespace(), &name) {
                Ok(()) => why,
                Err(left) => Refused::Other(format!(
                    "{why}; and the claim could not be deleted ({left}): delete the claim \
                     {name} in namespace {} with kubectl",
                    pool.namespace()
                )),
            }),
        }
    });
    kept.map_err(|why| forgotten_cluster(config_root, pool.name(), why, &locked))
}

/// `why`, once the cluster pool `name`'s record is forgotten.
fn forgotten_cluster(
    config_root: &Path,
    name: &str,
    why: Refused,
    locked: &runners::Locked,
) -> Refused {
    let forgotten = runners::forget_pool_locked(config_root, name, locked)
        .and_then(|()| runners::forget_kubeconfig(config_root, name));
    match forgotten {
        Ok(()) => why,
        Err(left) => Refused::Other(format!(
            "{why}; and the pool could not be forgotten ({left})"
        )),
    }
}

/// **The check before every link to the cluster pool `name`'s runner** (#3292, #3305), at
/// `now`, through `kubectl` (found on this machine), with the pool and its runner as this machine
/// keeps them now: the pool's pin follows a roll its pinned key vouches for and, moved, is kept
/// in `runners.json` before anything else ([`runners::keep_cluster_pin`]), so the next check
/// starts from it; then the operator must still vouch for the runner
/// ([`crate::clusterpool::before_link`]).
///
/// Right after a move, a Claim not yet signed by the new key refuses this link as
/// [`crate::clusterpool::Refused::NotVouched`], which only refuses it: the next try links once
/// the operator has signed it again, and nothing is unpaired.
pub fn before_cluster_link(
    config_root: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    name: &str,
    now: u64,
) -> Result<(), crate::clusterpool::Refused> {
    use crate::clusterpool::Refused as Not;
    let (pool, kept) = runners::cluster_runner_for(config_root, name)
        .map_err(|why| Not::Other(why.to_string()))?
        .ok_or_else(|| Not::Other(format!("no cluster pool named {name} is on this machine")))?;
    let was = pool.pin().map_err(|why| Not::Other(why.to_string()))?;
    let mut pin = was.clone();
    let checked = crate::clusterpool::before_link(
        &pool.kubectl(kubectl),
        &mut pin,
        pool.pool(),
        &kept.claimed,
        &kept.pod,
        &kept.pinned,
        now,
    );
    if pin != was
        && let Err(why) = runners::keep_cluster_pin(config_root, name, &pin)
    {
        tracing::warn!("purlis: pool {name}'s cluster operator key moved, and was not kept: {why}");
    }
    checked
}

/// What a cluster runner's removal is handed to take this desktop out of its peers over its
/// link, where that link is up.
pub type UnpairCluster<'a> =
    dyn Fn(&runners::ClusterPool, &runners::ClusterKept) -> Result<(), String> + 'a;

/// **Removes the cluster pool `name`** (#3292): this desktop out of its runner's peers with
/// `unpair`, where given, then its claim deleted, and with it its runner, through `kubectl`
/// pointed at the pool's cluster; the pool and its runner are forgotten once the claim is, and
/// every workspace that ran its chats on it runs them on this machine again. What could not be
/// deleted is refused with how to delete it, and the pool kept, so the removal can be asked
/// again.
pub fn remove_cluster(
    config_root: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    name: &str,
    unpair: Option<&UnpairCluster<'_>>,
) -> Result<(), Refused> {
    let locked = runners::lock(config_root);
    let pool = runners::cluster_pools(config_root)?
        .into_iter()
        .find(|pool| pool.name() == name)
        .ok_or_else(|| Refused::Other(format!("no pool named {name} is on this machine")))?;
    let kept = runners::cluster_runners(config_root)?
        .into_iter()
        .find(|kept| kept.name == name);
    if let Some(kept) = &kept {
        if let Some(unpair) = unpair
            && let Err(why) = unpair(&pool, kept)
        {
            // Its peers go with its claim's volume, so a runner that does not answer is no
            // reason to stop.
            tracing::info!("purlis: pool {name}'s runner was not unpaired: {why}");
        }
        pool.kubectl(kubectl)
            .delete_claim(&kept.claimed.namespace, &kept.claimed.name)
            .map_err(|why| {
                Refused::Other(format!(
                    "pool {name}'s claim could not be deleted ({why}); purlis keeps the pool \
                     until it is, so it can be removed again, or delete the claim {} in \
                     namespace {} with kubectl",
                    kept.claimed.name, kept.claimed.namespace
                ))
            })?;
        runners::forget_cluster_runner_locked(config_root, name, &locked)?;
    }
    runners::forget_pool_locked(config_root, name, &locked)?;
    runners::forget_kubeconfig(config_root, name)?;
    crate::machine::update(config_root, |store| {
        for recent in &mut store.recents {
            recent.chats_run_on.retain(|_, pool| pool != name);
        }
    })
    .map_err(|why| {
        Refused::Other(format!(
            "pool {name} was removed, but the workspaces that ran their chats on it could not \
             be set back to this machine: {why}"
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests;
