// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Which commands answer off the thread that asked (SC-2, #680).
//!
//! Tauri runs a synchronous command on the thread that handed it the request, which in the app
//! is the main thread: the one that pumps the window's events. A command that walks the plane's
//! files there holds the window for as long as the walk takes, and a plane with dozens of
//! workspaces and thousands of memories makes that a hitch a person sees. So the commands that
//! read every workspace, every memory or every session record are `async` and do their reading
//! on a blocking thread, as `workspace_repos` already did for git.
//!
//! **Every command that runs git answers off the window's thread too** (#1007): a git process
//! takes milliseconds to start before it does anything, so no such command can stay under the
//! 1 ms the window can spare. That is the worktree verbs (list, which piece a chat is in,
//! remove, done, merge, and New branch, which already was), and a workspace's create, its
//! at-risk reading and its remove. Create and remove also read the sidebar's model again under
//! its lock, which the watcher holds while it applies a change. Settings' two lists of grants
//! name who committed each of the project's grants, which asks git, so the dispatch grants'
//! read and revoke and the sandbox grants' read and revoke answer off it as well (#1543).
//!
//! **And every command that reads a branch's files** (#1007): a file for the light editor,
//! which may be 5 MiB, and Move aside…, which asks git whether `AGENTS.md` is the operator's.
//! Copy path and Reveal place the path on the disk off it too, then put it on the clipboard or
//! hand it to the file manager. The rest of `piecefiles.rs` already was. So does "Start a chat
//! here", which finds its branch's folder through the bounded reader: a busy reader gate holds
//! that ask for its deadline, twice over.
//!
//! **And every command that touches a file, a process or a lock another purlis may hold**
//! (#1949), whatever it measures here: a project switch asks the sandbox's state of five notices
//! and two reads of every pane on screen, and forty fast reads in a row are still the window
//! waiting. That is the sandbox's state and every answer that changes it, a pane's usage gauge
//! and persona grants, the network record's reads and Allows, the dispatch grants' reads and
//! answers, a session record, the pins and this machine's store, a chat's start, a project's
//! close and making one. What the window's tab commands change is changed in memory, in order,
//! and the files it lands in (the window's tabs in this machine's store, a project's reopen
//! record) are written by a writer of their own (`planes::WrittenBehind`), newest last; a
//! close and the quit write the last one themselves.
//!
//! **A close and a start, or a close and an open, can now meet.** A close holds the project's
//! turn until it has let go of it, so an open of the same project waits for it (a while, then
//! says so), and a chat's start into a project being let go of is refused, or ended where it
//! registered as the close ended the rest.
//!
//! **What stays synchronous is listed, each with its reason** (`SYNCHRONOUS` in this module's
//! costs tests, which fail on a synchronous command not on it): what the app holds in memory,
//! and what the window relies on the order of, below. `workspace_focused` checks a name and
//! hands the extensions' report to a thread of its own.
//!
//! **A command made `async` answers when its work is done, not in the order it was asked.**
//! Where the window reads and changes the same files, `app/src/commandOrder.ts` keeps them in
//! one lane per project: reads run together and wait for the last change before them, and a
//! change waits for everything before it. Every call in this crate that writes a file a lane
//! reads is listed in `app/src/writeSites.json` with the commands that reach it:
//! `lane_writes_guard_tests.rs` fails on one that is not, and on a command that reaches one
//! within two calls and is not named with it.
//!
//! Smart close's offer, its start and its cancel stay synchronous, for a terminal's reason. They
//! run no git, and the offer and the cancel read only what the app holds in memory, well under
//! 1 ms. The start can also touch the disk: for a task that had reported and was about to be
//! ended, it stands that end down (`dispatched::stand_down`), which reads the project's dispatch
//! records to find the task's newest one and writes it again marked kept open (#1610). Only a
//! task's start does that, once. The start types its prompt into the chat's pane, and a cancel
//! pressed straight after must land after it: Tauri keeps that order only for synchronous
//! commands, and that order is worth the one read and write.
//!
//! A terminal's own commands stay synchronous on purpose. They take well under 5 ms, and the
//! window relies on their order: a pane's resize must land before the watch that follows it
//! (#891). Nothing in this module runs in the app; it is the tests that hold both halves.

/// A test's hold on the work a command hands off the window (#1949): while a gate named for the
/// work is closed, that work waits at its start on its blocking thread, so a test can see what
/// the window does in the meantime without a race against how fast the work is.
#[cfg(test)]
pub(crate) mod gate {
    use std::collections::HashMap;
    use std::sync::{Arc, Condvar, Mutex};

    /// One closed gate: open until [`Gate::open`] says so.
    #[derive(Default)]
    pub(crate) struct Gate {
        open: Mutex<bool>,
        opened: Condvar,
    }

    impl Gate {
        /// Waits until the gate is opened.
        pub(crate) fn wait(&self) {
            let mut open = self.open.lock().unwrap_or_else(|e| e.into_inner());
            while !*open {
                open = self.opened.wait(open).unwrap_or_else(|e| e.into_inner());
            }
        }

        /// Lets every work held at it go, and every one after it.
        pub(crate) fn open(&self) {
            *self.open.lock().unwrap_or_else(|e| e.into_inner()) = true;
            self.opened.notify_all();
        }
    }

    fn closed() -> &'static Mutex<HashMap<String, Arc<Gate>>> {
        static CLOSED: std::sync::OnceLock<Mutex<HashMap<String, Arc<Gate>>>> =
            std::sync::OnceLock::new();
        CLOSED.get_or_init(Mutex::default)
    }

    /// Closes the gate for the work named `what` until the answer is dropped, which opens it.
    pub(crate) fn close(what: &str) -> Closed {
        let gate = Arc::new(Gate::default());
        closed()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(what.to_owned(), Arc::clone(&gate));
        Closed {
            what: what.to_owned(),
            gate,
        }
    }

    /// The gate the work named `what` waits at, where one is closed.
    pub(crate) fn of(what: &str) -> Option<Arc<Gate>> {
        closed()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(what)
            .cloned()
    }

    /// A gate a test closed: opened, and forgotten, when dropped.
    pub(crate) struct Closed {
        what: String,
        gate: Arc<Gate>,
    }

    impl Drop for Closed {
        fn drop(&mut self) {
            closed()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&self.what);
            self.gate.open();
        }
    }
}

#[cfg(test)]
#[path = "off_the_main_thread/tests/costs.rs"]
mod costs;

#[cfg(test)]
#[path = "off_the_main_thread/tests/lane_writes.rs"]
mod lane_writes;

#[cfg(test)]
pub(crate) mod tests;
