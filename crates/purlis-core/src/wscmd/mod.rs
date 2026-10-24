// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `charter workspace` verbs that act on a workspace as a whole, and the plane state
//! they read and write.
//!
//! A port of `charter/commands_workspace.py` and the halves of `charter/workspace.py` those
//! verbs use. What is here is the set that can be ported **whole**; what is not is named in
//! [`crate::wscmd`]'s own gap list below, because a verb ported halfway is worse than one
//! that is plainly absent — an operator who runs it gets a workspace in a state neither
//! charter agrees about.
//!
//! # What is here
//!
//! | Verb | Module |
//! |---|---|
//! | `workspace live [--off]` | [`live`] |
//! | `workspace remove` | [`remove`] |
//! | `workspace use`, `workspace unlock` | [`select`] |
//! | `workspace default` | [`select`] |
//! | `workspace snapshot` | [`snapshot`] |
//! | `workspace create` | [`create`] |
//! | `workspace reinit` | [`reinit`] |
//! | `workspace fork` / `duplicate` | [`fork`] |
//! | `workspace restore` (and `fork --restore`) | [`restore`] |
//! | `workspace rename` / `mv` | [`rename`] |
//! | `workspace forget`, `edit`, `archive`, `unarchive`, `move-memory` | [`memory`] |
//! | `workspace todo done` / `forget` | [`todo`] |
//!
//! # What is NOT here, and what it would take
//!
//! - **A withdrawal of the generated settings inside a CHECKOUT.** [`crate::wslayer::wire`]
//!   removes a `.claude/settings.json` the plane stopped declaring from the workspace
//!   directory. [`crate::guest::wire`] gives back only what it MIRRORED into a checkout: an
//!   agent or a skill the plane no longer has (#1583). The settings it generated there stay,
//!   hidden and with their line kept, until the workspace is removed; charter removed them
//!   there too. Whether purlis should is an open decision (#1583, #998), and nothing reads
//!   that file as left behind until it is made.
//!
//! What a checkout holds is read without writing it by [`crate::guest::status`], the exclude
//! block's own states among them (`withheld`, `unaccounted`, `unrecorded`, #998). Doctor's
//! `workspace layer` row reads every checkout and piece through it.
//!
//! # Where this is stricter than Python, on purpose
//!
//! - **Every name is checked before it is joined onto a path**, the pointer files included.
//!   `Path::join` throws the prefix away when handed an absolute path and `..` walks out of
//!   the plane (charter#442).
//! - **A clone charter could not read is work at risk**, which is Python's rule (charter#917)
//!   and is restated here because it is the one that gates a `remove`.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use crate::counts::{counted, plural};
use crate::names;
use crate::repocmd::{Say, Sink};

pub mod create;
pub mod drop;
pub mod ensure;
pub mod fork;
pub mod live;
pub mod memory;
pub mod reinit;
pub mod remove;
pub mod rename;
pub mod restore;
pub mod select;
pub mod snapshot;
pub mod todo;

/// One sentence for a workspace directory charter could not look at.
///
/// `workspace.cannot_check_workspace` — the sentence `reinit` and every command that shows
/// the plane's workspaces share, so no two of them send a reader to different repairs for one
/// directory.
///
/// The name is printed as it stands, which is what Python prints. It is a directory entry, so
/// it can hold anything; making it readable here would be this port answering a question
/// charter has not answered.
pub fn cannot_check_workspace(at: &Path, code: Option<i32>) -> String {
    let name = at.file_name().unwrap_or_default().to_string_lossy();
    let dir = at.display().to_string();
    format!(
        "workspace '{name}' cannot be checked — purlis changes nothing it cannot see; {}.",
        crate::memstore::uncheckable_fix(code, &dir, &dir)
    )
}

// The delimiters of the managed `.gitignore` block that records which workspaces are LIVE.
//
// Liveness lives in `.gitignore` and nowhere else, so it is git-visible and travels with
// the control plane — there is no second file to disagree with it.
//
// The block is read between either name's lines ([`names::LIVE_BEGIN`],
// [`names::LIVE_END`]) and written under the ones the plane writes
// ([`names::Name::writes_for`]): the block is committed, so it keeps charter's markers until
// the plane is migrated, and is rewritten in place under purlis's after (V93g, V93i).

