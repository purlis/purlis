// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one path resolver** (M75-22, spec §11.8; Q317.4): where a path sits in the policy's
//! vocabulary, for the sandbox compile ([`super::sandbox_rules`]) and for a hook that asks the
//! host about a path. Both read a path's ancestors from here and nowhere else, so what the
//! profile holds and what the hook is told cannot differ by how each spelt a folder.
//!
//! **A `Path`'s ancestors are** (Q311.3, spec §3.1) every folder above it, the workspace that
//! owns it (`Path.workspace`, committed: the plane's own layout), the repo whose clone holds it,
//! the branch folder it is in, and the named `PathSet`s that list it or a folder above it. The
//! resolver reads **no file and runs nothing**: the plane's layout is handed in ([`Layout`]), and
//! a path is only text, normalised without touching the disk. A caller resolves links first, as
//! a sandbox does for any path it compiles.
//!
//! **Spelling.** A path is absolute and `/`-separated; `.`, `..` and doubled slashes are folded
//! away, and a `..` above the root is no path ([`normalise`]). A repo is `<workspace>/<folder>`
//! and a branch folder `<workspace>/<folder>#<piece>`; a repo folder with a `#` in its name is a
//! repo but has no branch folders, since its branch ids would read two ways. Windows paths are
//! not read: the compile serves Seatbelt, Landlock and Claude Code's sandbox.
//!
//! **Case and Unicode form** (spec §11.8). Names are compared as the bytes they are spelt in: a
//! caller hands the path as the filesystem spells it (a `realpath`, which on macOS's
//! case-insensitive APFS gives the stored case). So that a path spelt another way never reads as
//! outside the plane's own folders, [`resolve`] refuses (`None`, which every caller treats as a
//! deny) a path that names one of the layout's folders (`workspaces`, a workspace, its
//! `.worktrees`, a clone or a set member) with another case. It cannot do the same for a folder a
//! rule names only in its own text, nor for NFC against NFD, so those stay the caller's: a path
//! must reach the resolver in the filesystem's own form.

use std::collections::{BTreeMap, BTreeSet};

/// The kinds of entity a path can be inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// A folder, or the path itself.
    Path,
    /// A workspace of the plane.
    Workspace,
    /// A repo's clone.
    Repo,
    /// A branch folder `purlis` cut.
    Branch,
    /// A named set of paths.
    PathSet,
}

impl Kind {
    /// The entity type's name in the schema.
    pub fn type_name(self) -> &'static str {
        match self {
            Kind::Path => "Path",
            Kind::Workspace => "Workspace",
            Kind::Repo => "Repo",
            Kind::Branch => "Branch",
            Kind::PathSet => "PathSet",
        }
    }

    /// The kind a schema entity type names, among the five a sandbox rule may name.
    pub fn of_type(name: &str) -> Option<Self> {
        [
            Kind::Path,
            Kind::Workspace,
            Kind::Repo,
            Kind::Branch,
            Kind::PathSet,
        ]
        .into_iter()
        .find(|kind| kind.type_name() == name)
    }
}

/// One entity a path can be inside: its kind and its id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entity {
    pub kind: Kind,
    pub id: String,
}

impl Entity {
    pub fn new(kind: Kind, id: impl Into<String>) -> Self {
        Entity {
            kind,
            id: id.into(),
        }
    }
}

/// A repo's clone folder, as the plane has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloneFolder {
    pub workspace: String,
    /// The repo's folder name in the workspace.
    pub repo: String,
    /// Its folder, absolute.
    pub folder: String,
}

/// The plane's layout the resolver reads, handed in by the caller.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Layout {
    /// The plane's root, absolute.
    pub root: String,
    /// Its workspaces, by name.
    pub workspaces: Vec<String>,
    /// The clones of its repos.
    pub clones: Vec<CloneFolder>,
}

/// What a path is inside, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The path, normalised.
    pub path: String,
    /// The workspace that owns it (`Path.workspace`), if any.
    pub workspace: Option<String>,
    /// Every entity it is in, itself as a `Path` included (Cedar's `in` is reflexive).
    pub members: BTreeSet<Entity>,
}

impl Resolved {
    /// Whether the path is in `entity`.
    pub fn is_in(&self, entity: &Entity) -> bool {
        self.members.contains(entity)
    }

