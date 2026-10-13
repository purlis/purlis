// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat asks the person to remove a workspace** (qw356).
//!
//! Removing a workspace deletes its memory, its todos and its clones: what other chats and the
//! person rely on. So a chat whose sandbox holds the removal never makes it, and until this
//! was written that refusal was where it ended, even in the chat purlis's own Safe remove
//! opens. Here the refusal becomes a question: `purlis workspace remove <name>` hands the name
//! to the app ([`crate::hookwire::Ask::Removal`]), the app holds it as a question on the asking
//! chat's tab, and **the person's press in the window is the only thing that removes
//! anything**, through the window's own remove and its guard ([`crate::wscmd::remove`]).
//!
//! **A request is a name, and the name is looked up** ([`named`]): it is a workspace of this
//! project as the project's own listing has it, or nothing is asked. Every word the question
//! says is purlis's; the name is drawn as data.
//!
//! The chat is told what the person decided on its next turn
//! ([`crate::dispatchtalk::RemovalWord`]), on the road the person's other answers take.

use std::path::Path;

/// The workspace `name` names in the project at `root`, as the project's own listing spells
/// it, or the sentence that says why nothing was asked: a name that cannot be a workspace's,
/// or no workspace of this project.
pub fn named(root: &Path, name: &str) -> Result<String, String> {
    if !crate::contain::workspace_name_ok(name) {
        return Err(no_such_said(name));
    }
    crate::workspaces::Plane::open(root)
        .workspaces()
        .ok()
        .and_then(|every| every.into_iter().find(|one| one == name))
        .ok_or_else(|| no_such_said(name))
}

/// What the asking chat's command says once the person has been asked. It replaces "ask the
/// person to remove it from the window": the asking is done, and the chat waits.
pub fn asked_said(workspace: &str) -> String {
    format!(
        "removing a workspace is the person's to decide, so nothing was removed: purlis has \
         asked the person, in its window, whether to remove '{}'. Wait for their answer. purlis \
         tells this chat what they decided on its next turn, so do not ask again and do not \
         remove it another way",
        crate::shown::short(workspace)
    )
}

/// What the asking chat's command says of a name that is no workspace of this project.
pub fn no_such_said(name: &str) -> String {
    format!(
        "this project has no workspace '{}', so the person was not asked and nothing was \
         removed",
        crate::shown::short(name)
    )
}

/// What a workspace holds that a removal deletes, as far as it is cheap to count: read from
/// the workspace's own stores, never from anything a chat said.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Holds {
    /// Its memories, archived ones left out.
    pub memories: usize,
    /// Its open todos.
    pub todos: usize,
}

/// What workspace `workspace` of the project at `root` holds now. A store that cannot be read
/// counts as none: the question still says what a removal deletes, in words.
pub fn holds(root: &Path, workspace: &str) -> Holds {
    let Ok(ws) = crate::workspaces::Plane::open(root).workspace(workspace) else {
        return Holds::default();
    };
    Holds {
        memories: ws.memories().map_or(0, |every| every.len()),
        todos: ws.todos().map_or(0, |every| every.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a project");
        std::fs::create_dir_all(dir.path().join("workspaces/alpha")).expect("alpha");
        dir
    }

    #[test]
    fn only_a_workspace_of_this_project_is_asked_about() {
        let dir = a_project();
        assert_eq!(named(dir.path(), "alpha"), Ok("alpha".to_owned()));
        // Not there, another spelling, a path, a control character: nothing is asked.
        for not_one in [
            "beta",
            "Alpha",
            "../alpha",
            "alpha/..",
            ".",
            "",
            "alpha\nbeta",
        ] {
            let why = named(dir.path(), not_one).expect_err(not_one);
            assert!(why.contains("the person was not asked"), "{why}");
            assert!(!why.contains('\n'), "{why:?}");
        }
    }

    #[test]
    fn what_the_command_says_is_that_the_person_was_asked_and_to_wait() {
        let said = asked_said("alpha");
        assert!(said.contains("nothing was removed"), "{said}");
        assert!(said.contains("asked the person, in its window"), "{said}");
        assert!(said.contains("Wait for their answer"), "{said}");
        // Never the old way round, which a sandboxed chat's person had to do by hand.
        assert!(!said.contains("terminal"), "{said}");
    }

    #[test]
    fn what_a_workspace_holds_is_counted_from_its_own_stores() {
        let dir = a_project();
        assert_eq!(holds(dir.path(), "alpha"), Holds::default());
        let ws = crate::workspaces::Plane::open(dir.path())
            .workspace("alpha")
            .expect("alpha");
        let now = chrono::NaiveDate::from_ymd_opt(2026, 10, 13)
            .and_then(|day| day.and_hms_opt(9, 0, 0))
            .expect("a time");
        ws.add_todo("Ship it", now).expect("a todo");
        ws.remember("Deploys go through the pipeline", now)
            .expect("a memory");
        assert_eq!(
            holds(dir.path(), "alpha"),
            Holds {
                memories: 1,
                todos: 1
            }
        );
        assert_eq!(holds(dir.path(), "no-such"), Holds::default());
    }
}
