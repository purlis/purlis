// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One read for a burst of asks, never an answer older than the ask** (felt speed round 4,
//! H10).
//!
//! The window asks some questions again on every change that might move their answer, and some
//! changes come in bursts: a restart of every chat moves the chats open, and each move asked
//! which chats run on an older sandbox, ten times in 48 seconds, each ask compiling every
//! chat's sandbox beside the others. [`Burst`] answers such a question once per burst:
//!
//! - an ask while no read is running starts one, and is answered by it;
//! - an ask while one is running waits for **the next** read, never the one already running,
//!   which may have looked before whatever the asker was told of; every ask that came while the
//!   same read ran shares that next read.
//!
//! So a burst of any size costs at most two reads, one after the other, and every answer comes
//! from a read that started after it was asked. Nothing is kept past the asks a read answers:
//! the next ask after a quiet spell reads again.
//!
//! In memory and in this process only: nothing is written, so there is no store to give a tier
//! (ADR 0069).

use std::sync::{Condvar, Mutex, PoisonError};

/// One question's reads, shared by everyone who asks it.
#[derive(Debug)]
pub struct Burst<V> {
    state: Mutex<State<V>>,
    finished: Condvar,
}

#[derive(Debug)]
struct State<V> {
    /// How many reads have started.
    started: u64,
    /// The newest read finished, by its number, and what it answered.
    last: Option<(u64, V)>,
    /// Whether a read is running now.
    running: bool,
    /// How many asks wait for a read.
    waiting: usize,
}

impl<V> Default for Burst<V> {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                started: 0,
                last: None,
                running: false,
                waiting: 0,
            }),
            finished: Condvar::new(),
        }
    }
}

impl<V: Clone> Burst<V> {
    /// The answer of a read that started after this ask: `read`'s, where this ask starts it,
    /// or the read another ask started once this one was waiting.
    pub fn ask(&self, read: impl FnOnce() -> V) -> V {
        let mut state = self.lock();
        // The read this ask needs: the next one to start, whether or not one runs now.
        let needed = state.started + 1;
        state.waiting += 1;
        loop {
            if let Some((_, answer)) = state.last.as_ref().filter(|(number, _)| *number >= needed) {
                let answer = answer.clone();
                state.waiting -= 1;
                return answer;
            }
            if !state.running {
                break;
            }
            state = self
                .finished
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        state.waiting -= 1;
        state.running = true;
        state.started += 1;
        let number = state.started;
        drop(state);
        // Lets the next ask read where this read panics, rather than leave it waiting for ever.
        let running = Running(self);
        let answer = read();
        std::mem::forget(running);
        let mut state = self.lock();
        state.running = false;
        state.last = Some((number, answer.clone()));
        drop(state);
        self.finished.notify_all();
        answer
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State<V>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// How many asks are waiting for a read now: for a test that must know they all are.
    #[cfg(test)]
    fn waiting(&self) -> usize {
        self.lock().waiting
    }
}

/// A read running, which a panic ends: the burst is told no read runs, and a waiting ask reads.
struct Running<'a, V>(&'a Burst<V>);

impl<V> Drop for Running<'_, V> {
    fn drop(&mut self) {
        self.0
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .running = false;
        self.0.finished.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    #[test]
    fn asks_made_while_a_read_runs_share_one_next_read_and_none_takes_the_running_one() {
        let burst = Arc::new(Burst::<usize>::default());
        let reads = Arc::new(AtomicUsize::new(0));
        let (inside, read_started) = mpsc::channel::<()>();
        let (release, let_go) = mpsc::channel::<()>();
        let let_go = Arc::new(Mutex::new(let_go));

        // The first ask starts a read, and holds it until it is let go.
        let first = {
            let (burst, reads, let_go) = (burst.clone(), reads.clone(), let_go.clone());
            std::thread::spawn(move || {
                burst.ask(|| {
                    let n = reads.fetch_add(1, Ordering::SeqCst) + 1;
                    inside.send(()).unwrap();
                    let_go.lock().unwrap().recv().unwrap();
                    n
                })
            })
        };
        read_started.recv().unwrap();

        // Ten more ask while it runs.
        let later: Vec<_> = (0..10)
            .map(|_| {
                let (burst, reads) = (burst.clone(), reads.clone());
                std::thread::spawn(move || burst.ask(|| reads.fetch_add(1, Ordering::SeqCst) + 1))
            })
            .collect();
        // Every one of them is waiting, or has been answered, before the first read ends.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while burst.waiting() + reads.load(Ordering::SeqCst) < 11
            && std::time::Instant::now() < until
        {
            std::thread::yield_now();
        }
        release.send(()).unwrap();
        assert_eq!(first.join().unwrap(), 1, "the first ask has its own read");
        let answers: Vec<usize> = later.into_iter().map(|one| one.join().unwrap()).collect();

        assert_eq!(
            answers,
            vec![2; 10],
            "each later ask is answered by the read after"
        );
        assert_eq!(reads.load(Ordering::SeqCst), 2, "eleven asks, two reads");
    }

    #[test]
    fn an_ask_after_a_read_finished_reads_again() {
        let burst = Burst::<u32>::default();
        let mut reads = 0;
        assert_eq!(
            burst.ask(|| {
                reads += 1;
                reads
            }),
            1
        );
        assert_eq!(
            burst.ask(|| {
                reads += 1;
                reads
            }),
            2,
            "nothing is kept past its asks"
        );
    }

    #[test]
    fn a_read_that_panics_lets_the_next_ask_read() {
        let burst = Arc::new(Burst::<u32>::default());
        let panicked = {
            let burst = burst.clone();
            std::thread::spawn(move || burst.ask(|| panic!("a read that fails"))).join()
        };
        assert!(panicked.is_err());
        assert_eq!(
            burst.ask(|| 7),
            7,
            "the burst is not held by the read that failed"
        );
    }
}
