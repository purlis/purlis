// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A pool sign-in started from the command is confirmed in the window.** `purlis profile sign-in
//! --pool` asks over the window line (`purlis_core::windowline`), and the answer is the person's
//! press in the dialog (`PoolSignInConfirm.tsx`), through commands only the window may call
//! (`ui::WINDOW_ONLY`).
//!
//! One waits at a time. It goes when the command hangs up, and after `THE_PERSON_HAS`, which is a
//! no. Once one has ended without a yes, the next waits a moment, so no loop keeps the dialog up.
//! A yes also lets the command keep that profile on that pool, once ([`take_the_yes`]).
#![cfg_attr(not(unix), allow(dead_code))]

use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

/// The event each window hears when the question changes.
pub const CHANGED: &str = "pool-sign-in-confirm";
/// How long after a question ended without a yes the next is refused.
const THE_NEXT_WAITS: Duration = Duration::from_secs(5);
/// How long a yes lets the command keep the profile.
const A_YES_LASTS: Duration = Duration::from_secs(300);
/// How often the wait looks at whether the command hung up.
const LOOKS_EVERY: Duration = Duration::from_millis(250);

/// What the person is asked.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSignInAsk {
    pub id: u32,
    pub profile: String,
    pub pool: String,
    /// What the terminal of the command that asks shows.
    pub code: String,
}

struct Waiting {
    ask: PoolSignInAsk,
    answer: Sender<bool>,
}

#[derive(Default)]
struct Slot {
    waiting: Option<Waiting>,
    ended_without_a_yes: Option<Instant>,
    /// The profile and pool of the last yes, and when.
    yes: Option<(String, String, Instant)>,
}

static SLOT: Mutex<Option<Slot>> = Mutex::new(None);
static LAST_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn with_slot<T>(act: impl FnOnce(&mut Slot) -> T) -> T {
    let mut held = SLOT.lock().unwrap_or_else(|held| held.into_inner());
    act(held.get_or_insert_with(Slot::default))
}

/// Empties the question however the asking ends.
struct Clears;

impl Drop for Clears {
    fn drop(&mut self) {
        with_slot(|slot| slot.waiting = None);
    }
}

/// How long the asking waits.
struct Waits<'a> {
    within: Duration,
    next_waits: Duration,
    hung_up: &'a dyn Fn() -> bool,
}

/// **Asks the person, and waits for their answer, the command to hang up, or `within`.** `tell` has
/// the windows draw it, and says false where none can; `took_back` has them stop. Only a yes is
/// `Ok`.
fn asking(
    profile: &str,
    pool: &str,
    code: &str,
    waits: &Waits<'_>,
    tell: impl FnOnce() -> bool,
    took_back: impl FnOnce(),
) -> Result<Option<String>, String> {
    let (answer, answered) = channel();
    let id = LAST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    with_slot(|slot| {
        if slot.waiting.is_some() {
            return Err(
                "a command is already waiting for an answer in the window: answer that one first"
                    .to_owned(),
            );
        }
        if slot
            .ended_without_a_yes
            .is_some_and(|ended| ended.elapsed() < waits.next_waits)
        {
            return Err(
                "the last one was not confirmed, so wait a moment and run this again".to_owned(),
            );
        }
        slot.waiting = Some(Waiting {
            ask: PoolSignInAsk {
                id,
                profile: profile.to_owned(),
                pool: pool.to_owned(),
                code: code.to_owned(),
            },
            answer,
        });
        Ok(())
    })?;
    let clears = Clears;
    let said = if tell() {
        let until = Instant::now() + waits.within;
        loop {
            match answered.recv_timeout(LOOKS_EVERY) {
                Ok(yes) => break Ok(yes),
                Err(_) if (waits.hung_up)() => break Err("the command ended".to_owned()),
                Err(_) if Instant::now() >= until => {
                    break Err("it was not answered in time".to_owned());
                }
                Err(_) => {}
            }
        }
    } else {
        Err("no purlis window is open".to_owned())
    };
    drop(clears);
    took_back();
    let yes = matches!(said, Ok(true));
    with_slot(|slot| {
        slot.yes = yes.then(|| (profile.to_owned(), pool.to_owned(), Instant::now()));
        slot.ended_without_a_yes = (!yes).then(Instant::now);
    });
    match said {
        Ok(true) => Ok(None),
        Ok(false) => Err("it was declined in the window".to_owned()),
        Err(why) => Err(why),
    }
}

/// **Whether the person just said yes to `profile` on `pool`**, and uses that yes up: what lets
/// the command have the window keep it there.
pub fn take_the_yes(profile: &str, pool: &str) -> bool {
    with_slot(|slot| match slot.yes.take() {
        Some((said_profile, said_pool, when)) => {
            said_profile == profile && said_pool == pool && when.elapsed() < A_YES_LASTS
        }
        None => false,
    })
}

