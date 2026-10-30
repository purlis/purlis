// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Skill studio's reads** (SK-2, #2765; SK-4, #2767; SK-5, #2768): every skill a chat in a
//! project gets, at its root, in one workspace or as one persona, with its level, and every one
//! left out with why, as `purlis_core::skills::in_the_studio` works them out; Share, which makes
//! a local skill shared; and a persona's `skills:` list, which its page adds to and takes from.
//! And each harness's own (SK-6, #2769): its plugins, the skills it finds itself in the person's
//! folders and in the workspace's clones, read only, each with a switch where the harness can
//! leave it out, and every one that clashes with a skill purlis hands the chat.
//! Behind the `skill-studio` flag until its finish line (#2786).

use serde::Serialize;
use specta::Type;

pub(crate) mod needs;

/// The catalogue row that opens the Skill studio on one scope's chats is this, then the scope's
/// key (`app/src/skillStudio.ts`).
pub(crate) const STUDIO_IN: &str = "skills.studio:";

/// What a persona's scope key starts with, before its name. A workspace's key is its plain name,
/// which never holds the `/`.
pub(crate) const PERSONA_SCOPE_KEY: &str = "persona/";

/// The catalogue row that opens the Skill studio on a chat as `persona`.
pub(crate) fn studio_as(persona: &str) -> String {
    format!("{STUDIO_IN}{PERSONA_SCOPE_KEY}{persona}")
}

/// The catalogue row that opens the Skill studio's Needs you on one scope's chats is this, then
/// the scope's key (#3695): a summary line's "need you".
pub(crate) const NEEDS_IN: &str = "skills.needs:";

/// The catalogue row that opens Needs you on a chat as `persona`.
pub(crate) fn needs_as(persona: &str) -> String {
    format!("{NEEDS_IN}{PERSONA_SCOPE_KEY}{persona}")
}

/// **How many items Needs you holds for a chat in `workspace` as `persona`** (#3695), read as
/// [`skill_studio`] reads them ([`with_the_projects_needs`]), for a summary line. `waiting` is
/// the files purlis's own proposed changes wait on.
pub(crate) fn needing_here(
    root: &std::path::Path,
    waiting: &[String],
    workspace: Option<&str>,
    persona: Option<&str>,
) -> usize {
    let built_in =
        crate::bundled_plugin_here().and_then(|plugin| purlis_core::skills::in_bundle(&plugin));
    let mut studio = studio(
        root,
        built_in.as_deref(),
        true,
        workspace,
        persona,
        &purlis_core::harness_plugin::Env::of(&[]),
    );
    with_the_projects_needs(&mut studio, root, waiting);
    studio.needs.len()
}

/// **The project's own items of Needs you, added to `studio`'s** (#3689): the old
/// `[harness_plugins]` keys, whose Ask Needs you holds while the studio is on. A file whose
/// removal waits among `waiting`, the files purlis's own proposed changes wait on, is asked
/// about no more. The one way the studio, its count and the summary lines add them.
pub(crate) fn with_the_projects_needs(
    studio: &mut Studio,
    root: &std::path::Path,
    waiting: &[String],
) {
    if !studio.on {
        return;
    }
    let old = crate::oldplugins::notice_of(root, waiting);
    studio.needs = needs::with_old_keys(std::mem::take(&mut studio.needs), old.as_ref());
}

/// The level a skill comes from, as the studio draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    BuiltIn,
    Project,
    Workspace,
    Persona,
    /// A harness skill: the harness's own, which purlis never hands over.
    Installed,
    /// A repo's own, in a clone (SK-6).
    Repo,
}

impl From<purlis_core::skills::Origin> for Level {
    fn from(origin: purlis_core::skills::Origin) -> Level {
        match origin {
            purlis_core::skills::Origin::BuiltIn => Level::BuiltIn,
            purlis_core::skills::Origin::Project => Level::Project,
            purlis_core::skills::Origin::Workspace => Level::Workspace,
            purlis_core::skills::Origin::Persona => Level::Persona,
            purlis_core::skills::Origin::Harness => Level::Installed,
            purlis_core::skills::Origin::Repo => Level::Repo,
        }
    }
}

/// One skill the studio lists: [`purlis_core::skills::StudioRow`], for the window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioSkill {
    /// Its name, or its folder's where it names none.
    pub name: String,
    pub level: Level,
    /// Its level as a person reads it: `project`, `local workspace`, the words the command line
    /// shows (`purlis_core::skills::level_shown`).
    pub label: String,
    /// Whether it is a local skill, kept on this machine in its level's `skills.local/`, which
    /// Share makes shared.
    pub local: bool,
    /// Where a project skill reaches: `root`, `all`, or the workspaces' names. `None` for a
    /// skill of another level.
    pub grant: Option<String>,
    /// The same, for the studio's grant control to start from (#2770).
    pub granted: Option<crate::skillchange::SkillGrant>,
    /// Whether the workspace the studio shows turned this project skill off (#2770): its switch
    /// turns it back on. `false` at the project's root.
    pub turned_off: bool,
    /// Where the studio shows a workspace's chats and this project skill's grant does not reach
    /// it: why it cannot be turned on there, and the grant that would reach it (#2770), the
    /// core's own sentence. `None` otherwise.
    pub not_granted: Option<String>,
    /// Whether the chat the studio shows gets it.
    pub on: bool,
    /// Why it is left out, in one line, where it is.
    pub left_out: Option<String>,
    /// Where it is left out because a chat could have made its level's folder (SK-9): that
    /// folder, which Accept takes (`skill_level_accept`).
    pub accept: Option<String>,
    /// What it does and when to use it: its frontmatter's `description`.
    pub description: String,
    /// Its folder, relative to the project where it is inside it; empty for an installed skill.
    pub folder: String,
    /// How a persona's skills name it: `purlis:<name>` for a built-in skill, `<plugin>:<name>`
    /// for a plugin's, else its name (#2768). What Add and Remove send.
    pub written: String,
    /// Whether the persona the studio shows names it, so Remove can take it out.
    pub named: bool,
    /// What is said of an installed skill the persona names that purlis cannot check this
    /// machine for, or `None`.
    pub unchecked: Option<String>,
    /// Whose a skill from outside the plane is: "you", a plugin's, a repo's. `None` for the
    /// plane's levels and the built-in skills (SK-6).
    pub owner: Option<Owner>,
    /// How a skill from outside the plane's harness can leave it out. `None` for the plane's
    /// levels and the built-in skills, whose switches are their own.
    pub switch: Option<SkillSwitch>,
    /// Its compatibility badge on each harness that would load it (SK-8, #2771): every harness
    /// for a skill a chat gets from purlis, its own for one from outside the plane, none for a
    /// skill left out.
    pub compat: Vec<SkillCompat>,
    /// What the built-in lints say of a skill from outside the plane (SK-8): that it widens the
    /// chat, and what else is worth knowing. Empty for the plane's and the built-in skills.
    pub lint: Vec<String>,
    /// Each harness whose chats a router hid it from the list of, and why (SK-15), each by its
    /// own budget: the chat still has it, through the index skill. Empty where every one lists it.
    pub hidden: Vec<SkillHiddenOn>,
    /// Whether that chat's workspace or persona pins it, so no router hides it.
    pub pinned: bool,
    /// Which of the studio's workspace and persona have it in their pins, as written (SK-15d):
    /// what the row's Pin switch shows. Empty for a skill from outside the plane.
    pub pinned_by: Vec<crate::skillchange::SkillPinner>,
}

