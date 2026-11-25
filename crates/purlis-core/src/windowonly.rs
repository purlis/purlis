// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A refusal whose only way out is a press in the window raises an ask** (R-328, Q181): no
//! window-only dead end.
//!
//! Where purlis refuses a chat something only the person's press in the window can change, the
//! refusal is raised here as an ask carrying that [`Control`], for the app to list in Asks; the
//! chat is told the ask was raised and what it is called, never "from the window" alone. Each
//! chat raises one ask per control, however often it is refused. The person's press clears it
//! ([`trust_this_clone`]), for every chat that waits on it, and so does their press on the
//! control's own Notice, which is the same act. An ask is raised whatever became of that Notice:
//! one the person dismissed comes back here as an ask once its condition stops a chat (R-335).
//!
//! The first refusal is a GitHub App token that can't be minted for a clone purlis has no record
//! of ([`crate::forge::apptoken::token_for`], R-330), whose control is **Trust this clone**.
//!
//! **Clones purlis has no record of are adopted only through one ask** (R-336, Q184): the project
//! has one **Trust these clones** for all of them, raised as the window reads its clones
//! ([`look_at_clones`]) and no chat's, never one per clone and never trusted automatically. Each
//! clone the person trusts ([`trust_this_clone`]) leaves it, and it goes with the last.
//!
//! The refusals and the clones' ask are kept in memory alone, by project, as the tokens they
//! stand in for are ([`crate::forge::apptoken`]): they are this run's, a chat's go when it ends
//! ([`forget`]), and a project's when it ends ([`project_ended`]).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::forge::route::HostScope;
use crate::pieces::Who;

#[cfg(test)]
mod tests;

/// **The press in the window that clears a refusal**, with what it acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    /// **Trust this clone** on a clone purlis has no record of, at the one address its remote
    /// named when the refusal was raised: what [`crate::forge::gitcredential::vouch::vouch_clone`]
    /// records.
    TrustClone {
        /// Its folder, relative to the project.
        folder: String,
        /// `<host>/<owner>/<repo>`, the address it is trusted at.
        place: String,
    },
    /// **Use one address** on a clone whose origin fetches from one address and pushes to
    /// another (A1), or names several (A5): the window sets origin's push URL to `to`, the
    /// address purlis recorded for the clone or, where it recorded none, its fetch address. The
    /// row shows `to` before the press (Q4), and the press sets exactly it, and only while
    /// origin is still as the row showed it. Trusting the clone stays a second press.
    OneAddress {
        /// Its folder, relative to the project.
        folder: String,
        /// The address origin fetches from when the row was raised, `<host>/<owner>/<repo>`.
        fetch: String,
        /// The address origin pushes to then.
        push: String,
        /// The address the press sets.
        to: String,
    },
    /// **Clone it** (A2): the window clones `repo` into `workspace` with the person's own
    /// sign-in, as its own clone, for a chat whose clone of a private repo failed for want of a
    /// forge CLI.
    CloneWithSignIn {
        workspace: String,
        /// The inventory's name for the repo, the one `purlis clone` takes: the press clones
        /// that row, and only while its address is still `repo` ([`clone_it_matches`]).
        name: String,
        /// `<host>/<owner>/<repo>`, the address the inventory's row for `name` has.
        repo: String,
    },
    /// **Record this repo** (A3): the window records the repo of the clone `clone` of
    /// `workspace` in the project's inventory, as `purlis discover` does for that one clone, for
    /// a chat whose read of the forge was refused because the project records no repo for it.
    /// The row shows `place` before the press, and the press records exactly it, and only while
    /// origin still names it.
    RecordRepo {
        workspace: String,
        /// The clone's folder name in the workspace.
        clone: String,
        /// The address origin names when the row was raised, `<host>/<owner>/<repo>`.
        place: String,
    },
    /// **Remove** a persona or a branch folder a chat asked to have removed (T1.1-67, Q304 Q2),
    /// with exactly what the removal would lose, as the app read it when the row was raised
    /// ([`crate::removalask`]). The press removes it only while it still holds exactly that
    /// ([`remove_listed`]); a listing that was not exact offers Keep only.
    Remove {
        /// A persona or a branch folder; a workspace's removal has its own ask.
        removing: crate::hookwire::Removing,
        /// The workspace the folder is in; empty for a persona.
        workspace: String,
        /// What goes, in words, one line each.
        listed: Vec<String>,
        /// Whether `listed` is everything that goes.
        exact: bool,
        /// A digest of everything that goes, whole: [`crate::removalask::Listing::digest`].
        digest: String,
    },
}

impl Control {
    /// The control's label, as the window draws it.
    pub fn label(&self) -> &'static str {
        match self {
            Self::TrustClone { .. } => TRUST_THIS_CLONE,
            Self::OneAddress { .. } => USE_ONE_ADDRESS,
            Self::CloneWithSignIn { .. } => CLONE_IT,
            Self::RecordRepo { .. } => RECORD_THIS_REPO,
            Self::Remove { removing, .. } => removal_label(removing),
        }
    }

    /// Whether the row offers a press. Every control does but a removal whose listing was not
    /// exact: nothing is removed on a list that may leave something out (Q304 Q2).
    pub fn pressable(&self) -> bool {
        !matches!(self, Self::Remove { exact: false, .. })
    }

    /// What identifies the ask a chat's newer one of the same kind replaces: the folder of a
    /// clone's asks (its remote may have changed since), the workspace and repo of a clone-it.
    fn slot(&self) -> (&'static str, String) {
        match self {
            Self::TrustClone { folder, .. } | Self::OneAddress { folder, .. } => {
                (self.label(), folder.clone())
            }
            Self::CloneWithSignIn { repo, .. } => (self.label(), repo.clone()),
            Self::RecordRepo { clone, .. } => (self.label(), clone.clone()),
            Self::Remove { removing, .. } => (self.label(), removal_name(removing)),
        }
    }

    /// What the ask says, in purlis's words.
    pub fn says(&self) -> String {
        match self {
            Self::TrustClone { folder, place } => format!(
                "{TRUST_SO_CHATS_CAN_USE_GITHUB}: {folder} was not cloned by purlis, and its \
                 remote is {place}. Trust it only if you made this clone yourself."
            ),
            Self::OneAddress {
                folder,
                fetch,
                push,
                to,
            } => format!(
                "Give origin one address: the remote of {folder} fetches from {fetch} and pushes \
                 to {push}, so purlis will not sign in for it. Pressing sets its push address \
                 to {to}, and nothing else. Trusting the clone is a separate press."
            ),
            Self::CloneWithSignIn {
                workspace, repo, ..
            } => {
                let mut parts = repo.splitn(3, '/');
                let (host, owner, name) = (
                    parts.next().unwrap_or_default(),
                    parts.next().unwrap_or_default(),
                    parts.next().unwrap_or_default(),
                );
                format!(
                    "Clone {name} for a chat: it is {owner}'s repository on {host}, and no forge \
                     command line is signed in here. Pressing clones it into workspaces/{workspace} \
                     with your sign-in, which purlis never gives a chat."
                )
            }
            Self::RecordRepo { clone, place, .. } => format!(
                "Record {clone} in this project's repos: its origin is {place}, which the project \
                 does not record, so purlis does not read its milestones for a chat. A chat can \
                 set a clone's origin, so record it only if this is the repo you mean. Pressing \
                 records that one repo, as `purlis discover` does for it, and nothing else."
            ),
            Self::Remove {
                removing,
                listed,
                exact,
                ..
            } => removal_says(removing, listed, *exact),
        }
    }
}

