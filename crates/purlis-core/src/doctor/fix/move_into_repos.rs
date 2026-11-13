// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`move-into-repos`** (#5374; ADR 0114, decision 4): **Move into repos/**, the one step that
//! moves a workspace's clones from its root into its `repos/` folder. It runs only when the
//! operator asks: both layouts are read for ever, so nothing here is ever needed.
//!
//! # One workspace at a time, all or nothing for each
//!
//! [`apply`] moves each workspace that has clones at its root, by [`move_workspace`]:
//!
//! 1. **Refused while any chat is open in the workspace**, naming them. A running harness holds
//!    the clone's path in its environment and every tool call it has queued, and a chat the app
//!    will reopen starts where its record says. The caller says which chats are open, because
//!    only the caller knows: the window from its own chats, the terminal from the app's record
//!    ([`open_in_record`]). Refused too where a clone's place in `repos/` is taken, or `repos`
//!    is no folder new repos go in ([`crate::repofolder::new_folder`]). And refused where a
//!    branch folder has a submodule checked out, naming each: its `.git` file and its module's
//!    `core.worktree` are relative paths between the branch folder and the clone's `.git`, so
//!    the move puts both a level off, and `git worktree repair` mends neither. Refusing is the
//!    simpler safe answer than rewriting them.
//! 2. The bytes of every file `git worktree repair` may rewrite are kept: each linked worktree's
//!    `.git` file, and each `.git/worktrees/<id>/gitdir` in the clone.
//! 3. Each clone is moved with one `rename(2)`, then `git worktree repair` runs in it with the
//!    new path of each linked worktree that moved with it, which mends both directions: a branch
//!    folder in `.worktrees/` finds its clone again, and a worktree inside the clone (Claude
//!    Code's own) is found by it. Each branch folder is then asked whether git reads it, and
//!    every submodule checked out in the clone or a branch folder is asked for its `git status`.
//! 4. **On any failure everything is put back**: each clone moved back, every kept file written
//!    back as it was, and a `repos/` this made removed. The workspace is then exactly as it was.
//!    The chats open in it are asked again once every clone has moved, and one opened during
//!    the move is a failure too (#5569).
//!
//! # A move cut short (#5569)
//!
//! Before anything moves, a journal names the workspace and its clones
//! (`.charter/move-into-repos.json`, [`JOURNAL`]); it is removed once the move is done or put
//! back. One still there was cut short by a crash, a kill or a power cut: some clones are in
//! `repos/` with their branch folders not repaired, which git cannot read. The doctor says so
//! ([`cut_short`]), and the next move finishes it first ([`apply`]): `git worktree repair` in
//! each clone the journal names that is in `repos/`, and in each a rollback moved back to the
//! root, whose branch folders may still point into `repos/`. Nothing is moved back: both layouts
//! are read, so finishing is the smaller step. A move never starts while a journal is there, so
//! a rollback that could not put everything back keeps its journal from the next workspace.
//!
//! # What the confirmation showed
//!
//! The window's confirmation names what moves. [`apply`] is handed what it showed ([`Shown`]),
//! and a workspace whose clones at its root are not those, or that it did not show, is refused
//! with nothing moved (#5569).
//!
//! # What it never touches
//!
//! Any harness's own stores (ADR 0050): Claude Code keeps a conversation under the folder it ran
//! in, so a chat started in the clone itself cannot be resumed by its harness after the move.
//! [`plan`] names those chats, from the workspace's session records, which still open. A chat
//! started in a branch folder is not named: branch folders do not move. A folder beside the repo
//! of its name in `repos/`, and an old clone named `repos`, are left, and the doctor's own rows
//! say what to do about each.

use std::path::{Path, PathBuf};

use super::Fixed;
use crate::harness::Harness;
use crate::repofolder::{self, Layout, Row};
use crate::worktree::git;

/// A chat the move leaves its harness unable to resume: by its session record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresumable {
    /// The record's title, as the Sessions panel shows it.
    pub title: String,
    /// The record, plane-relative, which still opens.
    pub record: String,
}

