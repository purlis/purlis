// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a piece of work cost, counted** (#7892, FS-G2): the programs it started, and how many
//! times it read the project's sandbox manifest ([`crate::sandbox::Plane::read`]) and the
//! sandbox's locks ([`crate::sandbox::policy::Locks::of`]), for a test that holds a read to a
//! ceiling by count rather than by the clock.
//!
//! - **Carried by the thread**, as git's isolation is ([`crate::worktree::git::within`]): a
//!   scope counts what its own thread does, and what a thread it is carried to does
//!   ([`carried`], [`Carried::within`]). A thread it is not carried to counts in no scope, so
//!   work handed to one that forgot to carry it reads as costing nothing: what a gate built on it
//!   checks first is that the work did pass through the door that carries it
//!   ([`Tally::doors`]).
//! - **Scoped to the test**: [`counted`] opens a scope of its own, so tests running at once on
//!   other threads never count into it.
//! - **Nothing outside a test build.** The noting calls compile to nothing, and [`Carried`] holds
//!   nothing, unless the crate is built for its own tests or with `test-support`: the product
//!   pays no lookup for a count nobody reads.
//!
//! Every program purlis starts goes through [`super::spawn`], so that is where a start is noted.

#[cfg(any(test, feature = "test-support"))]
use std::sync::Arc;
#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::{AtomicUsize, Ordering};

/// What a scope counted, once its work is done.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    /// Programs started ([`super::spawn`], and so [`super::output`] and [`super::status`]).
    pub started: usize,
    /// Reads of the project's sandbox manifest ([`crate::sandbox::Plane::read`]).
    pub plane_reads: usize,
    /// Reads of the sandbox's locks ([`crate::sandbox::policy::Locks::of`]).
    pub locks_reads: usize,
    /// Times the work entered a door that carries the scope to its blocking thread
    /// ([`note_a_door`]): none means the work ran somewhere the scope never reached.
    pub doors: usize,
    /// What each program started was asked to run, its program and arguments, in the order the
    /// starts were noted: for a test to print what a count is made of.
    pub programs: Vec<String>,
}

/// One scope's counts, shared by every thread it is carried to.
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Default)]
struct Counts {
    started: AtomicUsize,
    plane_reads: AtomicUsize,
    locks_reads: AtomicUsize,
    doors: AtomicUsize,
    programs: std::sync::Mutex<Vec<String>>,
}