    /// The path as the world's entity JSON: its attribute and every ancestor, listed whole, since
    /// an ancestor the world does not define carries no ancestors of its own.
    pub fn entity_json(&self) -> serde_json::Value {
        let parents: Vec<serde_json::Value> = self
            .members
            .iter()
            .filter(|entity| !(entity.kind == Kind::Path && entity.id == self.path))
            .map(|entity| serde_json::json!({"type": entity.kind.type_name(), "id": entity.id}))
            .collect();
        let mut json = serde_json::json!({
            "uid": {"type": "Path", "id": self.path},
            "attrs": {},
            "parents": parents,
        });
        if let Some(workspace) = &self.workspace {
            json["attrs"] = serde_json::json!({
                "workspace": {"__entity": {"type": "Workspace", "id": workspace}}
            });
        }
        json
    }
}

/// `path` with `.`, `..` and doubled slashes folded away, or none where it is not absolute or a
/// `..` climbs above the root.
pub fn normalise(path: &str) -> Option<String> {
    if !path.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(format!("/{}", parts.join("/")))
}

/// Whether `path` is `folder` or inside it, both normalised.
pub fn within(path: &str, folder: &str) -> bool {
    folder == "/"
        || path == folder
        || path
            .strip_prefix(folder)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The folder a workspace is at in `layout`.
pub fn workspace_folder(layout: &Layout, workspace: &str) -> String {
    format!(
        "{}/workspaces/{workspace}",
        layout.root.trim_end_matches('/')
    )
}

/// A named set's member, as a folder: absolute, or relative to the plane's root.
pub fn member_folder(layout: &Layout, member: &str) -> Option<String> {
    let member = member.trim_end_matches('/');
    if member.starts_with('/') {
        normalise(member)
    } else {
        normalise(&format!("{}/{member}", layout.root.trim_end_matches('/')))
    }
}

/// **Resolves `path`** against the plane's `layout` and its named `sets` (set name to members).
/// `None` for a path that is not absolute, or climbs above the root.
pub fn resolve(
    layout: &Layout,
    sets: &BTreeMap<String, Vec<String>>,
    path: &str,
) -> Option<Resolved> {
    let path = normalise(path)?;
    if spelt_in_another_case(layout, sets, &path) {
        return None;
    }
    let mut members = BTreeSet::new();
    // Every folder above, and the path itself.
    let mut folder = String::new();
    for part in path.split('/').filter(|part| !part.is_empty()) {
        folder.push('/');
        folder.push_str(part);
        members.insert(Entity::new(Kind::Path, folder.clone()));
    }
    // The root is above every path, as `std::path::Path::ancestors` has it.
    members.insert(Entity::new(Kind::Path, "/"));
    let mut workspace = None;
    for name in &layout.workspaces {
        let folder = normalise(&workspace_folder(layout, name))?;
        if within(&path, &folder) {
            members.insert(Entity::new(Kind::Workspace, name.clone()));
            workspace = Some(name.clone());
            // `<workspace>/.worktrees/<repo>/<piece>` is a branch folder.
            let rest = path.strip_prefix(&folder).unwrap_or_default();
            let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
            if let [dir, repo, piece, ..] = parts.as_slice()
                && *dir == ".worktrees"
            {
                members.insert(Entity::new(Kind::Repo, format!("{name}/{repo}")));
                if !repo.contains('#') {
                    members.insert(Entity::new(Kind::Branch, format!("{name}/{repo}#{piece}")));
                }
            }
        }
    }
    for clone in &layout.clones {
        if normalise(&clone.folder).is_some_and(|folder| within(&path, &folder)) {
            members.insert(Entity::new(
                Kind::Repo,
                format!("{}/{}", clone.workspace, clone.repo),
            ));
        }
    }
    for (name, listed) in sets {
        let names = |member: &String| {
            member_folder(layout, member).is_some_and(|folder| within(&path, &folder))
        };
        if listed.iter().any(names) {
            members.insert(Entity::new(Kind::PathSet, name.clone()));
        }
    }
    Some(Resolved {
        path,
        workspace,
        members,
    })
}

/// Whether `path` is inside one of the layout's own folders when read without regard to case, and
/// not when read exactly: spelt as the filesystem may take it, and as the plane does not.
fn spelt_in_another_case(
    layout: &Layout,
    sets: &BTreeMap<String, Vec<String>>,
    path: &str,
) -> bool {
    let lowered = path.to_lowercase();
    let other = |folder: &str| !within(path, folder) && within(&lowered, &folder.to_lowercase());
    let mut folders = vec![format!("{}/workspaces", layout.root.trim_end_matches('/'))];
    for name in &layout.workspaces {
        let folder = workspace_folder(layout, name);
        folders.push(format!("{folder}/.worktrees"));
        folders.push(folder);
    }
    folders.extend(layout.clones.iter().map(|clone| clone.folder.clone()));
    for member in sets.values().flatten() {
        folders.extend(member_folder(layout, member));
    }
    folders
        .iter()
        .filter_map(|folder| normalise(folder))
        .any(|folder| other(&folder))
}