/// What the move would do in one workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub workspace: String,
    /// The clones at its root that would move, by name, sorted.
    pub repos: Vec<String>,
    /// The chats started in one of them whose harness could not resume them after it.
    pub unresumable: Vec<Unresumable>,
    /// The clones a move cut short left in `repos/` unrepaired, by name, which the move
    /// finishes first ([`cut_short`]).
    pub finishing: Vec<String>,
}

/// What the confirmation showed would move in one workspace: [`Planned::repos`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub workspace: String,
    pub repos: Vec<String>,
}

/// The journal of a move under way, in the plane's `.charter/`: see the module's docs.
pub const JOURNAL: &str = "move-into-repos.json";

/// What [`JOURNAL`] holds: the workspace being moved, and the clones it moves, by name.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Journal {
    workspace: String,
    repos: Vec<String>,
}

fn journal_path(root: &Path) -> PathBuf {
    crate::names::state(root).join(JOURNAL)
}

fn read_journal(root: &Path) -> Option<Journal> {
    let path = journal_path(root);
    crate::contain::no_link_on_the_way(root, &path).ok()?;
    let journal: Journal = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
    (crate::contain::workspace_name_ok(&journal.workspace)
        && journal.repos.iter().all(|name| repofolder::name_ok(name)))
    .then_some(journal)
}

fn write_journal(root: &Path, journal: &Journal) -> std::io::Result<()> {
    crate::plane::private_dir(root, &crate::names::state(root))?;
    let text = serde_json::to_string(journal).map_err(std::io::Error::other)? + "\n";
    crate::rewrite::replace(
        root,
        &journal_path(root),
        text.as_bytes(),
        crate::rewrite::Mode::Private,
    )
}

/// **A move cut short**: the workspace its journal names, and the clones it names that are in
/// `repos/`, whose branch folders may not have been repaired. A clone counts whatever now stands
/// at its old place: a rollback that could not move it back left it there for that reason.
/// `None` with no journal, or one that names a workspace that cannot be read.
pub fn cut_short(root: &Path) -> Option<(String, Vec<String>)> {
    let journal = read_journal(root)?;
    let moved = journal
        .repos
        .into_iter()
        .filter(|name| {
            repofolder::new_folder(root, &journal.workspace, name)
                .is_ok_and(|to| to.join(".git").is_dir())
        })
        .collect();
    Some((journal.workspace, moved))
}

/// **Finish a move cut short**: `git worktree repair` in each clone it left in `repos/`, and in
/// each it names that is back at the root, whose branch folders may still point into `repos/`
/// (a rollback cut short between moving it back and writing their pointers back), then its
/// journal removed. What it did, or why it could not, with the journal kept. `None` with no
/// journal.
fn finish(root: &Path) -> Option<Result<Vec<String>, String>> {
    let (ws, moved) = cut_short(root)?;
    let dir = match crate::worktree::confine::workspace_dir(root, &ws) {
        Ok(dir) => std::fs::canonicalize(&dir).unwrap_or(dir),
        Err(why) => return Some(Err(why.to_string())),
    };
    let back: Vec<String> = read_journal(root)
        .map(|journal| journal.repos)
        .unwrap_or_default()
        .into_iter()
        .filter(|name| !moved.contains(name) && dir.join(name).join(".git").is_dir())
        .collect();
    for name in &back {
        let was = dir.join(repofolder::REPOS).join(name);
        if let Err(why) = repair(&was, &dir.join(name)) {
            return Some(Err(format!(
                "workspace '{ws}': the move into repos/ put back could not be finished: \
                 {name}: {why}"
            )));
        }
    }
    let mut branch_folders = 0;
    for name in &moved {
        let to = match repofolder::new_folder(root, &ws, name) {
            Ok(to) => to,
            Err(why) => return Some(Err(why.to_string())),
        };
        match repair(&dir.join(name), &to) {
            Ok(n) => branch_folders += n,
            Err(why) => {
                return Some(Err(format!(
                    "workspace '{ws}': the move into repos/ cut short could not be finished: \
                     {name}: {why}"
                )));
            }
        }
    }
    let _ = std::fs::remove_file(journal_path(root));
    if moved.is_empty() {
        return Some(Ok(Vec::new()));
    }
    Some(Ok(vec![format!(
        "✓ {ws}: finished the move into repos/ cut short ({}); git worktree repair mended {}",
        moved.join(", "),
        crate::counts::counted(branch_folders, "branch folder", "branch folders")
    )]))
}

