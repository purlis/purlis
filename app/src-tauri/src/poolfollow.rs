// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **After an update, a Docker pool's runner follows the desktop's version** (#2749, the
//! person's ruling of 2026-10-20).
//!
//! A while after each launch, and as often as the updater looks, the window reads its channel's
//! update manifest for **its own** version and takes the runner image it names
//! ([`purlis_core::runnerimage::of_this_version`]): signed by the updater's key over its line,
//! for this version, purlis's own image by digest. Then, on this machine:
//!
//! - **each pool of purlis's own image gets a runner of that image beside its old one**
//!   ([`purlis_core::pools::follow`]): new chats start on it, and the old one drains;
//! - **each draining runner with no chat left is removed** ([`purlis_core::pools::drain`]),
//!   unpaired first over a link of its own;
//! - **a pool of a team's own image is never moved**: its row says its runner is behind
//!   (`crate::dockerpools::docker_pool_version`).
//!
//! What fails is said in the app's log and tried again at the next look; nothing here asks the
//! person anything. A test build and a development build never look, as the updater does not.

#[cfg(unix)]
use purlis_core::dockerpool::Image;
#[cfg(unix)]
use purlis_core::pools::DrainLook;
#[cfg(unix)]
use tauri::Runtime;

#[cfg(unix)]
use crate::dockerpools::RunnerImageOffered;
#[cfg(unix)]
use crate::runnerlinks::{Machine, ToARunner};

/// Starts the looks, where this build looks for updates on its own.
#[cfg(unix)]
pub fn watch<R: Runtime>(app: &tauri::AppHandle<R>) {
    if !purlis_core::updates::checks_on_its_own(purlis_core::fence::FENCED, cfg!(debug_assertions))
    {
        return;
    }
    let Some(config) = purlis_core::machine::config_root() else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(purlis_core::updates::FIRST_CHECK_AFTER).await;
        loop {
            look(&app, &config).await;
            // A runner that drains goes soon after its last chat ends, not at the next look for
            // a new version (#4248): this asks only the runner, never the network.
            let until = std::time::Instant::now() + purlis_core::updates::CHECK_EVERY;
            while std::time::Instant::now() < until {
                tokio::time::sleep(DRAINS_LOOKED_AT_EVERY).await;
                drained(&app, &config).await;
            }
        }
    });
}

/// Not on this platform: the Docker pool runs on macOS and Linux alone.
#[cfg(not(unix))]
pub fn watch<R: tauri::Runtime>(_app: &tauri::AppHandle<R>) {}

/// How often each draining runner is asked whether its last chat has ended.
#[cfg(unix)]
const DRAINS_LOOKED_AT_EVERY: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Each draining runner with no chat left goes; nothing else is looked at.
#[cfg(unix)]
async fn drained<R: Runtime>(app: &tauri::AppHandle<R>, config: &std::path::Path) {
    if purlis_core::runners::draining(config).is_ok_and(|draining| draining.is_empty()) {
        return;
    }
    let looked_in = config.to_path_buf();
    let done = crate::off_the_window("removing runners that have drained", move || {
        let machine = Machine::here(&looked_in)?;
        Ok::<_, String>(follow_and_drain(&machine, None, &|to| {
            crate::runnerlinks::chats_on(to)
        }))
    })
    .await;
    links_to_gone_runners_end(app, config);
    match done {
        Ok(said) => {
            for line in said {
                tracing::warn!("purlis: {line}");
            }
        }
        Err(why) => tracing::warn!("purlis: the draining runners were not looked at: {why}"),
    }
}

/// One look: this version's runner image, then each pool and each draining runner.
#[cfg(unix)]
async fn look<R: Runtime>(app: &tauri::AppHandle<R>, config: &std::path::Path) {
    // Nothing to do on a machine that keeps no pool, and nothing is asked of the network.
    if purlis_core::runners::pools(config).is_ok_and(|pools| pools.is_empty())
        && purlis_core::runners::draining(config).is_ok_and(|draining| draining.is_empty())
    {
        return;
    }
    let found = runner_image(app).await;
    note_named(&found);
    let image = match found {
        Ok(Found::Named(image)) => Some(image),
        Ok(Found::NoneNamed | Found::OtherVersion) => None,
        Err(why) => {
            tracing::warn!("purlis: this version's runner image could not be read: {why}");
            None
        }
    };
    let looked_in = config.to_path_buf();
    let done = crate::off_the_window("moving pools to this version", move || {
        let machine = Machine::here(&looked_in)?;
        Ok::<_, String>(follow_and_drain(&machine, image.as_ref(), &|to| {
            crate::runnerlinks::chats_on(to)
        }))
    })
    .await;
    match done {
        Ok(said) => {
            for line in said {
                tracing::warn!("purlis: {line}");
            }
        }
        Err(why) => tracing::warn!("purlis: the pools could not be moved to this version: {why}"),
    }
    links_to_gone_runners_end(app, config);
}

