// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Grade after each edit, as an opt-in** (SK-58, #3238; spec #2762 decision 22, "optionally
//! after each edit, never on a schedule"; ADR 0105 §4).
//!
//! A grade spends the person's account, so grading after each edit is **off for every skill
//! until the person turns it on for that skill, on this machine, from a grade's plan**: the plan
//! shows the chats it runs and the dollar limit it is held to, and turning it on keeps exactly
//! those as what the person accepted ([`Accepted`]). Each saved edit after that grades the skill
//! on the profile and model accepted, held to the same cap and dollar limit as a grade pressed by
//! hand, and only while what it would run stays within what was accepted ([`next`]): a grade that
//! would run more chats, or may spend more, is not started, and the opt-in waits until the
//! person sees the cost again and turns it on again. Nothing grades on a timer, and an edit
//! whose copy was graded already is not graded twice.
//!
//! **Tier:** Machine, device-bound. It lives in this machine's app state of the project,
//! `<data>/app-state/<clone key>/skill-grade-after-edit.json` ([`FILE`], `crate::appstate`),
//! which no chat writes and no commit carries: the person who pays turns it on, and a
//! teammate's push never does. Read through no link, at most [`MOST_FILE_BYTES`], and a record
//! that does not hold together is read as off.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use super::{Level, cap, plain};

/// The file in this machine's app state of the project that holds each skill's opt-in.
pub const FILE: &str = "skill-grade-after-edit.json";

/// The version of the file this purlis writes and reads.
const VERSION: u32 = 1;

/// The most bytes of the file read: far past any machine's opt-ins.
const MOST_FILE_BYTES: u64 = 256 * 1024;

/// The most skills one project keeps an opt-in for.
const MOST_SKILLS: usize = 512;

/// The most chats an opt-in may have accepted: past any plan's.
const MOST_CHATS: u32 = 100_000;

/// **What the person accepted when they turned grading after each edit on**: the profile and
/// model its chats run on, the chats a grade of it ran then, and the cap and dollar limit it was
/// held to, as the plan they pressed it on said them. A grade after an edit runs only within
/// them ([`next`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Accepted {
    pub profile: String,
    /// The model picked, or `None` for the profile's own.
    #[serde(default)]
    pub model: Option<String>,
    pub chats: u32,
    pub token_cap: u64,
    pub limit_usd: f64,
    /// **The trigger tests the plan ran, by digest** (the review of #3238): a grade after an edit
    /// runs only these tests, so a test a chat could add changes nothing the person did not see.
    pub tests: String,
    /// **The outcome evals the plan ran and the files they stage, by digest** (SK-55): `None`
    /// where it ran none, as an opt-in kept before them says. A grade after an edit runs only
    /// these, so an eval a chat or a pull changes, or adds, runs nothing the person did not see.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcomes: Option<String>,
    /// **The outcome evals the person ticked on the plan**, by id: a grade after an edit runs
    /// these and no other, whatever the skill's evals file gains.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evals: Vec<u64>,
    /// **The version the plan compared with when it was turned on.** A grade after an edit
    /// compares with the version before that edit (#4016): each saved edit is committed, so that
    /// version moves with every edit and is not held to this one. A previous version's arm that
    /// the plan did not show runs more chats, and waits for the person as any growth does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
    /// **The model its chats ran on as the plan showed it** (#3622): the one their line names,
    /// where it names one, else the one picked. A grade after an edit whose chats would run on
    /// another waits for the person, as the plan's consent covers it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runs_on: Option<String>,
    /// When it was turned on.
    pub at: String,
    /// What the copy held when an edit last graded it, by its plan's consent: a save that
    /// changed nothing since is not graded again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graded: Option<String>,
    /// Why it waits for the person, where it does: a grade after an edit would have run more
    /// than they accepted, so it was not started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused: Option<String>,
}

impl Accepted {
    /// Whether it says only what purlis writes.
    fn sound(&self) -> bool {
        plain(&self.profile)
            && self.model.as_deref().is_none_or(plain)
            && self.runs_on.as_deref().is_none_or(super::price::model_id)
            && (1..=MOST_CHATS).contains(&self.chats)
            && (cap::LEAST..=cap::MOST).contains(&self.token_cap)
            && self.limit_usd.is_finite()
            && self.limit_usd > 0.0
            // No more than the largest cap at the dearest price a row may name.
            && self.limit_usd
                <= cap::MOST as f64 * super::price::MOST_READ_USD_PER_MILLION / 1_000_000.0
            && hex64(&self.tests)
            && self.outcomes.as_deref().is_none_or(hex64)
            && self.previous.as_deref().is_none_or(|commit| {
                matches!(commit.len(), 40 | 64) && commit.bytes().all(|b| b.is_ascii_hexdigit())
            })
            && self
                .graded
                .as_deref()
                .is_none_or(|word| word.len() == 64 && word.bytes().all(|b| b.is_ascii_hexdigit()))
            && self
                .paused
                .as_deref()
                .is_none_or(|why| why.len() <= 1024 && !why.chars().any(char::is_control))
    }
}