/// **The clones at workspace `ws`'s root the move takes**: the repos [`repofolder::holdings`]
/// lists at the root, but an old clone named `repos`, which is where they would go.
pub(crate) fn movable(root: &Path, ws: &str) -> Result<Vec<Row>, repofolder::Trouble> {
    Ok(repofolder::holdings(root, ws)?
        .repos
        .into_iter()
        .filter(|row| row.layout == Layout::Root && !named_repos(row))
        .collect())
}

/// Whether `row` is an old clone named `repos` (in any case), at the root.
pub(crate) fn named_repos(row: &Row) -> bool {
    row.layout == Layout::Root && row.repo.name.eq_ignore_ascii_case(repofolder::REPOS)
}

/// **What the move would do**, in each workspace that has clones at its root or a move cut
/// short to finish, sorted by name: what the confirmation shows. A workspace that cannot be read
/// is left out.
pub fn plan(root: &Path) -> Vec<Planned> {
    let Ok(workspaces) = crate::workspaces::Plane::open(root).workspaces() else {
        return Vec::new();
    };
    let short = cut_short(root);
    let mut planned = Vec::new();
    for ws in workspaces {
        let Ok(rows) = movable(root, &ws) else {
            continue;
        };
        let finishing = short
            .as_ref()
            .filter(|(of, _)| *of == ws)
            .map(|(_, moved)| moved.clone())
            .unwrap_or_default();
        if rows.is_empty() && finishing.is_empty() {
            continue;
        }
        planned.push(Planned {
            unresumable: unresumable(root, &ws, &rows),
            repos: rows.into_iter().map(|row| row.repo.name).collect(),
            finishing,
            workspace: ws,
        });
    }
    planned
}

/// The chats of workspace `ws`'s session records that ran in one of `rows`, or below it, with a
/// conversation their harness finds by that folder (or a harness the record does not name).
fn unresumable(root: &Path, ws: &str, rows: &[Row]) -> Vec<Unresumable> {
    let clones: Vec<String> = rows
        .iter()
        .filter_map(|row| relative(root, &row.repo.path))
        .collect();
    let place = crate::active::Place::Workspace(ws.to_string());
    crate::sessionrecord::list(root, &place)
        .into_iter()
        .filter(|listed| listed.conversation.is_some())
        .filter(|listed| {
            listed
                .harness
                .as_deref()
                .and_then(Harness::of_registry_or_kind)
                .is_none_or(Harness::keeps_conversations_by_directory)
        })
        .filter(|listed| {
            listed.cwd.as_deref().is_some_and(|cwd| {
                clones
                    .iter()
                    .any(|clone| cwd == clone || cwd.starts_with(&format!("{clone}/")))
            })
        })
        .map(|listed| Unresumable {
            title: listed.title,
            record: listed.shown,
        })
        .collect()
}

/// `path` below the plane, `/`-separated, as a session record's `cwd` spells it.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let below = path
        .strip_prefix(root)
        .ok()
        .map(Path::to_path_buf)
        .or_else(|| {
            let real = std::fs::canonicalize(root).ok()?;
            Some(path.strip_prefix(real).ok()?.to_path_buf())
        })?;
    let parts: Vec<&str> = below
        .components()
        .map(|part| part.as_os_str().to_str())
        .collect::<Option<_>>()?;
    Some(parts.join("/"))
}

/// **The chats the app's record holds in workspace `ws`**, by the name each is shown under: what
/// a terminal's move is refused over. Read whether or not an app listens on the plane: while one
/// does, the record is what it has open, and once it has quit, what it will reopen where the
/// record says, which a move would take away. A record that cannot be read names nothing. A chat
/// the record gives no folder could be in any workspace, so it is named in each (#5569).
pub fn open_in_record(root: &Path, ws: &str) -> Vec<String> {
    let Ok(record) = crate::reopen::read_or_refusal(root) else {
        return Vec::new();
    };
    let plane = crate::workspaces::Plane::open(root);
    record
        .chats
        .iter()
        .filter(|chat| {
            chat.cwd
                .as_deref()
                .is_none_or(|cwd| plane.workspace_of(cwd).is_some_and(|of| of == ws))
        })
        .map(|chat| crate::reopen::shown_name(chat, chat.harness().map(|h| h.name())))
        .collect()
}

