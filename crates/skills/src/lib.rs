// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a chat's skill set is, and why (SK-1, #2764; spec #2762).
//!
//! This crate is the one place purlis works skills out. Its main question is [`skill_set`]:
//! given the plane, the machine and a chat (its workspace, persona and harness), which skills
//! does the chat get, by which [`Route`] and from which [`Origin`], and which are left out, each
//! with a one-line [`Reason`]. A harness adapter asks it and turns the answer into argv, env and
//! config for its harness. It never reads a skill folder itself (ADR 0063).
//!
//! **The levels** (ADR 0099). A built-in skill ships inside purlis: one directory, `skills/`
//! inside the bundled plugin, with a folder per skill holding its `SKILL.md`. A project skill is
//! a folder in the plane's `skills/` (SK-2, #2765), and reaches a workspace's chats only through
//! its [`Grant`], which a workspace can turn off ([`Chat::turned_off`]) and never widen. A workspace
//! skill is a folder in `workspaces/<ws>/skills/`, for that workspace's chats alone (SK-4,
//! #2767). Each plane level also has its **local** skills, in `skills.local/` beside its
//! `skills/`, read only while git keeps them on this machine ([`Machine::kept_local`]); [`share`]
//! moves one to the shared folder. A persona skill is a folder in `personas/<p>/skills/`, for
//! that persona's chats in any workspace (SK-5, #2768), and the persona's `skills:` list names
//! more ([`Reference`]): a built-in skill, a harness skill, or a plane skill, which then reaches
//! the persona's chats whatever the workspace was granted. When two copies have one name the
//! narrowest wins: persona over workspace over project, local over shared, and a built-in
//! skill's name is never taken. Every skill is handed to a harness from where it lies, and none
//! gets a copy. A harness skill is the harness's to load: the set only says when one the persona
//! names is not on this machine ([`Machine::harness`]). purlis never writes one.
//!
//! **Harness and repo skills** (SK-6, #2769): what a harness finds itself, in the person's own
//! folders and plugins or in a clone a chat works in, is no level of the plane's. purlis-core
//! finds them ([`OutsideSkill`]) and [`SkillSet::with_outside`] lists them beside the plane's, read only,
//! each with whose it is, whether the project turned it off, and any plane skill it clashes
//! with. purlis hands none of them: the harness reads them where they lie.
//!
//! **Harness-agnostic.** The crate knows no harness by name. A chat's harness is described by
//! what it can do with skills ([`HarnessSkills`]), which purlis-core's harness declaration
//! says, so a new harness needs no change here.

use std::path::{Path, PathBuf};

mod beside;
pub mod budget;
pub mod change;
pub mod compat;
mod declared;
pub mod grade;
mod grant;
pub mod handed;
mod held;
pub mod lifecycle;
pub mod lint;
mod load;
pub mod moving;
mod read;
mod reference;
pub mod review;
pub mod router;
pub mod runs;
mod share;
mod stamp;
pub mod tree;
pub mod uses;

pub use budget::{Budget, Over, Spent};
pub use compat::{Compat, Field, Fit, Honours, ManualOnly, compat, handed_text, widening_said};
pub use declared::{Argument, Declared, Given, declared};
pub use grant::{Grant, grant_file, grant_of, set_grant};
pub use handed::{Frontmatter, MOST_BYTES, Refusal, read_skill_md, read_skill_md_of};
pub use lifecycle::{About, Deprecated, Lifecycle, about_of, set_lifecycle, set_owners};
pub use lint::{BuiltInLints, Finding, Lint, Severity, SkillFiles, SkillLint};
pub use load::{Entry, Loaded, Narrowing, POINTER, SkillFolder, by_name, find, load};
pub use moving::{Unmoved, move_between, move_skill};
pub use read::{
    DIR_IN_BUNDLE, FILE, frontmatter, in_bundle, listing, parsed, read, scan, scan_dir,
};
use reference::Seen;
pub use reference::{BUILT_IN_PREFIXES, OnMachine, Reference};
pub use router::{Hidden, HiddenWhy, INDEX, Index, Router, Routing, StillOver};
pub use share::{Shared, named, named_at, share};
pub use stamp::{Reload, Stamp};

/// A skill, as a chat is told about it: a folder in the open SKILL.md format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// The frontmatter's `name`.
    pub name: String,
    /// The frontmatter's `description`: what the skill does and when to use it.
    pub description: String,
    /// Its `SKILL.md`, which the model reads when the skill applies.
    pub path: PathBuf,
}

/// How a harness's chat comes to know its skills (ADR 0063).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// The harness loads the bundled plugin, `skills/` included, for the chat alone.
    Plugin,
    /// The harness is told the skills directory for the chat alone, and discovers the skills in
    /// it as its own.
    Config,
    /// The harness has no way to take a skills directory for one chat, so the chat's
    /// `SessionStart` briefing lists each skill with the path to its `SKILL.md`.
    Briefing,
    /// The harness has MCP and no skill format: the chat finds its skills with purlis's
    /// `skill_find` and loads one with `skill_load`, and is told so in one line, [`POINTER`]
    /// (ADR 0063 as amended 2026-10-20, the fallback route).
    Mcp,
    /// The harness finds it itself, where it lies: a harness or repo skill ([`OutsideSkill`]). purlis
    /// hands it nothing, and can only ask the harness to leave it out.
    Own,
}

impl Route {
    /// **Whether a harness on this route that leaves one of its own skills out by its name still
    /// gets the copy purlis hands of that name** (SK-15): on [`Route::Plugin`] purlis's copies
    /// come under their plugin's namespace, and on [`Route::Briefing`] purlis names them in the
    /// briefing, apart from what the harness finds itself. On [`Route::Config`] purlis's folder is
    /// one the harness discovers as its own, so a switch by name would reach purlis's copy too.
    pub fn off_by_name_spares_handed(self) -> bool {
        matches!(self, Self::Plugin | Self::Briefing)
    }
}

