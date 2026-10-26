// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What each harness does with a skill's frontmatter** (SK-8, #2771; decisions 18 and 58): the
//! compatibility badge a skill shows for each harness, and the `SKILL.md` purlis hands a harness
//! in place of the skill's own where the two must differ.
//!
//! A harness is described by what it reads ([`Honours`]), which its adapter in purlis-core says,
//! so this module names no harness. Two rules hold on every one:
//!
//! - **A skill can only narrow a chat** (decision 58). `allowed-tools` pre-approves tools and
//!   `hooks` runs commands, so both widen a chat: purlis never hands either over for a skill it
//!   hands ([`handed_text`]), and the badge says so. A harness or repo skill, which the harness
//!   reads where it lies, is marked as widening the chat where its harness applies them.
//!   `disallowed-tools` and `context: fork` narrow, and pass through where the harness applies
//!   them; elsewhere the badge says the narrowing is not applied.
//! - **Nothing is silently half-applied** (decision 18): every key a harness does not apply is on
//!   the badge, and a key that has an equivalent is translated ([`ManualOnly`]).

use crate::Origin;
use crate::handed::Refusal;

/// A frontmatter key a harness may read, by what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Field {
    Name,
    Description,
    /// `when_to_use`: more of when to use it.
    WhenToUse,
    /// `argument-hint`: what to pass it.
    ArgumentHint,
    /// `arguments`: the names of its arguments.
    Arguments,
    /// `disable-model-invocation`: manual-only.
    ManualOnly,
    /// `user-invocable`: whether a person can start it by name.
    UserInvocable,
    /// `allowed-tools`: tools pre-approved while it runs. Widens the chat.
    AllowedTools,
    /// `disallowed-tools`: tools taken away while it runs. Narrows the chat.
    DisallowedTools,
    Model,
    Effort,
    /// `context`: `fork` runs it in a fork of the chat. Narrows the chat.
    Context,
    /// `agent`: what its fork runs as.
    Agent,
    /// `hooks`: commands run on the chat's events. Widens the chat.
    Hooks,
    /// `paths`: the files it is for.
    Paths,
    /// `shell`: the shell its commands run in.
    Shell,
    /// `background`: whether it runs in the background.
    Background,
    /// `license`, `compatibility` and `metadata`: for a reader of the skill, never applied.
    License,
    Compatibility,
    Metadata,
}

impl Field {
    /// Every field, in the order a badge lists them.
    pub(crate) const ALL: [Field; 20] = [
        Field::Name,
        Field::Description,
        Field::WhenToUse,
        Field::ArgumentHint,
        Field::Arguments,
        Field::ManualOnly,
        Field::UserInvocable,
        Field::AllowedTools,
        Field::DisallowedTools,
        Field::Model,
        Field::Effort,
        Field::Context,
        Field::Agent,
        Field::Hooks,
        Field::Paths,
        Field::Shell,
        Field::Background,
        Field::License,
        Field::Compatibility,
        Field::Metadata,
    ];

    /// The key a `SKILL.md` writes it by.
    pub fn key(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Description => "description",
            Self::WhenToUse => "when_to_use",
            Self::ArgumentHint => "argument-hint",
            Self::Arguments => "arguments",
            Self::ManualOnly => "disable-model-invocation",
            Self::UserInvocable => "user-invocable",
            Self::AllowedTools => "allowed-tools",
            Self::DisallowedTools => "disallowed-tools",
            Self::Model => "model",
            Self::Effort => "effort",
            Self::Context => "context",
            Self::Agent => "agent",
            Self::Hooks => "hooks",
            Self::Paths => "paths",
            Self::Shell => "shell",
            Self::Background => "background",
            Self::License => "license",
            Self::Compatibility => "compatibility",
            Self::Metadata => "metadata",
        }
    }

    /// The field `key` writes, or `None` for a key purlis knows no harness to read.
    pub fn of(key: &str) -> Option<Field> {
        Self::ALL.into_iter().find(|field| field.key() == key)
    }

    /// What it asks for, as a person reads it on a badge.
    pub(crate) fn said(self) -> &'static str {
        match self {
            Self::Name => "its name",
            Self::Description => "its description",
            Self::WhenToUse => "when to use it",
            Self::ArgumentHint => "what to pass it",
            Self::Arguments => "its arguments",
            Self::ManualOnly => "running only when asked for",
            Self::UserInvocable => "whether it can be started by name",
            Self::AllowedTools => "the tools it pre-approves",
            Self::DisallowedTools => "the tools it takes away",
            Self::Model => "the model it asks for",
            Self::Effort => "the effort it asks for",
            Self::Context => "running in a fork of the chat",
            Self::Agent => "what its fork runs as",
            Self::Hooks => "the commands it adds to the chat's events",
            Self::Paths => "the files it is for",
            Self::Shell => "the shell its commands run in",
            Self::Background => "running in the background",
            Self::License => "its license",
            Self::Compatibility => "what it says it needs",
            Self::Metadata => "its metadata",
        }
    }

    /// Whether it widens a chat it applies to: more tools, or commands of its own.
    pub fn widens(self) -> bool {
        matches!(self, Self::AllowedTools | Self::Hooks)
    }

    /// Whether it narrows a chat it applies to: fewer tools, or a fork.
    pub(crate) fn narrows(self) -> bool {
        matches!(self, Self::DisallowedTools | Self::Context | Self::Agent)
    }

    /// Whether purlis reads it itself, whatever the harness: the name and description it lists,
    /// and the arguments Use skill… asks for (decision 61).
    fn purlis_reads(self) -> bool {
        matches!(
            self,
            Self::Name | Self::Description | Self::Arguments | Self::ArgumentHint
        )
    }

    /// Whether it is for a reader of the skill and never applied, by any harness: no badge
    /// counts it.
    fn for_a_reader(self) -> bool {
        matches!(self, Self::License | Self::Compatibility | Self::Metadata)
    }
}

