// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An org's skill controls in the studio and Settings** (#6031, `prototype/skill-policy` A):
//! what the project's `[skill_policy]` holds, read only, where it is set, what it does to each
//! source the window lists, and the skill acts this machine recorded. Every verdict and sentence
//! is the core's ([`purlis_core::skills::source::policy`]); the window decides nothing of them.

use std::ops::ControlFlow;
use std::path::Path;

use serde::Serialize;
use specta::Type;

use purlis_core::eventlog::{self, Event};
use purlis_core::settings::Which;
use purlis_core::skills::source::policy;

use super::{SourceOrigin, esc, seconds};
use crate::skillchange::flag_on;

/// **Which of the controls refuses**, what a row's mark says (#6031).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyKind {
    Blocked,
    NotAllowed,
    TooYoung,
    Undated,
    Unread,
}

impl From<policy::Kind> for PolicyKind {
    fn from(kind: policy::Kind) -> Self {
        match kind {
            policy::Kind::Blocked => PolicyKind::Blocked,
            policy::Kind::NotAllowed => PolicyKind::NotAllowed,
            policy::Kind::TooYoung => PolicyKind::TooYoung,
            policy::Kind::Undated => PolicyKind::Undated,
            policy::Kind::Unread => PolicyKind::Unread,
        }
    }
}

/// **What the controls would refuse, said before a review** (#6031): the core's sentence, and
/// when it can be taken where it is refused for its age alone.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct PolicyRefused {
    pub kind: PolicyKind,
    pub why: String,
    /// When it is old enough to take, in seconds since the epoch, where its source says when it
    /// was made.
    pub until: Option<f64>,
}

impl From<&policy::Refused> for PolicyRefused {
    fn from(refused: &policy::Refused) -> Self {
        PolicyRefused {
            kind: refused.kind.into(),
            why: esc(&refused.why),
            until: refused.until.map(seconds),
        }
    }
}

/// **The project's controls, as its shared settings hold them.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SkillPolicyState {
    /// The shared settings hold no `[skill_policy]`: no controls, and nothing audited.
    None,
    /// The table cannot be read, or this machine's file holds one too: every install and update
    /// is refused until it is fixed, and why.
    Refused { why: String },
    /// The table, each entry as a sentence names it.
    Set {
        /// `None` where it holds no allow list: every source not blocked is allowed.
        allow: Option<Vec<String>>,
        block: Vec<String>,
        min_age_days: u32,
        exempt: Vec<String>,
        /// What it does, in one line: what Sources says above its list.
        said: String,
    },
}

/// **What the controls do to one source** the window lists (#6031).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SourcePolicy {
    pub origin: SourceOrigin,
    /// Which of them refuses it, where one does.
    pub kind: Option<PolicyKind>,
    /// Why they do not allow every skill of it, in the core's sentence; `None` where they do.
    pub refused: Option<String>,
    /// The skills of it they do allow, where they allow some alone.
    pub only: Vec<String>,
}

/// **The project's skill controls** (#6031): where they are set, what they are, and what they do
/// to each source asked about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillPolicy {
    /// The shared settings file, as the project spells it: `charter.toml` or `purlis.toml`.
    pub file: String,
    pub state: SkillPolicyState,
    pub sources: Vec<SourcePolicy>,
}

/// [`skill_policy`] for the project at `root`, of `origins`.
pub(crate) fn policy_shown(root: &Path, origins: Vec<SourceOrigin>) -> SkillPolicy {
    let read = policy::of(root);
    let state = match &read {
        Ok(None) => SkillPolicyState::None,
        Err(why) => SkillPolicyState::Refused { why: esc(why) },
        Ok(Some(set)) => {
            let shown = set.shown();
            SkillPolicyState::Set {
                allow: shown
                    .allow
                    .map(|allow| allow.iter().map(|one| esc(one)).collect()),
                block: shown.block.iter().map(|one| esc(one)).collect(),
                min_age_days: u32::try_from(shown.min_age_days).unwrap_or(u32::MAX),
                exempt: shown.exempt.iter().map(|one| esc(one)).collect(),
                said: esc(&set.said()),
            }
        }
    };
    let sources = origins
        .into_iter()
        .map(|origin| {
            let (kind, refused, only) = match &read {
                Ok(None) => (None, None, Vec::new()),
                Err(why) => (Some(PolicyKind::Unread), Some(esc(why)), Vec::new()),
                Ok(Some(set)) => match set.admits_source(&origin.core()) {
                    Ok(()) => (None, None, Vec::new()),
                    Err(refused) => (
                        Some(refused.kind.into()),
                        Some(esc(&refused.why)),
                        refused.only.iter().map(|one| esc(one)).collect(),
                    ),
                },
            };
            SourcePolicy {
                origin,
                kind,
                refused,
                only,
            }
        })
        .collect();
    SkillPolicy {
        file: Which::Shared.file_at(root).to_owned(),
        state,
        sources,
    }
}

