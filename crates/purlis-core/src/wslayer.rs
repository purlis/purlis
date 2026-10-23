// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Charter's generated layer inside a `workspaces/<ws>/` DIRECTORY, and that directory's
//! structure stamp.
//!
//! The other half of [`crate::layer`]'s two targets. A chat launched in `workspaces/<ws>/`
//! reads Claude Code's project settings from that directory and the host does not walk up, so
//! without this it runs with none of the plane's ask/deny rules, none of its `enabledPlugins`
//! and no `$CHARTER_HARNESS` — charter#850 exactly. Agents and skills DO arrive, because
//! those walk up and a workspace directory is not a git boundary: half a layer, and the half
//! that was missing is the half that runs.
//!
//! # What this target has that a checkout does not
//!
//! The `.purlis-structure` stamp (`.charter-structure` before the rename) — the durable
//! upgrade anchor. A workspace created by an older charter can lack files a newer one expects, so the layout version it was built to is
//! stamped in a small local file; a workspace whose stamp is missing or older, or that is
//! missing a baseline file, reads as stale and is flagged until `charter workspace reinit`
//! heals it.
//!
//! # What a checkout has that this target does not
//!
//! **No `.git/info/exclude` entry, and that is measured rather than assumed.**
//! `/workspaces/*/*` is already in the plane's `.gitignore` and the managed LIVE block
//! un-ignores four named paths, none of them `.claude/`. Nothing generated in the DIRECTORY
//! can reach a commit, so there is no block to write for it and — the consequence that
//! matters — no `withheld` and no `unrecorded` row for it: those two exist in the Python
//! because a checkout's write is not made at all when its exclude line or its record cannot
//! be published first. A record that cannot be published here costs only the record, and the
//! layer still lands.
//!
//! **No machine-local document.** Claude Code reads `.claude/settings.local.json` at the git
//! root as well as in the session's own directory, and a workspace directory is inside the
//! plane's own repository, so the plane's copy already reaches it. It is therefore never
//! wanted in the directory — and since it is also the only path the harness itself writes
//! into ([`crate::guest`]'s `COWRITTEN`), [`Did::HarnessBehind`] is a state only a CHECKOUT
//! reaches.
//!
//! # The checkouts under it are wired here too
//!
//! [`wire`] descends into every checkout [`guest_trees`] finds and hands each to
//! [`crate::guest::wire`], whose rows come back labelled by [`checkout_label`]. That is
//! charter's own shape: its `wire_harnesses` materialises the workspace directory and then
//! extends with each guest tree's rows, so `reinit` is the repair for a clone made by an
//! older charter as well as for the directory around it. [`Did`] therefore covers both
//! targets.
//!
//! **Every checkout, which is not the same as every child.** A PIECE lives at
//! `.worktrees/<repo>/<piece>`, two levels below the workspace and under a directory with no
//! `.git` of its own, so nothing that walks the workspace's children finds it — it is reached
//! only by asking git to list the repository's worktrees
//! ([`crate::worktree::listing`]). Until M2.26 every piece `charter wt add` cut — the
//! directory it tells a worker to start a session in — got no layer from a launch, no
//! `reinit` repair and no row.
//!
//! # Where this is stronger than charter, and where it deliberately is not
//!
//! A checkout whose exclude block charter cannot write AT ALL gets no files here, where
//! charter writes the shared settings and the mirrored agents unhidden — [`crate::guest::wire`]
//! argues that one. A line the block leaves out over an untracked file of the operator's in
//! a sibling checkout is NOT that case, and is ported exactly: [`Did::Unhidden`] beside a
//! [`Did::Withheld`] machine-local file, with charter's own shared file written.
//!
//! # Deliberate divergences from the Python, both in the refusing direction
//!
//! 1. **A symlink anywhere on the way is refused, wherever it points.** The Python's
//!    `_inside` resolves and allows a link that lands back inside the workspace;
//!    [`contain::no_link_on_the_way`] refuses every one. This is [`crate::guest`]'s rule
//!    already, and it can only ever report `foreign` where the Python would have rewritten a
//!    file through a link of the operator's.
//! 2. **The plane's `.claude/settings.json` is read only when it resolves inside the plane.**
//!    The Python reads it wherever a link points. Same direction: charter mirrors less, never
//!    more.
//!
//! Neither is reachable from a fixture plane, so both are stated here rather than waved
//! through in a differential scenario.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::contain;
use crate::layer::{self, Record};

/// The layout version [`scaffold`] produces. Bump it when the baseline grows, and every
/// workspace an older charter made flags itself for `charter workspace reinit`.
///
/// v2: memory is a per-file DB (a `MEMORY.md` index), not a lone `notes.md`.
/// v3: the managed `.gitignore` block shares `changes/` (not its log).
/// v4: the workspace carries charter's harness layer (charter#850).
/// v5: every workspace has a `workspace.json`, from birth (charter#884).
pub const STRUCTURE_VERSION: u32 = 5;

/// Where that version is stamped ([`crate::names::STRUCTURE_STAMP`]). Its older spellings —
/// charter's `.charter-structure`, and `.edm-structure` before it — are migrated in place the
/// first time one is read ([`structure_marker`]).
///
/// Renaming the marker without moving it would silently reset every existing workspace to
/// v0: the new name is not there, so a fully up-to-date workspace reads as stale. Harmless
/// (reinit is additive) but wrong, noisy, and on a LIVE workspace it manufactures a commit.
pub const STRUCTURE_MARKER: &str = crate::names::STRUCTURE_STAMP.write;

/// The baseline files every workspace should have, in the order `reinit` names them.
pub const BASELINE: [&str; 4] = [
    "workspace.md",
    "workspace.json",
    "memory/MEMORY.md",
    "refs/README.md",
];

/// The top-level directory segments charter may generate a file under.
///
/// The boundary the DESTRUCTIVE side answers to, independent of any marker's digests: a
/// marker key naming a path outside these — `README.md`, `keep.txt` — names a file charter
/// never wrote and must never withdraw, whatever digest a planted marker records for it.
///
/// These are every registered harness's in-repo surface roots, as
/// `harness/registry.inherited_paths` gives them: Claude Code's `.claude/agents` and
/// `.claude/skills`, opencode's `.opencode/agent`, Codex's `.codex/skills`. Listed as the
/// roots rather than derived at runtime because charter-app has no harness registry of its
/// own yet — and stated here so the day it grows one, this is the call site to move.
const GENERATED_ROOTS: [&str; 3] = [".claude", ".opencode", ".codex"];

/// What charter found at one wanted path — READ ONLY. The Python's `_layer_status`.
///
/// `harness-edited` and `harness-behind` are absent on purpose: both require a path the
/// harness also writes into, and the only such path is a checkout's
/// `.claude/settings.local.json`, which a workspace directory never wants. See the module
/// docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    /// There, holding exactly what the plane wants.
    Ok,
    /// Not there at all.
    Missing,
    /// Charter's own file, holding content charter's record vouches for — and the project's
    /// note of what it offered too (`found`) — that the plane has moved on from.
    Stale,
    /// Somebody else's file — content charter cannot vouch for against the record, or a path
    /// reached through a symlink. Never repaired and never overwritten.
    ///
    /// The restraint ADR 0015 settles for an unstamped shim: charter cannot tell a file it
    /// wrote before the marker existed from one somebody rewrote, and guessing wrong in that
    /// direction destroys work.
    Foreign,
    /// There, and charter could not read it. Never folded into [`Found::Foreign`]: every
    /// `foreign` sentence advises moving a file aside, and charter only failed to read this
    /// one.
    Unreadable,
    /// A file charter generated, still exactly as written, that nothing generates any more.
    Unwanted,
    /// What [`Found::Stale`] would be on the record's word alone: the plane has newer text,
    /// and the record names the file there, but the project's own note of what it offered
    /// does not (`guest::vouched`). purlis cannot confirm it wrote that file, so it
    /// is never overwritten.
    Unconfirmed,
}

