// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis policy why` and the dry run** (M75 slice 18, #7027; spec §10.1 and §10.2; Q310.8,
//! Q313.1 and Q313.2, Q318.7).
//!
//! A person asks why an action would be allowed, asked or refused, and gets the answer without
//! anything running: the decision of [`decide`] under the policy in force, named in full
//! ([`Why`]): the deciding rules with their level and owner or file and line, the mode where it
//! changed the outcome, any lock and its owner, a grant that decided, the permits a forbid
//! overrode, and the nearest level that could loosen it ([`Loosen`]).
//!
//! **It changes nothing.** The host's decision seals what it sees tighten and spends a Once grant;
//! this one does neither ([`decide::How`]).
//!
//! **The dry run** ([`dry_run`]) answers as a branch, or a `policy.cedar` file laid over the
//! project, would: the same request judged with that policy as the project, its diff against the
//! policy in force ([`press::dry_run`], the rows the loosening press shows), and what the policy in
//! force still enforces until the person presses Accept. A chat may ask about itself only: that
//! is the CLI's rule, and the app answers it ([`for_chat`]) from what it records of the chat.

use std::path::Path;

use super::decide::{self, Asker, ChatFacts, Deciding, Decision, How, Outcome, Sealed};
use super::force::{Head, Version};
use super::levels::Chat;
use super::press::{self, Diff};
use super::{EVERYTHING_ELSE, Level};
use crate::enforcement::Resolved;

/// **A level that could loosen an answer** (Q313.1): who would have to change what.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Loosen {
    /// The level's word.
    pub level: String,
    /// Who owns what would change there.
    pub owner: String,
    /// What would change, in a sentence.
    pub how: String,
    /// Whether the change waits for the person's press: every level but the Organisation's.
    pub press: bool,
}

/// **Why a request came out as it did.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Why {
    /// The decision: the outcome before and after the mode, the deciding rules, the mode, a lock
    /// and a grant that decided.
    pub decision: Decision,
    /// The permits a forbid overrode (Q318.7), where what is enforced is not an allow.
    pub overrode: Vec<Deciding>,
    /// The levels that could loosen it, nearest the chat first. **Every one of them must change**
    /// where several are named: each holds a rule that decides.
    pub loosen: Vec<Loosen>,
}

/// **What `--policy` adds** (spec §10.2).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DryRun {
    /// The branch or file the answer is for, as it was named.
    pub policy: String,
    /// What differs between the policy in force and that policy: the rows the press shows.
    pub diff: Diff,
    /// What the policy in force enforces until the press, were that policy committed: a
    /// loosening waits, so this can be stricter than the answer.
    pub enforced_until_pressed: Outcome,
}

impl Why {
    /// What `decision` says, named: the permits it overrode and the levels that could loosen it,
    /// for a chat of `of` (a deny by the catch-all is loosened by a permit at the level nearest
    /// the chat).
    pub fn of(decision: Decision, of: Chat<'_>) -> Self {
        Why {
            overrode: decision.overrode.clone(),
            loosen: loosening(&decision, of),
            decision,
        }
    }
}

/// **Why `request` is decided as it is under the policy in force**, for the asker the caller
/// names: the project as committed and the accepted floor, the stricter winning. Nothing is
/// sealed, spent or written. `floor` is the app's, where there is one.
pub fn in_plane(
    root: &Path,
    asker: &Asker<'_>,
    request: (&str, (&str, &str)),
    sealed: &Sealed,
    floor: Option<&Resolved>,
) -> Why {
    let decision = decide::explained(
        root,
        asker,
        request,
        sealed,
        floor,
        How::Explain(Version::at_head(root)),
    );
    Why::of(decision, asker.of)
}

/// **The app's answer for a chat about itself**, by what it records of the chat
/// ([`ChatFacts`]), held to its floor as the host's own decision is.
pub fn for_chat(
    root: &Path,
    chat: u32,
    facts: &ChatFacts,
    floor: &Resolved,
    sealed: &Sealed,
    request: (&str, (&str, &str)),
) -> Why {
    let asker = decide::asker_of(chat, facts);
    let decision = decide::explained(
        root,
        &asker,
        request,
        sealed,
        Some(floor),
        How::Explain(Version::at_head(root)),
    );
    Why::of(decision, asker.of)
}

/// **The project as `policy` would make it**: a `policy.cedar` file laid over the project as
/// committed, where `policy` names a file, else the project as that branch (or any commit) holds
/// it. A name that is both is the file.
pub fn policy_version(root: &Path, policy: &str) -> Result<Version, String> {
    if Path::new(policy).is_file() {
        let text = std::fs::read_to_string(policy)
            .map_err(|why| format!("`{policy}` could not be read: {why}"))?;
        return match Version::at_head(root) {
            Head::Read(version) => Ok(version.with_rules(text)),
            Head::NoCommit | Head::Unread => Err(format!(
                "the project has no commit that reads, so `{policy}` has nothing to be laid over"
            )),
        };
    }
    Version::at_rev(root, policy)
}

