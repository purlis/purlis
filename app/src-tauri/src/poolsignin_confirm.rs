// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A pool sign-in started from the command is confirmed in the window**, and so is a profile
//! taken off a pool from the command (#5174), and a commit `purlis signing check` asks GitHub
//! about with the window's sign-in (#5625). `purlis profile sign-in --pool` and `purlis pool
//! remove-profile` ask over the window line (`purlis_core::windowline`), and the answer is the
//! person's press in the dialog (`PoolSignInConfirm.tsx`), through commands only the window may
//! call (`ui::WINDOW_ONLY`).
//!
//! One waits at a time. It goes when the command hangs up, and after `THE_PERSON_HAS`, which is a
//! no. Once one has ended without a yes, the next waits a moment, so no loop keeps the dialog up.
//! A yes to a sign-in also lets the command keep that profile on that pool, once
//! ([`take_the_yes`]); a yes to taking one off lets nothing else through.
#![cfg_attr(not(unix), allow(dead_code))]

use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

use crate::planes::PlaneId;

/// The event each window hears when the question changes.
pub const CHANGED: &str = "pool-sign-in-confirm";
/// How long after a question ended without a yes the next is refused.
const THE_NEXT_WAITS: Duration = Duration::from_secs(5);
/// How long a yes lets the command keep the profile.
const A_YES_LASTS: Duration = Duration::from_secs(300);
/// How often the wait looks at whether the command hung up.
const LOOKS_EVERY: Duration = Duration::from_millis(250);
/// How long a line this window typed for a pool's sign-in stands for the person's yes to it.
const A_TYPED_LINE_LASTS: Duration = Duration::from_secs(60);

/// Which command's question it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum Asks {
    /// `purlis profile sign-in --pool`.
    SignIn,
    /// `purlis pool remove-profile`: its runner forgets the profile, and its sign-in there.
    TakeOff,
    /// `purlis signing check` (#5625): the window asks GitHub how a pushed commit shows. Its
    /// `profile` is the repository (`owner/name`) and its `pool` the commit.
    SigningCheck,
}

/// Which command's question, the forge a signing check asks (#5793), and the project its ask is
/// listed under ([`waiting_in`]).
#[derive(Debug, Clone, Copy)]
struct Question<'a> {
    asks: Asks,
    on: Option<&'a str>,
    home: Option<&'a PlaneId>,
}

#[cfg(test)]
impl Question<'_> {
    /// A question that names no forge, listed under no project.
    fn of(asks: Asks) -> Self {
        Question {
            asks,
            on: None,
            home: None,
        }
    }
}

/// What the person is asked.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSignInAsk {
    pub id: u32,
    pub asks: Asks,
    pub profile: String,
    pub pool: String,
    /// What the terminal of the command that asks shows.
    pub code: String,
    /// The forge a signing check asks, as a sentence names it (`GitHub`, `GitLab on
    /// gitlab.example.org`, #5793); none for the other questions.
    #[specta(optional)]
    pub on: Option<String>,
}

struct Waiting {
    ask: PoolSignInAsk,
    answer: Sender<bool>,
    /// The project whose Asks lists it, and whose ✋ counts it (#6000).
    home: Option<PlaneId>,
    /// When it is taken back as a no, in seconds since 1970.
    until: u32,
}

#[derive(Default)]
struct Slot {
    waiting: Option<Waiting>,
    ended_without_a_yes: Option<Instant>,
    /// The profile and pool of the last yes, and when.
    yes: Option<(String, String, Instant)>,
    /// The profile and pool of the last sign-in line this window typed, and when
    /// ([`the_window_typed`]).
    typed: Option<(String, String, Instant)>,
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
    Question { asks, on, home }: Question<'_>,
    profile: &str,
    pool: &str,
    code: &str,
    waits: &Waits<'_>,
    tell: impl FnOnce() -> bool,
    took_back: impl FnOnce(),
) -> Result<Option<String>, String> {
    let (answer, answered) = channel();
    let id = LAST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let typed_here = with_slot(|slot| {
        let typed = slot.typed.take_if(|(said_profile, said_pool, when)| {
            asks == Asks::SignIn
                && said_profile == profile
                && said_pool == pool
                && when.elapsed() < A_TYPED_LINE_LASTS
        });
        if typed.is_some() {
            slot.yes = Some((profile.to_owned(), pool.to_owned(), Instant::now()));
        }
        typed.is_some()
    });
    if typed_here {
        return Ok(None);
    }
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
                asks,
                profile: profile.to_owned(),
                pool: purlis_core::shown::short(pool),
                code: code.to_owned(),
                on: on.map(str::to_owned),
            },
            answer,
            home: home.cloned(),
            until: u32::try_from(
                crate::sandboxing::now_secs().saturating_add(waits.within.as_secs()),
            )
            .unwrap_or(u32::MAX),
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
        slot.yes = (yes && asks == Asks::SignIn)
            .then(|| (profile.to_owned(), pool.to_owned(), Instant::now()));
        slot.ended_without_a_yes = (!yes).then(Instant::now);
    });
    match said {
        Ok(true) => Ok(None),
        Ok(false) => Err("it was declined in the window".to_owned()),
        Err(why) => Err(why),
    }
}

