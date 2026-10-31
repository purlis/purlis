// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Closing a workspace's todo and opening it again: `workspace todo done <slug>`,
//! `workspace todo reopen <slug>` and `workspace todo forget <slug>`, and the close a promote
//! left unfinished, which `workspace todo` makes first (ADR 0088 §5).
//!
//! A done todo stays, in `todos/archive/`, until it is reopened or forgotten (ADR 0106).
//!
//! **Two entrances, one operation** (ADR 0067 §2), as [`super::memory`]'s: the terminal's
//! `purlis ws todo` calls these with its printer, and the app calls them for a sandboxed chat
//! that asked ([`crate::brokered`]), so both print the same lines.

use crate::repocmd::{Say, Sink};
use crate::workspaces::{Plane, Workspace};

/// How a todo's state is changed: closed as done, deleted, or opened again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Close {
    /// `done`: journalled ("Closed todo: …"), then kept as done in `todos/archive/`.
    Done,
    /// `forget`: deleted, open or done, and nothing journalled.
    Forget,
    /// `reopen`: a done todo is open again, and nothing journalled.
    Reopen,
}

impl Close {
    /// The verb, as `ws todo` takes it.
    pub fn verb(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Forget => "forget",
            Self::Reopen => "reopen",
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

/// Who asks a todo's close: the person at a terminal, or a chat the app started, through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asker {
    Person,
    /// A chat's `forget` reaches the open todos only: deleting a done one is the person's act,
    /// in the window or a terminal (ADR 0106).
    Chat,
}

/// `ws todo done|reopen|forget <slug>` at a terminal: [`close_as`] by the person.
pub fn close(
    plane: &Plane,
    ws: &Workspace,
    how: Close,
    slug: &str,
    stamp: chrono::NaiveDateTime,
    say: Sink,
) -> u8 {
    close_as(plane, ws, how, slug, stamp, Asker::Person, say)
}

/// `ws todo done|reopen|forget <slug>`: close one todo of `ws` as done, open a done one again,
/// or delete one — open or done for the person, open only for a chat — saying what it did. The
/// exit code.
pub fn close_as(
    plane: &Plane,
    ws: &Workspace,
    how: Close,
    slug: &str,
    stamp: chrono::NaiveDateTime,
    asker: Asker,
    say: Sink,
) -> u8 {
    let root = plane.root();
    let name = ws.name();
    let open = ws.dir().join("todos");
    let done = open.join(crate::memstore::ARCHIVE);
    let see = match how {
        Close::Reopen => format!("purlis ws todo --done --workspace {name}"),
        Close::Done | Close::Forget => format!("purlis ws todo --workspace {name}"),
    };
    let slug = crate::memstore::py_strip(slug);
    if !crate::contain::segment_ok(slug) {
        say(Say::Fail(crate::repocmd::clone::not_a_segment(slug).into()));
        say(Say::Info(format!("  List the real ones: {see}").into()));
        return 1;
    }
    // Where the todo is looked for: among the open ones to close it, the done ones to reopen
    // it, and both to delete it, the open ones first.
    let found = match how {
        Close::Done => crate::memstore::resolve(root, &open, slug),
        Close::Reopen => crate::memstore::resolve(root, &done, slug),
        Close::Forget => match crate::memstore::resolve(root, &open, slug) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && asker == Asker::Person => {
                crate::memstore::resolve(root, &done, slug)
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::NotFound
                    && crate::memstore::resolve(root, &done, slug).is_ok() =>
            {
                say(Say::Fail(
                    format!(
                        "'{slug}' in workspace '{name}' is done, and a chat does not delete a \
                         done todo."
                    )
                    .into(),
                ));
                say(Say::Info(
                    "  Delete it yourself: from the Todos view, or `purlis ws todo forget` in a \
                     terminal."
                        .into(),
                ));
                return 1;
            }
            found => found,
        },
    };
    let path = match found {
        Ok(path) => path,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                let what = if how == Close::Reopen {
                    "done todo"
                } else {
                    "todo"
                };
                say(Say::Fail(
                    format!("no {what} '{slug}' in workspace '{name}'.").into(),
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
    let was_done = path.parent() == Some(done.as_path());
    let listed = if was_done {
        ws.done_todos()
    } else {
        ws.todos()
    };
    let title = listed
        .ok()
        .and_then(|todos| todos.into_iter().find(|t| t.slug == stem))
        .map_or_else(|| stem.clone(), |t| t.title);
    let changed = match how {
        Close::Done => {
            // The journal entry first, while the todo is still open to name: a failure after
            // it leaves the todo open rather than closed with nothing recorded.
            match ws.remember_titled(&format!("Closed todo: {title}"), None, stamp) {
                Ok(journal) => super::memory::said_remembered(plane, name, &journal, false, say),
                Err(e) => {
                    say(Say::Fail(e.to_string().into()));
                    return 1;
                }
            }
            ws.file_as_done(&stem).map(|_| ())
        }
        Close::Reopen => ws.reopen_todo(&stem).map(|_| ()),
        Close::Forget if was_done => ws.forget_done_todo(&stem),
        Close::Forget => ws.forget_todo(&stem),
    };
    if let Err(e) = changed {
        say(Say::Fail(e.to_string().into()));
        return 1;
    }
    say(Say::Done(match how {
        Close::Done => format!("Closed '{title}' in '{name}' — the journal has the trace.").into(),
        Close::Reopen => format!("Reopened '{title}' in '{name}'.").into(),
        Close::Forget if was_done => {
            format!("Deleted the done todo '{title}' from '{name}'.").into()
        }
        Close::Forget => {
            format!("Dropped '{title}' from '{name}' — abandoned, so nothing was journalled.")
                .into()
        }
    }));
    if how == Close::Done {
        say(Say::Info(
            format!("  It stays as done. Reopen it: purlis ws todo reopen {stem}").into(),
        ));
    }
    0
}
