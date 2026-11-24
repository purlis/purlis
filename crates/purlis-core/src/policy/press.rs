// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The loosening press** (M75 slice 15, #7024; spec §9.2; Q308a, Q309 points 5 and 6, Q322).
//!
//! A tightening of the project applies at once; a loosening waits for the person's press. This is
//! what the window shows at that press, and what the press does:
//!
//! - **The diff** ([`Diff`]) of the project as `HEAD` holds it and the local file, **against the
//!   policy in force** (spec §9.2, review round 3, SEC3-SF1): the accepted copy (the shipped
//!   default until there is one) and every version of the accepted floor, not only the copy. Each
//!   base is diffed on its own and a change keeps the worst label any base gives it, so a commit
//!   that loosens what the floor holds, though not what the accepted copy holds, is a loosening.
//!   Each rule is `loosens`, `tightens` or `changes` ([`Kind`]); each family's mode and each
//!   limit is shown before and after.
//! - **The commit** ([`Commit`]) and its author **as the commit claims it**: purlis cannot check
//!   who wrote a commit, so the window says only that the commit names this author. Read as
//!   history was written ([`crate::worktree::git::run_as_written`]), so a replace ref changes
//!   nothing it shows. Purlis enforces no
//!   commit review, and the press says so (Q309.6).
//! - **Why it is raised** ([`Reason`]): a commit that loosens, a commit past the accepted floor's
//!   cap, the removal of a tightening from the You level's local file (which has no commit and no
//!   author, Q322), or a store that was lost, diffed against the shipped default with what was
//!   lost ([`RePress`]).
//! - **What the local file would loosen** ([`Diff::local`]) is listed in every press, so an Accept
//!   on an unrelated commit can never clear one unseen (review round 8).
//!
//! [`accept`] is the press itself: it names the digest it was shown, takes the person's
//! [`answers::Press`], and starts the floor again from the project as shown, which is the new
//! accepted copy.

use std::io;
use std::path::Path;

use cedar_policy::Effect;

use crate::enforcement::{Mode, Template};
use crate::secrets::answers::{self, Deny, Standing};

use super::deny_state::RePress;
use super::force::{self, Head, Version};
use super::levels::{Chat, Loaded, Sources};
use super::limits::{self, Direction, Shipped};
use super::{FAMILIES, Rule};

/// How long git may take to answer one question about the commit.
const GIT_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// The no-one's chat a mode is asked for: the project's own modes, whatever a workspace or a
/// persona says.
const NO_CHAT: Chat<'static> = Chat {
    workspace: None,
    persona: None,
};

/// What a change does to what a chat may do. In the order a change is kept when two bases give
/// it two labels: the worst wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// It narrows what a chat may do.
    Tightens,
    /// It is an edit purlis cannot call narrower or wider, so it waits like a loosening.
    Changes,
    /// It widens what a chat may do.
    Loosens,
}

/// **A word of a row, read back from the wire** as the static word it was: a row crosses from the
/// app to `purlis policy why` as JSON, and a word no row says is no row.
fn a_word_of<'de, D: serde::Deserializer<'de>>(
    from: D,
    known: &[&'static str],
) -> Result<&'static str, D::Error> {
    use serde::Deserialize as _;
    let said = String::deserialize(from)?;
    known
        .iter()
        .find(|word| **word == said)
        .copied()
        .ok_or_else(|| serde::de::Error::custom(format!("`{said}` is no word of a diff row")))
}

fn a_word_of_a_rule_change<'de, D: serde::Deserializer<'de>>(
    from: D,
) -> Result<&'static str, D::Error> {
    a_word_of(
        from,
        &[
            "added",
            "removed",
            "edited",
            "now permits",
            "now forbids",
            "now asks",
            "no longer asks",
        ],
    )
}

fn a_mode_word<'de, D: serde::Deserializer<'de>>(from: D) -> Result<&'static str, D::Error> {
    a_word_of(from, &["off", "log", "enforce"])
}

