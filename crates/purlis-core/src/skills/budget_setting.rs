// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`settings.skills.budget`** (#4610, #5353; ADR 0102 as amended 2026-11-02 and 2026-11-05): the
//! skill budget a project or a workspace writes over its harnesses' numbers, for every program
//! and for one.
//!
//! ```toml
//! [skills.budget]              # every program
//! skills = 80
//! [skills.budget.claude-code]  # this program, over that
//! skills = 100
//! ```
//!
//! The project's is `[skills.budget]` in `charter.toml`, and a workspace's
//! `settings.skills.budget` in its `workspace.json`, which wins for each number it writes,
//! whichever way it writes it. A number not written stays the adapter's, on each harness.
//!
//! **A value past a harness's own limit is said, never refused** ([`warnings`], [`preview`]). Past
//! the budget the router still hides what it may, and pins stay the way to keep one skill listed.
//! Settings keeps the numbers: the Routing page's Change… and Settings' row write them here
//! ([`write`]), and a chat started after takes them.

use std::path::Path;

use purlis_skills::budget::Set;

use crate::harness::Harness;
use crate::settings::workspace::{KEY, SKILLS};
use crate::settings::{Edit, Refusal, Step, Value, Which, layer_text};

/// The key in `settings.skills`, and in `charter.toml`'s `[skills]`, that holds the budget.
pub const BUDGET: &str = "budget";
/// The key in [`BUDGET`] that holds the most skills a chat lists.
pub const SKILLS_KEY: &str = "skills";
/// The key in [`BUDGET`] that holds the most characters of what is listed of them.
pub const CHARS_KEY: &str = "chars";

/// **About how many characters of a listing cost a token**: 91 tokens for a 250-character
/// description and its 23 characters of line, measured on Haiku 5.5 (ADR 0102 as amended
/// 2026-11-02). Real prose may differ, so a person is only ever told "about".
const CHARS_PER_TOKEN: i64 = 3;

/// **What `value` writes** as a whole number of at least 1, leniently: any other writes nothing,
/// so the number below it stands, and a save would have refused it ([`refused`]).
fn set_of(value: Option<&toml::Value>) -> Set {
    let read = |key: &str| {
        value
            .and_then(|value| value.get(key))
            .and_then(toml::Value::as_integer)
            .filter(|number| *number >= 1)
            .and_then(|number| usize::try_from(number).ok())
    };
    Set {
        skills: read(SKILLS_KEY),
        chars: read(CHARS_KEY),
    }
}

/// `over`, with each number `set` writes put in its place.
fn laid_over(over: Set, set: Set) -> Set {
    Set {
        skills: set.skills.or(over.skills),
        chars: set.chars.or(over.chars),
    }
}

/// **What one level writes**: its numbers for every program, and its own for each.
#[derive(Debug, Clone, Default)]
struct Levels {
    every: Set,
    own: Vec<(Harness, Set)>,
}

impl Levels {
    fn of(value: Option<&toml::Value>) -> Self {
        Self {
            every: set_of(value),
            own: Harness::ALL
                .into_iter()
                .map(|harness| (harness, set_of(value.and_then(|v| v.get(harness.name())))))
                .collect(),
        }
    }

    fn own(&self, harness: Harness) -> Set {
        self.own
            .iter()
            .find(|(one, _)| *one == harness)
            .map_or_else(Set::default, |(_, set)| *set)
    }

    /// What it comes to on `harness`: its own number over the one for every program.
    fn on(&self, harness: Harness) -> Set {
        laid_over(self.every, self.own(harness))
    }
}

/// What the project at `root` writes.
fn the_projects(root: &Path) -> Levels {
    let text = layer_text(root, Which::Shared);
    let shared = text
        .text()
        .and_then(|text| text.parse::<toml::Table>().ok());
    Levels::of(
        shared
            .as_ref()
            .and_then(|top| top.get(SKILLS))
            .and_then(|table| table.get(BUDGET)),
    )
}

