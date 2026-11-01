// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A pool sign-in started from the command is confirmed in the window.** `purlis profile sign-in
//! --pool` asks over the window line ([`purlis_core::windowline`]), and the answer is the person's
//! press in the dialog (`PoolSignInConfirm.tsx`), through commands only the window may call
//! (`ui::WINDOW_ONLY`). One waits at a time. It is taken back after
//! [`purlis_core::windowline::THE_PERSON_HAS`], and that is a no.

use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use tauri::{Emitter, Manager};

/// The event each window hears when the question changes.
pub const CHANGED: &str = "pool-sign-in-confirm";

/// What the person is asked.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSignInAsk {
    pub id: u32,
    pub profile: String,
    pub pool: String,
}

struct Waiting {
    ask: PoolSignInAsk,
    answer: Sender<bool>,
}

static WAITING: Mutex<Option<Waiting>> = Mutex::new(None);
static LAST_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn waiting() -> std::sync::MutexGuard<'static, Option<Waiting>> {
    WAITING.lock().unwrap_or_else(|held| held.into_inner())
}

/// **Asks the person, and waits for their answer or `within`.** `tell` has the windows draw it, and
/// says false where none can; `took_back` has them stop. Only a yes is `Ok`.
fn asking(
    profile: &str,
    pool: &str,
    within: Duration,
    tell: impl FnOnce() -> bool,
    took_back: impl FnOnce(),
) -> Result<Option<String>, String> {
    let (answer, answered) = channel();
    let id = LAST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    {
        let mut slot = waiting();
        if slot.is_some() {
            return Err("another pool sign-in is waiting for an answer in the window".to_owned());
        }
        *slot = Some(Waiting {
            ask: PoolSignInAsk {
                id,
                profile: profile.to_owned(),
                pool: pool.to_owned(),
            },
            answer,
        });
    }
    let said = if tell() {
        answered
            .recv_timeout(within)
            .map_err(|_| "it was not answered in time".to_owned())
    } else {
        Err("no purlis window is open".to_owned())
    };
    *waiting() = None;
    took_back();
    match said {
        Ok(true) => Ok(None),
        Ok(false) => Err("it was declined in the window".to_owned()),
        Err(why) => Err(why),
    }
}

/// [`asking`], in the windows of `app`.
pub fn ask_the_person(
    app: &tauri::AppHandle,
    profile: &str,
    pool: &str,
) -> Result<Option<String>, String> {
    let tell = || {
        let shown: Vec<_> = app
            .webview_windows()
            .into_values()
            .filter(|window| window.is_visible().unwrap_or(false))
            .collect();
        for window in &shown {
            // It draws attention and does not take the keyboard from what the person is typing in.
            let _ = window.request_user_attention(Some(tauri::UserAttentionType::Informational));
        }
        let told = app.emit(CHANGED, ()).is_ok();
        told && !shown.is_empty()
    };
    let took_back = || {
        let _ = app.emit(CHANGED, ());
    };
    asking(
        profile,
        pool,
        purlis_core::windowline::THE_PERSON_HAS,
        tell,
        took_back,
    )
}

/// The question waiting for the person's answer, if one is.
#[tauri::command]
#[specta::specta]
pub fn pool_sign_in_asked() -> Option<PoolSignInAsk> {
    waiting().as_ref().map(|waiting| waiting.ask.clone())
}

/// **The person's answer to the question `id`**: only their press in the window calls this. One
/// no longer waiting is left alone.
#[tauri::command]
#[specta::specta]
pub fn answer_pool_sign_in(id: u32, yes: bool) {
    answer(id, yes);
}

fn answer(id: u32, yes: bool) {
    if let Some(waiting) = waiting().as_ref().filter(|waiting| waiting.ask.id == id) {
        let _ = waiting.answer.send(yes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These share one slot, so they run as one.
    #[test]
    fn only_the_answer_to_the_question_asked_counts() {
        let within = Duration::from_secs(30);

        // No window to ask in.
        assert!(asking("work", "team", within, || false, || ()).is_err());
        assert_eq!(pool_sign_in_asked(), None);

        // Not answered in time.
        let said = asking("work", "team", Duration::from_millis(10), || true, || ());
        assert!(said.unwrap_err().contains("in time"));
        assert_eq!(pool_sign_in_asked(), None);

        for (yes, went_on) in [(true, true), (false, false)] {
            let asker = std::thread::spawn(move || asking("work", "team", within, || true, || ()));
            let ask = loop {
                if let Some(ask) = pool_sign_in_asked() {
                    break ask;
                }
                std::thread::yield_now();
            };
            assert_eq!((ask.profile.as_str(), ask.pool.as_str()), ("work", "team"));

            // A second is refused while one waits, and an answer to another changes nothing.
            assert!(
                asking("other", "team", within, || panic!("nobody is told"), || ())
                    .unwrap_err()
                    .contains("another")
            );
            answer(ask.id + 1, true);
            assert_eq!(pool_sign_in_asked().as_ref(), Some(&ask));

            answer(ask.id, yes);
            assert_eq!(asker.join().unwrap().is_ok(), went_on);
            assert_eq!(pool_sign_in_asked(), None);
        }
    }
}
