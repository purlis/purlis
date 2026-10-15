// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The team's say over each person's sandbox level** (#2071, PR C; the operator's rulings of
//! 2026-10-14, SO-5, SO-8 and SO-9, and of 2026-10-15 on the spelling), in the committed
//! `charter.toml`:
//!
//! ```toml
//! [sandbox]
//! mode = "off"        # or "on"
//! policy = "local"    # or "never"; absent is "local"
//! ```
//!
//! - **`policy = "local"`** (the default): each person's own level on their machine
//!   ([`super::personal::in_force`]) holds, the floor where they chose none.
//! - **`policy = "never"`**: nobody runs below the full sandbox. "Start without the sandbox" is
//!   refused, and a person's own lower level is clamped to the full sandbox, each with a
//!   sentence naming the policy ([`policy::Locks::opt_out_refused`], [`never_said`]). `mode = "off"` beside it
//!   is refused ([`super::Refusal::ModeOff`]).
//! - **`mode = "off"`** under `local` is **the team's off**: the default becomes no sandbox,
//!   and it reaches each person only on their one Accept ([`team_off`], [`answer`]). Until they
//!   accept, their chats run at their own level, or the floor. Only an off that is committed
//!   asks anyone: a pushed file never silently weakens anyone, and a working-tree edit turns
//!   nothing off (#2156):
//!   - **What the ask says rests on what purlis verified itself** ([`Arrival`]). It names the
//!     commit and its author ("Your project turned the sandbox off (commit <sha> by <author>)")
//!     only where that commit is in the history of the upstream commit purlis's own fetch found
//!     ([`super::local::fetched_upstream`]), read as written. Otherwise it says the off is
//!     local and not on the upstream purlis fetched.
//!   - **The answer is kept for the off as purlis saw it**, in this machine's config home
//!     ([`super::local`]), which no sandboxed chat writes, never for a commit: a history
//!     rewritten under an off that stays asks nobody again. It is let go when purlis itself
//!     reads the project's file not turning the sandbox off, so an off purlis saw leave and
//!     come back asks again.
//!
//! **[`allowed_levels`] is the bound** a chat's start clamps the person's level to, and
//! [`level_for`] the level it runs at.

use std::path::Path;
use std::time::Duration;

use super::personal::Choice;
use super::policy;

/// The key of the team's policy, in `[sandbox]`.
pub const KEY: &str = "policy";

/// What `[sandbox] policy` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Policy {
    /// Each person's own level on their machine.
    #[default]
    Local,
    /// Nobody runs below the full sandbox.
    Never,
}

impl Policy {
    /// The policy `value` names: absent is `local`, and anything that is not `"never"` or
    /// `"local"` is read as `never` (`Err`), the stricter, so a typo never loosens anyone.
    pub fn of_value(value: Option<&toml::Value>) -> Result<Self, Self> {
        match value {
            None => Ok(Self::Local),
            Some(toml::Value::String(word)) if word == "local" => Ok(Self::Local),
            Some(toml::Value::String(word)) if word == "never" => Ok(Self::Never),
            Some(_) => Err(Self::Never),
        }
    }
}

/// What the committed file says of the team's say.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Team {
    pub policy: Policy,
    /// `mode = "off"` under `policy = "local"`: the team's off.
    pub off: bool,
}

impl Team {
    /// What the `[sandbox]` of `top`, a parsed `charter.toml`, says.
    pub fn of_top(top: Option<&toml::Table>) -> Self {
        let Some(table) = top
            .and_then(|top| top.get(super::TABLE))
            .and_then(toml::Value::as_table)
        else {
            return Self::default();
        };
        let policy = Policy::of_value(table.get(KEY)).unwrap_or_else(|stricter| stricter);
        let off = matches!(table.get("mode"), Some(toml::Value::String(word)) if word == "off");
        Self {
            policy,
            off: off && policy == Policy::Local,
        }
    }