/// **The dry run** (spec §10.2): the answer as `policy` would give it, and how it differs from the
/// policy in force. `Err` says why `policy` is no policy.
pub fn dry_run(
    root: &Path,
    asker: &Asker<'_>,
    request: (&str, (&str, &str)),
    sealed: &Sealed,
    floor: Option<&Resolved>,
    policy: &str,
) -> Result<(Why, DryRun), String> {
    let after = policy_version(root, policy)?;
    // As the policy would answer once accepted: it alone, no accepted copy or floor beside it.
    let decision = decide::explained(
        root,
        asker,
        request,
        sealed,
        floor,
        How::Alone(after.clone()),
    );
    // As it is enforced the day it is committed, before any press.
    let enforced = decide::explained(
        root,
        asker,
        request,
        sealed,
        floor,
        How::Explain(Head::Read(after.clone())),
    );
    let diff = press::with_sources(root, |sources| {
        press::dry_run(sources, &sealed.standing, &after)
    });
    Ok((
        Why::of(decision, asker.of),
        DryRun {
            policy: policy.to_owned(),
            diff,
            enforced_until_pressed: enforced.after_mode,
        },
    ))
}

/// **The request a person typed**: the action, and the kind and id of its resource. The kind is
/// the schema's own for the action; where an action takes more than one (`save`: `Plane` or
/// `Repo`), the person names it first, `Repo:acme/web`. `Err` says what is wrong and what the
/// choices are.
pub fn request_of(verb: &str, resource: &str) -> Result<(String, String, String), String> {
    let kinds = super::resource_kinds(verb);
    if kinds.is_empty() {
        let known: Vec<&str> = super::actions().iter().map(|(action, _)| *action).collect();
        return Err(format!(
            "`{verb}` is no action of the policy; the actions are {}",
            known.join(", ")
        ));
    }
    if let Some((named, id)) = resource.split_once(':')
        && kinds.iter().any(|kind| kind == named)
    {
        return Ok((verb.to_owned(), named.to_owned(), id.to_owned()));
    }
    match kinds {
        [only] => Ok((verb.to_owned(), only.clone(), resource.to_owned())),
        _ => Err(format!(
            "`{verb}` takes a {}: name which, as `{}:{resource}`",
            kinds.join(" or "),
            kinds[0]
        )),
    }
}

/// How far from the chat a level is: the person's own is nearest.
fn nearness(level: Level) -> usize {
    Level::ALL
        .iter()
        .rposition(|one| *one == level)
        .map_or(0, |at| Level::ALL.len() - 1 - at)
}

/// The level that holds a lock, from its owner (Q310.5): the project's, else the Organisation's.
fn lock_level(owner: &str) -> Level {
    if owner == "this project" {
        Level::Project
    } else {
        Level::Organisation
    }
}

fn loosen_at(level: Level, owner: &str, how: String) -> Loosen {
    Loosen {
        level: level.word().to_owned(),
        owner: owner.to_owned(),
        how,
        press: level != Level::Organisation,
    }
}

/// **The levels that could loosen `decision`** (Q313.1), nearest the chat first. Where a rule
/// decides, the level that wrote it must change or remove it: a permit below a forbid cannot pass
/// it (deny wins at every level, Q310.3). Where no permit applies, a permit at the level nearest
/// the chat would. A lock freezes every level below it, so only its own level and the ones above
/// can loosen anything it holds (Q310.5), and the lock itself is named.
fn loosening(decision: &Decision, of: Chat<'_>) -> Vec<Loosen> {
    if decision.after_mode == Outcome::Allow || decision.deny_state.is_some() {
        return Vec::new();
    }
    let mut found: Vec<(Level, Loosen)> = Vec::new();
    for rule in &decision.deciding {
        let Some(level) = Level::of_word(&rule.level) else {
            continue;
        };
        let how = if rule.id == EVERYTHING_ELSE {
            // No permit applies: the nearest level the chat has may add one.
            let nearest = if of.persona.is_some() {
                Level::Persona
            } else if of.workspace.is_some() {
                Level::Workspace
            } else {
                Level::Project
            };
            found.push((
                nearest,
                loosen_at(
                    nearest,
                    &rule.owner,
                    format!(
                        "add a permit for {}, or turn `{EVERYTHING_ELSE}` off",
                        decision.family
                    ),
                ),
            ));
            continue;
        } else {
            format!(
                "change or remove `{}` at {}:{}",
                rule.id, rule.file, rule.line
            )
        };
        found.push((level, loosen_at(level, &rule.owner, how)));
    }
    if let Some(owner) = &decision.lock {
        let level = lock_level(owner);
        // Below a lock nothing can loosen what it holds.
        found.retain(|(held, _)| nearness(*held) >= nearness(level));
        found.push((
            level,
            loosen_at(
                level,
                owner,
                format!("lift the lock on {} (`[policy.locks]`)", decision.family),
            ),
        ));
    }
    found.sort_by_key(|(level, _)| nearness(*level));
    let mut seen: Vec<String> = Vec::new();
    found
        .into_iter()
        .map(|(_, one)| one)
        .filter(|one| {
            let key = format!("{}|{}", one.level, one.how);
            let fresh = !seen.contains(&key);
            seen.push(key);
            fresh
        })
        .collect()
}