/// A skill router (SK-15, ADR 0102): the core's `Router`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum SkillRouter {
    /// purlis's own.
    BuiltIn,
    /// An extension that declares `skill-router`, by its id (SK-15b).
    Extension(String),
}

impl From<purlis_core::skills::Router> for SkillRouter {
    fn from(router: purlis_core::skills::Router) -> SkillRouter {
        match router {
            purlis_core::skills::Router::BuiltIn => SkillRouter::BuiltIn,
            purlis_core::skills::Router::Extension(id) => SkillRouter::Extension(id),
        }
    }
}

/// Why a router hid a skill: the core's `HiddenWhy`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum SkillHiddenWhy {
    /// The chat's skills are past its budget, and this one is not pinned.
    OverBudget,
    /// An extension router hid it, with its own one line (SK-15b).
    ByRouter(String),
}

impl From<purlis_core::skills::HiddenWhy> for SkillHiddenWhy {
    fn from(why: purlis_core::skills::HiddenWhy) -> SkillHiddenWhy {
        match why {
            purlis_core::skills::HiddenWhy::OverBudget => SkillHiddenWhy::OverBudget,
            purlis_core::skills::HiddenWhy::ByRouter(line) => {
                SkillHiddenWhy::ByRouter(line.as_str().to_owned())
            }
        }
    }
}

/// **A skill a router hid** from a chat's list (SK-15): which router, why, and the core's line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillHidden {
    pub router: SkillRouter,
    pub why: SkillHiddenWhy,
    /// The line a person is shown.
    pub said: String,
}

impl From<&purlis_core::skills::Hidden> for SkillHidden {
    fn from(hidden: &purlis_core::skills::Hidden) -> SkillHidden {
        SkillHidden {
            router: hidden.router.clone().into(),
            why: hidden.why.clone().into(),
            said: hidden.to_string(),
        }
    }
}

/// A skill a router hid on one harness's chats (SK-15).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillHiddenOn {
    pub harness: HarnessKind,
    /// The harness, as a person reads its name.
    pub title: String,
    pub hidden: SkillHidden,
}

impl From<&purlis_core::skills::HiddenOn> for SkillHiddenOn {
    fn from(on: &purlis_core::skills::HiddenOn) -> SkillHiddenOn {
        SkillHiddenOn {
            harness: on.harness.into(),
            title: on.harness.title().to_owned(),
            hidden: (&on.hidden).into(),
        }
    }
}

/// How well a skill fits a harness, as its badge draws it (SK-8): the core's `Fit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum SkillFit {
    /// Everything it asks for applies.
    Full,
    /// Something it asks for is left out, ignored or not applied.
    Partial,
    /// A harness or repo skill whose harness applies what widens the chat, or that purlis cannot
    /// read, so may.
    Widens,
    /// A skill purlis hands to no chat.
    Refused,
}

/// **A skill's compatibility badge on one harness** (SK-8, #2771; decision 18): how well it
/// fits, and the core's lines that say why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillCompat {
    pub harness: HarnessKind,
    /// The harness, as a person reads its name.
    pub title: String,
    pub fit: SkillFit,
    /// What the badge says, a line each: what applies, what is left out, what is carried over by
    /// purlis's equivalent, what is not applied.
    pub says: Vec<String>,
}

impl From<&purlis_core::skills::HarnessCompat> for SkillCompat {
    fn from(one: &purlis_core::skills::HarnessCompat) -> SkillCompat {
        use purlis_core::skills::Fit;
        SkillCompat {
            harness: one.harness.into(),
            title: one.harness.title().to_owned(),
            fit: match one.compat.fit {
                Fit::Full => SkillFit::Full,
                Fit::Partial => SkillFit::Partial,
                Fit::Widens => SkillFit::Widens,
                Fit::Refused => SkillFit::Refused,
            },
            says: one.compat.lines(),
        }
    }
}

/// Whose a skill from outside the plane is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Owner {
    /// The person's own.
    You,
    /// A plugin's, by its name.
    Plugin { name: String },
    /// A repo's, by its clone's name.
    Repo { name: String },
}

impl From<purlis_core::skills::Owner> for Owner {
    fn from(owner: purlis_core::skills::Owner) -> Owner {
        match owner {
            purlis_core::skills::Owner::You => Owner::You,
            purlis_core::skills::Owner::Plugin(name) => Owner::Plugin { name },
            purlis_core::skills::Owner::Repo(name) => Owner::Repo { name },
        }
    }
}

/// A harness, as the window and the core name one to each other: a profile's `kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessKind {
    Claude,
    Opencode,
    Codex,
}

impl From<purlis_core::harness::Harness> for HarnessKind {
    fn from(harness: purlis_core::harness::Harness) -> HarnessKind {
        match harness {
            purlis_core::harness::Harness::ClaudeCode => HarnessKind::Claude,
            purlis_core::harness::Harness::Opencode => HarnessKind::Opencode,
            purlis_core::harness::Harness::Codex => HarnessKind::Codex,
        }
    }
}