/// Where a skill in a chat's set comes from: its level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    /// It ships inside purlis, in the bundled plugin's `skills/`. It cannot be turned off.
    BuiltIn,
    /// The project's: a folder in the plane's `skills/` (ADR 0099). It reaches chats at the
    /// plane root, and a workspace's chats only through its grant.
    Project,
    /// One workspace's: a folder in `workspaces/<ws>/skills/`, for that workspace's chats alone.
    Workspace,
    /// One persona's: a folder in `personas/<p>/skills/`, for that persona's chats in any
    /// workspace (SK-5, #2768).
    Persona,
    /// The harness's own: in the person's harness folders or a harness plugin. The harness loads
    /// it, never purlis: the set holds one the persona names that this machine does not have as
    /// left out, and one purlis-core found as an [`OutsideSkill`] (SK-6, ADR 0099 §1).
    Harness,
    /// A repo's own: a skill a clone holds where its harness looks, such as its
    /// `.claude/skills`. Read only, and owned by the repo (SK-6).
    Repo,
}

impl Origin {
    /// The level a person is shown: `built-in`, `project`, `workspace`, `harness`, `repo`.
    pub fn level(self) -> &'static str {
        match self {
            Self::BuiltIn => "built-in",
            Self::Project => "project",
            Self::Workspace => "workspace",
            Self::Persona => "persona",
            Self::Harness => "installed",
            Self::Repo => "repo",
        }
    }

    /// **Whether purlis hands a skill of this origin over**, so writes the `SKILL.md` a harness
    /// reads (SK-8): a built-in skill or the plane's. A harness or repo skill is read by its
    /// harness where it lies.
    pub fn handed(self) -> bool {
        !matches!(self, Self::Harness | Self::Repo)
    }

    /// Whether a skill of this origin is one of the plane's own levels, which purlis hands a
    /// chat by its route: a project, a workspace or a persona skill.
    pub fn of_the_plane(self) -> bool {
        matches!(self, Self::Project | Self::Workspace | Self::Persona)
    }
}

/// **A skill's name without the namespace a harness started it by**: `purlis-project:deploy` is
/// `deploy`, and a name with none is itself. The one strip of a `<plugin>:<skill>` name.
pub fn leaf_name(name: &str) -> &str {
    name.rsplit_once(':').map_or(name, |(_, leaf)| leaf)
}

/// The level a person is shown for a skill of `origin`, local or shared: `project`,
/// `local project`, `workspace`, `local workspace`.
pub fn level_shown(origin: Origin, local: bool) -> String {
    if local {
        format!("local {}", origin.level())
    } else {
        origin.level().to_owned()
    }
}

/// What a chat's harness can do with skills: all this crate needs to know about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarnessSkills {
    /// How its chats are handed their skills.
    pub route: Route,
}

/// The chat a skill set is worked out for.
#[derive(Debug, Clone, Copy)]
pub struct Chat<'a> {
    /// The workspace it works in, where purlis knows it; `None` for a chat at the plane root.
    pub workspace: Option<&'a str>,
    /// The persona it runs as, or `None` for a chat on no persona.
    pub persona: Option<&'a str>,
    /// What its persona's `skills:` list names, in order (SK-5): each reaches the chat whatever
    /// its workspace was granted. Empty for a chat on no persona.
    pub names: &'a [Reference],
    /// The names of the project skills its workspace turned off (`settings.skills.off`, which
    /// purlis-core's Workspace settings layer reads). It only narrows: a name whose skill is not
    /// granted to the workspace stays not granted. Empty for a chat at the project's root.
    pub turned_off: &'a [String],
    /// Its harness, by what it can do with skills.
    pub harness: HarnessSkills,
}

/// **Whether git keeps a local skill's folder on this machine** (the live/local rule, ADR 0048):
/// `Ok` where git ignores its `SKILL.md` or the plane is no git repository, otherwise what git
/// said, as the end of a sentence a person reads ("git would commit it"). purlis-core asks git;
/// this crate only asks purlis-core.
pub type KeptLocal<'a> = dyn Fn(&Path) -> Result<(), String> + Sync + 'a;

/// **Why the skills of a workspace or persona folder reach no chat now** (SK-9), as the end of
/// a sentence, or `None` where they do. purlis-core asks its records of the running sandboxed
/// chats; this crate only asks purlis-core.
pub type MadeSince<'a> = dyn Fn(&Path) -> Option<String> + Sync + 'a;

/// What this machine holds that a chat's skills can come from.
#[derive(Clone, Copy, Default)]
pub struct Machine<'a> {
    /// The directory the built-in skills sit in, a folder per skill: the bundled plugin's
    /// `skills/` ([`in_bundle`]), where the app found it.
    pub built_in: Option<&'a Path>,
    /// How to ask git whether a local skill's folder stays on this machine. `None` reads no
    /// local skill: each is left out, saying purlis could not ask.
    pub kept_local: Option<&'a KeptLocal<'a>>,
    /// The harness skills the chat's harness has on this machine, which purlis-core's harness
    /// declaration reads. `None` where nobody looked: then a harness skill the persona names is
    /// taken as there, and never said to be missing.
    pub harness: Option<&'a [OnMachine]>,
    /// How to ask whether a workspace's or persona's folder was made while a chat that could
    /// have made it ran, by the folder. `None` asks nothing: every level is read.
    pub made_since: Option<&'a MadeSince<'a>>,
}

impl std::fmt::Debug for Machine<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Machine")
            .field("built_in", &self.built_in)
            .field("kept_local", &self.kept_local.is_some())
            .field("harness", &self.harness)
            .field("made_since", &self.made_since.is_some())
            .finish()
    }
}

/// One skill a chat gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effective {
    pub skill: Skill,
    /// The folder its level is reached from without following a link (SK-36): the plane's root,
    /// or the bundle's plugin folder for a built-in skill. A load opens every folder below it
    /// through the handle of the one above ([`load`]).
    pub root: PathBuf,
    /// How the chat's harness is handed it.
    pub route: Route,
    pub origin: Origin,
    /// Whether it is a local skill: kept on this machine, in its level's `skills.local/`.
    pub local: bool,
    /// Where the skill is deprecated (SK-19, decision 62): it still works, and a router lists it
    /// after the rest with its replacement (ADR 0102 §2). `None` for a skill in use.
    pub deprecated: Option<Deprecated>,
}