/// What the workspace `workspace` of the project at `root` writes.
fn the_workspaces(root: &Path, workspace: &str) -> Levels {
    let table = crate::settings::workspace::read(root, workspace);
    Levels::of(
        table
            .as_ref()
            .and_then(|top| top.get(SKILLS))
            .and_then(|table| table.get(BUDGET)),
    )
}

/// **What the plane at `root` writes for a chat in `workspace` on `harness`**: the project's, then
/// the workspace's over it, number by number.
fn set_in(root: &Path, workspace: Option<&str>, harness: Harness) -> Set {
    let project = the_projects(root).on(harness);
    workspace.map_or(project, |name| {
        laid_over(project, the_workspaces(root, name).on(harness))
    })
}

/// **The budget in force on `harness` for a chat in `workspace`** of the plane at `root`: the
/// adapter's numbers, with what the plane writes over them.
pub fn budget_in(root: &Path, workspace: Option<&str>, harness: Harness) -> purlis_skills::Budget {
    harness
        .skill_budget()
        .with(set_in(root, workspace, harness))
}

/// **What each harness would drop at what it is given in `workspace`**, one sentence each, in
/// harness order: what a Settings row and the Routing page say beside the numbers. Nothing where
/// nothing is written, since the adapters' own numbers sit under their limits. Never a refusal.
pub fn warnings(root: &Path, workspace: Option<&str>) -> Vec<String> {
    Harness::ALL
        .iter()
        .filter_map(|harness| harness.skill_limit_past(budget_in(root, workspace, *harness)))
        .collect()
}

/// **Who a number in force is written by.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    Workspace,
    Project,
    Default,
}

/// **One number in force on a program, and where it comes from** (#5353).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Number {
    pub in_force: usize,
    pub from: Written,
    /// What the level asked about writes for this program itself or for every program: what its
    /// Reset to default takes out, and what Change… starts from. `None` where it writes nothing.
    pub here: Option<usize>,
    /// What the project says, where it says and is not the level asked about.
    pub project: Option<usize>,
    /// The adapter's own.
    pub default: usize,
    /// Whether the number in force is one written for every program.
    pub on_every_program: bool,
}

impl Number {
    /// **The number in a line**: `100, this workspace · project says 60 · Claude Code's default
    /// is 60`, for a chat in `workspace` on `harness`.
    pub fn said(&self, workspace: Option<&str>, harness: Harness) -> String {
        let number = crate::skills::grouped(self.in_force);
        let program = harness.title();
        let every = if self.on_every_program {
            ", on every program"
        } else {
            ""
        };
        let default = format!(
            "{program}'s default is {}",
            crate::skills::grouped(self.default)
        );
        match self.from {
            Written::Default => format!("{number}, {program}'s default"),
            Written::Workspace => {
                let project = self
                    .project
                    .map(|said| format!(" · project says {}", crate::skills::grouped(said)))
                    .unwrap_or_default();
                format!("{number}, this workspace{every}{project} · {default}")
            }
            Written::Project => {
                let whose = if workspace.is_some() {
                    "the project"
                } else {
                    "this project"
                };
                format!("{number}, {whose}{every} · {default}")
            }
        }
    }
}

/// **A program's two numbers in force, each with where it comes from.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InForce {
    pub skills: Number,
    pub chars: Number,
}

/// **What a chat in `workspace` of the plane at `root` gets on `harness`, and from whom** (#5353).
/// `workspace` is also the level asked about: `None` is the project's.
pub fn in_force(root: &Path, workspace: Option<&str>, harness: Harness) -> InForce {
    let project = the_projects(root);
    let here = workspace.map(|name| the_workspaces(root, name));
    let default = harness.skill_budget();
    let number = |pick: fn(Set) -> Option<usize>, default: usize| {
        let at_project = pick(project.on(harness));
        let at_workspace = here.as_ref().and_then(|written| pick(written.on(harness)));
        let (in_force, from, whose_every) = match (at_workspace, at_project) {
            (Some(number), _) => (
                number,
                Written::Workspace,
                here.as_ref()
                    .is_some_and(|written| pick(written.own(harness)).is_none()),
            ),
            (None, Some(number)) => (
                number,
                Written::Project,
                pick(project.own(harness)).is_none(),
            ),
            (None, None) => (default, Written::Default, false),
        };
        Number {
            in_force,
            from,
            here: if workspace.is_some() {
                at_workspace
            } else {
                at_project
            },
            project: workspace.and(at_project),
            default,
            on_every_program: whose_every,
        }
    };
    InForce {
        skills: number(|set| set.skills, default.skills),
        chars: number(|set| set.chars, default.chars),
    }
}

