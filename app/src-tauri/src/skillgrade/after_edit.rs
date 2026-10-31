// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Grade after each edit, in a skill's tab** (SK-58, #3238; spec #2762 decision 22; ADR 0105
//! as amended 2026-10-28): the window's half of `purlis_core::skills::grades::after_edit`.
//!
//! - **Off until the person turns it on, from a plan** ([`skill_grade_after_edit_on`]): the plan
//!   they read shows the chats and the dollar limit, its consent is checked as Grade's is, and
//!   exactly those are kept as accepted. Turning it off ([`skill_grade_after_edit_off`]) stops a
//!   grade an edit started.
//! - **Only the editor's save counts** ([`saved`], called by `skill_save` once the edit is
//!   written): a save from the command line, a chat's change or a pull grades nothing. Once
//!   [`QUIET`] passes with no other save of the skill it grades the skill on the accepted profile
//!   and model, through the same start a press of Grade takes, and only within what was accepted
//!   (`after_edit::next`). A second save while that grade runs stops it, and the next starts after
//!   it; a grade pressed by hand is waited for, never stopped.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use specta::Type;
use tauri::Emitter as _;

use purlis_core::skills::grades::{self, after_edit};
use purlis_core::skills::verbs::Level;

use super::{Begin, On, begin, chats_on, running};
use crate::planes::{Held, PlaneId, Planes};
use crate::skillchange::{SkillLevel, flag_on};

/// How long a skill's editor must go without another save before the last one grades it: a
/// burst of saves grades once.
pub const QUIET: Duration = Duration::from_secs(10);

/// The longest a grade after an edit waits for another grade in the project to end before it
/// gives up and says so.
const MOST_WAIT: Duration = Duration::from_secs(60 * 60);

/// How often a waiting grade after an edit looks again.
const LOOK_EVERY: Duration = Duration::from_millis(500);

/// The event the window is told by when a saved edit did not grade a skill whose opt-in is on,
/// and why.
pub const EVENT: &str = "skill-grade-after-edit";

/// **A skill's opt-in, as its tab says it**: what the person accepted, and why it waits where it
/// does.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SkillGradeAfterEdit {
    pub profile: String,
    /// The model picked, or `None` for the profile's own.
    pub model: Option<String>,
    /// The most chats each grade runs, and the cap and dollar limit each is held to.
    pub chats: u32,
    pub token_cap: f64,
    pub limit_usd: f64,
    /// Why a saved edit does not grade it now: what a grade would run is past what was accepted.
    pub waits: Option<String>,
}

impl From<after_edit::Accepted> for SkillGradeAfterEdit {
    fn from(accepted: after_edit::Accepted) -> SkillGradeAfterEdit {
        SkillGradeAfterEdit {
            profile: accepted.profile,
            model: accepted.model,
            chats: accepted.chats,
            token_cap: accepted.token_cap as f64,
            limit_usd: accepted.limit_usd,
            waits: accepted.paused,
        }
    }
}

/// **What a saved edit did not grade, and why**, as [`EVENT`] tells the window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillGradeAfterEditStep {
    pub plane: PlaneId,
    pub skill: String,
    /// The copy, by its level (#3978): a same-named copy at another level is another skill.
    pub level: SkillLevel,
    pub said: String,
}

/// **The opt-in of the copy of `skill` at `level`**, where it is on.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grade_after_edit(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    skill: String,
) -> Result<Option<SkillGradeAfterEdit>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a skill's grading after each edit", move || {
        flag_on()?;
        Ok(after_edit::of(held.root(), &(&level).into(), &skill).map(Into::into))
    })
    .await
}

