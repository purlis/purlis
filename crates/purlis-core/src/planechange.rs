// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What changed in a plane on disk, by kind and path (FD-10).
//!
//! The app watches a plane and used to say only *that* it moved (`plane-changed`), so every
//! panel read its whole answer again on every write: a memory an agent saved made the sidebar
//! list every workspace's todos once more. This module names what a changed path IS — a todo
//! of one workspace, a memory of one persona, a session record, a manifest — so a reader can
//! ask again only when the change is one its answer is made of.
//!
//! And it answers, for the window, which of the answers it reads a change concerns
//! ([`Answer`], [`answers`]): the readers name their answer, and the mapping is here.
//!
//! **It is a reading of the plane format's paths, nothing more** (`docs/plane-format.md`): no
//! file is opened, and a path is classified by where it is, never by what is in it.
//!
//! **Not knowing is said, never guessed.** A batch holding a path this cannot place — one
//! outside the plane, which is what a root spelled through a link looks like — is answered
//! `None`, which a reader takes as "anything may have moved" and reads everything again, as
//! it did before this module. A reader that misses a change draws a stale panel; one told too
//! much only reads again.

use std::collections::BTreeSet;
use std::path::{Component, Path};

/// What a changed path is part of, as the readers of the plane divide it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A file at the plane root — `charter.toml`, `charter.local.toml`, `CLAUDE.md` — or the
    /// root itself.
    Project,
    /// The harness settings and sub-agents a chat reads at its start: `.claude/`.
    Harness,
    /// A workspace's own files: `workspace.json`, a clone arriving or going, the workspace
    /// directory itself; or `workspaces/` itself, with no workspace named.
    Workspace,
    /// A workspace's `todos/`.
    Todos,
    /// A memory store: a workspace's `memory/` or a persona's (`_shared` included).
    Memory,
    /// Session records: a workspace's `sessions/`, or the plane root's.
    Sessions,
    /// A persona's own files (`persona.md` and the rest), the persona directory itself; or
    /// `personas/` itself, with no persona named.
    Persona,
    /// A skill's files at any level (SK-29, #2876): the project's `skills/` or `skills.local/`, a
    /// workspace's (its workspace named) or a persona's (its persona named), the level's folder
    /// itself included ([`skill_at`]). Its create, edit, grant, turn-off and delete all land here.
    Skills,
    /// The plane's git standing, and no file of its tree: auto-save committed, pushed or
    /// fetched and left the tree as it was ([`saved`]). Never a path's kind. A look that moved
    /// the tree — a fast-forward, a save's rebase — is told as not known instead, because the
    /// watcher does not see every file it moved.
    Git,
    /// How the project's chats stand, and no file of its tree: something the app holds about a
    /// chat that its row says has changed ([`chats`]). Never a path's kind.
    Chats,
}

/// One changed path, and what it is part of.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct Change {
    pub kind: Kind,
    /// The workspace it is in, where it is in one.
    pub workspace: Option<String>,
    /// The persona it belongs to, where it belongs to one (`_shared` for the shared store).
    pub persona: Option<String>,
    /// The path, relative to the plane root, with `/` between its parts.
    pub path: String,
}

