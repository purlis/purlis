// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Skill studio's seams, each with purlis's own first** (ADR 0101 §1, #2777): the
//! templates a new skill starts from, the lints of a skill's files, the panels on a skill's tab
//! and the skill sources, each one interface that purlis's built-ins answer and each extension
//! that declares the studio capability answers too.
//!
//! - **Templates** ([`Templates`]): purlis's own ([`purlis_skills::change::BuiltInTemplates`]),
//!   then each extension's that declares `skill-template`, by `<extension>/<template>`.
//! - **Lints** ([`Studio::lints`]): each extension's that declares `skill-lint`, beside purlis's
//!   own ([`purlis_skills::BuiltInLints`]), through [`purlis_skills::SkillLint`]. A third
//!   party's finding warns and never stops a save.
//! - **Panels** ([`Studio::panels`]): purlis's own Runs panel ([`RunsPanel`]), then each panel an
//!   extension that declares `studio-panel` adds, each answered in panel blocks.
//! - **Sources**: the git source and each extension that declares `skill-source`, through
//!   [`super::source::SkillSource`], whose review stays purlis's
//!   ([`super::source::extension`]).
//!
//! An extension is asked only where this machine approved it and the project has it on, through
//! the executor's gate and inside its sandbox ([`crate::extension::studio`]).

use std::path::Path;

use purlis_skills::change::{BuiltInTemplates, Offer, SkillTemplate, TemplateId};
use purlis_skills::{Finding, Lint, Severity, SkillFiles, SkillLint};

use crate::executor::Executor;
use crate::extension::capability::Capability;
use crate::extension::project::Choices;
use crate::extension::studio::{self as wire, Ask, HandedFile, SkillFile};
use crate::panel::{Block, Empty, Table, TableRow, Tone};

use super::verbs::Level;

/// What the studio asks extensions with: the app's executor, this machine's record of
/// extensions, and the project the question is asked in.
#[derive(Clone, Copy)]
pub struct Studio<'a> {
    pub executor: &'a Executor,
    pub config_root: &'a Path,
    pub project: &'a Choices,
}

impl std::fmt::Debug for Studio<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Studio")
            .field("config_root", &self.config_root)
            .finish_non_exhaustive()
    }
}

/// One extension the studio asks, by its id and name, and what its studio capabilities declare.
#[derive(Debug, Clone)]
pub struct Asked {
    pub id: String,
    pub name: String,
    pub declared: wire::Declared,
}

/// **What the studio asks extensions with in the project at `root`, handed to `with`**: the
/// executor, this machine's record of extensions and the project's choices, read once here for
/// the window and the command line alike. With no config home, `None`: purlis's own alone.
pub fn with_studio<T>(
    executor: &Executor,
    root: &Path,
    with: impl FnOnce(Option<Studio<'_>>) -> T,
) -> T {
    let config = crate::machine::config_root();
    let project = Choices::read(root);
    with(config.as_deref().map(|config_root| Studio {
        executor,
        config_root,
        project: &project,
    }))
}

impl<'a> Studio<'a> {
    /// **Each extension this machine approved, the project has on, and that declares
    /// `capability`**, in id order, by its manifest as it is on disk now. The fingerprint is
    /// re-taken by the executor before any program starts, as for every question.
    pub fn declaring(&self, capability: Capability) -> Vec<Asked> {
        let loaded = crate::extension::read(self.config_root, self.executor.built_in());
        if loaded.unreadable.is_some() {
            return Vec::new();
        }
        loaded
            .registry
            .entries
            .iter()
            .filter(|(_, entry)| entry.in_force())
            .filter_map(|(id, entry)| {
                let manifest = crate::extension::manifest_at(&entry.path).ok()?;
                let declared = manifest.studio.clone()?;
                (manifest.capabilities.contains(&capability)
                    && crate::extension::facts::on_here(id, &manifest, self.project))
                .then(|| Asked {
                    id: id.clone(),
                    name: manifest.name.clone(),
                    declared,
                })
            })
            .collect()
    }

    /// Ask `extension` the studio question `ask`.
    pub fn ask(
        &self,
        extension: &str,
        ask: &Ask<'_>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        self.executor
            .studio(self.config_root, self.project, extension, ask)
    }

    /// **The lints of each extension that declares `skill-lint`**, to ask beside purlis's own.
    pub fn lints(&self) -> Vec<ExtensionLint<'a>> {
        self.declaring(Capability::SkillLint)
            .into_iter()
            .map(|asked| ExtensionLint {
                studio: *self,
                by: asked.declared.lint.clone().unwrap_or(asked.name),
                id: asked.id,
            })
            .collect()
    }

    /// **The panels on `skill`'s tab**, in the project at `plane`: purlis's Grades and Runs panels
    /// first, then each extension's, each answered or refused with a sentence. The Grades panel
    /// is of the copy at `level` ([`GradesPanel`]).
    pub fn panels(&self, plane: &Path, skill: &str, level: Option<&Level>) -> Vec<Answered> {
        let mut out = own_panels(plane, skill, level);
        let asked = self.declaring(Capability::StudioPanel);
        if asked.is_empty() {
            return out;
        }
        let files = skill_files(plane, skill);
        for one in asked {
            for panel in &one.declared.panels {
                out.push(Answered::of(
                    &ExtensionPanel {
                        studio: *self,
                        id: &one.id,
                        name: &one.name,
                        panel,
                        files: &files,
                    },
                    skill,
                ));
            }
        }
        out
    }
}