#[cfg(any(test, feature = "test-support"))]
impl Counts {
    /// Read once the work is done: every thread that counted into it was joined first, and the
    /// join orders their adds before these loads, so `Relaxed` reads them all.
    fn tally(&self) -> Tally {
        Tally {
            started: self.started.load(Ordering::Relaxed),
            plane_reads: self.plane_reads.load(Ordering::Relaxed),
            locks_reads: self.locks_reads.load(Ordering::Relaxed),
            doors: self.doors.load(Ordering::Relaxed),
            programs: self
                .programs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static HELD: std::cell::RefCell<Option<Arc<Counts>>> = const { std::cell::RefCell::new(None) };
}

/// **`work`, counted**: run in a scope of its own on this thread, with what it did on this
/// thread and on every thread the scope was carried to. A scope already held here is set aside
/// for `work`, and counts none of it.
#[cfg(any(test, feature = "test-support"))]
pub fn counted<T>(work: impl FnOnce() -> T) -> (T, Tally) {
    let counts = Arc::new(Counts::default());
    let done = Carried {
        counts: Some(Arc::clone(&counts)),
    }
    .within(work);
    (done, counts.tally())
}

/// The scope this thread counts in, to carry to a thread it hands work to: [`Carried::within`]
/// there. Outside a test build, nothing.
pub fn carried() -> Carried {
    Carried {
        #[cfg(any(test, feature = "test-support"))]
        counts: HELD.with_borrow(Clone::clone),
    }
}

/// A scope taken from one thread to be entered on another ([`carried`]).
#[derive(Debug, Clone, Default)]
pub struct Carried {
    #[cfg(any(test, feature = "test-support"))]
    counts: Option<Arc<Counts>>,
}

impl Carried {
    /// Runs `then` counting in this scope, and puts back the one this thread held, however it
    /// ends.
    pub fn within<T>(&self, then: impl FnOnce() -> T) -> T {
        #[cfg(any(test, feature = "test-support"))]
        {
            struct Restore(Option<Arc<Counts>>);
            impl Drop for Restore {
                fn drop(&mut self) {
                    let before = self.0.take();
                    HELD.with_borrow_mut(|held| *held = before);
                }
            }
            let before = HELD.with_borrow_mut(|held| std::mem::replace(held, self.counts.clone()));
            let _restore = Restore(before);
            then()
        }
        #[cfg(not(any(test, feature = "test-support")))]
        then()
    }
}

/// Counts one more of what `field` names in this thread's scope, where it has one.
#[cfg(any(test, feature = "test-support"))]
fn note(field: impl FnOnce(&Counts) -> &AtomicUsize) {
    HELD.with_borrow(|held| {
        if let Some(counts) = held {
            field(counts).fetch_add(1, Ordering::Relaxed);
        }
    });
}

/// `command`'s program is being started.
pub(crate) fn note_a_start(command: &std::process::Command) {
    #[cfg(any(test, feature = "test-support"))]
    HELD.with_borrow(|held| {
        if let Some(counts) = held {
            counts.started.fetch_add(1, Ordering::Relaxed);
            let argv = std::iter::once(command.get_program())
                .chain(command.get_args())
                .map(|part| part.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            counts
                .programs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(argv);
        }
    });
    #[cfg(not(any(test, feature = "test-support")))]
    let _ = command;
}

/// Whether this thread counts in a scope now: for a cache whose keeping turns on the clock, so
/// it keeps nothing where a count is taken, and the count does not depend on how long the test
/// ran. Outside a test build, never.
pub(crate) fn is_counting() -> bool {
    #[cfg(any(test, feature = "test-support"))]
    return HELD.with_borrow(Option::is_some);
    #[cfg(not(any(test, feature = "test-support")))]
    false
}

/// The project's sandbox manifest is being read.
pub(crate) fn note_a_plane_read() {
    #[cfg(any(test, feature = "test-support"))]
    note(|counts| &counts.plane_reads);
}

/// The sandbox's locks are being read.
pub(crate) fn note_a_locks_read() {
    #[cfg(any(test, feature = "test-support"))]
    note(|counts| &counts.locks_reads);
}

/// Work entered a door that carried this thread's scope to it: the app's one way of answering
/// off the window's thread (`off_the_window`) calls it on its blocking thread.
pub fn note_a_door() {
    #[cfg(any(test, feature = "test-support"))]
    note(|counts| &counts.doors);
}

#[cfg(all(test, unix))]
mod tests {
    use std::process::Command;

    use super::{Tally, carried, counted};

    fn start_one() {
        crate::forklock::status(&mut Command::new("true")).expect("true runs");
    }

    #[test]
    fn a_scope_counts_the_programs_its_own_thread_starts() {
        let ((), tally) = counted(|| {
            start_one();
            start_one();
        });
        assert_eq!(tally.started, 2);
        assert_eq!(tally.programs, ["true", "true"]);
    }

    #[test]
    fn a_scope_carried_to_another_thread_counts_what_that_thread_starts() {
        let ((), tally) = counted(|| {
            let scope = carried();
            std::thread::spawn(move || scope.within(start_one))
                .join()
                .expect("the thread");
        });
        assert_eq!(tally.started, 1);
    }

    #[test]
    fn what_a_folders_fan_out_starts_on_its_threads_counts_in_the_scope_that_asked() {
        let (_, tally) = counted(|| crate::finished::each_at_once(&[1, 2, 3], |_| start_one()));
        assert_eq!(tally.started, 3);
    }

    #[test]
    fn a_thread_the_scope_is_not_carried_to_counts_in_no_scope() {
        let ((), tally) = counted(|| {
            std::thread::spawn(start_one).join().expect("the thread");
        });
        assert_eq!(tally, Tally::default());
    }

    #[test]
    fn an_inner_scope_counts_only_its_own_work_and_the_outer_one_resumes() {
        let ((inner, ()), outer) = counted(|| {
            start_one();
            let (_, inner) = counted(start_one);
            start_one();
            (inner, ())
        });
        assert_eq!((inner.started, outer.started), (1, 2));
    }

    #[test]
    fn the_scope_counts_the_sandbox_manifest_and_locks_it_reads() {
        let dir = tempfile::tempdir().expect("a project");
        let (_, manifest) = counted(|| crate::sandbox::Plane::read(dir.path()));
        // The locks read the manifest too, for the team's `policy = "never"`.
        let (_, locks) = counted(|| crate::sandbox::policy::Locks::of(dir.path()));
        assert_eq!((manifest.plane_reads, manifest.locks_reads), (1, 0));
        assert_eq!((locks.plane_reads, locks.locks_reads), (1, 1));
    }
}