    /// What `text`, a whole `charter.toml`, says; nothing where it is not TOML.
    pub fn of_text(text: &str) -> Self {
        Self::of_top(text.parse::<toml::Table>().ok().as_ref())
    }

    /// What the project at `root` says now.
    pub fn of(root: &Path) -> Self {
        super::Plane::read(root).team()
    }
}

/// **The sentence a `never` policy is refused with**, wherever it refuses: Start without the
/// sandbox, a person's own lower level, Settings.
pub fn never_said() -> String {
    format!(
        "This project's policy requires the full sandbox ({}.{KEY} = \"never\" in {}): no chat \
         starts without it, and nobody turns it down on their machine.",
        super::TABLE,
        super::FILE
    )
}

/// The commit a team's off arrived in on the upstream, and who made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrived {
    pub commit: String,
    pub author: String,
}

/// **What purlis can say of where the team's off came from** (#2156).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arrival {
    /// In the history of the upstream commit purlis's own fetch found: this commit, by this
    /// author.
    Upstream(Arrived),
    /// Not verified on that upstream: the off is this clone's own so far.
    Local,
}

impl Arrival {
    /// What the ask says, before Accept and Keep it on for me.
    pub fn said(&self) -> String {
        match self {
            Self::Upstream(arrived) => {
                let short: String = arrived.commit.chars().take(7).collect();
                format!(
                    "Your project turned the sandbox off (commit {short} by {}).",
                    crate::shown::short(&arrived.author)
                )
            }
            Self::Local => "This project's sandbox off is local: it is not on the upstream purlis \
                            fetched."
                .to_owned(),
        }
    }
}

/// Where the team's off stands for the person on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamOff {
    /// The file does not turn it off (or its policy is `never`).
    No,
    /// The file turns it off, but no commit does: nobody is asked, nothing is off.
    NotCommitted,
    /// Committed, and this person has not answered this off: asked once.
    Asks(Arrival),
    /// They accepted it: no sandbox by default.
    Accepted(Arrival),
    /// They kept it on: their own level, or the floor.
    KeptOn(Arrival),
}

/// How long the history may take to say which commit turned it off.
const GIT_WAIT: Duration = Duration::from_secs(5);

/// **Where the team's off stands** for the project at `root` on this machine.
///
/// Where the project's file does not turn the sandbox off, an answer kept for an earlier off is
/// let go: purlis saw that off leave, so an off that comes back is asked again.
pub fn team_off(root: &Path) -> TeamOff {
    if !Team::of(root).off {
        if super::local::team_off_answer(root).is_some() {
            // Best effort: where this machine's file cannot be written from here, the window's
            // next read lets it go.
            let _ = super::local::forget_team_off(root);
        }
        return TeamOff::No;
    }
    let Some(arrival) = arrival(root) else {
        return TeamOff::NotCommitted;
    };
    match super::local::team_off_answer(root) {
        Some(answer) if answer.accepted => TeamOff::Accepted(arrival),
        Some(_) => TeamOff::KeptOn(arrival),
        None => TeamOff::Asks(arrival),
    }
}

/// How many of the manifest's commits are read back for the one that turned the sandbox off.
const MOST_COMMITS_READ: usize = 200;

/// History is read as it was written: no object stands in for another.
const AS_WRITTEN: &str = "--no-replace-objects";

/// Whether `said` is a commit's full id as git prints one.
fn is_commit_id(said: &str) -> bool {
    matches!(said.len(), 40 | 64) && said.bytes().all(|b| b.is_ascii_hexdigit())
}

/// **Where the team's off came from**: [`Arrival::Upstream`] where the commit it arrived in
/// ([`arrived`]) is in the history of the upstream commit purlis's own fetch found, read as
/// written, and turned the sandbox off as written ([`written_arrival`], which also names its
/// author); [`Arrival::Local`] otherwise. None where no commit turns it off.
fn arrival(root: &Path) -> Option<Arrival> {
    let arrived = arrived(root)?;
    if !on_fetched_upstream(root, &arrived.commit) {
        return Some(Arrival::Local);
    }
    Some(match written_arrival(root, &arrived.commit) {
        Some(author) => Arrival::Upstream(Arrived {
            commit: arrived.commit,
            author,
        }),
        None => Arrival::Local,
    })
}

