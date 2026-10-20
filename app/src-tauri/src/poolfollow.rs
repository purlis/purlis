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
    let image = match runner_image(app).await {
        Ok(image) => image,
        Err(why) => {
            tracing::warn!("purlis: this version's runner image could not be read: {why}");
            None
        }
    };
    let config = config.to_path_buf();
    let done = crate::off_the_window("moving pools to this version", move || {
        let machine = Machine::here(&config)?;
        Ok::<_, String>(follow_and_drain(&machine, image.as_ref(), &|to| {
            crate::runnerlinks::runs_a_chat(to)
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

/// **The runner image the update manifest names for this version**, or none where the
/// channel's manifest is of another version or names none (a release from before #2749).
/// One updater read, listed in the network log as every read is.
#[cfg(unix)]
async fn runner_image<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<Option<Image>, String> {
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
        return Ok(None);
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
    purlis_core::runnerimage::of_this_version(&update.raw_json, &pubkey, &ours.to_string())
}

/// **Moves each pool of purlis's own image on `machine` to `image`, and removes each draining
/// runner `runs_a_chat` says no chat is left on**: what was done, and what failed, a line each.
/// With no `image`, only the draining runners.
#[cfg(unix)]
pub fn follow_and_drain(
    machine: &Machine,
    image: Option<&Image>,
    runs_a_chat: &dyn Fn(&ToARunner) -> Result<bool, String>,
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
        match runs_a_chat(&to) {
            Ok(true) => continue,
            Ok(false) => {}
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