/// The label of the control that removes a persona a chat asked to have removed.
pub const REMOVE_PERSONA: &str = "Remove persona";

/// The label of the control that removes a branch folder with the work it holds.
pub const REMOVE_AND_DISCARD: &str = "Remove and discard";

/// The label of the control that removes a branch folder with the files git ignores in it.
pub const REMOVE_WITH_THESE_FILES: &str = "Remove with these files";

/// The label of the other answer to every removal: nothing is removed.
pub const KEEP: &str = "Keep";

/// The press's label for a removal of `removing`.
fn removal_label(removing: &crate::hookwire::Removing) -> &'static str {
    use crate::hookwire::Removing;
    match removing {
        Removing::BranchFolder { force: true, .. } => REMOVE_AND_DISCARD,
        Removing::BranchFolder { .. } => REMOVE_WITH_THESE_FILES,
        Removing::Persona { .. } | Removing::Workspace { .. } => REMOVE_PERSONA,
    }
}

/// What names the thing a removal is of, to tell two asks apart.
fn removal_name(removing: &crate::hookwire::Removing) -> String {
    use crate::hookwire::Removing;
    match removing {
        Removing::Workspace { name } | Removing::Persona { name } => name.clone(),
        Removing::BranchFolder { repo, piece, .. } => format!("{repo}/{piece}"),
    }
}

/// What a removal's row says, in purlis's words: the thing, exactly what goes, and what a press
/// and Keep do. The names are the project's own, drawn as data.
fn removal_says(removing: &crate::hookwire::Removing, listed: &[String], exact: bool) -> String {
    use crate::hookwire::Removing;
    let thing = match removing {
        Removing::Workspace { name } => format!("workspace {name}"),
        Removing::Persona { name } => format!("persona {name}"),
        Removing::BranchFolder { repo, piece, .. } => {
            format!("branch folder {repo} \u{b7} {piece}")
        }
    };
    let goes = if listed.is_empty() {
        "nothing that purlis could list".to_owned()
    } else {
        listed.join("; ")
    };
    if exact {
        format!(
            "Remove {thing}? Removing it deletes exactly this: {goes}. Pressing removes it only \
             while it still holds exactly that, and Keep removes nothing."
        )
    } else {
        format!(
            "Remove {thing}? Removing it deletes this: {goes}. purlis could not list everything \
             it would delete, so it offers Keep only: nothing is removed on a list that may leave \
             something out."
        )
    }
}

/// The label of the control that records a clone's repo in the project's inventory.
pub const RECORD_THIS_REPO: &str = "Record this repo";

/// The label of the control that gives a clone's origin one address.
pub const USE_ONE_ADDRESS: &str = "Use one address";

/// The label of the control that clones a repo for a chat with the person's sign-in.
pub const CLONE_IT: &str = "Clone it";

/// The label of the control that trusts a clone made outside purlis.
pub const TRUST_THIS_CLONE: &str = "Trust this clone";

/// What a refused mint's ask is headed (R-330).
pub const TRUST_SO_CHATS_CAN_USE_GITHUB: &str = "Trust this clone so chats can use GitHub";

/// **One refusal raised as an ask**: chat `session` of the project at `root`, waiting on
/// `control`, since `since` (seconds since the epoch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub root: PathBuf,
    pub session: u32,
    pub control: Control,
    pub since: u64,
}

/// Told that a project's asks moved, so the window reads them again.
type Told = Arc<dyn Fn() + Send + Sync>;

/// Every refusal raised, over every project.
static RAISED: Mutex<Vec<Refusal>> = Mutex::new(Vec::new());

/// Who is told, by project, that its refusals moved ([`telling`]).
static TOLD: Mutex<Vec<(PathBuf, Told)>> = Mutex::new(Vec::new());

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Tells whoever listens for project `root`'s asks.
fn tell(root: &Path) {
    let told: Vec<Told> = TOLD
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter(|(at, _)| at == root)
        .map(|(_, told)| Arc::clone(told))
        .collect();
    for told in told {
        told();
    }
}

/// **Whether a window-only refusal in the project at `root` is raised as an ask**, as
/// `[policy] consents` runs (Q308b): enforced, it is, as it always has been; under `log` it is
/// not, and one line says it would have been; under `off` it is not. The control stays the
/// window's in every mode: a press here releases a credential or makes a change only the
/// person makes, and which of them a chat may make needs a ruling of its own.
pub fn asks(root: &Path) -> bool {
    crate::enforcement::judged(
        root,
        None,
        crate::enforcement::Family::Consents,
        crate::enforcement::mode(root, crate::enforcement::Family::Consents),
        crate::enforcement::RULE_WINDOW_ONLY_ASK,
        None,
        (),
    )
    .is_some()
}

/// **What [`raise`] answers** (Q6, rule H): whether the ask is new, and whether a window listens
/// for the project, so that the person will see it. A refusal says "asked the person in Asks"
/// only where `listened`, and says nobody was asked otherwise ([`nobody_asked`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Raised {
    /// The chat had no such ask yet; the window was told.
    pub new: bool,
    /// A window listens for the project's asks ([`listens`]).
    pub listened: bool,
}

/// **Whether a window listens for the asks of the project at `root`** ([`telling`]): the app
/// that holds the project. Where none does, an ask raised is kept but nobody sees it.
pub fn listens(root: &Path) -> bool {
    TOLD.lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|(at, _)| at == root)
}