/// What charter DID at one path. The Python's `wire_harnesses`.
///
/// The four states only a CHECKOUT can reach — [`Did::Withheld`], [`Did::Unrecorded`],
/// [`Did::Unhidden`] and [`Did::Unlisted`] — came with M2.26, and all four rest on the same
/// bookkeeping: `git worktree list` per repository, and then `git status` per path per other
/// tree before a line is added or left out ([`crate::worktree::listing`],
/// [`crate::guest::unhidden`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Did {
    /// Written where there was nothing.
    Created,
    /// Charter's own file, brought up to what the plane says now.
    Refreshed,
    /// Already current. `reinit` prints nothing for these.
    Present,
    /// Charter generated it, nothing generates it now, and it was removed.
    Removed,
    /// Somebody else's file, left exactly as found.
    Foreign,
    /// Charter tried to write and the filesystem refused — or the whole directory resolves
    /// out of the plane.
    Blocked,
    /// A generated path charter cannot read, left exactly as it is.
    Unreadable,
    /// A path the HARNESS writes into too, holding content charter cannot vouch for as its
    /// own write — charter's `harness-behind`. Never [`Did::Foreign`] and never "remove it":
    /// the approvals in that file are the harness's own and deleting it to get charter's copy
    /// back destroys them. Only a checkout can reach it: the one co-written path is
    /// `.claude/settings.local.json`, which a workspace directory never wants.
    HarnessBehind,
    /// A machine-local file charter did not write, because the line that would hide it was
    /// left out of the shared exclude over a file of the operator's in another checkout
    /// reading it (charter#1072). Never [`Did::Blocked`]: nothing is in the way at that path.
    Withheld,
    /// A record charter could not publish BEFORE the write it covers, so it wrote nothing and
    /// kept every exclude line it had — charter's ruling H.
    Unrecorded,
    /// The block in a checkout's `.git/info/exclude` is right and still leaves out a line
    /// that checkout needs, because the line would hide a file of yours somewhere else
    /// reading the same file. [`crate::guest::unhidden`] says which, whose, and what clears
    /// it.
    Unhidden,
    /// The plane has newer text for this path, and purlis cannot confirm it wrote the file
    /// there now ([`Found::Unconfirmed`]), so it was left exactly as it is. Not a repair, and
    /// not one `reinit` makes either: the same gate stands at every wire.
    Unconfirmed,
    /// git could not list the worktrees of a checkout's repository, so none of them was
    /// checked or given charter's layer (charter#1072).
    ///
    /// Passed over in silence, a piece `charter wt add` cut kept no plugin and none of the
    /// plane's ask/deny rules while `doctor` was green and `reinit` said nothing to do. It is
    /// not a repair, and it is not one `reinit` can make.
    Unlisted,
}

impl Did {
    /// Whether this row is a repair `reinit` counts.
    pub fn is_repair(self) -> bool {
        matches!(self, Did::Created | Did::Refreshed | Did::Removed)
    }

    /// Whether this row names a state `reinit` cannot clear, so the closing line may not say
    /// "nothing to do" over it.
    ///
    /// [`Did::Withheld`] is deliberately NOT one of them, and that is charter's own list
    /// rather than an oversight this port could improve on: a withheld machine-local file
    /// always arrives beside a [`Did::Unhidden`] exclude row for the same checkout — the line
    /// left out is what withheld it — so the state IS counted, once, at the row that names
    /// whose file is in the way and what clears it.
    pub fn is_unresolved(self) -> bool {
        matches!(
            self,
            Did::Foreign
                | Did::Blocked
                | Did::Unreadable
                | Did::HarnessBehind
                | Did::Unrecorded
                | Did::Unhidden
                | Did::Unlisted
                | Did::Unconfirmed
        )
    }
}

/// One path and what happened to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub rel: String,
    pub did: Did,
}

/// What charter generates inside a workspace directory, as `{relative path: text}`.
///
/// One document today: the plane's own `.claude/settings.json`, filtered to the keys that
/// travel. Empty for a plane with nothing to say — the honest rendering, because writing an
/// empty `{}` would look like a layer.
pub fn want(plane: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(text) = layer::settings_document(plane) {
        out.insert(layer::SETTINGS.to_owned(), text);
    }
    out
}

/// Whether [`want`] can ever offer a workspace folder `rel`: the paths whose offers, noted in
/// the project's ledger, may vouch for a refresh ([`found`]) or a withdraw ([`unwanted`]) here.
fn offered_here(rel: &str) -> bool {
    rel == layer::SETTINGS
}

/// Whether `dir` is a workspace directory of THIS plane that charter may write into.
///
/// A `workspaces/<ws>` committed as a symlink out of the plane would otherwise plant the
/// whole baseline wherever it points, so the directory is refused **as a whole** — one
/// decision, rather than a race between several.
fn writable_directory(plane: &Path, dir: &Path) -> bool {
    let workspaces = plane.join("workspaces");
    let (Some(root), Some(here)) = (contain::resolved(&workspaces), contain::resolved(dir)) else {
        return false;
    };
    here != root && here.starts_with(&root)
}

/// `(relative path, what charter found)` for every path the layer wants — READ ONLY.
///
/// **Regenerate and compare**, never a stored diff: the generator's own output drifts as the
/// plane's settings move, so what "current" means is whatever the generator says today.
///
/// **Read only, because `doctor` calls this from a hook.** A check that writes is not a
/// check: it would report every workspace healthy by having just healed it.
///
/// **The DIRECTORY's half alone**, unlike [`wire`], which descends into every checkout and
/// piece under it. charter's read-only `harness_layer` does descend, and this still does not.
///
/// **Read by doctor's `workspace layer` row** (#994), which descends itself: it pairs this
/// with [`crate::guest::status`] for each checkout and piece [`guest_trees`] finds, inside one
/// [`crate::worktree::listing::answers`] block, so it pays no git spawn for a repository whose
/// common git directory has no `worktrees/` and one listing for one that has.
///
/// **Measured** on an ubuntu CI runner before that reader existed: [`guest_trees`] over a
/// workspace whose checkouts have no linked worktrees is **20.4 µs**; with three linked
/// worktrees it is **10.5 ms**, one spawn; and a whole [`wire`] of that workspace — the
/// directory, the clone and three pieces, each with its block and its `git status` questions —
/// is **55.3 ms**. The read asks git a question of the block only where a checkout has
/// something that is not current ([`crate::guest::status`]), which is what keeps it hook-path
/// work.
pub fn status(
    plane: &Path,
    dir: &Path,
    want_all: &BTreeMap<String, String>,
) -> Vec<(String, Found)> {
    if !writable_directory(plane, dir) {
        // Charter reads no layer through a directory that resolves out of the plane, and
        // writes none. The row names the directory rather than the tree behind the link.
        return vec![(layer::MARKER.to_owned(), Found::Foreign)];
    }
    let record = layer::read_record(dir);
    let offered = crate::guest::read_offered(plane);
    let mut rows = found(plane, dir, want_all, &record, &offered);
    rows.extend(
        unwanted(plane, dir, want_all, &record, &offered)
            .into_iter()
            .map(|rel| (rel, Found::Unwanted)),
    );
    rows
}

