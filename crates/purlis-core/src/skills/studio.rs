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
use crate::extension::studio::{self as wire, Ask, HandedFile, LintedSkill, SkillFile};
use crate::panel::{Block, Empty, Table, TableRow, Tone};

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
    /// The fingerprint this machine approved it by, or `None` for one of the app's own: what
    /// tells one version of it from the next (#3940).
    pub approved: Option<String>,
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
                    approved: entry.approved.clone(),
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
                many_at_once: asked.declared.lint_many_at_once,
                id: asked.id,
                approved: asked.approved,
            })
            .collect()
    }

    /// **The lints of each extension that declares `skill-lint`, as a studio list asks them**
    /// (#3940): answered from what each said of the same files before, and asked only within
    /// the list's time ([`ListLints`]).
    pub fn list_lints(&self) -> ListLints<ExtensionLint<'a>> {
        ListLints::new(self.lints(), &KEPT, ListTime::default())
    }

    /// **The panels on `skill`'s tab**, in the project at `plane`: purlis's Runs panel first,
    /// then each extension's, each answered or refused with a sentence. A skill's grades are not
    /// a panel: its Grades tab reads them whole (`skill_grades`, S6).
    pub fn panels(&self, plane: &Path, skill: &str) -> Vec<Answered> {
        let mut out = own_panels(plane, skill);
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

/// **purlis's own panels on `skill`'s tab**, in the project at `plane`: its Runs panel. What
/// every skill's tab draws first, with a studio or without one.
pub fn own_panels(plane: &Path, skill: &str) -> Vec<Answered> {
    vec![Answered::of(&RunsPanel { plane }, skill)]
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
    /// The fingerprint this machine approved it by ([`Asked::approved`]).
    approved: Option<String>,
    /// Whether it answers many skills in one question ([`wire::Declared::lint_many_at_once`]).
    many_at_once: bool,
}

impl SkillLint for ExtensionLint<'_> {
    fn findings(&self, skill: &SkillFiles<'_>) -> Vec<Finding> {
        let says = match self.asked(skill, None) {
            Ok(says) => says,
            Err(why) => vec![format!("could not lint this skill: {why}")],
        };
        said_by(&self.by, says)
    }
}

/// Each of `says` as a finding of the lint said under `by`: a warning.
fn said_by(by: &str, says: Vec<String>) -> Vec<Finding> {
    says.into_iter()
        .map(|says| Finding {
            severity: Severity::Warns,
            lint: Lint::Said {
                by: by.to_owned(),
                says,
            },
        })
        .collect()
}

impl ExtensionLint<'_> {
    /// What it answers of `skill`, by `until` where given, or why it did not.
    fn asked(
        &self,
        skill: &SkillFiles<'_>,
        until: Option<std::time::Instant>,
    ) -> Result<Vec<String>, String> {
        let files = handed_of(skill);
        let ask = Ask::Findings {
            skill: skill.name,
            files: &files,
        };
        let studio = &self.studio;
        match until {
            None => studio.ask(&self.id, &ask),
            Some(until) => studio.executor.studio_until(
                studio.config_root,
                studio.project,
                &self.id,
                &ask,
                until,
            ),
        }
        .and_then(|doc| wire::findings_of(&self.id, &doc))
    }
}

/// **`skill`'s files as a lint question hands them**: its `SKILL.md`, then its other text files
/// within one question's bounds ([`wire::handed`]).
fn handed_of(skill: &SkillFiles<'_>) -> Vec<HandedFile> {
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
    files
}

/// The most bytes of `skill`'s files a question hands ([`handed_of`]): what a question of many
/// skills is counted by ([`wire::MOST_AT_ONCE_BYTES`]).
fn handed_bytes(skill: &SkillFiles<'_>) -> usize {
    let others: usize = skill.others.iter().map(|(_, text)| text.len()).sum();
    skill.text.len() + others.min(wire::MOST_HANDED_BYTES)
}