/// **What a chat is told where no window listens for its project** (Q6, rule H), in place of
/// "asked the person in Asks": plainly that nobody was asked, with the way out in words,
/// `then` being what the person does once purlis is open on the project ("press Use one address
/// on the clone").
pub fn nobody_asked(root: &Path, then: &str) -> String {
    let project = root.file_name().map_or_else(
        || "this project".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    format!(
        "Nobody was asked: no purlis window is open on {project}, so open purlis on {project}, \
         then {then}"
    )
}

/// Where a block is written for a chat of a project ([`blocking`]).
type Blocks = Arc<dyn Fn(u32, crate::eventlog::stops::Stop) + Send + Sync>;

/// Who writes a block for a chat, by project: the app's Chat log writer.
static BLOCKS: Mutex<Vec<(PathBuf, Blocks)>> = Mutex::new(Vec::new());

/// From now on, `write` writes the blocks of project `root`'s chats ([`nobody_asked_block`]): one
/// writer per project, the last one set.
pub fn blocking(root: &Path, write: Blocks) {
    let mut held = BLOCKS.lock().unwrap_or_else(|e| e.into_inner());
    held.retain(|(at, _)| at != root);
    held.push((root.to_path_buf(), write));
}

/// The guard arm a "nobody was asked" block names.
pub const NOBODY_ASKED_ARM: &str = "no-window";

/// **Writes the block of a refusal nobody was asked about** (Q6): chat `session` of the project
/// at `root` was refused and no window listens, so the stop is written to its Chat log, source
/// Guard. Answers whether a writer took it.
pub fn nobody_asked_block(root: &Path, session: u32) -> bool {
    let write = BLOCKS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(at, _)| at == root)
        .map(|(_, write)| Arc::clone(write));
    let Some(write) = write else {
        return false;
    };
    use crate::eventlog::stops::{Block, Source, Stop, Target};
    write(
        session,
        Stop::Block(Block {
            tool: "purlis".to_owned(),
            source: Source::Guard,
            qualifier: None,
            target: Target::guard_arm(NOBODY_ASKED_ARM).ok(),
            args: None,
        }),
    );
    true
}

/// **What a chat is told of a refusal it raised `control` for** (rule H): the `asked` words where
/// a window listens, with `then` its way out in words; otherwise that nobody was asked, with the
/// same way out, and a block written. One place for every refusal that raises an ask.
pub fn told_of(root: &Path, session: u32, raised: Raised, then: &str) -> String {
    if raised.listened {
        return format!("purlis asked the person in Asks. {then}");
    }
    nobody_asked_block(root, session);
    nobody_asked(root, then)
}

/// **Raises chat `session`'s refusal in the project at `root`, whose way out is `control`**:
/// one ask per chat and control. A chat's earlier ask of the same kind for the same clone at
/// another place goes: its remote changed, so that ask's press would act on nothing, and the new
/// ask replaces it rather than stacking. Answers whether it is new and whether a window listens
/// ([`Raised`]).
pub fn raise(root: &Path, session: u32, control: Control) -> Raised {
    let new = {
        let mut raised = RAISED.lock().unwrap_or_else(|e| e.into_inner());
        raised.retain(|one| {
            !(one.root == root
                && one.session == session
                && one.control.slot() == control.slot()
                && one.control != control)
        });
        let already = raised
            .iter()
            .any(|one| one.root == root && one.session == session && one.control == control);
        if !already {
            raised.push(Refusal {
                root: root.to_path_buf(),
                session,
                control,
                since: now(),
            });
        }
        !already
    };
    if new {
        tell(root);
    }
    Raised {
        new,
        listened: listens(root),
    }
}

/// **Every refusal raised in the project at `root`** and not yet cleared, oldest first.
pub fn every(root: &Path) -> Vec<Refusal> {
    RAISED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter(|one| one.root == root)
        .cloned()
        .collect()
}

/// **The person pressed `control` in the project at `root`, and what it asks was done** (Clone
/// it's press, made by the window): every chat's refusal it clears goes.
pub fn pressed_by_the_window(root: &Path, control: &Control) {
    pressed(root, control);
}

/// **The person pressed `control` in the project at `root`**: every chat's refusal it clears
/// goes.
fn pressed(root: &Path, control: &Control) {
    let gone = {
        let mut raised = RAISED.lock().unwrap_or_else(|e| e.into_inner());
        let before = raised.len();
        raised.retain(|one| !(one.root == root && one.control == *control));
        raised.len() != before
    };
    if gone {
        tell(root);
    }
}

/// **Chat `session` of the project at `root` ended**: its refusals go with it.
pub fn forget(root: &Path, session: u32) {
    let gone = {
        let mut raised = RAISED.lock().unwrap_or_else(|e| e.into_inner());
        let before = raised.len();
        raised.retain(|one| !(one.root == root && one.session == session));
        raised.len() != before
    };
    if gone {
        tell(root);
    }
}

/// **The project at `root` ended**: every refusal raised in it goes, its Trust these clones, and
/// its listener, as [`crate::forge::apptoken::project_ended`] forgets its tokens.
pub fn project_ended(root: &Path) {
    RAISED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|one| one.root != root);
    UNRECORDED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|one| one.root != root);
    TOLD.lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|(at, _)| at != root);
    BLOCKS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|(at, _)| at != root);
}

/// From now on, `told` is told each time project `root`'s refusals move: one listener per
/// project, the last one set.
pub fn telling(root: &Path, told: Told) {
    let mut listening = TOLD.lock().unwrap_or_else(|e| e.into_inner());
    listening.retain(|(at, _)| at != root);
    listening.push((root.to_path_buf(), told));
}

/// **The person's Trust this clone**, from its ask or its Notice: records the clone at `folder`
/// (relative to the project at `root`) at `place`, as
/// [`crate::forge::gitcredential::vouch::vouch_clone`] does, and clears every chat's refusal
/// that waited on it, and the clone leaves the project's Trust these clones ([`clones_ask`]).
/// Answers the address trusted, or why nothing was (and nothing is cleared).
pub fn trust_this_clone(
    host: &HostScope,
    config_root: &Path,
    root: &Path,
    folder: &str,
    place: &str,
) -> Result<String, String> {
    let trusted = crate::forge::gitcredential::vouch::vouch_clone(
        host,
        config_root,
        root,
        &root.join(folder),
        place,
    )?;
    pressed(
        root,
        &Control::TrustClone {
            folder: folder.to_owned(),
            place: place.to_owned(),
        },
    );
    clone_trusted(root, folder);
    Ok(trusted)
}