impl From<HarnessKind> for purlis_core::harness::Harness {
    fn from(kind: HarnessKind) -> purlis_core::harness::Harness {
        match kind {
            HarnessKind::Claude => purlis_core::harness::Harness::ClaudeCode,
            HarnessKind::Opencode => purlis_core::harness::Harness::Opencode,
            HarnessKind::Codex => purlis_core::harness::Harness::Codex,
        }
    }
}

/// How a harness can leave one of its own skills out of a chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SkillSwitch {
    /// On its own: the studio draws a switch.
    Alone,
    /// Not on its own, and why, which the studio says in place of a switch.
    Not { why: String },
}

impl From<purlis_core::skills::Switch> for SkillSwitch {
    fn from(switch: purlis_core::skills::Switch) -> SkillSwitch {
        match switch {
            purlis_core::skills::Switch::Alone => SkillSwitch::Alone,
            purlis_core::skills::Switch::Not(why) => SkillSwitch::Not { why },
        }
    }
}

/// One harness in the studio (SK-6): the skills it finds itself. Its plugins are its own to turn
/// on or off (#3467): a plugin's skills are among these, read-only, each naming its plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioHarness {
    /// Which harness it is, which a switch names.
    pub harness: HarnessKind,
    pub title: String,
    /// The skills it finds itself, each by the name it knows it by.
    pub skills: Vec<StudioSkill>,
    /// The project's skills a repo of the workspace leaves out of its chats in that clone, each
    /// with why (SK-53): the core's `HarnessInTheStudio::repo_left_out`.
    pub repo_left_out: Vec<StudioSkill>,
}

/// A harness or repo skill with the name of a skill purlis hands a chat (ADR 0099 §3): both
/// load, so the studio marks purlis's copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioClash {
    pub name: String,
    /// The level of the plane's copy, and whether it is local.
    pub level: Level,
    pub local: bool,
    /// The harness that loads the other copy, as a person reads its name.
    pub harness: String,
    /// The other copy: a harness's or a repo's, and whose.
    pub outside: Level,
    pub owner: Owner,
    pub outcome: ClashOutcome,
    /// What the row of the plane's copy says of it, in the core's words
    /// (`purlis_core::skills::clash_said`, SK-46): the same line `purlis skill list` prints.
    pub said: String,
}

/// What came of a clash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum ClashOutcome {
    /// Both copies load.
    BothLoad,
    /// The repo's narrower copy is the one the chat gets; the plane's is left out.
    RepoWins,
    /// Both load under one name, and the chat may use either: a copy named as one of purlis's
    /// own skills where the program reads every skill by its bare name.
    EitherOne,
}

/// Each pinner a chat in `workspace` on `persona` reads, with the pins it has written (SK-15d).
fn pinners_of(
    root: &std::path::Path,
    workspace: Option<&str>,
    persona: Option<&str>,
) -> Vec<(purlis_core::skills::PinnedBy, Vec<String>)> {
    workspace
        .map(|ws| purlis_core::skills::PinnedBy::Workspace(ws.to_owned()))
        .into_iter()
        .chain(persona.map(|p| purlis_core::skills::PinnedBy::Persona(p.to_owned())))
        .map(|by| {
            let pins = purlis_core::skills::pins_written(root, &by);
            (by, pins)
        })
        .collect()
}

/// Which of `pinners` pin the skill `name` (SK-15d).
fn pinned_by_of(
    pinners: &[(purlis_core::skills::PinnedBy, Vec<String>)],
    name: &str,
) -> Vec<crate::skillchange::SkillPinner> {
    pinners
        .iter()
        .filter(|(_, pins)| pins.iter().any(|one| one == name))
        .map(|(by, _)| by.clone().into())
        .collect()
}

/// **One of the plane's or the built-in skills' rows, for the window**, for a chat at the root or
/// in `workspace`, which turned off the project skills `off` and is pinned by `pinned_by`
/// (SK-15d): its grant, and where the grant does not reach the workspace, why. The studio's rows
/// and each persona's (#3266) alike.
fn plane_shown(
    root: &std::path::Path,
    workspace: Option<&str>,
    off: &[String],
    pinned_by: Vec<crate::skillchange::SkillPinner>,
    row: purlis_core::skills::StudioRow,
) -> StudioSkill {
    let grant = (row.origin == purlis_core::skills::Origin::Project).then(|| {
        purlis_core::skills::grant_of(&root.join(&row.folder))
            .unwrap_or(purlis_core::skills::Grant::Root)
    });
    let not_granted = match (&grant, workspace) {
        (Some(grant), Some(ws)) if !grant.reaches(ws) => {
            Some(purlis_core::skills::not_granted(&row.name, ws, grant, true))
        }
        _ => None,
    };
    StudioSkill {
        granted: grant.map(Into::into),
        not_granted,
        turned_off: row.origin == purlis_core::skills::Origin::Project && off.contains(&row.name),
        name: row.name,
        level: row.origin.into(),
        label: purlis_core::skills::level_shown(row.origin, row.local),
        local: row.local,
        grant: row.grant,
        on: row.left_out.is_none(),
        accept: row.accept,
        left_out: row.left_out,
        description: row.description,
        folder: row.folder,
        written: row.written,
        named: row.named,
        unchecked: row
            .unchecked
            .then(|| purlis_core::skills::UNCHECKED.to_owned()),
        owner: None,
        switch: None,
        compat: row.compat.iter().map(Into::into).collect(),
        lint: row.lint,
        hidden: row.hidden.iter().map(Into::into).collect(),
        pinned: row.pinned,
        pinned_by,
    }
}

/// **One persona's own skills** (#3266), as the Skills view lists them: the rows of its own
/// level, for a chat as it in the workspace the view shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct PersonaSkills {
    pub persona: String,
    pub skills: Vec<StudioSkill>,
}

/// Every persona's own skills in the project at `root`, for a chat in `workspace` or at the
/// root: nothing while the flag is off (`purlis_core::skills::persona_rows`).
pub(crate) fn persona_skills(
    root: &std::path::Path,
    built_in: Option<&std::path::Path>,
    on: bool,
    workspace: Option<&str>,
) -> Vec<PersonaSkills> {
    if !on {
        return Vec::new();
    }
    let off = workspace
        .map(|ws| purlis_core::skills::turned_off(root, ws))
        .unwrap_or_default();
    purlis_core::skills::persona_rows(built_in, root)
        .into_iter()
        .map(|purlis_core::skills::PersonaRows { persona, rows }| {
            let pinners = pinners_of(root, workspace, Some(&persona));
            PersonaSkills {
                skills: rows
                    .into_iter()
                    .map(|row| {
                        let by = pinned_by_of(&pinners, &row.name);
                        plane_shown(root, workspace, &off, by, row)
                    })
                    .collect(),
                persona,
            }
        })
        .collect()
}