/// One skill a chat does not get, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftOut {
    /// Its name, or its folder's name where it is left out for naming none.
    pub name: String,
    pub origin: Origin,
    /// Whether it is a local skill.
    pub local: bool,
    /// Its folder.
    pub folder: PathBuf,
    pub reason: Reason,
}

/// Why a skill is left out of a chat's set. Its [`std::fmt::Display`] is the one line a person
/// is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reason {
    /// The folder is not a whole skill, so no harness would load it.
    NotASkill(Missing),
    /// A project skill, for a chat in a workspace its grant does not reach (ADR 0099 §2).
    NotGranted,
    /// A project skill whose grant cannot be read, so it is read as `root`, for a chat in a
    /// workspace: what is wrong with it.
    GrantUnread(String),
    /// A project skill granted to the chat's workspace, which turned it off (ADR 0099 §2).
    TurnedOff,
    /// A skill of the same name, from the level named, is in the set instead (ADR 0099 §3).
    NameTaken { by: Origin, local: bool },
    /// Another folder at the same level names the same skill, and is in the set instead.
    Twin { origin: Origin, local: bool },
    /// A local skill git does not keep on this machine, or that purlis could not ask git about:
    /// the sentence that says which, and how to fix it.
    NotKeptLocal(String),
    /// A skill the chat's persona names that this machine does not have for the chat's harness
    /// (SK-5): the chat starts without it, and its tab says so.
    NotOnThisMachine,
    /// A built-in skill the chat's persona names, that no built-in skill is: a typo, or a name
    /// from another version of purlis.
    NoSuchBuiltIn,
    /// A harness or repo skill the project turned off for this harness's chats, which purlis
    /// asks the harness to leave out.
    OffForTheHarness,
    /// A harness or repo skill named as one of purlis's built-in skills, which never loses to
    /// any other copy (SK-6): the chat is handed purlis's.
    HiddenByBuiltIn,
    /// A plane skill a repo copy could hide, where purlis could not list a skills folder of the
    /// chat's clone whole ([`Unread::Crowded`], SK-44): left out, since purlis cannot tell whether
    /// the repo has a skill of its name.
    RepoUnread {
        /// The clone's name.
        repo: String,
    },
    /// A plane skill its project archived (SK-19, decision 62): it never loads, takes no name
    /// from another copy, and is restored by making it active again.
    Archived,
    /// A workspace or persona skill whose level's folder was made while a chat that could have
    /// made it was running (SK-9): the sentence that says which, and when it reaches chats.
    MadeWhileAChatRan(String),
    /// A plane skill named as purlis's index skill ([`router::INDEX`], ADR 0102 §2), which a
    /// router shows in its place: it never loads until it is renamed.
    IndexName,
    /// A harness or repo skill named as purlis's index skill (SK-15): the name is purlis's in
    /// every chat, so the index a router hands is the one copy of it.
    HiddenByIndex,
}

impl Reason {
    /// **Whether a repo of the chat's clone is why** (SK-53, #3213): a narrower repo copy took
    /// the name ([`Self::NameTaken`] by [`Origin::Repo`], decision 51), or purlis could not read
    /// a skills folder of the clone whole ([`Self::RepoUnread`]). A plane skill left out so is
    /// one only a chat in that clone loses, and its tab says so (decision 41).
    pub fn of_a_repo(&self) -> bool {
        matches!(
            self,
            Self::RepoUnread { .. }
                | Self::NameTaken {
                    by: Origin::Repo,
                    ..
                }
        )
    }
}

/// What a folder lacks to be a whole skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// It holds no `SKILL.md`, or one that cannot be read as text.
    File,
    /// Its `SKILL.md`'s frontmatter names no `name`.
    Name,
    /// Its `SKILL.md`'s frontmatter names no `description`.
    Description,
    /// It, its `SKILL.md`, or the level's folder it is in is a link, which purlis never reads
    /// through (SK-36).
    Link,
    /// Its `SKILL.md` is larger than purlis reads ([`MOST_BYTES`]).
    TooLarge,
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotASkill(Missing::File) => {
                f.write_str("not a skill: its folder has no SKILL.md")
            }
            Self::NotASkill(Missing::Name) => {
                f.write_str("not a skill: its SKILL.md names no `name`")
            }
            Self::NotASkill(Missing::Description) => {
                f.write_str("not a skill: its SKILL.md names no `description`")
            }
            Self::NotASkill(Missing::Link) => f.write_str("a link, so purlis doesn't read it"),
            Self::NotASkill(Missing::TooLarge) => write!(
                f,
                "its SKILL.md is larger than purlis reads ({} KiB)",
                MOST_BYTES / 1024
            ),
            Self::NotGranted => f.write_str(
                "not granted to this workspace: a project skill is for chats at the project's root \
                 until the project grants it",
            ),
            Self::GrantUnread(why) => write!(
                f,
                "not granted to this workspace: its grant cannot be read ({why}), so it is for \
                 chats at the project's root only"
            ),
            Self::TurnedOff => f.write_str("turned off in this workspace"),
            Self::NameTaken {
                by: Origin::BuiltIn,
                ..
            } => f.write_str(
                "a built-in skill has this name, and a built-in skill is never replaced",
            ),
            Self::NameTaken { by, local } => write!(
                f,
                "hidden by the {} skill of this name",
                level_shown(*by, *local)
            ),
            Self::Twin { origin, local } => write!(
                f,
                "hidden by another {} skill of this name",
                level_shown(*origin, *local)
            ),
            Self::NotKeptLocal(why) => f.write_str(why),
            Self::NotOnThisMachine => {
                f.write_str("its persona names it, and it is not installed on this machine")
            }
            Self::NoSuchBuiltIn => {
                f.write_str("its persona names it, and purlis has no built-in skill of this name")
            }
            Self::OffForTheHarness => f.write_str("turned off for this project's chats"),
            Self::HiddenByBuiltIn => f.write_str("hidden by purlis's built-in skill of this name"),
            Self::RepoUnread { repo } => write!(
                f,
                "purlis could not read the skills folder of {repo} whole, so a copy of this \
                 skill may be there, and a chat in that repo would get that one"
            ),
            Self::Archived => {
                f.write_str("archived: it never loads until it is restored from its tab")
            }
            Self::MadeWhileAChatRan(why) => f.write_str(why),
            Self::IndexName => f.write_str(
                "purlis's index skill has this name, so this skill never loads: rename it",
            ),
            Self::HiddenByIndex => f.write_str("hidden by purlis's index skill of this name"),
        }
    }
}