/// **The Use one address the clone at `folder` (relative to the project at `root`) needs**, as
/// its row would show it (Q4): `None` where its origin names one address, or git cannot say.
/// `to` is the address purlis recorded for the clone in the config home at `config_root`, or its
/// fetch address where it recorded none. Read from the clone now, never from a chat's words.
pub fn one_address_for(config_root: &Path, root: &Path, folder: &str) -> Option<Control> {
    let clone = root.join(folder);
    let places = crate::forge::gitcredential::remote::places_now(&clone)?;
    let (fetch, push) = (places.first()?.clone(), places.last()?.clone());
    if places.iter().all(|place| *place == fetch) {
        return None;
    }
    let to = crate::forge::gitcredential::vouch::recorded_remote(config_root, &clone)
        .unwrap_or_else(|| fetch.clone());
    Some(Control::OneAddress {
        folder: folder.to_owned(),
        fetch,
        push,
        to,
    })
}

/// **The person's Use one address**: sets the push URL of the origin of the clone at
/// `shown.folder` to exactly `shown.to`, the address its row showed, and clears every chat's ask
/// for it. Answers the address set. Refused, with a sentence, nothing changed and the ask kept,
/// where origin is no longer as the row showed it (`fetch` and `push`), or `to` is neither the
/// address recorded for the clone nor its fetch address.
///
/// **Setting an address is never trusting** (Q4, Q181, Q257): the clone is not recorded here. Where
/// it has no record, each chat that waited on this press is given Trust this clone for the one
/// address it now has, a second press.
pub fn use_one_address(
    _host: &HostScope,
    config_root: &Path,
    root: &Path,
    shown: &Control,
) -> Result<String, String> {
    let Control::OneAddress {
        folder,
        fetch,
        push,
        to,
    } = shown
    else {
        return Err(format!("{} is not a Use one address", shown.label()));
    };
    let clone = root.join(folder);
    let changed = || {
        format!(
            "The remote of {folder} changed since the window showed it, so purlis changed \
             nothing. Look at it again and choose"
        )
    };
    let places = crate::forge::gitcredential::remote::places_now(&clone).ok_or_else(changed)?;
    let as_shown = places.first() == Some(fetch)
        && places.last() == Some(push)
        && places.iter().all(|place| place == fetch || place == push);
    let recorded = crate::forge::gitcredential::vouch::recorded_remote(config_root, &clone);
    if !as_shown || !(to == fetch || recorded.as_deref() == Some(to)) {
        return Err(changed());
    }
    let url = format!("https://{to}");
    let set = crate::worktree::git::run(
        &clone,
        &["remote", "set-url", "--push", "origin", &url],
        crate::worktree::git::READ,
    )
    .map_err(|why| format!("purlis could not start git: {why}"))?;
    if !set.ok() {
        return Err(format!(
            "git would not set the push address of {folder}: {}",
            set.err.trim()
        ));
    }
    let waiting: Vec<u32> = every(root)
        .into_iter()
        .filter(|one| one.control == *shown)
        .map(|one| one.session)
        .collect();
    pressed(root, shown);
    if !crate::forge::gitcredential::vouch::trusted(config_root, &clone) {
        for session in waiting {
            raise(
                root,
                session,
                Control::TrustClone {
                    folder: folder.clone(),
                    place: to.clone(),
                },
            );
        }
    }
    Ok(to.clone())
}

/// **The clones of a chat's workspace its refusal may name** (A3), as the GitHub App mint filters
/// them ([`crate::forge::apptoken`], Q249, Q257): `started`, the clones its workspace had when
/// the chat started, as they were then (`None`: every clone), and `writes`, the folders the chat
/// may be granted, none of which may hold a clone's git directory. A clone a chat made or can
/// write names no repo of the person's: its origin is the chat's to set.
#[derive(Debug, Clone, Copy, Default)]
pub struct Reach<'a> {
    pub started: Option<&'a [crate::forge::apptoken::Started]>,
    pub writes: &'a [PathBuf],
}

/// **`<host>/<path>` as a row shows it**, or why it cannot be shown: an origin is set by whoever
/// can write the clone, so a place with a space or a control character in it, or one longer than
/// [`crate::forge::PLACE_AT_MOST`], is never put before the person ([`crate::forge::plain_place`]).
fn shown_place(live: &crate::forge::pr::Repo) -> Result<String, String> {
    let place = format!("{}/{}", live.forge.host, live.path);
    if !crate::forge::plain_place(&place) {
        return Err(
            "The origin of the clone names an address with a space or a control character in \
             it, or one too long to show, so purlis shows none and records nothing."
                .to_owned(),
        );
    }
    Ok(place)
}

/// The repo's bare name: the last part of its path, the name `purlis discover` keys it by.
fn bare_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The SSH address a record of `path` on `host` carries, as the forge's own form gives it: the
/// scp-like form for a host on its default port, and `ssh://` for a host that names a port,
/// where `host:path` would read the port as the start of the path.
fn ssh_url_for(host: &str, path: &str) -> String {
    if host.contains(':') {
        format!("ssh://git@{host}/{path}.git")
    } else {
        format!("git@{host}:{path}.git")
    }
}

/// **What a clone must be for Record this repo to be raised or pressed for it**: on GitHub (a
/// GitLab clone's record is `purlis discover`'s: the record built here is GitHub's), with an
/// address that can be shown ([`shown_place`]), in a folder named as origin names the repo (the
/// inventory is read by that name, so a record under another would leave the read refused), and
/// not one the project records another repo for. Answers the place the row shows. With
/// `recorded_ok`, a clone the project already records at this repo passes (a press that is a
/// second); otherwise it is refused as having nothing to record.
fn recordable(
    root: &Path,
    clone: &str,
    live: &crate::forge::pr::Repo,
    recorded_ok: bool,
) -> Result<String, String> {
    let short = crate::shown::short(clone);
    if live.forge.kind != crate::forge::Kind::GitHub {
        return Err(format!(
            "{short} is a GitLab clone, which Record this repo does not record. Record it with \
             `purlis discover`."
        ));
    }
    let place = shown_place(live)?;
    if bare_name(&live.path) != clone {
        return Err(format!(
            "{short} is in a folder named other than its origin's repo ({place}), and the \
             project records a repo by its name. Record it with `purlis discover`."
        ));
    }
    if let Some(row) = crate::catalogue::milestones::row_named(root, clone) {
        if !row.path_with_namespace.eq_ignore_ascii_case(&live.path) {
            return Err(format!(
                "The project records a different repo for {short} ({}), and a chat's origin does \
                 not replace it. Fix it with `purlis discover`.",
                crate::shown::short(&row.path_with_namespace)
            ));
        }
        if !recorded_ok
            && crate::catalogue::milestones::recorded(root, clone)
                .is_some_and(|recorded| crate::catalogue::milestones::same(live, &recorded))
        {
            return Err(format!("The project records the repo of {short} already."));
        }
    }
    Ok(place)
}