/// The window's links to a runner kept no more end, so its chats say they ended (#4168).
#[cfg(unix)]
fn links_to_gone_runners_end<R: Runtime>(app: &tauri::AppHandle<R>, config: &std::path::Path) {
    if let Some(host) = tauri::Manager::try_state::<crate::sessionhost::SessionHost>(app) {
        host.connection().links_to_gone_runners_end(config);
    }
}

/// What the last look found of this version's runner image: `None` until a look answered,
/// then the image it named, or none where it named none.
#[cfg(unix)]
static NAMED: std::sync::Mutex<Option<Option<Image>>> = std::sync::Mutex::new(None);

/// **What a look found of this version's runner image** (#2880).
#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// The channel's manifest is of another version than this desktop's: one behind or ahead
    /// of its channel. Nothing is said of this version's release.
    OtherVersion,
    /// This version's manifest names no runner image, as a release whose image failed does.
    NoneNamed,
    /// This version's manifest names it.
    Named(Image),
}

/// **What `manifest`, the channel's update manifest as the updater read it, says of the runner
/// image of `version`** (#2880), checked with the updater key `pubkey`: none read, or one of
/// another version, says nothing of it.
#[cfg(unix)]
pub fn found_in(
    manifest: Option<&serde_json::Value>,
    pubkey: &str,
    version: &str,
) -> Result<Found, String> {
    let Some(manifest) = manifest else {
        return Ok(Found::OtherVersion);
    };
    if manifest.get("version").and_then(serde_json::Value::as_str) != Some(version) {
        return Ok(Found::OtherVersion);
    }
    Ok(
        match purlis_core::runnerimage::of_this_version(manifest, pubkey, version)? {
            Some(image) => Found::Named(image),
            None => Found::NoneNamed,
        },
    )
}

/// **Notes whether this version's manifest named a runner image** (#2880), from a look's
/// answer: only a manifest of this very version says so. One that names none, as a release
/// whose runner image failed does, leaves the pools where they are, and the Pools group says they
/// do not move to this version. A manifest of another version, or a look that could not be made,
/// notes nothing.
#[cfg(unix)]
pub fn note_named(found: &Result<Found, String>) {
    let named = match found {
        Ok(Found::Named(image)) => Some(image.clone()),
        Ok(Found::NoneNamed) => None,
        Ok(Found::OtherVersion) | Err(_) => return,
    };
    *NAMED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(named);
}

/// **Whether this version's release named a runner image**, as the last look found it: `None`
/// until a look answered (#2880).
#[cfg(unix)]
pub fn this_versions_image_named() -> Option<bool> {
    NAMED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map(Option::is_some)
}

/// **The runner image the last look found named for this version**: none until one did.
#[cfg(unix)]
pub fn this_versions_image_noted() -> Option<Image> {
    NAMED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .flatten()
}

/// **What the Add a pool dialog is told of what the manifest offers** (the person's ruling of
/// 2026-10-30): this version's image, a newer version's, or none; and why, where it could not
/// be read or was refused.
#[cfg(unix)]
pub fn said(
    offered: Result<purlis_core::runnerimage::Offered, String>,
) -> Result<RunnerImageOffered, String> {
    use purlis_core::runnerimage::Offered;
    Ok(match offered? {
        Offered::Ours(image) => RunnerImageOffered::Ours {
            image: image.as_str().to_owned(),
        },
        Offered::Newer { image, version } => RunnerImageOffered::Newer {
            image: image.as_str().to_owned(),
            version,
        },
        Offered::None => RunnerImageOffered::NoneNamed,
    })
}