/// A chat's skill set: what it gets, and what it does not and why.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillSet {
    /// Every skill the chat gets, by name.
    pub effective: Vec<Effective>,
    /// Every skill left out, by name, and the copies of one name from the widest level to the
    /// narrowest.
    pub left_out: Vec<LeftOut>,
    /// Every harness skill the chat's persona names that its harness loads by itself, in the
    /// order its `skills:` list names them (SK-5). purlis never hands one over.
    pub from_the_harness: Vec<FromHarness>,
    /// Every skill from outside the plane that has the name of a skill purlis hands the chat, by name
    /// ([`SkillSet::with_outside`]), and what came of it.
    pub clashes: Vec<Clash>,
    /// What a router made of the set (SK-15, ADR 0102), or `None` where none ran: which skills
    /// its listing hides, each still in [`Self::effective`], and the chat's pins.
    pub routing: Option<Routing>,
}

/// A harness skill a chat's persona names, which the chat's harness loads by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FromHarness {
    pub reference: Reference,
    /// Whether purlis saw it on this machine. `false` where it could not look: a plugin whose
    /// skills it cannot list, or a set worked out without looking at the harness.
    pub checked: bool,
}

/// **A skill from outside the plane** (SK-6, #2769): one a harness finds itself, in the
/// person's own folders or its plugins (a harness skill), or in a clone a chat works in (a repo
/// skill). purlis-core finds it; purlis lists it and never writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutsideSkill {
    pub skill: Skill,
    /// The folder it is reached from (SK-36): a repo skill's clone, whose every folder below is
    /// opened without following a link; the person's own skill's folder itself, read as it is.
    pub root: PathBuf,
    /// [`Origin::Harness`] or [`Origin::Repo`].
    pub origin: Origin,
    /// The name the harness knows it by, which a project turns it off by: its `name`, or
    /// `<plugin>:<name>` for a skill a plugin brings.
    pub id: String,
    pub owner: Owner,
    /// How the harness can leave it out of a chat.
    pub switch: Switch,
    /// Whether the project turned it off for this harness's chats.
    pub off: bool,
    /// Why purlis did not read it, where it did not (SK-44): listed for a person to see, its
    /// `description` the sentence that says so, and never linted or badged.
    pub unread: Option<Unread>,
}

/// **Why purlis did not read a skill from outside the plane** (SK-44, #3133). The harness may
/// still load what is there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unread {
    /// Its folder is a link purlis does not follow. Named by its folder's name, which the
    /// harness loads it by: it takes that name as any repo skill does.
    Link,
    /// Its `SKILL.md` is one purlis does not read: a hard link in a clone, or larger than
    /// [`MOST_BYTES`]. Named by its folder's name, as [`Self::Link`] is.
    Refused,
    /// No one skill: a folder purlis could not list whole (more names than it reads, or a read
    /// that failed partway), named by its own name. A skill
    /// of any name may be in it, so a repo's leaves out every plane skill its harness would let
    /// a repo copy hide ([`Reason::RepoUnread`]), and it takes no name itself.
    Crowded,
}

/// Whose a skill from outside the plane is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    /// The person's own, in their folders for the harness.
    You,
    /// A plugin's, by the name the harness gives it.
    Plugin(String),
    /// A repo's, by the name of its clone.
    Repo(String),
}

/// How a harness can leave one of its own skills out of a chat, as its adapter says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Switch {
    /// On its own, for one chat.
    Alone,
    /// Not on its own, and the sentence that says why and what can, such as its plugin: the
    /// studio says it plainly rather than draw a switch that does nothing.
    Not(String),
}

/// `you`, the plugin's name, or `repo <clone>`: whose a skill is, as a person reads it.
impl std::fmt::Display for Owner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::You => f.write_str("you"),
            Self::Plugin(plugin) => f.write_str(plugin),
            Self::Repo(clone) => write!(f, "repo {clone}"),
        }
    }
}

/// [`Owner`] and [`Switch`] on the wire: tagged by `kind`, each field named, as the window's
/// bindings spell them. serde cannot tag a variant that holds a bare string, so each is
/// serialized through its named twin here.
mod wire {
    #[derive(serde::Serialize)]
    #[serde(tag = "kind", rename_all = "kebab-case")]
    pub(super) enum Owner<'a> {
        You,
        Plugin { name: &'a str },
        Repo { name: &'a str },
    }

    #[derive(serde::Serialize)]
    #[serde(tag = "kind", rename_all = "kebab-case")]
    pub(super) enum Switch<'a> {
        Alone,
        Not { why: &'a str },
    }
}

/// `{"kind": "you"}`, `{"kind": "plugin", "name": …}`, `{"kind": "repo", "name": …}`.
impl serde::Serialize for Owner {
    fn serialize<S: serde::Serializer>(&self, to: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::You => wire::Owner::You,
            Self::Plugin(name) => wire::Owner::Plugin { name },
            Self::Repo(name) => wire::Owner::Repo { name },
        }
        .serialize(to)
    }
}