/// **The Record this repo the clone `clone` of workspace `workspace` needs** (A3), as its row
/// would show it: `Err` with a sentence where there is nothing to record, because the clone is
/// not one of the workspace's that a chat's refusal may name ([`Reach`]), its origin names no
/// repo on a forge the project declares, or the project records it already or another repo for
/// it ([`recordable`]). Read from the clone now, never from a chat's words.
pub fn record_repo_for(
    root: &Path,
    workspace: &str,
    clone: &str,
    reach: Reach<'_>,
) -> Result<Control, String> {
    let live = live_repo_of(root, workspace, clone, reach)?;
    let place = recordable(root, clone, &live, false)?;
    Ok(Control::RecordRepo {
        workspace: workspace.to_owned(),
        clone: clone.to_owned(),
        place,
    })
}

/// The repo the origin of the clone `clone` of `workspace` names now, on a forge the project
/// declares, among the clones `reach` allows.
fn live_repo_of(
    root: &Path,
    workspace: &str,
    clone: &str,
    reach: Reach<'_>,
) -> Result<crate::forge::pr::Repo, String> {
    let clones = crate::repos::clones(root, workspace).map_err(|_| {
        format!(
            "purlis could not read the repos of {}.",
            crate::shown::short(workspace)
        )
    })?;
    let repos = match reach.started {
        Some(started) => crate::forge::apptoken::there_at_start(clones.repos, started),
        None => clones.repos,
    };
    let repos = crate::forge::apptoken::held_from_chats(
        repos,
        &crate::forge::apptoken::grantable_beside(root, reach.writes),
    );
    let Some(found) = repos.iter().find(|one| one.name == clone) else {
        return Err(format!(
            "{} is not a repo of workspace {} that was there when the chat started and that no \
             chat can write.",
            crate::shown::short(clone),
            crate::shown::short(workspace)
        ));
    };
    crate::forge::pr::Repo::of_clone(root, &found.path)
}

/// **Whether the inventory's row `name` has the address `repo`** (`<host>/<owner>/<repo>`), the
/// check of Clone it's press: the row names the repo by `name`, and the clone it makes is the
/// row's, so a row that now has another address (or none) is refused with a sentence, and
/// nothing is cloned.
pub fn clone_it_matches(root: &Path, name: &str, repo: &str) -> Result<(), String> {
    let cfg = crate::forge::load_config(root).unwrap_or_default();
    let group = crate::forge::group_of(&cfg, 0);
    let doc = crate::inventory::load(root, &group)?;
    let all = crate::inventory::repos(root, &doc, &crate::forge::exclude_of(&cfg, 0));
    let changed = || {
        format!(
            "The project's repo {} is not at {repo} any more, so purlis cloned nothing. Look at \
             the ask again and choose",
            crate::shown::short(name)
        )
    };
    let row = crate::inventory::find(&all, name).ok_or_else(changed)?;
    let url = crate::repocmd::clone::https_url(row, root).map_err(|_| changed())?;
    match crate::forge::gitcredential::place_of(&url) {
        Some(place) if place == repo => Ok(()),
        _ => Err(changed()),
    }
}

/// **The words a chat is told where its read of a clone's forge was refused for want of a
/// record** (A3), after the ask for it was raised or not: raised, by [`told_of`]'s rule H.
/// `Err` where no ask could be raised, whose sentence says why and, where `purlis discover` is
/// the way out, names it: the clone has nothing to record or is not one a chat's refusal may
/// name ([`record_repo_for`]), the chat was not refused ([`refused_for_want_of_a_record`]), or
/// `[policy] consents` raises none.
pub fn ask_to_record(
    root: &Path,
    session: u32,
    workspace: &str,
    clone: &str,
    reach: Reach<'_>,
) -> Result<String, String> {
    // Only after a refusal the chat really had: the line is the chat's own word, and a chat
    // could hand it for any clone it can name.
    if !refused_for_want_of_a_record(root) {
        return Err(format!(
            "The project's policy does not refuse a read of {} for want of a record, so \
             nothing was asked.",
            crate::shown::short(clone)
        ));
    }
    let control = record_repo_for(root, workspace, clone, reach)?;
    if !asks(root) {
        return Err(format!(
            "The project records no repo for {}. Record it with `purlis discover`.",
            crate::shown::short(clone)
        ));
    }
    let raised = raise(root, session, control);
    Ok(told_of(
        root,
        session,
        raised,
        "Record this repo is on the ask, or run `purlis discover`.",
    ))
}

/// Whether the project's `[policy] guards.forge` enforces the check that refuses a read of a
/// clone the project records no repo for: under `log` or `off` the read goes ahead and nothing
/// refuses ([`crate::chattools::forge_milestones`]). The other half of a real refusal, that the
/// clone is unrecorded, is [`record_repo_for`]'s. Asked without a log line.
fn refused_for_want_of_a_record(root: &Path) -> bool {
    crate::enforcement::mode(root, crate::enforcement::Family::GuardsForge)
        == crate::enforcement::Mode::Enforce
}

/// **What the app said to the ask for Record this repo** (A3), as the chat's own `purlis` heard
/// it over the hook channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSaid {
    /// The app raised the ask or refused it, in its own words ([`ask_to_record`]).
    Told(String),
    /// The app could not read the ask ([`crate::hookwire::UNKNOWN_LINE`]): it is older than this
    /// purlis, so nobody was asked. Never a refusal of the repo.
    Older,
    /// No app answered, or none was there.
    Unheard,
}

/// **What a chat is told for a clone the project records no repo for** (A3), whatever the app
/// answered: its words where it raised the ask; where it could not read the ask, that the app
/// and this purlis are different builds, with the restart words; where none answered, that
/// nobody was asked ([`nobody_asked`]). Each names `purlis discover`, which the chat may run
/// itself.
pub fn record_repo_refusal(root: &Path, clone: &str, app: AppSaid) -> String {
    let clone = crate::shown::short(clone);
    match app {
        AppSaid::Told(words) => words,
        AppSaid::Older => format!(
            "This project records no repo for {clone}, and nobody was asked: the app and this \
             purlis are different builds, so the app could not read the ask. Ask the person to \
             restart purlis (Restart to update, or quit and reopen it), or to update the purlis \
             plugin of this chat's harness, so both are one build. Or record it with \
             `purlis discover`."
        ),
        AppSaid::Unheard => format!(
            "This project records no repo for {clone}. {}",
            nobody_asked(root, "run `purlis discover` to record it.")
        ),
    }
}