/// The workspaces marked LIVE — `workspace.live_workspaces`.
///
/// A set, and sorted: the block is rewritten from it, and a set with an order is what keeps
/// two `set_live` calls from producing two different files for the same plane.
pub fn live_workspaces(root: &Path) -> BTreeSet<String> {
    let Ok(text) = std::fs::read_to_string(root.join(".gitignore")) else {
        return BTreeSet::new();
    };
    live_in(&text)
}

/// The workspaces the managed block in `text` marks LIVE.
fn live_in(text: &str) -> BTreeSet<String> {
    block_lines(text).into_iter().filter_map(live_line).collect()
}

/// The workspace a line inside the managed block un-ignores, or `None`.
///
/// Python's `re.match(r"!/workspaces/([^/]+)/workspace\.json", s)` — `match`, so a PREFIX,
/// which is why the tail is only required to START with `workspace.json`. Faithful rather
/// than tightened: the two implementations must read one plane's `.gitignore` the same way.
fn live_line(line: &str) -> Option<String> {
    let rest = line.strip_prefix("!/workspaces/")?;
    let (name, tail) = rest.split_once('/')?;
    (!name.is_empty() && tail.starts_with("workspace.json")).then(|| name.to_string())
}

/// The managed block un-ignoring every LIVE workspace's shareable paths.
///
/// Each path is listed **twice** — the directory and its contents — because un-ignoring
/// `…/memory` alone re-includes the directory entry and none of the files inside it.
///
/// `sessions` is the workspace's session records (ADR 0064): they follow the workspace, LIVE
/// or LOCAL, like its memory and todos. So does `work`, the work link log (ADR 0088 §3): unlike
/// the piece claim log and the landing log, it records links that must reach the operator's
/// other devices.
///
/// `changes` needs the pair **and a third line that re-ignores `changes/log`**, and that
/// asymmetry is the design of the store rather than an exception to it: a change record holds
/// intent, which is what a teammate needs and git cannot derive, while
/// `changes/log/<device>.jsonl` holds a past-tense declaration carrying merge shas, appended
/// per host without a lock, and is committed **never**. Re-ignoring works only because its
/// parent was re-included two lines above — git cannot re-include a file whose parent
/// directory is excluded — which is why the three lines are written together.
pub fn live_block<'a>(root: &Path, names: impl IntoIterator<Item = &'a str>) -> String {
    let mut lines = vec![self::names::LIVE_BEGIN.writes_for(root).to_string()];
    let mut sorted: Vec<&str> = names.into_iter().collect();
    sorted.sort_unstable();
    for n in sorted {
        for line in [
            format!("!/workspaces/{n}/workspace.json"),
            format!("!/workspaces/{n}/workspace.md"),
            format!("!/workspaces/{n}/memory"),
            format!("!/workspaces/{n}/memory/**"),
            format!("!/workspaces/{n}/todos"),
            format!("!/workspaces/{n}/todos/**"),
            format!("!/workspaces/{n}/work"),
            format!("!/workspaces/{n}/work/**"),
            format!("!/workspaces/{n}/sessions"),
            format!("!/workspaces/{n}/sessions/**"),
        ]
        .into_iter()
        .chain(skills_pair(n))
        .chain([
            format!("!/workspaces/{n}/changes"),
            format!("!/workspaces/{n}/changes/**"),
            format!("/workspaces/{n}/changes/log/"),
        ]) {
            lines.push(line);
        }
    }
    lines.push(self::names::LIVE_END.writes_for(root).to_string());
    lines.join("\n")
}

/// Rewrite the managed block for exactly `names`, creating it when it is absent.
///
/// The three placements are Python's `_write_live_block`, in its order: replace the block
/// that is there, else put one straight after the `!/workspaces/.gitkeep` line `charter init`
/// writes (so the un-ignores sit with the rule they qualify), else append.
///
/// **Whole or not at all, one writer at a time** (#358): this file is what keeps `/.charter/`
/// out of git, so it is replaced by rename under [`crate::rewrite::update`]'s lock, and one
/// that is there but cannot be read as text is refused rather than rewritten from nothing.
pub fn write_live_block<'a>(
    root: &Path,
    names: impl IntoIterator<Item = &'a str>,
) -> io::Result<()> {
    let block = live_block(root, names);
    rewrite_gitignore(root, |text| Some(with_block(text, &block))).map(|_| ())
}