/// `{"kind": "alone"}` or `{"kind": "not", "why": …}`.
impl serde::Serialize for Switch {
    fn serialize<S: serde::Serializer>(&self, to: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Alone => wire::Switch::Alone,
            Self::Not(why) => wire::Switch::Not { why },
        }
        .serialize(to)
    }
}

/// Where a copy of a skill sits in the plane: its level, and whether it is a local skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct At {
    pub origin: Origin,
    pub local: bool,
}

/// **A skill from outside the plane with the name of one purlis hands the chat** (ADR 0099 §3,
/// decision 51), and what came of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    pub name: String,
    /// The plane's copy.
    pub plane: At,
    /// The other copy: [`Origin::Harness`] or [`Origin::Repo`].
    pub outside: Origin,
    pub owner: Owner,
    pub outcome: Outcome,
}

/// What came of a [`Clash`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Both load: the harness reads its own whatever purlis says, and purlis hands its copy too
    /// (Claude Code, whose plane skills come as their own namespaced plugin; or a harness skill,
    /// which is wider than the plane's copy).
    BothLoad,
    /// The repo's copy is the narrower one and purlis left its own out, so the chat gets one
    /// copy of the name (decision 51): where purlis builds what the harness is handed.
    RepoWins,
}

impl SkillSet {
    /// The one directory the effective built-in skills sit in, a folder per skill, or `None` for
    /// a set with none: the bundle's `skills/`, which every route hands over as it is.
    pub fn built_in_dir(&self) -> Option<&Path> {
        let mut dirs = self
            .effective
            .iter()
            .filter(|effective| effective.origin == Origin::BuiltIn)
            .filter_map(|effective| effective.skill.path.parent().and_then(Path::parent));
        let dir = dirs.next()?;
        debug_assert!(
            dirs.all(|other| other == dir),
            "every built-in skill sits in one directory"
        );
        Some(dir)
    }

    /// **The plane's skills a repo of the chat's clone leaves out** ([`Reason::of_a_repo`]),
    /// in the order the set left them out: what the chat's tab and the studio say it starts
    /// without because of that repo (SK-53).
    pub fn left_out_by_a_repo(&self) -> impl Iterator<Item = &LeftOut> {
        self.left_out
            .iter()
            .filter(|left| left.origin.of_the_plane() && left.reason.of_a_repo())
    }

    /// The effective skills that come from the plane, by name: what a route hands over beside
    /// the built-in directory (ADR 0063 as amended 2026-10-20). Each has one name in the set. A
    /// harness or repo skill is never among them: the harness reads those itself. Nor is one a
    /// router hid ([`Self::hidden_by_router`]): the route hands the index skill in its place.
    pub fn from_the_plane(&self) -> impl Iterator<Item = &Effective> {
        self.listed()
            .filter(|effective| effective.origin.of_the_plane())
    }

    /// **The set, with the skills from outside the plane beside what purlis hands** (SK-6,
    /// #2769): each one on, by [`Route::Own`], unless the project turned it off, when it is left
    /// out saying so. Where one has the name of a skill purlis hands the chat, the narrowest copy
    /// wins wherever purlis chooses (decision 51): a repo skill is narrower than the plane's, and
    /// on a route where purlis builds what the harness is handed ([`Route::Config`],
    /// [`Route::Briefing`]) the plane's copy is left out, hidden by the repo's. Otherwise both
    /// load: a harness skill is the person's own and wider, and on [`Route::Plugin`] the plane's
    /// copies come as their own namespaced plugin. Either way the pair is a [`Clash`]. Two
    /// outside skills of one id are both listed: the harness has them both.
    #[must_use]
    pub fn with_outside(
        mut self,
        route: Route,
        outside: impl IntoIterator<Item = OutsideSkill>,
    ) -> Self {
        for one in outside {
            if one.unread == Some(Unread::Crowded) {
                // Fail closed: any plane skill may have a repo copy purlis did not read.
                let repo = match &one.owner {
                    Owner::Repo(repo) => Some(repo.clone()),
                    Owner::You | Owner::Plugin(_) => None,
                };
                if let Some(repo) = repo.filter(|_| !one.off && route != Route::Plugin) {
                    let (hidden, kept) = std::mem::take(&mut self.effective)
                        .into_iter()
                        .partition(|e| e.route != Route::Own && e.origin != Origin::BuiltIn);
                    self.effective = kept;
                    self.left_out
                        .extend(hidden.into_iter().map(|hidden: Effective| {
                            LeftOut {
                                name: hidden.skill.name,
                                origin: hidden.origin,
                                local: hidden.local,
                                folder: hidden
                                    .skill
                                    .path
                                    .parent()
                                    .map(Path::to_path_buf)
                                    .unwrap_or_default(),
                                reason: Reason::RepoUnread { repo: repo.clone() },
                            }
                        }));
                }
                continue;
            }
            let handed = (!one.off)
                .then(|| {
                    self.effective
                        .iter()
                        .position(|e| e.route != Route::Own && e.skill.name == one.id)
                })
                .flatten();
            let folder = one
                .skill
                .path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default();
            // The index skill's name is purlis's on every route, whether or not a router hands
            // one to this chat (SK-15): where the harness can leave its own copy out by that name
            // and keep purlis's, it is asked to ([`Route::off_by_name_spares_handed`]); elsewhere
            // it loads, and its studio row says so.
            if one.id == router::INDEX && !one.off && route.off_by_name_spares_handed() {
                self.left_out.push(LeftOut {
                    name: one.id,
                    origin: one.origin,
                    local: false,
                    folder,
                    reason: Reason::HiddenByIndex,
                });
                continue;
            }
            // A built-in skill never loses to any other copy: purlis's is the one handed, and
            // the other is listed as hidden by it, whatever the route.
            if handed.is_some_and(|at| self.effective[at].origin == Origin::BuiltIn) {
                self.left_out.push(LeftOut {
                    name: one.id,
                    origin: one.origin,
                    local: false,
                    folder,
                    reason: Reason::HiddenByBuiltIn,
                });
                continue;
            }
            if let Some(at) = handed {
                let plane = &self.effective[at];
                let outcome = if one.origin == Origin::Repo && route != Route::Plugin {
                    Outcome::RepoWins
                } else {
                    Outcome::BothLoad
                };
                self.clashes.push(Clash {
                    name: one.id.clone(),
                    plane: At {
                        origin: plane.origin,
                        local: plane.local,
                    },
                    outside: one.origin,
                    owner: one.owner.clone(),
                    outcome,
                });
                if outcome == Outcome::RepoWins {
                    let hidden = self.effective.remove(at);
                    self.left_out.push(LeftOut {
                        name: hidden.skill.name,
                        origin: hidden.origin,
                        local: hidden.local,
                        folder: hidden
                            .skill
                            .path
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_default(),
                        reason: Reason::NameTaken {
                            by: Origin::Repo,
                            local: false,
                        },
                    });
                }
            }
            if one.off {
                self.left_out.push(LeftOut {
                    name: one.id,
                    origin: one.origin,
                    local: false,
                    folder,
                    reason: Reason::OffForTheHarness,
                });
            } else {
                self.effective.push(Effective {
                    root: one.root,
                    skill: Skill {
                        name: one.id,
                        ..one.skill
                    },
                    route: Route::Own,
                    origin: one.origin,
                    local: false,
                    deprecated: None,
                });
            }
        }
        self.effective
            .sort_by(|a, b| a.skill.name.cmp(&b.skill.name));
        widest_first(&mut self.left_out);
        self.clashes.sort_by(|a, b| a.name.cmp(&b.name));
        self
    }
}