impl Kind {
    /// The word the window says.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Tightens => "tightens",
            Kind::Changes => "changes",
            Kind::Loosens => "loosens",
        }
    }
}

/// One rule that differs, by its `@id`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuleChange {
    /// Its `@id`.
    pub id: String,
    /// What the change does.
    pub kind: Kind,
    /// What happened to it: `added`, `removed`, `edited`, `now permits`, `now forbids`, `now
    /// asks` or `no longer asks`.
    #[serde(deserialize_with = "a_word_of_a_rule_change")]
    pub what: &'static str,
}

/// One family's, or one rule's (`rules.<@id>`), mode that differs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModeChange {
    /// The family's key, or `rules.<@id>`.
    pub key: String,
    /// The mode in force before: `off`, `log` or `enforce`.
    #[serde(deserialize_with = "a_mode_word")]
    pub before: &'static str,
    /// The mode after.
    #[serde(deserialize_with = "a_mode_word")]
    pub after: &'static str,
    /// What the change does.
    pub kind: Kind,
}

/// One `[policy.limits]` number that differs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LimitChange {
    /// The key, `<family>.<kebab>`.
    pub key: String,
    /// The number before; none is no cap.
    pub before: Option<i64>,
    /// The number after.
    pub after: Option<i64>,
    /// What the change does, by the key's direction.
    pub kind: Kind,
}

/// One tightening of the You level's local file that is sealed and no longer in the file, or no
/// longer as strict there: a loosening that waits for the press and has no commit (Q322).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LocalLoosening {
    /// What it sets: `template`, a family's key, or `rules.<@id>`.
    pub key: String,
    /// The word the sealed tightening holds.
    pub was: String,
    /// The word that runs once it is accepted.
    pub now: String,
}

/// **What differs between the policy in force and the project as it stands.**
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Diff {
    /// Each rule that differs.
    pub rules: Vec<RuleChange>,
    /// Each mode that differs.
    pub modes: Vec<ModeChange>,
    /// Each limit that differs.
    pub limits: Vec<LimitChange>,
    /// Each tightening of the local file the press would let go.
    pub local: Vec<LocalLoosening>,
    /// **Each other decision-relevant part that differs** (sets, workspace and persona tables,
    /// locks, limits of the Organisation and your own file, the shared vault registry, the plane's
    /// workspaces and personas, a sealed version that does not read): named, and waiting like any
    /// change, so Accept never seals a difference no row shows.
    pub other: Vec<OtherChange>,
}

/// One part of the policy no typed row covers, that differs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OtherChange {
    /// What differs, as the window says it: `policy.sets.protected-branches`.
    pub part: String,
}

impl Diff {
    /// Whether any rule, mode or limit loosens or changes: what waits for the press. A tightening
    /// alone applies at once and asks nothing.
    pub fn waits(&self) -> bool {
        let waits = |kind: Kind| kind != Kind::Tightens;
        self.rules.iter().any(|one| waits(one.kind))
            || self.modes.iter().any(|one| waits(one.kind))
            || self.limits.iter().any(|one| waits(one.kind))
            || !self.other.is_empty()
    }
}

/// One thing a diff row has: a key to be one row by, and a label.
trait Row {
    fn key(&self) -> String;
    fn kind(&self) -> Kind;
}

impl Row for RuleChange {
    fn key(&self) -> String {
        self.id.clone()
    }
    fn kind(&self) -> Kind {
        self.kind
    }
}

impl Row for ModeChange {
    fn key(&self) -> String {
        self.key.clone()
    }
    fn kind(&self) -> Kind {
        self.kind
    }
}

impl Row for LimitChange {
    fn key(&self) -> String {
        self.key.clone()
    }
    fn kind(&self) -> Kind {
        self.kind
    }
}

