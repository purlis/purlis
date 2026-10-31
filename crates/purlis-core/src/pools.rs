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
//!   version is made beside the pool's, checked and paired, kept as the pool's runner, and the
//!   one it replaces is kept draining: it keeps its chats and starts no new one, and once its
//!   last chat has ended its sign-ins are carried to the pool's runner and it is removed
//!   ([`drain`], #4011, #4055). A pool of a team's own image is never moved.
//!
//! A pool's chats start on its one runner that is not draining: the one called by the pool's
//! name, or the one made beside it after an update.
//!
//! - **A runner removed outside purlis is made again on the person's word** ([`make_again`],
//!   #4149): Docker lists no container of its id, so no link to it comes back. Its pool's row
//!   and a refused start offer Make the runner again, which makes a runner beside it from the
//!   pool's own image and limits, carries the sign-ins of its home where that is still there,
//!   pairs it, keeps it as the pool's runner and removes the gone one.
//!
//! - **A cluster pool is added with its runner claimed and paired at once** ([`add_cluster`],
//!   #3292): the pool is recorded with the cluster operator key the person checked, a kept
//!   runner is claimed from the cluster's Pool as who the kubeconfig is, waited on until the
//!   operator vouches for its key, paired over this desktop's own `kubectl exec` and kept. What
//!   fails deletes the claim and forgets the pool. Removing it deletes its claim, and with it its
//!   runner ([`remove_cluster`]).

use std::path::Path;

use purlis_session_protocol::hostlink::Pinned;

use crate::dockerpool::{Docker, Missing, Runner};
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
        })
        .map(|kept| noted_where_made(config_root, docker, kept, &locked));
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
/// draining ([`runners::succeed_locked`]). Nothing of the old runner's home is carried to it
/// while the old runner has chats (#4055, ruled 2026-10-30): a new chat on it asks to sign in
/// there, and the old runner's sign-ins follow once it drains ([`drain`]). The runner kept, or
/// `None` where the pool is of `image` already.
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
    // Made on another engine, the new runner would take the pool from the one it runs on (#4166).
    if let Ok(kept) = runners::runner_for(config_root, name) {
        on_its_engine(docker, &kept)?;
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
        })
        .map(|kept| noted_where_made(config_root, docker, kept, &locked));
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

/// **Refuses unless Docker reaches the engine that made `kept`** (#4166): nothing is made in its
/// place or removed on another's word. A runner kept before its engine was kept passes.
fn on_its_engine(docker: &Docker, kept: &Kept) -> Result<(), Refused> {
    let Some(made_on) = &kept.engine else {
        return Ok(());
    };
    if docker.engine_id().ok().flatten().as_ref() == Some(made_on) {
        return Ok(());
    }
    Err(Refused::Other(format!(
        "Docker does not reach the engine runner {} was made on, so it stays as it is until it \
         does",
        kept.name()
    )))
}

/// **Keeps with `kept`, a runner just made, the engine that made it and its home's `CreatedAt`**
/// (#4166, #4167). What cannot be read or kept is left out, and that runner is then treated as
/// one made before these were kept: it is said in the log and costs no runner.
fn noted_where_made(
    config_root: &Path,
    docker: &Docker,
    mut kept: Kept,
    locked: &runners::Locked,
) -> Kept {
    let read = |what: &str, answer: Result<Option<String>, crate::dockerpool::Refused>| {
        answer.unwrap_or_else(|why| {
            tracing::warn!("purlis: runner {}'s {what} is not kept: {why}", kept.name());
            None
        })
    };
    let engine = read("engine", docker.engine_id());
    let home_created = read("home's making", docker.home_created(&kept.made));
    let noted = runners::note_locked(
        config_root,
        kept.name(),
        |row| {
            row.engine.clone_from(&engine);
            row.home_created.clone_from(&home_created);
        },
        locked,
    );
    match noted {
        Ok(()) => {
            kept.engine = engine;
            kept.home_created = home_created;
        }
        Err(why) => tracing::warn!(
            "purlis: where runner {} was made is not kept: {why}",
            kept.name()
        ),
    }
    kept
}

/// **A runner made again** (#4149, [`make_again`]): the runner kept, and whether the sign-ins
/// of the one it replaces were carried to it; where not, its profiles are signed in again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MadeAgain {
    pub kept: Kept,
    pub sign_ins_kept: bool,
}

/// **Makes the pool `name`'s runner again, once it was removed outside purlis** (#4149): Docker
/// lists no container of its id ([`Docker::gone`]). As after an update ([`follow`]), a runner is
/// made beside it, under a name of its own ([`crate::dockerpool::remade_name`]), from the pool's
/// own image, held to its limits and its time before it suspends, on a home of its own; the
/// sign-ins of the gone runner's home are carried to it where that home is still there and is
/// the one purlis made for it ([`Docker::copy_profiles`]); it is checked, paired with `desktop`
/// and kept as the pool's runner. The gone runner is then removed, its home with it. The
/// profiles the pool keeps are kept on it again as after the pool was added: the window asks for
/// them.
///
/// A new home and a new pairing, never the old home taken up: the old home holds the gone
/// runner's own link key and peers, which a runner is paired without, and Docker can say no more
/// of a volume than its name and when it was made.
///
/// Refused, with nothing made, for a runner Docker still lists, stopped or suspended among
/// them: that one resumes. What fails takes back the runner this made; the pool stays as it was.
pub fn make_again(
    config_root: &Path,
    docker: &Docker,
    name: &str,
    desktop: &Pinned,
) -> Result<MadeAgain, Refused> {
    let again = crate::dockerpool::remade_name(name).map_err(|why| Refused::Other(why.0))?;
    make_again_as(config_root, docker, name, &again, desktop)
}