/// **Apply the move to the project at `root`**: first a move cut short is finished
/// ([`cut_short`]), then each workspace with clones at its root is moved, with the chats `open`
/// says are open in it. Where `shown` is what a confirmation showed, a workspace is moved only
/// as it showed it. Refused when nothing was done and no workspace could be moved.
pub fn apply(
    root: &Path,
    open: &dyn Fn(&Path, &str) -> Vec<String>,
    shown: Option<&[Shown]>,
) -> Fixed {
    apply_with(root, open, shown, None)
}

/// **[`apply`], keeping the record of clones and offering the trust in the same step** (R-339,
/// Q184): with `records`, each clone purlis has a record of keeps it where it moved to (the
/// record is kept by the clone's path), and each it has none of ([`offered`]) is trusted where
/// it moved to on the person's yes ([`Records::trust`]), at the one address its remote named
/// before the move and still names. Without the yes nothing is trusted, and the move says so.
pub fn apply_with(
    root: &Path,
    open: &dyn Fn(&Path, &str) -> Vec<String>,
    shown: Option<&[Shown]>,
    records: Option<&Records<'_>>,
) -> Fixed {
    let mut said = Vec::new();
    match finish(root) {
        Some(Ok(lines)) => said.extend(lines),
        Some(Err(why)) => return Fixed::Refused(why),
        None => {}
    }
    let planned: Vec<Planned> = plan(root)
        .into_iter()
        .filter(|one| !one.repos.is_empty())
        .collect();
    if planned.is_empty() {
        if said.is_empty() {
            said.push("✓ every repo is in its workspace's repos/ — nothing to do.".to_owned());
        }
        return Fixed::Ran {
            said,
            complete: true,
        };
    }
    let finished = !said.is_empty();
    let mut refused = Vec::new();
    for one in &planned {
        let expected = match shown {
            None => None,
            Some(shown) => match shown.iter().find(|s| s.workspace == one.workspace) {
                Some(s) => Some(s.repos.as_slice()),
                None => {
                    let why = format!(
                        "workspace '{}' was not in the confirmation, and has repos to move now \
                         ({}); nothing in it was moved",
                        one.workspace,
                        one.repos.join(", ")
                    );
                    said.push(format!("✗ {why}"));
                    refused.push(why);
                    continue;
                }
            },
        };
        let chats = || open(root, &one.workspace);
        let before =
            records.map(|records| before_the_move(root, &one.workspace, records.config_root));
        match move_workspace(root, &one.workspace, &chats, expected, &mut |from, to| {
            std::fs::rename(from, to)
        }) {
            Ok(lines) => {
                said.extend(lines);
                if let (Some(records), Some(before)) = (records, before) {
                    said.extend(after_the_move(root, &one.workspace, before, records));
                }
                if let Some(line) = unresumable_said(&one.unresumable) {
                    said.push(line);
                }
            }
            Err(why) => {
                said.push(format!("✗ {why}"));
                refused.push(why);
            }
        }
    }
    if refused.len() == planned.len() && !finished {
        return Fixed::Refused(refused.join(" "));
    }
    Fixed::Ran {
        said,
        complete: refused.is_empty(),
    }
}

/// **The record of clones a move keeps, and the trust it offers in the same step** (R-339,
/// Q184): [`apply_with`]'s.
pub struct Records<'a> {
    /// The config home the record of clones is kept in.
    pub config_root: &'a Path,
    /// The person's yes to trusting the clones the move offers ([`offered`]), with the host's
    /// scope, which only the person's own process holds. `None` trusts nothing.
    pub trust: Option<&'a crate::forge::route::HostScope>,
}