/// **purlis's own panels on `skill`'s tab**, in the project at `plane`: its Grades panel, of the
/// copy at `level`, then its Runs panel. What every skill's tab draws first, with a studio or
/// without one.
pub fn own_panels(plane: &Path, skill: &str, level: Option<&Level>) -> Vec<Answered> {
    vec![
        Answered::of(&GradesPanel { plane, level }, skill),
        Answered::of(&RunsPanel { plane }, skill),
    ]
}

// ---------------------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------------------

/// **Every template a new skill can start from**: purlis's own, then each extension's that
/// declares `skill-template` ([`Studio::declaring`]), each by its extension and its own id
/// ([`TemplateId::Extension`]). With no studio, purlis's own alone.
#[derive(Debug, Clone, Copy)]
pub struct Templates<'a> {
    pub studio: Option<Studio<'a>>,
}

impl SkillTemplate for Templates<'_> {
    fn offered(&self) -> Vec<Offer> {
        let mut out = BuiltInTemplates.offered();
        let Some(studio) = self.studio else {
            return out;
        };
        for asked in studio.declaring(Capability::SkillTemplate) {
            for template in &asked.declared.templates {
                out.push(Offer {
                    id: TemplateId::Extension {
                        extension: asked.id.clone(),
                        template: template.id.clone(),
                    },
                    label: format!("{} · {}", template.title, asked.name),
                    about: template.about.clone(),
                });
            }
        }
        out
    }

    fn text(&self, id: &TemplateId, name: &str, description: &str) -> Result<String, String> {
        let TemplateId::Extension {
            extension,
            template,
        } = id
        else {
            return BuiltInTemplates.text(id, name, description);
        };
        let studio = self
            .studio
            .ok_or_else(|| format!("purlis has no template of the extension '{extension}' here"))?;
        let doc = studio.ask(
            extension,
            &Ask::Template {
                template,
                name,
                description,
            },
        )?;
        wire::template_of(extension, name, &doc)
    }
}

// ---------------------------------------------------------------------------------------
// Lints
// ---------------------------------------------------------------------------------------

/// **An extension's lint**, through the interface purlis's own answers by: handed the skill's
/// text files in the question, answered in lines purlis shows as warnings under its name.
#[derive(Debug, Clone)]
pub struct ExtensionLint<'a> {
    studio: Studio<'a>,
    id: String,
    /// What a finding is said under: its declared title, or the extension's name.
    by: String,
}