/// [`status`]'s comparison for the wanted paths alone, against a record already in hand.
///
/// **Stale only when the record AND the project's note of what it offered (`offered`) vouch
/// for the text there**, at that very path, for a path this layer offers ([`offered_here`]).
/// The record sits in the workspace folder, where a chat may be able to write it: on its own,
/// a forged entry naming the operator's own settings would have the next launch overwrite
/// them. One the record alone vouches for is [`Found::Unconfirmed`] (#1583).
fn found(
    _plane: &Path,
    dir: &Path,
    want_all: &BTreeMap<String, String>,
    record: &Record,
    offered: &BTreeMap<String, Vec<String>>,
) -> Vec<(String, Found)> {
    let mut rows = Vec::new();
    for (rel, want) in want_all {
        let path = dir.join(rel);
        // `symlink_metadata`, so a DANGLING link is not read as "absent" and written through.
        // Only a path that is certainly not there is `missing`; one the filesystem will not
        // answer for goes on to the read and is `unreadable` there.
        if matches!(path.symlink_metadata(), Err(ref e) if crate::memstore::is_absent(e)) {
            rows.push((rel.clone(), Found::Missing));
            continue;
        }
        if contain::no_link_on_the_way(dir, &path).is_err() {
            // Charter neither reads through it — the digest on the far end is not evidence
            // the file is charter's — nor writes through it.
            rows.push((rel.clone(), Found::Foreign));
            continue;
        }
        let Ok(have) = std::fs::read_to_string(&path) else {
            rows.push((rel.clone(), Found::Unreadable));
            continue;
        };
        let on_disk = layer::digest(&have);
        if &have == want {
            rows.push((rel.clone(), Found::Ok));
        } else if record.recorded(rel).contains(&on_disk) {
            // Only content a record LISTS is charter's to overwrite, pending or settled, and
            // only once the project's own note confirms purlis offered that text here.
            let confirmed = offered_here(rel) && crate::guest::vouched(offered, rel, &on_disk);
            rows.push((
                rel.clone(),
                if confirmed {
                    Found::Stale
                } else {
                    Found::Unconfirmed
                },
            ));
        } else {
            rows.push((rel.clone(), Found::Foreign));
        }
    }
    rows
}

/// Files charter generated here, still exactly as written, that nothing generates now.
///
/// READ ONLY — [`withdraw`] removes them and [`status`] reports them, and both ask here so
/// the two cannot disagree about which files those are.
///
/// **Only a path under one of charter's generated roots** ([`GENERATED_ROOTS`]): a marker
/// charter trusts can still name a file charter never wrote, and withdrawing on its word
/// deletes the operator's own file.
///
/// **Only while the file still matches a digest charter recorded** — a file the operator has
/// since edited is not charter's to delete, and unreadable counts as not charter's for the
/// same reason.
///
/// **Only a text purlis itself offered at that very path** (`offered`, the project's ledger
/// [`crate::guest::read_offered`]). The record sits in the workspace folder, where a chat may
/// be able to write it, so on its own it proves nothing: a forged entry naming the operator's
/// own file, with that file's digest, would otherwise have purlis delete it. The ledger lives
/// in the project's app state, which no chat can write, and nothing noted is nothing
/// withdrawn (#1583).
///
/// **Never a held path**: a plane file charter cannot READ is not a plane that stopped
/// declaring something, and withdrawing over it took every workspace's `enabledPlugins`,
/// `env` and `deny` away over a typo.
fn unwanted(
    plane: &Path,
    dir: &Path,
    want_all: &BTreeMap<String, String>,
    record: &Record,
    offered: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    let held = layer::held(plane);
    let mut roots: BTreeSet<&str> = GENERATED_ROOTS.into_iter().collect();
    for rel in want_all.keys() {
        roots.insert(root_of(rel));
    }
    let mut out = Vec::new();
    // In the record's own path order, which `BTreeMap` fixes. A set's iteration order would
    // be whatever a hash gives, and `reinit`'s report must not inherit that.
    for rel in record.paths() {
        if want_all.contains_key(rel) {
            continue;
        }
        if held && rel.as_str() == layer::SETTINGS {
            continue;
        }
        if !roots.contains(root_of(rel)) {
            continue;
        }
        let path = dir.join(rel);
        if contain::no_link_on_the_way(dir, &path).is_err() {
            continue;
        }
        let Ok(have) = std::fs::read_to_string(&path) else {
            continue;
        };
        let have = layer::digest(&have);
        // Only a path this layer offers a workspace folder ([`want`]). The ledger also notes
        // the agents and skills a CHECKOUT is offered, at the same relative paths, and an
        // offer made to a checkout vouches for nothing in a workspace folder.
        if record.recorded(rel).contains(&have)
            && offered_here(rel)
            && crate::guest::vouched(offered, rel, &have)
        {
            out.push(rel.clone());
        }
    }
    out
}

/// The SHALLOWEST directory above `path` whose own check meets `code` — else `path` itself,
/// whose check already met it.
///
/// A check through a symlink fails at the first component that fails, not at the path it asked
/// about: with a workspace's `refs/` a loop, `refs/README.md` answers ELOOP, and "fix the
/// symlink loop at refs/README.md" sends the reader to a file that is no link at all. A
/// directory that answers anything but `code` is not taken for the cause, so a filesystem that
/// changed in between costs precision and never names a path that did not fail that way.
pub fn stopped_at(path: &Path, code: Option<i32>) -> PathBuf {
    let mut parents: Vec<&Path> = path.ancestors().skip(1).collect();
    parents.reverse();
    for up in parents {
        if matches!(up.metadata(), Err(e) if e.raw_os_error() == code && !crate::memstore::is_absent(&e))
        {
            return up.to_path_buf();
        }
    }
    path.to_path_buf()
}

/// Whether `path` resolves inside `root`, with BOTH ends resolved.
fn resolves_inside(path: &Path, root: &Path) -> bool {
    match (contain::resolved(path), contain::resolved(root)) {
        (Some(here), Some(root)) => here.starts_with(&root),
        _ => false,
    }
}

/// The top-level segment of a relative path — the whole of it when there is no slash.
fn root_of(rel: &str) -> &str {
    rel.split('/').next().unwrap_or(rel)
}