/// Rewrite the managed block for the workspaces it already lists — what `reinit` does to
/// bring an older block's per-workspace paths up to date. Read and written under one lock.
pub fn refresh_live_block(root: &Path) -> io::Result<()> {
    rewrite_gitignore(root, |text| {
        let names = live_in(text);
        Some(with_block(
            text,
            &live_block(root, names.iter().map(String::as_str)),
        ))
    })
    .map(|_| ())
}

/// The two lines that share LIVE workspace `name`'s skills (SK-4, #2860): its skills folder,
/// spelled by the one level rule ([`purlis_skills::level_rel`]), and what is in it. None for a
/// name that is no workspace folder's.
fn skills_pair(name: &str) -> Vec<String> {
    purlis_skills::level_rel(&purlis_skills::Whose::Workspace(name.to_owned()), false)
        .map(|rel| vec![format!("!/{rel}"), format!("!/{rel}/**")])
        .unwrap_or_default()
}

/// **The skills lines an older LIVE block lacks** (SK-28, #2875): a `.gitignore` written
/// before a LIVE workspace shared its `skills/` (SK-4) has a block without each workspace's
/// [`skills_pair`], and one written before a level's local skills were ignored lacks that
/// level's line ([`crate::scaffold::skills_local_ignores`], by the one level rule, a persona's
/// included). What `purlis doctor` names and its `live-skills` fix adds.
///
/// Empty where no workspace is LIVE: nothing is shared then, and the block is not this
/// finding's. The block's other lines (`sessions`, `work`) are `workspace-reinit`'s.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillsLines {
    /// Each LIVE workspace the block does not share the skills of, with the lines it lacks.
    pub unshared: BTreeMap<String, Vec<String>>,
    /// The local skills ignore lines the file lacks.
    pub unignored: Vec<String>,
}

impl SkillsLines {
    /// Whether nothing is missing.
    pub fn is_empty(&self) -> bool {
        self.unshared.is_empty() && self.unignored.is_empty()
    }

    /// Every missing line: each workspace's pair, by its name, then the ignore lines.
    pub fn lines(&self) -> Vec<&str> {
        self.unshared
            .values()
            .flatten()
            .chain(&self.unignored)
            .map(String::as_str)
            .collect()
    }
}

/// The header the `live-skills` fix appends the local skills ignore lines under.
const SKILLS_LINES_HEADER: &str = "added by `purlis doctor --fix live-skills`";

/// [`SkillsLines`] of the plane at `root`, as its `.gitignore` stands; empty when there is
/// none.
pub fn skills_lines_missing(root: &Path) -> io::Result<SkillsLines> {
    let path = root.join(".gitignore");
    match std::fs::read(&path) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(|text| skills_lines_in(&text))
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "it is not UTF-8 text")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(SkillsLines::default()),
        Err(e) => Err(e),
    }
}

/// [`SkillsLines`] of the `.gitignore` text `text`.
fn skills_lines_in(text: &str) -> SkillsLines {
    let live = live_in(text);
    if live.is_empty() {
        return SkillsLines::default();
    }
    let in_block = block_lines(text);
    let unshared = live
        .into_iter()
        .filter_map(|name| {
            let lacks: Vec<String> = skills_pair(&name)
                .into_iter()
                .filter(|line| !in_block.contains(&line.as_str()))
                .collect();
            (!lacks.is_empty()).then_some((name, lacks))
        })
        .collect();
    let present = crate::scaffold::text::stripped_lines(text);
    let unignored = crate::scaffold::skills_local_ignores()
        .into_iter()
        .filter(|line| !present.contains(&line.as_str()))
        .collect();
    SkillsLines {
        unshared,
        unignored,
    }
}

/// Every line inside a managed block of `text`, stripped as [`live_in`] reads them.
fn block_lines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in crate::mdsection::split_lines(text) {
        let line = crate::memstore::py_strip(line);
        if names::LIVE_BEGIN.recognises(line) {
            inside = true;
        } else if names::LIVE_END.recognises(line) {
            inside = false;
        } else if inside {
            out.push(line);
        }
    }
    out
}

/// **Add the skills lines an older LIVE block lacks, and nothing else** (SK-28, #2875): each
/// LIVE workspace's [`skills_pair`] goes inside the block, straight after that workspace's last
/// line there, and the local skills ignore lines are appended under a header. Every byte that
/// was there is kept; the block's other missing lines are left to `workspace-reinit`. Read and
/// written under one lock, as [`set_live`] is. What it added, empty when nothing was missing.
pub fn add_skills_lines(root: &Path) -> io::Result<SkillsLines> {
    let mut added = SkillsLines::default();
    rewrite_gitignore(root, |text| {
        added = skills_lines_in(text);
        if added.is_empty() {
            return None;
        }
        let shared = with_pairs(text, &added.unshared);
        let ignores: Vec<&str> = added.unignored.iter().map(String::as_str).collect();
        Some(crate::scaffold::with_appended(&shared, &ignores, SKILLS_LINES_HEADER).0)
    })?;
    Ok(added)
}