/// **Turns grading after each edit on for the copy of `skill` at `level`**, from the plan the
/// person read: on `profile` and `model`, as Grade would run, and only while that plan still
/// holds (`consent`). What it keeps as accepted is that plan's chats, cap and dollar limit.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grade_after_edit_on(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    skill: String,
    profile: Option<String>,
    model: Option<String>,
    consent: String,
) -> Result<SkillGradeAfterEdit, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("turning grading after each edit on", move || {
        flag_on()?;
        let level: Level = (&level).into();
        let root = held.root();
        let on = chats_on(root, profile, model);
        let planned = grades::planned(root, &level, &skill, on.profile.as_deref())?;
        let accepted = accepted_of(&planned, on, &consent, &skill)?;
        after_edit::turn_on(root, &level, &skill, accepted.clone())?;
        Ok(accepted.into())
    })
    .await
}

/// **What the person accepts by turning grading after each edit on from `planned`**, on `on`:
/// the plan's chats, its cap and its dollar limit there. Refused where the plan no longer holds
/// (`consent`), has nothing to grade, or cannot run on that profile.
fn accepted_of(
    planned: &grades::Planned,
    on: On,
    consent: &str,
    skill: &str,
) -> Result<after_edit::Accepted, String> {
    // The plan the person read, whole: what it grades and the profile, model and limits it
    // shows (#3622).
    super::plan_holds(
        planned,
        &on,
        consent,
        skill,
        "grading after each edit was not turned on",
    )?;
    let runs_on = super::terms_of(planned, &on).model.map(str::to_owned);
    let Some(plan) = planned.plan else {
        return Err(format!(
            "{skill} has no trigger tests to grade, so grading after each edit was not turned on."
        ));
    };
    on.checked?;
    let profile = on
        .profile
        .ok_or("No profile was found, so grading after each edit was not turned on.")?;
    let request = planned.request.as_ref().ok_or_else(|| {
        format!(
            "{skill} has no trigger tests to grade, so grading after each edit was not turned on."
        )
    })?;
    Ok(after_edit::Accepted {
        profile,
        model: on.model,
        // With its previous version's arm counted, as the plan shows it (#4035).
        chats: planned.after_edit_chats.unwrap_or(plan.chats),
        token_cap: planned.cap.tokens,
        limit_usd: planned.cap.cost_usd(&on.priced),
        tests: after_edit::tests_of(request),
        previous: planned.previous.as_ref().map(|one| one.commit.clone()),
        runs_on,
        at: purlis_core::dispatch::stamp(chrono::Utc::now()),
        graded: None,
        paused: None,
    })
}

/// **Turns grading after each edit off for the copy of `skill` at `level`**, and stops a grade an
/// edit of it started.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grade_after_edit_off(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    skill: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("turning grading after each edit off", move || {
        flag_on()?;
        let level: Level = (&level).into();
        after_edit::turn_off(held.root(), &level, &skill)?;
        if let Some(key) = after_edit::key(&level, &skill) {
            stop_after_edit(&plane, &key);
        }
        Ok(())
    })
    .await
}

/// Stops the grade in `plane` an edit of the copy `key` started (`after_edit::key`: its level
/// and name), where one runs: another level's copy of the same name is another skill.
fn stop_after_edit(plane: &PlaneId, key: &str) {
    for one in running().values() {
        if one.plane == *plane && one.after_edit.as_deref() == Some(key) {
            one.stop.store(true, Ordering::SeqCst);
        }
    }
}

/// **The saves waiting out [`QUIET`]**, by project and copy: the last save's time, and whether a
/// thread waits on it.
static BURSTS: std::sync::LazyLock<Mutex<HashMap<(PlaneId, String), Burst>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// **A burst of saves of one copy**: the last one's time. Its grade is due once [`QUIET`] passes
/// after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Burst {
    last: Instant,
}

impl Burst {
    /// A burst begun by a save at `at`.
    pub(crate) fn at(at: Instant) -> Burst {
        Burst { last: at }
    }

    /// Another save, at `at`: the grade is due [`QUIET`] after it.
    pub(crate) fn saved(&mut self, at: Instant) {
        self.last = self.last.max(at);
    }

