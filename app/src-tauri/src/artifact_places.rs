// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where the window goes for a `purlis://` link in an artifact's page** (ADR 0120 §5 as
//! amended, #5967; `docs/artifact-links.md`).
//!
//! The core reads the link into a [`Place`] of a closed list
//! ([`purlis_core::artifact::places`]). This module asks the artifact's own project whether the
//! place is there, and answers the window a [`Goes`]: a workspace to focus, a todo's view to
//! open, or a chat to show. Never anything else, so a link can only navigate.
//!
//! - A workspace must be one of the project's.
//! - A todo must be one of its workspace's open todos: the todo's view shows only those.
//! - A chat must be open here, found by its lasting id (ADR 0066).
//! - An issue goes to the chat open here that works on it (ADR 0088's work links); a todo's
//!   tracker key goes to the todo. With no such chat it is refused.

use std::path::Path;

use purlis_core::artifact::places::Place;
use purlis_core::work::TrackerKey;
use serde::Serialize;

/// **Where the window goes** for a link into purlis: only these, each a navigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "to", rename_all = "kebab-case")]
pub enum Goes {
    /// Focus workspace `workspace`.
    Workspace { workspace: String },
    /// Open todo `todo` of workspace `workspace`, called `title`, in its view.
    Todo {
        workspace: String,
        todo: String,
        title: String,
    },
    /// Show the chat open as `session`.
    Chat { session: u32 },
}

/// What the artifact's project holds, as a link into it is checked.
pub(crate) trait Here {
    /// The project's workspaces.
    fn workspaces(&self) -> Vec<String>;
    /// The title of workspace `workspace`'s open todo whose file's stem is `todo`, if it has one.
    fn todo_title(&self, workspace: &str, todo: &str) -> Option<String>;
    /// The session the chat whose lasting id is `chat` is open as, if it is open here.
    fn session_of(&self, chat: &str) -> Option<u32>;
    /// The ids of the chats linked to work item `key`.
    fn chats_on(&self, key: &TrackerKey) -> Vec<String>;
}

/// **Where the window goes for `place`**, in the project `here` describes, or the sentence that
/// refuses it.
pub(crate) fn goes(place: Place, here: &impl Here) -> Result<Goes, String> {
    match place {
        Place::Workspace { workspace } => {
            workspace_is(&workspace, here)?;
            Ok(Goes::Workspace { workspace })
        }
        Place::Todo { workspace, todo } => todo_is(workspace, todo, here),
        Place::Chat { chat } => here.session_of(&chat).map_or_else(
            || Err("that chat is not open in purlis now, so purlis did not follow the link".into()),
            |session| Ok(Goes::Chat { session }),
        ),
        Place::Issue { key } => {
            if let Some((workspace, todo)) = key.todo_parts() {
                return todo_is(workspace.to_owned(), todo.to_owned(), here);
            }
            here.chats_on(&key)
                .iter()
                .find_map(|chat| here.session_of(chat))
                .map(|session| Goes::Chat { session })
                .ok_or_else(|| {
                    format!(
                        "no chat open in purlis works on {key}, so purlis did not follow the link"
                    )
                })
        }
    }
}

fn workspace_is(workspace: &str, here: &impl Here) -> Result<(), String> {
    if here.workspaces().iter().any(|one| one == workspace) {
        Ok(())
    } else {
        Err(format!(
            "this project has no workspace {workspace}, so purlis did not follow the link"
        ))
    }
}

fn todo_is(workspace: String, todo: String, here: &impl Here) -> Result<Goes, String> {
    workspace_is(&workspace, here)?;
    match here.todo_title(&workspace, &todo) {
        Some(title) => Ok(Goes::Todo {
            workspace,
            todo,
            title,
        }),
        None => Err(format!(
            "{workspace} has no open todo {todo}, so purlis did not follow the link"
        )),
    }
}

/// **The project at `root`, with the chats `chats` holds**: what a link from one of its
/// artifacts is checked against.
pub(crate) struct Project<'a> {
    pub(crate) root: &'a Path,
    pub(crate) chats: &'a crate::chats::Chats,
}

