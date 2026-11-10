// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Ask less, as the window answers** (ADR 0121 decision 7, #6010): the answers Asks notes,
//! the rules made from an offer, Rules I made with each one's Undo, and End cool-off. The
//! record is the core's (`purlis_core::askrules`), this machine's alone; which asks offer a
//! rule, and which are held in a cool-off, is said where the ✋ counts them
//! ([`crate::asking::with_rules`]).
//!
//! **A rule is written by its ask's own answer.** Asks answers the ask with the grant the person
//! chose, through the path the ask names, checked and audited as any answer there is; this then
//! keeps the record of it, naming the grant that answer wrote, so Undo takes away exactly that
//! grant, as Revoke on Settings' Granted list does.

use crate::asking::RuleScope;
use crate::planes::{PlaneId, Planes};
use crate::sandboxing::{GrantLevel, GrantWhat};
use purlis_core::askrules::{Asked, Rule, Store};

/// What is said where this machine keeps no record.
const NO_STORE: &str =
    "purlis keeps no record of answers on this machine: its data folder can't be written.";

fn store() -> Result<Store, String> {
    Store::here().ok_or_else(|| NO_STORE.to_owned())
}

fn asked(plane: &PlaneId, session: u32, request: String) -> Asked {
    Asked {
        project: purlis_core::askrules::project_of(plane.root()),
        session,
        request,
    }
}

/// **Notes an answer the person gave in Asks** (#6010): to request `request` of chat `session`,
/// the option `option`, which `allows` it or not. What offers a rule, and starts a cool-off.
#[tauri::command]
#[specta::specta]
pub fn ask_answered(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    request: String,
    option: String,
    allows: bool,
) -> Result<(), String> {
    planes.held(&plane)?;
    store()?
        .answered(
            &asked(&plane, session, request),
            &option,
            allows,
            crate::sandboxing::now_secs(),
        )
        .map_err(|why| format!("purlis could not note the answer: {why}"))
}

/// **A rule an offer made**, as the window tells it once the ask's answer went through.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct RuleMade {
    pub session: u32,
    pub request: String,
    /// The ask's source, as it lists it.
    pub source: crate::asking::AskSource,
    /// What the ask said.
    pub says: String,
    /// The grant chosen: its id and its words.
    pub option: String,
    pub label: String,
    pub scope: RuleScope,
    /// What the grant names, and its target: a sandbox block's, as its ask's path names them.
    /// None for a run of a program, whose grant is the chat's.
    pub what: Option<GrantWhat>,
    pub target: Option<String>,
}

/// One rule, as Rules I made lists it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RuleMadeShown {
    pub id: String,
    /// The project it is in, where it is open now: Undo needs it open.
    pub plane: Option<PlaneId>,
    /// The project's folder name.
    pub project: String,
    /// When it was made, in seconds since 1970.
    pub at: u32,
    pub source: String,
    /// What the ask said: data, never markup.
    pub says: String,
    /// What it allows, and for whom, in its ask's words.
    pub label: String,
    pub scope: RuleScope,
    /// How many asks it has saved since it was made.
    pub saved: u32,
    /// Its grant was found, and Undo takes it away. Where not, Undo only forgets the rule.
    pub undoes: bool,
}

fn shown(rule: Rule, open: &[PlaneId]) -> RuleMadeShown {
    RuleMadeShown {
        plane: open.iter().find(|one| one.root() == rule.root).cloned(),
        project: rule.root.file_name().map_or_else(
            || rule.root.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        ),
        at: u32::try_from(rule.at).unwrap_or(u32::MAX),
        undoes: rule.grant.is_some(),
        id: rule.id,
        source: rule.source,
        says: rule.says,
        label: rule.label,
        scope: rule.scope.into(),
        saved: rule.saved,
    }
}

/// **Every rule made from an offer**, newest first: Rules I made.
#[tauri::command]
#[specta::specta]
pub fn rules_made(planes: tauri::State<'_, Planes>) -> Result<Vec<RuleMadeShown>, String> {
    let open = planes.open_now();
    let mut rules = store()?.read().rules;
    rules.reverse();
    Ok(rules.into_iter().map(|rule| shown(rule, &open)).collect())
}

