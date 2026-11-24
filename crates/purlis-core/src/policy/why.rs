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
use super::grants::{NEVER_ID, UNREADABLE_ID};
use super::levels::{Chat, PROJECT_OWNER, owner_for};
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
    /// **What `--policy` adds** (spec §10.2), where it was asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<DryRun>,
    /// **Why the sealed side was not read**, where it was not: grants, nevers, the accepted copy
    /// and the accepted floor are the app's to read, so an answer made with no app says so
    /// ([`SEALED_UNREAD`]). Never set by the app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed_unread: Option<String>,
}

/// **What `purlis policy why` says when no app answered**: the sealed store reads as unverifiable
/// outside the app (`Sealed::of` gives `NotTheApp`), so the answer is the committed project's alone.
pub const SEALED_UNREAD: &str = "the sealed side was not read: no purlis app answered, and only \
     the app can read what you accepted, so grants, nevers, the accepted copy and the accepted \
     floor are not shown, and a dry run's diff is against the shipped default. Start the app \
     and run this again";

/// **What `--policy` adds** (spec §10.2).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DryRun {
    /// The branch or file the answer is for, as it was named.
    pub policy: String,
    /// The commit `HEAD` named when this was computed: what the policy in force was read at.
    pub head: Option<String>,
    /// The commit the branch resolved to, in full; none for a file, which is read as it is.
    pub resolved: Option<String>,
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
            dry_run: None,
            sealed_unread: None,
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

/// The most of a `policy.cedar` file a dry run reads: a policy file is text a person wrote, and
/// the largest one the loader reads is far below this.
const A_POLICY_FILE_IS_AT_MOST: u64 = 1 << 20;

/// **The dry run for a chat about itself** ([`for_chat`], and [`dry_run`] as that chat would
/// ask it): by what the app records of the chat, held to its floor.
pub fn dry_run_for_chat(
    root: &Path,
    chat: u32,
    facts: &ChatFacts,
    floor: &Resolved,
    sealed: &Sealed,
    request: (&str, (&str, &str)),
    policy: &str,
) -> Result<Why, String> {
    let asker = decide::asker_of(chat, facts);
    dry_run(root, &asker, request, sealed, Some(floor), policy)
}

/// **The project as `policy` would make it**, and the commit it resolved to where it named one: a
/// branch (or any commit) as that commit holds it, else a `policy.cedar` file, a relative name
/// being the plane root's, laid over the project as committed. **A name that is both is the
/// branch**, so a stray file can never stand in for what a branch holds.
pub fn policy_version(root: &Path, policy: &str) -> Result<(Version, Option<String>), String> {
    if let Some(sha) = Version::resolve_rev(root, policy)? {
        return Version::at_sha(root, &sha)
            .map(|version| (version, Some(sha)))
            .ok_or_else(|| format!("the project's files at `{policy}` could not be read"));
    }
    let path = root.join(policy);
    if !path.is_file() {
        return Err(format!(
            "`{policy}` is neither a file nor a branch of this project"
        ));
    }
    let text = read_bounded(&path)
        .map_err(|why| format!("`{policy}` could not be read: {why}"))?;
    match Version::at_head(root) {
        Head::Read(version) => Ok((version.with_rules(text), None)),
        Head::NoCommit | Head::Unread => Err(format!(
            "the project has no commit that reads, so `{policy}` has nothing to be laid over"
        )),
    }
}

/// `path`'s text, refused past [`A_POLICY_FILE_IS_AT_MOST`].
fn read_bounded(path: &Path) -> Result<String, String> {
    use std::io::Read as _;
    let mut text = String::new();
    std::fs::File::open(path)
        .map_err(|why| why.to_string())?
        .take(A_POLICY_FILE_IS_AT_MOST + 1)
        .read_to_string(&mut text)
        .map_err(|why| why.to_string())?;
    if text.len() as u64 > A_POLICY_FILE_IS_AT_MOST {
        return Err(format!(
            "it is longer than {A_POLICY_FILE_IS_AT_MOST} bytes, which no policy file is"
        ));
    }
    Ok(text)
}