/// **The answer as JSON**: what [`Why`] holds, and the dry run beside it where one was asked for.
pub fn json(why: &Why, dry: Option<&DryRun>) -> String {
    #[derive(serde::Serialize)]
    struct Shown<'a> {
        #[serde(flatten)]
        why: &'a Why,
        #[serde(skip_serializing_if = "Option::is_none")]
        dry_run: Option<&'a DryRun>,
    }
    let mut out =
        serde_json::to_string_pretty(&Shown { why, dry_run: dry }).expect("an answer is JSON");
    out.push('\n');
    out
}

/// A rule as a line names it: `` `id` (level, owner) at file:line ``.
fn named(rule: &Deciding) -> String {
    format!("{} at {}:{}", rule.said(), rule.file, rule.line)
}

/// **The answer as the person reads it**: what [`Why`] holds, and the dry run's rows where one was
/// asked for. `request` is the verb and the resource as the person typed them.
pub fn text(why: &Why, dry: Option<&DryRun>, request: (&str, &str)) -> String {
    use std::fmt::Write as _;
    let decision = &why.decision;
    let mut out = String::new();
    let _ = writeln!(out, "purlis policy why {} {}", request.0, request.1);
    let _ = writeln!(
        out,
        "outcome: {} (family {}, mode {}{})",
        decision.after_mode.word(),
        decision.family,
        decision.mode.word(),
        if decision.would_have_blocked() {
            format!(
                "; the rules say {} and the mode lets it through",
                decision.outcome.word()
            )
        } else {
            String::new()
        }
    );
    if let Some(unasked) = &decision.unasked {
        let _ = writeln!(
            out,
            "refused because the request cannot be asked: {unasked}"
        );
    }
    if let Some(state) = &decision.deny_state {
        let _ = writeln!(out, "the plane is in the deny state: {state:?}");
    }
    for rule in &decision.deciding {
        let _ = writeln!(out, "decided by: {}", named(rule));
    }
    if let Some(owner) = &decision.lock {
        let _ = writeln!(out, "locked by: {owner}");
    }
    if let Some(grant) = &decision.grant {
        let _ = writeln!(out, "a grant decided: {grant}");
    }
    for rule in &why.overrode {
        let _ = writeln!(out, "permit overridden by a forbid: {}", named(rule));
    }
    for (at, one) in why.loosen.iter().enumerate() {
        let lead = if at == 0 {
            "could be loosened by"
        } else {
            "and by"
        };
        let press = if one.press {
            ", then the press"
        } else {
            ", with no press"
        };
        let _ = writeln!(
            out,
            "{lead}: the {} level ({}): {}{press}",
            one.level, one.owner, one.how
        );
    }
    if let Some(dry) = dry {
        let _ = writeln!(
            out,
            "as `{}` would answer: {}",
            dry.policy,
            decision.after_mode.word()
        );
        if dry.enforced_until_pressed != decision.after_mode {
            let _ = writeln!(
                out,
                "until you press Accept the policy in force still enforces: {}",
                dry.enforced_until_pressed.word()
            );
        }
        if dry.diff == Diff::default() {
            let _ = writeln!(
                out,
                "no rule, mode or limit differs from the policy in force"
            );
        }
        for row in &dry.diff.rules {
            let _ = writeln!(out, "rule `{}` {} ({})", row.id, row.kind.word(), row.what);
        }
        for row in &dry.diff.modes {
            let _ = writeln!(
                out,
                "mode {} {}: {} to {}",
                row.key,
                row.kind.word(),
                row.before,
                row.after
            );
        }
        for row in &dry.diff.limits {
            let shown = |n: Option<i64>| n.map_or("no cap".to_owned(), |n| n.to_string());
            let _ = writeln!(
                out,
                "limit {} {}: {} to {}",
                row.key,
                row.kind.word(),
                shown(row.before),
                shown(row.after)
            );
        }
        for row in &dry.diff.local {
            let _ = writeln!(
                out,
                "your local file's `{}` would loosen: {} to {}",
                row.key, row.was, row.now
            );
        }
        for row in &dry.diff.other {
            let _ = writeln!(out, "{} differs", row.part);
        }
    }
    out
}

#[cfg(test)]
#[path = "why_tests.rs"]
mod tests;