impl SkillLint for ExtensionLint<'_> {
    fn findings(&self, skill: &SkillFiles<'_>) -> Vec<Finding> {
        let mut files = vec![HandedFile {
            path: purlis_skills::FILE.to_owned(),
            text: skill.text.to_owned(),
            executable: false,
        }];
        let others: Vec<SkillFile> = skill
            .others
            .iter()
            .map(|(path, text)| SkillFile {
                path: path.clone(),
                bytes: text.clone().into_bytes(),
                executable: false,
            })
            .collect();
        files.extend(wire::handed(&others));
        let said = self
            .studio
            .ask(
                &self.id,
                &Ask::Findings {
                    skill: skill.name,
                    files: &files,
                },
            )
            .and_then(|doc| wire::findings_of(&self.id, &doc));
        let says = match said {
            Ok(says) => says,
            Err(why) => vec![format!("could not lint this skill: {why}")],
        };
        says.into_iter()
            .map(|says| Finding {
                severity: Severity::Warns,
                lint: Lint::Said {
                    by: self.by.clone(),
                    says,
                },
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------
// Panels
// ---------------------------------------------------------------------------------------

/// **A panel on a skill's tab** (the StudioPanel capability, ADR 0101 §6): its key, its title,
/// who adds it, and its blocks for a skill. purlis's Runs panel ([`RunsPanel`]) and each
/// extension's answer through it.
pub trait StudioPanel {
    /// What the window holds it by: unique across every contributor.
    fn key(&self) -> PanelKey;
    fn title(&self) -> String;
    /// The extension that adds it, by name, or `None` for purlis's own.
    fn by(&self) -> Option<String>;
    /// Its blocks for the skill `skill`, or why there are none.
    fn blocks(&self, skill: &str) -> Result<Vec<Block>, String>;
}

/// **Which panel on a skill's tab**: purlis's Runs panel, or one an extension adds, by the
/// extension's id and the panel's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelKey {
    Grades,
    Runs,
    Extension { extension: String, panel: String },
}

/// One panel on a skill's tab, answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    pub key: PanelKey,
    pub title: String,
    pub by: Option<String>,
    /// Its blocks, or the sentence saying why it has none.
    pub blocks: Result<Vec<Block>, String>,
}

impl Answered {
    /// `panel`'s answer for the skill `skill`.
    pub fn of(panel: &dyn StudioPanel, skill: &str) -> Self {
        Self {
            key: panel.key(),
            title: panel.title(),
            by: panel.by(),
            blocks: panel.blocks(skill),
        }
    }
}

/// **purlis's Grades panel** (SK-16, #2779), the built-in runner's scorecard: every grade of one
/// copy of the skill the eval record holds, newest first, with the version it graded, what its
/// chats ran on, each arm's pass rate and the difference the skill made. A pass or a fail comes
/// from here alone, never from a run (decision 24).
///
/// **One copy's grades** (#3353, #3275): same-named copies at two levels are two skills, so the
/// panel reads the rows of the copy at `level` alone, or, with none, of the copy the skill's tab
/// shows first (the first of [`Level::copies_of`]).
#[derive(Debug, Clone, Copy)]
pub struct GradesPanel<'a> {
    pub plane: &'a Path,
    pub level: Option<&'a Level>,
}

impl StudioPanel for GradesPanel<'_> {
    fn key(&self) -> PanelKey {
        PanelKey::Grades
    }

    fn title(&self) -> String {
        "Grades".to_owned()
    }

    fn by(&self) -> Option<String> {
        None
    }

    fn blocks(&self, skill: &str) -> Result<Vec<Block>, String> {
        let first = match self.level {
            Some(_) => None,
            None => Level::copies_of(self.plane, skill)
                .next()
                .map(|(level, _)| level),
        };
        let Some(level) = self.level.or(first.as_ref()) else {
            return Ok(grades_blocks(&[], skill));
        };
        let read = super::grades::read(self.plane, skill, Some(level));
        let mut blocks = grades_blocks(&read.grades, skill);
        // What the record left out is said, never dropped unseen (#3273).
        if let Some(said) = read.said() {
            blocks.push(Block::Note {
                text: said,
                tone: Tone::Trouble,
            });
        }
        Ok(blocks)
    }
}

/// A share as a person reads it: `67%`.
fn share_said(share: f64) -> String {
    format!("{:.0}%", share * 100.0)
}