/// Remove what charter generated here and no longer generates.
///
/// A generated file only ever ARRIVED until the layer started carrying the plane's `ask` and
/// `deny` rules. Now a plane that drops its last rule stops wanting a whole generated file —
/// so without this, a restriction could be put in force in every workspace and never lifted
/// from any of them. `charter guard` has no remove verb; hand-editing the plane's settings is
/// how a rule goes, and a mirror that is one-way turns that edit into a lie.
fn withdraw(
    plane: &Path,
    dir: &Path,
    want_all: &BTreeMap<String, String>,
    record: &mut Record,
    offered: &BTreeMap<String, Vec<String>>,
) -> Vec<Row> {
    let mut rows = Vec::new();
    for rel in unwanted(plane, dir, want_all, record, offered) {
        // Not a path checked and then removed by path: the classification's answer is stale
        // by the unlink, so the unlink happens inside the folders the walk down held, and a
        // folder swapped for a link on the way cannot carry it out of `dir` (#1583).
        if layer::withdraw_below(dir, rel.as_str()).is_err() {
            // Gone since it was read, reached through a link, or held by a directory that will
            // not let go. It stays in the record and stays reported `unwanted`, and the next
            // wire tries again.
            continue;
        }
        record.forget(&rel);
        rows.push(Row {
            rel,
            did: Did::Removed,
        });
    }
    // An entry whose file is PROVED gone vouches for nothing, and is forgotten. Proved:
    // `lexists` answered False for a refused `lstat` too, and one EACCES during a launch
    // forgot `settings.json` for good.
    let gone: Vec<String> = record
        .paths()
        .filter(|rel| {
            matches!(dir.join(rel.as_str()).symlink_metadata(), Err(ref e) if crate::memstore::is_absent(e))
        })
        .cloned()
        .collect();
    for rel in gone {
        record.forget(&rel);
    }
    rows
}

/// Materialise the harness layer into the workspace directory `dir`.
///
/// **Refresh, not create-once.** A file whose digest is in the record is charter's, and
/// charter brings its own files up to date; a file whose digest is not is the operator's and
/// is left exactly as found. Since #1583 the record is not enough alone: the project must
/// have noted offering that text here too ([`found`]), or the file is left as it is and
/// reported [`Did::Unconfirmed`]. Create-once was right about the operator's files and wrong about
/// charter's: a document generated by an old version survived every upgrade afterwards while
/// `doctor` reported the tree wired (ADR 0015).
///
/// Called from [`scaffold`], so a launch gets it. `charter workspace reinit` is the repair.
///
/// # The checkouts inside it, too
///
/// A clone or a linked worktree under the workspace is a git root of its own, and it cuts a
/// chat off from the plane far more sharply than this directory does: project settings are
/// read from the session's own directory and the walk for agents and skills stops at the git
/// root. So every checkout [`guest_trees`] finds is wired through [`crate::guest::wire`] and
/// its rows come back prefixed with [`checkout_label`] — which is what makes `reinit` the
/// repair for a clone, for a worktree beside it, and for a piece, as well as for the
/// directory around them.
///
/// **Its root is contained as one decision, exactly as this directory's is.** A
/// `workspaces/<ws>/<name>` linked at a repository OUTSIDE the plane is a tree whose own root
/// has already escaped, and [`crate::guest`]'s per-file guard is relative to *that* root, so
/// it would pass. charter's `_wired_tree_ok` asks the same question of the tree root, and so
/// does this.
///
/// # One listing per repository, for the whole call
///
/// The pieces and the shared exclude block ask git the same question, and a launch wires
/// every checkout twice over. [`crate::worktree::listing::answers`] is opened here, so each
/// repository's `git worktree list` is paid for once for the length of this wire however many
/// readers want it.
pub fn wire(plane: &Path, dir: &Path) -> Vec<Row> {
    let _answers = crate::worktree::listing::answers();
    let want_all = want(plane);
    if !writable_directory(plane, dir) {
        // Writing the layer through a `workspaces/<ws>` that resolves out of the plane would
        // land the whole thing outside. Refused as one decision, and no tree behind the link
        // is descended.
        return vec![Row {
            rel: layer::MARKER.to_owned(),
            did: Did::Blocked,
        }];
    }
    // A record charter wrote under its old name is charter's record still: moved to the
    // purlis name before it is read, bytes unchanged, so what it vouched for stays charter's.
    let _ = layer::carry_over(dir);
    // Before anything is written or withdrawn: a copy goes later only if this says purlis
    // offered that text here.
    crate::guest::note_offered(plane, &want_all);
    let offered = crate::guest::read_offered(plane);
    let mut record = layer::read_record(dir);
    let before = record.clone();
    let mut rows = withdraw(plane, dir, &want_all, &mut record, &offered);
    let mut writes: Vec<(String, Found)> = Vec::new();
    let mut published = before;
    for (rel, what) in found(plane, dir, &want_all, &record, &offered) {
        match what {
            Found::Unreadable => rows.push(Row {
                rel,
                did: Did::Unreadable,
            }),
            Found::Foreign => rows.push(Row {
                rel,
                did: Did::Foreign,
            }),
            // Never written over: purlis cannot confirm the text there is its own.
            Found::Unconfirmed => rows.push(Row {
                rel,
                did: Did::Unconfirmed,
            }),
            Found::Ok => {
                // A record that does not say what an `ok` file holds is SETTLED on it:
                // pending over a write that finished, or settled by a launch that lost a race
                // to another, whose record then named text the file no longer held — and the
                // plane's next move would have called charter's own file foreign.
                if record.names(&rel)
                    && let Some(text) = want_all.get(&rel)
                {
                    record.settle(&rel, layer::digest(text));
                }
                rows.push(Row {
                    rel,
                    did: Did::Present,
                });
            }
            // Already tried above; there is nothing here to write.
            Found::Unwanted => {}
            Found::Missing | Found::Stale => writes.push((rel, what)),
        }
    }
    if !writes.is_empty() {
        // **Intent first.** Every file about to be written is published as PENDING — the
        // digests it may hold while the write is under way — before a byte of it changes, so
        // no kill between the write and its record can leave a file on disk that the record
        // calls somebody else's.
        //
        // Unlike a checkout, a workspace directory does not abandon the write when this
        // publish fails: there is no exclude block whose line the next launch would drop, so
        // a record that cannot be published costs only the record and the layer still lands.
        let mut intent = record.clone();
        for (rel, _) in &writes {
            if let Some(text) = want_all.get(rel) {
                intent.pend(rel, layer::digest(text));
            }
        }
        let _ = layer::publish(dir, &intent);
        record = intent;
        // What the settle below is compared with: the record THIS pass published, never the
        // one it read. Compared with the record read at the start, a pass that wrote a deleted
        // file again ended on that very record and skipped the settle, so the pending entry it
        // had just published stayed pending for good.
        published = record.clone();
    }
    for (rel, what) in writes {
        let Some(text) = want_all.get(&rel) else {
            continue;
        };
        match layer::write_into(dir, &rel, text) {
            Err(_) => {
                // The entry stays pending and the file holds what it held — writes are whole —
                // and the next wire writes it again.
                rows.push(Row {
                    rel,
                    did: Did::Blocked,
                });
            }
            Ok(()) => {
                record.settle(&rel, layer::digest(text));
                rows.push(Row {
                    rel,
                    did: if what == Found::Missing {
                        Did::Created
                    } else {
                        Did::Refreshed
                    },
                });
            }
        }
    }
    if record != published {
        // Only when something changed. Rewriting the record on every launch would move a
        // workspace's mtimes for a call that changed nothing.
        let _ = layer::publish(dir, &record);
    }
    // NOT sorted: the order is the Python's — what was withdrawn, then what was found in
    // path order, then what was written. `reinit` prints these one by one, so an order of
    // this port's own would be a different report for one plane.
    //
    // The checkouts come AFTER the directory's own rows, which is charter's order too: its
    // `wire_harnesses` materialises the workspace directory, then extends with each guest
    // tree's rows.
    let (trees, unlisted) = guest_trees(dir);
    for tree in trees {
        rows.extend(guest_rows(plane, dir, &tree));
    }
    // Last, as charter appends them: a listing git could not give is a fact about the
    // repository rather than about any one path in it.
    rows.extend(unlisted_rows(dir, &unlisted));
    rows
}