/// **The dry run** (spec §10.2): the answer as `policy` would give it, with how it differs from the
/// policy in force in [`Why::dry_run`]. `Err` says why `policy` is no policy.
pub fn dry_run(
    root: &Path,
    asker: &Asker<'_>,
    request: (&str, (&str, &str)),
    sealed: &Sealed,
    floor: Option<&Resolved>,
    policy: &str,
) -> Result<Why, String> {
    let (after, resolved) = policy_version(root, policy)?;
    // The commit the policy in force is read at, once: both sides below read it again, and the
    // person is told which one this was computed for.
    let head = Version::head_sha(root);
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
    let mut why = Why::of(decision, asker.of);
    why.dry_run = Some(DryRun {
        policy: policy.to_owned(),
        head,
        resolved,
        diff,
        enforced_until_pressed: enforced.after_mode,
    });
    Ok(why)
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

fn loosen_at(level: Level, owner: &str, how: String) -> Loosen {
    Loosen {
        level: level.word().to_owned(),
        owner: owner.to_owned(),
        how,
        press: level != Level::Organisation,
    }
}

/// **The answers the person gave** decide in place of a rule of any file ([`NEVER_ID`],
/// [`UNREADABLE_ID`]): what they are, in a sentence.
fn answer_said(id: &str) -> Option<&'static str> {
    match id {
        NEVER_ID => Some("a never you gave: this action on this resource is not allowed"),
        UNREADABLE_ID => Some(
            "your sealed nevers, which this build cannot read, so nothing is allowed until they do",
        ),
        _ => None,
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
        if answer_said(&rule.id).is_some() {
            // The person's own answer, which they remove themselves: no commit, no press.
            found.push((
                Level::You,
                Loosen {
                    level: Level::You.word().to_owned(),
                    owner: rule.owner.clone(),
                    how: "remove that answer in the window's list of your answers".to_owned(),
                    press: false,
                },
            ));
            continue;
        }
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
                    &owner_for(nearest, of, ""),
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
        // The level the loader said holds the lock. Only a decision built where it was not
        // recorded falls back on its owner's name, which an Organisation may spell as it likes.
        let level = decision.lock_level.unwrap_or_else(|| {
            if owner == PROJECT_OWNER {
                Level::Project
            } else {
                Level::Organisation
            }
        });
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

/// **The answer as JSON**: what [`Why`] holds, the dry run and the note on the sealed side
/// included.
pub fn json(why: &Why) -> String {
    let mut out = serde_json::to_string_pretty(why).expect("an answer is JSON");
    out.push('\n');
    out
}

/// A rule as a line names it: `` `id` (level, owner) at file:line ``.
fn named(rule: &Deciding) -> String {
    match answer_said(&rule.id) {
        Some(said) => said.to_owned(),
        None => format!("{} at {}:{}", rule.said(), rule.file, rule.line),
    }
}

/// **The answer as the person reads it**: what [`Why`] holds, and the dry run's rows where one was
/// asked for. `request` is the verb and the resource as the person typed them.
pub fn text(why: &Why, request: (&str, &str)) -> String {
    use std::fmt::Write as _;
    let decision = &why.decision;
    let dry = why.dry_run.as_ref();
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
        let _ = writeln!(
            out,
            "computed for HEAD {} against {}",
            dry.head.as_deref().unwrap_or("(no commit)"),
            match &dry.resolved {
                Some(sha) => format!("`{}` at {sha}", dry.policy),
                None => format!("the file `{}`, as it reads now", dry.policy),
            }
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
    if let Some(unread) = &why.sealed_unread {
        let _ = writeln!(out, "note: {unread}");
    }
    out
}

#[cfg(test)]
#[path = "why_tests.rs"]
mod tests;