/// Adds `row` to `rows`, or, where one is there for the same key, keeps the worse of the two.
fn keep_worst<T: Row>(rows: &mut Vec<T>, row: T) {
    match rows.iter().position(|held| held.key() == row.key()) {
        Some(at) if rows[at].kind() < row.kind() => rows[at] = row,
        Some(_) => {}
        None => rows.push(row),
    }
}

fn is_permit(rule: &Rule) -> bool {
    rule.policy.effect() == Effect::Permit
}

/// A rule's text without its `@ask` annotation, whitespace folded: however the text lays its
/// annotations out, two rules that differ only in `@ask` read the same.
fn without_ask(text: &str) -> String {
    let mut rest = text.to_owned();
    while let Some(at) = rest.find("@ask(") {
        let end = rest[at..]
            .find(')')
            .map_or(rest.len(), |close| at + close + 1);
        rest.replace_range(at..end, " ");
    }
    rest.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What a rule's change does, `None` where it is the same rule. A forbid is a restriction and a
/// permit a grant, so adding a forbid or taking out a permit tightens (spec §6.3).
fn rule_change(before: Option<&Rule>, after: Option<&Rule>) -> Option<(Kind, &'static str)> {
    match (before, after) {
        (None, None) => None,
        (None, Some(rule)) if is_permit(rule) => Some((Kind::Loosens, "added")),
        (None, Some(_)) => Some((Kind::Tightens, "added")),
        (Some(rule), None) if is_permit(rule) => Some((Kind::Tightens, "removed")),
        (Some(_), None) => Some((Kind::Loosens, "removed")),
        (Some(was), Some(now))
            if was.level == now.level && was.policy.to_string() == now.policy.to_string() =>
        {
            None
        }
        (Some(was), Some(now)) => Some(match (is_permit(was), is_permit(now)) {
            (false, true) => (Kind::Loosens, "now permits"),
            (true, false) => (Kind::Tightens, "now forbids"),
            // Dropping or adding `@ask` is a tightening or a loosening only where nothing else
            // about the rule moved with it.
            (false, false)
                if was.ask != now.ask
                    && was.level == now.level
                    && without_ask(&was.policy.to_string())
                        == without_ask(&now.policy.to_string()) =>
            {
                if now.ask {
                    (Kind::Loosens, "now asks")
                } else {
                    (Kind::Tightens, "no longer asks")
                }
            }
            _ => (Kind::Changes, "edited"),
        }),
    }
}

/// The kind a mode moving from `before` to `after` is: a lower mode loosens a family or a forbid,
/// and a higher one loosens a permit, as [`levels`] counts it.
fn mode_kind(before: Mode, after: Mode, permit: bool) -> Option<Kind> {
    let (before, after) = if permit {
        (after, before)
    } else {
        (before, after)
    };
    match after.cmp(&before) {
        std::cmp::Ordering::Less => Some(Kind::Loosens),
        std::cmp::Ordering::Greater => Some(Kind::Tightens),
        std::cmp::Ordering::Equal => None,
    }
}

/// How loose a limit's number is in its key's direction: no number is the loosest there is.
fn looseness(direction: Direction, number: Option<i64>) -> i128 {
    match (direction, number) {
        (_, None) => i128::MAX,
        (Direction::Raised, Some(n)) => i128::from(n),
        (Direction::Lowered, Some(n)) => -i128::from(n),
    }
}

/// What a loaded policy says for the limit `spec`: its project's number, else the shipped one.
fn limit_of(loaded: &Loaded, spec: &limits::Spec) -> Option<i64> {
    loaded
        .project_limits
        .get(&spec.key)
        .copied()
        .or(match spec.shipped {
            Shipped::Number(n) => Some(n),
            Shipped::Unset | Shipped::ByMemory => None,
        })
}

/// A rule's mode as the decision reads it: its own `[policy.rules]` entry, else its family's, as
/// [`super::decide`] does. A rule that names no family's action is `enforce`.
fn rule_mode(loaded: &Loaded, id: &str, rule: Option<&Rule>) -> Mode {
    loaded.rule_modes.get(id).copied().unwrap_or_else(|| {
        rule.and_then(|rule| rule.actions().first().and_then(|a| super::family_of(a)))
            .map_or(Mode::Enforce, |family| loaded.mode_for(family, NO_CHAT))
    })
}

/// What `after` changes against one base, `before`: rules, modes and limits.
fn diff_pair(before: &Loaded, after: &Loaded) -> Diff {
    let mut diff = Diff::default();
    let mut ids = before.policy.rules();
    for id in after.policy.rules() {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    for id in ids {
        if let Some((kind, what)) = rule_change(before.policy.rule(&id), after.policy.rule(&id)) {
            diff.rules.push(RuleChange { id, kind, what });
        }
    }
    for family in FAMILIES {
        let (was, now) = (
            before.mode_for(family, NO_CHAT),
            after.mode_for(family, NO_CHAT),
        );
        if let Some(kind) = mode_kind(was, now, false) {
            diff.modes.push(ModeChange {
                key: family.to_owned(),
                before: was.word(),
                after: now.word(),
                kind,
            });
        }
    }
    let named: std::collections::BTreeSet<&String> = before
        .rule_modes
        .keys()
        .chain(after.rule_modes.keys())
        .collect();
    for id in named {
        let rule = after.policy.rule(id).or_else(|| before.policy.rule(id));
        let (was, now) = (rule_mode(before, id, rule), rule_mode(after, id, rule));
        if let Some(kind) = mode_kind(was, now, rule.is_some_and(is_permit)) {
            diff.modes.push(ModeChange {
                key: format!("rules.{id}"),
                before: was.word(),
                after: now.word(),
                kind,
            });
        }
    }
    let (was_parts, now_parts) = (before.other_parts(), after.other_parts());
    let parts: std::collections::BTreeSet<&String> =
        was_parts.keys().chain(now_parts.keys()).collect();
    for part in parts {
        // A `[policy.rules]` entry is always its own row, even where a mode row shows it too: that
        // row's label reads the entry as its first action's family does, and a rule over two
        // families' actions can loosen in the other.
        if was_parts.get(part) != now_parts.get(part) {
            diff.other.push(OtherChange { part: part.clone() });
        }
    }
    for spec in limits::table() {
        let (was, now) = (limit_of(before, spec), limit_of(after, spec));
        let (loose_was, loose_now) = (looseness(spec.loosens, was), looseness(spec.loosens, now));
        if loose_was != loose_now {
            diff.limits.push(LimitChange {
                key: spec.key.clone(),
                before: was,
                after: now,
                kind: if loose_now > loose_was {
                    Kind::Loosens
                } else {
                    Kind::Tightens
                },
            });
        }
    }
    diff
}

/// How strict `word` is as the value of the local tightening `key`: a template's place in the
/// order, or a mode's. `None` for a word that is neither.
fn strictness(key: &str, word: &str) -> Option<usize> {
    if key == "template" {
        Template::ALL.iter().position(|t| t.word() == word)
    } else {
        Mode::parse(word).map(|mode| mode as usize)
    }
}

/// The tightenings `before` holds from the local file that `after` no longer runs as strictly.
fn local_loosenings(before: &Loaded, after: &Loaded) -> Vec<LocalLoosening> {
    let mut found = Vec::new();
    for tightening in &before.tightenings {
        let key = tightening.key.as_str();
        let now = if key == "template" {
            after.template_for(NO_CHAT).word().to_owned()
        } else if let Some(id) = key.strip_prefix("rules.") {
            rule_mode(after, id, after.policy.rule(id))
                .word()
                .to_owned()
        } else {
            after.mode_for(key, NO_CHAT).word().to_owned()
        };
        let (Some(was), Some(is)) = (strictness(key, &tightening.value), strictness(key, &now))
        else {
            continue;
        };
        if is < was {
            found.push(LocalLoosening {
                key: tightening.key.clone(),
                was: tightening.value.clone(),
                now,
            });
        }
    }
    found
}

/// **The diff of `after` against every base**, each on its own, the worst label of a change kept.
pub fn diff(bases: &[Loaded], after: &Loaded) -> Diff {
    let mut all = Diff::default();
    for base in bases {
        let one = diff_pair(base, after);
        for row in one.rules {
            keep_worst(&mut all.rules, row);
        }
        for row in one.modes {
            keep_worst(&mut all.modes, row);
        }
        for row in one.limits {
            keep_worst(&mut all.limits, row);
        }
        for row in one.other {
            if !all.other.iter().any(|held| held.part == row.part) {
                all.other.push(row);
            }
        }
        for row in local_loosenings(base, after) {
            if !all.local.iter().any(|held| held.key == row.key) {
                all.local.push(row);
            }
        }
    }
    all
}

/// Why the press is raised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// A commit loosened (or changed) something the policy in force holds.
    Loosening,
    /// A commit went past the accepted floor's cap: the deny state until this press.
    PastTheCap,
    /// A tightening was taken out of the You level's local file: no commit, no author.
    LocalRemoval,
    /// An update of purlis took a forbid out of the shipped default, or loosened one: no commit and
    /// no author, since nothing in the project changed (spec §7.8).
    ShippedDefault,
    /// The sealed store was lost: diffed against the shipped default.
    LostStore,
}

/// The commit `HEAD` names, and the author it names itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// Its full sha.
    pub sha: String,
    /// Its subject line.
    pub subject: String,
    /// **The author the commit claims**, as written (`name <email>`). Purlis cannot check who
    /// wrote a commit, so the window says only that the commit names this author (Q309.6).
    pub author: String,
}