/// Whether `word` is a SHA-256 digest in hex.
fn hex64(word: &str) -> bool {
    word.len() == 64 && word.bytes().all(|b| b.is_ascii_hexdigit())
}

/// **The digest of a grade's trigger tests**, as an opt-in pins them.
pub fn tests_of(request: &purlis_skills::grade::GradeRequest) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(serde_json::to_vec(&request.triggers).unwrap_or_default())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// **The digest of a grade's outcome evals and of the input files they stage**, as an opt-in pins
/// them (SK-55): `None` where it runs none.
pub fn outcomes_of(planned: &super::Planned) -> Option<String> {
    use sha2::Digest as _;
    let request = planned.request.as_ref()?;
    if request.outcomes.is_empty() {
        return None;
    }
    let mut digest = sha2::Sha256::new();
    digest.update(serde_json::to_vec(&request.outcomes).unwrap_or_default());
    if let Some(copy) = &planned.copy {
        for (path, text) in &copy.inputs {
            if request
                .outcomes
                .iter()
                .any(|eval| eval.files.contains(path))
            {
                digest.update(path.as_bytes());
                digest.update(b"\0");
                digest.update(text.as_bytes());
                digest.update(b"\0");
            }
        }
    }
    Some(
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

/// **The chats the plan that turns grading after each edit on shows and keeps** (#4035): the
/// chats of `plan`, with its previous version's arm counted where it has none yet and its edits
/// are committed (`committed`: a copy git holds, not one only on this machine). Each saved edit
/// of such a copy is committed, so a skill with one version gains a previous version at its
/// first edit, and every grade after an edit then runs that arm too. Its cap and dollar limit
/// stay the plan's: they hold each grade whole, however many chats it runs.
pub fn chats_to_accept(plan: &purlis_skills::grade::Plan, committed: bool) -> u32 {
    if !committed {
        return plan.chats;
    }
    let arms = u32::try_from(super::ARMS_WITH_PREVIOUS.len()).unwrap_or(u32::MAX);
    plan.chats.max(plan.per_arm.saturating_mul(arms))
}

/// The file, as it is written.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Kept {
    v: u32,
    #[serde(default)]
    skills: BTreeMap<String, Accepted>,
}

/// Where the file of the project at `root` is: `None` where this machine has no data home.
fn path_of(root: &Path) -> Option<PathBuf> {
    Some(crate::appstate::dir(root)?.join(FILE))
}

/// **The copy a skill's opt-in is kept by**: its level's folder relative to the project, and its
/// name (`skills/deploy`). `None` for a copy no folder of the project holds.
pub fn key(level: &Level, name: &str) -> Option<String> {
    level.rel().map(|rel| format!("{rel}/{name}"))
}

/// The file as it reads: nothing where it is absent, not a file of its own, too large, or not
/// what purlis writes.
fn read(root: &Path) -> Kept {
    let Some(path) = path_of(root) else {
        return Kept::default();
    };
    let Ok(meta) = std::fs::symlink_metadata(&path) else {
        return Kept::default();
    };
    if !meta.is_file() || meta.len() > MOST_FILE_BYTES {
        return Kept::default();
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let Ok(file) = crate::contain::nofollow(&mut options).open(&path) else {
        return Kept::default();
    };
    let mut text = String::new();
    if file
        .take(MOST_FILE_BYTES)
        .read_to_string(&mut text)
        .is_err()
    {
        return Kept::default();
    }
    match serde_json::from_str::<Kept>(&text) {
        Ok(kept) if kept.v == VERSION && kept.skills.len() <= MOST_SKILLS => Kept {
            v: VERSION,
            skills: kept
                .skills
                .into_iter()
                .filter(|(_, accepted)| accepted.sound())
                .collect(),
        },
        _ => Kept::default(),
    }
}

/// Writes `kept` as the file in the project's app state `dir`, whole or not at all, private,
/// through no link.
fn write(dir: &Path, kept: &Kept) -> Result<(), String> {
    let path = dir.join(FILE);
    let text = serde_json::to_string_pretty(kept).map_err(|why| not_kept(&why.to_string()))?;
    crate::plane::write_private(dir, &path, text.as_bytes())
        .map_err(|why| not_kept(&why.to_string()))
}

/// The lock file beside [`FILE`] whose `flock` each change holds while it reads, changes and
/// writes the file: two changes at once (a grade after an edit keeping what it graded while the
/// person turns the opt-in off) take turns, so neither writes back what the other took away.
pub const LOCK: &str = "skill-grade-after-edit.lock";

/// How long a change waits for another to let go of [`LOCK`].
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// Takes [`LOCK`] for one change of the file of the project at `root`: the lock, and the project's
/// app state the file is written in.
fn held(root: &Path) -> Result<(crate::filelock::Held, PathBuf), String> {
    let dir = crate::appstate::made(root).map_err(|why| not_kept(&why))?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let file = crate::contain::nofollow(&mut options)
        .open(dir.join(LOCK))
        .map_err(|why| not_kept(&why.to_string()))?;
    let held = crate::filelock::lock_within(file, LOCK_WAIT, "grading after each edit")
        .map_err(|why| not_kept(&why.to_string()))?;
    Ok((held, dir))
}

/// What a write that failed says.
fn not_kept(why: &str) -> String {
    format!(
        "Grading after each edit could not be kept on this machine ({}), so nothing was changed.",
        crate::shown::short(why)
    )
}

/// **The opt-in of the copy of `name` at `level`** in the project at `root`, where it is on.
pub fn of(root: &Path, level: &Level, name: &str) -> Option<Accepted> {
    let key = key(level, name)?;
    read(root).skills.remove(&key)
}

/// **Turns grading after each edit on for the copy of `name` at `level`**, with `accepted`: what
/// the plan the person pressed it on showed. Refused for a copy no folder holds, and for what
/// purlis would not read back.
pub fn turn_on(root: &Path, level: &Level, name: &str, accepted: Accepted) -> Result<(), String> {
    let key = key(level, name).ok_or_else(|| {
        format!("{name} is no copy of this project's, so it cannot be graded after each edit.")
    })?;
    if !accepted.sound() {
        return Err(format!(
            "What a grade of {name} runs could not be kept as accepted, so grading after each \
             edit was not turned on."
        ));
    }
    let (_held, dir) = held(root)?;
    let mut kept = read(root);
    if !kept.skills.contains_key(&key) && kept.skills.len() >= MOST_SKILLS {
        return Err(format!(
            "This project grades {MOST_SKILLS} skills after each edit already: turn one off \
             first."
        ));
    }
    kept.v = VERSION;
    kept.skills.insert(key, accepted);
    write(&dir, &kept)
}

/// **Turns grading after each edit off for the copy of `name` at `level`**: nothing where it was
/// off.
pub fn turn_off(root: &Path, level: &Level, name: &str) -> Result<(), String> {
    let Some(key) = key(level, name) else {
        return Ok(());
    };
    let (_held, dir) = held(root)?;
    let mut kept = read(root);
    if kept.skills.remove(&key).is_none() {
        return Ok(());
    }
    kept.v = VERSION;
    write(&dir, &kept)
}

/// **Changes the opt-in of the copy of `name` at `level` by `change`**, where it is on: what a
/// grade after an edit keeps (what it graded, or why it waits). Nothing where it is off.
pub fn update(
    root: &Path,
    level: &Level,
    name: &str,
    change: impl FnOnce(&mut Accepted),
) -> Result<(), String> {
    let Some(key) = key(level, name) else {
        return Ok(());
    };
    let (_held, dir) = held(root)?;
    let mut kept = read(root);
    let Some(accepted) = kept.skills.get_mut(&key) else {
        return Ok(());
    };
    change(accepted);
    kept.v = VERSION;
    write(&dir, &kept)
}

/// **Carries the opt-in of the copy of `old` at `from` to the copy it is now, `new` at `to`**
/// (#3612): what a rename, a move between levels and a share do, so grading after each edit
/// follows the skill. What the person accepted goes with it unchanged, and it still grades only
/// within that ([`next`]): where the plan of the copy's new place runs more chats, may spend more,
/// runs other tests, compares with another version or runs on another profile or model, the next
/// edit waits for the person to see the plan again. Nothing where it was off.
pub fn moved(root: &Path, from: &Level, old: &str, to: &Level, new: &str) -> Result<(), String> {
    let (Some(was), Some(is)) = (key(from, old), key(to, new)) else {
        return Ok(());
    };
    // Read before the lock is taken, so a change of a skill never graded makes no state.
    if was == is || of(root, from, old).is_none() {
        return Ok(());
    }
    let (_held, dir) = held(root)?;
    let mut kept = read(root);
    let Some(accepted) = kept.skills.remove(&was) else {
        return Ok(());
    };
    kept.v = VERSION;
    kept.skills.insert(is, accepted);
    write(&dir, &kept)
}

/// **Drops the opt-in of the copy of `name` at `level`** (#3612): what its delete does. A skill
/// brought back is graded after each edit again only once the person turns it on again, from
/// its plan. Nothing where it was off.
pub fn dropped(root: &Path, level: &Level, name: &str) -> Result<(), String> {
    if of(root, level, name).is_none() {
        return Ok(());
    }
    turn_off(root, level, name)
}

/// **What a grade after an edit would run now**: the chats of the copy's plan, the cap and the
/// dollar limit it would be held to, its trigger tests, and its consent, the digest of what it
/// would grade.
#[derive(Debug, Clone, PartialEq)]
pub struct Now {
    /// The profile and the model its chats would run on (#3622).
    pub profile: String,
    pub runs_on: Option<String>,
    pub chats: u32,
    pub token_cap: u64,
    pub limit_usd: f64,
    pub tests: String,
    pub outcomes: Option<String>,
    pub consent: String,
    /// Whether it compares with a previous version (#4035): what a grade of a skill with one
    /// version gains at its first saved edit.
    pub previous: bool,
}

impl Now {
    /// What a grade of `planned` would run on `terms` (its profile, the model its chats run on,
    /// and `limit_usd`, the plan's cap at that model's price): `None` where it has nothing to
    /// grade.
    pub fn of(planned: &super::Planned, terms: &super::Terms<'_>) -> Option<Now> {
        let request = planned.request.as_ref()?;
        let limit_usd = terms.limit_usd;
        Some(Now {
            profile: terms.profile?.to_owned(),
            runs_on: terms.model.map(str::to_owned),
            chats: planned.plan?.chats,
            token_cap: planned.cap.tokens,
            limit_usd,
            tests: tests_of(request),
            outcomes: outcomes_of(planned),
            consent: planned.consent.clone()?,
            previous: planned.previous.is_some(),
        })
    }
}

/// What a saved edit does under an opt-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// It grades the copy.
    Grade,
    /// The copy holds what an edit last graded: nothing runs.
    Same,
    /// It waits for the person, and why: what it would run is past what they accepted, or it
    /// waits already.
    Waits(String),
}

/// **What a saved edit does under `accepted`, with `now` what a grade would run**: it grades
/// only while the grade runs no more chats, and is held to no larger a cap or dollar limit,
/// than the person accepted; never twice for the same copy; and not while the opt-in waits for
/// the person.
pub fn next(accepted: &Accepted, now: &Now) -> Next {
    if let Some(why) = &accepted.paused {
        return Next::Waits(why.clone());
    }
    if accepted.graded.as_deref() == Some(now.consent.as_str()) {
        return Next::Same;
    }
    if now.profile != accepted.profile || now.runs_on != accepted.runs_on {
        return Next::Waits(
            "The profile or the model a grade of it runs on changed since you turned grading \
             after each edit on, so no grade was started after your edit: see the plan again and \
             turn it on again."
                .to_owned(),
        );
    }
    if now.tests != accepted.tests {
        return Next::Waits(
            "Its trigger tests changed since you turned grading after each edit on, so no grade \
             was started after your edit: see the plan again and turn it on again."
                .to_owned(),
        );
    }
    if now.outcomes != accepted.outcomes {
        return Next::Waits(
            "Its outcome evals changed since you turned grading after each edit on, so no grade \
             was started after your edit: see the plan again and turn it on again."
                .to_owned(),
        );
    }
    // The version it compares with is the one before the edit (#4016): each saved edit is
    // committed and moves it, so it is not held to the one the plan showed. The plan that turns
    // it on counts that version's arm (#4035, [`chats_to_accept`]); an opt-in that accepted
    // fewer chats waits below, and says why where that arm is what grew.
    if now.chats > accepted.chats && now.previous && accepted.previous.is_none() {
        return Next::Waits(format!(
            "Your edit made a version of it to compare with, so a grade of it now also runs its \
             previous version: {}, more than the {} you accepted. No grade was started after \
             your edit: see the cost again and turn grading after each edit on again.",
            crate::counts::counted(now.chats, "chat", "chats"),
            accepted.chats
        ));
    }
    if now.chats > accepted.chats {
        return Next::Waits(format!(
            "A grade of it now runs {}, more than the {} you accepted, so no grade was started \
             after your edit: see the cost again and turn grading after each edit on again.",
            crate::counts::counted(now.chats, "chat", "chats"),
            accepted.chats
        ));
    }
    // A cent's rounding past what was shown is still what was shown.
    if now.token_cap > accepted.token_cap || now.limit_usd > accepted.limit_usd + 0.005 {
        return Next::Waits(format!(
            "A grade of it may now spend up to ${:.2} ({}), more than the ${:.2} you accepted, so \
             no grade was started after your edit: see the cost again and turn grading after \
             each edit on again.",
            now.limit_usd,
            crate::counts::counted(now.token_cap, "token", "tokens"),
            accepted.limit_usd
        ));
    }
    Next::Grade
}

#[cfg(test)]
mod tests;