/// Whether `commit` is in the history of the upstream commit purlis's own fetch kept for
/// `root`, read as written ([`written_ancestor`]). Never where nothing was kept, the repository
/// has grafts, or git does not answer.
fn on_fetched_upstream(root: &Path, commit: &str) -> bool {
    let Some(upstream) = super::local::fetched_upstream(root) else {
        return false;
    };
    if !is_commit_id(&upstream) || !is_commit_id(commit) {
        return false;
    }
    let git = |args: &[&str]| crate::worktree::git::run(root, args, GIT_WAIT).ok();
    // Grafts give a commit parents it was not written with.
    let Some(grafts) = git(&["rev-parse", "--git-path", "info/grafts"]).filter(|run| run.ok())
    else {
        return false;
    };
    let grafts = Path::new(grafts.out.trim());
    let grafts = if grafts.is_absolute() {
        grafts.to_path_buf()
    } else {
        root.join(grafts)
    };
    if grafts.exists() {
        return false;
    }
    written_ancestor(root, commit, &upstream)
}

/// Whether `commit` is in the history of `upstream` as that history was written, not as the
/// clone's store holds it: every commit on the way from `upstream` down to `commit` is read back
/// and hashed, kept only where its bytes are the ones its id names, and the way is walked
/// through the parents those checked bytes name. git lists the commits on the way; it is never
/// taken at its word on what they say. Never for a repository whose ids are not SHA-1.
fn written_ancestor(root: &Path, commit: &str, upstream: &str) -> bool {
    use std::collections::HashMap;
    let kind = gix::hash::Kind::Sha1;
    let Ok(upstream) = gix::ObjectId::from_hex(upstream.as_bytes()) else {
        return false;
    };
    let Ok(commit) = gix::ObjectId::from_hex(commit.as_bytes()) else {
        return false;
    };
    if upstream.kind() != kind || commit.kind() != kind {
        return false;
    }
    let range = format!("{commit}..{upstream}");
    let Some(listed) = crate::worktree::git::run(
        root,
        &[AS_WRITTEN, "rev-list", "--ancestry-path", &range],
        GIT_WAIT,
    )
    .ok()
    .filter(crate::worktree::git::Run::ok) else {
        return false;
    };
    let mut asked: Vec<gix::ObjectId> = listed
        .out
        .lines()
        .filter_map(|line| gix::ObjectId::from_hex(line.trim().as_bytes()).ok())
        .collect();
    asked.push(commit);
    let Some(read) = written(root, &asked) else {
        return false;
    };
    // Each commit read back as written, by the parents its bytes name.
    let mut parents = HashMap::new();
    for (id, (object, bytes)) in &read {
        if *object != gix::objs::Kind::Commit {
            continue;
        }
        let Ok(read) = gix::objs::CommitRef::from_bytes(bytes, kind) else {
            continue;
        };
        parents.insert(*id, read.parents().collect::<Vec<_>>());
    }
    let mut seen = std::collections::HashSet::new();
    let mut ahead = vec![upstream];
    while let Some(at) = ahead.pop() {
        if !seen.insert(at) {
            continue;
        }
        let Some(of) = parents.get(&at) else { continue };
        if at == commit {
            return true;
        }
        ahead.extend(of.iter().copied());
    }
    false
}

