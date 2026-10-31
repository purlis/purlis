// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A router's grade** (SK-15e, #3233; ADR 0102 §8): the trigger tests of a library, run with
//! the router under grade in place and with the built-in router, recorded with the library's
//! commit, and shown where the router is chosen.
//!
//! - **The library** is the set a chat of the workspace and persona that chose the router gets:
//!   every project, workspace and persona skill it is handed, each read once ([`SkillCopy`]).
//!   Every one is laid out in each chat, so the router is asked about the same candidates a real
//!   chat's start asks it about; the ones with trigger tests are graded.
//! - **Each chat is a grade's chat** ([`super::ready_to_grade`]), in a throwaway piece of its own
//!   ([`crate::skills::verbs::drafts::lay_out_library_arm`]). In the router's arm its start asks
//!   the router, as any chat's start does (SK-15b); in the built-in router's arm nothing is asked.
//! - **The consent** is one digest of the router, every copy and every test: a grade started on a
//!   plan that no longer holds is refused, as a skill's is.
//! - **The record** is `personas/_grades/routers/<YYYY-MM>.<device>.jsonl`, beside the skills'
//!   eval record, one row a grade: the router, the library's commit (the plane's `HEAD` where
//!   every copy is committed there, else none), every copy's version, and each skill's report.
//!   A grade of a library that holds a copy that is this machine's alone is kept in this
//!   machine's state, never committed.

use std::path::{Path, PathBuf};

use purlis_skills::grade::routers::{self as engine, RouterGradeReport, RouterGradeRequest};
use purlis_skills::grade::{self, Arm, GradeRequest, Plan, RUNS};

use super::{
    LeftOut, MOST_LINE_BYTES, MOST_RUNS, SkillCopy, Spent, cap, copy_of, dollars, file_text,
    is_local, left_out_why, plain, record_files, report_holds_together, stamped_by, tokens,
};
use crate::skills::verbs::Level;
use crate::skilluse::Content;

/// The folder under the eval record's that routers' grades are kept in.
pub const DIR: &str = "routers";

/// The version of a row this purlis writes and reads.
pub const VERSION: u32 = 1;

/// The most skills of a library a router's grade lays out in each chat.
pub const MOST_SKILLS: usize = 200;

/// The most grades of one router read back, newest first.
const MOST_GRADES: usize = 50;

/// **One copy of the library**, at its level, read once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryCopy {
    pub level: Level,
    pub copy: SkillCopy,
}

/// **A skill of the library a router's grade grades**: its trigger tests' count, the ones it
/// leaves out, or why its tests could not be read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PlannedSkill {
    pub skill: String,
    pub tests: usize,
    pub left_out: Vec<LeftOut>,
    pub refused: Option<String>,
}

/// **What a router's grade would run**, before it runs.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RouterPlanned {
    pub router: String,
    pub workspace: Option<String>,
    pub persona: Option<String>,
    /// How many skills each chat is handed: the library.
    pub library: usize,
    /// The skills of it with trigger tests, and the ones whose tests could not be read.
    pub skills: Vec<PlannedSkill>,
    pub request: Option<RouterGradeRequest>,
    pub plan: Option<Plan>,
    /// The commit the library is, where every copy is committed at the plane's `HEAD`.
    pub commit: Option<String>,
    pub cap: cap::Cap,
    /// What the person consents to by pressing Grade on this plan ([`consent_of`]).
    pub consent: Option<String>,
    /// The pins of the workspace or persona that chose the router: every chat of the grade
    /// reads them, so the router is asked about what it may hide at a real start there.
    pub pins: Vec<String>,
    /// The library as it was read once: what every chat of a grade started on this plan is
    /// handed, and what its record says it graded. Never shown, so never sent.
    #[serde(skip)]
    pub copies: Vec<LibraryCopy>,
}