/// One checkout's rows, labelled the way charter labels them — `<checkout>/<path>`.
///
/// The status vocabularies are two spellings of one question and the mapping is stated here
/// rather than at each printing site, so `reinit` and anything that reports a layer cannot
/// come to disagree about what one row means.
///
/// A tree whose ROOT resolves out of the plane is refused whole and nothing in it is read or
/// written — `_wired_tree_ok`. It is reported at the checkout's exclude path rather than at
/// the marker, because that is the path a reader goes to.
fn guest_rows(plane: &Path, dir: &Path, tree: &Path) -> Vec<Row> {
    let label = checkout_label(dir, tree);
    let exclude = format!("{label}/.git/info/exclude");
    // `_wired_tree_ok`, against ONE base and not two — and that is a fact about this binary
    // rather than a simplification. charter answers a piece to `[plane] worktrees` or
    // `$CHARTER_WORKTREES` when one is declared; [`crate::worktree`] **refuses a relocated
    // root by name rather than following it** (`Refusal::Relocated`). So every checkout this
    // binary can reach — a clone, a worktree beside it, a piece at `.worktrees/<repo>/<piece>`
    // — is spelled under `workspaces/<ws>` and answers to `workspaces/`, exactly as the
    // workspace directory does. The day the relocated root is ported, this is the call site
    // that grows its second base.
    if !writable_directory(plane, tree) {
        return vec![Row {
            rel: exclude,
            did: Did::Blocked,
        }];
    }
    let wired = crate::guest::wire(plane, tree);
    if matches!(wired.hidden, crate::guest::Hidden::Blocked(..)) {
        // Nothing was written, so there are no file rows to report: the block is the one
        // fact, and it is the one charter's `blocked` exclude row carries too.
        return vec![Row {
            rel: exclude,
            did: Did::Blocked,
        }];
    }
    let mut rows: Vec<Row> = wired
        .rows
        .into_iter()
        .map(|row| Row {
            rel: format!("{label}/{}", row.rel),
            did: match row.status {
                crate::guest::Status::Created => Did::Created,
                crate::guest::Status::Refreshed => Did::Refreshed,
                crate::guest::Status::Current => Did::Present,
                crate::guest::Status::Foreign => Did::Foreign,
                crate::guest::Status::Theirs => Did::HarnessBehind,
                crate::guest::Status::Blocked => Did::Blocked,
                crate::guest::Status::Unreadable => Did::Unreadable,
                crate::guest::Status::Withheld => Did::Withheld,
                crate::guest::Status::Unrecorded => Did::Unrecorded,
                crate::guest::Status::Removed => Did::Removed,
                crate::guest::Status::Unconfirmed => Did::Unconfirmed,
            },
        })
        .collect();
    // The block's own row, and only when there is one to report: charter appends it when it
    // owns something in the checkout or the block was not already right, and a checkout where
    // every wanted path holds somebody else's file has neither.
    let did = match wired.block {
        crate::guest::Block::Created => Did::Created,
        crate::guest::Block::Refreshed => Did::Refreshed,
        crate::guest::Block::Present => Did::Present,
        crate::guest::Block::Unhidden => Did::Unhidden,
        crate::guest::Block::Blocked => Did::Blocked,
        crate::guest::Block::Untouched => return rows,
    };
    rows.push(Row { rel: exclude, did });
    rows
}

/// How a row of workspace `dir` names the guest checkout `tree` — charter's
/// `checkout_label`.
///
/// The path from the workspace directory when `tree` is inside it (`svc`,
/// `.worktrees/svc/p1`), and the absolute path of a piece under a relocated worktree root,
/// which no path from the workspace spells readably.
///
/// `tree.file_name()` was the label while every guest was a direct child. A piece's name is
/// its own directory's alone, so two repositories' `p1` would read as one checkout, and a
/// piece named like a clone as that clone.
pub fn checkout_label(dir: &Path, tree: &Path) -> String {
    let inside = normalise(dir);
    let spelled = normalise(tree);
    match spelled.strip_prefix(&inside) {
        Ok(rel) if !rel.as_os_str().is_empty() => rel.display().to_string(),
        _ => spelled.display().to_string(),
    }
}

/// A path with `.` and `..` folded lexically — never `canonicalize`, which would also follow
/// links charter was not asked to follow. charter's `os.path.normpath`.
fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `(checkout, the path inside it)` for a layer row of workspace `dir` naming a file in one
/// of its guest checkouts — `None` for a row of the workspace directory's own. charter's
/// `checkout_row`.
///
/// One answer for every command that words a checkout's file differently from the
/// workspace's, because only a checkout is somebody else's repository: a file charter did not
/// write is committed there with `git add -f`, and nowhere else.
///
/// A PIECE's row is read back by path arithmetic — `.worktrees/<repo>/<piece>/…` — rather
/// than by asking git again for every row a command prints. No `.git` test on that branch,
/// which the clone's spelling below needs and this one does not: a row of the workspace
/// directory's own never lies under a piece's root.
pub fn checkout_row(dir: &Path, rel: &str) -> Option<(PathBuf, String)> {
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.len() > 3 && parts[0] == crate::worktree::DIR_NAME {
        let tree = dir.join(parts[0]).join(parts[1]).join(parts[2]);
        return Some((tree, parts[3..].join("/")));
    }
    let (head, inner) = rel.split_once('/')?;
    (!inner.is_empty() && crate::guest::git_dir(&dir.join(head)).is_some())
        .then(|| (dir.join(head), inner.to_string()))
}

/// `(every guest checkout of workspace `dir`, the checkouts whose worktrees git could not
/// list)` — charter's `guest_trees` and `unlisted`, which come out of one listing.
///
/// The direct children are a clone or a worktree beside it. The rest are PIECES, at
/// `.worktrees/<repo>/<piece>`: a piece sits two levels below its workspace and `.worktrees`
/// holds no `.git` of its own, so every piece `charter wt add` cut — the directory it tells a
/// worker to start a session in — got no layer from a launch, no `reinit` repair and no row
/// until this listing existed (charter#951).
///
/// **Asked of git, the only registry** (ADR 0011): a worktree made by hand at the layout's
/// path counts and one removed by hand does not. Not a walk of `.worktrees/`, which would
/// take a directory somebody copied for a live worktree. Through
/// [`crate::worktree::listing`], so a launch or a status render asks each repository once
/// however many readers want the answer, and not at all where the common directory has no
/// `worktrees/`.
///
/// A listing git cannot give is NAMED rather than passed over in silence (charter#1072): a
/// worktree nobody can see is not a worktree that is fine.
pub fn guest_trees(dir: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let children = checkouts(dir);
    let (mut pieces, unlisted) = pieces(dir, &children);
    let mut all = children;
    all.append(&mut pieces);
    (all, unlisted)
}

