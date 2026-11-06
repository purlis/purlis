// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The host's index of one project, kept up to date in the background (ADR 0079 §4).
//!
//! [`Keeper::start`] opens the [`Writer`] on a thread of its own, so a project opens without
//! waiting for a build: the first build, or the stat sweep at start, runs there. After it, three
//! things bring the index up to date, each re-reading only what it names:
//!
//! - **the watcher's paths** ([`Keeper::touched`]): each batch the project's watch reports, or
//!   every source when it cannot say what moved;
//! - **the commit range after a pull** ([`Keeper::pulled`]): the paths `git diff --name-only`
//!   names between the commit the index last reconciled and the one checked out now, or every
//!   source when the old commit is no longer an ancestor;
//! - **the stat sweep at start**, which the writer runs as it opens.
//!
//! While work is queued, readers are told the index is refreshing, and are answered anyway.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use super::Writer;

/// How long a burst of messages is gathered before the index is touched: one sweep for a pull
/// that lands a hundred files.
const GATHER: Duration = Duration::from_millis(50);

/// How long the keeper waits with nothing to do before it compacts deleted terms out of the file.
const IDLE: Duration = Duration::from_secs(600);

enum Message {
    Touched(Option<Vec<PathBuf>>),
    Pulled,
    /// Answered once every message sent before it has been handled.
    Settled(Sender<()>),
}

/// The background writer of one project's index. Dropping it stops the thread, which lets go of
/// `writer.lock`.
pub struct Keeper {
    sent: Option<Sender<Message>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Keeper {
    /// Starts keeping the index of the project at `root`. `say` is told what went wrong, once
    /// per failure, in a sentence.
    pub fn start(root: PathBuf, say: std::sync::Arc<dyn Fn(&str) + Send + Sync>) -> Self {
        let (sent, heard) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("project-index".into())
            .spawn(move || run(&root, &heard, say.as_ref()))
            .ok();
        Self {
            sent: Some(sent),
            thread,
        }
    }

    /// Paths that changed under the project, or `None` for "anything may have moved".
    pub fn touched(&self, paths: Option<Vec<PathBuf>>) {
        self.send(Message::Touched(paths));
    }

    /// The checked-out commit may have moved: a pull, a save's rebase, a branch change.
    pub fn pulled(&self) {
        self.send(Message::Pulled);
    }

    /// Waits until every change told before this call is in the index, or `within` has passed.
    /// Whether it settled.
    pub fn settle(&self, within: Duration) -> bool {
        let (tell, told) = mpsc::channel();
        self.send(Message::Settled(tell));
        told.recv_timeout(within).is_ok()
    }

    fn send(&self, message: Message) {
        if let Some(sent) = &self.sent {
            let _ = sent.send(message);
        }
    }
}

impl Drop for Keeper {
    fn drop(&mut self) {
        // Closing the channel ends the thread's loop.
        self.sent.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The keeper's thread.
fn run(root: &Path, heard: &Receiver<Message>, say: &(dyn Fn(&str) + Send + Sync)) {
    let mut writer = match Writer::open(root) {
        Ok(writer) => writer,
        Err(why) => {
            say(&format!(
                "{} is not indexed, so recall reads its files ({why})",
                root.display()
            ));
            // Answer every wait, so nothing blocks on an index that will not come.
            for message in heard {
                if let Message::Settled(tell) = message {
                    let _ = tell.send(());
                }
            }
            return;
        }
    };
    if let Some(head) = head(root) {
        let _ = writer.reconciled(&head);
    }
    let _ = writer.compact();
    let mut idle_since_delete = false;
    loop {
        let first = match heard.recv_timeout(IDLE) {
            Ok(message) => message,
            Err(RecvTimeoutError::Timeout) => {
                if idle_since_delete {
                    let _ = writer.compact();
                    idle_since_delete = false;
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let _ = writer.refreshing(true);
        let mut batch = vec![first];
        while let Ok(more) = heard.recv_timeout(GATHER) {
            batch.push(more);
        }
        let mut every = false;
        let mut paths: Vec<PathBuf> = Vec::new();
        let mut waiting = Vec::new();
        for message in batch {
            match message {
                Message::Touched(None) => every = true,
                Message::Touched(Some(more)) => paths.extend(more),
                Message::Pulled => match pulled_paths(root, writer.commit().as_deref()) {
                    Some(more) => paths.extend(more),
                    None => every = true,
                },
                Message::Settled(tell) => waiting.push(tell),
            }
        }
        let swept = if every {
            writer.sweep(None)
        } else if paths.is_empty() {
            Ok(Default::default())
        } else {
            writer.sweep(Some(&paths))
        };
        match swept {
            Ok(swept) => idle_since_delete |= swept.deleted > 0,
            Err(why) => say(&format!(
                "{}'s index could not be updated ({why})",
                root.display()
            )),
        }
        if let Some(head) = head(root) {
            let _ = writer.reconciled(&head);
        }
        let _ = writer.refreshing(false);
        for tell in waiting {
            let _ = tell.send(());
        }
    }
}

/// The commit checked out at `root`, if git can say.
fn head(root: &Path) -> Option<String> {
    let run = crate::worktree::git::run(
        root,
        &["rev-parse", "--verify", "-q", "HEAD"],
        crate::worktree::git::READ,
    )
    .ok()?;
    run.ok()
        .then(|| run.line().to_string())
        .filter(|h| !h.is_empty())
}

/// The paths a move from `since` to the commit checked out now changed, under the folders the
/// index reads; `None` when that cannot be told, and every source is swept instead.
fn pulled_paths(root: &Path, since: Option<&str>) -> Option<Vec<PathBuf>> {
    use crate::worktree::git::{READ, run};
    let since = since?;
    let now = head(root)?;
    if since == now {
        return Some(Vec::new());
    }
    let ancestor = run(root, &["merge-base", "--is-ancestor", since, &now], READ).ok()?;
    if !ancestor.ok() {
        return None;
    }
    let diff = run(
        root,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            "-z",
            since,
            &now,
            "--",
            "workspaces",
            "personas",
            "sessions",
        ],
        READ,
    )
    .ok()?;
    if !diff.ok() {
        return None;
    }
    Some(
        diff.out
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(|p| root.join(p))
            .collect(),
    )
}