/// An arm's pass rate as its cell says it: the mean, and the spread over the runs where there is
/// one (`67% ± 12`).
fn arm_said(graded: &super::grades::Graded, arm: purlis_skills::grade::Arm) -> String {
    match graded.report.arm(arm).and_then(|score| score.pass_rate) {
        None => "Not run".to_owned(),
        Some(stat) if stat.stddev > 0.0 => {
            format!("{} ± {:.0}", share_said(stat.mean), stat.stddev * 100.0)
        }
        Some(stat) => share_said(stat.mean),
    }
}

/// **The version a grade graded, as its row and its scorecard say it**: the commit, shortened,
/// or that it held changes not saved.
pub fn version_said(graded: &super::grades::Graded) -> String {
    match (&graded.commit, graded.content()) {
        (Some(commit), _) => super::history::short(commit).to_owned(),
        (None, Some(crate::skilluse::Content::Uncommitted)) => "Changes not saved".to_owned(),
        (None, _) => "Not known".to_owned(),
    }
}

/// **What a grade's chats ran on, as its row says it** (SK-60, #3240): the harness, then the
/// model they reported, where the record says one (`Claude Code · claude-sonnet-4-5`).
pub fn ran_on_said(graded: &super::grades::Graded) -> String {
    let harness = graded
        .harness
        .as_deref()
        .map(crate::skilluse::harness_title)
        .unwrap_or_else(|| "Not recorded".to_owned());
    match &graded.model {
        Some(model) => format!("{harness} · {model}"),
        None => harness,
    }
}

/// The Grades panel's blocks for `grades` of `skill`: a sentence on what a grade is, then the
/// table, or the empty table's claim and its way on.
fn grades_blocks(grades: &[super::grades::Graded], skill: &str) -> Vec<Block> {
    use purlis_skills::grade::Arm;
    let mut out = Vec::new();
    if !grades.is_empty() {
        out.push(Block::Note {
            text: format!(
                "Each grade ran the skill's trigger tests in chats with the skill and without \
                 it, and with its previous version where it had one, {} each, and counts the \
                 tests each chat got right. Its difference is against the previous version where \
                 it compared one, else against no skill.",
                crate::counts::counted(purlis_skills::grade::RUNS, "time", "times")
            ),
            tone: Tone::Plain,
        });
    }
    let rows = grades
        .iter()
        .map(|graded| {
            let (passed, of) = graded.report.passed_with();
            // The version it compared with, where it compared one (SK-57).
            let previous = graded.compared_with().map(super::history::short);
            TableRow {
                key: graded.grade.clone(),
                cells: vec![
                    // A grade a saved edit started says so (SK-58).
                    if graded.after_edit {
                        format!("{} · after an edit", said_at(&graded.ts))
                    } else {
                        said_at(&graded.ts)
                    },
                    version_said(graded),
                    ran_on_said(graded),
                    format!("{passed} of {of}"),
                    arm_said(graded, Arm::With),
                    arm_said(graded, Arm::Without),
                    previous
                        .map(|commit| format!("{} · {commit}", arm_said(graded, Arm::Previous)))
                        .unwrap_or_default(),
                    graded
                        .report
                        .delta
                        .map(|delta| match previous {
                            Some(commit) => {
                                format!("{:+.0} points against {commit}", delta * 100.0)
                            }
                            None => format!("{:+.0} points", delta * 100.0),
                        })
                        .unwrap_or_default(),
                    crate::counts::counted(u64::from(graded.spent.chats), "chat", "chats"),
                    super::grades::ended_said(graded.report.ended).to_owned(),
                ],
            }
        })
        .collect();
    out.push(Block::Table(Table {
        caption: format!("Grades of {skill}"),
        columns: [
            "Graded",
            "Version",
            "Ran on",
            "Tests passed",
            "With the skill",
            "Without it",
            "Previous version",
            "Difference",
            "Chats",
            "Ended",
        ]
        .map(str::to_owned)
        .to_vec(),
        rows,
        empty: Empty {
            headline: format!("No grades of {skill} yet"),
            body: Some(
                "Grade runs the skill's trigger tests in chats with the skill and without it, \
                 once you have seen what it costs."
                    .into(),
            ),
            offer: None,
        },
    }));
    out
}

/// How many runs the Runs panel lists at most.
pub const RUNS_SHOWN: usize = 200;