impl Here for Project<'_> {
    fn workspaces(&self) -> Vec<String> {
        purlis_core::workspaces::Plane::open(self.root)
            .workspaces()
            .unwrap_or_default()
    }

    fn todo_title(&self, workspace: &str, todo: &str) -> Option<String> {
        purlis_core::workspaces::Plane::open(self.root)
            .workspace(workspace)
            .ok()?
            .todos()
            .ok()?
            .into_iter()
            .find(|one| one.slug == todo)
            .map(|one| one.title)
    }

    fn session_of(&self, chat: &str) -> Option<u32> {
        self.chats.session_of_id(chat)
    }

    fn chats_on(&self, key: &TrackerKey) -> Vec<String> {
        purlis_core::work::log::fold(self.root).chats_on(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const LINKED: &str = "01M70V414APWHKNRPG7YHK6J20";
    const CLOSED: &str = "01M70V414APWHKNRPG7YHK6J21";

    #[derive(Default)]
    struct Fake {
        open: HashMap<&'static str, u32>,
        on: HashMap<String, Vec<String>>,
    }

    impl Here for Fake {
        fn workspaces(&self) -> Vec<String> {
            vec!["alpha".into(), "beta".into()]
        }
        fn todo_title(&self, workspace: &str, todo: &str) -> Option<String> {
            (workspace == "alpha" && todo == "fix-login").then(|| "Fix the login".into())
        }
        fn session_of(&self, chat: &str) -> Option<u32> {
            self.open.get(chat).copied()
        }
        fn chats_on(&self, key: &TrackerKey) -> Vec<String> {
            self.on.get(key.as_str()).cloned().unwrap_or_default()
        }
    }

    fn here() -> Fake {
        Fake {
            open: HashMap::from([(LINKED, 7)]),
            on: HashMap::from([
                (
                    "github:github.com/purlis/purlis#5967".to_owned(),
                    vec![CLOSED.to_owned(), LINKED.to_owned()],
                ),
                (
                    "github:github.com/purlis/purlis#1".to_owned(),
                    vec![CLOSED.to_owned()],
                ),
            ]),
        }
    }

    fn issue(key: &str) -> Place {
        Place::Issue {
            key: TrackerKey::parse(key).unwrap(),
        }
    }

    #[test]
    fn a_workspace_of_the_project_is_focused_and_another_is_refused() {
        assert_eq!(
            goes(
                Place::Workspace {
                    workspace: "beta".into()
                },
                &here()
            ),
            Ok(Goes::Workspace {
                workspace: "beta".into()
            })
        );
        assert_eq!(
            goes(
                Place::Workspace {
                    workspace: "gamma".into()
                },
                &here()
            ),
            Err("this project has no workspace gamma, so purlis did not follow the link".into())
        );
    }

    #[test]
    fn a_todo_its_workspace_holds_is_opened_and_a_missing_one_refused() {
        let todo = |workspace: &str, todo: &str| Place::Todo {
            workspace: workspace.into(),
            todo: todo.into(),
        };
        assert_eq!(
            goes(todo("alpha", "fix-login"), &here()),
            Ok(Goes::Todo {
                workspace: "alpha".into(),
                todo: "fix-login".into(),
                title: "Fix the login".into()
            })
        );
        assert_eq!(
            goes(todo("alpha", "gone"), &here()),
            Err("alpha has no open todo gone, so purlis did not follow the link".into())
        );
        assert!(goes(todo("gamma", "fix-login"), &here()).is_err());
    }

    #[test]
    fn a_chat_open_here_is_shown_by_its_session_and_a_closed_one_refused() {
        assert_eq!(
            goes(
                Place::Chat {
                    chat: LINKED.into()
                },
                &here()
            ),
            Ok(Goes::Chat { session: 7 })
        );
        assert!(
            goes(
                Place::Chat {
                    chat: CLOSED.into()
                },
                &here()
            )
            .is_err_and(|why| why.contains("not open in purlis"))
        );
    }

    #[test]
    fn an_issue_goes_to_the_open_chat_that_works_on_it() {
        assert_eq!(
            goes(issue("github:github.com/purlis/purlis#5967"), &here()),
            Ok(Goes::Chat { session: 7 })
        );
        assert_eq!(
            goes(issue("github:github.com/purlis/purlis#1"), &here()),
            Err(
                "no chat open in purlis works on github:github.com/purlis/purlis#1, so purlis \
                 did not follow the link"
                    .into()
            )
        );
    }

    #[test]
    fn a_todo_s_tracker_key_goes_to_the_todo() {
        assert_eq!(
            goes(issue("todo:alpha/fix-login"), &here()),
            Ok(Goes::Todo {
                workspace: "alpha".into(),
                todo: "fix-login".into(),
                title: "Fix the login".into()
            })
        );
    }
}