/// `text` with each workspace's `lines` inserted after its last line inside a managed block,
/// ended as that line is.
fn with_pairs(text: &str, lines: &BTreeMap<String, Vec<String>>) -> String {
    let all: Vec<&str> = text.split_inclusive('\n').collect();
    let mut last: BTreeMap<&str, usize> = BTreeMap::new();
    let mut inside = false;
    for (at, line) in all.iter().enumerate() {
        let stripped = crate::memstore::py_strip(line);
        if names::LIVE_BEGIN.recognises(stripped) {
            inside = true;
        } else if names::LIVE_END.recognises(stripped) {
            inside = false;
        } else if inside {
            let path = stripped.trim_start_matches('!').trim_start_matches('/');
            if let Some(name) = lines.keys().find(|name| {
                path.strip_prefix("workspaces/")
                    .and_then(|rest| rest.strip_prefix(name.as_str()))
                    .is_some_and(|rest| rest.starts_with('/'))
            }) {
                last.insert(name, at);
            }
        }
    }
    let mut out = String::with_capacity(text.len());
    for (at, line) in all.iter().enumerate() {
        out.push_str(line);
        for (name, _) in last.iter().filter(|(_, after)| **after == at) {
            let end = if line.ends_with("\r\n") { "\r\n" } else { "\n" };
            if !line.ends_with('\n') {
                out.push_str(end);
            }
            for added in &lines[*name] {
                out.push_str(added);
                out.push_str(end);
            }
        }
    }
    out
}

/// The plane's `.gitignore`, read, handed to `change` (`""` when absent), and replaced with
/// what it returns unless that is `None`. The path is gated as itself by
/// [`crate::rewrite::replace`]: a `.gitignore` symlinked out of the plane would otherwise take
/// this write with it.
fn rewrite_gitignore(root: &Path, change: impl FnOnce(&str) -> Option<String>) -> io::Result<bool> {
    let path = root.join(".gitignore");
    crate::contain::no_link_on_the_way(root, &path)?;
    crate::rewrite::update(root, &path, |text| Ok(change(text.unwrap_or_default())))
}

/// `text` with the managed block set to `block`.
fn with_block(text: &str, block: &str) -> String {
    if first_of(text, &names::LIVE_BEGIN).is_some() {
        replace_every_block(text, block)
    } else if let Some(at) = text.find("!/workspaces/.gitkeep\n") {
        let cut = at + "!/workspaces/.gitkeep\n".len();
        format!("{}{block}\n{}", &text[..cut], &text[cut..])
    } else {
        // `text.rstrip("\n")` — newlines alone, which is what Python strips here. A trailing
        // space in somebody's `.gitignore` is theirs and is kept.
        format!("{}\n{block}\n", text.trim_end_matches('\n'))
    }
}