/// **How a harness keeps a manual-only skill out of its model's choice**, as its adapter says.
/// Every harness purlis runs has a way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualOnly {
    /// It reads `disable-model-invocation` itself, and leaves the skill's description out.
    Read,
    /// purlis lists its skills to it ([`crate::Route::Briefing`]), and lists a manual-only one
    /// by its name and file alone, to run only when the person names it ([`crate::listing`]).
    Listing,
    /// It lists only a skill that has a description, and still runs one by name: purlis hands
    /// it the skill without its description ([`handed_text`]).
    WithoutDescription,
}

/// **What a harness reads of a skill's frontmatter**: all this module needs to know of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Honours {
    /// The fields it applies, beyond the ones purlis reads itself on every harness (the name,
    /// the description and the arguments).
    pub reads: &'static [Field],
    /// How it keeps a manual-only skill out of its model's choice.
    pub manual: ManualOnly,
    /// The agents of its own a skill's fork may run as (`context: fork` with `agent`), which
    /// carry no settings a skill could bring. A fork as any other agent, such as a repo's, which
    /// may carry its own permissions and hooks, would widen the chat: purlis leaves its `agent`
    /// out ([`crate::handed::Frontmatter::emit`]).
    pub agents: &'static [&'static str],
}

impl Honours {
    fn applies(&self, field: Field) -> bool {
        field.purlis_reads() || self.reads.contains(&field)
    }
}

/// How well a skill fits a harness, as its badge says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Everything it asks for applies.
    Full,
    /// Something it asks for is left out, ignored or not applied, and the badge says what.
    Partial,
    /// A harness or repo skill whose harness applies what widens the chat, or whose frontmatter
    /// purlis cannot read, so may.
    Widens,
    /// A skill purlis hands no chat: its frontmatter or its files are refused ([`Refusal`]).
    Refused,
}

/// **A skill's compatibility badge on one harness**: how well it fits, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compat {
    pub fit: Fit,
    /// What widens the chat, which purlis leaves out of a skill it hands.
    dropped: Vec<Field>,
    /// What widens the chat, in a harness or repo skill whose harness applies it.
    widens: Vec<Field>,
    /// What the harness has no key for and purlis carries over by an equivalent.
    translated: Vec<Field>,
    /// What the harness does not apply.
    ignored: Vec<Field>,
    /// How many keys it sets that no harness reads.
    unknown: usize,
    /// Why purlis cannot read it, where it cannot.
    refused: Option<Refusal>,
}

/// **The one sentence that says a skill widens the chat**, by what it asks for: the badge of a
/// harness or repo skill and the lint of one say it the same way.
pub fn widening_said(fields: &[Field]) -> String {
    format!(
        "Widens the chat: {} apply while it runs, and purlis cannot leave them out of a skill \
         it does not hand over.",
        said(fields)
    )
}

impl Compat {
    /// **The badge of a skill from `origin` purlis cannot read**, as `why` says: one it hands is
    /// handed to no chat, and a harness or repo skill may widen the chat.
    pub fn refused(why: Refusal, origin: Origin) -> Compat {
        Compat {
            fit: if origin.handed() {
                Fit::Refused
            } else {
                Fit::Widens
            },
            dropped: Vec::new(),
            widens: Vec::new(),
            translated: Vec::new(),
            ignored: Vec::new(),
            unknown: 0,
            refused: Some(why),
        }
    }

    /// Whether it says the skill widens the chat.
    pub fn widens(&self) -> bool {
        self.fit == Fit::Widens
    }