/// **purlis's Runs panel** (SK-3), the StudioPanel capability's first user: every run of the
/// skill the project's skill log holds, newest first, with what started it and its signals —
/// tool calls, tool errors, refusals, asks, tokens and time. Never a pass or a fail: those come
/// from grading.
#[derive(Debug, Clone, Copy)]
pub struct RunsPanel<'a> {
    pub plane: &'a Path,
}

impl StudioPanel for RunsPanel<'_> {
    fn key(&self) -> PanelKey {
        PanelKey::Runs
    }

    fn title(&self) -> String {
        "Runs".to_owned()
    }

    fn by(&self) -> Option<String> {
        None
    }

    fn blocks(&self, skill: &str) -> Result<Vec<Block>, String> {
        Ok(runs_blocks(
            &super::studio_runs(self.plane, Some(skill), RUNS_SHOWN),
            skill,
        ))
    }
}

/// The Runs panel's blocks for `runs` of `skill`: a sentence on what a run is, then the table, or
/// the empty table's claim and its way on.
fn runs_blocks(runs: &[super::StudioRun], skill: &str) -> Vec<Block> {
    let mut out = Vec::new();
    if !runs.is_empty() {
        out.push(Block::Note {
            text: "Each time a chat used the skill, from then until its turn ended. What \
                   happened inside it, never a verdict."
                .to_owned(),
            tone: Tone::Plain,
        });
    }
    let small = |n: u64| n.to_string();
    let rows = runs
        .iter()
        .enumerate()
        .map(|(at, nested)| {
            let run = &nested.run;
            let end = run.end.as_ref();
            let count =
                |of: fn(&crate::skilluse::End) -> u64| end.map(of).map(small).unwrap_or_default();
            TableRow {
                key: run.id.clone().unwrap_or_else(|| format!("before-{at}")),
                cells: vec![
                    said_at(&run.started),
                    run.persona
                        .clone()
                        .unwrap_or_else(|| "No persona".to_owned()),
                    run.ran_on().unwrap_or_else(|| "Not recorded".to_owned()),
                    started_by_said(run.started_by).to_owned(),
                    ended_said(end.map(|end| end.ended), run.id.is_none()).to_owned(),
                    count(|end| end.tool_calls),
                    count(|end| end.tool_errors),
                    count(|end| end.refusals),
                    count(|end| end.asks),
                    match end {
                        None => String::new(),
                        Some(end) => end.tokens.map_or_else(|| "Not reported".to_owned(), small),
                    },
                    end.map(|end| time_said(end.ms)).unwrap_or_default(),
                    nesting_said(nested),
                ],
            }
        })
        .collect();
    out.push(Block::Table(Table {
        caption: format!("Runs of {skill}"),
        columns: [
            "Started",
            "Persona",
            "Ran on",
            "Started by",
            "Ended",
            "Tool calls",
            "Errors",
            "Refused",
            "Asks",
            "Tokens",
            "Time",
            "Nested",
        ]
        .map(str::to_owned)
        .to_vec(),
        rows,
        empty: Empty {
            headline: format!("No runs of {skill} yet"),
            body: Some("A run starts when a chat uses the skill, and ends with its turn.".into()),
            // A chat, where a skill is used: the catalogue's New tab.
            offer: Some(NEW_CHAT.to_owned()),
        },
    }));
    out
}

/// Where a run nests (SK-19): the skill whose run it started inside, and the skills that ran
/// inside it.
fn nesting_said(nested: &super::StudioRun) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(inside) = &nested.inside {
        parts.push(format!("Inside {inside}"));
    }
    if !nested.ran.is_empty() {
        parts.push(format!("Ran {}", nested.ran.join(", ")));
    }
    parts.join("; ")
}

/// The catalogue row that opens a new chat, the empty Runs panel's way out.
const NEW_CHAT: &str = "chat.new";

