// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a persona's `skills:` list names, one entry at a time (SK-5, #2768; ADR 0099 as amended
//! 2026-10-21), and the harness skills a machine has to check a named one against.

/// The prefixes that name a built-in skill: `purlis:<name>`, and `charter:<name>`, which personas
/// written before the rename still say. The migration rewrites the old one (#2784); until then
/// both mean the same skill.
pub const BUILT_IN_PREFIXES: [&str; 2] = ["purlis", "charter"];

/// The prefix of [`BUILT_IN_PREFIXES`] personas written before the rename say, which the migration
/// rewrites (#2784).
pub const OLD_BUILT_IN_PREFIX: &str = BUILT_IN_PREFIXES[1];

/// **Built-in skills that changed their name after they took [`crate::RESERVED_PREFIX`]**, the
/// name each had and the one it has (ADR 0099 as amended 2026-11-01). The old one is purlis's as
/// the prefix is, so it means this skill even bare. The four of #5724 (spec #5714 Q32, Q33) took
/// names that say what each is for; their runs, grades and misses carry over by this table, and
/// no stub keeps an old name in the bundle.
pub const RENAMED: &[(&str, &str)] = &[
    ("purlis-building-purlis-ui", "purlis-building-ui"),
    ("purlis-compact", "purlis-tidy-memory"),
    ("purlis-working-in-a-clone", "purlis-repo-work"),
    ("purlis-update", "purlis-upgrade"),
    ("purlis-add-curation-action", "purlis-curation-action"),
];

/// **Built-in skills purlis no longer ships, with no skill in their place**, each by its last
/// name and where what it said is now (ADR 0099 as amended 2026-11-12). Every name it had
/// ([`RENAMED`], `purlis:` or `charter:`, bare) still reads as it ([`Reference::retired`]), so a
/// persona's list naming it is told it was removed rather than that it names nothing, and a run
/// recorded under it keeps its name ([`crate::run_name`]).
pub const RETIRED: &[(&str, &str)] = &[(
    "purlis-building-ui",
    "its guide is `docs/building-ui.md` in the purlis repository",
)];

/// **What an entry that names another workspace's skill starts with** (SK-5a, #2933):
/// `workspaces/<ws>:<name>`, the plane's own path to the workspace, then the skill's name. No
/// plugin's name holds a `/`, so it names no harness skill.
pub const WORKSPACE_PREFIX: &str = "workspaces/";

/// One entry of a persona's `skills:` list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reference {
    /// `purlis:purlis-<name>`: a built-in skill, by its own name. `purlis:<name>` and
    /// `charter:<name>` read as it ([`Reference::parse`]).
    BuiltIn(String),
    /// `<plugin>:<name>`: a skill of a harness plugin, which the harness loads.
    Harness { plugin: String, name: String },
    /// `<name>`: the plane skill of that name the chat can reach (the persona's own, its
    /// workspace's, or a project skill, whatever it was granted), else the harness's own skill
    /// of that name.
    Named(String),
    /// `workspaces/<ws>:<name>`: the workspace skill `name` of the workspace `ws`, which reaches
    /// the persona's chats in every workspace, not only in `ws` (SK-5a, #2933; ADR 0099 as
    /// amended 2026-11-10). A bare `<name>` reaches only the chat's own workspace's.
    Workspace { workspace: String, name: String },
}

impl Reference {
    /// The entry `written`, as a persona's `skills:` list holds it, or `None` for one that names
    /// nothing: blank, or with a blank side to its `:`. A built-in skill written by a name from
    /// before each took [`crate::RESERVED_PREFIX`] (`purlis:handoff`) or from before it changed
    /// ([`RENAMED`]) reads as its name now (ADR 0099 as amended 2026-11-01), so no plane has to
    /// change.
    pub fn parse(written: &str) -> Option<Self> {
        let written = written.trim();
        if written.is_empty() {
            return None;
        }
        let Some((prefix, name)) = written.split_once(':') else {
            return Some(match renamed(written) {
                Some(own) => Self::BuiltIn(own.to_owned()),
                None => Self::Named(written.to_owned()),
            });
        };
        let (prefix, name) = (prefix.trim(), name.trim());
        if prefix.is_empty() || name.is_empty() {
            return None;
        }
        if let Some(workspace) = prefix.strip_prefix(WORKSPACE_PREFIX) {
            let workspace = workspace.trim();
            return crate::plain(workspace).then(|| Self::Workspace {
                workspace: workspace.to_owned(),
                name: name.to_owned(),
            });
        }
        Some(if BUILT_IN_PREFIXES.contains(&prefix) {
            Self::BuiltIn(built_in_named(name))
        } else {
            Self::Harness {
                plugin: prefix.to_owned(),
                name: name.to_owned(),
            }
        })
    }

    /// **Whether the entry `written` names a built-in skill with [`OLD_BUILT_IN_PREFIX`]**
    /// (#2934): one the migration rewrites (#2784). [`Self::parse`] reads it as the same
    /// [`Self::BuiltIn`] either way.
    pub fn written_old(written: &str) -> bool {
        matches!(Self::parse(written), Some(Self::BuiltIn(_)))
            && written
                .split_once(':')
                .is_some_and(|(prefix, _)| prefix.trim() == OLD_BUILT_IN_PREFIX)
    }