/// **The library a chat in `workspace` as `persona` of the project at `root` is handed**: every
/// skill of the plane it gets, by its level, built-in skills left out (every chat has them).
fn library_of(
    root: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
) -> Result<Vec<(Level, String)>, String> {
    let (set, _pins) = crate::skills::unrouted(
        None,
        crate::skills::PlaneSkills {
            plane: root,
            workspace,
            repo: &[],
            routed: None,
        },
        persona,
        crate::harness::Harness::ClaudeCode,
    );
    let mut library = Vec::new();
    for one in &set.effective {
        use purlis_skills::Origin;
        if !matches!(
            one.origin,
            Origin::Project | Origin::Workspace | Origin::Persona
        ) {
            continue;
        }
        let Some(level) = one
            .skill
            .path
            .parent()
            .and_then(Path::parent)
            .and_then(|dir| dir.strip_prefix(root).ok())
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .and_then(|rel| Level::of_rel(&rel))
        else {
            continue;
        };
        library.push((level, one.skill.name.clone()));
    }
    if library.len() > MOST_SKILLS {
        return Err(format!(
            "this library has {} skills, more than the {MOST_SKILLS} a router's grade lays out \
             in each chat",
            library.len()
        ));
    }
    Ok(library)
}

/// **The plan of grading the router `router`** chosen by `workspace` or `persona` in the project
/// at `root`: its library, read once, its trigger tests, and what the grade would run.
pub fn planned(
    root: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    router: &str,
) -> Result<RouterPlanned, String> {
    if !crate::extension::an_id(router) {
        return Err("a router's grade names no router".to_owned());
    }
    let head_before = head(root);
    let pins = crate::skills::pins_or_none(root, workspace, persona);
    let mut copies = Vec::new();
    let mut skills = Vec::new();
    let mut requests = Vec::new();
    for (level, name) in library_of(root, workspace, persona)? {
        // A copy that cannot be read is left out of every chat, and the plan names it.
        let read = copy_of(root, &level, &name, &[])
            .and_then(|copy| Ok((copy, crate::skills::verbs::find(root, &level, &name)?)));
        let (copy, folder) = match read {
            Ok(read) => read,
            Err(why) => {
                skills.push(PlannedSkill {
                    skill: name.clone(),
                    tests: 0,
                    left_out: Vec::new(),
                    refused: Some(format!("{why}, so it is left out of every chat")),
                });
                continue;
            }
        };
        let fixtures = grade::fixtures_of(
            root,
            Path::new(&crate::skills::verbs::rel(root, &folder)),
            &name,
        );
        let mut left_out = Vec::new();
        let tests: Vec<grade::TriggerTest> = fixtures
            .triggers
            .read()
            .map(|tests| {
                tests
                    .iter()
                    .enumerate()
                    .filter(|(at, test)| match left_out_why(&test.query) {
                        Some(why) => {
                            left_out.push(LeftOut { at: *at, why });
                            false
                        }
                        None => true,
                    })
                    .map(|(_, test)| test.clone())
                    .collect()
            })
            .unwrap_or_default();
        let refused = fixtures
            .triggers
            .refused()
            .map(purlis_skills::grade::Refused::in_window);
        if !tests.is_empty() || refused.is_some() || !left_out.is_empty() {
            skills.push(PlannedSkill {
                skill: name.clone(),
                tests: tests.len(),
                left_out,
                refused,
            });
        }
        if !tests.is_empty() {
            requests.push(GradeRequest {
                skill: name.clone(),
                triggers: tests,
                outcomes: Vec::new(),
                arms: engine::ARMS.to_vec(),
                runs: RUNS,
            });
        }
        copies.push(LibraryCopy { level, copy });
    }
    let request = (!requests.is_empty()).then(|| RouterGradeRequest {
        router: router.to_owned(),
        skills: requests,
    });
    let plan = request.as_ref().map(engine::plan);
    let committed = copies
        .iter()
        .all(|one| matches!(one.copy.content, Content::Tree(_)));
    let commit = head_before
        .filter(|_| committed)
        .filter(|before| head(root).as_ref() == Some(before));
    let consent = request
        .as_ref()
        .map(|request| consent_of(request, workspace, persona, &pins, &copies));
    Ok(RouterPlanned {
        router: router.to_owned(),
        workspace: workspace.map(str::to_owned),
        persona: persona.map(str::to_owned),
        library: copies.len(),
        skills,
        request,
        plan,
        commit,
        cap: cap::token_cap(root),
        consent,
        pins,
        copies,
    })
}

/// The commit the plane at `root` has checked out, or `None`.
fn head(root: &Path) -> Option<String> {
    crate::worktree::git::run(
        root,
        &["rev-parse", "--verify", "-q", "HEAD"],
        crate::worktree::git::READ,
    )
    .ok()
    .filter(crate::worktree::git::Run::ok)
    .map(|run| run.out.trim().to_owned())
    .filter(|commit| hex(commit))
}