/// A stamp as a row says it: `2026-10-20 10:00 UTC` for a stamp written in UTC, as every run's
/// is; the stamp itself for one this cannot read.
pub fn said_at(stamp: &str) -> String {
    let bytes = stamp.as_bytes();
    let digits = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .is_some_and(|part| part.iter().all(u8::is_ascii_digit))
    };
    let shaped = bytes.len() >= 16
        && digits(0..4)
        && bytes[4] == b'-'
        && digits(5..7)
        && bytes[7] == b'-'
        && digits(8..10)
        && bytes[10] == b'T'
        && digits(11..13)
        && bytes[13] == b':'
        && digits(14..16);
    if shaped {
        format!("{} {} UTC", &stamp[..10], &stamp[11..16])
    } else {
        stamp.to_owned()
    }
}

/// What started a run, as its row says it.
fn started_by_said(started: Option<purlis_skills::runs::StartedBy>) -> &'static str {
    use purlis_skills::runs::StartedBy;
    match started {
        Some(StartedBy::SkillTool) => "The chat started it",
        Some(StartedBy::Read) => "The chat read the skill",
        Some(StartedBy::McpLoad) => "The chat loaded it through purlis",
        Some(StartedBy::UseSkill) => "You, with Use skill…",
        Some(StartedBy::Curation) => "A curation action",
        Some(StartedBy::Trigger) => "A trigger",
        Some(StartedBy::Grade) => "A grade",
        None => "From before runs were recorded",
        Some(_) => "Not known",
    }
}

/// How a run ended, as its row says it: `from_before` for a use from before runs were recorded.
fn ended_said(ended: Option<purlis_skills::runs::Ended>, from_before: bool) -> &'static str {
    use purlis_skills::runs::Ended;
    match ended {
        Some(Ended::Turn) => "With its turn",
        Some(Ended::Interrupted) => "Interrupted",
        Some(Ended::Session) => "With the chat",
        None if from_before => "Not recorded",
        None => "Running, or not recorded",
        Some(_) => "Not known",
    }
}

/// A run's time, as a person reads it: `850 ms`, `12 s`, `3 min 4 s`.
fn time_said(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else if ms < 60_000 {
        format!("{} s", ms / 1000)
    } else {
        format!("{} min {} s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

/// **An extension's panel on a skill's tab**, handed the skill's text files in the question.
struct ExtensionPanel<'a> {
    studio: Studio<'a>,
    id: &'a str,
    name: &'a str,
    panel: &'a wire::Panel,
    files: &'a [HandedFile],
}

impl StudioPanel for ExtensionPanel<'_> {
    fn key(&self) -> PanelKey {
        PanelKey::Extension {
            extension: self.id.to_owned(),
            panel: self.panel.id.clone(),
        }
    }

    fn title(&self) -> String {
        self.panel.title.clone()
    }

    fn by(&self) -> Option<String> {
        Some(self.name.to_owned())
    }

    fn blocks(&self, skill: &str) -> Result<Vec<Block>, String> {
        let doc = self.studio.ask(
            self.id,
            &Ask::Panel {
                panel: &self.panel.id,
                skill,
                files: self.files,
            },
        )?;
        wire::blocks_of(self.id, &doc)
    }
}

/// The text files of the skill `name` in the project at `plane`, from the narrowest level that
/// holds it, as a question hands them; none where no level does.
fn skill_files(plane: &Path, name: &str) -> Vec<HandedFile> {
    for level in super::verbs::Level::every(plane).iter().rev() {
        let Ok((folder, text)) = super::verbs::text_of(plane, level, name) else {
            continue;
        };
        let skill_md = folder.join(purlis_skills::FILE);
        let mut files: Vec<SkillFile> = vec![SkillFile {
            path: purlis_skills::FILE.to_owned(),
            bytes: text.into_bytes(),
            executable: false,
        }];
        // Read through the skill's folder held from the project's root, never through a link
        // (SK-36), and no further than a question carries.
        if let Ok(held) = purlis_skills::SkillFolder::open(plane, &skill_md) {
            for entry in held.entries() {
                let purlis_skills::Entry::File(rel) = entry else {
                    continue;
                };
                let Some(text) = held.read(&rel, wire::MOST_HANDED_BYTES as u64) else {
                    continue;
                };
                files.push(SkillFile {
                    path: rel.to_string_lossy().into_owned(),
                    bytes: text.into_bytes(),
                    executable: false,
                });
            }
        }
        return wire::handed(&files);
    }
    Vec::new()
}

#[cfg(test)]
mod tests;
