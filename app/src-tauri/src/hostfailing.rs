// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The window says its session host is failing** (#2007, ADR 0068 §3): the window's
//! supervisor starts its host again after each unexpected exit, and after three within five
//! minutes its breaker trips and it stops. Then this holds what the window says, with the last
//! lines the host wrote to its log, and **Try again**, which starts it once more. Until the host
//! runs, the window draws the plane and starts no chat, and each start says why.

use std::sync::{Arc, Mutex, PoisonError};

/// The event the window is sent when the breaker trips, and again, with nothing, when the host
/// is tried again.
pub const FAILING: &str = "session-host-failing";

/// The session host is failing: the window's supervisor stopped starting it again.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct Failing {
    /// How many unexpected exits tripped the breaker.
    pub exits: u32,
    /// Within how many minutes.
    pub within_minutes: u32,
    /// The last lines the host wrote to its log, oldest first.
    pub said: Vec<String>,
}

type TryAgain = Arc<dyn Fn() + Send + Sync>;
type Tell = Arc<dyn Fn(Option<Failing>) + Send + Sync>;

/// Whether the session host is failing now, what tries it again, and who is told.
#[derive(Default)]
pub struct HostFailing {
    now: Mutex<Option<Failing>>,
    try_again: Mutex<Option<TryAgain>>,
    tell: Mutex<Option<Tell>>,
}

impl HostFailing {
    /// The supervisor's breaker tripped: the window is told.
    pub fn failing(&self, failing: Failing) {
        *lock(&self.now) = Some(failing.clone());
        self.told(Some(failing));
    }

    /// The host is being tried again: the window is told it is not failing any more.
    pub fn not_failing(&self) {
        if lock(&self.now).take().is_some() {
            self.told(None);
        }
    }

    /// What the window is told the host's failing is, now.
    pub fn now(&self) -> Option<Failing> {
        lock(&self.now).clone()
    }

    /// What Try again does: the supervisor's.
    pub fn tried_again_by(&self, try_again: TryAgain) {
        *lock(&self.try_again) = Some(try_again);
    }

    /// Who is told each change: the window.
    pub fn telling(&self, tell: Tell) {
        *lock(&self.tell) = Some(tell);
    }

    /// The person pressed Try again: refused where there is nothing to try again.
    pub fn try_again(&self) -> Result<(), String> {
        if lock(&self.now).is_none() {
            return Err(
                "purlis's session host is not failing, so there is nothing to try again".into(),
            );
        }
        let try_again = lock(&self.try_again).clone();
        match try_again {
            Some(try_again) => {
                try_again();
                Ok(())
            }
            None => Err("purlis's session host cannot be started from this window".into()),
        }
    }

    fn told(&self, failing: Option<Failing>) {
        let tell = lock(&self.tell).clone();
        if let Some(tell) = tell {
            tell(failing);
        }
    }
}

/// Whether the session host is failing, and what the window says of it.
#[tauri::command]
#[specta::specta]
pub fn session_host_failing(failing: tauri::State<'_, Arc<HostFailing>>) -> Option<Failing> {
    failing.now()
}

/// **Try again**: the supervisor starts the session host once more, its breaker's count
/// started over.
#[tauri::command]
#[specta::specta]
pub fn start_the_session_host_again(
    failing: tauri::State<'_, Arc<HostFailing>>,
) -> Result<(), String> {
    failing.try_again()
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three_exits() -> Failing {
        Failing {
            exits: 3,
            within_minutes: 5,
            said: vec!["purlisd: the last line".into()],
        }
    }

    #[test]
    fn the_window_is_told_when_the_host_fails_and_when_it_is_tried_again() {
        let failing = HostFailing::default();
        let told = Arc::new(Mutex::new(Vec::new()));
        failing.telling({
            let told = Arc::clone(&told);
            Arc::new(move |now| told.lock().unwrap().push(now))
        });
        let tried = Arc::new(Mutex::new(0));
        failing.tried_again_by({
            let tried = Arc::clone(&tried);
            Arc::new(move || *tried.lock().unwrap() += 1)
        });

        failing.failing(three_exits());
        assert_eq!(failing.now(), Some(three_exits()));
        failing.try_again().expect("tried again");
        failing.not_failing();

        assert_eq!(*tried.lock().unwrap(), 1);
        assert_eq!(*told.lock().unwrap(), [Some(three_exits()), None]);
        assert_eq!(failing.now(), None);
    }

    #[test]
    fn try_again_is_refused_while_the_host_is_not_failing() {
        let failing = HostFailing::default();
        failing.tried_again_by(Arc::new(|| panic!("started while not failing")));

        assert!(failing.try_again().is_err());
    }
}