/// **The clones the move offers to trust** (R-339): those at a workspace's root it would move
/// that purlis has no record of in the config home at `config_root`, each with the addresses its
/// remote names ([`crate::forge::gitcredential::vouch::unvouched_clones`]). The confirmation
/// shows them and asks. Nothing is trusted here.
pub fn offered(
    root: &Path,
    config_root: &Path,
) -> Vec<crate::forge::gitcredential::vouch::Unvouched> {
    let Ok(workspaces) = crate::workspaces::Plane::open(root).workspaces() else {
        return Vec::new();
    };
    let moving: Vec<String> = workspaces
        .iter()
        .filter_map(|ws| movable(root, ws).ok())
        .flatten()
        .map(|row| crate::sandbox::local::clone_key(&row.repo.path))
        .collect();
    crate::forge::gitcredential::vouch::unvouched_clones(root, config_root)
        .into_iter()
        .filter(|clone| moving.contains(&crate::sandbox::local::clone_key(&clone.path)))
        .collect()
}

/// What the record of clones says of one clone at a workspace's root, before it moves.
struct Before {
    name: String,
    /// The address purlis has it recorded at, where it has a record of it.
    recorded: Option<String>,
    /// Where purlis has none: the one address its remote names, which the move offers to trust
    /// at, or `None` where it names more than one.
    offered: Option<Option<String>>,
}

/// [`Before`] for each clone at workspace `ws`'s root, as the record in `config_root` stands.
fn before_the_move(root: &Path, ws: &str, config_root: &Path) -> Vec<Before> {
    let recorded = crate::forge::gitcredential::read_push_remotes(config_root);
    let unrecorded = offered(root, config_root);
    let Ok(rows) = movable(root, ws) else {
        return Vec::new();
    };
    rows.into_iter()
        .map(|row| {
            let key = crate::sandbox::local::clone_key(&row.repo.path);
            Before {
                recorded: recorded.get(&key).cloned(),
                offered: unrecorded
                    .iter()
                    .find(|clone| crate::sandbox::local::clone_key(&clone.path) == key)
                    .map(|clone| clone.place.clone()),
                name: row.repo.name,
            }
        })
        .collect()
}

/// After workspace `ws` moved: each clone purlis had a record of is recorded again where it now
/// is, and each it had none of is trusted there on the person's yes. What it did, line by line.
fn after_the_move(
    root: &Path,
    ws: &str,
    before: Vec<Before>,
    records: &Records<'_>,
) -> Vec<String> {
    let Ok(holdings) = repofolder::holdings(root, ws) else {
        return Vec::new();
    };
    let mut said = Vec::new();
    for one in before {
        let Some(row) = holdings
            .repos
            .iter()
            .find(|row| row.layout == Layout::Repos && row.repo.name == one.name)
        else {
            continue;
        };
        let folder = relative(root, &row.repo.path).unwrap_or_else(|| one.name.clone());
        if let Some(recorded) = one.recorded {
            // The record is kept by the clone's path, so the one it had stays behind unless it
            // is written again: purlis cloned it, or the person trusted it, already. Only while
            // its remote is still the one it was recorded at (Q257): a remote changed since is
            // not trusted again on the strength of the old record.
            if crate::forge::gitcredential::remote::remote_now(&row.repo.path).as_deref()
                != Some(recorded.as_str())
            {
                said.push(format!(
                    "• {folder} was trusted at {recorded} before it moved, and its remote has \
                     changed since, so purlis did not trust it again. Trust it with Trust these \
                     clones in the window's Asks, only if you made it yourself"
                ));
                continue;
            }
            if let Err(why) = crate::forge::gitcredential::remote::record(
                records.config_root,
                &row.repo.path,
                recorded.clone(),
            ) {
                said.push(format!(
                    "✗ {folder} was trusted at {recorded} before it moved, and {why}. Trust it \
                     again with Trust these clones in the window's Asks"
                ));
            }
            continue;
        }
        match (one.offered, records.trust) {
            (None, _) => {}
            (Some(None), _) => said.push(format!(
                "• {folder} was not trusted: its remote names more than one address. Give origin \
                 one address, then trust it with Trust these clones in the window's Asks"
            )),
            (Some(Some(place)), Some(host)) => match crate::windowonly::trust_this_clone(
                host,
                records.config_root,
                root,
                &folder,
                &place,
            ) {
                Ok(trusted) => said.push(format!("✓ trusted {folder} at {trusted}")),
                Err(why) => said.push(format!("✗ {folder} was not trusted: {why}")),
            },
            (Some(Some(place)), None) => said.push(format!(
                "• {folder} ({place}) was not trusted: purlis did not clone it. Trust it with \
                 Trust these clones in the window's Asks, only if you made it yourself"
            )),
        }
    }
    said
}