/// The folder, at a plane level, its shared skills sit in, a folder per skill (ADR 0099): the
/// plane root's `skills/`, and each workspace's.
pub const PROJECT_DIR: &str = "skills";

/// The folder, beside a level's [`PROJECT_DIR`], its local skills sit in (ADR 0099 §1).
pub const LOCAL_DIR: &str = "skills.local";

/// **Who a plane level's skills belong to** (ADR 0099 §1): the project, one workspace, or one
/// persona. With shared or local, it is the whole of a level; [`level_rel`] is where its folder
/// is.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Whose {
    /// The project's: the plane root's `skills/`.
    Project,
    /// One workspace's, by its folder's name.
    Workspace(String),
    /// One persona's, by its name (SK-5, #2768; SK-32, #3003).
    Persona(String),
}

/// The folder of the persona `persona`'s skills in the plane at `plane`: its `skills/`, or its
/// `skills.local/` where `local` (SK-5, ADR 0099 §1). `None` for a name that is not one plain
/// name, which is no folder of the plane's.
pub fn persona_dir(plane: &Path, persona: &str, local: bool) -> Option<PathBuf> {
    level_dir(plane, &Whose::Persona(persona.to_owned()), local)
}

/// The folder of the plane level `at`'s skills: its `skills.local/` where `local`. `None` for a
/// workspace or persona name that is not one plain name, which is no folder of the plane's.
pub fn level_dir(plane: &Path, at: &Whose, local: bool) -> Option<PathBuf> {
    level_rel(at, local).map(|rel| plane.join(rel))
}

/// The folder the plane's workspaces sit in.
const WORKSPACES_DIR: &str = "workspaces";

/// The folder the plane's personas sit in.
const PERSONAS_DIR: &str = "personas";

/// **The one rule for where a plane level's skills are**, relative to the plane with `/` between
/// the parts: `skills`, `skills.local`, `workspaces/<ws>/skills`, `workspaces/<ws>/skills.local`,
/// `personas/<p>/skills`, `personas/<p>/skills.local`. `None` for a workspace or persona name
/// that is not one plain name. [`level_dir`] is it under a plane, and [`level_of_rel`] reads it
/// back.
pub fn level_rel(at: &Whose, local: bool) -> Option<String> {
    let dir = if local { LOCAL_DIR } else { PROJECT_DIR };
    match at {
        Whose::Project => Some(dir.to_owned()),
        Whose::Workspace(ws) => plain(ws).then(|| format!("{WORKSPACES_DIR}/{ws}/{dir}")),
        Whose::Persona(persona) => {
            plain(persona).then(|| format!("{PERSONAS_DIR}/{persona}/{dir}"))
        }
    }
}

impl Whose {
    /// **Every kind of level's owner, its name `*`**: what a pattern over every level of every
    /// plane is spelled from by the one rule ([`level_rel`]), so a level added here is in each
    /// pattern too.
    pub fn every_kind() -> [Whose; 3] {
        [
            Whose::Project,
            Whose::Workspace("*".to_owned()),
            Whose::Persona("*".to_owned()),
        ]
    }

    /// The workspace it is, or `None`.
    pub fn workspace(&self) -> Option<&str> {
        match self {
            Whose::Workspace(ws) => Some(ws),
            Whose::Project | Whose::Persona(_) => None,
        }
    }

    /// The persona it is, or `None`.
    pub fn persona(&self) -> Option<&str> {
        match self {
            Whose::Persona(persona) => Some(persona),
            Whose::Project | Whose::Workspace(_) => None,
        }
    }
}

/// **Every level's local folder, as a glob relative to the plane** (ADR 0099 §1):
/// `skills.local`, `workspaces/*/skills.local`, `personas/*/skills.local`, by the one rule. What
/// the plane's `.gitignore` baseline and a save's excludes are spelled from.
pub fn local_globs() -> Vec<String> {
    Whose::every_kind()
        .iter()
        .filter_map(|whose| level_rel(whose, true))
        .collect()
}

/// Whether the plane-relative path `rel` (`/` between its parts) is a level's local folder or
/// inside one, by the one rule.
pub fn in_local_level(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    (1..=parts.len().min(3))
        .any(|at| level_of_rel(&parts[..at].join("/")).is_some_and(|(_, local)| local))
}