/// What one path under `root` is, or `None` when it is not under `root`.
pub fn classify(root: &Path, path: &Path) -> Option<Change> {
    let inside = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for part in inside.components() {
        match part {
            Component::Normal(name) => parts.push(name.to_str()?.to_owned()),
            Component::CurDir => {}
            // `..` or a second root would put the path somewhere this cannot name.
            _ => return None,
        }
    }
    let named = |at: usize| parts.get(at).cloned();
    if let Some(skill) = skill_at(&parts.join("/")) {
        return Some(Change {
            kind: Kind::Skills,
            workspace: skill.whose.workspace().map(str::to_owned),
            persona: skill.whose.persona().map(str::to_owned),
            path: parts.join("/"),
        });
    }
    let (kind, workspace, persona) = match parts.first().map(String::as_str) {
        None => (Kind::Project, None, None),
        Some(".claude") => (Kind::Harness, None, None),
        Some("sessions") => (Kind::Sessions, None, None),
        Some("workspaces") => {
            let kind = match parts.get(2).map(String::as_str) {
                Some("todos") => Kind::Todos,
                Some("memory") => Kind::Memory,
                Some("sessions") => Kind::Sessions,
                _ => Kind::Workspace,
            };
            (kind, named(1), None)
        }
        Some("personas") => {
            let kind = match parts.get(2).map(String::as_str) {
                Some("memory") => Kind::Memory,
                _ => Kind::Persona,
            };
            (kind, None, named(1))
        }
        Some(_) => (Kind::Project, None, None),
    };
    Some(Change {
        kind,
        workspace,
        persona,
        path: parts.join("/"),
    })
}

/// What a batch of changed paths under `root` is, each change once, in a stable order — or
/// `None` when any one of them cannot be placed (see the module's header).
pub fn of_batch<'a>(root: &Path, paths: impl IntoIterator<Item = &'a Path>) -> Option<Vec<Change>> {
    let mut changes = BTreeSet::new();
    for path in paths {
        changes.insert(classify(root, path)?);
    }
    Some(changes.into_iter().collect())
}

/// What auto-save did to the plane: a save, a push or a fetch, which moves the git standing
/// and no answer read from the tree.
pub fn saved() -> Change {
    Change {
        kind: Kind::Git,
        workspace: None,
        persona: None,
        path: String::new(),
    }
}

/// What the app holds about a chat has changed in a way its row says (#1484): a task's
/// question to the chat that asked was opened or answered, or the report it owes was settled.
/// The rows are part of the sidebar's answer and of no other, and nothing on disk says so.
pub fn chats() -> Change {
    Change {
        kind: Kind::Chats,
        workspace: None,
        persona: None,
        path: String::new(),
    }
}

/// One answer the window reads from the plane, named so a reader can say which one it is and
/// be told again only when a change concerns it.
///
/// **The question "which answer does this change concern" is the core's**, here beside what
/// the answers are read from: the window names the answer it holds and nothing else, so a
/// command that comes to read another store changes this mapping and not every reader.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(tag = "answer", rename_all = "camelCase")]
pub enum Answer {
    /// The sidebar (`plane_sidebar`): the workspaces with their todos, visions, colours and
    /// LIVE marks, and the personas with the default one.
    Sidebar,
    /// A workspace's panels (`workspace_panels`): everything in that workspace, and the
    /// personas with their memory counts. `None` is every workspace's: a change to the
    /// personas or the project concerns whichever is focused.
    Panels { workspace: Option<String> },
    /// The plane root's panels (`plane_root_panels`): its session records.
    RootPanels,
    /// The instructions a chat reads once at its start (`chats_plane_updated`,
    /// [`crate::instructions::Stamp`]): `CLAUDE.md` and the project's files, the harness
    /// settings and sub-agents, each persona's charter. Never a workspace, a todo, a memory
    /// or a session record (FD-10d).
    Instructions,
    /// The curation actions offered on each subject (`curation_offers`): the personas'
    /// curation files, the project's launch profile and harness declarations, and the
    /// workspaces a subject can be. Never a todo, a memory or a session record (FD-10d).
    Curations,
    /// What the project has on and the theme it draws: `charter.toml`, `charter.local.toml`
    /// and each `workspace.json`.
    Settings,
    /// The git standings (the alerts, the Saving rows): the plane's shape, and what auto-save
    /// did. Not a memory or a session record — the commonest write there is, and the standings
    /// follow it on their own clock (FD-10).
    Git,
    /// The view tabs: any of the plane's stores, and nothing auto-save did.
    Views,
}