/// **The level a window's Change… writes**: never a persona's or a person's (#5353).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level<'a> {
    Project,
    Workspace(&'a str),
}

impl<'a> Level<'a> {
    /// The level of a chat in `workspace`.
    pub fn of(workspace: Option<&'a str>) -> Self {
        workspace.map_or(Level::Project, Level::Workspace)
    }
}

/// A key under the level's budget.
fn at(keys: &[&str]) -> Vec<Step> {
    let mut path = vec![Step::Key(SKILLS.to_owned()), Step::Key(BUDGET.to_owned())];
    path.extend(keys.iter().map(|key| Step::Key((*key).to_owned())));
    path
}

/// A number as the file writes it.
fn integer(number: usize) -> Result<Value, String> {
    i64::try_from(number)
        .map(Value::Integer)
        .map_err(|_| format!("{number} is too large to write down, so nothing changed"))
}

/// **What makes `harness` come to `wanted` at a level that writes `written`**, one key at a time.
/// A number written for every program stops being one where `harness` leaves it: each other
/// program that had it gets it as its own, so nothing moves for them.
fn edits_for(written: &Levels, harness: Harness, wanted: Set) -> Result<Vec<Edit>, String> {
    let mut edits = Vec::new();
    let keys: [(&str, fn(Set) -> Option<usize>); 2] =
        [(SKILLS_KEY, |set| set.skills), (CHARS_KEY, |set| set.chars)];
    for (key, pick) in keys {
        let number = pick(wanted);
        if number == pick(written.on(harness)) {
            continue;
        }
        if let Some(every) = pick(written.every) {
            edits.push(Edit {
                path: at(&[key]),
                value: None,
            });
            for other in Harness::ALL.into_iter().filter(|one| *one != harness) {
                if pick(written.own(other)).is_none() {
                    edits.push(Edit {
                        path: at(&[other.name(), key]),
                        value: Some(integer(every)?),
                    });
                }
            }
        }
        edits.push(Edit {
            path: at(&[harness.name(), key]),
            value: number.map(integer).transpose()?,
        });
    }
    Ok(edits)
}

/// **`text` with the budget's tables it left empty taken out**: a program's, then the budget, then
/// `[skills]`, so a reset leaves the project's file as it was found. A workspace's save does this
/// itself.
fn without_empty_tables(text: &str) -> Result<String, String> {
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| crate::shown::short(e.message()))?;
    let empty = |item: Option<&toml_edit::Item>| {
        item.and_then(toml_edit::Item::as_table_like)
            .is_some_and(|table| table.is_empty())
    };
    if let Some(skills) = doc
        .get_mut(SKILLS)
        .and_then(toml_edit::Item::as_table_like_mut)
    {
        if let Some(budget) = skills
            .get_mut(BUDGET)
            .and_then(toml_edit::Item::as_table_like_mut)
        {
            for harness in Harness::ALL {
                if empty(budget.get(harness.name())) {
                    budget.remove(harness.name());
                }
            }
        }
        if empty(skills.get(BUDGET)) {
            skills.remove(BUDGET);
        }
    }
    if empty(doc.get(SKILLS)) {
        doc.remove(SKILLS);
    }
    Ok(doc.to_string())
}