/// **The press the window shows**: why it is raised, what changes, the commit, and what was
/// lost where the store was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Press {
    /// Why it is raised.
    pub reason: Reason,
    /// What changes against the policy in force.
    pub diff: Diff,
    /// The commit accepted: none for a local file's removal or a shipped default's update, which
    /// have none.
    pub commit: Option<Commit>,
    /// What was lost, for a [`Reason::LostStore`].
    pub lost: Option<RePress>,
    /// What Accept names: a digest of the project as shown and why, so a press on a project that
    /// changed since accepts nothing.
    pub digest: String,
}

/// The git answer's text, where git answered and succeeded.
fn git_out(root: &Path, args: &[&str]) -> Option<String> {
    let run = crate::worktree::git::run_as_written(root, args, GIT_WAIT).ok()?;
    run.ok().then(|| run.out.trim().to_owned())
}

/// The row for a sealed version whose shipped default does not read: an unread side, which denies.
const UNREAD_SHIPPED: &str = "the shipped default this was judged under does not read";

/// The commit `HEAD` names, read as history was written.
fn commit_of(root: &Path) -> Option<Commit> {
    let sha = git_out(root, &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])?;
    let said = git_out(root, &["log", "-1", "--format=%an <%ae>%x1f%s", &sha])?;
    let (author, subject) = said.split_once('\u{1f}')?;
    Some(Commit {
        author: author.to_owned(),
        sha,
        subject: subject.to_owned(),
    })
}