/// [`make_again`], the runner made again called `again`.
fn make_again_as(
    config_root: &Path,
    docker: &Docker,
    name: &str,
    again: &str,
    desktop: &Pinned,
) -> Result<MadeAgain, Refused> {
    let locked = runners::lock(config_root);
    let pool = runners::pools(config_root)?
        .into_iter()
        .find(|pool| pool.name() == name)
        .ok_or_else(|| Refused::Other(format!("no pool named {name} is on this machine")))?;
    let old = runners::runner_for(config_root, name)?;
    let gone = match docker
        .missing(&old.made, old.engine.as_deref())
        .map_err(|why| Refused::Other(why.0))?
    {
        Some(Missing::Removed(gone)) => gone,
        // Made here, it would be a second runner on the wrong engine (#4166).
        Some(Missing::OnAnotherEngine) => {
            return Err(Refused::Other(format!(
                "Docker points at another engine than the one runner {} was made on, so purlis \
                 does not make it again; point Docker back at that engine",
                old.name()
            )));
        }
        None => {
            return Err(Refused::Other(format!(
                "runner {}'s container is still there, so purlis does not make it again; it is \
                 linked to as it is",
                old.name()
            )));
        }
    };
    clear_leftovers_locked(config_root, docker, again, &locked)?;
    let made = docker
        .declared(pool.image())
        .and_then(|(user, home)| Runner::new(again, pool.image().clone(), user, &home))
        .map(|runner| {
            runner
                .limited(pool.limits())
                .suspending_after(pool.suspend_after())
        })
        .and_then(|runner| docker.make_for(&runner, Some(&runners::desktop_label(config_root))))
        .map_err(|why| Refused::Other(own_time_said(&pool, why.0)))?;
    let sign_ins_kept =
        gone.home_kept && carried_from_its_own_home(config_root, docker, &old, &made, name);
    let kept = docker
        .pair(&made, desktop)
        .map_err(|why| Refused::Other(why.0))
        .and_then(|answered| {
            runners::succeed_locked(config_root, name, &made, &answered, desktop, &locked)
        })
        .map(|kept| noted_where_made(config_root, docker, kept, &locked));
    let kept = match kept {
        Ok(kept) => kept,
        Err(why) => {
            return Err(match docker.remove(&made) {
                Ok(()) => why,
                Err(left) => Refused::Other(format!(
                    "{why}; and the runner made again could not all be removed ({left}): \
                     remove the container {} and the volume {} with docker",
                    made.container.as_str(),
                    made.runner.volume()
                )),
            });
        }
    };
    // The gone runner goes now, its home with it: nothing of it is linked to again.
    match runners::remove_locked(config_root, docker, old.name(), None, &locked) {
        Ok(removed) => {
            if let Some(left) = removed.left() {
                tracing::warn!(
                    "purlis: runner {} of pool {name} was made again, and the one removed \
                     outside purlis is not all removed: {left}",
                    old.name()
                );
            }
        }
        Err(why) => tracing::warn!(
            "purlis: runner {} of pool {name} was made again, and the one removed outside \
             purlis was not removed: {why}",
            old.name()
        ),
    }
    Ok(MadeAgain {
        kept,
        sign_ins_kept,
    })
}

/// How far the Docker host's clock may run ahead of this machine's when the home of a runner
/// kept before #4167, which has no `CreatedAt` noted, is held to its pairing (#4149): a Docker
/// Desktop VM drifts after a sleep.
const CLOCKS_APART: u64 = 10 * 60;

/// **Whether the home volume under `old`'s name is the one purlis made for it**: it still prints
/// the `CreatedAt` kept when it was made (#4167), whatever the clocks say. A runner kept before
/// that is held to its pairing, give or take [`CLOCKS_APART`] (#4149).
fn is_its_own_home(docker: &Docker, old: &Kept) -> Result<bool, crate::dockerpool::Refused> {
    match &old.home_created {
        Some(kept) => Ok(docker.home_created(&old.made)?.as_ref() == Some(kept)),
        None => Ok(docker
            .home_made_at(&old.made)?
            .is_some_and(|at| at <= old.paired.saturating_add(CLOCKS_APART))),
    }
}