/// **Writes what `level` says for `harness`**, each number in `wanted` or, for none, taken out so
/// the number below it stands (an empty box; Reset to default). `Ok(false)` where it already says
/// that, and nothing is written. A number below 1 is refused; a large one never is. A running chat
/// keeps what it was handed: the new number reaches chats started after this.
pub fn write(root: &Path, level: Level<'_>, harness: Harness, wanted: Set) -> Result<bool, String> {
    crate::settings::writable(root)?;
    if wanted.skills == Some(0) || wanted.chars == Some(0) {
        return Err(
            "A skill budget is a whole number of at least 1, so nothing changed.".to_owned(),
        );
    }
    match level {
        Level::Project => {
            let read = crate::settings::read(root, Which::Shared)?;
            let edits = edits_for(&the_projects(root), harness, wanted)?;
            if edits.is_empty() {
                return Ok(false);
            }
            let text = without_empty_tables(&crate::settings::edited(&read.text, &edits)?)?;
            let base = read.exists.then_some(read.text.as_str());
            crate::settings::save_edit(root, Which::Shared, base, &text)
                .map_err(|why| why.join(" "))?;
        }
        Level::Workspace(workspace) => {
            let read = crate::settings::workspace::read_file(root, workspace)?;
            let edits = edits_for(&the_workspaces(root, workspace), harness, wanted)?;
            if edits.is_empty() {
                return Ok(false);
            }
            let base = read.exists.then_some(read.text.as_str());
            crate::settings::workspace::save(root, workspace, base, &edits)
                .map(|_| ())
                .map_err(|why| why.join(" "))?;
        }
    }
    Ok(true)
}

/// **What a typed number would do to new chats** (#5353): what the Change… dialog says as it is
/// typed, for a chat at the root of `workspace` (or the project), as `persona` where the studio is
/// filtered to one, in no clone, by the built-in router: the same chat the Routing page's preview
/// is of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// Skills such a chat lists at the numbers typed.
    pub listed: usize,
    /// Skills it would hide at them.
    pub hidden: usize,
    /// By name, those it hides now and would list.
    pub now_listed: Vec<String>,
    /// By name, those it lists now and would hide.
    pub now_hidden: Vec<String>,
    /// About how many more tokens each new chat spends on its listing, or fewer below zero.
    pub more_tokens: i64,
    /// What the program would drop, past a limit of its own purlis cannot move.
    pub past_limit: Option<String>,
    /// What stays listed past the numbers typed, where something does: the pins, or what no pin
    /// decides.
    pub over: Option<String>,
    /// The numbers it works out at, each typed or else the one below it.
    pub skills: usize,
    pub chars: usize,
}

/// [`Preview`] of `wanted`, what the level of `workspace` would say for `harness`, each number
/// typed or, for none, the one below that level.
pub fn preview(
    built_in: Option<&Path>,
    root: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    harness: Harness,
    wanted: Set,
) -> Preview {
    use purlis_skills::StillOver;
    let below = if workspace.is_some() {
        the_projects(root).on(harness)
    } else {
        Set::default()
    };
    let now_budget = budget_in(root, workspace, harness);
    let then_budget = harness.skill_budget().with(laid_over(below, wanted));
    let at = super::PlaneSkills {
        plane: root,
        workspace,
        repo: &[],
        routed: None,
    };
    let set = super::ask(built_in, Some(at), persona, harness.skills(), None, None);
    let pins = super::pins_or_none(root, workspace, persona);
    let now = set.clone().routed(now_budget, &pins);
    let then = set.routed(then_budget, &pins);
    // What the budget counts of a listing: not a manual-only skill, nor a harness's own.
    let counted = |set: &purlis_skills::SkillSet| -> Vec<String> {
        set.listed()
            .filter(|e| e.origin != purlis_skills::Origin::Harness)
            .filter(|e| !purlis_skills::declared(&e.skill).manual)
            .map(|e| e.skill.name.clone())
            .collect()
    };
    let (listed_now, listed_then) = (counted(&now), counted(&then));
    let more_chars = i64::try_from(then.spends(then_budget)).unwrap_or(i64::MAX)
        - i64::try_from(now.spends(now_budget)).unwrap_or(i64::MAX);
    let routing = then.routing.as_ref();
    let over = routing.and_then(|routing| match routing.still_over? {
        StillOver::Pins => Some(format!(
            "{} are still listed, so a chat's list goes over this.",
            crate::counts::counted(routing.pinned.len(), "pinned skill", "pinned skills"),
        )),
        StillOver::NotHideable => Some(
            "purlis's own skills and the ones a program and its repos bring are still listed, \
             so a chat's list goes over this."
                .to_owned(),
        ),
    });
    Preview {
        listed: listed_then.len(),
        hidden: routing.map_or(0, |routing| routing.hidden.len()),
        now_listed: listed_then
            .iter()
            .filter(|name| !listed_now.contains(name))
            .cloned()
            .collect(),
        now_hidden: listed_now
            .iter()
            .filter(|name| !listed_then.contains(name))
            .cloned()
            .collect(),
        more_tokens: (more_chars as f64 / CHARS_PER_TOKEN as f64).round() as i64,
        past_limit: harness.skill_limit_past(then_budget),
        over,
        skills: then_budget.skills,
        chars: then_budget.chars,
    }
}

