// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a host says in its log when it was slow** (#6437). A window gave up on a host's answer
//! to a Close after 30 s on a loaded macOS runner, and the host's log said nothing of it, so
//! nobody could tell what had held it. Two things can hold an answer, and the host now says
//! which:
//!
//! - **The call's lane, or the call itself.** A call keeps its order behind the earlier calls of
//!   its lane (a chat's commands, one after another), and then takes its own time. An answer
//!   [`SAID_FROM`] or more after its call came is said, with how much of it was each
//!   ([`answered_late`]).
//! - **The host's one thread.** A host serves on a runtime of one thread
//!   (`crate::run`), so anything that blocks it holds up every link. A look made every
//!   [`LOOKS_EVERY`] that comes [`SAID_FROM`] or more late is said ([`watch_the_thread`]): what
//!   blocked it is then whatever the lines around it name.
//!
//! The host's log is its standard error, which the window that started it keeps in its own log
//! (`sessionhost.rs`), and a scenario run keeps the window's log when it fails.

use std::time::{Duration, Instant};

/// How late an answer, or a look at the host's thread, is before it is said.
pub const SAID_FROM: Duration = Duration::from_secs(1);

/// How often the host's thread is looked at.
pub const LOOKS_EVERY: Duration = Duration::from_millis(250);

/// What the host says of call `word`, answered `waited` behind its lane's earlier calls and
/// `ran` carrying it out: nothing for one answered within [`SAID_FROM`] of coming.
pub fn answered_late(word: &str, waited: Duration, ran: Duration) -> Option<String> {
    let took = waited + ran;
    (took >= SAID_FROM).then(|| {
        format!(
            "a `{word}` was answered {} ms after it came: {} ms behind the earlier calls of its \
             lane, then {} ms carrying it out",
            took.as_millis(),
            waited.as_millis(),
            ran.as_millis()
        )
    })
}

/// What the host says of a look at its thread that came `late` past when it was due: nothing
/// for one under [`SAID_FROM`].
pub fn thread_held(late: Duration) -> Option<String> {
    (late >= SAID_FROM).then(|| {
        format!(
            "this host's thread was held {} ms: nothing it serves was answered meanwhile",
            late.as_millis()
        )
    })
}

/// Looks at the host's thread every [`LOOKS_EVERY`] for as long as the runtime runs, saying
/// each look that came late ([`thread_held`]).
pub async fn watch_the_thread() {
    loop {
        let due = Instant::now() + LOOKS_EVERY;
        tokio::time::sleep_until(due.into()).await;
        if let Some(said) = thread_held(due.elapsed()) {
            crate::say(&said);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_that_came_late_says_how_much_its_lane_held_it_and_how_much_it_took_itself() {
        assert_eq!(
            answered_late(
                "close",
                Duration::from_millis(29_800),
                Duration::from_millis(107)
            )
            .as_deref(),
            Some(
                "a `close` was answered 29907 ms after it came: 29800 ms behind the earlier calls \
                 of its lane, then 107 ms carrying it out"
            )
        );
    }

    #[test]
    fn an_answer_within_a_second_of_its_call_is_not_said() {
        assert_eq!(
            answered_late(
                "write",
                Duration::from_millis(400),
                Duration::from_millis(599)
            ),
            None
        );
        assert!(
            answered_late(
                "write",
                Duration::from_millis(400),
                Duration::from_millis(600)
            )
            .is_some()
        );
    }

    #[test]
    fn a_look_at_the_hosts_thread_a_second_late_is_said_and_one_sooner_is_not() {
        assert_eq!(thread_held(Duration::from_millis(999)), None);
        assert_eq!(
            thread_held(Duration::from_millis(29_907)).as_deref(),
            Some("this host's thread was held 29907 ms: nothing it serves was answered meanwhile")
        );
    }
}