/// **The objects `ids` name, read as written**: each read back from the clone's store and kept
/// only where its bytes hash to its id, by its kind and bytes. An object missing, or whose bytes
/// are not the ones its id names, is left out. None where git does not answer.
fn written(
    root: &Path,
    ids: &[gix::ObjectId],
) -> Option<std::collections::HashMap<gix::ObjectId, (gix::objs::Kind, Vec<u8>)>> {
    let mut asked = String::new();
    for id in ids {
        asked.push_str(&format!("{id}\n"));
    }
    let read = crate::worktree::git::run_with_input(
        root,
        &[AS_WRITTEN, "cat-file", "--batch"],
        asked.into_bytes(),
        GIT_WAIT,
    )
    .ok()
    .filter(|run| run.code == Some(0))?;
    let mut kept = std::collections::HashMap::new();
    let mut rest = read.out.as_slice();
    while !rest.is_empty() {
        let (record, after) = batch_record(rest)?;
        rest = after;
        let Some((id, object, bytes)) = record else {
            continue;
        };
        let hashed = gix::objs::compute_hash(id.kind(), object, bytes);
        if hashed.ok() == Some(id) {
            kept.insert(id, (object, bytes.to_vec()));
        }
    }
    Some(kept)
}

/// An object read back: its id, kind and bytes.
type Record<'a> = (gix::ObjectId, gix::objs::Kind, &'a [u8]);

/// One record of `git cat-file --batch`'s answer: the object's id, kind and bytes (none for an
/// object that is missing), and what follows it. None where the answer is not one.
fn batch_record(answer: &[u8]) -> Option<(Option<Record<'_>>, &[u8])> {
    let end = answer.iter().position(|&b| b == b'\n')?;
    let header = std::str::from_utf8(&answer[..end]).ok()?;
    let after = &answer[end + 1..];
    let mut words = header.split(' ');
    let id = words.next()?;
    match (words.next(), words.next(), words.next()) {
        (Some("missing" | "ambiguous"), None, None) => Some((None, after)),
        (Some(kind), Some(size), None) => {
            let size: usize = size.parse().ok()?;
            let bytes = after.get(..size)?;
            let after = after.get(size..)?.strip_prefix(b"\n")?;
            let record = gix::ObjectId::from_hex(id.as_bytes())
                .ok()
                .zip(gix::objs::Kind::from_bytes(kind.as_bytes()).ok())
                .map(|(id, kind)| (id, kind, bytes));
            Some((record, after))
        }
        _ => None,
    }
}

/// **Who wrote `commit`, where it turned the sandbox off as written and the upstream still has
/// it off**: the author its own bytes name, where those bytes are the ones its id names, the
/// manifest it holds (every tree on the way to it and the file itself read as written) turns the
/// sandbox off, its first parent's, read the same way, does not, and the manifest of the upstream
/// commit purlis's own fetch found, read the same way, still does. None otherwise, and for a
/// repository whose ids are not SHA-1.
fn written_arrival(root: &Path, commit: &str) -> Option<String> {
    let commit = gix::ObjectId::from_hex(commit.as_bytes()).ok()?;
    if commit.kind() != gix::hash::Kind::Sha1 {
        return None;
    }
    let upstream = super::local::fetched_upstream(root)?;
    let upstream = gix::ObjectId::from_hex(upstream.as_bytes()).ok()?;
    let path = manifest_path(root)?;
    let arrived = written_commit(root, commit)?;
    if !written_off(root, arrived.tree, &path)? {
        return None;
    }
    if let Some(parent) = arrived.parent
        && written_off(root, written_commit(root, parent)?.tree, &path)?
    {
        return None;
    }
    if !written_off(root, written_commit(root, upstream)?.tree, &path)? {
        return None;
    }
    Some(arrived.author)
}

/// A commit read as written: its tree, first parent and author.
struct WrittenCommit {
    tree: gix::ObjectId,
    parent: Option<gix::ObjectId>,
    author: String,
}