/// What `value`, the budget at `shown`, holds that [`set_of`] would not read, each at its key
/// under `at`.
fn refused(value: &toml::Value, shown: &str, file: &str, at: &[&str]) -> Vec<Refusal> {
    refused_in(value, shown, file, at, true)
}

/// [`refused`], of the budget itself (`top`, which may also hold a table per program) or of one
/// program's table.
fn refused_in(
    value: &toml::Value,
    shown: &str,
    file: &str,
    at: &[&str],
    top: bool,
) -> Vec<Refusal> {
    let shape = format!(
        "a budget is {{\"{SKILLS_KEY}\": 60, \"{CHARS_KEY}\": 15000}}, with any program's own \
         as {{\"{}\": {{\"{SKILLS_KEY}\": 100}}}}",
        Harness::ClaudeCode.name()
    );
    let Some(table) = value.as_table() else {
        return vec![Refusal::at(
            format!("{shown} in {file} is not a table, so it changes nothing — {shape}"),
            at,
        )];
    };
    table
        .iter()
        .flat_map(|(key, value)| {
            let mut place = at.to_vec();
            place.push(key);
            if top && Harness::of_kind(key).is_some() {
                return refused_in(value, &format!("{shown}.{key}"), file, &place, false);
            }
            let why = if key != SKILLS_KEY && key != CHARS_KEY {
                format!("{shown}.{key} in {file} is not read — {shape}")
            } else if value.as_integer().is_some_and(|number| number >= 1) {
                return Vec::new();
            } else {
                format!(
                    "{shown}.{key} in {file} is not a whole number of at least 1, so the \
                     number below it stands — {shape}"
                )
            };
            vec![Refusal::at(why, &place)]
        })
        .collect()
}

/// What a workspace's `settings.skills.budget` holds that its reader would not read.
pub fn refused_in_workspace(value: &toml::Value, file: &str) -> Vec<Refusal> {
    refused(
        value,
        &format!("{KEY}.{SKILLS}.{BUDGET}"),
        file,
        &[SKILLS, BUDGET],
    )
}

/// **What `text`, the settings file `which` of the project at `root`, says wrong of `[skills]`**:
/// the Shared file holds `budget` alone, and the Local file no `[skills]`, since a budget is the
/// project's to agree on.
pub fn keyed_at(root: &Path, text: &str, which: Which) -> Vec<Refusal> {
    let file = which.file_at(root);
    let Ok(top) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let Some(value) = top.get(SKILLS) else {
        return Vec::new();
    };
    if which != Which::Shared {
        return vec![Refusal::at(
            format!(
                "[{SKILLS}] in {file} is not read: a project's skill budget is set in {}",
                Which::Shared.file_at(root)
            ),
            &[SKILLS],
        )];
    }
    let Some(table) = value.as_table() else {
        return vec![Refusal::at(
            format!("[{SKILLS}] in {file} is not a table"),
            &[SKILLS],
        )];
    };
    table
        .iter()
        .flat_map(|(key, value)| {
            if key == BUDGET {
                refused(
                    value,
                    &format!("[{SKILLS}] {BUDGET}"),
                    file,
                    &[SKILLS, BUDGET],
                )
            } else {
                vec![Refusal::at(
                    format!(
                        "[{SKILLS}] {} in {file} is not read: a project's [{SKILLS}] holds \
                         {BUDGET} alone",
                        crate::shown::short(key)
                    ),
                    &[SKILLS, key],
                )]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
