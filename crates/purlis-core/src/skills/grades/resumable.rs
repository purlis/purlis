// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A stopped grade, kept so it can be resumed** (#5718, the operator's rulings of 2026-11-08).
//!
//! A grade that stops short of its plan (at its token cap or dollar limit, at the program's usage
//! limit, with no network, or by the person's Stop) keeps what each of its chats did, in its
//! place ([`grade::Kept`]), and what decided its result ([`Deciding`]). **Resume** then runs only
//! the cases it did not finish and joins them to the kept ones in one scorecard
//! ([`grade::resume`]).
//!
//! - **Offered only while nothing that decides the result has changed** ([`Deciding::changed`]):
//!   the skill's text, its tests, the profile its chats run on, the model, and the grade's
//!   settings. Otherwise only Grade again is offered, saying what changed.
//! - **Its own cap**: a resume is planned and confirmed like any grade ([`Plan`]), its cap
//!   counting only what it spends, and its report says both runs' spend.
//! - **Never by itself**: it spends money, so it waits for the person's press.
//! - **One per copy**: a new grade of the copy (or of its draft) forgets the stopped one
//!   ([`forget_of`]), so Resume always goes on from the copy's latest grade.
//!
//! Kept in this machine's app state of the project ([`crate::appstate`]),
//! `<data>/app-state/<clone key>/skill-grade-resume/<grade>.json`: the chats ran here, and their
//! kept screens are this machine's too. Read as this machine wrote it: a plain name, a file of its
//! own followed through no link, at most [`MOST_BYTES`].
//!
//! **Tier:** Machine, device-bound, transient: forgotten once the grade is resumed or the copy is
//! graded again; losing it loses only the Resume button.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use purlis_skills::grade::{self, GradeEnd, GradeRequest, Kept};

use super::{SkillCopy, Snapshot, cap, draft, plain};

/// The folder of this machine's app state of the project the stopped grades are kept in.
pub const DIR_NAME: &str = "skill-grade-resume";

/// The version of a kept stopped grade this purlis writes and reads.
pub const VERSION: u32 = 1;

/// The most bytes of one kept stopped grade read.
pub const MOST_BYTES: u64 = 16 * 1024 * 1024;

/// **A stopped grade, as it is kept to resume** (#5718).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Stopped {
    pub v: u32,
    /// The grade's id, as its row in the record has it.
    pub grade: String,
    pub skill: String,
    /// The copy's level, as its row records it.
    pub level: Option<String>,
    /// The draft it graded, where it graded one: its mark, as its row has it, and its id, as the
    /// drafts are found by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<draft::Mark>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_of: Option<String>,
    /// What it asked, and what each of its chats did.
    pub request: GradeRequest,
    pub kept: Kept,
    /// What decided its result as it started.
    pub deciding: Deciding,
    /// How it stopped, and when the program's usage limit resets, where it stopped there.
    pub ended: GradeEnd,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<String>,
    /// When it stopped.
    pub ts: String,
}

impl Stopped {
    /// The chats a resume of it would run.
    pub fn left(&self) -> u32 {
        self.kept.left(&self.request)
    }

    /// **The chats it ran, counted as an estimate shares what it spent over** (#5718): each
    /// trigger test's chat it asked for, and each outcome eval's and its judge's.
    fn asked(&self) -> u64 {
        let tests: usize = self.kept.triggers.iter().flatten().map(Vec::len).sum();
        let evals: u64 = self
            .kept
            .outcomes
            .iter()
            .flatten()
            .flatten()
            .map(|one| u64::from(one.asked))
            .sum();
        tests as u64 + evals
    }
}

/// **What decides a grade's result** (#5718, the operator's ruling 2 of 2026-11-08), each as a
/// digest, so a resume is offered only while each is what it was.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Deciding {
    /// The skill's own files as they were read (its fixtures apart), the content its folder held,
    /// and the version it compares with.
    pub skill: String,
    /// Its trigger tests and the outcome evals ticked, with the files they stage.
    pub tests: String,
    /// The profile its chats run on, the program (its harness) that profile starts, and the
    /// model picked (the profile's own where none).
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    pub model: Option<String>,
    /// The grade's settings: its arms, its runs, its chats' shell, and its Grader.
    pub settings: String,
}

/// **What a grade's chats run on**: the profile, the program it starts (its harness's name), and
/// the model picked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct On<'a> {
    pub profile: Option<&'a str>,
    pub program: Option<&'a str>,
    pub model: Option<&'a str>,
}

/// **What changed of what decides a grade's result**, each as a sentence's part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Changed {
    Skill,
    Tests,
    Program,
    Model,
    Settings,
}