/// **The runner image a new pool is offered** (the person's ruling of 2026-10-30): what the Add
/// a pool dialog's Image box shows as its default.
///
/// - A build that never looks for updates, a local build with no update channel, is offered
///   none, and nothing is read.
/// - The image a look found named for this version is not read again.
/// - On the stable channel, this version's own release's manifest is read
///   ([`purlis_core::updates::Channel::release_endpoint`]), so a machine with an update waiting
///   is still offered its own version's image. Where that release has no manifest, or it
///   cannot be read, the channel's is.
/// - On the dev channel, whose one manifest is always its newest build's, the channel's is
///   read: of this version, its image; of a newer one, that version's, said to be so, with an
///   update of purlis to it waiting.
///
/// Every image offered is checked as a look checks this version's
/// ([`purlis_core::runnerimage::offered`]): signed by the updater's key over its line, for the
/// version the manifest names, purlis's own image by digest.
#[cfg(unix)]
pub async fn image_offered<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<RunnerImageOffered, String> {
    if !purlis_core::updates::checks_on_its_own(purlis_core::fence::FENCED, cfg!(debug_assertions))
    {
        return Ok(RunnerImageOffered::LocalBuild);
    }
    if let Some(image) = this_versions_image_noted() {
        return Ok(RunnerImageOffered::Ours {
            image: image.as_str().to_owned(),
        });
    }
    let ours = app.package_info().version.to_string();
    let manifest = crate::updates::manifest_for_a_new_pool(app).await?;
    let offered = purlis_core::runnerimage::offered(
        &manifest,
        &crate::updates::configured_pubkey(app),
        &ours,
    );
    if let Ok(purlis_core::runnerimage::Offered::Ours(image)) = &offered {
        note_named(&Ok(Found::Named(image.clone())));
    }
    said(offered)
}

/// **The runner image the update manifest names for this version**, or none where the
/// channel's manifest is of another version or names none (a release from before #2749).
/// One updater read ([`crate::updates::this_versions_manifest`]).
#[cfg(unix)]
async fn runner_image<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<Found, String> {
    let ours = app.package_info().version.to_string();
    let manifest = crate::updates::this_versions_manifest(app).await?;
    found_in(
        manifest.as_ref(),
        &crate::updates::configured_pubkey(app),
        &ours,
    )
}

/// **How many chats each pool's draining runners still keep, as the last look found them**
/// (#2865): what the Pools group says a pool's older runner waits on.
#[cfg(unix)]
pub fn waiting_on(config: &std::path::Path, pool: &str) -> u32 {
    lock_waiting()
        .iter()
        .filter(|((home, of, _), _)| home == config && of == pool)
        .map(|(_, chats)| *chats)
        .sum()
}

/// **What pool `pool`'s draining runners waited on is let go of** (#2865): the pool is removed,
/// its runners with it.
#[cfg(unix)]
pub fn forget_waiting(config: &std::path::Path, pool: &str) {
    lock_waiting().retain(|(home, of, _), _| !(home == config && of == pool));
}

/// A draining runner, by its config home, its pool and its name.
#[cfg(unix)]
type Waiting = (std::path::PathBuf, String, String);

#[cfg(unix)]
fn lock_waiting() -> std::sync::MutexGuard<'static, std::collections::BTreeMap<Waiting, u32>> {
    static WAITING: std::sync::Mutex<std::collections::BTreeMap<Waiting, u32>> =
        std::sync::Mutex::new(std::collections::BTreeMap::new());
    WAITING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// **Moves each pool of purlis's own image on `machine` to `image`, and removes each draining