/// Every `BEGIN … END` span in `text`, replaced by `block` —
/// `re.sub(BEGIN.*?END, block, text, flags=DOTALL)`.
///
/// **EVERY span, not the first**, and the differential is what found that: Python's `re.sub`
/// takes `count=0` by default, which means all. A plane whose `.gitignore` holds two managed
/// blocks — one the fixture carries and one a hand or an older charter added — had the second
/// left behind by a `live --off`, so the workspace the operator had just made private was
/// still un-ignored by the block further down, and the next `charter save` would have
/// committed its memory. Non-greedy, so two blocks in a row are two spans and not one.
///
/// A BEGIN with no END after it matches nothing and is left exactly as it is, which is what
/// `re.sub` does: half a managed block is something an operator edited, and this is not the
/// command that repairs it.
fn replace_every_block(text: &str, block: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((start, _)) = first_of(rest, &names::LIVE_BEGIN) {
        let Some((offset, len)) = first_of(&rest[start..], &names::LIVE_END) else {
            break;
        };
        let end = start + offset + len;
        out.push_str(&rest[..start]);
        out.push_str(block);
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// Where the first spelling of `marker` starts in `text`, and how long it is there — either
/// name's, whichever comes first.
fn first_of(text: &str, marker: &names::Name) -> Option<(usize, usize)> {
    marker
        .spellings()
        .filter_map(|s| text.find(s).map(|at| (at, s.len())))
        .min_by_key(|&(at, len)| (at, std::cmp::Reverse(len)))
}

/// Mark `name` LIVE or LOCAL; `true` when the liveness actually changed.
///
/// The read of the current block and the write of the next one are one act under the plane's
/// lock, so two workspaces made LIVE at once are both LIVE afterwards (#358).
pub fn set_live(root: &Path, name: &str, live: bool) -> io::Result<bool> {
    rewrite_gitignore(root, |text| {
        let mut names = live_in(text);
        if names.contains(name) == live {
            return None;
        }
        if live {
            names.insert(name.to_string());
        } else {
            names.remove(name);
        }
        Some(with_block(
            text,
            &live_block(root, names.iter().map(String::as_str)),
        ))
    })
}

/// Move `old`'s LIVE entry to `new`; `true` when the block changed. A LOCAL `old` changes
/// nothing, so a rename run twice writes the block once. Read and written under one lock, as
/// [`set_live`] is.
pub fn rename_live(root: &Path, old: &str, new: &str) -> io::Result<bool> {
    rewrite_gitignore(root, |text| {
        let mut names = live_in(text);
        if !names.remove(old) {
            return None;
        }
        names.insert(new.to_string());
        Some(with_block(
            text,
            &live_block(root, names.iter().map(String::as_str)),
        ))
    })
}

/// A LIVE workspace's shareable paths, plane-relative — what `live --off` and `save` hand to
/// git (`commands_workspace._ws_meta_paths`).
///
/// **Filtered by existence deliberately**: these go to git as literal paths, and
/// `git rm --cached` on one that was never tracked fails the whole call — taking the manifest
/// and the memory down with a workspace that simply had no todos.
///
/// `changes/` is asked a sharper question, because for it existence is not a safe proxy for
/// the one being asked: `todos/` is born with its index and is never empty afterwards, while
/// a `changes/` can be emptied by `charter change forget` and one holding nothing but the
/// never-committed `changes/log/` is the same case. Both are "exists, nothing tracked", which
/// is exactly the shape that fails the whole call.
pub fn meta_paths(root: &Path, name: &str) -> Vec<String> {
    let dir = root.join("workspaces").join(name);
    let mut out: Vec<String> = [
        "workspace.json",
        "workspace.md",
        "memory",
        "todos",
        crate::work::log::DIR_NAME,
        crate::sessionrecord::DIR,
        purlis_skills::PROJECT_DIR,
    ]
    .into_iter()
    .filter(|rel| dir.join(rel).exists())
    .map(|rel| format!("workspaces/{name}/{rel}"))
    .collect();
    if has_change_records(&dir) {
        out.push(format!("workspaces/{name}/changes"));
    }
    out
}

/// Whether this workspace holds at least one change record — `change.has_records`.
fn has_change_records(workspace_dir: &Path) -> bool {
    let Ok(reader) = std::fs::read_dir(workspace_dir.join("changes")) else {
        return false;
    };
    reader
        .filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().ends_with(".json"))
}

/// Whether the plane has a directory for `name`.
///
/// `symlink_metadata` and not `exists()`: a dangling link at `workspaces/<name>` is not a
/// workspace and is not nothing either — `exists()` answers false for one, and a `create`
/// that believed it would then write through the link.
pub fn workspace_dir_exists(root: &Path, name: &str) -> bool {
    root.join("workspaces")
        .join(name)
        .symlink_metadata()
        .is_ok()
}

/// The workspace's directory, name-checked first.
pub fn workspace_dir(root: &Path, name: &str) -> Option<PathBuf> {
    crate::contain::workspace_name_ok(name).then(|| root.join("workspaces").join(name))
}

// ----------------------------------------------------------------------------------------
// work that removing a workspace would destroy
// ----------------------------------------------------------------------------------------

/// One reason a workspace holds work that removing it would discard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtRisk {
    /// What to look at: a clone's name, or `<repo>/<piece>` for a worktree.
    pub what: String,
    /// The sentence, already joined to the name: `alpha: 2 unpushed commit(s)`.
    pub said: String,
}

impl std::fmt::Display for AtRisk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.said)
    }
}