/// [`asking`], in the windows of `app`.
#[cfg(unix)]
pub fn ask_the_person(
    app: &tauri::AppHandle,
    profile: &str,
    pool: &str,
    code: &str,
    hung_up: &dyn Fn() -> bool,
) -> Result<Option<String>, String> {
    use tauri::{Emitter, Manager};
    let tell = || {
        let shown: Vec<_> = app
            .webview_windows()
            .into_values()
            .filter(|window| window.is_visible().unwrap_or(false))
            .collect();
        for window in &shown {
            // It draws attention and leaves the keyboard where the person has it.
            let _ = window.request_user_attention(Some(tauri::UserAttentionType::Informational));
        }
        app.emit(CHANGED, ()).is_ok() && !shown.is_empty()
    };
    let took_back = || {
        let _ = app.emit(CHANGED, ());
    };
    asking(
        profile,
        &purlis_core::shown::short(pool),
        code,
        &Waits {
            within: purlis_core::windowline::THE_PERSON_HAS,
            next_waits: THE_NEXT_WAITS,
            hung_up,
        },
        tell,
        took_back,
    )
}

/// The question waiting for the person's answer, if one is.
#[tauri::command]
#[specta::specta]
pub fn pool_sign_in_asked() -> Option<PoolSignInAsk> {
    with_slot(|slot| slot.waiting.as_ref().map(|waiting| waiting.ask.clone()))
}

/// **The person's answer to the question `id`**: only their press in the window calls this. One
/// no longer waiting is left alone.
#[tauri::command]
#[specta::specta]
pub fn answer_pool_sign_in(id: u32, yes: bool) {
    with_slot(|slot| {
        if let Some(waiting) = slot.waiting.as_ref().filter(|waiting| waiting.ask.id == id) {
            let _ = waiting.answer.send(yes);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waits(within: Duration, hung_up: &'static (dyn Fn() -> bool + Sync)) -> Waits<'static> {
        Waits {
            within,
            next_waits: Duration::ZERO,
            hung_up,
        }
    }

    const STAYS: &(dyn Fn() -> bool + Sync) = &|| false;

    fn asked_in_a_thread() -> std::thread::JoinHandle<Result<Option<String>, String>> {
        std::thread::spawn(|| {
            asking(
                "work",
                "team",
                "A1B2C3",
                &waits(Duration::from_secs(30), STAYS),
                || true,
                || (),
            )
        })
    }

    fn the_question() -> PoolSignInAsk {
        loop {
            if let Some(ask) = pool_sign_in_asked() {
                return ask;
            }
            std::thread::yield_now();
        }
    }

    /// These share one slot, so they run as one.
    #[test]
    fn only_the_answer_to_the_question_asked_counts_and_a_yes_lets_one_keep() {
        let soon = waits(Duration::from_millis(10), STAYS);
        let ask = |waits: &Waits<'_>, tell: fn() -> bool| {
            asking("work", "team", "A1B2C3", waits, tell, || ())
        };

        // No window to ask in.
        assert!(ask(&soon, || false).is_err());
        assert_eq!(pool_sign_in_asked(), None);

        // Not answered in time.
        assert!(ask(&soon, || true).unwrap_err().contains("in time"));
        assert_eq!(pool_sign_in_asked(), None);

        // The command hung up.
        let gone = waits(Duration::from_secs(30), &|| true);
        assert!(ask(&gone, || true).unwrap_err().contains("ended"));
        assert_eq!(pool_sign_in_asked(), None);

        // Soon after one ended without a yes, the next is refused and nobody is told.
        let after_a_no = Waits {
            within: Duration::from_secs(30),
            next_waits: Duration::from_secs(60),
            hung_up: STAYS,
        };
        assert!(
            ask(&after_a_no, || panic!("nobody is told"))
                .unwrap_err()
                .contains("wait a moment")
        );

        // A no.
        let asker = asked_in_a_thread();
        let question = the_question();
        assert_eq!(question.code, "A1B2C3");

        // A second is refused while one waits, and an answer to another changes nothing.
        assert!(
            ask(&soon, || panic!("nobody is told"))
                .unwrap_err()
                .contains("already waiting")
        );
        answer_pool_sign_in(question.id + 1, true);
        assert_eq!(pool_sign_in_asked().as_ref(), Some(&question));

        answer_pool_sign_in(question.id, false);
        assert!(asker.join().unwrap().is_err());
        assert_eq!(pool_sign_in_asked(), None);
        assert!(!take_the_yes("work", "team"), "a no lets nothing be kept");

        // A yes goes on, and lets that profile be kept on that pool once.
        let asker = asked_in_a_thread();
        answer_pool_sign_in(the_question().id, true);
        assert!(asker.join().unwrap().is_ok());
        assert!(!take_the_yes("other", "team"));
        let asker = asked_in_a_thread();
        answer_pool_sign_in(the_question().id, true);
        assert!(asker.join().unwrap().is_ok());
        assert!(take_the_yes("work", "team"));
        assert!(!take_the_yes("work", "team"), "once");
    }
}