/// What the studio shows: whether it is on, whose chat it shows, and its skills by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct Studio {
    pub on: bool,
    /// The workspace whose chats it shows, or `None` for a chat at the project's root.
    pub workspace: Option<String>,
    /// The persona whose chats it shows, which brings its skills into every workspace, or
    /// `None` for a chat on no persona (SK-5).
    pub persona: Option<String>,
    /// Every workspace of the project, which it can be shown for.
    pub workspaces: Vec<String>,
    /// Every level a skill can be made at or moved to, with its words (#2770).
    pub levels: Vec<crate::skillchange::StudioLevel>,
    pub skills: Vec<StudioSkill>,
    /// Each harness, with its plugins and its own skills (SK-6).
    pub harnesses: Vec<StudioHarness>,
    /// Every skill from outside the plane that has the name of a skill a chat gets, by name.
    pub clashes: Vec<StudioClash>,
    /// What it offers to start from (SK-22, #2785): the core's `skills::first_open`.
    pub first: StudioFirst,
    /// Why a chat reads none of the pins of the workspace or the persona it is filtered to,
    /// where it reads none (SK-15d): its rows' Pin switches then say so.
    pub pins_refused: Option<crate::skillchange::SkillPinsRefused>,
    /// The declared harnesses skills are not supported on (SK-30): each skill's badge names them,
    /// and the harnesses' list says why in one line.
    pub not_supported: Vec<StudioNotSupported>,
    /// The router its chats are routed by, and how it did at each one's start (SK-15b). Null
    /// while the studio is off.
    pub router: Option<StudioRouter>,
    /// Where purlis cannot confirm which of the project's workspace and persona folders the
    /// person accepted (#3718): the one item the studio shows for the project, whose Accept
    /// accepts the folders it lists (`skill_levels_confirm`). Null otherwise.
    pub levels_to_confirm: Option<StudioLevelsToConfirm>,
    /// **What in these skills needs the person** (#3689): the Needs you page's items, by kind,
    /// each with why and one fix ([`needs`]). Empty while the studio is off.
    pub needs: Vec<needs::NeedsYou>,
}

/// **The one item for the project's folders** (#3718): what it says, and the folders its Accept
/// accepts, relative to the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioLevelsToConfirm {
    pub said: String,
    pub folders: Vec<String>,
}

/// **A declared harness skills are not supported on** (SK-30, #2985): the core's
/// `skills::NotSupported`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioNotSupported {
    /// The word a profile's `kind` names it by.
    pub name: String,
    pub title: String,
    /// Why its chats get no skills, in one line.
    pub said: String,
}

impl From<purlis_core::skills::NotSupported> for StudioNotSupported {
    fn from(one: purlis_core::skills::NotSupported) -> Self {
        Self {
            name: one.name,
            title: one.title,
            said: one.said,
        }
    }
}

/// **The router a studio's chats are routed by** (SK-15b, ADR 0102 §5, §6), and how it did at
/// the start of each such chat open here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioRouter {
    /// Which router, in a sentence; or why the one chosen is not one, where it is not.
    pub said: String,
    /// Whether the choice names no router, so its chats are routed by the built-in one.
    pub problem: bool,
    /// The extension's id, where an extension's router is chosen.
    pub extension: Option<String>,
    /// Each chat open here that its start routed by an extension, by its number, with how
    /// that went: its timing, and whether it fell back on the built-in router.
    pub chats: Vec<StudioRouted>,
    /// **The router's newest grade** (SK-15e, #3233), in a sentence: how many of the library's
    /// trigger tests pass with it and with the built-in router, and the library's commit it was
    /// graded on. Null where it has none, and for the built-in router.
    pub grade: Option<String>,
    /// **What the scope's own choice writes** (#3692): the persona's `skill-router:` line, or
    /// else the workspace's `settings.skills.router`, as written. Null where it writes none,
    /// and at the project's root, which chooses no router. What Change… starts from.
    pub written: Option<String>,
    /// **A chat's skill budget** (SK-12, ADR 0102 §1): past it the router decides what a chat's
    /// list shows. The same on every harness.
    pub budget: StudioBudget,
}

/// **A chat's skill budget**, as the Routing page says it: the most skills its list holds, and
/// the most characters of their descriptions together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct StudioBudget {
    pub skills: u32,
    pub chars: u32,
}

impl Default for StudioBudget {
    fn default() -> Self {
        let budget = purlis_core::harness::SKILL_BUDGET;
        StudioBudget {
            skills: u32::try_from(budget.skills).unwrap_or(u32::MAX),
            chars: u32::try_from(budget.chars).unwrap_or(u32::MAX),
        }
    }
}

/// One chat a router extension was asked about at its start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioRouted {
    pub session: u32,
    pub routing: crate::useskill::SkillRouting,
    /// **Every skill the routing hid at its start, by name** (#3692): the routing record's, so
    /// the Routing page says what a chat started without on its list.
    pub hidden: Vec<String>,
}

impl StudioRouted {
    /// Chat `session`'s routing at its start, as the Routing page lists it.
    pub(crate) fn of(session: u32, routed: &purlis_core::skills::router::Routed) -> Self {
        StudioRouted {
            session,
            routing: routed.into(),
            hidden: routed
                .record
                .hidden
                .iter()
                .map(|one| one.skill.clone())
                .collect(),
        }
    }
}