/// runner `chats_on` says no chat is left on**: what was done, and what failed, a line each.
/// With no `image`, only the draining runners. How many chats each one still keeps is noted for
/// the Pools group ([`waiting_on`]).
#[cfg(unix)]
pub fn follow_and_drain(
    machine: &Machine,
    image: Option<&Image>,
    chats_on: &dyn Fn(&ToARunner) -> Result<u32, String>,
) -> Vec<String> {
    let mut said = Vec::new();
    if let Some(image) = image {
        let pools = purlis_core::runners::pools(&machine.config).unwrap_or_default();
        let desktop = machine.desktop().map(|desktop| desktop.pinned());
        for pool in pools
            .iter()
            .filter(|pool| pool.image() != image && pool.image().is_purlis_own())
        {
            let moved = desktop.clone().and_then(|desktop| {
                purlis_core::pools::follow(
                    &machine.config,
                    &machine.docker,
                    pool.name(),
                    image,
                    &desktop,
                )
                .map_err(|why| why.to_string())
            });
            said.push(match moved {
                Ok(Some(kept)) => format!(
                    "pool {} starts its new chats on runner {}, of this version; its older \
                     runner keeps its chats until they end",
                    pool.name(),
                    kept.name()
                ),
                Ok(None) => continue,
                Err(why) => format!(
                    "pool {} stays on its runner, which is not of this version: {why}",
                    pool.name()
                ),
            });
        }
    }
    for kept in purlis_core::runners::draining(&machine.config).unwrap_or_default() {
        // Its drain began and only its home is left, so nothing is left to ask: only its removal
        // is tried again (#4298). Its home listed here says this is its engine: with no engine
        // kept, a Docker reaching another one lists no such volume, and the row stays (#4149).
        if (kept.carried || kept.carry_failed)
            && matches!(
                machine.docker.gone(&kept.made),
                Ok(Some(purlis_core::dockerpool::Gone { home_kept: true }))
            )
        {
            lock_waiting().remove(&(
                machine.config.clone(),
                kept.pool().to_owned(),
                kept.name().to_owned(),
            ));
            said.push(said_of_a_drain(
                kept.name(),
                purlis_core::pools::drain(&machine.config, &machine.docker, kept.name(), None),
            ));
            continue;
        }
        // Removed on the word of the engine that made it (#4168), it holds no chat and cannot be
        // asked. A runner kept before its engine was has no such word: it stays as below.
        if kept.engine.is_some()
            && matches!(
                machine.docker.missing(&kept.made, kept.engine.as_deref()),
                Ok(Some(purlis_core::dockerpool::Missing::Removed(_)))
            )
        {
            lock_waiting().remove(&(
                machine.config.clone(),
                kept.pool().to_owned(),
                kept.name().to_owned(),
            ));
            said.push(
                match purlis_core::pools::drain_removed(
                    &machine.config,
                    &machine.docker,
                    kept.name(),
                ) {
                    Ok(removed) => match removed.left() {
                        None => format!(
                            "draining runner {} was removed outside purlis, so it is forgotten \
                             and its chats ended with it",
                            kept.name()
                        ),
                        Some(left) => {
                            format!("draining runner {} is not all removed: {left}", kept.name())
                        }
                    },
                    Err(why) => format!("draining runner {} was not removed: {why}", kept.name()),
                },
            );
            continue;
        }
        let to = match ToARunner::of_kept(machine, &kept) {
            Ok(to) => to,
            Err(why) => {
                said.push(format!(
                    "draining runner {} was not asked: {why}",
                    kept.name()
                ));
                continue;
            }
        };
        let at = (
            machine.config.clone(),
            kept.pool().to_owned(),
            kept.name().to_owned(),
        );
        // Nothing is woken to be asked (#2051). One Docker no longer lists is not taken as idle
        // on its own (#4149): Docker may only be pointing at another engine, and its chats run
        // on where it is. One that answered its unpairing cannot be asked, and takes no chat
        // (#4146): only its removal is left.
        let look = machine
            .ran(&kept)
            .map(|ran| purlis_core::pools::drain_look(&kept, ran));
        let chats = match look {
            _ if kept.unpaired => Ok(0),
            Ok(DrainLook::Held) => {
                lock_waiting().remove(&at);
                said.push(format!(
                    "draining runner {} stopped during its drain, so it stays with its home \
                     until it is removed in Devices",
                    kept.name()
                ));
                continue;
            }
            Ok(DrainLook::NoChat) => Ok(0),
            Ok(DrainLook::Ask) | Err(_) => chats_on(&to),
        };
        match chats {
            Ok(0) => {
                lock_waiting().remove(&at);
            }
            Ok(chats) => {
                lock_waiting().insert(at, chats);
                continue;
            }
            // Not known to be idle, so it stays until the next look.
            Err(why) => {
                said.push(format!(
                    "draining runner {} stays until it answers: {why}",
                    kept.name()
                ));
                continue;
            }
        }
        let unpair = |_: &purlis_core::runners::Kept| crate::runnerlinks::unpair(&to);
        said.push(said_of_a_drain(
            kept.name(),
            purlis_core::pools::drain(&machine.config, &machine.docker, kept.name(), Some(&unpair)),
        ));
    }
    said
}

/// The line a look says of removing the draining runner `name`.
#[cfg(unix)]
fn said_of_a_drain(
    name: &str,
    removed: Result<purlis_core::runners::Removed, purlis_core::runners::Refused>,
) -> String {
    match removed {
        Ok(removed) => match removed.left() {
            None => format!("draining runner {name} had no chat left and is removed"),
            Some(left) => format!("draining runner {name} is not all removed: {left}"),
        },
        Err(why) => format!("draining runner {name} was not removed: {why}"),
    }
}

#[cfg(all(test, unix))]
mod tests;