/// **This window just typed the line that signs `profile` in on `pool`** (Open in terminal): the
/// person's click there is their yes to it. The command that line runs asks this window as any
/// does, and is answered with that yes, once, within [`A_TYPED_LINE_LASTS`], with no dialog. The
/// same line run again later, from a shell's history, is asked of the person.
pub fn the_window_typed(profile: &str, pool: &str) {
    with_slot(|slot| {
        slot.typed = Some((profile.to_owned(), pool.to_owned(), Instant::now()));
    });
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

/// **The project a command's confirmation is listed under** (#6000): the one the command runs in,
/// `from`, where this window holds it; else the one in front of the window the person has
/// focused (`fronts`, focused first), else the first this window holds. None where it holds none:
/// then only the dialog asks.
fn home_of(
    from: Option<&std::path::Path>,
    held: &[PlaneId],
    fronts: &[PlaneId],
) -> Option<PlaneId> {
    let same = |one: &PlaneId, root: &std::path::Path| {
        one.root() == root
            || matches!(
                (std::fs::canonicalize(one.root()), std::fs::canonicalize(root)),
                (Ok(held), Ok(theirs)) if held == theirs
            )
    };
    from.and_then(|root| held.iter().find(|one| same(one, root)))
        .or_else(|| fronts.iter().find(|one| held.contains(one)))
        .or_else(|| held.iter().min_by(|a, b| a.as_str().cmp(b.as_str())))
        .cloned()
}

/// [`asking`], in the windows of `app`: listed in Asks under the project [`home_of`] picks, from
/// `from`, the project the command runs in where it said one.
#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
pub fn ask_the_person(
    app: &tauri::AppHandle,
    asks: Asks,
    on: Option<&str>,
    from: Option<&std::path::Path>,
    profile: &str,
    pool: &str,
    code: &str,
    hung_up: &dyn Fn() -> bool,
) -> Result<Option<String>, String> {
    use tauri::{Emitter, Manager};
    let held: Vec<PlaneId> = app
        .try_state::<crate::planes::Planes>()
        .map(|planes| {
            planes
                .every_held()
                .iter()
                .map(|one| one.plane_id().clone())
                .collect()
        })
        .unwrap_or_default();
    let mut windows: Vec<_> = app.webview_windows().into_values().collect();
    windows.sort_by_key(|window| !window.is_focused().unwrap_or(false));
    let fronts: Vec<PlaneId> = app
        .try_state::<crate::planes::Showing>()
        .map(|showing| {
            windows
                .iter()
                .filter_map(|window| showing.front_of(window.label()))
                .collect()
        })
        .unwrap_or_default();
    let home = home_of(from, &held, &fronts);
    // Its project's ✋ counts it and its notification is sent (`asknotify`), as for every ask.
    let poke = || {
        if let Some(home) = &home {
            crate::asknotify::poke(app, home);
        }
    };
    let tell = || {
        let shown: Vec<_> = app
            .windows()
            .into_values()
            .filter(|window| window.is_visible().unwrap_or(false))
            .collect();
        for window in &shown {
            // It draws attention and leaves the keyboard where the person has it.
            let _ = window.request_user_attention(Some(tauri::UserAttentionType::Informational));
        }
        poke();
        app.emit(CHANGED, ()).is_ok() && !shown.is_empty()
    };
    let took_back = || {
        poke();
        let _ = app.emit(CHANGED, ());
    };
    asking(
        Question {
            asks,
            on,
            home: home.as_ref(),
        },
        profile,
        pool,
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

/// **The question waiting, where project `plane`'s Asks lists it** (#6000, ADR 0121 decision 2): a
/// command's confirmation in the window is a Blocking ask, listed and counted under the one
/// project it was filed under ([`home_of`]), with when it is taken back as a no.
pub fn waiting_in(plane: &PlaneId) -> Option<(PoolSignInAsk, u32)> {
    with_slot(|slot| {
        slot.waiting
            .as_ref()
            .filter(|waiting| waiting.home.as_ref() == Some(plane))
            .map(|waiting| (waiting.ask.clone(), waiting.until))
    })
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

    fn plane(root: &str) -> PlaneId {
        PlaneId::for_tests(std::path::Path::new(root))
    }

    /// **#6000: a confirmation is filed under the project its command runs in**, where this
    /// window holds it; else under the one in front of the focused window; else the first held.
    #[test]
    fn a_confirmation_is_filed_under_the_commands_project_else_the_one_in_front() {
        let held = [plane("/p/b"), plane("/p/a"), plane("/p/c")];
        let from = Some(std::path::Path::new("/p/c"));
        assert_eq!(home_of(from, &held, &[plane("/p/b")]), Some(plane("/p/c")));
        let elsewhere = Some(std::path::Path::new("/p/z"));
        assert_eq!(
            home_of(elsewhere, &held, &[plane("/p/z"), plane("/p/b")]),
            Some(plane("/p/b")),
            "a project this window does not hold is passed over"
        );
        assert_eq!(home_of(None, &held, &[]), Some(plane("/p/a")));
        assert_eq!(home_of(from, &[], &[plane("/p/b")]), None);
    }

    fn asked_in_a_thread() -> std::thread::JoinHandle<Result<Option<String>, String>> {
        std::thread::spawn(|| {
            asking(
                Question::of(Asks::SignIn),
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
            asking(
                Question::of(Asks::SignIn),
                "work",
                "team",
                "A1B2C3",
                waits,
                tell,
                || (),
            )
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

        // #5174: a yes to taking a profile off answers only that, so it lets no keep through.
        let asker = std::thread::spawn(|| {
            asking(
                Question::of(Asks::TakeOff),
                "work",
                "team",
                "D4E5F6",
                &waits(Duration::from_secs(30), STAYS),
                || true,
                || (),
            )
        });
        let question = the_question();
        assert_eq!(question.asks, Asks::TakeOff);
        assert_eq!(
            waiting_in(&PlaneId::for_tests(std::path::Path::new("/p/a"))),
            None,
            "a question filed under no project is listed in none"
        );
        answer_pool_sign_in(question.id, true);
        assert!(asker.join().unwrap().is_ok());
        assert!(!take_the_yes("work", "team"), "no keep on this yes");

        // #6000: a question is listed in the Asks of the project it was filed under alone, until
        // it is taken back two minutes on.
        let a = PlaneId::for_tests(std::path::Path::new("/p/a"));
        let b = PlaneId::for_tests(std::path::Path::new("/p/b"));
        let home = a.clone();
        let asker = std::thread::spawn(move || {
            asking(
                Question {
                    asks: Asks::SignIn,
                    on: None,
                    home: Some(&home),
                },
                "work",
                "team",
                "A1B2C3",
                &waits(Duration::from_secs(120), STAYS),
                || true,
                || (),
            )
        });
        let question = the_question();
        let (listed, until) = waiting_in(&a).expect("listed under its project");
        assert_eq!(listed, question);
        let now = u32::try_from(crate::sandboxing::now_secs()).unwrap();
        assert!(
            (now + 110..=now + 121).contains(&until),
            "{until} against {now}"
        );
        assert_eq!(waiting_in(&b), None, "and under no other");
        answer_pool_sign_in(question.id, false);
        assert!(asker.join().unwrap().is_err());
        assert_eq!(
            waiting_in(&a),
            None,
            "an answered question is listed nowhere"
        );
        with_slot(|slot| slot.ended_without_a_yes = None);

        // A line this window just typed is the person's yes to its own sign-in, once, with no
        // dialog; it lets that profile be kept there once, as a yes does.
        the_window_typed("work", "team");
        assert!(ask(&soon, || panic!("nobody is asked")).is_ok());
        assert!(take_the_yes("work", "team"));
        // The same line run again, from a shell's history, is asked of the person.
        assert!(ask(&soon, || true).unwrap_err().contains("in time"));
        // A line typed for another profile, or for a pool's taking off, stands for nothing.
        let after = waits(Duration::from_millis(10), STAYS);
        the_window_typed("other", "team");
        assert!(ask(&after, || true).is_err());
        the_window_typed("work", "team");
        assert!(
            asking(
                Question::of(Asks::TakeOff),
                "work",
                "team",
                "D4E5F6",
                &soon,
                || true,
                || ()
            )
            .unwrap_err()
            .contains("in time")
        );
    }
}