/// The router chats in `workspace` as `persona` of the project at `root` are routed by.
fn router_of(
    root: &std::path::Path,
    workspace: Option<&str>,
    persona: Option<&str>,
) -> StudioRouter {
    use purlis_core::skills::router::{Chosen, chosen};
    let (said, problem, extension) = match chosen(root, workspace, persona) {
        Ok(Chosen::BuiltIn) => (
            "Chats here are routed by the built-in router.".to_owned(),
            false,
            None,
        ),
        Ok(Chosen::Extension(id)) => (
            format!(
                "Chats here are routed by {}, asked at each chat's start; the rows show what \
                 the built-in router would hide.",
                purlis_core::skills::Router::Extension(id.clone()).shown()
            ),
            false,
            Some(id),
        ),
        Err(why) => (format!("{why}."), true, None),
    };
    let grade = extension.as_deref().and_then(|id| {
        purlis_core::skills::grades::routers::grades_of(root, id, workspace, persona)
            .first()
            .map(purlis_core::skills::grades::routers::RouterGraded::said)
    });
    let pinner = persona
        .map(|p| purlis_core::skills::PinnedBy::Persona(p.to_owned()))
        .or_else(|| workspace.map(|ws| purlis_core::skills::PinnedBy::Workspace(ws.to_owned())));
    StudioRouter {
        said,
        problem,
        extension,
        chats: Vec::new(),
        grade,
        written: pinner.and_then(|by| purlis_core::skills::router::written(root, &by)),
        budget: StudioBudget::default(),
    }
}

/// **What the studio offers when it opens** (SK-22, #2785; decision 42), as the core works it
/// out (`purlis_core::skills::first_open`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Type)]
pub struct StudioFirst {
    /// Whether the project has no skill of its own, so the three starts are offered: make one
    /// with a chat, browse skill sources, start blank.
    pub starts: bool,
    /// Where the project's skills are still where only some chat programs read them: how many,
    /// and the doctor's fix that moves them (SK-21).
    pub unmoved: Option<StudioUnmoved>,
}

/// The project's skills still to move into the studio's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioUnmoved {
    pub skills: u32,
    /// The doctor's fix that moves them, by its id (`planeDoctorFix`).
    pub fix: String,
}

impl From<purlis_core::skills::first_open::FirstOpen> for StudioFirst {
    fn from(first: purlis_core::skills::first_open::FirstOpen) -> Self {
        StudioFirst {
            starts: first.starts,
            unmoved: first.unmoved.map(|unmoved| StudioUnmoved {
                skills: u32::try_from(unmoved.skills).unwrap_or(u32::MAX),
                fix: unmoved.fix.id().to_owned(),
            }),
        }
    }
}

/// The studio of the project at `root`, for a chat at its root (`workspace` `None`) or in
/// `workspace`, as `persona` or on none, with the built-in skills in `built_in`: nothing while
/// the flag is off. Its list is the command line's (`purlis_core::skills::studio_list`).
pub(crate) fn studio(
    root: &std::path::Path,
    built_in: Option<&std::path::Path>,
    on: bool,
    workspace: Option<&str>,
    persona: Option<&str>,
    env: &purlis_core::harness_plugin::Env<'_>,
) -> Studio {
    let workspace = workspace.map(str::to_owned);
    let persona = persona.map(str::to_owned);
    if !on {
        return Studio {
            on,
            workspace,
            persona,
            workspaces: Vec::new(),
            levels: Vec::new(),
            skills: Vec::new(),
            harnesses: Vec::new(),
            clashes: Vec::new(),
            first: StudioFirst::default(),
            pins_refused: None,
            not_supported: Vec::new(),
            router: None,
            levels_to_confirm: None,
            needs: Vec::new(),
        };
    }
    let off = workspace
        .as_deref()
        .map(|ws| purlis_core::skills::turned_off(root, ws))
        .unwrap_or_default();
    // Each pinner the studio is filtered to, with what it has written (SK-15d).
    let pinners = pinners_of(root, workspace.as_deref(), persona.as_deref());
    let pins_refused = pinners
        .iter()
        .find_map(|(by, _)| crate::skillchange::pins_refused(root, by));
    let pinned_by = |name: &str| pinned_by_of(&pinners, name);
    let purlis_core::skills::StudioList {
        rows,
        harnesses: all,
        not_supported,
    } = purlis_core::skills::studio_list(
        built_in,
        root,
        workspace.as_deref(),
        persona.as_deref(),
        env,
    );
    let first = purlis_core::skills::first_open::first_open(root, &rows).into();
    let skills = rows
        .into_iter()
        .map(|row| {
            let by = pinned_by(&row.name);
            plane_shown(root, workspace.as_deref(), &off, by, row)
        })
        .collect();
    let (harnesses, clashes) = harnesses(&all);
    let workspaces = purlis_core::workspaces::Plane::open(root)
        .workspaces()
        .unwrap_or_default();
    let router = router_of(root, workspace.as_deref(), persona.as_deref());
    let mut studio = Studio {
        on,
        workspace,
        persona,
        workspaces,
        levels: crate::skillchange::levels(root),
        skills,
        harnesses,
        clashes,
        first,
        pins_refused,
        not_supported: not_supported.into_iter().map(Into::into).collect(),
        router: Some(router),
        levels_to_confirm: purlis_core::skills::levels::to_confirm(root).map(|(said, folders)| {
            StudioLevelsToConfirm {
                said: said.to_owned(),
                folders,
            }
        }),
        needs: Vec::new(),
    };
    studio.needs = needs::of(&studio);
    studio
}

/// A row of a skill from outside the plane, for the window: read only, with whose it is and how
/// its harness can leave it out.
fn outside_shown(row: purlis_core::skills::StudioRow) -> StudioSkill {
    let (owner, switch) = match row.outside {
        Some(purlis_core::skills::OutsideRow { owner, switch }) => {
            (Some(owner.into()), Some(switch.into()))
        }
        None => (None, None),
    };
    StudioSkill {
        granted: None,
        not_granted: None,
        turned_off: false,
        name: row.name,
        level: row.origin.into(),
        label: purlis_core::skills::level_shown(row.origin, row.local),
        local: row.local,
        grant: row.grant,
        on: row.left_out.is_none(),
        accept: row.accept,
        left_out: row.left_out,
        description: row.description,
        folder: row.folder,
        written: row.written,
        named: row.named,
        unchecked: None,
        owner,
        switch,
        compat: row.compat.iter().map(Into::into).collect(),
        lint: row.lint,
        hidden: row.hidden.iter().map(Into::into).collect(),
        pinned: row.pinned,
        pinned_by: Vec::new(),
    }
}

