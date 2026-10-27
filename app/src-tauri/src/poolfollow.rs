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
use tauri::Runtime;

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
            tokio::time::sleep(purlis_core::updates::CHECK_EVERY).await;
        }
    });
}

/// Not on this platform: the Docker pool runs on macOS and Linux alone.
#[cfg(not(unix))]
pub fn watch<R: tauri::Runtime>(_app: &tauri::AppHandle<R>) {}

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
    let config = config.to_path_buf();
    let done = crate::off_the_window("moving pools to this version", move || {
        let machine = Machine::here(&config)?;
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
}

/// Whether the last look found a runner image named for this version: `None` until a look
/// answered.
#[cfg(unix)]
static NAMED: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);

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
        Ok(Found::Named(_)) => true,
        Ok(Found::NoneNamed) => false,
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
    *NAMED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// **The runner image the update manifest names for this version**, or none where the
/// channel's manifest is of another version or names none (a release from before #2749).
/// One updater read, listed in the network log as every read is.
#[cfg(unix)]
async fn runner_image<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<Found, String> {
    use tauri_plugin_updater::UpdaterExt;
    let endpoint = crate::updates::channel_now().endpoint();
    let url = endpoint
        .parse()
        .map_err(|why| format!("purlis's update endpoint is not a url: {why}"))?;
    let ours = app.package_info().version.clone();
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|why| format!("purlis's update endpoint was refused: {why}"))?
        // The manifest of this very version is the one asked for, not a newer one.
        .version_comparator(move |current, remote| remote.version == current)
        .build()
        .map_err(|why| format!("purlis's updater could not be built: {why}"))?;
    let started = std::time::Instant::now();
    let found = updater.check().await;
    purlis_core::netlog::updater_read(&endpoint, found.is_ok(), started.elapsed());
    let Some(update) = found.map_err(|why| why.to_string())? else {
        return Ok(Found::OtherVersion);
    };
    let pubkey = app
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("pubkey"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    found_in(Some(&update.raw_json), &pubkey, &ours.to_string())
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
        // A suspended runner runs no chat (#2051): it is not woken to be asked.
        let suspended = matches!(machine.docker.running(&kept.made), Ok(false));
        let chats = if suspended { Ok(0) } else { chats_on(&to) };
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
        said.push(
            match purlis_core::pools::drain(
                &machine.config,
                &machine.docker,
                kept.name(),
                Some(&unpair),
            ) {
                Ok(removed) => match removed.left() {
                    None => format!(
                        "draining runner {} had no chat left and is removed",
                        kept.name()
                    ),
                    Some(left) => {
                        format!("draining runner {} is not all removed: {left}", kept.name())
                    }
                },
                Err(why) => format!("draining runner {} was not removed: {why}", kept.name()),
            },
        );
    }
    said
}

#[cfg(all(test, unix))]
mod tests;
