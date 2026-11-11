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
use purlis_core::askrules::{Asked, Declared, Rule, Store, measures};
use purlis_core::sandboxblock::record::{Entry, Record as NetworkRecord};

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
/// the option `option`, which `allows` it or not, to an ask raised at `raised` (seconds since
/// 1970) where its source knows. What offers a rule, starts a cool-off, and counts in the week's
/// measures (#6021). `request` is empty where the ask names none: it counts in the measures alone.
#[tauri::command]
#[specta::specta]
pub async fn ask_answered(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    request: String,
    option: String,
    allows: bool,
    raised: Option<u32>,
) -> Result<(), String> {
    planes.held(&plane)?;
    // On a blocking thread: it writes this machine's record (`<data>/inbox/rules.json`).
    crate::off_the_window("noting an answer", move || {
        store()?
            .answered(
                &asked(&plane, session, request),
                &option,
                allows,
                raised.map(u64::from),
                crate::sandboxing::now_secs(),
            )
            .map_err(|why| format!("purlis could not note the answer: {why}"))
    })
    .await
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
    /// **The tool line an Always allow showed** (#6177), and whose tools it went to: kept with the
    /// rule only where the persona's tools hold it so, for Undo to take out.
    #[specta(optional)]
    #[serde(default)]
    pub declared: Option<DeclaredMade>,
}

/// A tool line Always allow wrote, as its ask showed it ([`RuleMade::declared`]).
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct DeclaredMade {
    pub persona: String,
    pub line: String,
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

/// The lines purlis's own proxy wrote for the project at `root` on this machine, kept 30 days:
/// what the asks a host's rule saved are read from (#6021). None where it keeps no record.
fn connections_of(root: &std::path::Path, now: u64) -> Vec<Entry> {
    NetworkRecord::to_read().map_or_else(Vec::new, |record| record.read(root, now))
}

fn shown(rule: Rule, open: &[PlaneId], now: u64) -> RuleMadeShown {
    // Counted over what the network record still keeps of the rule's project.
    let saved = measures::saved_by(&rule, &connections_of(&rule.root, now), 0);
    RuleMadeShown {
        plane: open.iter().find(|one| one.root() == rule.root).cloned(),
        project: rule.root.file_name().map_or_else(
            || rule.root.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        ),
        at: u32::try_from(rule.at).unwrap_or(u32::MAX),
        undoes: rule.grant.is_some() || rule.declared.is_some(),
        id: rule.id,
        source: rule.source,
        says: rule.says,
        label: rule.label,
        scope: rule.scope.into(),
        saved,
    }
}

/// **One project's week of asking** (ADR 0121 decision 18, #6021), as All chats says it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct AskMeasure {
    pub plane: PlaneId,
    /// The asks answered in Asks in the last seven days.
    pub asks: u32,
    /// The median time from an ask being raised to its answer, in seconds, over those whose
    /// raising is known.
    pub median_secs: Option<u32>,
    /// The asks its rules saved in the last seven days.
    pub saved: u32,
}

/// **Every open project's week of asking** (#6021), on this machine: All chats' line per
/// project.
#[tauri::command]
#[specta::specta]
pub async fn ask_measures(planes: tauri::State<'_, Planes>) -> Result<Vec<AskMeasure>, String> {
    let open = planes.open_now();
    // On a blocking thread: it reads this machine's record (`<data>/inbox/rules.json`) and each
    // project's network record.
    crate::off_the_window("reading the week's asks", move || {
        let record = store()?.read();
        let now = crate::sandboxing::now_secs();
        Ok(open
            .into_iter()
            .map(|plane| {
                let week = measures::measures(
                    &record,
                    &purlis_core::askrules::project_of(plane.root()),
                    &connections_of(plane.root(), now),
                    now,
                );
                AskMeasure {
                    plane,
                    asks: week.asks,
                    median_secs: week
                        .median_secs
                        .map(|secs| u32::try_from(secs).unwrap_or(u32::MAX)),
                    saved: week.saved,
                }
            })
            .collect())
    })
    .await
}

/// **Every rule made from an offer**, newest first: Rules I made.
#[tauri::command]
#[specta::specta]
pub async fn rules_made(planes: tauri::State<'_, Planes>) -> Result<Vec<RuleMadeShown>, String> {
    let open = planes.open_now();
    // On a blocking thread: it reads this machine's record (`<data>/inbox/rules.json`), and each
    // rule's project's network record for the asks it saved.
    crate::off_the_window("reading the rules made", move || {
        Ok(listed(&store()?, &open))
    })
    .await
}