/// Every checkout directly inside the workspace directory that charter is a GUEST in — a
/// clone or a linked worktree, each a git root of its own.
///
/// `.git` is a DIRECTORY in a clone and a FILE reading `gitdir: <path>` in a linked worktree,
/// and both count: this asks "where would a chat's cwd be cut off from the plane's layer",
/// and a worktree's root is a git boundary exactly as a clone's is.
///
/// **Asked through [`crate::guest::git_dir`], never as "is there a `.git` here".** A linked
/// worktree's `.git` is a FILE naming its admin directory, and one charter cannot reach — an
/// unreadable `worktrees/`, a `gitdir:` that points nowhere — is not a checkout charter can
/// wire. Measured on the differential: with `worktrees/` at mode 000 the shallower test
/// reported that worktree `blocked` where charter passes over it.
pub fn checkouts(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| crate::guest::git_dir(p).is_some())
        .collect();
    out.sort();
    out
}

/// `(pieces, the checkouts whose listing git could not give)` — charter's `_pieces`.
///
/// One entry per REPOSITORY in the second list, under the first checkout of it: a clone and a
/// worktree of it at the workspace's top level share one list.
fn pieces(dir: &Path, children: &[PathBuf]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    use crate::worktree::listing;

    let mut found: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    let mut unlisted: Vec<(PathBuf, PathBuf)> = Vec::new();
    for tree in children {
        // Never `None`: every one of `children` has a git directory, which is all the
        // exclude's answer needs.
        let Some(exclude) = crate::guest::exclude_file(tree) else {
            continue;
        };
        let Some(common) = exclude.parent().and_then(Path::parent) else {
            continue;
        };
        let admin = common.join("worktrees");
        let refused = match std::fs::read_dir(&admin) {
            Ok(_) => false,
            Err(e) if listing::gone(&e) => continue,
            // Still asked of git, which may read what charter cannot. But measured on git
            // 2.50.1, a git that cannot read `worktrees/` lists the clone alone and exits 0,
            // so its answer is no evidence the list is whole.
            Err(_) => true,
        };
        let listed = listing::worktree_list(tree, common);
        // The exit code too: a git that fails prints its `fatal:` to stderr and nothing to
        // parse, which reads as a repository with no pieces.
        let ok = matches!(&listed, Ok(run) if run.ok());
        if refused || !ok {
            let key = contain::resolved(common).unwrap_or_else(|| common.to_path_buf());
            if !unlisted.iter().any(|(seen, _)| *seen == key) {
                unlisted.push((key, tree.clone()));
            }
        }
        let Ok(run) = listed else { continue };
        for row in crate::worktree::porcelain::parse(&run.out) {
            // Under THIS workspace's root only: another workspace's piece of the same
            // repository is that workspace's to wire and to take away.
            let Some(at) = piece_at(dir, &row.path) else {
                continue;
            };
            // No `prunable` or `bare` test: a worktree git calls prunable has no `.git` that
            // points anywhere, and a bare repository's entry is a git directory with none, so
            // `git_dir` answers both.
            if crate::guest::git_dir(&at).is_none() {
                continue;
            }
            // Keyed by where it LANDS, because two checkouts of one repository — a clone and
            // a worktree of it at the workspace's top level — list the same pieces.
            let key = contain::resolved(&at).unwrap_or_else(|| at.clone());
            found.insert(key, at);
        }
    }
    (
        found.into_values().collect(),
        unlisted.into_iter().map(|(_, tree)| tree).collect(),
    )
}

/// Where a piece `path` lives under workspace `dir`, spelled under the ROOT it matched —
/// `None` when it is not one of this workspace's pieces. charter's `_piece_at`, narrowed to
/// the one root this binary has (see [`guest_rows`] on the relocated root).
///
/// **The spelling is the root's own, never `path`'s.** Git records a worktree by its resolved
/// path — measured on git 2.50.1, a piece cut through a linked `.worktrees` is listed where
/// the link points — while charter spells a plane as it was handed it. So the root is tried
/// as spelled and then resolved, and the answer is always the root's spelling, which is what
/// the containment test above judges.
fn piece_at(dir: &Path, path: &Path) -> Option<PathBuf> {
    let root = dir.join(crate::worktree::DIR_NAME);
    let spellings = [
        (normalise(path), normalise(&root)),
        (
            contain::resolved(path)?,
            contain::resolved(&root).unwrap_or_else(|| normalise(&root)),
        ),
    ];
    for (here, there) in spellings {
        // `strip_prefix`, never a string prefix: `.worktrees-old/…` begins with `.worktrees`,
        // and so does another repository's `<root>/api-old/…` beside `<root>/api`.
        let Ok(rel) = here.strip_prefix(&there) else {
            continue;
        };
        let parts: Vec<_> = rel.components().collect();
        if parts.len() >= 2 {
            return Some(root.join(parts[0].as_os_str()).join(parts[1].as_os_str()));
        }
    }
    None
}

/// One `unlisted` row per checkout of workspace `dir` whose worktrees git could not list.
///
/// Labelled `<checkout>/.git/worktrees`, where git keeps that list, as a checkout's exclude
/// row is labelled `.git/info/exclude`: a row of a checkout's OWN, so [`checkout_row`] reads
/// it back to that checkout for the command printing it to word what clears it.
fn unlisted_rows(dir: &Path, unlisted: &[PathBuf]) -> Vec<Row> {
    unlisted
        .iter()
        .map(|tree| Row {
            rel: format!("{}/.git/worktrees", checkout_label(dir, tree)),
            did: Did::Unlisted,
        })
        .collect()
}

/// What clears an [`Did::Unlisted`] row for `tree`: the remedy for the directory git keeps
/// its worktrees in, worded for the errno reading it meets NOW. charter's `unlisted_fix`.
///
/// No errno when it reads now and git still gave no list — a git that failed, timed out or
/// could not be run — and the wording reads that as a refusal, as it reads any errno it has
/// not measured.
pub fn unlisted_fix(tree: &Path) -> String {
    let Some(exclude) = crate::guest::exclude_file(tree) else {
        return String::new();
    };
    let Some(admin) = exclude
        .parent()
        .and_then(Path::parent)
        .map(|common| common.join("worktrees"))
    else {
        return String::new();
    };
    let code = std::fs::read_dir(&admin)
        .err()
        .and_then(|e| e.raw_os_error());
    let shown = admin.display().to_string();
    crate::memstore::uncheckable_fix(code, &shown, &shown)
}

// --------------------------------------------------------------------------------------- //
// the structure stamp                                                                       //
// --------------------------------------------------------------------------------------- //

/// The stamp's path, migrating an older spelling (`.charter-structure`, `.edm-structure`) the
/// first time it is read.
///
/// Rename rather than re-stamp, so a genuinely older marker keeps its own version instead of
/// being claimed as current. The newest spelling there wins (V93e) and is the one moved; any
/// older one beside it is a leftover, removed.
pub fn structure_marker(dir: &Path) -> PathBuf {
    let new = dir.join(STRUCTURE_MARKER);
    let at = crate::names::STRUCTURE_STAMP.in_dir(dir, |p| p.symlink_metadata().is_ok());
    if at.found == crate::names::Found::Old && std::fs::rename(&at.name, &new).is_err() {
        // Unreadable, or across devices: still read the old one.
        return at.name;
    }
    for leftover in at.leftovers {
        // The newest one already won.
        let _ = std::fs::remove_file(&leftover);
    }
    new
}