/// The commit `id` names, read as written ([`written`]). None where it is not, or is no commit.
fn written_commit(root: &Path, id: gix::ObjectId) -> Option<WrittenCommit> {
    let read = written(root, &[id])?;
    let (object, bytes) = read.get(&id)?;
    if *object != gix::objs::Kind::Commit {
        return None;
    }
    let read = gix::objs::CommitRef::from_bytes(bytes, id.kind()).ok()?;
    Some(WrittenCommit {
        tree: read.tree(),
        parent: read.parents().next(),
        author: String::from_utf8_lossy(read.author().ok()?.name)
            .trim()
            .to_owned(),
    })
}

/// The manifest's path from the top of the repository, one name per directory.
fn manifest_path(root: &Path) -> Option<Vec<String>> {
    let manifest = crate::names::manifest(root);
    let name = manifest.file_name()?.to_str()?.to_owned();
    let prefix = crate::worktree::git::run(root, &["rev-parse", "--show-prefix"], GIT_WAIT)
        .ok()
        .filter(crate::worktree::git::Run::ok)?;
    let mut path: Vec<String> = prefix
        .out
        .trim_end_matches(['\n', '\r'])
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();
    path.push(name);
    Some(path)
}

/// Whether the file at `path` under `tree` turns the sandbox off, read as written: every tree on
/// the way and the file itself are kept only where their bytes are the ones their ids name. A
/// path that holds no file turns nothing off. None where an object on the way is not as written.
fn written_off(root: &Path, tree: gix::ObjectId, path: &[String]) -> Option<bool> {
    let mut at = tree;
    for (depth, name) in path.iter().enumerate() {
        let read = written(root, &[at])?;
        let (object, bytes) = read.get(&at)?;
        if *object != gix::objs::Kind::Tree {
            return None;
        }
        let entries = gix::objs::TreeRef::from_bytes(bytes, at.kind()).ok()?;
        let Some(entry) = entries.entries.iter().find(|entry| {
            let named: &[u8] = entry.filename;
            named == name.as_bytes()
        }) else {
            return Some(false);
        };
        let last = depth + 1 == path.len();
        match (last, entry.mode.is_tree(), entry.mode.is_blob()) {
            (false, true, _) | (true, _, true) => at = entry.oid.to_owned(),
            _ => return Some(false),
        }
    }
    let read = written(root, &[at])?;
    let (object, bytes) = read.get(&at)?;
    if *object != gix::objs::Kind::Blob {
        return None;
    }
    Some(Team::of_text(&String::from_utf8_lossy(bytes)).off)
}

/// **The commit the team's off arrived in**: of the commits that changed the manifest, newest
/// first, the last of the run at its head whose `[sandbox]` turns the sandbox off, the one
/// before it not turning it off (so a change of `mode` in any other table, or of anything else,
/// is never an arrival). None outside git, where `HEAD`'s file does not turn it off (only the
/// working tree says off), or where the history cannot be read.
fn arrived(root: &Path) -> Option<Arrived> {
    let manifest = crate::names::manifest(root);
    let name = manifest.file_name()?.to_str()?;
    let git = |args: &[&str]| {
        crate::worktree::git::run(root, args, GIT_WAIT)
            .ok()
            .filter(crate::worktree::git::Run::ok)
            .map(|run| run.out)
    };
    let off_at = |commit: &str| {
        git(&[AS_WRITTEN, "show", &format!("{commit}:./{name}")])
            .is_some_and(|text| Team::of_text(&text).off)
    };
    if !off_at("HEAD") {
        return None;
    }
    let most = MOST_COMMITS_READ.to_string();
    let log = git(&[
        AS_WRITTEN,
        "log",
        "-n",
        &most,
        "--format=%H%x1f%an",
        "HEAD",
        "--",
        name,
    ])?;
    let mut arrived = None;
    for line in log.lines() {
        let (commit, author) = line.split_once('\u{1f}')?;
        if !off_at(commit) {
            break;
        }
        arrived = Some(Arrived {
            commit: commit.to_owned(),
            author: author.to_owned(),
        });
    }
    arrived
}