impl InTheList for ExtensionLint<'_> {
    fn by(&self) -> &str {
        &self.by
    }

    fn version(&self) -> [u8; 32] {
        use sha2::Digest as _;
        let mut hash = sha2::Sha256::new();
        for part in [
            self.id.as_bytes(),
            self.approved.as_deref().unwrap_or_default().as_bytes(),
            // The project's choices, its settings for the extension among them, and its plane.
            format!("{:?}", self.studio.project).as_bytes(),
        ] {
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part);
        }
        hash.finalize().into()
    }

    fn ask(
        &self,
        skill: &SkillFiles<'_>,
        until: std::time::Instant,
    ) -> Result<Vec<String>, String> {
        self.asked(skill, Some(until))
    }

    fn many_at_once(&self) -> bool {
        self.many_at_once
    }

    fn ask_each(
        &self,
        skills: &[&SkillFiles<'_>],
        until: std::time::Instant,
    ) -> Result<Vec<Vec<String>>, String> {
        let linted: Vec<LintedSkill> = skills
            .iter()
            .map(|skill| LintedSkill {
                skill: skill.name.to_owned(),
                files: handed_of(skill),
            })
            .collect();
        let names: Vec<&str> = skills.iter().map(|skill| skill.name).collect();
        let studio = &self.studio;
        studio
            .executor
            .studio_until(
                studio.config_root,
                studio.project,
                &self.id,
                &Ask::FindingsOfEach { skills: &linted },
                until,
            )
            .and_then(|doc| wire::findings_of_each(&self.id, &names, &doc))
    }
}

// ---------------------------------------------------------------------------------------
// Lints in a list (#3940)
// ---------------------------------------------------------------------------------------

/// **How long one studio list asks the extensions' lints for**, from its first question: past
/// it, a list asks no more, and shows what each said of the same files before.
pub const LIST_LINT_TIME: std::time::Duration = std::time::Duration::from_millis(100);

/// **How long one lint asked in a list has to answer**, its confined program's start inside it,
/// as a router has (ADR 0102 §6). A list that started its last question just before
/// [`LIST_LINT_TIME`] ran out spends both, at most.
///
/// **A lint that answers many skills at once** (#4195) is asked one question for the list's
/// skills, by the most a list ever spends: [`LIST_LINT_TIME`] and this, from the list's first
/// question. So its program starts once for the list, and one that takes longer than this to
/// start still answers in it, while the list spends no more time than one asking each skill.
pub const LIST_LINT_DEADLINE: std::time::Duration = wire::ROUTER_DEADLINE;

/// **How long a lint that did not answer in a list waits before a list asks it again** about the
/// same files: a slow or failing extension costs a list one question at most this often, and the
/// editor's live lint, which waits for it, says why. Short, as a question also goes unanswered
/// while the extension's one slot is busy with another (the editor's live lint).
pub const ASK_AGAIN_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

/// The most answers kept: past it, one is dropped for each one kept.
const MOST_KEPT: usize = 4096;

/// **What tells one skill's files from another's**: its name, its `SKILL.md` and every other
/// file handed a lint, each part with its length so no two lists of parts hash alike. Hashed once
/// per skill a list lints, however many lints ask about it.
fn files_digest(skill: &SkillFiles<'_>) -> [u8; 32] {
    use sha2::Digest as _;
    let mut hash = sha2::Sha256::new();
    let mut part = |bytes: &[u8]| {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    };
    part(skill.name.as_bytes());
    part(skill.text.as_bytes());
    for (path, text) in skill.others {
        part(path.as_bytes());
        part(text.as_bytes());
    }
    hash.finalize().into()
}

/// What `lint`'s answer about the files `files` hashed to is kept by.
fn key_of(version: [u8; 32], files: [u8; 32]) -> [u8; 32] {
    use sha2::Digest as _;
    let mut hash = sha2::Sha256::new();
    hash.update(version);
    hash.update(files);
    hash.finalize().into()
}

/// What each extension's lint said of a skill's files, in this process, by [`InTheList::key`].
static KEPT: std::sync::Mutex<Kept> = std::sync::Mutex::new(Kept::new());

/// **What lints asked in a list said, by the files they were asked about**: kept while the app
/// runs, never written, so nothing to remove ([`ListLints`]).
#[derive(Debug, Default)]
pub struct Kept(std::collections::BTreeMap<[u8; 32], Was>);

#[derive(Debug, Clone)]
enum Was {
    Said(Vec<String>),
    /// It did not answer, at this time.
    Unanswered(std::time::Instant),
}

impl Kept {
    pub const fn new() -> Self {
        Self(std::collections::BTreeMap::new())
    }
}