/// The answers `changes` concern, each once, in a stable order — or `None`, "every answer",
/// when what changed is not known.
pub fn answers(changes: Option<&[Change]>) -> Option<Vec<Answer>> {
    let mut concerned = BTreeSet::new();
    for change in changes? {
        concerned.extend(answers_of(change));
    }
    Some(concerned.into_iter().collect())
}

/// The answers one change concerns.
fn answers_of(change: &Change) -> Vec<Answer> {
    let panels = || Answer::Panels {
        workspace: change.workspace.clone(),
    };
    let every_panel = || Answer::Panels { workspace: None };
    match change.kind {
        Kind::Project => vec![
            Answer::Sidebar,
            every_panel(),
            Answer::Instructions,
            Answer::Curations,
            Answer::Settings,
            Answer::Git,
            Answer::Views,
        ],
        Kind::Harness => vec![
            Answer::Instructions,
            Answer::Curations,
            Answer::Git,
            Answer::Views,
        ],
        // A skill is part of what its level's chats started with (#2770): the window asks again
        // which chats run on skills that changed since. Each level's skills concern what its
        // other files do, the instructions with them.
        Kind::Skills if change.persona.is_some() => answers_of(&Change {
            kind: Kind::Persona,
            ..change.clone()
        }),
        Kind::Skills if change.workspace.is_some() => vec![
            Answer::Sidebar,
            panels(),
            Answer::Instructions,
            Answer::Curations,
            Answer::Settings,
            Answer::Git,
            Answer::Views,
        ],
        Kind::Skills => answers_of(&Change {
            kind: Kind::Project,
            ..change.clone()
        }),
        Kind::Workspace => vec![
            Answer::Sidebar,
            panels(),
            Answer::Curations,
            Answer::Settings,
            Answer::Git,
            Answer::Views,
        ],
        Kind::Todos => vec![Answer::Sidebar, panels(), Answer::Git, Answer::Views],
        Kind::Memory => vec![panels(), Answer::Views],
        Kind::Sessions if change.workspace.is_none() => vec![Answer::RootPanels, Answer::Views],
        Kind::Sessions => vec![panels(), Answer::Views],
        Kind::Persona => vec![
            Answer::Sidebar,
            every_panel(),
            Answer::Instructions,
            Answer::Curations,
            Answer::Git,
            Answer::Views,
        ],
        Kind::Git => vec![Answer::Git],
        Kind::Chats => vec![Answer::Sidebar],
    }
}

/// **Where a skill's path is**, by the one rule of where a level's skills are
/// ([`purlis_skills::level_of_rel`]): the level's owner, whether it is the local folder, and the
/// skill's folder name where the path is in one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillAt {
    pub whose: purlis_skills::Whose,
    pub local: bool,
    /// The skill's folder name, or the name of the file beside it without its `.toml`; `None`
    /// for the level's folder itself.
    pub folder: Option<String>,
}