/// `(version, blocker)` for the structure stamp — the ONE read of it.
///
/// Opened `O_NOFOLLOW | O_NONBLOCK` and judged by the descriptor: `read_to_string` opened a
/// FIFO at that name and waited for a writer that never came, so one stray local file froze
/// `reinit` and every status-line render in the workspace. A link is not read through either,
/// wherever it points: charter never writes the stamp through one, so a version read through
/// one is not charter's.
pub fn stamp(dir: &Path) -> (u32, Option<Blocker>) {
    let (version, blocker) = read_stamp(dir);
    (version.unwrap_or(0), blocker)
}

/// [`stamp`], with a stamp nobody could read kept apart from an old one (#1289).
///
/// `None` is a stamp that is there, or may be, but that the filesystem would not answer
/// about: the workspace's folder cannot be listed, or the stamp cannot be opened or read. An
/// absent stamp, a link, a directory and text that is not a number are all `Some(0)`: each is
/// a stamp charter can see is not current, and `reinit` either writes it or names it.
fn read_stamp(dir: &Path) -> (Option<u32>, Option<Blocker>) {
    let marker = structure_marker(dir);
    let file = match contain::open_no_link(dir, &marker) {
        Ok(file) => file,
        Err(_) => {
            // Absent, a link (`O_NOFOLLOW`'s ELOOP), or unanswered.
            let blocker = in_the_way(dir, &marker);
            let absent = blocker.is_none()
                && marker.symlink_metadata().is_err_and(|e| {
                    matches!(
                        e.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                    )
                });
            let version = (blocker.is_some() || absent).then_some(0);
            return (version, blocker);
        }
    };
    let Ok(meta) = file.metadata() else {
        return (None, None);
    };
    if !meta.is_file() {
        return (
            Some(0),
            Some(Blocker {
                path: marker,
                why: if meta.is_dir() {
                    Why::IsADirectory
                } else {
                    Why::NotARegularFile
                },
            }),
        );
    }
    // A bounded read, not `read_to_string`: the stamp is a few bytes, and a regular file at
    // that name that is not one is not charter's to swallow whole.
    use std::io::Read;
    let mut text = String::new();
    match file.take(4096).read_to_string(&mut text) {
        Ok(_) => (Some(text.trim().parse().unwrap_or(0)), None),
        // Bytes that are not text are a stamp charter did not write, so not a current one.
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => (Some(0), None),
        Err(_) => (None, None),
    }
}

/// Write the stamp, never through a link.
///
/// `O_NOFOLLOW` refuses a link wherever it points, `O_NONBLOCK` a FIFO nobody reads instead of
/// hanging the launch, and a directory refuses itself. The stamp is local, so a refused stamp
/// costs only the stamp: the workspace re-flags stale, and nothing is written into a tree
/// charter did not choose.
pub fn write_stamp(dir: &Path) -> std::io::Result<()> {
    use std::io::Write;
    let marker = structure_marker(dir);
    let mut file = contain::create_no_link(dir, &marker)?;
    file.write_all(format!("{STRUCTURE_VERSION}\n").as_bytes())
}

/// Why a path of the layout is not the thing the layout needs there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// A symlink whose target is not there. Charter writes nothing through it.
    Dangling,
    /// Something that is not a directory, where a directory goes.
    NotADirectory,
    /// A symlink charter will not follow — where a file belongs, wherever it points, or
    /// where a directory belongs and it does not land on one inside the plane.
    IsALink,
    /// A directory where a file goes.
    IsADirectory,
    /// Any other file that is not a regular one — a FIFO, a socket, a device.
    NotARegularFile,
}

/// The path that stands where the layout needs something else, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocker {
    pub path: PathBuf,
    pub why: Why,
}

/// The path from `base` down to `p` that stands where the layout needs something else — or
/// `None` when nothing does.
///
/// "Gone" is read as "create it", and a create through a link to nowhere, or under a file,
/// raises out of `workspace reinit`. So "gone because something stands where its directory
/// goes" is told apart here, ONCE, for `reinit` to name and for [`scaffold`] to write nothing
/// under: the two cannot disagree about one path.
///
/// A link where a FILE belongs is in the way wherever it points — git stores a symlink as a
/// symlink, so a teammate's commit can put one at `refs/README.md`, and a plain write through
/// it creates a file wherever it points. A link where a DIRECTORY belongs is in the way unless
/// it lands on a directory inside the plane's data.
///
/// Nothing ABOVE `base` is asked: the plane root may be a link for reasons nobody committed
/// (`/var` is one on macOS), and every caller has already reached `base` through it.
pub fn in_the_way(base: &Path, p: &Path) -> Option<Blocker> {
    let Ok(below) = p.strip_prefix(base) else {
        return None;
    };
    let mut walk = vec![base.to_path_buf()];
    let steps: Vec<_> = below.components().collect();
    for step in steps.iter().take(steps.len().saturating_sub(1)) {
        let mut next = walk.last().expect("seeded with base").clone();
        next.push(step);
        walk.push(next);
    }
    walk.push(p.to_path_buf());
    let last = walk.len() - 1;
    for (n, q) in walk.iter().enumerate() {
        let Ok(link) = q.symlink_metadata().map(|m| m.file_type().is_symlink()) else {
            // ONE clause. Absent is what a create makes, and unanswered is not charter's to
            // call in the way either; neither belongs here.
            return None;
        };
        if n == last {
            // The leaf. A link there is in the way wherever it points: no target makes a file
            // link one charter writes through.
            return link.then(|| Blocker {
                path: p.to_path_buf(),
                why: Why::IsALink,
            });
        }
        let Ok(meta) = q.metadata() else {
            // The name is THERE — the `symlink_metadata` above answered for it — and resolves
            // to nothing. A create under it raises, so it is in the way rather than gone.
            return Some(Blocker {
                path: q.clone(),
                why: Why::Dangling,
            });
        };
        if !meta.is_dir() {
            return Some(Blocker {
                path: q.clone(),
                why: if link {
                    Why::IsALink
                } else {
                    Why::NotADirectory
                },
            });
        }
        // A directory of the layout that IS a link answers to the same containment every
        // write here does — and to a DIFFERENT tree depending on which directory it is, which
        // is the Python's split verbatim. The workspace directory itself must resolve inside
        // `workspaces/`, because that is the test `scaffold` and the layer refuse the whole
        // directory by; anything beneath it must resolve inside the workspace.
        //
        // Both ends are RESOLVED before they are compared. On macOS `/var` is itself a link,
        // so a lexical `starts_with` against an unresolved base calls every link in a
        // `$TMPDIR` plane an escape.
        let inside = if n == 0 {
            base.parent().unwrap_or(base)
        } else {
            base
        };
        if link && !resolves_inside(q, inside) {
            return Some(Blocker {
                path: q.clone(),
                why: Why::IsALink,
            });
        }
    }
    None
}

/// What one baseline path answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baseline {
    pub rel: &'static str,
    pub path: PathBuf,
    /// `Some(true)`/`Some(false)` when the filesystem answered, `None` when it would not.
    pub there: Option<bool>,
    /// The errno behind a `there` of `None`, which is what words the repair: a symlink loop
    /// names the link, because no permission bit is in its way.
    pub code: Option<i32>,
    /// What stands where this path's directory — or the path itself — goes.
    pub blocker: Option<Blocker>,
}