impl Changed {
    /// What it is, as a sentence names it.
    pub fn said(self) -> &'static str {
        match self {
            Self::Skill => "the skill's text",
            Self::Tests => "its tests",
            Self::Program => "the program its chats run on",
            Self::Model => "the model",
            Self::Settings => "the grade's settings",
        }
    }
}

impl Deciding {
    /// **What decides a grade of `request`, of the copy `copy`, run `on` a profile, its program
    /// and a model, by the Grader `grader`** (its id; `None` for purlis's own runner).
    pub fn of(request: &GradeRequest, copy: &SkillCopy, on: On<'_>, grader: Option<&str>) -> Self {
        use sha2::Digest as _;
        let fixture = |path: &str| path.starts_with(&format!("{}/", grade::EVALS_DIR));
        let files = |digest: &mut sha2::Sha256, snapshot: &Snapshot| {
            for (path, text) in snapshot.files.iter().filter(|(path, _)| !fixture(path)) {
                super::digest_input(digest, path, text.as_bytes());
            }
        };
        let mut skill = sha2::Sha256::new();
        skill.update(copy.content.word().as_bytes());
        files(&mut skill, &copy.snapshot);
        if let Some(previous) = &copy.previous {
            skill.update(b"\0previous\0");
            skill.update(previous.commit.as_deref().unwrap_or_default().as_bytes());
            files(&mut skill, &previous.snapshot);
        }
        let mut tests = sha2::Sha256::new();
        tests.update(serde_json::to_vec(&request.triggers).unwrap_or_default());
        tests.update(b"\0");
        tests.update(serde_json::to_vec(&request.outcomes).unwrap_or_default());
        for (path, bytes) in copy.inputs.iter().filter(|(path, _)| {
            request
                .outcomes
                .iter()
                .any(|eval| eval.files.contains(*path))
        }) {
            super::digest_input(&mut tests, path, bytes);
        }
        let mut settings = sha2::Sha256::new();
        settings.update(serde_json::to_vec(&request.arms).unwrap_or_default());
        settings.update([request.runs, u8::from(request.shell)]);
        settings.update(grader.unwrap_or_default().as_bytes());
        Self {
            skill: super::hex_of(skill),
            tests: super::hex_of(tests),
            profile: on.profile.map(str::to_owned),
            program: on.program.map(str::to_owned),
            model: on.model.map(str::to_owned),
            settings: super::hex_of(settings),
        }
    }

    /// **What of `now` is not what it was**, in the order a sentence names them: none where a
    /// resume may go on.
    pub fn changed(&self, now: &Deciding) -> Vec<Changed> {
        [
            (self.skill != now.skill, Changed::Skill),
            (self.tests != now.tests, Changed::Tests),
            (
                self.profile != now.profile || self.program != now.program,
                Changed::Program,
            ),
            (self.model != now.model, Changed::Model),
            (self.settings != now.settings, Changed::Settings),
        ]
        .into_iter()
        .filter_map(|(differs, what)| differs.then_some(what))
        .collect()
    }
}

/// **Why Resume is not offered, in a sentence**: what changed, joined as a person says a list.
pub fn changed_said(changed: &[Changed]) -> Option<String> {
    let parts: Vec<&str> = changed.iter().map(|one| one.said()).collect();
    let list = match parts.as_slice() {
        [] => return None,
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    Some(format!(
        "Resume is not offered: {list} changed since it stopped. Grade again runs it whole."
    ))
}

/// **What resuming a stopped grade would run, before it runs** (#5718, the operator's ruling 3):
/// the chats left, what they would spend from what its chats spent per chat so far, and the cap
/// it is held to, which counts only its own spend; and what the stopped grade spent, so the
/// scorecard can say both runs' together.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Plan {
    pub left: u32,
    /// The tokens and dollars the chats left would spend, at what the stopped grade's chats spent
    /// each; `None` where they did not say.
    pub tokens_left: Option<u64>,
    pub usd_left: Option<f64>,
    pub cap: cap::Cap,
    pub earlier: grade::Earlier,
}

/// **The plan of resuming `stopped`, held to `cap`.**
pub fn plan(stopped: &Stopped, cap: cap::Cap) -> Plan {
    let left = stopped.left();
    let asked = stopped.asked();
    let earlier = stopped.kept.earlier;
    let per = |all: f64| (asked > 0).then(|| all / asked as f64 * f64::from(left));
    Plan {
        left,
        tokens_left: earlier
            .tokens
            .and_then(|tokens| per(tokens as f64))
            .map(|tokens| tokens.ceil() as u64),
        usd_left: earlier.usd.and_then(per),
        cap,
        earlier,
    }
}