    /// **What the badge says, one line each**: what is left out, what widens, what is carried by
    /// an equivalent, what is not applied, and what nothing reads. One line where it all applies.
    /// It names no key.
    pub fn lines(&self) -> Vec<String> {
        if let Some(why) = &self.refused {
            return vec![match self.fit {
                Fit::Refused => format!("purlis hands it to no chat: {why}."),
                _ => format!("It may widen the chat: purlis cannot read it, as {why}."),
            }];
        }
        let mut lines = Vec::new();
        if self.fit == Fit::Full && self.translated.is_empty() {
            lines.push("Everything it asks for applies.".to_owned());
        }
        if !self.dropped.is_empty() {
            lines.push(format!(
                "Left out, because a skill may only narrow a chat: {}.",
                said(&self.dropped)
            ));
        }
        if !self.widens.is_empty() {
            lines.push(widening_said(&self.widens));
        }
        if !self.translated.is_empty() {
            lines.push(format!(
                "Carried over by purlis's equivalent: {}.",
                said(&self.translated)
            ));
        }
        let (narrowing, rest): (Vec<Field>, Vec<Field>) =
            self.ignored.iter().partition(|field| field.narrows());
        if !narrowing.is_empty() {
            lines.push(format!("Narrowing not applied: {}.", said(&narrowing)));
        }
        if !rest.is_empty() {
            lines.push(format!("Ignored: {}.", said(&rest)));
        }
        if self.unknown > 0 {
            lines.push("Some of what it sets is read by nothing.".to_owned());
        }
        lines
    }
}

/// `fields` as a person reads a list of what they ask for.
fn said(fields: &[Field]) -> String {
    let words: Vec<&str> = fields.iter().map(|field| field.said()).collect();
    match words.as_slice() {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// **The badge of a skill whose `SKILL.md` is `text`, from `origin`, on a harness that reads
/// `honours`**. A skill purlis hands over ([`Origin::handed`]) has what would widen the chat left
/// out and what has an equivalent translated, and is refused where its frontmatter is
/// ([`crate::handed`]). A harness or repo skill is read by its harness where it lies: one that
/// purlis cannot read may widen the chat.
pub fn compat(text: &str, origin: Origin, honours: &Honours) -> Compat {
    let handed = origin.handed();
    let mut out = Compat {
        fit: Fit::Full,
        dropped: Vec::new(),
        widens: Vec::new(),
        translated: Vec::new(),
        ignored: Vec::new(),
        unknown: 0,
        refused: None,
    };
    let read = match crate::handed::parse(text) {
        Ok(read) => read,
        Err(why) => return Compat::refused(why, origin),
    };
    out.unknown = read.unknown().len();
    let manual = read.manual();
    for field in read.fields() {
        if field.for_a_reader() {
            continue;
        }
        // A fork as an agent the harness does not ship may carry its own settings (decision 58).
        if handed && field == Field::Agent && !read.agent_kept(honours) {
            out.dropped.push(field);
            continue;
        }
        let applies = honours.applies(field);
        if field.widens() {
            match (handed, applies) {
                (true, _) => out.dropped.push(field),
                (false, true) => out.widens.push(field),
                (false, false) => out.ignored.push(field),
            }
        } else if field == Field::ManualOnly {
            match (manual, handed, honours.manual) {
                // `disable-model-invocation: false` asks for nothing.
                (false, ..) | (true, _, ManualOnly::Read) => {}
                (true, true, ManualOnly::Listing | ManualOnly::WithoutDescription) => {
                    out.translated.push(field);
                }
                (true, false, _) => out.ignored.push(field),
            }
        } else if !applies {
            out.ignored.push(field);
        }
    }
    out.fit = if !out.widens.is_empty() {
        Fit::Widens
    } else if out.dropped.is_empty() && out.ignored.is_empty() && out.unknown == 0 {
        Fit::Full
    } else {
        Fit::Partial
    };
    out
}

/// **The `SKILL.md` purlis hands a harness that reads `honours`, for a skill whose own is
/// `text`** ([`crate::handed`]): a frontmatter purlis writes of the keys it keeps, never one
/// that widens the chat, and a manual-only skill without its description where that is how the
/// harness keeps one out; then the body as it is. Refused, handing nothing, where the
/// frontmatter is.
pub fn handed_text(text: &str, honours: &Honours) -> Result<String, Refusal> {
    crate::handed::handed(text, honours)
}

/// [`handed_text`] for a skill the set holds as `name`, which its handed frontmatter says: a
/// built-in skill on a route that names it its own ([`crate::Route::built_in_name`]) is handed
/// as `purlis-<name>`, its folder's name in the set's folder, as the skill format asks.
pub fn handed_text_as(text: &str, honours: &Honours, name: &str) -> Result<String, Refusal> {
    crate::handed::handed_as(text, honours, Some(name))
}

#[cfg(test)]
mod tests;