/// Every rule `store` keeps, newest first, as Rules I made lists it.
fn listed(store: &Store, open: &[PlaneId]) -> Vec<RuleMadeShown> {
    let now = crate::sandboxing::now_secs();
    let mut rules = store.read().rules;
    rules.reverse();
    rules
        .into_iter()
        .map(|rule| shown(rule, open, now))
        .collect()
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
pub async fn rule_made(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    made: RuleMade,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    // On a blocking thread: it reads the chat's grants and writes this machine's record
    // (`<data>/inbox/rules.json`).
    crate::off_the_window("keeping a rule", move || keep_rule(&held, &plane, made)).await
}

/// [`rule_made`]'s work: the grant `made`'s answer wrote, found in `held`, kept with its rule.
fn keep_rule(held: &crate::planes::Held, plane: &PlaneId, made: RuleMade) -> Result<(), String> {
    let now = crate::sandboxing::now_secs();
    let grant = match (made.what, made.target.as_deref()) {
        (Some(what), Some(target)) => level_of(&made.option).and_then(|level| {
            crate::sandboxing::grant_made(held, made.session, what, target, level, now)
        }),
        _ if made.option == crate::asking::RUN_ALLOW_CHAT => crate::sandboxing::grant_made(
            held,
            made.session,
            GrantWhat::RunGrant,
            "",
            GrantLevel::Chat,
            now,
        ),
        _ => None,
    };
    // An Always allow's line, where the persona's tools hold it as its ask showed it (#6177).
    let declared = made
        .declared
        .filter(|_| made.option == crate::asking::RUN_ALLOW_ALWAYS)
        .filter(|one| {
            purlis_core::settings::always_allow::holds(plane.root(), &one.persona, &one.line)
        })
        .map(|one| Declared {
            persona: one.persona,
            line: one.line,
        });
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
            declared,
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
    if rule.grant.is_some() || rule.declared.is_some() {
        let Some(plane) = open.iter().find(|one| one.root() == rule.root).cloned() else {
            return Err(format!(
                "Open {} to undo this rule: its grant is kept there.",
                rule.root.display()
            ));
        };
        let held = planes.held(&plane)?;
        let network = planes.network().cloned();
        let browsers_ended = std::sync::Arc::clone(planes.browsers_ended());
        // On a blocking thread: it writes the project's grants, its manifest and this machine's
        // network record.
        crate::off_the_window("undoing a rule", move || {
            take_away(&held, &plane, network.as_ref(), &browsers_ended, &rule)
        })
        .await?;
    }
    store
        .undo(&id)
        .map_err(|why| format!("purlis could not forget the rule: {why}"))?;
    crate::off_the_window("reading the rules made", move || Ok(listed(&store, &open))).await
}

/// [`undo_rule`]'s work: `rule`'s grant revoked as Revoke revokes it, and the tool line an
/// Always allow wrote taken out (#6177). One gone already is nothing to undo.
fn take_away(
    held: &crate::planes::Held,
    plane: &PlaneId,
    network: Option<&purlis_core::sandboxblock::record::Record>,
    browsers_ended: &crate::chatbrowsers::Teller,
    rule: &Rule,
) -> Result<(), String> {
    // Taken away already, from the Granted list or by its chat ending: nothing to undo.
    if let Some(grant) = &rule.grant
        && crate::sandboxing::all_grants_of(held)
            .iter()
            .any(|one| one.id == *grant)
    {
        crate::sandboxing::revoke_granted(held, plane, network, browsers_ended, grant)?;
    }
    match &rule.declared {
        Some(declared) => take_out_the_line(held, network, declared),
        None => Ok(()),
    }
}

/// **Takes the tool line an Always allow wrote out of the persona's tools** (#6177), through the
/// writer that wrote it. Where the person had allowed all of the persona's list, the Allow moves
/// with the line, audited, as it moved when the line was written, so the rest of the list stays
/// in force. A line gone already, or changed since, is left as it is.
fn take_out_the_line(
    held: &crate::planes::Held,
    network: Option<&purlis_core::sandboxblock::record::Record>,
    declared: &Declared,
) -> Result<(), String> {
    use purlis_core::settings::always_allow::{self, Removed};
    let removed =
        always_allow::remove(held.root(), &declared.persona, &declared.line).map_err(|why| {
            format!(
                "purlis could not take the tool line out of {}'s tools: {why}",
                declared.persona
            )
        })?;
    match removed {
        Removed::Taken { allow: Some(shown) } => crate::sandboxing::allow_shown(
            held.root(),
            &shown,
            &held.audit_then_record(network),
            crate::sandboxing::now_secs(),
        )
        .map_err(|why| {
            format!(
                "The tool line is out of {}'s tools, but the rest of its list waits on your \
                 Allow again: {why}",
                declared.persona
            )
        }),
        Removed::Taken { allow: None } | Removed::Gone => Ok(()),
    }
}

/// **End cool-off** (#6010): chat `session`'s ask for `request` is counted and notified again,
/// from now.
#[tauri::command]
#[specta::specta]
pub async fn end_cool_off(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    request: String,
) -> Result<(), String> {
    planes.held(&plane)?;
    // On a blocking thread: it writes this machine's record (`<data>/inbox/rules.json`).
    crate::off_the_window("ending a cool-off", move || {
        store()?
            .ended(
                &asked(&plane, session, request),
                crate::sandboxing::now_secs(),
            )
            .map_err(|why| format!("purlis could not end the cool-off: {why}"))
    })
    .await
}