/// **The person's Record this repo** (A3): records the repo of the clone `shown.clone`, as
/// `purlis discover` does for that one clone, and clears every chat's ask for it. Answers the
/// address recorded. Refused, with a sentence, nothing recorded and the ask kept, where origin no
/// longer names the address the row showed, or the clone is no longer one a press may record
/// ([`recordable`]); `writes` are the folders the project's live chats may write, whose clones
/// are never recorded. Nothing about the forge is asked: the record is what origin names, so
/// what a chat later reads is the repo the person saw. It is keyed by the repo's bare name from
/// origin ([`bare_name`]), as `purlis discover` keys it. The forge is not asked, so the branch is
/// unknown: the record names none, and the plane format writes `main` for it
/// ([`crate::inventory::record`]), which nothing reads to choose a branch (`clone` checks out the
/// remote's own HEAD). Its SSH address is the forge's own form ([`ssh_url_for`]).
pub fn record_this_repo(
    root: &Path,
    shown: &Control,
    writes: &[PathBuf],
) -> Result<String, String> {
    let Control::RecordRepo {
        workspace,
        clone,
        place,
    } = shown
    else {
        return Err(format!("{} is not a Record this repo", shown.label()));
    };
    let changed = || {
        format!(
            "The origin of {clone} changed since the window showed it, so purlis recorded \
             nothing. Look at it again and choose"
        )
    };
    let reach = Reach {
        started: None,
        writes,
    };
    let live = live_repo_of(root, workspace, clone, reach).map_err(|_| changed())?;
    let now = recordable(root, clone, &live, true)?;
    if now != *place {
        return Err(changed());
    }
    let host = &live.forge.host;
    let path = &live.path;
    let record = crate::forge::RepoRecord {
        id: None,
        name: bare_name(path).to_owned(),
        path_with_namespace: path.to_owned(),
        default_branch: None,
        description: String::new(),
        web_url: format!("https://{place}"),
        ssh_url: ssh_url_for(host, path),
        topics: Vec::new(),
        forge: live.forge.kind,
    };
    let cfg = crate::forge::load_config(root)?;
    crate::inventory::add(
        root,
        &crate::forge::group_of(&cfg, 0),
        &[crate::inventory::record(&record, "unknown")],
    )?;
    pressed(root, shown);
    Ok(place.clone())
}

/// **The Remove a persona's or a branch folder's removal needs**, as its row would show it
/// (T1.1-67): the thing read now, with exactly what it would lose, never from a chat's words.
/// `workspace` is the chat's workspace, which a branch folder is looked up in. `Err` with a
/// sentence where there is nothing to ask: no such thing, or a workspace, whose removal has its
/// own ask.
pub fn removal_for(
    root: &Path,
    workspace: Option<&str>,
    removing: &crate::hookwire::Removing,
) -> Result<Control, String> {
    use crate::hookwire::Removing;
    match removing {
        Removing::Workspace { .. } => {
            Err("a workspace's removal is asked as the workspace's own".to_owned())
        }
        Removing::Persona { name } => {
            let listing = crate::removalask::persona_listing(root, name)?;
            Ok(Control::Remove {
                removing: removing.clone(),
                workspace: String::new(),
                listed: listing.lines,
                exact: listing.exact,
                digest: listing.digest,
            })
        }
        Removing::BranchFolder { repo, piece, .. } => {
            let Some(workspace) = workspace else {
                return Err(
                    "a chat at the project's root has no branch folders to remove".to_owned(),
                );
            };
            let loss = crate::removalask::folder_loss(root, workspace, repo, piece)?;
            if loss.listing.lines.is_empty() {
                return Err(format!(
                    "{} \u{b7} {} holds nothing a plain removal would lose, so nobody needs to be \
                     asked",
                    crate::shown::short(repo),
                    crate::shown::short(piece)
                ));
            }
            // The flags are what the app read, never what the line said.
            Ok(Control::Remove {
                removing: Removing::BranchFolder {
                    repo: repo.clone(),
                    piece: piece.clone(),
                    force: loss.force,
                    ignored: loss.ignored,
                },
                workspace: workspace.to_owned(),
                listed: loss.listing.lines,
                exact: loss.listing.exact,
                digest: loss.listing.digest,
            })
        }
    }
}

/// **Who asked for a removal, as the app records them** (never a word of the line's): the chat's
/// number, the [`Who`] its piece log credits (whose `chat` is its stable id, ADR 0066, which the
/// enforcement log names it by, Q308a), the chats the app has (what a branch folder's one live
/// writer is judged against, #3901; `None` where no app said, which refuses a folder another chat
/// holds) and whether the caller already holds the folder's claim and its checked folder (the
/// brokered route's removal, which asks from inside them).
#[derive(Debug, Clone, Copy)]
pub struct Asking<'a> {
    pub session: u32,
    pub who: &'a Who,
    pub live: Option<&'a [crate::piececlaim::Live]>,
    pub claimed: bool,
}

/// What [`ask_to_remove`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removal {
    /// The ask was handled: the sentence the chat is told (`Err` where nobody was asked).
    /// **Nothing was removed.**
    Asked(Result<String, String>),
    /// `guards.plane` lifted the ask (`log` or `off`), so the removal ran as the chat's own would
    /// have, held to everything but the ask: what it said, or why it removed nothing.
    Ran(Result<String, String>),
}

impl Removal {
    /// The words the chat is told, whichever it was.
    pub fn words(self) -> String {
        match self {
            Self::Asked(said) | Self::Ran(said) => said.unwrap_or_else(|why| why),
        }
    }
}

/// **What a chat is told once it asked for a removal**: raised by [`told_of`]'s rule H, or the
/// sentence that says why nothing was asked. Raises one ask per chat and thing (a newer listing
/// of the same thing replaces the older), and `[policy] consents` decides whether it is raised
/// at all, as for every window-only ask.
///
/// `guards.plane` (Q308b) lifts **only the ask**: under `log` the check runs and allows (a line
/// in the asking chat's log names the rule), and under `off` it does not run, and the removal
/// then runs through every guard its broker route holds it to ([`run_removal`]): never forced,
/// one live writer per folder, git in the checked folder.
pub fn ask_to_remove(
    root: &Path,
    asking: &Asking<'_>,
    workspace: Option<&str>,
    removing: &crate::hookwire::Removing,
) -> Removal {
    let mode = crate::enforcement::mode(root, crate::enforcement::Family::GuardsPlane);
    if crate::enforcement::judged(
        root,
        asking.who.chat.as_deref(),
        crate::enforcement::Family::GuardsPlane,
        mode,
        crate::enforcement::RULE_REMOVAL_ASK,
        None,
        (),
    )
    .is_none()
    {
        return Removal::Ran(run_removal(root, workspace, removing, Run::Lifted(asking)));
    }
    Removal::Asked(raise_removal(root, asking.session, workspace, removing))
}

