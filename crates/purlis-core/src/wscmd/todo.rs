// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Closing a workspace's todo: `workspace todo done <slug>` and `workspace todo forget <slug>`,
//! and the close a promote left unfinished, which `workspace todo` makes first (ADR 0088 §5).
//!
//! **Two entrances, one operation** (ADR 0067 §2), as [`super::memory`]'s: the terminal's
//! `purlis ws todo` calls these with its printer, and the app calls them for a sandboxed chat
//! that asked ([`crate::brokered`]), so both print the same lines.

use crate::repocmd::{Say, Sink};
use crate::workspaces::{Plane, Workspace};

/// How a todo is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Close {
    /// `done`: journalled ("Closed todo: …"), then dropped.
    Done,
    /// `forget`: abandoned, so dropped and nothing journalled.
    Forget,
}

impl Close {
    /// The verb, as `ws todo` takes it.
    pub fn verb(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Forget => "forget",
        }
    }
}

/// Finish closing every todo of `ws` a promote already aliased to an issue, when a crash cut
/// the promote short after its alias (ADR 0088 §5). Best effort: a log or a store purlis cannot
/// read closes nothing here, and `ws todo` goes on as it would have.
pub fn finish_promotes(plane: &Plane, ws: &Workspace, stamp: chrono::NaiveDateTime, say: Sink) {
    use crate::work::{log, promote};
    if !log::dir_for(plane.root(), ws.name()).exists() {
        return;
    }
    if let Ok(closed) = promote::finish_closes(ws, &log::fold(plane.root()), stamp) {
        for (title, to) in closed {
            say(Say::Done(
                format!(
                    "Closed '{title}' in '{}' — it was promoted to {to}.",
                    ws.name()
                )
                .into(),
            ));
        }
    }
}

/// `ws todo done|forget <slug>`: close one todo of `ws`, saying what it did. The exit code.
pub fn close(
    plane: &Plane,
    ws: &Workspace,
    how: Close,
    slug: &str,
    stamp: chrono::NaiveDateTime,
    say: Sink,
) -> u8 {
    let root = plane.root();
    let name = ws.name();
    let dir = ws.dir().join("todos");
    let see = format!("purlis ws todo --workspace {name}");
    let slug = crate::memstore::py_strip(slug);
    if !crate::contain::segment_ok(slug) {
        say(Say::Fail(crate::repocmd::clone::not_a_segment(slug).into()));
        say(Say::Info(format!("  List the real ones: {see}").into()));
        return 1;
    }
    let path = match crate::memstore::resolve(root, &dir, slug) {
        Ok(path) => path,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                say(Say::Fail(
                    format!("no todo '{slug}' in workspace '{name}'.").into(),
                ));
            } else {
                say(Say::Fail(e.to_string().into()));
            }
            say(Say::Info(format!("  List the real ones: {see}").into()));
            return 1;
        }
    };
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let title = ws
        .todos()
        .ok()
        .and_then(|open| open.into_iter().find(|t| t.slug == stem))
        .map_or_else(|| stem.clone(), |t| t.title);
    if how == Close::Done {
        // The journal entry first, while the todo is still there to name: it is the only
        // evidence that survives the close.
        match ws.remember_titled(&format!("Closed todo: {title}"), None, stamp) {
            Ok(journal) => super::memory::said_remembered(plane, name, &journal, false, say),
            Err(e) => {
                say(Say::Fail(e.to_string().into()));
                return 1;
            }
        }
    }
    if let Err(e) = ws.forget_todo(slug) {
        say(Say::Fail(e.to_string().into()));
        return 1;
    }
    say(Say::Done(match how {
        Close::Done => format!("Closed '{title}' in '{name}' — the journal has the trace.").into(),
        Close::Forget => {
            format!("Dropped '{title}' from '{name}' — abandoned, so nothing was journalled.")
                .into()
        }
    }));
    0
}