/// **The person's answer to the team's off** (Accept, or Keep it on for me, or a change of mind
/// in Settings), kept for the off as it stands: refused where no committed off stands.
pub fn answer(root: &Path, accept: bool) -> Result<(), String> {
    let answer = answering(root, accept)?;
    super::local::answer_team_off(root, answer)
        .map_err(|why| format!("purlis could not keep your answer on this machine: {why}"))
}

/// [`answer`]'s judgement alone: the answer to keep, or why there is none to give.
pub fn answering(root: &Path, accept: bool) -> Result<super::local::TeamOffAnswer, String> {
    match team_off(root) {
        TeamOff::Asks(_) | TeamOff::Accepted(_) | TeamOff::KeptOn(_) => {
            Ok(super::local::TeamOffAnswer { accepted: accept })
        }
        TeamOff::No | TeamOff::NotCommitted => Err(
            "The project does not turn the sandbox off in a commit, so there is nothing to \
             answer."
                .to_owned(),
        ),
    }
}

/// **How far a person may turn a chat's sandbox down** in a project: the lowest level, the
/// level where they chose none, and why it is held, where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    /// The lowest level a chat may run at.
    pub lowest: Choice,
    /// The level of a chat whose person chose none.
    pub default: Choice,
    /// Why nobody may go lower, where a policy holds it: shown beside the switch, and given as
    /// the refusal of Start without the sandbox.
    pub why: Option<String>,
}

impl Bound {
    /// `policy = "local"`: every level, the floor by default, or no sandbox where the person
    /// accepted the team's off.
    pub fn local(accepted_off: bool) -> Self {
        Self {
            lowest: Choice::Off,
            default: if accepted_off {
                Choice::Off
            } else {
                Choice::Floor
            },
            why: None,
        }
    }

    /// The full sandbox and nothing lower, held by `why`.
    pub fn full(why: String) -> Self {
        Self {
            lowest: Choice::On,
            default: Choice::On,
            why: Some(why),
        }
    }

    /// **The level a chat runs at**, given what the person chose: their choice, or the default,
    /// never lower than [`Self::lowest`].
    pub fn clamp(&self, chosen: Option<Choice>) -> Choice {
        // `Choice` is ordered strictest first.
        chosen.unwrap_or(self.default).min(self.lowest)
    }

    /// Whether `choice` is one a person may pick.
    pub fn allows(&self, choice: Choice) -> bool {
        choice <= self.lowest
    }
}

/// **The bound for the project at `root` on this machine**: the full sandbox where the team's
/// `never` or an administrator's policy requires it, else every level, by default the full
/// sandbox where the project turned it on, the floor where it has not, or no sandbox once the
/// person accepted the team's off.
pub fn allowed_levels(root: &Path) -> Bound {
    allowed_levels_under(root, &policy::Locks::of(root))
}

/// [`allowed_levels`] under `locks`, the locks a start already read.
pub fn allowed_levels_under(root: &Path, locks: &policy::Locks) -> Bound {
    if let Some(why) = locks.opt_out_refused() {
        return Bound::full(why);
    }
    let mut bound = Bound::local(matches!(team_off(root), TeamOff::Accepted(_)));
    // A project that turned the full sandbox on starts at it a chat whose person chose no level
    // (#2071): only a level of theirs takes one lower, as far as it is allowed. Not one whose
    // `mode = "off"` is the team's off, whose default is the floor until the person accepts it.
    let plane = super::Plane::read(root);
    if plane.in_force(locks).is_some() && !plane.team().off {
        bound.default = Choice::On;
    }
    bound
}

/// **The level a chat started as `persona` (or none) runs at** in the project at `root`: the
/// person's own level as far as they confirmed it ([`super::personal::in_force`]), within
/// [`allowed_levels`].
pub fn level_for(root: &Path, persona: Option<&str>) -> Choice {
    allowed_levels(root).clamp(super::personal::in_force(root).for_persona(persona))
}

#[cfg(test)]
mod tests;
