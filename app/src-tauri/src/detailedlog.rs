// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Settings › Diagnostics' **Detailed remote log for one hour** (#7864): the window's two
//! commands over `purlis_core::detailedlog`'s switch, and the watcher that turns it off when its
//! hour is up by the wall clock, whether or not a window asks.

use std::sync::atomic::{AtomicBool, Ordering};

use purlis_core::detailedlog::{self, Detailed};

/// What the switch says, as the window reads it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct DetailedRemoteLog {
    /// When it turns itself off, in RFC 3339, while it is on; `null` while it is off.
    pub until: Option<String>,
}

impl From<Detailed> for DetailedRemoteLog {
    fn from(said: Detailed) -> Self {
        Self {
            until: said.until.map(|until| until.to_string()),
        }
    }
}

/// Whether the detailed remote log is on, and until when.
#[tauri::command]
#[specta::specta]
pub fn detailed_remote_log() -> DetailedRemoteLog {
    detailedlog::this_process()
        .at(jiff::Timestamp::now())
        .into()
}

/// Turns the detailed remote log on, for an hour from now, or off now.
#[tauri::command]
#[specta::specta]
pub fn set_detailed_remote_log(on: bool) -> DetailedRemoteLog {
    let switch = detailedlog::this_process();
    if !on {
        return switch.turn_off().into();
    }
    let said = switch.turn_on(jiff::Timestamp::now());
    watch_until_it_is_off();
    said.into()
}

/// How often the watcher reads the wall clock while the switch is on.
const LOOKS_EVERY: std::time::Duration = std::time::Duration::from_secs(60);

/// Whether a watcher runs.
static WATCHING: AtomicBool = AtomicBool::new(false);

/// **Turns the switch off once its hour is up, whether or not a window asks**: one thread that
/// reads the wall clock every minute while it is on. Not one sleep of an hour: a sleep's clock
/// stops while the machine sleeps, and the hour is the person's clock's.
fn watch_until_it_is_off() {
    watch_with(&WATCHING, |watch| {
        std::thread::Builder::new()
            .name("purlis-detailed-log".into())
            .spawn(watch)
            .map(drop)
    });
}

/// Starts the watcher with `spawn`, unless `watching` says one runs. **A watcher that never
/// started is not running**: `watching` is put back where `spawn` fails, so the next turn on
/// starts one, and the switch is never left on for good with nobody to turn it off.
fn watch_with(
    watching: &'static AtomicBool,
    spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<()>,
) {
    if watching.swap(true, Ordering::SeqCst) {
        return;
    }
    let watch = Box::new(move || {
        loop {
            std::thread::sleep(LOOKS_EVERY);
            if detailedlog::this_process()
                .at(jiff::Timestamp::now())
                .until
                .is_some()
            {
                continue;
            }
            watching.store(false, Ordering::SeqCst);
            // Turned on again between the look and the store: watch on, unless another
            // watcher has started since.
            let on_again = detailedlog::this_process()
                .at(jiff::Timestamp::now())
                .until
                .is_some();
            if !on_again || watching.swap(true, Ordering::SeqCst) {
                return;
            }
        }
    });
    if let Err(why) = spawn(watch) {
        watching.store(false, Ordering::SeqCst);
        tracing::warn!(
            "purlis: the detailed remote log's watcher did not start ({why}): it turns off when \
             Settings next reads it after its hour"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_watcher_that_could_not_start_is_started_by_the_next_turn_on() {
        static WATCHING: AtomicBool = AtomicBool::new(false);
        watch_with(&WATCHING, |_| Err(std::io::Error::other("no threads left")));
        assert!(!WATCHING.load(Ordering::SeqCst), "nothing watches");

        let mut started = false;
        watch_with(&WATCHING, |_| {
            started = true;
            Ok(())
        });
        assert!(started, "the next turn on starts one");
        assert!(WATCHING.load(Ordering::SeqCst));
    }

    #[test]
    fn a_second_turn_on_while_one_watches_starts_no_other() {
        static WATCHING: AtomicBool = AtomicBool::new(false);
        watch_with(&WATCHING, |_| Ok(()));
        let mut started = false;
        watch_with(&WATCHING, |_| {
            started = true;
            Ok(())
        });
        assert!(!started);
    }
}