impl Baseline {
    /// Whether [`scaffold`] may write this path: it is certainly not there, and nothing is in
    /// the way.
    fn clear(&self) -> bool {
        self.there == Some(false) && self.blocker.is_none()
    }
}

/// `{rel: what it answered}` for every baseline path — the ONE classification
/// [`structure_status`] reports and [`scaffold`] writes by.
pub fn baseline(dir: &Path) -> Vec<Baseline> {
    BASELINE
        .into_iter()
        .map(|rel| {
            let path = dir.join(rel);
            // Through the link (`metadata`, not `symlink_metadata`): the question "is
            // `workspace.json` there" means the manifest behind the link, and a dangling link
            // is a manifest that is not there.
            let (there, code) = match path.metadata() {
                Ok(_) => (Some(true), None),
                Err(ref e) if crate::memstore::is_absent(e) => (Some(false), None),
                Err(e) => (None, e.raw_os_error()),
            };
            // Asked of what answered "there" as well as of what answered "gone": a link at a
            // baseline name to a file that IS there answers "there" through the link, and is
            // in the way all the same.
            let blocker = there.and_then(|_| in_the_way(dir, &path));
            Baseline {
                rel,
                path,
                there,
                code,
                blocker,
            }
        })
        .collect()
}

/// Is the workspace's on-disk layout current?
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Structure {
    /// No baseline file missing AND the stamp up to date.
    pub ok: bool,
    /// Baseline paths that are certainly not there with nothing in the way — the ones
    /// [`scaffold`] adds.
    pub missing: Vec<&'static str>,
    /// Baseline paths the filesystem would not answer for. Not `missing`: scaffolding over a
    /// path charter cannot see is a write into something charter cannot see.
    pub unreadable: Vec<Baseline>,
    /// Baseline paths, and the stamp, with something in the way.
    pub in_the_way: Vec<(String, Blocker)>,
    /// The stamp's version, 0 when it is absent, not a regular file or not a number.
    pub version: u32,
    pub target: u32,
    /// The stamp is there, or may be, but the filesystem would not answer about it (#1289).
    /// `version` then reads 0, which is no version to be behind: see [`needs_reinit`].
    pub stamp_unread: bool,
}

/// [`Structure`] for the workspace directory `dir`.
pub fn structure_status(dir: &Path) -> Structure {
    let seen = baseline(dir);
    let missing: Vec<&'static str> = seen.iter().filter(|b| b.clear()).map(|b| b.rel).collect();
    let (read, stamp_blocker) = read_stamp(dir);
    let version = read.unwrap_or(0);
    let mut blocked: Vec<(String, Blocker)> = seen
        .iter()
        .filter_map(|b| b.blocker.clone().map(|w| (b.rel.to_owned(), w)))
        .collect();
    // The stamp is a row of `in_the_way` too, never of `missing`: an absent stamp is the
    // version line's to report, and one that is not a regular file is what `reinit` names.
    if let Some(why) = stamp_blocker {
        blocked.push((STRUCTURE_MARKER.to_owned(), why));
    }
    Structure {
        ok: missing.is_empty() && version >= STRUCTURE_VERSION,
        missing,
        unreadable: seen.iter().filter(|b| b.there.is_none()).cloned().collect(),
        in_the_way: blocked,
        version,
        target: STRUCTURE_VERSION,
        stamp_unread: read.is_none(),
    }
}

/// Whether an existing workspace's structure is known to be stale.
///
/// **Not one it cannot read** (#1289), as [`crate::footer`]'s reading says it: a stamp the
/// filesystem will not answer about is unknown, not old, and a `reinit` could not reach it
/// either, so `false`. A baseline file that is certainly missing is still stale, stamp or not:
/// `reinit` can create it whether or not it can stamp.
pub fn needs_reinit(dir: &Path) -> bool {
    if !dir.exists() {
        return false;
    }
    let status = structure_status(dir);
    !status.missing.is_empty() || (!status.stamp_unread && status.version < status.target)
}

/// Create a workspace's baseline structure: `memory/` (the per-file DB and its index),
/// `refs/`, the `workspace.md` charter, the `workspace.json` manifest, the harness layer,
/// and the structure-version stamp. Idempotent and additive.
///
/// **Writes nothing when the workspace directory itself resolves outside the plane.** Below
/// that, each baseline path is judged by [`baseline`] and the stamp by the kernel: nothing is
/// written for a path that could not be checked or has something in the way, so `reinit`
/// names what this refuses rather than raising out of it.
pub fn scaffold(
    plane: &crate::workspaces::Plane,
    name: &str,
    now: chrono::DateTime<chrono::Utc>,
    author: &str,
) -> Vec<Row> {
    scaffold_as(
        plane,
        name,
        now,
        author,
        crate::sandbox::chat_is_sandboxed(),
    )
}

/// [`scaffold`], told whether it runs in a sandboxed chat (#2046): there, the workspace's
/// manifest is left for the app, which makes it as it next makes the workspace's structure
/// (`wscmd::ensure`, a clone the app makes for the chat).
fn scaffold_as(
    plane: &crate::workspaces::Plane,
    name: &str,
    now: chrono::DateTime<chrono::Utc>,
    author: &str,
    sandboxed: bool,
) -> Vec<Row> {
    let Ok(workspace) = plane.workspace(name) else {
        return Vec::new();
    };
    let dir = workspace.dir().to_path_buf();
    if !writable_directory(plane.root(), &dir) {
        return Vec::new();
    }
    let clear: BTreeMap<&str, bool> = baseline(&dir)
        .into_iter()
        .map(|b| (b.rel, b.clear()))
        .collect();
    let is_clear = |rel: &str| clear.get(rel).copied().unwrap_or(false);
    if is_clear("memory/MEMORY.md") {
        let _ = workspace.scaffold_memory();
    }
    if is_clear("refs/README.md") {
        let refs = dir.join("refs");
        if std::fs::create_dir_all(&refs).is_ok() {
            // Created only where it is certainly absent, and through the containment gate at
            // the exact path opened — the check above is a `stat`, and a link planted between
            // the two is followed by a plain write.
            let readme = refs.join("README.md");
            if let Ok(mut file) = contain::create_no_link(&dir, &readme) {
                use std::io::Write;
                let _ = file.write_all(
                    format!(
                        "# {name} — task references\n\nDrop docs, links, and snippets for \
                         this task here (local, gitignored).\n"
                    )
                    .as_bytes(),
                );
            }
        }
    }
    if is_clear("workspace.md") {
        let _ = workspace.scaffold_charter();
    }
    // A sandboxed chat writes no workspace's manifest (#2046): the app scaffolds it.
    if is_clear("workspace.json") && !sandboxed {
        let _ = workspace.scaffold_manifest(now, author);
    }
    let layer = wire(plane.root(), &dir);
    // The stamp is LOCAL, so a refused write costs only the stamp: the workspace re-flags
    // stale and nothing is written into a tree charter did not choose. `reinit` reads it back
    // rather than trusting this call, because a swallowed failure printed "added structure
    // v0 → v5" over a stamp that was never written.
    let _ = write_stamp(&dir);
    layer
}

#[cfg(test)]
mod tests;