/// The entries that count, as the policy in force counts them: where a commit went past the cap,
/// the ones the store set aside.
fn entries_of(standing: &Standing) -> &[answers::Entry] {
    if standing.is_deny() {
        &standing.kept
    } else {
        &standing.entries
    }
}

/// **What a project is diffed against** (spec §9.2): the accepted copy (the shipped default until
/// there is one) and every version of the accepted floor, or the shipped default alone where the
/// store was lost; and what does not read, which is a row and never a silent removal. The press
/// ([`of_head`]) and the dry run ([`dry_run`]) both take their bases from here, so they show the
/// same diff.
fn held_against(standing: &Standing, lost_store: bool) -> (Vec<Version>, Vec<&'static str>) {
    let none = Version::default();
    let entries = entries_of(standing);
    let mut held: Vec<Version> = Vec::new();
    // What does not read is a row, never a silent removal: the shipped default stands in for an
    // accepted copy that does not read, and the press is the way out.
    let mut unread: Vec<&'static str> = Vec::new();
    if lost_store {
        held.push(none);
    } else {
        match force::accepted_in(entries) {
            Ok(Some(accepted)) if !accepted.reads() => {
                unread.push(UNREAD_SHIPPED);
                held.push(none);
            }
            Ok(accepted) => held.push(accepted.unwrap_or(none)),
            Err(()) => {
                unread.push("the accepted copy does not read");
                held.push(none);
            }
        }
        for one in force::floor_in(entries) {
            match one {
                Ok(version) if !version.reads() => unread.push(UNREAD_SHIPPED),
                Ok(version) => held.push(version),
                Err(()) => unread.push("a sealed version of the accepted floor does not read"),
            }
        }
    }
    (held, unread)
}

