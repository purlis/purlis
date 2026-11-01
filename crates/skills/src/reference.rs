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
}

impl Reference {
    /// The entry `written`, as a persona's `skills:` list holds it, or `None` for one that names
    /// nothing: blank, or with a blank side to its `:`. A built-in skill written by a name from
    /// before each took [`crate::RESERVED_PREFIX`] (`purlis:handoff`) reads as its name now
    /// (ADR 0099 as amended 2026-11-01), so no plane has to change.
    pub fn parse(written: &str) -> Option<Self> {
        let written = written.trim();
        if written.is_empty() {
            return None;
        }
        let Some((prefix, name)) = written.split_once(':') else {
            return Some(Self::Named(written.to_owned()));
        };
        let (prefix, name) = (prefix.trim(), name.trim());
        if prefix.is_empty() || name.is_empty() {
            return None;
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
    if name.starts_with(crate::RESERVED_PREFIX) {
        name.to_owned()
    } else {
        format!("{}{name}", crate::RESERVED_PREFIX)
    }
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
/// was written), `<plugin>:<name>`, or `<name>`.
impl std::fmt::Display for Reference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuiltIn(name) => write!(f, "{}:{name}", BUILT_IN_PREFIXES[0]),
            Self::Harness { plugin, name } => write!(f, "{plugin}:{name}"),
            Self::Named(name) => f.write_str(name),
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