/// The level whose skills folder `rel` is, as [`level_rel`] spells it: whose it is, and whether
/// it is local. `None` for any other path.
pub fn level_of_rel(rel: &str) -> Option<(Whose, bool)> {
    let local = |name: &str| match name {
        PROJECT_DIR => Some(false),
        LOCAL_DIR => Some(true),
        _ => None,
    };
    let parts: Vec<&str> = rel.split('/').collect();
    match parts.as_slice() {
        [one] => Some((Whose::Project, local(one)?)),
        [WORKSPACES_DIR, ws, one] if plain(ws) => {
            Some((Whose::Workspace((*ws).to_owned()), local(one)?))
        }
        [PERSONAS_DIR, persona, one] if plain(persona) => {
            Some((Whose::Persona((*persona).to_owned()), local(one)?))
        }
        _ => None,
    }
}

/// The file beside a skill's `folder` that holds what purlis alone knows about it:
/// `<folder>.toml` (ADR 0099 §1). Share moves it with the folder.
pub fn beside(folder: &Path) -> PathBuf {
    let mut name = folder.file_name().unwrap_or_default().to_os_string();
    name.push(".toml");
    folder.with_file_name(name)
}

/// Whether `name` is one plain path component, as a workspace's folder name is.
pub(crate) fn plain(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!(
        (parts.next(), parts.next()),
        (Some(std::path::Component::Normal(_)), None)
    ) && !name.contains(['/', '\\'])
}

/// What `chat`'s skill set is, and why, on `machine` in the plane at `plane` (`None` for a chat
/// purlis knows no plane for, which gets the built-in skills alone).
///
/// Every built-in skill reaches every chat by its harness's route, and its name is never taken.
/// Then the plane's levels, narrowest first, so the first copy of a name is the one handed over
/// and every other copy is left out saying which copy hid it (ADR 0099 §3): the persona's local
/// skills, its shared skills, the workspace's local skills, its shared skills, the project's
/// local skills and its shared skills. A project skill reaches a chat at the plane root; a chat
/// in a workspace gets it only where its grant reaches that workspace, or the chat's persona
/// names it, and the workspace has not turned it off. A local skill is read only where
/// `machine` says git keeps it on this machine. A folder that is not a whole skill is left out
/// with what it lacks. Last, each skill the persona names and the chat does not get, and no
/// copy of explains, is left out: a built-in skill there is none of, and a harness skill this
/// machine does not have.
pub fn skill_set(plane: Option<&Path>, machine: &Machine<'_>, chat: &Chat<'_>) -> SkillSet {
    let mut set = SkillSet::default();
    let route = chat.harness.route;
    if let Some(dir) = machine.built_in {
        let root = dir.parent().unwrap_or(dir);
        level(
            &mut set,
            (root, dir),
            (Origin::BuiltIn, false),
            route,
            |_| None,
        );
    }
    if let Some(plane) = plane {
        if let Some(persona) = chat.persona {
            for local in [true, false] {
                if let Some(dir) = persona_dir(plane, persona, local) {
                    let made = made_since(machine, &plane.join("personas").join(persona));
                    level(
                        &mut set,
                        (plane, &dir),
                        (Origin::Persona, local),
                        route,
                        |skill| {
                            if let Some(why) = &made {
                                return Some(Reason::MadeWhileAChatRan(why.clone()));
                            }
                            if local {
                                kept_here(machine, folder_of_skill(skill))
                            } else {
                                None
                            }
                        },
                    );
                }
            }
        }
        let workspace = chat.workspace.filter(|ws| plain(ws));
        if let Some(ws) = workspace {
            for local in [true, false] {
                if let Some(dir) = level_dir(plane, &Whose::Workspace(ws.to_owned()), local) {
                    let made = made_since(machine, &plane.join(WORKSPACES_DIR).join(ws));
                    level(
                        &mut set,
                        (plane, &dir),
                        (Origin::Workspace, local),
                        route,
                        |skill| {
                            if let Some(why) = &made {
                                return Some(Reason::MadeWhileAChatRan(why.clone()));
                            }
                            if local {
                                kept_here(machine, folder_of_skill(skill))
                            } else {
                                None
                            }
                        },
                    );
                }
            }
        }
        for local in [true, false] {
            let Some(dir) = level_dir(plane, &Whose::Project, local) else {
                continue;
            };
            level(
                &mut set,
                (plane, &dir),
                (Origin::Project, local),
                route,
                |skill| {
                    let folder = folder_of_skill(skill);
                    if local && let Some(why) = kept_here(machine, folder) {
                        return Some(why);
                    }
                    // A persona brings what it names into every workspace, whatever the workspace
                    // was granted or turned off (ADR 0099 as amended 2026-10-21).
                    if chat.names.iter().any(|one| one.names_plane(&skill.name)) {
                        return None;
                    }
                    chat.workspace
                        .and_then(|ws| reach(folder, ws, chat.turned_off, &skill.name))
                },
            );
        }
    }
    named_elsewhere(&mut set, machine, chat.names);
    set.effective
        .sort_by(|a, b| a.skill.name.cmp(&b.skill.name));
    widest_first(&mut set.left_out);
    set
}

/// Why the levels of the workspace or persona folder `owner` reach no chat, where `machine`
/// asks (SK-9).
fn made_since(machine: &Machine<'_>, owner: &Path) -> Option<String> {
    machine.made_since.and_then(|asked| asked(owner))
}

/// Why a project skill in `folder`, named `name`, does not reach a chat in `workspace`, which
/// turned `off` off, or `None` where it does.
fn reach(folder: &Path, workspace: &str, off: &[String], name: &str) -> Option<Reason> {
    let grant = match grant_of(folder) {
        Ok(grant) => grant,
        Err(why) => return Some(Reason::GrantUnread(why)),
    };
    if !plain(workspace) || !grant.reaches(workspace) {
        return Some(Reason::NotGranted);
    }
    off.iter()
        .any(|one| one == name)
        .then_some(Reason::TurnedOff)
}