/// **The diff of `after` against the policy in force**, as the press would show it were `after`
/// the project's `HEAD` (`policy why --policy`, spec §10.2): the same bases ([`held_against`]) and
/// the same rows ([`diff_all`]). Nothing is raised, sealed or accepted.
pub fn dry_run(sources: &Sources<'_>, standing: &Standing, after: &Version) -> Diff {
    let lost_store = standing
        .causes()
        .iter()
        .any(|cause| cause.a_chat_can_reach() && !matches!(cause, Deny::PastTheFloorCap));
    let (held, unread) = held_against(standing, lost_store);
    let mut diff = diff_all(sources, &held, &unread, after);
    let alone: Vec<Version> = held.iter().map(Version::unpinned).collect();
    name_the_shipped_default(sources, &mut diff, &alone, &unread, after);
    diff
}

/// **The row that says a shipped default's own change is part of what waits** (spec §7.8): where
/// the project alone, each base under the shipped default this build ships (`alone`), diffs
/// differently, the update is a cause too, and no commit is. The press ([`of_head`]) and the dry
/// run ([`dry_run`]) both add it here, so they show the same rows.
fn name_the_shipped_default(
    sources: &Sources<'_>,
    diff: &mut Diff,
    alone: &[Version],
    unread: &[&'static str],
    after: &Version,
) {
    if diff.waits() && *diff != diff_all(sources, alone, unread, after) {
        diff.other.push(OtherChange {
            part: format!(
                "the shipped default, as purlis {} ships it",
                env!("CARGO_PKG_VERSION")
            ),
        });
    }
}

/// **The press `head` raises for the plane at `root`**, or none where nothing waits.
///
/// In the deny state a store that was lost raises the re-press, diffed against the shipped
/// default; a commit past the cap raises its own, diffed against what the store set aside.
pub fn of_head(
    root: &Path,
    sources: &Sources<'_>,
    standing: &Standing,
    head: &Head,
) -> Option<Press> {
    let Head::Read(version) = head else {
        return None;
    };
    let causes = standing.causes();
    let lost_store = causes
        .iter()
        .any(|cause| cause.a_chat_can_reach() && !matches!(cause, Deny::PastTheFloorCap));
    let past_the_cap = causes.contains(&Deny::PastTheFloorCap);
    if standing.is_deny() && !lost_store && !past_the_cap {
        // Only the keyring is at fault: no press mends it.
        return None;
    }
    let (held, unread) = held_against(standing, lost_store);
    let mut diff = diff_all(sources, &held, &unread, version);
    // What the project alone changes, each base under the shipped default this build ships: a
    // shipped default's own change is the update's, never a commit's (spec §7.8).
    let alone: Vec<Version> = held.iter().map(Version::unpinned).collect();
    name_the_shipped_default(sources, &mut diff, &alone, &unread, version);
    // What the commit itself loosens, the local file left out of both sides: a commit's loosening
    // and a local file's removal are different presses (Q322).
    let committed_after = version.committed();
    let commit_waits = diff_all(
        sources,
        &alone.iter().map(Version::committed).collect::<Vec<_>>(),
        &unread,
        &committed_after,
    )
    .waits();
    let shipped_waits = diff_all(
        sources,
        &held.iter().map(Version::committed).collect::<Vec<_>>(),
        &unread,
        &committed_after,
    )
    .waits();
    let reason = if lost_store {
        Reason::LostStore
    } else if past_the_cap {
        Reason::PastTheCap
    } else if commit_waits {
        Reason::Loosening
    } else if shipped_waits {
        Reason::ShippedDefault
    } else if !diff.local.is_empty() {
        Reason::LocalRemoval
    } else {
        return None;
    };
    // A commit is named only where one could have caused the press. Past the cap, a base that
    // holds the project's own text under another shipped default says an update did (it counts
    // toward the cap, spec §7.8), and no commit is blamed.
    let commit = match reason {
        Reason::LocalRemoval | Reason::ShippedDefault => None,
        Reason::PastTheCap
            if held
                .iter()
                .any(|base| base.unpinned().committed() == version.unpinned().committed()) =>
        {
            None
        }
        _ => commit_of(root),
    };
    Some(Press {
        reason,
        lost: (reason == Reason::LostStore)
            .then(|| RePress::of(standing))
            .flatten(),
        digest: digest_of(version, reason, &held, &diff, commit.as_ref()),
        commit,
        diff,
    })
}

/// What Accept names: the project as shown, why the press was raised, what it was diffed against
/// (every base), the rows it shows and the commit, so a press on anything that changed since it
/// was shown accepts nothing.
fn digest_of(
    version: &Version,
    reason: Reason,
    bases: &[Version],
    diff: &Diff,
    commit: Option<&Commit>,
) -> String {
    use sha2::Digest;
    // A base that is the project as shown says nothing a row could: the app seals the version it
    // sees, so the floor may gain exactly that between the press shown and Accept.
    let shown = version.pinned();
    let bases: Vec<&Version> = bases.iter().filter(|base| **base != shown).collect();
    let text = format!(
        "{reason:?}\n{}\n{}\n{diff:?}\n{}",
        serde_json::to_string(version).expect("a version is JSON"),
        serde_json::to_string(&bases).expect("versions are JSON"),
        commit.map_or("", |one| one.sha.as_str()),
    );
    crate::extension::hex(&sha2::Sha256::digest(text.as_bytes()))
}

/// **The diff of `after` against every base**: the typed rows, then every other part of the
/// loaded policy and of the version that differs, then what did not read.
fn diff_all(
    sources: &Sources<'_>,
    bases: &[Version],
    unread: &[&'static str],
    after: &Version,
) -> Diff {
    let loaded: Vec<Loaded> = bases
        .iter()
        .map(|one| force::loaded_for(sources, one))
        .collect();
    let mut all = diff(&loaded, &force::loaded_for(sources, after));
    let mut part = |name: &str| {
        if !all.other.iter().any(|held| held.part == name) {
            all.other.push(OtherChange {
                part: name.to_owned(),
            });
        }
    };
    for base in bases {
        if base.vaults != after.vaults {
            part("the shared vault registry (vaults.json)");
        }
        if base.workspaces != after.workspaces {
            part("the project's workspaces");
        }
        if base.personas != after.personas {
            part("the project's personas");
        }
    }
    for what in unread {
        part(what);
    }
    all
}

/// The sources the window judges the project with: its own files and the project's workspaces and
/// personas, with no Organisation's folder read here.
pub(super) fn with_sources<T>(root: &Path, then: impl FnOnce(&Sources<'_>) -> T) -> T {
    let plane = crate::workspaces::Plane::open(root);
    let workspaces = plane.workspaces().unwrap_or_default();
    let personas = plane.personas().unwrap_or_default();
    let workspaces: Vec<&str> = workspaces.iter().map(String::as_str).collect();
    let personas: Vec<&str> = personas.iter().map(String::as_str).collect();
    let organisation = super::organisation::in_force();
    then(&Sources {
        root,
        workspaces: &workspaces,
        personas: &personas,
        organisation: organisation
            .as_ref()
            .map(super::organisation::Found::as_sources),
        accepted: None,
    })
}

/// **The shipped rules an earlier purlis shipped and this one does not, that the plane at `root`
/// holds a rule of the same `@id` for** ([`force::orphaned_replacements`]), judged with the same
/// sources as the press, the Organisation in force included.
pub fn orphaned(root: &Path) -> Vec<String> {
    let standing = answers::standing(root);
    with_sources(root, |sources| {
        force::orphaned_replacements(sources, &standing)
    })
}

/// **The press the plane at `root` raises now**, none where nothing waits.
pub fn of(root: &Path) -> Option<Press> {
    let standing = answers::standing(root);
    with_sources(root, |sources| {
        of_head(root, sources, &standing, &Version::at_head(root))
    })
}

/// **The person's press: accept the project as `shown` named it** (`digest`), looser where it is
/// looser. The accepted copy becomes the project as `HEAD` holds it and the local file, and the
/// accepted floor starts again, so a sealed tightening of the local file that was taken out no
/// longer holds. In the deny state the store is mended ([`answers::recommit`]), which keeps the
/// nevers. Refused, with nothing changed, where the project changed since it was shown.
pub fn accept(root: &Path, press: &answers::Press, shown: &str) -> io::Result<()> {
    let standing = answers::standing(root);
    let head = Version::at_head(root);
    let held = with_sources(root, |sources| of_head(root, sources, &standing, &head));
    let (Some(held), Head::Read(version)) = (held, &head) else {
        return Err(io::Error::other(
            "nothing waits for your press now, or the project could not be read, so nothing was \
             accepted; look at it again",
        ));
    };
    if held.digest != shown {
        return Err(io::Error::other(
            "the project's policy changed since it was shown, so nothing was accepted; look at \
             it again",
        ));
    }
    // Accepted under the shipped default this build ships, for good: a later purlis that loosens
    // it waits for its own press.
    let text = serde_json::to_string(&version.pinned()).expect("a version is JSON");
    if standing.is_deny() {
        let mut keep: Vec<answers::Entry> = Vec::new();
        // A commit past the cap leaves a store that verified: what it held but the floor, the
        // old accepted copy and the nevers (which the mend keeps itself) is kept.
        if held.reason == Reason::PastTheCap {
            keep.extend(
                standing
                    .kept
                    .iter()
                    .filter(|entry| {
                        ![
                            answers::FLOOR,
                            answers::ACCEPTED,
                            answers::PAST_THE_CAP,
                            answers::NEVER,
                        ]
                        .contains(&entry.kind.as_str())
                    })
                    .cloned(),
            );
        }
        keep.push(answers::Entry::new(answers::ACCEPTED, &text));
        answers::recommit(root, press, &keep).map(|_| ())
    } else {
        answers::commit(root, |draft| {
            for entry in draft.entries().to_vec() {
                if [answers::ACCEPTED, answers::FLOOR, answers::PAST_THE_CAP]
                    .contains(&entry.kind.as_str())
                {
                    draft.remove(press, &entry.kind, &entry.text);
                }
            }
            draft.add(press, answers::ACCEPTED, &text);
        })
        .map(|_| ())
    }
}

#[cfg(test)]
#[path = "press_tests.rs"]
mod tests;