    /// Whether a save of it came after `at`.
    pub(crate) fn saved_after(&self, at: Instant) -> bool {
        self.last > at
    }

    /// How long until its grade is due, from `now`: nothing once it is.
    pub(crate) fn left(&self, now: Instant) -> Duration {
        (self.last + QUIET).saturating_duration_since(now)
    }
}

/// **The editor saved an edit of the copy of `skill` at `level`** in the project `plane`: where
/// grading after each edit is on for it, a grade an edit started is stopped now, and once
/// [`QUIET`] passes with no other save, the copy is graded within what was accepted. Nothing
/// where it is off.
pub(crate) fn saved(
    app: tauri::AppHandle,
    held: Arc<Held>,
    plane: PlaneId,
    level: Level,
    skill: String,
) {
    if after_edit::of(held.root(), &level, &skill).is_none() {
        return;
    }
    let Some(key) = after_edit::key(&level, &skill) else {
        return;
    };
    // A second edit stops the grade the first one started: what ran is scored and recorded.
    stop_after_edit(&plane, &key);
    let burst = (plane.clone(), key);
    {
        let mut bursts = BURSTS.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(one) = bursts.get_mut(&burst) {
            one.saved(Instant::now());
            return;
        }
        bursts.insert(burst.clone(), Burst::at(Instant::now()));
    }
    let unheld = burst.clone();
    let spawned = std::thread::Builder::new()
        .name("purlis-grade-after-edit".to_owned())
        .spawn(move || waited_then_graded(&app, &held, &burst, &level, &skill));
    if let Err(why) = spawned {
        tracing::warn!("purlis: a grade after an edit could not wait for the edit ({why})");
        // Let go of, so the next save begins a burst a thread waits on.
        BURSTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&unheld);
    }
}

/// **The one thread of a burst of saves**: it waits until the saves go [`QUIET`] and no grade
/// runs in the project, then grades the copy as it is then. The burst is let go of only once a
/// grade started with no save since it was decided on: a save that came while the grade was
/// read and started stops that grade, and the thread waits and grades again, so a second edit
/// always stops the grade of the first; and a grade another start took the project from under
/// is waited for, never reported as one that did not run.
fn waited_then_graded(
    app: &tauri::AppHandle,
    held: &Arc<Held>,
    burst: &(PlaneId, String),
    level: &Level,
    skill: &str,
) {
    let (plane, key) = burst;
    loop {
        wait_out(burst);
        let free = project_free(plane, skill);
        if !free {
            BURSTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(burst);
            tell(
                app,
                plane,
                level,
                skill,
                "Another grade in this project ran for over an hour, so your edit was not \
                 graded: press Grade to grade it."
                    .to_owned(),
            );
            return;
        }
        let decided = Instant::now();
        let outcome = grade_after(
            app,
            Arc::clone(held),
            plane.clone(),
            level.clone(),
            skill.to_owned(),
        );
        let mut bursts = BURSTS.lock().unwrap_or_else(PoisonError::into_inner);
        let saved_since = bursts
            .get(burst)
            .is_some_and(|one| one.saved_after(decided));
        if saved_since && outcome == Outcome::Started {
            stop_after_edit(plane, key);
        }
        if saved_since || outcome == Outcome::Busy {
            continue;
        }
        bursts.remove(burst);
        return;
    }
}

/// Waits until `burst` has gone [`QUIET`] with no save.
fn wait_out(burst: &(PlaneId, String)) {
    loop {
        let left = {
            let bursts = BURSTS.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(one) = bursts.get(burst) else {
                return;
            };
            let left = one.left(Instant::now());
            if left.is_zero() {
                return;
            }
            left
        };
        std::thread::sleep(left);
    }
}