/// Each harness in the studio, and every clash, for a chat whose plane skills are `set`.
fn harnesses(
    all: &[purlis_core::skills::HarnessInTheStudio],
) -> (Vec<StudioHarness>, Vec<StudioClash>) {
    let mut clashes: Vec<StudioClash> = Vec::new();
    let harnesses = all
        .iter()
        .map(|one| {
            let adapter = one.harness.adapter().plugins();
            for clash in &one.set.clashes {
                clashes.push(StudioClash {
                    name: clash.name.clone(),
                    level: clash.plane.origin.into(),
                    local: clash.plane.local,
                    harness: adapter.title().to_owned(),
                    outside: clash.outside.into(),
                    owner: clash.owner.clone().into(),
                    outcome: match clash.outcome {
                        purlis_core::skills::Outcome::BothLoad => ClashOutcome::BothLoad,
                        purlis_core::skills::Outcome::RepoWins => ClashOutcome::RepoWins,
                        purlis_core::skills::Outcome::EitherOne => ClashOutcome::EitherOne,
                    },
                    said: purlis_core::skills::clash_said(one.harness, clash),
                });
            }
            StudioHarness {
                harness: one.harness.into(),
                title: adapter.title().to_owned(),
                skills: one.rows.iter().cloned().map(outside_shown).collect(),
                repo_left_out: one
                    .repo_left_out
                    .iter()
                    .cloned()
                    .map(outside_shown)
                    .collect(),
            }
        })
        .collect();
    (harnesses, clashes)
}

/// What started a skill run, as the Runs panel draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum RunStart {
    /// The chat's own Skill tool.
    SkillTool,
    /// The chat read the skill's `SKILL.md`.
    Read,
    /// The chat loaded the skill with purlis's `skill_load` tool.
    McpLoad,
    /// The person picked it in Use skill… and sent the prompt it was typed into.
    UseSkill,
    /// A curation action typed the prompt the person sent.
    Curation,
    /// A trigger sent the prompt.
    Trigger,
    /// A grade sent the prompt: a trigger test's chat.
    Grade,
    /// A start a newer purlis wrote.
    Unknown,
}

impl From<purlis_core::skills::runs::StartedBy> for RunStart {
    fn from(started_by: purlis_core::skills::runs::StartedBy) -> RunStart {
        use purlis_core::skills::runs::StartedBy;
        match started_by {
            StartedBy::SkillTool => RunStart::SkillTool,
            StartedBy::Read => RunStart::Read,
            StartedBy::McpLoad => RunStart::McpLoad,
            StartedBy::UseSkill => RunStart::UseSkill,
            StartedBy::Curation => RunStart::Curation,
            StartedBy::Trigger => RunStart::Trigger,
            StartedBy::Grade => RunStart::Grade,
            _ => RunStart::Unknown,
        }
    }
}

/// How a skill run ended, as the Runs panel draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum RunEnd {
    /// With its turn.
    Turn,
    /// A new prompt came first.
    Interrupted,
    /// The chat ended under it.
    Session,
    /// An end a newer purlis wrote.
    Unknown,
}

impl From<purlis_core::skills::runs::Ended> for RunEnd {
    fn from(ended: purlis_core::skills::runs::Ended) -> RunEnd {
        use purlis_core::skills::runs::Ended;
        match ended {
            Ended::Turn => RunEnd::Turn,
            Ended::Interrupted => RunEnd::Interrupted,
            Ended::Session => RunEnd::Session,
            _ => RunEnd::Unknown,
        }
    }
}

/// One skill run, as a skill's Runs panel lists it (SK-3): what started it and its signals,
/// never a pass or a fail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioRun {
    /// Its id, or null for a use from before runs were recorded.
    pub id: Option<String>,
    pub skill: String,
    pub persona: Option<String>,
    /// What started it, or null for a use from before runs were recorded.
    pub started_by: Option<RunStart>,
    /// When it started.
    pub started: String,
    /// How it ended, or null while no end is recorded.
    pub ended: Option<RunEnd>,
    pub ms: Option<u32>,
    pub tool_calls: Option<u32>,
    pub tool_errors: Option<u32>,
    pub refusals: Option<u32>,
    pub asks: Option<u32>,
    /// Tokens spent inside it, where the harness reported them.
    pub tokens: Option<u32>,
    /// The skill whose run this one started inside, where it is a child span (SK-19).
    pub inside: Option<String>,
    /// The skills whose runs started inside this one, each once (SK-19).
    pub ran: Vec<String>,
    /// What it ran on, as a person calls it (`Claude Code`), or null where its start does not
    /// say.
    pub ran_on: Option<String>,
}

/// A count as the window's bindings carry it: at most `u32::MAX`.
pub(crate) fn small(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Every run of `skill` the project at `root` has recorded, newest first, at most `limit`.
pub(crate) fn runs(root: &std::path::Path, skill: &str, limit: usize) -> Vec<StudioRun> {
    purlis_core::skills::studio_runs(root, Some(skill), limit)
        .into_iter()
        .map(|nested| {
            let run = &nested.run;
            let end = run.end.as_ref();
            StudioRun {
                inside: nested.inside.clone(),
                ran: nested.ran.clone(),
                id: run.id.clone(),
                skill: run.skill.clone(),
                persona: run.persona.clone(),
                started_by: run.started_by.map(RunStart::from),
                started: run.started.clone(),
                ended: end.map(|end| RunEnd::from(end.ended)),
                ms: end.map(|end| small(end.ms)),
                tool_calls: end.map(|end| small(end.tool_calls)),
                tool_errors: end.map(|end| small(end.tool_errors)),
                refusals: end.map(|end| small(end.refusals)),
                asks: end.map(|end| small(end.asks)),
                tokens: end.and_then(|end| end.tokens).map(small),
                ran_on: run.ran_on(),
            }
        })
        .collect()
}

/// The group a skill's totals row is of (SK-20).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", content = "name", rename_all = "kebab-case")]
pub enum StatsGroup {
    /// A harness, as a person calls it; a persona or a workspace, by name.
    Named(String),
    /// No persona, or the project's root.
    None,
    /// Runs whose start does not say.
    NotRecorded,
}

/// One row of a skill's totals: a group's runs, their signals and their tokens (SK-20).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StatsRow {
    pub group: StatsGroup,
    pub runs: u32,
    /// Runs whose end is recorded: the ones the signals are counted from.
    pub ended: u32,
    pub tool_calls: u32,
    pub tool_errors: u32,
    pub refusals: u32,
    pub asks: u32,
    /// Tokens spent inside them, or null where no run said its tokens.
    pub tokens: Option<u32>,
    pub ms: u32,
}