/// **The project's skill controls, read only** (#6031): the `[skill_policy]` its shared settings
/// hold, and what it does to each of `origins`, the sources the window lists. Nothing while the
/// flag is off.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_policy(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    origins: Vec<SourceOrigin>,
) -> Result<SkillPolicy, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the skill controls", move || {
        flag_on()?;
        Ok(policy_shown(held.root(), origins))
    })
    .await
}

/// One of the three skill acts an org's project audits (SK-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum SkillActKind {
    Install,
    Update,
    Remove,
}

/// Where a recorded act was made: the person's own scope either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum SkillActBy {
    /// In the Skill studio, in the window.
    Window,
    /// With `purlis skill` at a terminal, written by the session host.
    Terminal,
}

/// **One skill act this machine recorded** (#6031), as the audit wrote it: metadata only.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SkillAct {
    /// When it was written, in seconds since the epoch.
    pub at: f64,
    pub act: SkillActKind,
    pub skills: Vec<String>,
    /// The source, as an audit names it.
    pub source: String,
    /// The commit or revision.
    pub pin: String,
    /// The level, as a sentence names it.
    pub level: String,
    pub by: SkillActBy,
}

/// **The skill acts this machine recorded**, newest first, at most [`MOST_ACTS`].
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SkillActs {
    pub acts: Vec<SkillAct>,
    /// Whether there are older ones than those listed.
    pub more: bool,
}

/// The most acts Settings lists.
pub(crate) const MOST_ACTS: usize = 50;

/// `event` as a recorded skill act, where it is one.
fn act_of(event: &Event) -> Option<SkillAct> {
    let act = match event.kind.as_str() {
        policy::INSTALL_KIND => SkillActKind::Install,
        policy::UPDATE_KIND => SkillActKind::Update,
        policy::REMOVE_KIND => SkillActKind::Remove,
        _ => return None,
    };
    let text = |key: &str| {
        event
            .body
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(esc)
            .unwrap_or_default()
    };
    Some(SkillAct {
        at: seconds(eventlog::written_at_ms(&event.ulid)? / 1000),
        act,
        skills: event
            .body
            .get("skills")
            .and_then(serde_json::Value::as_array)
            .map(|skills| {
                skills
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(esc)
                    .collect()
            })
            .unwrap_or_default(),
        source: text("source"),
        pin: text("pin"),
        level: text("level"),
        by: if text("scope") == policy::MadeAt::Terminal.scope() {
            SkillActBy::Terminal
        } else {
            SkillActBy::Window
        },
    })
}

/// [`skill_acts`] of the log in `dir`: newest first, at most [`MOST_ACTS`].
pub(crate) fn acts_in(dir: &Path) -> Result<SkillActs, String> {
    let mut acts = Vec::new();
    let mut more = false;
    eventlog::read_back(dir, |event| {
        let Some(act) = act_of(&event) else {
            return ControlFlow::Continue(());
        };
        if acts.len() == MOST_ACTS {
            more = true;
            return ControlFlow::Break(());
        }
        acts.push(act);
        ControlFlow::Continue(())
    })
    .map_err(|why| format!("purlis could not read this machine's event log: {why}"))?;
    Ok(SkillActs { acts, more })
}

/// **The skill acts this machine recorded** (#6031, SK-11): every install, update and removal of
/// a skill from a source the audit wrote, newest first. Nothing while the flag is off.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_acts(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<SkillActs, String> {
    planes.held(&plane)?;
    let dir = match crate::EVENT_LOG.get() {
        Some(Ok(dir)) => dir.clone(),
        Some(Err(refused)) => {
            return Err(format!(
                "purlis keeps no event log on this machine: {}",
                refused.why
            ));
        }
        None => return Err("purlis keeps no event log on this machine.".to_owned()),
    };
    crate::off_the_window("reading the recorded skill acts", move || {
        flag_on()?;
        acts_in(&dir)
    })
    .await
}

#[cfg(test)]
mod tests;
