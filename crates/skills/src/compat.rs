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

use crate::Declared;

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
    pub const ALL: [Field; 20] = [
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
    pub fn said(self) -> &'static str {
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
    pub fn narrows(self) -> bool {
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
    /// It has no way: a manual-only skill is listed like any other.
    Not,
}

/// **What a harness reads of a skill's frontmatter**: all this module needs to know of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Honours {
    /// The fields it applies, beyond the ones purlis reads itself on every harness (the name,
    /// the description and the arguments).
    pub reads: &'static [Field],
    /// How it keeps a manual-only skill out of its model's choice.
    pub manual: ManualOnly,
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
    /// A harness or repo skill whose harness applies what widens the chat.
    Widens,
}

/// **A skill's compatibility badge on one harness**: how well it fits, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compat {
    pub fit: Fit,
    /// What widens the chat, which purlis leaves out of a skill it hands.
    pub dropped: Vec<Field>,
    /// What widens the chat, in a harness or repo skill whose harness applies it.
    pub widens: Vec<Field>,
    /// What the harness has no key for and purlis carries over by an equivalent.
    pub translated: Vec<Field>,
    /// What the harness does not apply.
    pub ignored: Vec<Field>,
    /// The keys no harness reads, as written.
    pub unknown: Vec<String>,
}

impl Compat {
    /// **What the badge says, one line each**: what is left out, what widens, what is carried by
    /// an equivalent, what is not applied, and what nothing reads. One line where it all applies.
    pub fn lines(&self) -> Vec<String> {
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
            lines.push(format!(
                "Widens the chat: {} apply, and purlis cannot leave them out of a skill it does \
                 not hand over.",
                said(&self.widens)
            ));
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
        if !self.unknown.is_empty() {
            lines.push(format!("Read by nothing: {}.", self.unknown.join(", ")));
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

/// The fields `text`'s frontmatter sets, in the order written, and the keys no harness reads.
fn fields_of(text: &str) -> (Vec<Field>, Vec<String>) {
    let mut fields = Vec::new();
    let mut unknown = Vec::new();
    for (key, _) in crate::read::entries(text) {
        match Field::of(&key) {
            Some(field) if !fields.contains(&field) => fields.push(field),
            Some(_) => {}
            None if !unknown.contains(&key) => unknown.push(key),
            None => {}
        }
    }
    (fields, unknown)
}

/// **The badge of a skill whose `SKILL.md` is `text`, on a harness that reads `honours`**.
/// `handed` says whether purlis hands it over (a built-in skill or the plane's), so leaves out
/// what widens the chat and translates what has an equivalent, or the harness reads it where it
/// lies (a harness or repo skill).
pub fn compat(text: &str, handed: bool, honours: &Honours) -> Compat {
    let (fields, unknown) = fields_of(text);
    let manual = crate::declared::declared_in(text).manual;
    let mut out = Compat {
        fit: Fit::Full,
        dropped: Vec::new(),
        widens: Vec::new(),
        translated: Vec::new(),
        ignored: Vec::new(),
        unknown,
    };
    for field in fields {
        if field.for_a_reader() {
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
                (true, ..) => out.ignored.push(field),
            }
        } else if !applies {
            out.ignored.push(field);
        }
    }
    out.fit = if !out.widens.is_empty() {
        Fit::Widens
    } else if out.dropped.is_empty() && out.ignored.is_empty() && out.unknown.is_empty() {
        Fit::Full
    } else {
        Fit::Partial
    };
    out
}

/// **Whether a harness that reads `honours` keeps a manual-only skill out of its model's
/// listing**, by its own key or purlis's equivalent: then such a skill costs nothing in its
/// budget ([`crate::Budget::manual_free`]).
pub fn manual_kept_out(honours: &Honours) -> bool {
    honours.manual != ManualOnly::Not
}

/// Whether `declared` is a manual-only skill a harness that reads `honours` is handed without its
/// description ([`ManualOnly::WithoutDescription`]).
fn without_description(declared: &Declared, honours: &Honours) -> bool {
    declared.manual && honours.manual == ManualOnly::WithoutDescription
}

/// **The `SKILL.md` purlis hands a harness that reads `honours`, for a skill whose own is
/// `text`**, or `None` where it is handed as it is: every byte as written, but the entries that
/// widen the chat (`allowed-tools`, `hooks`) left out, and, for a manual-only skill on a harness
/// that keeps one out by its description alone, the description too.
pub fn handed_text(text: &str, honours: &Honours) -> Option<String> {
    let drop_description = without_description(&crate::declared::declared_in(text), honours);
    let left_out: Vec<(usize, usize)> = crate::read::entries(text)
        .into_iter()
        .filter(|(key, _)| {
            Field::of(key).is_some_and(|field| {
                field.widens() || (drop_description && field == Field::Description)
            })
        })
        .map(|(_, span)| span)
        .collect();
    if left_out.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    for (start, end) in left_out {
        out.push_str(&text[from..start]);
        from = end;
    }
    out.push_str(&text[from..]);
    Some(out)
}

#[cfg(test)]
mod tests;