/// Clones **and worktrees** in the workspace holding uncommitted or unpushed work —
/// `commands_workspace._work_at_risk`.
///
/// **Only that.** Open todos are deliberately not here, though `remove` reports them: this
/// list is what is *unrecoverable*, and a todo is a note about the future, not work that
/// ceases to exist. A workspace whose todos were all abandoned is precisely the one worth
/// deleting, and making that case demand `--force` would teach the habit of reaching for
/// `--force` — which is how a guard stops protecting the commits it exists for.
///
/// **A clone charter could not read is at risk by definition** (charter#917), and this is the
/// sharpest instance of that rule in the whole plane: what this list is empty of is what
/// [`remove`] hands to a recursive delete. Reading a `git status` that FAILED as "no
/// uncommitted changes" makes a clone charter could not look at indistinguishable from one it
/// looked at and found empty — and then deletes it.
pub fn work_at_risk(root: &Path, ws: &str) -> Vec<AtRisk> {
    let mut out = Vec::new();
    let found = match crate::repos::clones(root, ws) {
        Ok(found) => found,
        // The workspace directory itself could not be read or is not contained. Nothing below
        // can be checked, so the whole workspace is at risk rather than empty.
        Err(why) => {
            return vec![AtRisk {
                what: ws.to_string(),
                said: format!("{ws}: could not be read — {why}"),
            }];
        }
    };
    // A clone charter refused to look at is one it cannot say anything about, and `remove`
    // is about to delete it. Said, never dropped — `repos::clones`' own rule.
    for (name, why) in &found.refused {
        out.push(AtRisk {
            what: name.clone(),
            said: format!("{name}: could not be read — {why}"),
        });
    }
    for repo in &found.repos {
        match crate::repos::state_of(&repo.path) {
            Err(why) => out.push(AtRisk {
                what: repo.name.clone(),
                said: format!("{}: could not be read — {why}", repo.name),
            }),
            Ok(state) if !state.clean() => out.push(AtRisk {
                what: repo.name.clone(),
                said: format!("{}: uncommitted changes", repo.name),
            }),
            // Unpushed only where git names an upstream — a branch with none is not "ahead
            // of" anything, and charter does not call a fresh local branch work at risk.
            Ok(state) if state.upstream.is_some() && state.ahead > 0 => out.push(AtRisk {
                what: repo.name.clone(),
                said: format!(
                    "{}: {}",
                    repo.name,
                    counted(state.ahead, "unpushed commit", "unpushed commits")
                ),
            }),
            Ok(_) => {}
        }
    }
    out.extend(worktrees_at_risk(root, ws, &found.repos));
    out
}