/// Whether `word` is a whole commit id.
fn hex(word: &str) -> bool {
    matches!(word.len(), 40 | 64)
        && word
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// **What the person consented to**: one digest of the router, the scope it was chosen in, every
/// trigger test the plan showed, and every copy of the library as it was read, by its level,
/// what its folder held and its files.
pub fn consent_of(
    request: &RouterGradeRequest,
    workspace: Option<&str>,
    persona: Option<&str>,
    pins: &[String],
    copies: &[LibraryCopy],
) -> String {
    use sha2::Digest as _;
    let mut digest = sha2::Sha256::new();
    let mut part = |bytes: &[u8]| {
        digest.update(bytes);
        digest.update(b"\0");
    };
    part(request.router.as_bytes());
    part(workspace.unwrap_or_default().as_bytes());
    part(persona.unwrap_or_default().as_bytes());
    part(pins.join("\n").as_bytes());
    part(&serde_json::to_vec(&request.skills).unwrap_or_default());
    for one in copies {
        part(one.level.rel().unwrap_or_default().as_bytes());
        part(one.copy.snapshot.name.as_bytes());
        part(one.copy.content.word().as_bytes());
        for (path, text) in &one.copy.snapshot.files {
            part(path.as_bytes());
            part(text.as_bytes());
        }
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// **One copy of the library, as a router's grade records it.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecordedCopy {
    pub skill: String,
    pub level: Option<String>,
    pub tree: String,
    pub commit: Option<String>,
}

impl RecordedCopy {
    /// `one` as its grade records it.
    pub fn of(one: &LibraryCopy) -> Self {
        Self {
            skill: one.copy.snapshot.name.clone(),
            level: one.level.rel(),
            tree: one.copy.content.word().to_owned(),
            commit: one.copy.commit.clone(),
        }
    }
}

/// **One router's grade, as its record holds it.**
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RouterGraded {
    pub v: u32,
    pub grade: String,
    pub router: String,
    pub workspace: Option<String>,
    pub persona: Option<String>,
    /// The commit the library is, where every copy was committed at the plane's `HEAD`.
    pub commit: Option<String>,
    pub library: Vec<RecordedCopy>,
    pub harness: Option<String>,
    pub profile: Option<String>,
    pub ts: String,
    pub runs: u8,
    pub report: RouterGradeReport,
    pub spent: Spent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl RouterGraded {
    /// **What a person is told of it** where the router is chosen: each arm's trigger tests
    /// passed, and the library's commit it was graded on.
    pub fn said(&self) -> String {
        let (router, of) = self.report.passed_in(Arm::Router);
        let (built_in, built_in_of) = self.report.passed_in(Arm::BuiltInRouter);
        let on = self.library_said();
        let when = self.ts.get(..10).unwrap_or(&self.ts);
        // A grade purlis stopped under before it was scored (#3616) passed nothing and failed
        // nothing: it says so, and no count.
        if super::Spends::stopped_unscored(self) {
            return format!(
                "Interrupted on {on}, {when}: the grade stopped while it ran, before any \
                 trigger test was scored."
            );
        }
        let ended = match self.report.ended {
            grade::GradeEnd::Whole => String::new(),
            end => format!(" ({})", super::ended_said(end).to_lowercase()),
        };
        format!(
            "Graded on {on}, {when}{ended}: {router} of {of} trigger tests pass with {}, and \
             {built_in} of {built_in_of} with the built-in router.",
            purlis_skills::Router::Extension(self.router.clone()).shown(),
        )
    }

    /// **The library it was graded on**, in words: its commit, or that it held changes not
    /// committed.
    pub fn library_said(&self) -> String {
        match &self.commit {
            Some(commit) => format!("the library at {}", short(commit)),
            None => "a library with changes not committed".to_owned(),
        }
    }

    /// Whether a row read back says only what purlis writes, as a skill's grade is held
    /// ([`super::Graded`]): plain names, whole commit ids, a time, and each skill's report one
    /// the built-in runner could have written.
    pub(super) fn sound(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        let names = |name: &Option<String>| name.as_deref().is_none_or(plain);
        let copies = self.library.len() <= MOST_SKILLS
            && self.library.iter().all(|copy| {
                plain(&copy.skill)
                    && Content::parse(&copy.tree).is_some()
                    && copy.commit.as_deref().is_none_or(hex)
                    && copy.level.as_deref().is_none_or(super::level_word)
            });
        let parts = self.report.skills.len() <= MOST_SKILLS
            && self.report.skills.iter().all(|part| {
                plain(&part.skill)
                    && report_holds_together(&part.report, self.runs)
                    && part
                        .report
                        .arms
                        .iter()
                        .all(|arm| engine::ARMS.contains(&arm.arm))
            });
        self.v <= VERSION
            && crate::extension::an_id(&self.router)
            && self.report.router == self.router
            && plain(&self.report.grader)
            && plain(&self.grade)
            && names(&self.workspace)
            && names(&self.persona)
            && names(&self.harness)
            && names(&self.profile)
            && self.commit.as_deref().is_none_or(hex)
            && stamped_by(&self.ts, now)
            && (1..=MOST_RUNS).contains(&self.runs)
            && self.spent.tokens.is_none_or(tokens)
            && self.spent.cost_usd.is_none_or(dollars)
            // Its arms count what it asked for: only a skill's interrupted row says it (#3602).
            && self.spent.asked_for.is_none()
            && self.report.spent_tokens.is_none_or(tokens)
            && self.model.as_deref().is_none_or(super::price::model_id)
            && copies
            && parts
    }
}

/// **The report of a router's grade purlis stopped under before it was scored** (#3616): no skill
/// scored, the caps it was held to and the price its dollar limit was worked out at, interrupted,
/// as a skill's is ([`super::underway::interrupted_report`]).
pub fn interrupted_report(
    router: &str,
    cap: u64,
    cost_cap_usd: Option<f64>,
    priced: Option<grade::Priced>,
) -> RouterGradeReport {
    RouterGradeReport {
        grader: grade::BUILT_IN.to_owned(),
        router: router.to_owned(),
        skills: Vec::new(),
        ended: grade::GradeEnd::Interrupted,
        not_started: None,
        cap: Some(cap),
        spent_tokens: None,
        cost_cap_usd,
        spent_usd: None,
        priced,
    }
}

/// Whether any copy of `copies` is this machine's alone: a grade of their library is kept in
/// this machine's state, never committed.
pub fn library_is_local(root: &Path, copies: &[LibraryCopy]) -> bool {
    copies.iter().any(|one| is_local(root, &one.level))
}

impl super::Spends for RouterGraded {
    /// Every chat of every skill's arms, ran or not. A grade purlis stopped under before it
    /// scored a skill asked for none to go by: a router's row never counts them (#3602).
    fn asked(&self) -> u64 {
        self.report
            .skills
            .iter()
            .map(|part| super::asked_in(&part.report.arms))
            .sum()
    }

    fn stopped_unscored(&self) -> bool {
        self.report.ended == grade::GradeEnd::Interrupted && self.report.skills.is_empty()
    }

    fn spent(&self) -> &Spent {
        &self.spent
    }

    fn ts(&self) -> &str {
        &self.ts
    }

    fn profile(&self) -> Option<&str> {
        self.profile.as_deref()
    }

    fn harness(&self) -> Option<&str> {
        self.harness.as_deref()
    }

    fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }
}

/// **What a grade of a router would cost, from its grade before** (#3618), as a skill's plan
/// works it out ([`super::estimate_of`]), and whether that grade is known to have run on the
/// library this one runs on: a chat is handed the whole library, so one of another library may
/// spend otherwise, and the plan says so.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RouterEstimate {
    pub estimate: super::Estimate,
    pub same_library: bool,
}

/// **The estimate of grading `planned` on `profile`** (#3618), from the grades of its router in
/// its scope in the project at `root`: `None` where it has none to go by, or the plan runs no
/// chat. A grade purlis stopped under before it scored a skill is none to go by: its row says no
/// count of the chats it asked for (#3602), so it is passed over, never read as one that ran no
/// chat.
pub fn estimate(
    root: &Path,
    planned: &RouterPlanned,
    profile: Option<&str>,
) -> Option<RouterEstimate> {
    let chats = planned.plan?.chats;
    let grades: Vec<RouterGraded> = grades_of(
        root,
        &planned.router,
        planned.workspace.as_deref(),
        planned.persona.as_deref(),
    )
    .into_iter()
    .filter(|row| !super::Spends::stopped_unscored(row))
    .collect();
    let (estimate, last) = super::estimate_from(&grades, chats, profile)?;
    // The same library: the same copies, each by its level, its name and the tree its folder
    // held, whatever commit holds them. A copy with changes not committed says no tree, so a
    // library holding one is never known to be the same.
    let held = |copies: &mut dyn Iterator<Item = RecordedCopy>| {
        let mut held: Vec<_> = copies
            .map(|copy| (copy.level, copy.skill, copy.tree))
            .collect();
        held.sort();
        held
    };
    let now = held(&mut planned.copies.iter().map(RecordedCopy::of));
    let committed = |copies: &[(Option<String>, String, String)]| {
        copies
            .iter()
            .all(|(_, _, tree)| matches!(Content::parse(tree), Some(Content::Tree(_))))
    };
    let then = held(&mut last.library.iter().cloned());
    Some(RouterEstimate {
        estimate,
        same_library: committed(&now) && then == now,
    })
}

/// The first twelve characters of a commit id.
fn short(commit: &str) -> &str {
    commit.get(..12).unwrap_or(commit)
}

/// `personas/_grades/routers/<YYYY-MM>.<device>.jsonl`.
fn path_for(root: &Path, when: chrono::DateTime<chrono::Utc>, device: &str) -> PathBuf {
    dir(root).join(format!("{}.{device}.jsonl", when.format("%Y-%m")))
}

/// The committed folder of routers' grades.
fn dir(root: &Path) -> PathBuf {
    root.join("personas").join(super::DIR_NAME).join(DIR)
}

/// This machine's folder of routers' grades of a library with a copy of its own, in its app state
/// of the project (`crate::appstate`): `None` where this machine has no data home.
fn local_dir(root: &Path) -> Option<PathBuf> {
    Some(crate::appstate::dir(root)?.join(super::LOCAL_DIR).join(DIR))
}

/// **Appends `graded` to this month's file** of the project at `root`: the committed record, or,
/// where any copy of its library is this machine's alone, this machine's state. `None` where the
/// project is one this purlis may not write, or the write was refused.
pub fn record(
    root: &Path,
    copies: &[LibraryCopy],
    graded: &RouterGraded,
    device: &str,
) -> Option<PathBuf> {
    record_to(root, library_is_local(root, copies), graded, || {
        device.to_owned()
    })
}

/// [`record`], to this machine's state where `local`, else to the committed record of the device
/// `device` names: asked only then, as naming it reads this machine's store (a grade under way
/// recorded at a start, #3616).
pub(super) fn record_to(
    root: &Path,
    local: bool,
    graded: &RouterGraded,
    device: impl FnOnce() -> String,
) -> Option<PathBuf> {
    let row = serde_json::to_value(graded).ok()?;
    let now = chrono::Utc::now();
    if local {
        crate::appstate::made(root).ok()?;
        let path = local_dir(root)?.join(format!("{}.jsonl", now.format("%Y-%m")));
        return super::append_local(&path, &row).ok();
    }
    if crate::compat::read_only(root).is_some() {
        return None;
    }
    crate::dispatch::append(&path_for(root, now, &device()), root, &row)
}

/// **The grades of the router `router` chosen by `workspace` or `persona`** in the project at
/// `root`, newest first, at most [`MOST_GRADES`]: the committed record's and this machine's. A
/// grade of the same router in another scope is of another library, and is not one of these. A
/// row that says more than purlis writes is skipped.
pub fn grades_of(
    root: &Path,
    router: &str,
    workspace: Option<&str>,
    persona: Option<&str>,
) -> Vec<RouterGraded> {
    grades_at(root, router, workspace, persona, chrono::Utc::now())
}

fn grades_at(
    root: &Path,
    router: &str,
    workspace: Option<&str>,
    persona: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<RouterGraded> {
    let mut files = record_files(&dir(root));
    if let Some(local) = local_dir(root) {
        files.extend(record_files(&local));
    }
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    let mut out: Vec<RouterGraded> = Vec::new();
    for file in files {
        if out.len() >= MOST_GRADES {
            break;
        }
        let Ok(text) = file_text(&file) else { continue };
        for line in crate::mdsection::split_lines(&text) {
            let Some(row) = (line.len() <= MOST_LINE_BYTES)
                .then(|| serde_json::from_str::<RouterGraded>(line).ok())
                .flatten()
            else {
                continue;
            };
            if row.router == router
                && row.workspace.as_deref() == workspace
                && row.persona.as_deref() == persona
                && row.sound(now)
            {
                out.push(row);
            }
        }
    }
    out.sort_by(|a, b| b.ts.cmp(&a.ts));
    out.truncate(MOST_GRADES);
    out
}

#[cfg(test)]
mod tests;