/// The line that names the chats a move leaves unresumable, or none when there are none.
pub fn unresumable_said(chats: &[Unresumable]) -> Option<String> {
    let (one, they) = match chats.len() {
        0 => return None,
        1 => ("This chat", "its session record still opens"),
        _ => ("These chats", "their session records still open"),
    };
    let named: Vec<&str> = chats.iter().map(|chat| chat.title.as_str()).collect();
    Some(format!(
        "• {one} started in a clone that moved, and their harness can no longer resume them: \
         {}; {they}.",
        named.join(", ")
    ))
}

/// **Move workspace `ws`'s clones at its root into its `repos/`**, all or nothing, moving each
/// with `mv` (`std::fs::rename`; a test hands one that fails). `open` asks for the chats open in
/// it, by the name each is shown under: any at all before the move and nothing moves, any after
/// it and everything is put back. `expected` is the clones the confirmation showed, by name:
/// where it is given and the clones at the root are others, nothing moves. What it did, line by
/// line, or why nothing changed.
pub fn move_workspace(
    root: &Path,
    ws: &str,
    open: &dyn Fn() -> Vec<String>,
    expected: Option<&[String]>,
    mv: &mut dyn FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<Vec<String>, String> {
    let chats = open();
    if !chats.is_empty() {
        let (a_chat, it) = if chats.len() == 1 {
            ("a chat", "it")
        } else {
            ("chats", "them")
        };
        return Err(format!(
            "workspace '{ws}' has {a_chat} open in it ({}): close {it} first; nothing was moved",
            chats.join(", ")
        ));
    }
    let rows = movable(root, ws).map_err(|why| why.to_string())?;
    if let Some(expected) = expected {
        let now: Vec<&str> = rows.iter().map(|row| row.repo.name.as_str()).collect();
        let mut was: Vec<&str> = expected.iter().map(String::as_str).collect();
        was.sort_unstable();
        if now != was {
            return Err(format!(
                "workspace '{ws}' changed since the confirmation was shown: it showed {}, and \
                 the repos to move now are {}. Nothing was moved; press Fix again to see what \
                 would move",
                listed(&was),
                listed(&now)
            ));
        }
    }
    if rows.is_empty() {
        return Ok(vec![format!(
            "✓ {ws}: every repo is in repos/ — nothing to do."
        )]);
    }
    let mut moves: Vec<(Row, PathBuf)> = Vec::new();
    for row in rows {
        let to = repofolder::new_folder(root, ws, &row.repo.name)
            .map_err(|why| format!("{why}; nothing was moved"))?;
        if std::fs::symlink_metadata(&to).is_ok() {
            return Err(format!(
                "workspace '{ws}' already has repos/{} — move it aside to move {} there; \
                 nothing was moved",
                row.repo.name, row.repo.name
            ));
        }
        moves.push((row, to));
    }
    let held: Vec<String> = moves
        .iter()
        .flat_map(|(row, _)| linked(&row.repo.path))
        .filter_map(|tree| {
            let subs = submodules(&tree);
            (!subs.is_empty()).then(|| format!("{} ({})", tree.display(), subs.join(", ")))
        })
        .collect();
    if !held.is_empty() {
        return Err(format!(
            "workspace '{ws}' has submodules in its branch folders, whose links git worktree \
             repair does not mend after a move: {}. Run git submodule deinit --all in each, or \
             move the workspace by hand; nothing was moved",
            held.join("; ")
        ));
    }
    let kept: Vec<(PathBuf, Vec<u8>)> = moves
        .iter()
        .flat_map(|(row, _)| repairable(&row.repo.path))
        .collect();
    if std::fs::symlink_metadata(journal_path(root)).is_ok() {
        return Err(format!(
            "workspace '{ws}': a move into repos/ cut short is not finished yet; run the move \
             again to finish it first; nothing was moved"
        ));
    }
    let journal = Journal {
        workspace: ws.to_owned(),
        repos: moves.iter().map(|(row, _)| row.repo.name.clone()).collect(),
    };
    write_journal(root, &journal).map_err(|why| {
        format!(
            "workspace '{ws}': the move's journal could not be written ({why}); nothing was moved"
        )
    })?;
    let mut undo = Undo {
        moved: Vec::new(),
        made: None,
        kept,
        journal: journal_path(root),
    };
    let folder = match crate::worktree::confine::workspace_dir(root, ws) {
        Ok(dir) => dir.join(repofolder::REPOS),
        Err(why) => return Err(undo.run(ws, &why.to_string())),
    };
    if std::fs::symlink_metadata(&folder).is_err() {
        // `mkdir` alone, never through a link: `new_folder` answered with nothing there.
        if let Err(why) = std::fs::create_dir(&folder) {
            return Err(undo.run(ws, &format!("repos/ could not be made ({why})")));
        }
        undo.made = Some(folder);
    }
    let mut branch_folders = 0;
    for (row, to) in &moves {
        let name = &row.repo.name;
        let from = std::fs::canonicalize(&row.repo.path).unwrap_or_else(|_| row.repo.path.clone());
        if let Err(why) = mv(&row.repo.path, to) {
            return Err(undo.run(
                ws,
                &format!("{name} could not be moved into repos/ ({why})"),
            ));
        }
        undo.moved.push((row.repo.path.clone(), to.clone()));
        match repair(&from, to) {
            Ok(n) => branch_folders += n,
            Err(why) => return Err(undo.run(ws, &format!("{name}: {why}"))),
        }
    }
    // A chat that opened while the clones moved may hold one of their old paths (#5569).
    let late = open();
    if !late.is_empty() {
        return Err(undo.run(
            ws,
            &format!("a chat opened in it during the move ({})", late.join(", ")),
        ));
    }
    // Done: a journal left behind would only be finished again, which repairs nothing twice.
    let _ = std::fs::remove_file(&undo.journal);
    let names: Vec<&str> = moves
        .iter()
        .map(|(row, _)| row.repo.name.as_str())
        .collect();
    let mut said = vec![format!("✓ {ws}: moved {} into repos/", names.join(", "))];
    if branch_folders > 0 {
        said.push(format!(
            "✓ {ws}: git worktree repair mended {}",
            crate::counts::counted(branch_folders, "branch folder", "branch folders")
        ));
    }
    Ok(said)
}

/// What [`move_workspace`] puts back on a failure.
struct Undo {
    /// `(from, to)`, in the order moved.
    moved: Vec<(PathBuf, PathBuf)>,
    /// The `repos/` it made, if it made one.
    made: Option<PathBuf>,
    /// Every file `git worktree repair` may rewrite, by where it is before the move, as it was.
    kept: Vec<(PathBuf, Vec<u8>)>,
    /// The move's journal, removed once everything is put back, and kept where something could
    /// not be, so the next move finishes what is left in `repos/`.
    journal: PathBuf,
}

impl Undo {
    /// Put everything back, and say why it failed and what could not be put back.
    fn run(&self, ws: &str, why: &str) -> String {
        let mut left = Vec::new();
        for (from, to) in self.moved.iter().rev() {
            if let Err(e) = std::fs::rename(to, from) {
                left.push(format!(
                    "{} could not be moved back to {} ({e})",
                    to.display(),
                    from.display()
                ));
            }
        }
        for (path, bytes) in &self.kept {
            if std::fs::read(path).ok().as_deref() != Some(bytes.as_slice())
                && let Err(e) = std::fs::write(path, bytes)
            {
                left.push(format!(
                    "{} could not be written back ({e})",
                    path.display()
                ));
            }
        }
        if let Some(made) = &self.made
            && let Err(e) = std::fs::remove_dir(made)
        {
            left.push(format!("{} could not be removed ({e})", made.display()));
        }
        if left.is_empty() {
            let _ = std::fs::remove_file(&self.journal);
            format!("workspace '{ws}': {why}; everything was put back as it was")
        } else {
            format!(
                "workspace '{ws}': {why}; not everything could be put back: {}",
                left.join("; ")
            )
        }
    }
}

/// Names as a sentence lists them, or `none`.
fn listed(names: &[&str]) -> String {
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}

/// The files `git worktree repair` may rewrite for the clone at `clone`, with their bytes: each
/// linked worktree's `.git` file, and each `.git/worktrees/<id>/gitdir` in the clone.
fn repairable(clone: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files: Vec<PathBuf> = linked(clone)
        .into_iter()
        .map(|tree| tree.join(".git"))
        .collect();
    if let Ok(admin) = std::fs::read_dir(clone.join(".git/worktrees")) {
        files.extend(admin.flatten().map(|entry| entry.path().join("gitdir")));
    }
    files
        .into_iter()
        .filter(|file| std::fs::symlink_metadata(file).is_ok_and(|m| m.is_file()))
        .filter_map(|file| Some((file.clone(), std::fs::read(&file).ok()?)))
        .collect()
}

/// The linked worktrees of the clone at `clone`, as git lists them.
fn linked(clone: &Path) -> Vec<PathBuf> {
    match git::run(clone, &["worktree", "list", "--porcelain"], git::READ) {
        Ok(run) if run.ok() => crate::worktree::porcelain::parse(&run.out)
            .into_iter()
            .skip(1)
            .map(|row| row.path)
            .collect(),
        _ => Vec::new(),
    }
}

/// `git worktree repair` in the clone moved from `from` (as the filesystem resolves it) to `to`,
/// handing it the new path of each linked worktree that was inside the clone, then each linked
/// worktree asked whether git reads it, and every submodule checked out in the clone or in one
/// of them asked for its status. How many linked worktrees there are, or what failed.
fn repair(from: &Path, to: &Path) -> Result<usize, String> {
    let real = std::fs::canonicalize(to).map_err(|why| format!("its new folder ({why})"))?;
    let trees = linked(&real);
    let moved: Vec<String> = trees
        .iter()
        .filter_map(|tree| tree.strip_prefix(from).ok())
        .map(|below| real.join(below).display().to_string())
        .collect();
    let mut argv = vec!["worktree", "repair"];
    argv.extend(moved.iter().map(String::as_str));
    match git::run(&real, &argv, git::READ) {
        Ok(run) if run.ok() => {}
        Ok(run) => return Err(format!("git worktree repair failed ({})", run.err.trim())),
        Err(why) => return Err(format!("git worktree repair could not run ({why})")),
    }
    // A worktree git calls prunable (its folder long gone) was not working before the move, and
    // is no part of what the move must keep working.
    let trees: Vec<PathBuf> = linked(&real).into_iter().filter(|t| t.is_dir()).collect();
    for tree in &trees {
        let read = git::run(tree, &["rev-parse", "--git-dir"], git::READ);
        if !read.as_ref().is_ok_and(git::Run::ok) {
            return Err(format!(
                "git does not read its branch folder {} after the repair",
                tree.display()
            ));
        }
    }
    for tree in std::iter::once(&real).chain(&trees) {
        unreadable_submodule(tree)?;
    }
    Ok(trees.len())
}

/// The submodules checked out in the working tree at `tree`, by path: each gitlink in its index
/// whose folder holds a `.git`.
fn submodules(tree: &Path) -> Vec<String> {
    let Ok(run) = git::run(tree, &["ls-files", "--stage", "-z"], git::READ) else {
        return Vec::new();
    };
    if !run.ok() {
        return Vec::new();
    }
    run.out
        .split('\0')
        .filter_map(|entry| entry.strip_prefix("160000 "))
        .filter_map(|rest| rest.split_once('\t').map(|(_, path)| path.to_owned()))
        .filter(|path| std::fs::symlink_metadata(tree.join(path).join(".git")).is_ok())
        .collect()
}

/// `git status` in every submodule checked out under `tree`, and theirs in turn, without writing
/// an index: the first git cannot read, by its path.
fn unreadable_submodule(tree: &Path) -> Result<(), String> {
    for sub in submodules(tree) {
        let at = tree.join(&sub);
        match git::run(
            &at,
            &["--no-optional-locks", "status", "--porcelain"],
            git::READ,
        ) {
            Ok(run) if run.ok() => unreadable_submodule(&at)?,
            Ok(run) => {
                return Err(format!(
                    "git does not read its submodule {} after the move ({})",
                    at.display(),
                    run.err.trim()
                ));
            }
            Err(why) => {
                return Err(format!(
                    "git status could not run in its submodule {} ({why})",
                    at.display()
                ));
            }
        }
    }
    Ok(())
}