/// A skill's totals, by what it ran on, by persona and by workspace (SK-20).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Type)]
pub struct SkillStats {
    pub by_harness: Vec<StatsRow>,
    pub by_persona: Vec<StatsRow>,
    pub by_workspace: Vec<StatsRow>,
}

/// **One totals row** of `group`'s `totals`, as the window draws it: a skill's totals by a
/// grouping (SK-20), and one version's runs (SK-19).
pub(crate) fn stats_row(group: StatsGroup, totals: purlis_core::skilluse::Totals) -> StatsRow {
    StatsRow {
        group,
        runs: small(totals.runs),
        ended: small(totals.ended),
        tool_calls: small(totals.tool_calls),
        tool_errors: small(totals.tool_errors),
        refusals: small(totals.refusals),
        asks: small(totals.asks),
        tokens: (totals.tokens_said > 0).then(|| small(totals.tokens)),
        ms: small(totals.ms),
    }
}

/// The totals of every run of `skill` the project at `root` has recorded
/// (`purlis_core::skilluse::stats`).
pub(crate) fn stats(root: &std::path::Path, skill: &str) -> SkillStats {
    use purlis_core::skilluse::{By, Group};
    let runs: Vec<purlis_core::skilluse::Run> =
        purlis_core::skills::studio_runs(root, Some(skill), usize::MAX)
            .into_iter()
            .map(|nested| nested.run)
            .collect();
    let rows = |by: By| -> Vec<StatsRow> {
        purlis_core::skilluse::stats(&runs, by)
            .into_iter()
            .map(|row| {
                let group = match by {
                    By::Harness => row.group.harness_titled(),
                    _ => row.group,
                };
                stats_row(
                    match group {
                        Group::Named(name) => StatsGroup::Named(name),
                        Group::None => StatsGroup::None,
                        Group::NotRecorded => StatsGroup::NotRecorded,
                    },
                    row.totals,
                )
            })
            .collect()
    };
    SkillStats {
        by_harness: rows(By::Harness),
        by_persona: rows(By::Persona),
        by_workspace: rows(By::Workspace),
    }
}

/// The totals of skill `skill`'s runs in the project, by what they ran on, by persona and by
/// workspace: none while the flag is off. On a blocking thread, as it reads the project's skill
/// log.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_stats(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    skill: String,
) -> Result<SkillStats, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the skill's runs", move || {
        if !purlis_core::skills::studio_on_here() {
            return Ok(SkillStats::default());
        }
        Ok(stats(held.root(), &skill))
    })
    .await
}

/// How many runs a skill's Runs panel lists at most.
const RUNS_SHOWN: usize = 200;

/// The runs of skill `skill` in the project, newest first, for its Runs panel: none while the
/// flag is off. On a blocking thread, as it reads the project's skill log.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_runs(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    skill: String,
) -> Result<Vec<StudioRun>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the skill's runs", move || {
        if !purlis_core::skills::studio_on_here() {
            return Ok(Vec::new());
        }
        Ok(runs(held.root(), &skill, RUNS_SHOWN))
    })
    .await
}

/// **Which panel on a skill's tab**, as the window holds it: purlis's Runs panel, or one an
/// extension adds, by its id and the panel's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SkillPanelKey {
    Grades,
    Runs,
    Extension { extension: String, panel: String },
}

/// The word [`SkillPanelKey::Runs`] is told to the window by, which it names that panel by.
pub(crate) const RUNS_PANEL: &str = "runs";

impl From<purlis_core::skills::studio::PanelKey> for SkillPanelKey {
    fn from(key: purlis_core::skills::studio::PanelKey) -> SkillPanelKey {
        use purlis_core::skills::studio::PanelKey;
        match key {
            PanelKey::Grades => SkillPanelKey::Grades,
            PanelKey::Runs => SkillPanelKey::Runs,
            PanelKey::Extension { extension, panel } => {
                SkillPanelKey::Extension { extension, panel }
            }
        }
    }
}

/// **One panel on a skill's tab** (ADR 0101 §6), as the window draws it: purlis's Grades and Runs
/// panels, then each panel an extension that declares `studio-panel` adds, each answered in panel blocks
/// or refused with the core's sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillPanel {
    pub key: SkillPanelKey,
    pub title: String,
    /// The extension that adds it, by name, or null for purlis's own.
    pub by: Option<String>,
    pub blocks: Vec<crate::panels::PanelBlock>,
    /// Why it has no blocks, in the core's words, or null where it answered.
    pub refused: Option<String>,
}

/// The panels on skill `skill`'s tab in the project at `root`, asked with `studio`, the Grades
/// panel of the copy at `level` (#3353).
pub(crate) fn panels(
    root: &std::path::Path,
    skill: &str,
    level: Option<&purlis_core::skills::verbs::Level>,
    studio: Option<purlis_core::skills::studio::Studio<'_>>,
) -> Vec<SkillPanel> {
    let answered = match studio {
        Some(studio) => studio.panels(root, skill, level),
        None => purlis_core::skills::studio::own_panels(root, skill, level),
    };
    answered
        .into_iter()
        .map(|panel| {
            let (blocks, refused) = match panel.blocks {
                Ok(blocks) => (
                    blocks.iter().map(crate::panels::PanelBlock::from).collect(),
                    None,
                ),
                Err(why) => (Vec::new(), Some(why)),
            };
            SkillPanel {
                key: panel.key.into(),
                title: panel.title,
                by: panel.by,
                blocks,
                refused,
            }
        })
        .collect()
}

/// **The panels on skill `skill`'s tab** (ADR 0101 §6): purlis's Grades panel, of the copy at
/// `level` the tab shows (null: the copy it shows first, #3353), and its Runs panel, which lists
/// every run of the skill, newest first, then each extension's. None while the flag is off. On a
/// blocking thread, as it reads the project's skill log and asks each extension.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_panels(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    skill: String,
    level: Option<crate::skillchange::SkillLevel>,
) -> Result<Vec<SkillPanel>, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("reading the skill's panels", move || {
        if !purlis_core::skills::studio_on_here() {
            return Ok(Vec::new());
        }
        Ok(purlis_core::skills::studio::with_studio(
            &executor,
            held.root(),
            |studio| {
                let level = level.as_ref().map(purlis_core::skills::verbs::Level::from);
                panels(held.root(), &skill, level.as_ref(), studio)
            },
        ))
    })
    .await
}