/// **A lint a studio list asks**, and what tells one question of it from another.
pub trait InTheList {
    /// What its findings are said under.
    fn by(&self) -> &str;
    /// What tells it from every other lint, and this version of it and the project's choices
    /// from another: with the skill's files ([`files_digest`]), what its answer is kept by.
    fn version(&self) -> [u8; 32];
    /// Its answer about `skill`, by `until`, or why there is none.
    fn ask(&self, skill: &SkillFiles<'_>, until: std::time::Instant)
    -> Result<Vec<String>, String>;
    /// Whether it answers many skills in one question ([`Self::ask_each`], #4195).
    fn many_at_once(&self) -> bool {
        false
    }
    /// Its answers about each of `skills`, in their order, in one question by `until`, or why
    /// there are none. Asked only where [`Self::many_at_once`].
    fn ask_each(
        &self,
        skills: &[&SkillFiles<'_>],
        until: std::time::Instant,
    ) -> Result<Vec<Vec<String>>, String> {
        let _ = (skills, until);
        Err(format!("{} answers one skill at a time", self.by()))
    }
}

/// The time a list gives its lints, and the clock it reads.
#[derive(Clone, Copy)]
pub struct ListTime {
    pub time: std::time::Duration,
    pub deadline: std::time::Duration,
    pub now: fn() -> std::time::Instant,
}

impl Default for ListTime {
    fn default() -> Self {
        Self {
            time: LIST_LINT_TIME,
            deadline: LIST_LINT_DEADLINE,
            now: std::time::Instant::now,
        }
    }
}

/// **The extensions' lints, as one studio list asks them** (#3940): one [`SkillLint`] for them
/// all. Each answers a skill's files from what it said of the same files before, kept while
/// the app runs. Only a question with no such answer is asked, and only within the list's time
/// ([`LIST_LINT_TIME`]), each by its deadline ([`LIST_LINT_DEADLINE`]): past it, the list shows
/// what it has, and the next list asks on. A lint that did not answer shows nothing in the list,
/// and is asked again after [`ASK_AGAIN_AFTER`]. So a list whose skills were all asked about
/// costs no question at all, and the window and Needs you's count read the same answers.
pub struct ListLints<L> {
    lints: Vec<L>,
    kept: &'static std::sync::Mutex<Kept>,
    time: ListTime,
    /// When the list asked its first question.
    began: std::cell::Cell<Option<std::time::Instant>>,
}

impl<L: InTheList> ListLints<L> {
    pub fn new(lints: Vec<L>, kept: &'static std::sync::Mutex<Kept>, time: ListTime) -> Self {
        Self {
            lints,
            kept,
            time,
            began: std::cell::Cell::new(None),
        }
    }

    /// Whether it has any lint to ask.
    pub fn is_empty(&self) -> bool {
        self.lints.is_empty()
    }

    /// What `lint` says of `skill`: what it said of the same files, or its answer now, within
    /// the list's time.
    fn said(&self, lint: &L, skill: &SkillFiles<'_>, files: [u8; 32]) -> Vec<String> {
        let key = key_of(lint.version(), files);
        let now = (self.time.now)();
        if let Some(said) = self.before(&key, now) {
            return said;
        }
        if self.began_within(now).is_none() {
            return Vec::new();
        }
        let answer = lint.ask(skill, now + self.time.deadline);
        self.keep(key, answer)
    }

    /// **What `lint`, which answers many skills at once, says of each of `skills`** (#4195): what
    /// it said of the same files, and for the rest one question, by the most the list spends
    /// ([`LIST_LINT_DEADLINE`]). More than one question holds ([`in_questions`]) are asked one
    /// after another, each only within the list's time.
    fn said_at_once(
        &self,
        lint: &L,
        skills: &[SkillFiles<'_>],
        files: &[[u8; 32]],
    ) -> Vec<Vec<String>> {
        let version = lint.version();
        let keys: Vec<[u8; 32]> = files.iter().map(|files| key_of(version, *files)).collect();
        let now = (self.time.now)();
        let mut out: Vec<Vec<String>> = vec![Vec::new(); skills.len()];
        let mut asking: Vec<usize> = Vec::new();
        for (at, key) in keys.iter().enumerate() {
            match self.before(key, now) {
                Some(said) => out[at] = said,
                None => asking.push(at),
            }
        }
        for question in in_questions(&asking, skills) {
            let Some(began) = self.began_within((self.time.now)()) else {
                break;
            };
            let these: Vec<&SkillFiles<'_>> = question.iter().map(|at| &skills[*at]).collect();
            let answers = match lint.ask_each(&these, began + self.time.time + self.time.deadline) {
                Ok(answers) if answers.len() == question.len() => {
                    answers.into_iter().map(Ok).collect()
                }
                Ok(_) => vec![Err(String::new()); question.len()],
                Err(why) => vec![Err(why); question.len()],
            };
            for (at, answer) in question.iter().zip(answers) {
                out[*at] = self.keep(keys[*at], answer);
            }
        }
        out
    }

    /// What was kept by `key` at `now`: its answer, or nothing for one that did not answer
    /// [`ASK_AGAIN_AFTER`] ago; `None` where it is to be asked.
    fn before(&self, key: &[u8; 32], now: std::time::Instant) -> Option<Vec<String>> {
        let before = self
            .kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0
            .get(key)
            .cloned();
        match before {
            Some(Was::Said(says)) => Some(says),
            Some(Was::Unanswered(at)) if now.saturating_duration_since(at) < ASK_AGAIN_AFTER => {
                Some(Vec::new())
            }
            _ => None,
        }
    }

    /// When the list asked its first question, where `now` is within its time to ask another:
    /// the first question is asked now.
    fn began_within(&self, now: std::time::Instant) -> Option<std::time::Instant> {
        let began = self.began.get().unwrap_or(now);
        self.began.set(Some(began));
        (now.saturating_duration_since(began) < self.time.time).then_some(began)
    }

    /// Keep `answer` by `key`, and say what the list shows of it.
    fn keep(&self, key: [u8; 32], answer: Result<Vec<String>, String>) -> Vec<String> {
        let was = match &answer {
            Ok(says) => Was::Said(says.clone()),
            Err(_) => Was::Unanswered((self.time.now)()),
        };
        let mut kept = self
            .kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // An answer another list kept meanwhile stands over a question this one could not ask:
        // the extension's one slot may have been that list's.
        if matches!(was, Was::Unanswered(_)) && matches!(kept.0.get(&key), Some(Was::Said(_))) {
            return answer.unwrap_or_default();
        }
        if kept.0.len() >= MOST_KEPT
            && let Some(any) = kept.0.keys().next().copied()
        {
            // The keys are hashes, so the first is any one: no list's answers are all dropped.
            kept.0.remove(&any);
        }
        kept.0.insert(key, was);
        answer.unwrap_or_default()
    }
}

/// **The skills at `asking` of `skills`, in questions of many at once**: each at most
/// [`wire::MOST_AT_ONCE`] skills and [`wire::MOST_AT_ONCE_BYTES`] of their files, in order.
fn in_questions(asking: &[usize], skills: &[SkillFiles<'_>]) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut bytes = 0usize;
    for at in asking {
        let more = handed_bytes(&skills[*at]);
        match out.last_mut() {
            Some(last)
                if last.len() < wire::MOST_AT_ONCE && bytes + more <= wire::MOST_AT_ONCE_BYTES =>
            {
                last.push(*at);
                bytes += more;
            }
            _ => {
                out.push(vec![*at]);
                bytes = more;
            }
        }
    }
    out
}

impl<L: InTheList> SkillLint for ListLints<L> {
    fn findings(&self, skill: &SkillFiles<'_>) -> Vec<Finding> {
        self.findings_of_each(std::slice::from_ref(skill))
            .pop()
            .unwrap_or_default()
    }

    /// Each lint asked about every skill of the list: one that answers many at once in one
    /// question ([`InTheList::many_at_once`], #4195), any other one skill at a time.
    fn findings_of_each(&self, skills: &[SkillFiles<'_>]) -> Vec<Vec<Finding>> {
        let mut out: Vec<Vec<Finding>> = vec![Vec::new(); skills.len()];
        if self.lints.is_empty() || skills.is_empty() {
            return out;
        }
        let files: Vec<[u8; 32]> = skills.iter().map(files_digest).collect();
        for lint in &self.lints {
            let said: Vec<Vec<String>> = if lint.many_at_once() {
                self.said_at_once(lint, skills, &files)
            } else {
                skills
                    .iter()
                    .zip(&files)
                    .map(|(skill, files)| self.said(lint, skill, *files))
                    .collect()
            };
            for (each, says) in out.iter_mut().zip(said) {
                each.extend(said_by(lint.by(), says));
            }
        }
        out
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