/// **What pressing Resume on `plan` of `stopped` consents to**: the grade it goes on from, what
/// decides it now (`deciding`), the chats left and the limits `terms` shows, in one digest. A press
/// made on a plan that no longer holds is refused.
pub fn consent_of(
    stopped: &Stopped,
    deciding: &Deciding,
    plan: &Plan,
    terms: &super::Terms<'_>,
) -> String {
    use sha2::Digest as _;
    let mut digest = sha2::Sha256::new();
    digest.update(stopped.grade.as_bytes());
    digest.update(b"\0");
    digest.update(serde_json::to_vec(deciding).unwrap_or_default());
    digest.update(plan.left.to_be_bytes());
    super::consent_on(&super::hex_of(digest), terms)
}

/// The folder the stopped grades of the project at `root` are kept in, or `None` where this
/// machine has no data home.
fn dir(root: &Path) -> Option<PathBuf> {
    Some(crate::appstate::dir(root)?.join(DIR_NAME))
}

/// The file `grade` is kept in, where its id is a plain name.
fn path_of(root: &Path, grade: &str) -> Option<PathBuf> {
    (plain(grade) && !grade.bytes().all(|b| b == b'.'))
        .then(|| dir(root).map(|dir| dir.join(format!("{grade}.json"))))
        .flatten()
}

/// **Keeps `stopped`** in the project at `root`: written whole beside its place, then renamed over
/// it, private, following no link.
pub fn keep(root: &Path, stopped: &Stopped) -> std::io::Result<PathBuf> {
    crate::appstate::made(root).map_err(std::io::Error::other)?;
    let path = path_of(root, &stopped.grade)
        .ok_or_else(|| std::io::Error::other("the grade's id is not a plain name"))?;
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("the grade has no folder"))?;
    std::fs::create_dir_all(dir)?;
    if !std::fs::symlink_metadata(dir)?.is_dir() {
        return Err(std::io::Error::other("the folder is not a folder"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let text = serde_json::to_vec(stopped).map_err(std::io::Error::other)?;
    if text.len() as u64 > MOST_BYTES {
        return Err(std::io::Error::other(
            "the stopped grade is too large to keep",
        ));
    }
    let partial = dir.join(format!(".{}.json.partial", stopped.grade));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let written = (|| {
        let mut file = crate::contain::nofollow(&mut options).open(&partial)?;
        file.write_all(&text)?;
        file.sync_all()?;
        std::fs::rename(&partial, &path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    written.map(|()| path)
}

/// **The stopped grade `grade`** of the project at `root`, where this machine kept it and it
/// reads as this purlis writes it.
pub fn of(root: &Path, grade: &str) -> Option<Stopped> {
    let path = path_of(root, grade)?;
    let meta = std::fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MOST_BYTES {
        return None;
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let file = crate::contain::nofollow(&mut options).open(&path).ok()?;
    let mut bytes = Vec::new();
    file.take(MOST_BYTES).read_to_end(&mut bytes).ok()?;
    let stopped: Stopped = serde_json::from_slice(&bytes).ok()?;
    (stopped.v <= VERSION && stopped.grade == grade && stopped.ended.resumable()).then_some(stopped)
}

/// **Forgets the stopped grade `grade`**: it was resumed.
pub fn forget(root: &Path, grade: &str) {
    if let Some(path) = path_of(root, grade) {
        let _ = std::fs::remove_file(path);
    }
}

/// The most kept files looked at as a new grade forgets its copy's.
const MOST_FILES: usize = 600;

/// **Every stopped grade this machine keeps of the project at `root`**, newest first: what
/// Needs you lists, each saying when Resume can run.
pub fn all(root: &Path) -> Vec<Stopped> {
    let Some(dir) = dir(root) else {
        return Vec::new();
    };
    if !std::fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) {
        return Vec::new();
    }
    let Ok(reader) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut stopped: Vec<Stopped> = reader
        .take(MOST_FILES)
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            of(root, name.strip_suffix(".json")?)
        })
        .collect();
    stopped.sort_by(|a, b| b.ts.cmp(&a.ts));
    stopped
}

/// **Forgets every stopped grade of the copy of `skill` recorded at `level`, or of its draft where
/// `draft`** (a draft's mint): a new grade of it starts, and Resume goes on from that one.
pub fn forget_of(root: &Path, skill: &str, level: Option<&str>, draft: Option<&str>) {
    let Some(dir) = dir(root) else {
        return;
    };
    if !std::fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) {
        return;
    }
    let Ok(reader) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in reader.take(MOST_FILES).filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(grade) = name.strip_suffix(".json") else {
            continue;
        };
        if let Some(stopped) = of(root, grade)
            && stopped.skill == skill
            && stopped.level.as_deref() == level
            && stopped.draft.as_ref().map(|mark| mark.id.as_str()) == draft
        {
            forget(root, grade);
        }
    }
}

#[cfg(test)]
mod tests;
