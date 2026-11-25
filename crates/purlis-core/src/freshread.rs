// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One read at a time, shared by everyone who asked while it ran**.
//!
//! The app answers each chat's guard by reading the project's committed `[policy]` from git, and
//! every chat's guard asks before every tool call. N chats asking at once meant N reads, two git
//! processes each, so a guard's wait grew with the number of chats open.
//!
//! A [`Fresh`] runs `read` for the first asker and lets the askers that arrive while it runs wait
//! for **the next run**, which one of them starts and all of them share. An answer is therefore
//! always from a read that **began after its asker arrived**: a commit made before the ask is
//! seen, exactly as when every ask reads alone, and N askers cost at most two reads, not N.

use std::sync::{Condvar, Mutex, PoisonError};

struct State<T> {
    /// Reads begun, and reads finished (a read finishes in the order it began).
    started: u64,
    finished: u64,
    running: bool,
    /// The answer of read number `finished`.
    last: Option<T>,
}

/// A read whose concurrent askers share it.
pub struct Fresh<T> {
    state: Mutex<State<T>>,
    done: Condvar,
}

impl<T: Clone> Fresh<T> {
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(State {
                started: 0,
                finished: 0,
                running: false,
                last: None,
            }),
            done: Condvar::new(),
        }
    }

    /// The answer of a read that began after this call did: `read` run here, or the run of
    /// another asker that no one's earlier read covers.
    pub fn get(&self, read: impl Fn() -> T) -> T {
        let lock = || self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut state = lock();
        // The first read that begins after this call: the one after any now running.
        let needed = state.started + 1;
        loop {
            if state.finished >= needed
                && let Some(answer) = &state.last
            {
                return answer.clone();
            }
            if state.running {
                state = self
                    .done
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
                continue;
            }
            state.running = true;
            state.started += 1;
            let number = state.started;
            drop(state);
            // Even a read that panics must give the next asker its turn.
            let ended = Ends {
                fresh: self,
                number,
                answer: None,
            };
            let answer = read();
            ended.finish(answer.clone());
            return answer;
        }
    }
}

/// A read in progress: records its answer, or only lets go if the read panicked, so the askers
/// waiting on it start a read of their own rather than wait for ever.
struct Ends<'a, T: Clone> {
    fresh: &'a Fresh<T>,
    number: u64,
    answer: Option<T>,
}

impl<T: Clone> Ends<'_, T> {
    fn finish(mut self, answer: T) {
        self.answer = Some(answer);
    }
}

impl<T: Clone> Drop for Ends<'_, T> {
    fn drop(&mut self) {
        let mut state = self
            .fresh
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        state.running = false;
        if let Some(answer) = self.answer.take() {
            state.finished = self.number;
            state.last = Some(answer);
        }
        drop(state);
        self.fresh.done.notify_all();
    }
}

impl<T: Clone> Default for Fresh<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    /// Thirty-two askers at once, over a read that takes a while: each gets an answer from a read
    /// that began after it arrived, and the reads are a handful, never one an asker.
    #[test]
    fn askers_that_arrive_together_share_a_read_that_began_after_each_arrived() {
        let fresh = Fresh::<u64>::new();
        let began = AtomicU64::new(0);
        let barrier = std::sync::Barrier::new(32);
        let answers: Vec<(u64, u64)> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..32)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        let reads_before = began.load(Ordering::SeqCst);
                        let answer = fresh.get(|| {
                            let me = began.fetch_add(1, Ordering::SeqCst) + 1;
                            std::thread::sleep(Duration::from_millis(60));
                            me
                        });
                        (reads_before, answer)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("an asker"))
                .collect()
        });
        let reads = began.load(Ordering::SeqCst);
        assert!(reads <= 3, "{reads} reads for 32 askers");
        for (reads_before, answer) in answers {
            assert!(
                answer > reads_before,
                "an answer of read {answer} was given to an asker that arrived after {reads_before} began"
            );
        }
    }

    #[test]
    fn a_lone_asker_reads_once_and_each_later_asker_reads_again() {
        let fresh = Fresh::<u32>::new();
        let reads = std::sync::atomic::AtomicU32::new(0);
        let read = || reads.fetch_add(1, Ordering::SeqCst) + 1;
        assert_eq!(fresh.get(read), 1);
        assert_eq!(fresh.get(read), 2);
    }
}