/// The level a sandbox block's Allow `option` grants at, by its id.
fn level_of(option: &str) -> Option<GrantLevel> {
    [
        GrantLevel::Once,
        GrantLevel::Chat,
        GrantLevel::You,
        GrantLevel::Machine,
        GrantLevel::Project,
    ]
    .into_iter()
    .find(|level| crate::asking::level_id(*level) == option)
}

/// **Keeps the rule an offer made** (#6010), once its ask's answer went through: with the grant
/// that answer wrote, found on Settings' Granted list, which Undo takes away.
#[tauri::command]
#[specta::specta]
pub fn rule_made(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    made: RuleMade,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    let now = crate::sandboxing::now_secs();
    let grant = match (made.what, made.target.as_deref()) {
        (Some(what), Some(target)) => level_of(&made.option).and_then(|level| {
            crate::sandboxing::grant_made(&held, made.session, what, target, level, now)
        }),
        _ if made.option == crate::asking::RUN_ALLOW_CHAT => crate::sandboxing::grant_made(
            &held,
            made.session,
            GrantWhat::RunGrant,
            "",
            GrantLevel::Chat,
            now,
        ),
        _ => None,
    };
    let source = serde_json::to_value(made.source)
        .ok()
        .and_then(|word| word.as_str().map(str::to_owned))
        .unwrap_or_default();
    store()?
        .make(Rule {
            id: format!("{now}-{}-{}", made.session, made.option),
            at: now,
            project: purlis_core::askrules::project_of(plane.root()),
            root: plane.root().to_path_buf(),
            session: made.session,
            request: made.request,
            source,
            says: made.says,
            label: made.label,
            option: made.option,
            scope: made.scope.into(),
            grant,
            saved: 0,
        })
        .map(|_| ())
        .map_err(|why| format!("purlis could not keep the rule: {why}"))
}

/// **Undo in Rules I made** (#6010): the rule's grant is taken away, audited, as Revoke on
/// Settings' Granted list takes it, and the rule is forgotten. A grant that is no longer there
/// is not an error: the rule is forgotten all the same. Answers the list as it is now.
#[tauri::command]
#[specta::specta]
pub async fn undo_rule(
    planes: tauri::State<'_, Planes>,
    id: String,
) -> Result<Vec<RuleMadeShown>, String> {
    let store = store()?;
    let Some(rule) = store.read().rules.into_iter().find(|one| one.id == id) else {
        return Err("That rule is no longer kept.".to_owned());
    };
    let open = planes.open_now();
    if let Some(grant) = rule.grant.clone() {
        let Some(plane) = open.iter().find(|one| one.root() == rule.root).cloned() else {
            return Err(format!(
                "Open {} to undo this rule: its grant is kept there.",
                rule.root.display()
            ));
        };
        let held = planes.held(&plane)?;
        let network = planes.network().cloned();
        let browsers_ended = std::sync::Arc::clone(planes.browsers_ended());
        crate::off_the_window("undoing a rule", move || {
            // Taken away already, from the Granted list or by its chat ending: nothing to undo.
            if !crate::sandboxing::all_grants_of(&held)
                .iter()
                .any(|one| one.id == grant)
            {
                return Ok(());
            }
            crate::sandboxing::revoke_granted(
                &held,
                &plane,
                network.as_ref(),
                &browsers_ended,
                &grant,
            )
        })
        .await?;
    }
    store
        .undo(&id)
        .map_err(|why| format!("purlis could not forget the rule: {why}"))?;
    let mut rules = store.read().rules;
    rules.reverse();
    Ok(rules.into_iter().map(|rule| shown(rule, &open)).collect())
}

/// **End cool-off** (#6010): chat `session`'s ask for `request` is counted and notified again,
/// from now.
#[tauri::command]
#[specta::specta]
pub fn end_cool_off(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    request: String,
) -> Result<(), String> {
    planes.held(&plane)?;
    store()?
        .ended(
            &asked(&plane, session, request),
            crate::sandboxing::now_secs(),
        )
        .map_err(|why| format!("purlis could not end the cool-off: {why}"))
}