/// **Carries the sign-ins of `old`'s home to `made`** (#4149), only where that home is the one
/// purlis made for it ([`is_its_own_home`]), so not one made of its name since its container
/// went, and its runner's user the same. What is not carried is said in the log, and the
/// profiles are signed in again.
fn carried_from_its_own_home(
    config_root: &Path,
    docker: &Docker,
    old: &Kept,
    made: &crate::dockerpool::Made,
    pool: &str,
) -> bool {
    let its_own = match is_its_own_home(docker, old) {
        Ok(its_own) => its_own,
        Err(why) => {
            tracing::warn!(
                "purlis: runner {}'s home could not be read: {why}",
                old.name()
            );
            false
        }
    };
    if !its_own {
        tracing::warn!(
            "purlis: the home of runner {} is not the one made for it, or says not when it was \
             made, so its sign-ins were not carried; sign the profiles in again on pool {pool}",
            old.name()
        );
        return false;
    }
    if crate::runnertaint::ran_unsandboxed(config_root, &old.made.container) {
        tracing::warn!(
            "purlis: a chat ran on runner {} without the sandbox, or may have, so its sign-ins \
             were not carried; sign the profiles in again on pool {pool}",
            old.name()
        );
        return false;
    }
    if old.made.runner.user() != made.runner.user() {
        tracing::warn!(
            "purlis: runner {} ran as {} and the runner made again as {}, so its sign-ins were \
             not carried; sign the profiles in again on pool {pool}",
            old.name(),
            old.made.runner.user().spelled(),
            made.runner.user().spelled()
        );
        return false;
    }
    match docker.copy_profiles(&old.made, made, &runners::desktop_label(config_root), &[]) {
        Ok(()) => true,
        Err(why) => {
            tracing::warn!(
                "purlis: the sign-ins of runner {} were not carried to the runner made again \
                 for pool {pool}, so its profiles are signed in again there: {why}",
                old.name()
            );
            false
        }
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
///
/// First this desktop is taken out of its peers, so nothing links to it while its sign-ins are
/// carried (#4146). A runner that runs and was not taken out stays until the next look; one that
/// is stopped cannot be asked, and the drain goes on. Then its
/// sign-ins are carried to its pool's runner, on the Docker host ([`Docker::copy_profiles`],
/// #4011), once, however many looks its removal takes, and only then is it removed with its
/// home's volume, so a profile signed in on the pool stays signed in and no sign-in is ever held
/// by two runners at once (#4055, ruled 2026-10-30). One the pool's runner signed in on its own
/// is kept over the carried one, and one signed out of the pool is not carried (#4145). A sign-in
/// that cannot be carried is said in the log, and the runner is removed all the same: that
/// profile is signed in again on the pool, unless its removal fails too and the next look's second
/// try carries it (#4298). None is carried to or from a runner a chat ran on without the sandbox
/// ([`crate::runnertaint`]).
pub fn drain(
    config_root: &Path,
    docker: &Docker,
    name: &str,
    unpair: Option<&Unpair<'_>>,
) -> Result<Removed, Refused> {
    let locked = runners::lock(config_root);
    match runners::named(config_root, name)? {
        Some(kept) if kept.draining => {
            // Another engine knows nothing of its container or its home (#4166).
            on_its_engine(docker, &kept)?;
            let peer = if kept.unpaired {
                None
            } else {
                unpair.map(|unpair| unpair(&kept))
            };
            if matches!(peer, Some(Err(_))) && docker.ran(&kept.made).is_ok_and(|ran| ran.running) {
                return Err(Refused::Other(format!(
                    "runner {name} runs and was not unpaired, so it stays until the next look"
                )));
            }
            if matches!(peer, Some(Ok(())))
                && let Err(why) =
                    runners::note_locked(config_root, name, |kept| kept.unpaired = true, &locked)
            {
                tracing::warn!("purlis: runner {name} may be asked again: {why}");
            }
            carry_once(config_root, docker, &kept, false, &locked);
            let mut removed = runners::remove_locked(config_root, docker, name, None, &locked)?;
            removed.peer = peer;
            Ok(removed)
        }
        Some(_) => Err(Refused::Other(format!(
            "runner {name} is its pool's runner, not one draining, so it was not removed"
        ))),
        None => Err(Refused::Other(format!(
            "no runner named {name} is kept on this machine"
        ))),
    }
}

/// **Carries `kept`'s sign-ins once, however many looks its removal takes** (#4055), and once more
/// where Docker failed the first try (#4298). A second try comes later, so like a carry from a
/// runner with no container (`only_its_own_home`) it takes only a home that is still its own.
fn carry_once(
    config_root: &Path,
    docker: &Docker,
    kept: &Kept,
    only_its_own_home: bool,
    locked: &runners::Locked,
) {
    if kept.carried {
        return;
    }
    let name = kept.name();
    let its_own = if only_its_own_home || kept.carry_failed {
        is_its_own_home(docker, kept)
    } else {
        Ok(true)
    };
    let dockers_failure = match its_own {
        Ok(true) => carry_sign_ins(config_root, docker, kept),
        Ok(false) => false,
        Err(why) => {
            tracing::warn!("purlis: runner {name}'s home could not be read to carry from: {why}");
            true
        }
    };
    let try_again = dockers_failure && !kept.carry_failed;
    if let Err(why) = runners::note_locked(
        config_root,
        name,
        |kept| {
            kept.carried = !try_again;
            kept.carry_failed = try_again;
        },
        locked,
    ) {
        tracing::warn!("purlis: runner {name} may be carried from again: {why}");
    }
}

/// **Carries the draining runner `drained`'s sign-ins to its pool's runner** (#4011, #4055),
/// as it drains: what cannot be carried is said in the log and never holds the drain back.
/// Whether what stopped it was Docker's failure, which may pass, and not a rule that holds.
fn carry_sign_ins(config_root: &Path, docker: &Docker, drained: &Kept) -> bool {
    let pool = drained.pool();
    let Ok(to) = runners::runner_for(config_root, pool) else {
        return false;
    };
    if to.name() == drained.name() {
        return false;
    }
    if [&drained.made, &to.made]
        .iter()
        .any(|made| crate::runnertaint::ran_unsandboxed(config_root, &made.container))
    {
        tracing::warn!(
            "purlis: a chat ran on runner {} or on pool {pool}'s runner {} without the sandbox, \
             or may have, so its profiles' sign-ins were not carried; sign them in again on pool \
             {pool}",
            drained.name(),
            to.name()
        );
        return false;
    }
    if drained.made.runner.user() != to.made.runner.user() {
        tracing::warn!(
            "purlis: runner {} runs as {} and pool {pool}'s runner {} as {}, so its profiles' \
             sign-ins were not carried; sign them in again on pool {pool}",
            drained.name(),
            drained.made.runner.user().spelled(),
            to.name(),
            to.made.runner.user().spelled()
        );
        return false;
    }
    match docker.copy_profiles(
        &drained.made,
        &to.made,
        &runners::desktop_label(config_root),
        &drained.signed_out,
    ) {
        Ok(()) => false,
        Err(why) => {
            tracing::warn!(
                "purlis: the sign-ins of runner {} were not carried to pool {pool}'s runner {}, \
                 so its profiles are signed in again there unless a later look carries them: \
                 {why}",
                drained.name(),
                to.name()
            );
            true
        }
    }
}

/// **Removes the draining runner `name`, which was removed outside purlis** (#4168): the engine
/// that made it no longer lists its container, so none of its chats runs and it cannot be asked.
/// Its sign-ins are carried only from a home still there that is its own ([`is_its_own_home`]),
/// and its row and whatever is left of it go. Refused for a runner Docker still lists, or where
/// another engine answers.
pub fn drain_removed(config_root: &Path, docker: &Docker, name: &str) -> Result<Removed, Refused> {
    let locked = runners::lock(config_root);
    let kept = runners::named(config_root, name)?
        .filter(|kept| kept.draining)
        .ok_or_else(|| Refused::Other(format!("no runner {name} is draining on this machine")))?;
    // With no engine kept, nothing says the engine answering is the one that made it.
    if kept.engine.is_none() {
        return Err(Refused::Other(format!(
            "runner {name} has no engine kept, so it is not known to be removed"
        )));
    }
    let missing = docker
        .missing(&kept.made, kept.engine.as_deref())
        .map_err(|why| Refused::Other(why.0))?;
    let Some(Missing::Removed(gone)) = missing else {
        return Err(Refused::Other(format!(
            "runner {name} is not known to be removed, so it stays as it is"
        )));
    };
    if gone.home_kept {
        carry_once(config_root, docker, &kept, true, &locked);
    }
    runners::remove_locked(config_root, docker, name, None, &locked)
}

/// A draining runner's name, and whether a sign-out ran there or why not.
pub type DrainingSignOut = (String, Result<(), String>);

/// **What signing `profile` out of pool `pool` did on its draining runners** (#4121, #4145): each
/// one's name and whether its harness's sign-out ran. Whatever it did, no drain carries that
/// sign-in ([`runners::hold_from_carrying`]), so a runner it did not reach holds the sign-in only
/// until it is removed with its home. The harness's sign-out runs only on a runner not in `seen`,
/// those an earlier look of this sign-out answered for (#4298).
pub fn sign_out_draining(
    config_root: &Path,
    docker: &Docker,
    pool: &str,
    profile: &str,
    seen: &[DrainingSignOut],
) -> Result<Vec<DrainingSignOut>, Refused> {
    Ok(runners::hold_from_carrying(config_root, pool, profile)?
        .into_iter()
        .filter(|kept| !seen.iter().any(|(runner, _)| runner == kept.name()))
        .map(|kept| {
            let did = docker.sign_out(&kept.made, profile);
            (kept.name().to_owned(), did)
        })
        .collect())
}

/// **The claim a cluster pool's runner is made by on this desktop** (#3292): the pool's name and
/// this desktop's label, made of its device id ([`crate::clusterpool::desktop_label`]), so two
/// desktops of one person never take each other's claim, within the 63 characters a kept
/// claim's pod is named in.
pub fn claim_name(device: &str, pool: &str) -> String {
    format!("{pool}-{}", crate::clusterpool::desktop_label(device))
}

/// **The lock an add of the cluster pool `name` holds from start to end** (#3292): a removal of
/// that pool takes it first, so it waits for the add to finish rather than meet a pool half
/// made.
fn adding(config_root: &Path, name: &str) -> crate::machine::Lock {
    crate::machine::Lock::named(config_root, &format!("cluster-add-{name}.lock"))
}

/// How long adding a cluster pool waits for the cluster operator to vouch for its runner.
pub const VOUCHED_WITHIN: std::time::Duration = std::time::Duration::from_secs(300);

/// How often it asks.
pub const ASKED_EVERY: std::time::Duration = std::time::Duration::from_secs(2);

/// **The programs a pool's kubectl runs, as they are found now** (#3292): kubectl itself and each
/// credential plugin `context` (the pool's context as JSON) names, each both as it is named (a
/// link on the way among them) and by its real path, links followed. Refused, with a sentence,
/// for a plugin purlis does not find, and for a program it cannot hold for no chat to write
/// ([`holdable`]).
fn programs_run(
    kubectl: &crate::clusterpool::Kubectl,
    context: &str,
    home: Option<&Path>,
) -> Result<Vec<std::path::PathBuf>, Refused> {
    let said = |why: crate::clusterpool::Refused| Refused::Other(why.to_string());
    let mut named = vec![kubectl.program().to_path_buf()];
    for command in crate::clusterpool::plugins_named(context).map_err(said)? {
        named.push(kubectl.plugin_at(&command).ok_or_else(|| {
            Refused::Other(format!(
                "the kubeconfig's context runs the credential plugin {command}, which purlis did \
                 not find beside kubectl or in this machine's own folders, so nothing was added; \
                 name it by its absolute path in the kubeconfig"
            ))
        })?);
    }
    let mut programs: Vec<std::path::PathBuf> = Vec::new();
    for program in named {
        let chain = links_to(&program).map_err(|why| {
            Refused::Other(format!(
                "{} could not be read as a program ({why}), so nothing was added",
                program.display()
            ))
        })?;
        if let Some(real) = chain.last() {
            package_holdable(real, home)?;
        }
        for path in chain {
            if !programs.contains(&path) {
                programs.push(path);
            }
        }
    }
    for program in &programs {
        holdable(program)?;
    }
    Ok(programs)
}

/// **`program` and every link on the way to its file** (#3292): the name as given, each link it
/// leads through, in order, and last the real file, links followed. At most 40 links, as a
/// system follows them.
fn links_to(program: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut chain = vec![program.to_path_buf()];
    let mut at = program.to_path_buf();
    for _ in 0..40 {
        let Ok(next) = std::fs::read_link(&at) else {
            break;
        };
        let next = match at.parent() {
            Some(folder) if next.is_relative() => folder.join(next),
            _ => next,
        };
        chain.push(next.clone());
        at = next;
    }
    let real = program.canonicalize()?;
    if !chain.contains(&real) {
        chain.push(real);
    }
    Ok(chain)
}

/// **Whether the package `real`, a program's real file, belongs to can be held whole** (#3292):
/// never where it is the whole home folder, `/` or a folder above the home, which would keep
/// every chat from writing anything there. Refused with a sentence that says what to do.
fn package_holdable(real: &Path, home: Option<&Path>) -> Result<(), Refused> {
    let package = crate::sandbox::package_root(real);
    let too_wide =
        package.parent().is_none() || home.is_some_and(|home| home.starts_with(&package));
    if too_wide {
        return Err(Refused::Other(format!(
            "{} is in {}, which purlis would have to keep every chat from writing to hold it, \
             so nothing was added; move it into a folder of its own and name it there in the \
             kubeconfig",
            real.display(),
            package.display()
        )));
    }
    Ok(())
}

/// **Whether `program` can be held for no chat to write** (#3292): a file, where it and the
/// folder it is in belong to this user or the system and nobody else may write either.
fn holdable(program: &Path) -> Result<(), Refused> {
    let cannot = |why: &str| {
        Err(Refused::Other(format!(
            "{} {why}, so purlis cannot keep it from being changed and nothing was added; \
             install it where only you or the system can write",
            program.display()
        )))
    };
    let Ok(file) = std::fs::metadata(program) else {
        return cannot("could not be read");
    };
    if !file.is_file() {
        return cannot("is not a program's file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let me = purlis_same_user::Uid::effective().as_raw();
        let Some(folder) = program
            .parent()
            .and_then(|folder| std::fs::metadata(folder).ok())
        else {
            return cannot("is in a folder that could not be read");
        };
        for (what, held) in [("it", &file), ("its folder", &folder)] {
            if held.uid() != me && held.uid() != 0 {
                return cannot(&format!("is held by someone else ({what})"));
            }
            if held.mode() & 0o022 != 0 {
                return cannot(&format!("can be written by others ({what})"));
            }
        }
    }
    Ok(())
}

/// **Adds the cluster pool `pool` on this machine** (#3292), through `kubectl` (found on this
/// machine, pointed at the pool's kubeconfig and context here), for the desktop that is
/// `desktop`:
///
/// - the cluster's Pool must still name the key the person checked;
/// - **the pool keeps its own copy of the context** ([`runners::keep_kubeconfig`]), and every
///   later run of its kubectl is pointed at it, whatever the kubeconfig says later;
/// - the programs its kubectl runs are found and kept with the pool, for no chat to write
///   ([`runners::cluster_holds`]); one that cannot be held refuses the add;
/// - a kept runner is claimed in its namespace, marked as this desktop's, waited on for at most
///   `within` until the operator vouches for it, paired and kept.
///
/// `runners.json` is locked only for each write, never across the wait. What fails deletes the
/// claim, the copy and the pool, and says so.
pub fn add_cluster(
    config_root: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    pool: &runners::ClusterPool,
    desktop: &Pinned,
    within: std::time::Duration,
) -> Result<runners::ClusterKept, Refused> {
    let _adding = adding(config_root, pool.name());
    let taken_from = pool.kubectl(kubectl);
    let pin = pool.pin()?;
    // One read of the Pool, so the pin checked and the opt-in recorded are of one version of it.
    let seen = taken_from
        .pool(pool.pool())
        .map_err(|why| Refused::Other(why.to_string()))?;
    let named = crate::clusterpool::PoolPin::first(pool.pool(), &seen)
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
    let held = programs_run(
        &taken_from,
        &context,
        crate::profiles::home()
            .map(|home| crate::sandbox::real(&home))
            .as_deref(),
    )?;
    // Recorded pointing at its own copy, which is kept only once its name is the pool's.
    let pool = &pool
        .kept_at(&runners::kubeconfig_kept_at(config_root, pool.name()))
        .holding(held)?
        .minting_in_cluster(seen.mints_in_cluster);
    runners::record_cluster_pool_locked(config_root, pool, &runners::lock(config_root))?;
    if let Err(why) = runners::keep_kubeconfig(config_root, pool.name(), &context) {
        return Err(forgotten_cluster(config_root, pool.name(), why));
    }
    let kubectl = pool.kubectl(kubectl);
    let name = claim_name(&desktop.device, pool.name());
    let label = crate::clusterpool::desktop_label(&desktop.device);
    let paired = crate::clusterpool::claim_and_pair(
        &kubectl,
        &pin,
        &crate::clusterpool::ClaimTo {
            pool: pool.pool(),
            namespace: pool.namespace(),
            name: &name,
            desktop: &label,
        },
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
        let locked = runners::lock(config_root);
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
    kept.map_err(|why| forgotten_cluster(config_root, pool.name(), why))
}

/// `why`, once the cluster pool `name`'s record and its copy of the context are forgotten. The
/// copy goes even where the record could not.
fn forgotten_cluster(config_root: &Path, name: &str, why: Refused) -> Refused {
    let record = runners::forget_pool_locked(config_root, name, &runners::lock(config_root));
    let copy = runners::forget_kubeconfig(config_root, name);
    match record.and(copy) {
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

/// [`before_cluster_link`], for a link built for the runner `reached` names
/// ([`runners::ClusterKept::reached`]): refused unless that is the runner kept under `name` now.
pub fn before_cluster_link_to(
    config_root: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    name: &str,
    reached: &str,
    now: u64,
) -> Result<(), crate::clusterpool::Refused> {
    use crate::clusterpool::Refused as Not;
    let kept = runners::cluster_runner_for(config_root, name)
        .map_err(|why| Not::Other(why.to_string()))?
        .ok_or_else(|| Not::Other(format!("no cluster pool named {name} is on this machine")))?
        .1;
    if kept.reached() != reached {
        return Err(Not::Other(format!(
            "this link was made for a runner pool {name} no longer keeps"
        )));
    }
    before_cluster_link(config_root, kubectl, name, now)
}

/// **Puts a workspace's repos on the Claim of the cluster pool `name`'s runner** (#4207), as a chat
/// is sent there. Only for a pool that opted in, which says so as it is added; the cluster's policy
/// and the operator decide from the Claim and the Pool as they are now.
pub fn add_repos_to_claim(
    config_root: &Path,
    name: &str,
    repos: impl FnOnce() -> Vec<String>,
) -> Result<(), String> {
    let Some((pool, kept)) =
        runners::cluster_runner_for(config_root, name).map_err(|why| why.to_string())?
    else {
        return Ok(());
    };
    if !pool.mints_in_cluster() {
        return Ok(());
    }
    let repos = repos();
    if repos.is_empty() {
        return Ok(());
    }
    let kubectl = kubectl_here(config_root)?.within(VOUCHED_NOW_WITHIN);
    pool.kubectl(&kubectl)
        .add_claim_repos(&kept.claimed.namespace, &kept.claimed.name, &repos)
        .map_err(|why| why.to_string())
}

/// How long each kubectl run of the check before a value is sent waits (#4071).
pub const VOUCHED_NOW_WITHIN: std::time::Duration = std::time::Duration::from_secs(10);

/// **This machine's `kubectl`, by its absolute path** (#3292,
/// [`crate::clusterpool::Kubectl::on_this_machine`]): where its installers put it, else an
/// absolute folder of the process's `PATH`; its caches kept in a folder of purlis's under
/// `config_root` ([`runners::kubectl_cache`]).
pub fn kubectl_here(config_root: &Path) -> Result<crate::clusterpool::Kubectl, String> {
    let path = crate::envvar::var("PATH");
    let home = crate::envvar::var_os("HOME").map(std::path::PathBuf::from);
    let cache = runners::kubectl_cache(config_root).map_err(|why| why.to_string())?;
    crate::clusterpool::Kubectl::on_this_machine(path.as_deref(), home.as_deref())
        .map(|kubectl| kubectl.with_cache_dir(cache))
        .map_err(|why| why.to_string())
}

/// **[`before_cluster_link`], now, on this machine's `kubectl`, held to a time** (#4071): what a
/// cluster runner must pass, at the gate and again after Allow, before a value or a token goes
/// to it. A kubectl that is missing, or a cluster that does not answer within
/// [`VOUCHED_NOW_WITHIN`], is a refusal. Answers the uid of the runner's pod
/// ([`vouched_now_with`]).
pub fn vouched_now(config_root: &Path, name: &str) -> Result<String, crate::clusterpool::Refused> {
    let kubectl = kubectl_here(config_root)
        .map_err(crate::clusterpool::Refused::Other)?
        .within(VOUCHED_NOW_WITHIN);
    vouched_now_with(config_root, &kubectl, name, runners::now())
}

/// [`vouched_now`], at `now`, through `kubectl`: [`before_cluster_link`], then what the cluster
/// says of the runner beside the statement, for a value alone: a claim or pool stopped, or an
/// operator no longer saying it vouches, is refused ([`crate::clusterpool::Kubectl::serving`]).
/// Answers the uid of the runner's pod, which binds what the person allowed to that pod.
pub fn vouched_now_with(
    config_root: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    name: &str,
    now: u64,
) -> Result<String, crate::clusterpool::Refused> {
    use crate::clusterpool::Refused as Not;
    before_cluster_link(config_root, kubectl, name, now)?;
    let (pool, kept) = runners::cluster_runner_for(config_root, name)
        .map_err(|why| Not::Other(why.to_string()))?
        .ok_or_else(|| Not::Other(format!("no cluster pool named {name} is on this machine")))?;
    let kubectl = pool.kubectl(kubectl);
    kubectl.serving(&kept.claimed.namespace, &kept.claimed.name, pool.pool())?;
    kubectl.pod_uid(
        &crate::clusterpool::runner_namespace(&kept.claimed.requester),
        &kept.pod,
    )
}

/// **Whether the cluster pool `name`, kept as `cluster` with its runner `kept`, is shown to be
/// the person's own now** (#3819, #4164), for a profile to reach its runner or sign in there: the
/// gate a vault value or a token passes ([`crate::poollogins::Bound::of_a_cluster_pool`], #4071),
/// the person's mark, no chat run there without the sandbox, and its operator vouching for its
/// claim now, read again after. Answers the runner it judged, the only one anything then reaches
/// ([`cluster_runner_still_yours`]). Its cluster is asked only for a marked pool. The gate's own
/// reason goes to the log, so no word of the cluster's reaches the person.
pub fn cluster_runner_yours(
    config_root: &Path,
    name: &str,
    cluster: &runners::ClusterPool,
    kept: &runners::ClusterKept,
    vouch: &dyn Fn(&Path, &str) -> Result<String, crate::clusterpool::Refused>,
) -> Result<runners::ClusterKept, String> {
    if !cluster.owned() {
        return Err(format!(
            "pool {name} is not marked as your own, so purlis keeps no profile of yours on its \
             runner and signs none in there. Only you can mark it: turn on This is my own on its \
             row"
        ));
    }
    match crate::poollogins::Bound::of_a_cluster_pool(
        config_root,
        name,
        Some(&kept.reached()),
        vouch,
    ) {
        Ok(Some(_)) => Ok(kept.clone()),
        Ok(None) => Err(format!("no cluster pool named {name} is on this machine")),
        Err(why) => {
            tracing::info!("purlis: pool {name}'s runner keeps no profile now: {why}");
            Err(format!(
                "purlis could not confirm pool {name}'s runner as yours just now, so it kept no \
                 profile there and signed none in. Try again; if it stays so, look at the pool's \
                 row"
            ))
        }
    }
}

/// **The cluster pool `name` and its runner, still as the gate judged them** (#3819, #4164), read
/// again right before a link, a sign-in or a pasted code reaches it: the same runner `judged`,
/// still marked, and no chat run there without the sandbox since. The cluster is not asked again
/// here: a link's own check vouches for it before every connection, and a sign-in's before it
/// runs.
pub fn cluster_runner_still_yours(
    config_root: &Path,
    name: &str,
    judged: &runners::ClusterKept,
) -> Result<(runners::ClusterPool, runners::ClusterKept), String> {
    let changed = || {
        format!(
            "pool {name} or its runner changed since purlis confirmed it as yours, so nothing \
             of your profiles went there. Try again"
        )
    };
    let (cluster, kept) = runners::cluster_runner_for(config_root, name)
        .map_err(|why| why.to_string())?
        .ok_or_else(changed)?;
    if kept != *judged || !cluster.owned() {
        return Err(changed());
    }
    if crate::runnertaint::pool_ran_unsandboxed(config_root, name) {
        return Err(crate::runnertaint::sent_nothing_on(name));
    }
    Ok((cluster, kept))
}

/// **Whether the cluster pool `name`'s runner says `profile` is signed in** (#4164), by its own
/// `purlis profile sign-in --check` in the vouched pod ([`cluster_sign_in`]), for a sign-in with
/// no link to the runner to ask over. Refused where the check every link makes fails, or the
/// runner says neither, as for a profile it does not keep.
pub fn cluster_standing(
    config_root: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    name: &str,
    judged: &runners::ClusterKept,
    profile: &str,
) -> Result<crate::profilestore::sign_in::Standing, String> {
    let login = cluster_sign_in(
        config_root,
        kubectl,
        name,
        judged,
        profile,
        None,
        crate::dockerpool::Typed::Driven,
    )?;
    crate::dockerpool::standing_by_login(&login, profile)
}

/// **The runner's own sign-in of its profile `profile` on the cluster pool `name`'s runner**
/// (#3819), to `provider`'s plan where one is named, by `typed`: through `kubectl` pointed at
/// the pool's context, into the pod its pairing named, in its requester's runner namespace, as
/// every link to it is ([`Kubectl::connect_args`]), checked first as every link is
/// ([`before_cluster_link`]), and only to `judged`, the runner the caller's gate judged.
/// Refused, with nothing to run, where either fails.
pub fn cluster_sign_in(
    config_root: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    name: &str,
    judged: &runners::ClusterKept,
    profile: &str,
    provider: Option<&str>,
    typed: crate::dockerpool::Typed,
) -> Result<crate::profilestore::sign_in::Login, String> {
    let (kubectl, namespace, pod) = checked_runner(config_root, kubectl, name, judged)?;
    let (env, cleared) = kubectl.environment();
    let text = |value: std::ffi::OsString| {
        value
            .into_string()
            .map_err(|_| "kubectl's settings are not text purlis can pass on".to_owned())
    };
    Ok(crate::profilestore::sign_in::Login {
        program: text(kubectl.program().as_os_str().to_owned())?,
        args: kubectl.sign_in_args(&namespace, &pod, typed, profile, provider),
        env: env
            .into_iter()
            .map(|(name, value)| Ok((text(name)?, text(value)?)))
            .collect::<Result<_, String>>()?,
        cleared: cleared.into_iter().map(text).collect::<Result<_, _>>()?,
    })
}

/// The cluster pool `name`'s kubectl, pointed at its context, and its runner's namespace and pod,
/// once the check every link makes has passed ([`before_cluster_link`]), and only where the
/// runner it keeps now is `judged` (#3819): nothing is built from a runner kept since.
fn checked_runner(
    config_root: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    name: &str,
    judged: &runners::ClusterKept,
) -> Result<(crate::clusterpool::Kubectl, String, String), String> {
    before_cluster_link(config_root, kubectl, name, runners::now())
        .map_err(|why| why.to_string())?;
    let (pool, kept) = runners::cluster_runner_for(config_root, name)
        .map_err(|why| why.to_string())?
        .ok_or_else(|| format!("no cluster pool named {name} is on this machine"))?;
    if kept != *judged {
        return Err(format!(
            "pool {name}'s runner changed since purlis confirmed it as yours, so nothing ran there"
        ));
    }
    Ok((
        pool.kubectl(kubectl),
        crate::clusterpool::runner_namespace(&kept.claimed.requester),
        kept.pod,
    ))
}

/// **The version the cluster pool `name`'s runner's `purlis` says it is** (#3819), asked over
/// `kubectl exec` after the link's check, held to [`VOUCHED_NOW_WITHIN`]: a driven sign-in
/// runs only on a runner of this desktop's purlis (#4043).
pub fn cluster_runner_version(
    config_root: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    name: &str,
    judged: &runners::ClusterKept,
) -> Result<String, String> {
    let (kubectl, namespace, pod) = checked_runner(config_root, kubectl, name, judged)?;
    let args = kubectl.purlis_args(&namespace, &pod, &["--version"]);
    let said = kubectl
        .within(VOUCHED_NOW_WITHIN)
        .run(&args, None, crate::runnerlink::MOST_LINE)
        .map_err(|why| why.to_string())?;
    crate::dockerpool::version_in(&said)
        .map(str::to_owned)
        .ok_or_else(|| format!("pool {name}'s runner did not say its version as purlis does"))
}

/// **Runs the cluster pool `name`'s runner's own sign-out of its profile `profile`** (#3819),
/// over `kubectl exec` after the link's check, given the time a Docker runner's sign-out has.
pub fn cluster_sign_out(
    config_root: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    name: &str,
    judged: &runners::ClusterKept,
    profile: &str,
) -> Result<(), String> {
    let (kubectl, namespace, pod) = checked_runner(config_root, kubectl, name, judged)?;
    let args = kubectl.purlis_args(&namespace, &pod, &["profile", "sign-out", profile]);
    kubectl
        .within(crate::dockerpool::SIGN_OUT_WITHIN)
        .run(&args, None, 64 * 1024)
        .map(drop)
        .map_err(|why| why.to_string())
}

/// **Whether the cluster pool `name`'s runner keeps its home past its pod** (#3819, spec #4023
/// Q6): its claim, as the cluster holds it now, is a kept one, so a sign-in there outlives the
/// pod; a fresh one's lasts only for that claim. Asked on this machine's `kubectl`, held to
/// [`VOUCHED_NOW_WITHIN`].
pub fn claim_keeps_its_home(config_root: &Path, name: &str) -> Result<bool, String> {
    let kubectl = kubectl_here(config_root)?.within(VOUCHED_NOW_WITHIN);
    claim_keeps_its_home_with(config_root, &kubectl, name)
}

/// [`claim_keeps_its_home`], on `kubectl`: only the claim this desktop kept, by its uid, is read.
/// Refused, in words, where the cluster cannot say.
pub fn claim_keeps_its_home_with(
    config_root: &Path,
    kubectl: &crate::clusterpool::Kubectl,
    name: &str,
) -> Result<bool, String> {
    let (pool, kept) = runners::cluster_runner_for(config_root, name)
        .map_err(|why| why.to_string())?
        .ok_or_else(|| format!("no cluster pool named {name} is on this machine"))?;
    let seen = pool
        .kubectl(kubectl)
        .claim_seen(&kept.claimed.namespace, &kept.claimed.name)
        .map_err(|why| why.to_string())?
        .filter(|seen| seen.uid == kept.claimed.uid)
        .ok_or_else(|| {
            format!("pool {name}'s cluster no longer holds the claim this desktop made")
        })?;
    Ok(seen.kept)
}

/// What a cluster runner's removal is handed to take this desktop out of its peers over its
/// link, where that link is up.
pub type UnpairCluster<'a> =
    dyn Fn(&runners::ClusterPool, &runners::ClusterKept) -> Result<(), String> + 'a;

/// **Removes the cluster pool `name`** (#3292), on the desktop whose device id is `device`:
///
/// 1. once an add of it still under way has finished, this desktop out of its runner's peers
///    with `unpair`, where given, before the store is locked, since a link's own check may keep
///    the pool's pin;
/// 2. its claim deleted through `kubectl` (none where this machine has none), and with it its
///    runner, only where the claim is the one this desktop keeps (its uid) or, where no runner
///    was kept, one marked as this desktop's and asked for by who this kubeconfig is;
/// 3. the pool, its runner, its copy of the context, and every workspace's choice of it, all
///    forgotten here whatever the cluster answered, so a pool never stays stuck.
///
/// What may be left in the cluster is answered in one sentence, with how to delete it; nothing
/// where nothing is.
pub fn remove_cluster(
    config_root: &Path,
    kubectl: Option<&crate::clusterpool::Kubectl>,
    name: &str,
    device: Option<&str>,
    unpair: Option<&UnpairCluster<'_>>,
) -> Result<Option<String>, Refused> {
    // An add of it still under way finishes first.
    runners::usable_pool_name(name).map_err(Refused::Other)?;
    let _adding = adding(config_root, name);
    let pool = runners::cluster_pools(config_root)?
        .into_iter()
        .find(|pool| pool.name() == name)
        .ok_or_else(|| Refused::Other(format!("no pool named {name} is on this machine")))?;
    let kept = runners::cluster_runners(config_root)?
        .into_iter()
        .find(|kept| kept.name == name);
    if let (Some(kept), Some(unpair)) = (&kept, unpair)
        && let Err(why) = unpair(&pool, kept)
    {
        // Its peers go with its claim's volume, so a runner that does not answer is no reason
        // to stop.
        tracing::info!("purlis: pool {name}'s runner was not unpaired: {why}");
    }
    let left = claim_released(&pool, kept.as_ref(), kubectl, device);
    {
        let locked = runners::lock(config_root);
        runners::forget_cluster_runner_locked(config_root, name, &locked)?;
        runners::forget_pool_locked(config_root, name, &locked)?;
    }
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
    Ok(left)
}

/// **Releases the claim of `pool`'s runner** ([`remove_cluster`]): what may be left in the
/// cluster, in one sentence with how to delete it; `None` where nothing is.
fn claim_released(
    pool: &runners::ClusterPool,
    kept: Option<&runners::ClusterKept>,
    kubectl: Option<&crate::clusterpool::Kubectl>,
    device: Option<&str>,
) -> Option<String> {
    let (namespace, claim) = match (kept, device) {
        (Some(kept), _) => (kept.claimed.namespace.clone(), kept.claimed.name.clone()),
        (None, Some(device)) => (pool.namespace().to_owned(), claim_name(device, pool.name())),
        (None, None) => {
            return Some(format!(
                "Pool {} was removed here, but a claim of it may still hold a runner in the \
                 cluster (purlis could not read this desktop's device id, so it could not tell \
                 which claim is its own); list them with kubectl get claims.runners.purlis.dev \
                 --namespace {} and delete only the one this desktop made, which starts with the \
                 pool's name: other desktops' claims of the pool start with it too.",
                pool.name(),
                pool.namespace()
            ));
        }
    };
    let left = |why: &str| {
        Some(format!(
            "Pool {} was removed here, but its claim {claim} in namespace {namespace} may still \
             hold a runner in the cluster ({why}); delete it with kubectl delete \
             claims.runners.purlis.dev {claim} --namespace {namespace}.",
            pool.name()
        ))
    };
    let Some(kubectl) = kubectl else {
        return left("purlis found no kubectl on this machine");
    };
    if !pool.kubeconfig().is_file() {
        return left("the pool's copy of its kubeconfig's context is gone");
    }
    let kubectl = pool.kubectl(kubectl);
    let seen = match kubectl.claim_seen(&namespace, &claim) {
        Ok(None) => return None,
        Ok(Some(seen)) => seen,
        Err(why) => return left(&why.to_string()),
    };
    let ours = match (kept, device) {
        (Some(kept), _) => seen.uid == kept.claimed.uid,
        (None, Some(device)) => {
            seen.desktop.as_deref() == Some(crate::clusterpool::desktop_label(device).as_str())
                && kubectl
                    .whoami()
                    .is_ok_and(|me| seen.requester.as_deref() == Some(me.as_str()))
        }
        (None, None) => false,
    };
    if !ours {
        return left("it is not the claim this desktop made, so purlis left it");
    }
    match kubectl.delete_claim(&namespace, &claim) {
        Ok(()) => None,
        Err(why) => left(&why.to_string()),
    }
}

#[cfg(test)]
mod tests;