/// Worktrees of this workspace holding work that removing it would destroy (charter#91): their
/// uncommitted changes, their commits no other ref reaches, and the git state each folder alone
/// holds (#1472).
///
/// The clone loop cannot see these and never could: it iterates the CLONES, and a clone's
/// `.git` is a directory where a linked worktree's is a FILE. That exclusion is right for
/// counting repos, and it is exactly what left worktrees unguarded while the delete took them
/// anyway.
///
/// **The rule differs from the clone rule, not just the paths.** A worktree is at risk when
/// it holds commits reachable from no other ref — the work that would actually cease to exist
/// — which `charter wt remove` uses too, so the two guards refuse on identical grounds.
/// Keeping them identical is the point: a workspace that removed what a worktree refused to
/// would be the original bug again.
///
/// Not "has no upstream", which is what this was first written as (charter#91) and then
/// narrowed (charter#104): a parallel agent's piece has no upstream from the moment it is
/// created, so that reading refused over pieces with nothing to lose.
///
/// This fires even when the worktree directory lives outside the workspace — a relocated
/// worktree root, which a recursive delete of `workspaces/<ws>` never touches. That is not an
/// oversight: a linked worktree keeps its objects in the CLONE's object store, so removing
/// the clone destroys those commits whether or not the directory survives.
fn worktrees_at_risk(root: &Path, ws: &str, repos: &[crate::repos::Repo]) -> Vec<AtRisk> {
    let mut out = Vec::new();
    for repo in repos {
        let pieces = match crate::worktree::list(root, ws, &repo.name) {
            Ok(pieces) => pieces,
            // git would not list them, so charter does not know what is there. Named in the
            // shape this function already uses for its other "could not be checked".
            Err(why) => {
                out.push(AtRisk {
                    what: repo.name.clone(),
                    said: format!("{}: could not be checked for worktrees — {why}", repo.name),
                });
                continue;
            }
        };
        for piece in pieces {
            let label = format!("{}/{}", repo.name, piece.piece);
            // A registration whose directory is gone holds nothing.
            if piece.prunable.is_some() {
                continue;
            }
            match crate::worktree::dirt_of(&piece.path) {
                crate::worktree::Dirt::Unknown => {
                    out.push(AtRisk {
                        what: label.clone(),
                        said: format!("{label}: could not be checked for uncommitted changes"),
                    });
                    continue;
                }
                crate::worktree::Dirt::Dirty => {
                    out.push(AtRisk {
                        what: label.clone(),
                        said: format!("{label}: uncommitted changes"),
                    });
                    continue;
                }
                crate::worktree::Dirt::Clean => {}
            }
            match crate::worktree::unique_commits_of(&piece.path, piece.branch.as_deref()) {
                None => out.push(AtRisk {
                    what: label.clone(),
                    said: format!("{label}: could not be checked for unique commits"),
                }),
                Some(0) => {}
                Some(alone) => out.push(AtRisk {
                    what: label.clone(),
                    said: format!(
                        "{label}: {} that {} nowhere else",
                        counted(alone, "commit", "commits"),
                        plural(alone, "exists", "exist")
                    ),
                }),
            }
            // What git keeps for the folder alone, which `wt remove` refuses over too (#1472):
            // removing the workspace deletes it with the folder.
            match crate::worktree::standing::own_state(&piece.path) {
                None => out.push(AtRisk {
                    what: label.clone(),
                    said: format!("{label}: could not be checked for git state of its own"),
                }),
                Some(own) if own.is_empty() => {}
                Some(own) => out.push(AtRisk {
                    what: label.clone(),
                    said: format!("{label}: {}", own.said().join("; ")),
                }),
            }
        }
    }
    out
}

/// Repos whose current branch would not restore for another engineer —
/// `commands_workspace._restore_blockers`.
///
/// The 'enforce push' guard: a manifest branch is only meaningful if it is actually on the
/// remote. A clone charter could not read blocks too (charter#917) — this list being empty is
/// what lets [`snapshot`] write a manifest that claims to capture reality, and a `git status`
/// that failed contributes the same emptiness as one that found nothing.
pub fn restore_blockers(root: &Path, ws: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(found) = crate::repos::clones(root, ws) else {
        return vec![format!("{ws}: could not be read")];
    };
    for (name, why) in &found.refused {
        out.push(format!("{name}: could not be read — {why}"));
    }
    for repo in &found.repos {
        match crate::repos::state_of(&repo.path) {
            Err(why) => out.push(format!("{}: could not be read — {why}", repo.name)),
            Ok(state) if !state.clean() => {
                out.push(format!("{}: uncommitted changes", repo.name));
            }
            Ok(state) if state.upstream.is_none() => out.push(format!(
                "{}: branch '{}' isn't pushed to a remote",
                repo.name,
                branch_word(&state.head)
            )),
            Ok(state) if state.ahead > 0 => {
                out.push(format!(
                    "{}: {}",
                    repo.name,
                    counted(state.ahead, "unpushed commit", "unpushed commits")
                ));
            }
            Ok(_) => {}
        }
    }
    out
}

/// What `git rev-parse --abbrev-ref HEAD` prints for a tree — `_repo_branch`.
///
/// `HEAD` for a detached one, which is the literal word git prints and is what charter
/// records; an unborn branch has a name and git prints it.
pub fn branch_word(head: &crate::repos::Head) -> String {
    match head {
        crate::repos::Head::Branch(b) | crate::repos::Head::Unborn(b) => b.clone(),
        crate::repos::Head::Detached(_) => "HEAD".to_string(),
    }
}

/// `git config user.name`, or `$USER`, or `unknown` — `_git_user`.
pub fn git_user(root: &Path) -> String {
    if let Ok(run) =
        crate::worktree::git::run(root, &["config", "user.name"], crate::worktree::git::READ)
        && run.ok()
        && !run.line().is_empty()
    {
        return run.line().to_string();
    }
    std::env::var("USER")
        .ok()
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Say a list of reasons under a headline, one per line, the way charter does.
pub(crate) fn say_each(say: Sink, lines: impl IntoIterator<Item = String>) {
    for line in lines {
        say(Say::Fail(format!("  {line}")));
    }
}

#[cfg(test)]
mod tests;