/// Each skill `names` holds that the set neither gets nor leaves out already: a built-in skill
/// there is none of is left out (where the built-in skills were read); a harness skill is the
/// harness's to load where `machine` has it or could not tell, and left out where it looked and
/// found none. A plane skill named and found in no level is a harness skill to look for, since the
/// harness may have it by that name.
fn named_elsewhere(set: &mut SkillSet, machine: &Machine<'_>, names: &[Reference]) {
    for reference in names {
        let said = reference.to_string();
        let held = |set: &SkillSet| {
            set.effective.iter().any(|e| e.skill.name == said)
                || set.left_out.iter().any(|l| l.name == said)
                || set
                    .from_the_harness
                    .iter()
                    .any(|h| h.reference == *reference)
        };
        if held(set) {
            continue;
        }
        let (origin, folder, reason) = match reference {
            Reference::BuiltIn(name) => {
                let Some(dir) = machine.built_in else {
                    continue;
                };
                let built_in = set
                    .effective
                    .iter()
                    .any(|e| e.origin == Origin::BuiltIn && e.skill.name == *name);
                if built_in {
                    continue;
                }
                (Origin::BuiltIn, dir.join(name), Reason::NoSuchBuiltIn)
            }
            _ => match machine.harness.map(|here| reference.on(here)) {
                Some(Seen::Missing) => (Origin::Harness, PathBuf::new(), Reason::NotOnThisMachine),
                seen => {
                    set.from_the_harness.push(FromHarness {
                        reference: reference.clone(),
                        checked: seen == Some(Seen::Here),
                    });
                    continue;
                }
            },
        };
        set.left_out.push(LeftOut {
            name: said,
            origin,
            local: false,
            folder,
            reason,
        });
    }
}

/// Why the local skill in `folder` is not read, or `None` where git keeps it on this machine.
fn kept_here(machine: &Machine<'_>, folder: &Path) -> Option<Reason> {
    let Some(ask) = machine.kept_local else {
        return Some(Reason::NotKeptLocal(
            "a local skill is used only while it stays on this machine, and purlis could not ask \
             git whether it does"
                .to_owned(),
        ));
    };
    ask(folder).err().map(|why| {
        Reason::NotKeptLocal(format!(
            "a local skill is used only while it stays on this machine, and {why}: purlis \
             reinit keeps it here"
        ))
    })
}

/// A skill's folder: its `SKILL.md`'s directory.
fn folder_of_skill(skill: &Skill) -> &Path {
    skill.path.parent().unwrap_or(&skill.path)
}

/// The rank of a level, widest first: the order a person reads the copies of one name in.
fn rank(origin: Origin, local: bool) -> u8 {
    let level = match origin {
        Origin::BuiltIn => 0,
        Origin::Project => 1,
        Origin::Workspace => 2,
        Origin::Persona => 3,
        Origin::Harness => 4,
        Origin::Repo => 5,
    };
    level * 2 + u8::from(local)
}

/// The left-out skills by name, and the copies of one name widest level first. Stable, so two
/// folders of one level stay in the order of their folders' names.
fn widest_first(left_out: &mut [LeftOut]) {
    left_out.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then(rank(a.origin, a.local).cmp(&rank(b.origin, b.local)))
    });
}

/// Every folder in `dir`, one level's, into `set`: each whole skill the chat gets, unless a
/// skill of its name is already in the set or `held` says why not; every other folder left out
/// with its reason. Folders are taken in the order of their names, so a tie inside one level
/// always goes the same way.
///
/// `dir` is under `root`, the folder it is reached from without following a link
/// ([`read::scan`]): the plane's root, or the bundle's plugin folder for the built-in skills.
fn level(
    set: &mut SkillSet,
    (root, dir): (&Path, &Path),
    (origin, local): (Origin, bool),
    route: Route,
    held: impl Fn(&Skill) -> Option<Reason>,
) {
    let Ok(rel) = dir.strip_prefix(root) else {
        return;
    };
    let mut folders = read::scan(root, rel);
    folders.sort_by(|a, b| folder_of(a).cmp(folder_of(b)));
    for folder in folders {
        let skill = match folder {
            Ok(skill) => skill,
            Err((folder, missing)) => {
                set.left_out.push(LeftOut {
                    name: folder
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    origin,
                    local,
                    folder,
                    reason: Reason::NotASkill(missing),
                });
                continue;
            }
        };
        let folder = skill
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        // An archived copy never loads, and takes no name from a wider one (decision 62).
        let about = if origin.of_the_plane() {
            about_of(&folder).unwrap_or_default()
        } else {
            About::default()
        };
        if about.lifecycle == Lifecycle::Archived {
            set.left_out.push(LeftOut {
                name: skill.name,
                origin,
                local,
                folder,
                reason: Reason::Archived,
            });
            continue;
        }
        if origin.of_the_plane() && skill.name == router::INDEX {
            set.left_out.push(LeftOut {
                name: skill.name,
                origin,
                local,
                folder,
                reason: Reason::IndexName,
            });
            continue;
        }
        let taken = set
            .effective
            .iter()
            .find(|effective| effective.skill.name == skill.name)
            .map(|effective| {
                if effective.origin == origin && effective.local == local {
                    Reason::Twin { origin, local }
                } else {
                    Reason::NameTaken {
                        by: effective.origin,
                        local: effective.local,
                    }
                }
            });
        match taken.or_else(|| held(&skill)) {
            None => set.effective.push(Effective {
                skill,
                root: root.to_path_buf(),
                route,
                origin,
                local,
                deprecated: match about.lifecycle {
                    Lifecycle::Deprecated(deprecated) => Some(deprecated),
                    _ => None,
                },
            }),
            Some(reason) => set.left_out.push(LeftOut {
                name: skill.name,
                origin,
                local,
                folder,
                reason,
            }),
        }
    }
}

/// The folder a scanned entry is about.
fn folder_of(entry: &Result<Skill, (PathBuf, Missing)>) -> &Path {
    match entry {
        Ok(skill) => skill.path.parent().unwrap_or(&skill.path),
        Err((folder, _)) => folder,
    }
}

#[cfg(test)]
mod tests;