/// Waits, at most [`MOST_WAIT`], until no grade runs in `plane` (one grade runs in a project at
/// a time): whether none does. A grade of `skill` an edit started was stopped by the save, and
/// ends at its running chat's next look.
fn project_free(plane: &PlaneId, skill: &str) -> bool {
    let began = Instant::now();
    loop {
        if !running().values().any(|one| one.plane == *plane) {
            return true;
        }
        if began.elapsed() >= MOST_WAIT {
            tracing::info!("purlis: a grade of {skill} after an edit gave up waiting");
            return false;
        }
        std::thread::sleep(LOOK_EVERY);
    }
}

/// Tells the window that a saved edit did not grade the copy of `skill` at `level`, and why.
fn tell(app: &tauri::AppHandle, plane: &PlaneId, level: &Level, skill: &str, said: String) {
    let _ = app.emit(
        EVENT,
        SkillGradeAfterEditStep {
            plane: plane.clone(),
            skill: skill.to_owned(),
            level: level.into(),
            said,
        },
    );
}

/// What one try at grading after an edit came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// A grade started.
    Started,
    /// Another grade took the project between the wait and the start: it waits again.
    Busy,
    /// Nothing is to run: off, graded already, waiting for the person, or refused and said.
    Done,
}

/// **Grades the copy of `skill` at `level` after an edit**, where its opt-in is on and what a
/// grade would run is within what was accepted (`after_edit::next`); else it waits, and says why.
fn grade_after(
    app: &tauri::AppHandle,
    held: Arc<Held>,
    plane: PlaneId,
    level: Level,
    skill: String,
) -> Outcome {
    let root = held.root().to_path_buf();
    // Read again now: the person may have turned it off while the saves went quiet.
    let Some(accepted) = after_edit::of(&root, &level, &skill) else {
        return Outcome::Done;
    };
    let not_graded = |why: String| {
        tell(
            app,
            &plane,
            &level,
            &skill,
            format!("Your edit was not graded: {why}"),
        );
        Outcome::Done
    };
    let on = chats_on(
        &root,
        Some(accepted.profile.clone()),
        accepted.model.clone(),
    );
    let planned = match grades::planned(&root, &level, &skill, Some(&accepted.profile)) {
        Ok(planned) => planned,
        Err(why) => return not_graded(why),
    };
    let waits = |why: String| {
        let kept = why.clone();
        if let Err(not) = after_edit::update(&root, &level, &skill, |one| one.paused = Some(kept)) {
            tracing::warn!("purlis: why a grade after an edit waits was not kept ({not})");
        }
        tell(app, &plane, &level, &skill, why);
        Outcome::Done
    };
    if let Err(why) = &on.checked {
        return waits(format!(
            "No grade can run on {} now, so your edit was not graded: {why}",
            accepted.profile
        ));
    }
    let Some(now) = after_edit::Now::of(&planned, &super::terms_of(&planned, &on)) else {
        return not_graded(format!("{skill} has no trigger tests to grade."));
    };
    match after_edit::next(&accepted, &now) {
        after_edit::Next::Same => Outcome::Done,
        after_edit::Next::Waits(why) => waits(why),
        after_edit::Next::Grade => {
            let started = begin(
                app.clone(),
                held,
                Begin {
                    plane: plane.clone(),
                    level: level.clone(),
                    skill: skill.clone(),
                    profile: Some(accepted.profile.clone()),
                    model: accepted.model.clone(),
                    after_edit: true,
                },
                planned,
            );
            match started {
                // Kept once it starts: a save that changes nothing since grades nothing again.
                Ok(_) => {
                    let consent = now.consent;
                    if let Err(why) =
                        after_edit::update(&root, &level, &skill, |one| one.graded = Some(consent))
                    {
                        tracing::warn!(
                            "purlis: what a grade after an edit graded was not kept ({why})"
                        );
                    }
                    Outcome::Started
                }
                // Another start took the project since the wait: wait for it, never say no.
                Err(_) if running().values().any(|one| one.plane == plane) => Outcome::Busy,
                Err(why) => not_graded(why),
            }
        }
    }
}

#[cfg(test)]
mod tests;