/// [`SkillAt`] of `path` (plane-relative, `/` between its parts), or `None` for a path in no
/// level's skills: `skills/deploy/SKILL.md`, `workspaces/ops/skills.local/x/run.sh`,
/// `personas/reviewer/skills/review.toml`.
pub fn skill_at(path: &str) -> Option<SkillAt> {
    let parts: Vec<&str> = path.split('/').collect();
    // The project's level is its first part; a workspace's or a persona's, its first three.
    let deep = matches!(parts.first(), Some(&"workspaces" | &"personas"));
    let depth = if deep { 3 } else { 1 };
    let level = parts.get(..depth)?.join("/");
    let (whose, local) = purlis_skills::level_of_rel(&level)?;
    let folder = parts.get(depth).map(|name| {
        name.strip_suffix(".toml")
            .filter(|_| parts.len() == depth + 1)
            .unwrap_or(name)
            .to_owned()
    });
    Some(SkillAt {
        whose,
        local,
        folder,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skill_at_every_level_is_the_skills_kind_of_its_owner() {
        assert_eq!(
            at("skills/deploy/SKILL.md"),
            change(Kind::Skills, None, None, "skills/deploy/SKILL.md")
        );
        assert_eq!(at("skills.local/x").kind, Kind::Skills);
        assert_eq!(at("skills").kind, Kind::Skills, "the level's folder itself");
        assert_eq!(
            at("workspaces/ops/skills/deploy.toml"),
            change(
                Kind::Skills,
                Some("ops"),
                None,
                "workspaces/ops/skills/deploy.toml"
            )
        );
        assert_eq!(
            at("personas/reviewer/skills.local/r/SKILL.md"),
            change(
                Kind::Skills,
                None,
                Some("reviewer"),
                "personas/reviewer/skills.local/r/SKILL.md"
            )
        );
        // Beside a level, and not one.
        assert_eq!(at("skillset.md").kind, Kind::Project);
        assert_eq!(at("workspaces/ops/skillset.md").kind, Kind::Workspace);
        assert_eq!(at("personas/reviewer/persona.md").kind, Kind::Persona);
        assert_eq!(at("workspaces/ops/todos/skills").kind, Kind::Todos);
        assert_eq!(at(".claude/skills/x/SKILL.md").kind, Kind::Harness);
    }

    #[test]
    fn a_skills_path_says_its_level_and_its_folder() {
        assert_eq!(
            skill_at("workspaces/ops/skills.local/deploy/scripts/run.sh"),
            Some(SkillAt {
                whose: purlis_skills::Whose::Workspace("ops".to_owned()),
                local: true,
                folder: Some("deploy".to_owned()),
            })
        );
        assert_eq!(
            skill_at("skills/deploy.toml").and_then(|one| one.folder),
            Some("deploy".to_owned()),
            "the file beside a folder is that skill's"
        );
        assert_eq!(skill_at("skills").and_then(|one| one.folder), None);
        assert_eq!(skill_at("workspaces/ops"), None);
        assert_eq!(skill_at("workspaces/../skills/x"), None);
    }

    #[test]
    fn a_skill_concerns_what_its_levels_other_files_concern_and_the_instructions() {
        let project = answers_to(at("charter.toml"));
        assert_eq!(answers_to(at("skills/deploy/SKILL.md")), project);
        let persona = answers_to(at("personas/steward/persona.md"));
        assert_eq!(
            answers_to(at("personas/steward/skills/r/SKILL.md")),
            persona
        );
        assert_eq!(
            answers_to(at("workspaces/alpha/skills/deploy/SKILL.md")),
            vec![
                Answer::Sidebar,
                panels(Some("alpha")),
                Answer::Instructions,
                Answer::Curations,
                Answer::Settings,
                Answer::Git,
                Answer::Views
            ]
        );
    }

    #[test]
    fn a_workspaces_skill_concerns_what_its_chats_started_with_and_its_todo_does_not() {
        assert!(
            answers_of(&at("workspaces/ops/skills/deploy/SKILL.md"))
                .contains(&Answer::Instructions)
        );
        assert!(
            answers_of(&at("workspaces/ops/skills.local/x/SKILL.md"))
                .contains(&Answer::Instructions)
        );
        assert!(answers_of(&at("skills/deploy/SKILL.md")).contains(&Answer::Instructions));
        assert!(!answers_of(&at("workspaces/ops/todos/a.md")).contains(&Answer::Instructions));
        assert!(!answers_of(&at("workspaces/ops/skillset.md")).contains(&Answer::Instructions));
    }

    const ROOT: &str = "/home/dev/plane";

    fn at(path: &str) -> Change {
        classify(Path::new(ROOT), &Path::new(ROOT).join(path)).expect("inside the plane")
    }

    fn change(kind: Kind, workspace: Option<&str>, persona: Option<&str>, path: &str) -> Change {
        Change {
            kind,
            workspace: workspace.map(str::to_owned),
            persona: persona.map(str::to_owned),
            path: path.to_owned(),
        }
    }

    #[test]
    fn a_path_spelled_from_the_current_directory_is_placed_like_any_other() {
        // A relative root of "" leaves the path's own leading `.`: it names nothing, and must
        // not read as a part this cannot place.
        assert_eq!(
            classify(Path::new(""), Path::new("./workspaces/alpha/todos/m8-1.md")),
            Some(change(
                Kind::Todos,
                Some("alpha"),
                None,
                "workspaces/alpha/todos/m8-1.md"
            ))
        );
    }

    #[test]
    fn a_todo_is_a_todo_of_its_workspace() {
        assert_eq!(
            at("workspaces/alpha/todos/m8-1.md"),
            change(
                Kind::Todos,
                Some("alpha"),
                None,
                "workspaces/alpha/todos/m8-1.md"
            )
        );
    }

    #[test]
    fn a_workspace_memory_and_a_persona_memory_are_both_memory_of_their_owner() {
        assert_eq!(
            at("workspaces/alpha/memory/20260919-120000-note.md"),
            change(
                Kind::Memory,
                Some("alpha"),
                None,
                "workspaces/alpha/memory/20260919-120000-note.md"
            )
        );
        assert_eq!(
            at("personas/steward/memory/MEMORY.md"),
            change(
                Kind::Memory,
                None,
                Some("steward"),
                "personas/steward/memory/MEMORY.md"
            )
        );
        assert_eq!(
            at("personas/_shared/memory/fact.md").persona.as_deref(),
            Some("_shared")
        );
    }

    #[test]
    fn session_records_are_sessions_of_their_workspace_or_of_the_root() {
        assert_eq!(
            at("workspaces/alpha/sessions/2026-10-02-1650-x.md"),
            change(
                Kind::Sessions,
                Some("alpha"),
                None,
                "workspaces/alpha/sessions/2026-10-02-1650-x.md"
            )
        );
        assert_eq!(
            at("sessions/2026-10-02-1650-x.md"),
            change(Kind::Sessions, None, None, "sessions/2026-10-02-1650-x.md")
        );
    }

    #[test]
    fn a_manifest_a_clone_or_the_workspace_itself_is_the_workspace() {
        for path in [
            "workspaces/alpha",
            "workspaces/alpha/workspace.json",
            "workspaces/alpha/svc",
        ] {
            assert_eq!(at(path), change(Kind::Workspace, Some("alpha"), None, path));
        }
        assert_eq!(
            at("workspaces"),
            change(Kind::Workspace, None, None, "workspaces")
        );
    }

    #[test]
    fn a_persona_charter_is_its_persona_and_the_root_files_are_the_project() {
        assert_eq!(
            at("personas/steward/persona.md"),
            change(
                Kind::Persona,
                None,
                Some("steward"),
                "personas/steward/persona.md"
            )
        );
        assert_eq!(
            at("personas"),
            change(Kind::Persona, None, None, "personas")
        );
        assert_eq!(at(".claude/agents/x.md").kind, Kind::Harness);
        assert_eq!(
            at("charter.toml"),
            change(Kind::Project, None, None, "charter.toml")
        );
        assert_eq!(at("CLAUDE.md").kind, Kind::Project);
        assert_eq!(at(""), change(Kind::Project, None, None, ""));
    }

    #[test]
    fn a_path_outside_the_plane_is_not_placed() {
        assert_eq!(
            classify(Path::new(ROOT), Path::new("/home/dev/elsewhere/x.md")),
            None
        );
    }

    #[test]
    fn a_batch_is_each_change_once_and_unknown_as_a_whole_if_one_path_is_unplaced() {
        let root = Path::new(ROOT);
        let todo = root.join("workspaces/alpha/todos/a.md");
        let memory = root.join("workspaces/alpha/memory/b.md");
        let batch = [todo.as_path(), memory.as_path(), todo.as_path()];
        assert_eq!(
            of_batch(root, batch),
            Some(vec![
                change(
                    Kind::Todos,
                    Some("alpha"),
                    None,
                    "workspaces/alpha/todos/a.md"
                ),
                change(
                    Kind::Memory,
                    Some("alpha"),
                    None,
                    "workspaces/alpha/memory/b.md"
                ),
            ])
        );
        let outside = Path::new("/elsewhere/c.md");
        assert_eq!(of_batch(root, [todo.as_path(), outside]), None);
    }

    fn answers_to(change: Change) -> Vec<Answer> {
        answers(Some(&[change])).expect("placed")
    }

    fn panels(workspace: Option<&str>) -> Answer {
        Answer::Panels {
            workspace: workspace.map(str::to_owned),
        }
    }

    #[test]
    fn not_knowing_what_changed_concerns_every_answer() {
        assert_eq!(answers(None), None);
    }

    #[test]
    fn a_todo_concerns_the_sidebar_its_own_workspaces_panels_and_the_git_standings() {
        // Not the instructions a chat started on, nor the curations: neither is made of a
        // todo (FD-10d).
        assert_eq!(
            answers_to(at("workspaces/beta/todos/a.md")),
            vec![
                Answer::Sidebar,
                panels(Some("beta")),
                Answer::Git,
                Answer::Views
            ]
        );
    }

    #[test]
    fn a_memory_concerns_its_workspaces_panels_and_the_views_and_never_the_sidebar() {
        assert_eq!(
            answers_to(at("workspaces/beta/memory/m.md")),
            vec![panels(Some("beta")), Answer::Views]
        );
        // A persona's memory is counted on every workspace's Personas panel.
        assert_eq!(
            answers_to(at("personas/steward/memory/m.md")),
            vec![panels(None), Answer::Views]
        );
    }

    #[test]
    fn a_session_record_concerns_its_workspaces_panels_or_the_plane_roots() {
        assert_eq!(
            answers_to(at("workspaces/alpha/sessions/r.md")),
            vec![panels(Some("alpha")), Answer::Views]
        );
        assert_eq!(
            answers_to(at("sessions/r.md")),
            vec![Answer::RootPanels, Answer::Views]
        );
    }

    #[test]
    fn the_settings_files_concern_the_settings_and_the_harness_the_instructions_and_curations() {
        assert_eq!(
            answers_to(at("charter.toml")),
            vec![
                Answer::Sidebar,
                panels(None),
                Answer::Instructions,
                Answer::Curations,
                Answer::Settings,
                Answer::Git,
                Answer::Views
            ]
        );
        assert_eq!(
            answers_to(at("workspaces/alpha/workspace.json")),
            vec![
                Answer::Sidebar,
                panels(Some("alpha")),
                Answer::Curations,
                Answer::Settings,
                Answer::Git,
                Answer::Views
            ]
        );
        assert_eq!(
            answers_to(at(".claude/agents/x.md")),
            vec![
                Answer::Instructions,
                Answer::Curations,
                Answer::Git,
                Answer::Views
            ]
        );
        assert_eq!(
            answers_to(at("personas/steward/persona.md")),
            vec![
                Answer::Sidebar,
                panels(None),
                Answer::Instructions,
                Answer::Curations,
                Answer::Git,
                Answer::Views
            ]
        );
    }

    #[test]
    fn a_save_concerns_the_git_standings_and_nothing_read_from_the_tree() {
        assert_eq!(answers_to(saved()), vec![Answer::Git]);
    }

    #[test]
    fn how_the_chats_stand_concerns_the_sidebar_and_nothing_else() {
        assert_eq!(answers_to(chats()), vec![Answer::Sidebar]);
    }

    #[test]
    fn a_batch_concerns_each_answer_once() {
        let root = Path::new(ROOT);
        let changes = of_batch(
            root,
            [
                root.join("workspaces/alpha/memory/a.md").as_path(),
                root.join("workspaces/alpha/memory/b.md").as_path(),
            ],
        )
        .expect("placed");
        assert_eq!(
            answers(Some(&changes)),
            Some(vec![panels(Some("alpha")), Answer::Views])
        );
    }
}