    /// **The old name and the new of the built-in skill the entry `written` names by a name it
    /// had before [`RENAMED`]** (#5724): one a persona's list still reads, and the lint asks to
    /// have rewritten. `None` for an entry by a name a skill has now, and for any other.
    pub fn written_renamed(written: &str) -> Option<(&'static str, &'static str)> {
        let written = written.trim();
        let name = written
            .split_once(':')
            .map_or(written, |(_, name)| name.trim());
        let own = if name.starts_with(crate::RESERVED_PREFIX) {
            name.to_owned()
        } else if written.contains(':') {
            format!("{}{name}", crate::RESERVED_PREFIX)
        } else {
            return None;
        };
        let prefixed = written
            .split_once(':')
            .is_none_or(|(prefix, _)| BUILT_IN_PREFIXES.contains(&prefix.trim()));
        RENAMED
            .iter()
            .find(|(old, _)| prefixed && *old == own)
            .copied()
    }

    /// **The retired built-in skill it names ([`RETIRED`]), with where what it said is now**:
    /// `purlis:<name>` or `charter:<name>` by any name it had, or its own name bare, which
    /// purlis's reserved prefix keeps from naming any other skill. `None` for any other entry.
    pub fn retired(&self) -> Option<(&'static str, &'static str)> {
        let name = match self {
            Self::BuiltIn(own) => own.as_str(),
            Self::Named(bare) => bare.as_str(),
            _ => return None,
        };
        RETIRED.iter().find(|(old, _)| *old == name).copied()
    }

    /// Whether it names the plane skill `name`: only a bare name can.
    pub fn names_plane(&self, name: &str) -> bool {
        matches!(self, Self::Named(named) if named == name)
    }

    /// Whether a machine with the harness skills `here` has it, as far as it can tell.
    pub(crate) fn on(&self, here: &[OnMachine]) -> Seen {
        let mut seen = Seen::Missing;
        for one in here {
            match (self, one) {
                (
                    Self::Named(name),
                    OnMachine::Skill {
                        plugin: None,
                        name: has,
                    },
                ) if name == has => return Seen::Here,
                (
                    Self::Harness { plugin, name },
                    OnMachine::Skill {
                        plugin: Some(from),
                        name: has,
                    },
                ) if plugin == from && name == has => return Seen::Here,
                (Self::Harness { plugin, .. }, OnMachine::Unlisted(from)) if plugin == from => {
                    seen = Seen::Unknown;
                }
                _ => {}
            }
        }
        seen
    }
}

/// The built-in skill `name` says, under whichever of its names it was written.
fn built_in_named(name: &str) -> String {
    let own = if name.starts_with(crate::RESERVED_PREFIX) {
        name.to_owned()
    } else {
        format!("{}{name}", crate::RESERVED_PREFIX)
    };
    renamed(&own).map_or(own, str::to_owned)
}

/// **The name a built-in skill has now, where `name` is one it had** ([`RENAMED`]), else
/// `name`: how a grade's row, a run and a miss recorded under an old name are read under the new.
pub fn name_now(name: &str) -> &str {
    renamed(name).unwrap_or(name)
}

/// **Every old name of a built-in skill ([`RENAMED`]) that `text` names**, with the name it has
/// now, each once, in the table's order: by its own name (`purlis-compact`) or by a command's
/// (`purlis:compact`, `charter:compact`), as a whole word. What the persona lint warns of in a
/// curation action's prompt (#5724), whose chat would ask for a skill no harness has.
pub fn old_names_in(text: &str) -> Vec<(&'static str, &'static str)> {
    RENAMED
        .iter()
        .filter(|(old, _)| {
            let short = &old[crate::RESERVED_PREFIX.len()..];
            [
                (*old).to_owned(),
                format!("{}:{short}", BUILT_IN_PREFIXES[0]),
                format!("{}:{short}", BUILT_IN_PREFIXES[1]),
            ]
            .iter()
            .any(|needle| named_whole(text, needle))
        })
        .copied()
        .collect()
}

/// Whether `text` holds `name` with no letter, digit, `-` or `_` on either side of it.
fn named_whole(text: &str, name: &str) -> bool {
    let part =
        |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
    text.match_indices(name).any(|(at, _)| {
        !part(text[..at].chars().next_back()) && !part(text[at + name.len()..].chars().next())
    })
}

/// The name now of the built-in skill that used to be `name`, if one did ([`RENAMED`]).
fn renamed(name: &str) -> Option<&'static str> {
    RENAMED
        .iter()
        .find(|(old, _)| *old == name)
        .map(|(_, new)| *new)
}

/// What a machine's harness skills say of one reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Seen {
    Here,
    /// Its plugin is installed, and its skills cannot be listed.
    Unknown,
    Missing,
}

/// How a persona's `skills:` list says it: `purlis:<name>` for a built-in skill (whichever prefix
/// was written), `<plugin>:<name>`, `workspaces/<ws>:<name>`, or `<name>`.
impl std::fmt::Display for Reference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuiltIn(name) => write!(f, "{}:{name}", BUILT_IN_PREFIXES[0]),
            Self::Harness { plugin, name } => write!(f, "{plugin}:{name}"),
            Self::Named(name) => f.write_str(name),
            Self::Workspace { workspace, name } => {
                write!(f, "{WORKSPACE_PREFIX}{workspace}:{name}")
            }
        }
    }
}

/// One harness skill a chat's harness has on this machine, as its declaration reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnMachine {
    /// A skill by name: the person's own (`plugin` `None`), or one of a plugin's.
    Skill {
        plugin: Option<String>,
        name: String,
    },
    /// A plugin installed whose skills cannot be listed: a skill named under it is neither said to
    /// be there nor said to be missing, only that purlis could not check it.
    Unlisted(String),
}