fn raise_removal(
    root: &Path,
    session: u32,
    workspace: Option<&str>,
    removing: &crate::hookwire::Removing,
) -> Result<String, String> {
    let control = removal_for(root, workspace, removing)?;
    if !asks(root) {
        return Err(
            "Nobody was asked: this project's policy raises no asks, so nothing was removed. \
             The person removes it themselves."
                .to_owned(),
        );
    }
    let label = control.label();
    let raised = raise(root, session, control);
    Ok(told_of(
        root,
        session,
        raised,
        &format!("{label} is on the ask; nothing is removed until they press it."),
    ))
}

/// **What a chat is told of a removal it asked for**, whatever the app answered: the app's words
/// where it raised the ask; where it could not read the ask, that the app and this purlis are
/// different builds, with the restart words (never a refusal); where none answered, that nobody
/// was asked, with the way out in words (rule H). `what` names the thing ("persona 'x'"),
/// `then` what the person does once purlis is open.
pub fn removal_refusal(root: &Path, what: &str, then: &str, app: AppSaid) -> String {
    match app {
        AppSaid::Told(words) => words,
        AppSaid::Older => format!(
            "Nothing was removed, and nobody was asked about {what}: the app and this purlis are \
             different builds, so the app could not read the ask. Ask the person to restart \
             purlis (Restart to update, or quit and reopen it), or to update the purlis plugin \
             of this chat's harness, so both are one build."
        ),
        AppSaid::Unheard => {
            let project = root.file_name().map_or_else(
                || "this project".to_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
            format!(
                "Nothing was removed. No purlis window answered, so nobody was asked about \
                 {what}: open purlis on {project}, then run this again, and {then}"
            )
        }
    }
}

/// **The person's press on a removal**: removes what the row listed, as the person's own
/// `purlis persona remove` or `purlis worktree remove` would, and only while it still holds
/// exactly what the row listed. The thing is read again first. If anything differs (a new
/// change, a new commit, another ignored file, a memory more, an edit to a file already listed),
/// the row is replaced by what it lists now and **nothing is removed**; a listing that was never
/// exact is not pressed at all. `who` is the window's, which a removal ends a folder's claim as.
/// Answers what the removal said, or why nothing was removed.
pub fn remove_listed(root: &Path, shown: &Control, who: &Who) -> Result<String, String> {
    let Control::Remove {
        removing,
        workspace,
        listed,
        exact,
        ..
    } = shown
    else {
        return Err(format!("{} is not a removal", shown.label()));
    };
    if !exact {
        return Err(
            "purlis could not list everything this would delete, so it removes \
                    nothing: Keep it, or remove it yourself in a terminal"
                .to_owned(),
        );
    }
    let workspace_now = (!workspace.is_empty()).then_some(workspace.as_str());
    let now = removal_for(root, workspace_now, removing).inspect_err(|_| {
        // It is gone, or holds nothing now: the row waits on nothing.
        pressed(root, shown);
    })?;
    if now != *shown {
        let Control::Remove { listed: fresh, .. } = &now else {
            unreachable!("removal_for answers a removal");
        };
        refresh(root, shown, &now);
        return Err(format!(
            "What it holds changed since the window listed it ({} lines then, {} now), so \
             purlis removed nothing. The row lists what it holds now: look at it again and \
             choose",
            listed.len(),
            fresh.len()
        ));
    }
    let said = run_removal(root, workspace_now, removing, Run::Press(who))?;
    pressed(root, shown);
    Ok(said)
}

/// Whose removal [`run_removal`] runs, which decides what holds it.
#[derive(Clone, Copy)]
enum Run<'a> {
    /// The person's press: the flags the row was raised with, any folder (the person's own
    /// terminal hands any over), and the removal ends the folder's claim.
    Press(&'a Who),
    /// A chat's removal whose ask `guards.plane` lifted: **never forced**, whatever the line
    /// said, and held as the broker route holds it ([`crate::piececlaim::removing`]).
    Lifted(&'a Asking<'a>),
}

/// **Removes a persona or a branch folder as the person's own `purlis persona remove` or `purlis
/// worktree remove` would**: what the press does once the listing is confirmed, and what a
/// removal does under `guards.plane` = `log` or `off`. A branch folder is removed under its claim
/// and with git run in its checked clone and folder, as the broker's own route removes it: a chat
/// is refused a folder another live chat holds (#3901), and nothing here forces. Answers what the
/// removal said; `Err` with it where nothing was removed.
fn run_removal(
    root: &Path,
    workspace: Option<&str>,
    removing: &crate::hookwire::Removing,
    run: Run<'_>,
) -> Result<String, String> {
    use crate::hookwire::Removing;
    let mut said = Vec::new();
    let mut sink = |line: crate::repocmd::Say| said.push(line.to_string());
    let code = match (removing, workspace) {
        (Removing::Persona { name }, _) => crate::personaverbs::define::remove(
            root,
            name,
            false,
            &crate::active::ActivePersona {
                name: None,
                rung: crate::active::PersonaRung::Nothing,
            },
            &mut sink,
        ),
        (
            Removing::BranchFolder {
                repo, piece, force, ..
            },
            Some(workspace),
        ) => {
            // The press discards what it listed; a lifted ask never does.
            let force = matches!(run, Run::Press(_)) && *force;
            let remove = |sink: &mut dyn FnMut(crate::repocmd::Say)| {
                crate::piececmd::remove(
                    root,
                    workspace,
                    repo,
                    piece,
                    force,
                    false,
                    crate::finished::Forge::none(),
                    sink,
                )
            };
            let (who, judged, claimed) = match run {
                Run::Press(who) => (who, crate::piececlaim::Judged::ByThePerson, false),
                Run::Lifted(asking) => (
                    asking.who,
                    crate::piececlaim::Judged::ByAChat(asking.live),
                    asking.claimed,
                ),
            };
            if claimed {
                // The broker route asked from inside the claim and the checked folder.
                remove(&mut sink)
            } else {
                let at = crate::worktree::Located {
                    workspace: workspace.to_owned(),
                    repo: repo.clone(),
                    piece: piece.clone(),
                };
                let held = crate::worktree::git::isolation().unwrap_or_default();
                let mut not_run = None;
                let removed = crate::piececlaim::removing(
                    root,
                    &at,
                    who,
                    judged,
                    jiff::Timestamp::now(),
                    || match crate::gitbroker::in_a_checked_folder(
                        root,
                        workspace,
                        repo,
                        piece,
                        &held,
                        || remove(&mut sink),
                    ) {
                        Ok(code) => code,
                        Err(crate::gitbroker::NotRun::Repo(why)) => {
                            not_run = Some(why);
                            1
                        }
                        Err(crate::gitbroker::NotRun::Folder(why)) => {
                            not_run = Some(why.to_string());
                            1
                        }
                    },
                );
                match (removed, not_run) {
                    (_, Some(why)) | (Err(why), _) => {
                        return Err(format!("Nothing was removed. {why}"));
                    }
                    (Ok(code), None) => code,
                }
            }
        }
        _ => {
            return Err(
                "Nothing was removed: that is not a persona or a branch folder of a \
                        workspace."
                    .to_owned(),
            );
        }
    };
    let said = said.join("\n");
    if code == 0 {
        Ok(said)
    } else {
        Err(format!("Nothing was removed. {said}"))
    }
}

/// Replaces every chat's refusal that waited on `old` with `new`, and tells the window.
fn refresh(root: &Path, old: &Control, new: &Control) {
    let moved = {
        let mut raised = RAISED.lock().unwrap_or_else(|e| e.into_inner());
        let mut moved = false;
        for one in raised
            .iter_mut()
            .filter(|one| one.root == root && one.control == *old)
        {
            one.control = new.clone();
            moved = true;
        }
        moved
    };
    if moved {
        tell(root);
    }
}

/// **The person kept it** (Keep on a removal): every chat's ask for it goes, and nothing is
/// removed.
pub fn keep_listed(root: &Path, shown: &Control) {
    pressed(root, shown);
}

/// The label of the one ask for the project's clones purlis has no record of (R-336).
pub const TRUST_THESE_CLONES: &str = "Trust these clones";

/// **The one ask for the clones purlis has no record of** in the project at `root` (R-336,
/// Q184): every clone of its workspaces that purlis did not clone and the person has not
/// trusted ([`crate::forge::gitcredential::vouch::unvouched_clones`]), however many there are.
/// Raised by [`look_at_clones`], and listed with no chat's: it waits on the person alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustTheseClones {
    pub root: PathBuf,
    /// Each clone, by workspace and then name, with the address its remote names now.
    pub clones: Vec<crate::forge::gitcredential::vouch::Unvouched>,
    /// Since when it was raised, in seconds since the epoch: a clone added to it later keeps
    /// the ask's age.
    pub since: u64,
}