/// Whether the Skill studio is on in this window: this machine's flag, or the variable over it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_studio_on() -> Result<bool, String> {
    crate::off_the_window("reading this machine's flags", || {
        Ok(purlis_core::skills::studio_on_here())
    })
    .await
}

/// **Turns the Skill studio on or off on this machine** (Settings › You › Previews), then says
/// whether it is on, which `PURLIS_SKILL_STUDIO` still decides where it is set.
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_skill_studio_on(
    planes: tauri::State<'_, crate::planes::Planes>,
    on: bool,
) -> Result<bool, String> {
    let held = planes.every_held();
    crate::off_the_window("turning the Skill studio preview on or off", move || {
        let root = purlis_core::machine::config_root()
            .ok_or_else(|| "There is no config home on this machine to keep it in.".to_owned())?;
        purlis_core::skills::set_studio_on(&root, on)?;
        // The old plugin keys' Ask is the Inbox's only while the studio is off (#3689).
        for one in &held {
            one.chats().asks_may_have_moved();
        }
        Ok(purlis_core::skills::studio_on_here())
    })
    .await
}

/// The Skill studio of the project, for a chat at its root or in `workspace`, as `persona` or on
/// none: every skill with its level, and why each left out is left out. On a blocking thread, as
/// it reads the project's skill folders.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_studio(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
    persona: Option<String>,
) -> Result<Studio, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the project's skills", move || {
        let built_in =
            crate::bundled_plugin_here().and_then(|plugin| purlis_core::skills::in_bundle(&plugin));
        let mut studio = studio(
            held.root(),
            built_in.as_deref(),
            purlis_core::skills::studio_on_here(),
            workspace.as_deref(),
            persona.as_deref(),
            &purlis_core::harness_plugin::Env::of(&[]),
        );
        with_the_projects_needs(
            &mut studio,
            held.root(),
            &held.proposals().paths_of(crate::proposals::BY_PURLIS),
        );
        // How the router did at the start of each chat open here in the studio's scope (SK-15b).
        if let Some(router) = studio.router.as_mut() {
            router.chats = held
                .chats()
                .routings()
                .into_iter()
                .filter(|one| one.workspace == workspace && one.persona == persona)
                .map(|one| StudioRouted::of(one.session, &one.routed))
                .collect();
        }
        Ok(studio)
    })
    .await
}

/// **Every persona's own skills** (#3266): what the Skills view lists under Personas, for a chat
/// in `workspace` or at the root, in one read rather than one studio per persona. On a blocking
/// thread, as it reads the project's skill folders.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_persona_skills(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
) -> Result<Vec<PersonaSkills>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the personas' skills", move || {
        let built_in =
            crate::bundled_plugin_here().and_then(|plugin| purlis_core::skills::in_bundle(&plugin));
        Ok(persona_skills(
            held.root(),
            built_in.as_deref(),
            purlis_core::skills::studio_on_here(),
            workspace.as_deref(),
        ))
    })
    .await
}

/// **Share** (#2767): moves the local skill `name` of `whose` (the project's root, a workspace
/// or a persona) into that level's `skills/`, committed (#2770), or says why not.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_share(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    whose: crate::skillchange::SkillWhose,
    name: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("sharing the skill", move || {
        if !purlis_core::skills::studio_on_here() {
            return Err("The Skill studio is not turned on on this machine.".to_owned());
        }
        crate::skillchange::share(held.root(), &(&whose).into(), &name)
    })
    .await
}

/// What a change to a persona's skills does, as the studio asks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum PersonaSkillChange {
    Add,
    Remove,
}

impl From<PersonaSkillChange> for purlis_core::skills::persona::Change {
    fn from(change: PersonaSkillChange) -> Self {
        match change {
            PersonaSkillChange::Add => Self::Add,
            PersonaSkillChange::Remove => Self::Remove,
        }
    }
}

/// **Adds a skill to a persona's skills, or takes one out** (SK-5, #2768): Add and Remove in the
/// Skill studio filtered to the persona. `written` is the skill as the studio's row writes it.
/// Answers whether the persona's definition changed. Refused while the studio's flag is off.
#[tauri::command]
#[specta::specta]
pub(crate) async fn persona_skill(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    persona: String,
    written: String,
    change: PersonaSkillChange,
) -> Result<bool, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("changing the persona's skills", move || {
        if !purlis_core::skills::studio_on_here() {
            return Err("The Skill studio is off on this machine.".to_owned());
        }
        let changed =
            purlis_core::skills::persona::change(held.root(), &persona, &written, change.into())?;
        if changed {
            held.wrote(&["personas".to_owned()]);
        }
        Ok(changed)
    })
    .await
}

/// **The skills a chat's persona names that it started without** (SK-5, #2768): what its tab's
/// Notice says, until it is put away. `None` for a chat that has every one.
#[tauri::command]
#[specta::specta]
pub(crate) async fn chat_skills_missing(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
) -> Result<Option<crate::chats::SkillsMissing>, String> {
    let held = planes.held(&plane)?;
    Ok(held.chats().skills_missing(session))
}

/// Puts a chat's missing-skills Notice away.
#[tauri::command]
#[specta::specta]
pub(crate) async fn put_away_chat_skills_missing(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    held.chats().put_away_skills_missing(session);
    Ok(())
}

/// **Turns one of a harness's own skills on or off** (SK-6, #2769) for the chats the studio
/// shows (a plugin's is the plugin's, and refused, #3467): at the project's root on this machine alone (`charter.local.toml`), in a
/// workspace in its own settings, as `purlis_core::harness_plugin::switch_in_the_studio` writes
/// it. A chat started after it is handed the change.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_studio_switch(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: Option<String>,
    harness: HarnessKind,
    id: String,
    on: bool,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("turning it on or off", move || {
        if !purlis_core::skills::studio_on_here() {
            return Err("The Skill studio is not turned on on this machine.".to_owned());
        }
        let built_in = purlis_core::skills::built_in_names(
            crate::bundled_plugin_here()
                .and_then(|plugin| purlis_core::skills::in_bundle(&plugin))
                .as_deref(),
        );
        purlis_core::harness_plugin::switch_in_the_studio(
            held.root(),
            workspace.as_deref(),
            harness.into(),
            &id,
            on,
            &built_in,
        )
    })
    .await
}

#[cfg(test)]
mod tests;