/// The one Trust these clones of each project that has one.
static UNRECORDED: Mutex<Vec<TrustTheseClones>> = Mutex::new(Vec::new());

/// **Reads the clones of the project at `root` that purlis has no record of in the config home
/// at `config_root`, and raises or settles its one ask for them** (R-336): one ask however many
/// there are and however often they are read, changed only where they changed, and gone once
/// there is none. Nothing is recorded: a clone is trusted only on the person's answer
/// ([`trust_this_clone`]). Answers the clones, for the Notice that lists each.
///
/// **The ask lists only the clones a press can trust and a mint would count** (Q257, Q249): not
/// one whose git directory is inside `writes`, the folders the project's live chats may write,
/// or a folder any chat may be granted (the clones
/// [`crate::forge::apptoken::held_from_chats`] drops), so a clone a chat made or can write never
/// asks the person; and not one whose origin names several addresses, which no press can trust
/// (the doctor names it, with the fix).
pub fn look_at_clones(
    root: &Path,
    config_root: &Path,
    writes: &[PathBuf],
) -> Vec<crate::forge::gitcredential::vouch::Unvouched> {
    let clones = crate::forge::gitcredential::vouch::unvouched_clones(root, config_root);
    if settle_clones(root, to_ask(root, &clones, writes)) {
        tell(root);
    }
    clones
}

/// The clones of `clones` that Trust these clones lists ([`look_at_clones`]).
fn to_ask(
    root: &Path,
    clones: &[crate::forge::gitcredential::vouch::Unvouched],
    writes: &[PathBuf],
) -> Vec<crate::forge::gitcredential::vouch::Unvouched> {
    let out_of_reach = crate::forge::apptoken::grantable_beside(root, writes);
    let plane = crate::workspaces::Plane::open(root.to_path_buf());
    let held: Vec<String> = plane
        .workspaces()
        .unwrap_or_default()
        .iter()
        .filter_map(|workspace| crate::repos::clones(root, workspace).ok())
        .flat_map(|clones| crate::forge::apptoken::held_from_chats(clones.repos, &out_of_reach))
        .map(|repo| crate::sandbox::local::clone_key(&repo.path))
        .collect();
    clones
        .iter()
        .filter(|clone| clone.place.is_some())
        .filter(|clone| held.contains(&crate::sandbox::local::clone_key(&clone.path)))
        .cloned()
        .collect()
}

/// Sets the project's one ask to `clones`, none taking it away. Answers whether it moved.
fn settle_clones(root: &Path, clones: Vec<crate::forge::gitcredential::vouch::Unvouched>) -> bool {
    let mut asks = UNRECORDED.lock().unwrap_or_else(|e| e.into_inner());
    match asks.iter().position(|one| one.root == root) {
        None if clones.is_empty() => false,
        None => {
            asks.push(TrustTheseClones {
                root: root.to_path_buf(),
                clones,
                since: now(),
            });
            true
        }
        Some(at) if clones.is_empty() => {
            asks.remove(at);
            true
        }
        Some(at) if asks[at].clones == clones => false,
        Some(at) => {
            asks[at].clones = clones;
            true
        }
    }
}

/// **The project's one ask for its clones purlis has no record of**, as the last read of them
/// left it ([`look_at_clones`]), or `None` where there is none.
pub fn clones_ask(root: &Path) -> Option<TrustTheseClones> {
    UNRECORDED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|one| one.root == root)
        .cloned()
}

/// The clone at `folder` was trusted: it leaves the project's Trust these clones.
fn clone_trusted(root: &Path, folder: &str) {
    let Some(ask) = clones_ask(root) else {
        return;
    };
    let left = ask
        .clones
        .into_iter()
        .filter(|clone| clone.folder != folder)
        .collect();
    if settle_clones(root, left) {
        tell(root);
    }
}
