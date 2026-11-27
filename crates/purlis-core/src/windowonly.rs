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
//! **A chat's push to a branch it has no record of** is the second ([`Control::LetChatPush`],
//! ADR 0128, amended 2026-11-23): an update to an existing, unprotected branch that no chat owns
//! in the app's record, or another does (pushed before the route existed, by the person, a
//! watcher or an ended chat), raises **Let this chat push it**. The person's **Allow** records
//! the chat as the branch's owner at the tip the forge holds *when they press*, read as the
//! person ([`let_chat_push`]), and from then on the tip rule applies as to any branch the chat
//! made. **Keep blocked** ([`keep_push_blocked`]) leaves it refused for the chat's life, and the
//! chat is not asked again. Never a protected, default, deploy or pages branch: those raise no
//! ask. No chat, and not the CLI, can record an owner: only this press does.
//!
//! The refusals and the clones' ask are kept in memory alone, by project, as the tokens they
//! stand in for are ([`crate::forge::apptoken`]): they are this run's, a chat's go when it ends
//! ([`forget`]), and a project's when it ends ([`project_ended`]).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::forge::route::HostScope;

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
    /// **Let this chat push a branch it has no record of** (ADR 0128, amended 2026-11-23): what
    /// [`let_chat_push`] records, keyed as the created-branch record is
    /// ([`crate::forge::createdbranches`]), by the chat's id, the forge's id of the repo and the
    /// branch. The project is the ask's own, never part of what is pressed.
    LetChatPush {
        /// The id the route records the chat's pushes under.
        chat: String,
        host: String,
        /// The lower-case repo path, for the person reading the ask.
        repo: String,
        /// The forge's numeric id of the repo.
        repo_id: u64,
        branch: String,
    },
}

impl Control {
    /// The control's label, as the window draws it.
    pub fn label(&self) -> &'static str {
        match self {
            Self::TrustClone { .. } => TRUST_THIS_CLONE,
            Self::LetChatPush { .. } => ALLOW,
        }
    }

    /// What the ask says, in purlis's words. `owner` is the other chat the record names as the
    /// owner of a branch, when there is one ([`Refusal::owner`]).
    pub fn says(&self, owner: Option<&str>) -> String {
        match self {
            Self::TrustClone { folder, place } => format!(
                "{TRUST_SO_CHATS_CAN_USE_GITHUB}: {folder} was not cloned by purlis, and its \
                 remote is {place}. Trust it only if you made this clone yourself."
            ),
            Self::LetChatPush {
                host, repo, branch, ..
            } => {
                let whose = match owner {
                    None => "Nobody owns it in purlis's record".to_owned(),
                    Some(chat) => format!("Chat {chat} owns it in purlis's record, not this chat"),
                };
                format!(
                    "{branch} in {repo} on {host} is not a branch this chat made through purlis. \
                     {whose}. Allow records this chat as its owner at the commit the forge has \
                     on it when you press, and then it can add to the branch's history. Keep \
                     blocked leaves the push refused."
                )
            }
        }
    }
}

/// The label of the control that lets a chat push a branch it has no record of.
pub const ALLOW: &str = "Allow";

/// The label of the answer that leaves such a push refused.
pub const KEEP_BLOCKED: &str = "Keep blocked";

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
    /// For a Let this chat push ask: the other chat the record names as the branch's owner,
    /// `None` where nobody owns it. Said in the ask; never part of what an ask is (its key).
    pub owner: Option<String>,
}

impl Refusal {
    /// What the ask says, in purlis's words.
    pub fn says(&self) -> String {
        self.control.says(self.owner.as_deref())
    }
}

/// Told that a project's asks moved, so the window reads them again.
type Told = Arc<dyn Fn() + Send + Sync>;

/// Every refusal raised, over every project.
static RAISED: Mutex<Vec<Refusal>> = Mutex::new(Vec::new());

/// What the person **kept blocked**, by project and chat ([`keep_push_blocked`]): in memory, for
/// the chat's life, so a chat refused again is not asked again.
static KEPT: Mutex<Vec<(PathBuf, u32, Control)>> = Mutex::new(Vec::new());

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
///
/// **The forge route's NotYourBranch refusal is not asked here: its Let this chat push it is
/// raised in every `consents` mode** ([`PushAsks`]; ADR 0125, amended 2026-11-27, §10; #7749).
/// The push stays refused in every mode until the person's press, so turning the family down
/// would only leave it with no way out.
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

/// **Raises chat `session`'s refusal in the project at `root`, whose way out is `control`**:
/// one ask per chat and control. A chat's earlier Trust this clone for the same folder at
/// another place goes: its remote changed, so that ask's press would trust nothing, and the new
/// ask replaces it rather than stacking. Answers whether it is new.
pub fn raise(root: &Path, session: u32, control: Control) -> bool {
    raise_owned(root, session, control, None)
}

/// [`raise`], for a Let this chat push ask that names the other chat `owner` as the owner.
pub fn raise_owned(root: &Path, session: u32, control: Control, owner: Option<String>) -> bool {
    let new = {
        let mut raised = RAISED.lock().unwrap_or_else(|e| e.into_inner());
        if let Control::TrustClone { folder, .. } = &control {
            raised.retain(|one| {
                let Control::TrustClone { folder: theirs, .. } = &one.control else {
                    return true;
                };
                !(one.root == root
                    && one.session == session
                    && theirs == folder
                    && one.control != control)
            });
        }
        let already = raised
            .iter()
            .any(|one| one.root == root && one.session == session && one.control == control);
        if !already {
            raised.push(Refusal {
                root: root.to_path_buf(),
                session,
                control,
                since: now(),
                owner,
            });
        }
        !already
    };
    if new {
        tell(root);
    }
    new
}

/// **Raises chat `session`'s Let this chat push ask `control` only while fewer than `at_most` of
/// its push asks wait**, counting and adding under the one lock, so pushes served at once on
/// several threads cannot each see room and all raise. Answers whether it raised it.
fn raise_push_capped(
    root: &Path,
    session: u32,
    control: Control,
    owner: Option<String>,
    at_most: usize,
) -> bool {
    let new = {
        let mut raised = RAISED.lock().unwrap_or_else(|e| e.into_inner());
        let waiting = raised
            .iter()
            .filter(|one| {
                one.root == root
                    && one.session == session
                    && matches!(one.control, Control::LetChatPush { .. })
            })
            .count();
        let already = raised
            .iter()
            .any(|one| one.root == root && one.session == session && one.control == control);
        if already || waiting >= at_most {
            false
        } else {
            raised.push(Refusal {
                root: root.to_path_buf(),
                session,
                control,
                since: now(),
                owner,
            });
            true
        }
    };
    if new {
        tell(root);
    }
    new
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

/// **Chat `session` of the project at `root` ended**: its refusals go with it, and what the
/// person kept blocked for it.
pub fn forget(root: &Path, session: u32) {
    KEPT.lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|(at, one, _)| !(at == root && *one == session));
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
    KEPT.lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|(at, _, _)| at != root);
    UNRECORDED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|one| one.root != root);
    TOLD.lock()
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

/// **Whether the person kept `control` blocked for chat `session`** of the project at `root`: the
/// route does not raise it again, and tells the chat so.
pub fn is_kept_blocked(root: &Path, session: u32, control: &Control) -> bool {
    KEPT.lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|(at, one, kept)| at == root && *one == session && kept == control)
}

/// Whether `control` is waiting for `session` of the project at `root`.
fn is_waiting(root: &Path, session: u32, control: &Control) -> bool {
    RAISED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|one| one.root == root && one.session == session && one.control == *control)
}

/// **The person's Keep blocked** on a Let this chat push ask: the ask goes, nothing is recorded,
/// and it is not raised again for this chat. Refused unless the ask is waiting in this project
/// for this chat.
pub fn keep_push_blocked(root: &Path, session: u32, control: &Control) -> Result<(), String> {
    if !matches!(control, Control::LetChatPush { .. }) || !is_waiting(root, session, control) {
        return Err("This ask is not waiting any more.".to_owned());
    }
    KEPT.lock().unwrap_or_else(|e| e.into_inner()).push((
        root.to_path_buf(),
        session,
        control.clone(),
    ));
    pressed(root, control);
    Ok(())
}

/// **The person's Allow** on a Let this chat push ask (ADR 0128, amended 2026-11-23): records the
/// chat as the owner of the branch at the tip `read_tip(host, repo, branch)` answers, **read from
/// the forge as the person now** and never the chat's word, and clears the ask. From then on the
/// tip rule applies as to a branch the chat made. Answers the tip recorded.
///
/// **Refused** (and nothing recorded) unless the ask is waiting for `session` in the project at
/// `root`: the control is a key to an ask this process raised, so another project's ask, one that
/// was answered, or one a chat made up grants nothing; unless the project has an id to hold it by
/// ([`crate::scaffold::planefile::id_to_hold_by`]: its stored one, else one derived from its path);
/// and unless the forge answers a full commit id. The project in the record is `root`'s own. Like
/// every provisioning act it takes the person's scope ([`HostScope`]), which no chat's process
/// tree can claim.
pub fn let_chat_push(
    _host: &HostScope,
    config_root: &Path,
    root: &Path,
    session: u32,
    control: &Control,
    read_tip: &dyn Fn(&str, &str, &str) -> Result<String, String>,
) -> Result<String, String> {
    let Control::LetChatPush {
        chat,
        host,
        repo,
        repo_id,
        branch,
    } = control
    else {
        return Err("This ask does not let a chat push.".to_owned());
    };
    if !is_waiting(root, session, control) {
        return Err("This ask is not waiting any more.".to_owned());
    }
    // The id the route reads the record by ([`crate::scaffold::planefile::id_to_hold_by`]).
    let project_id = crate::scaffold::planefile::id_to_hold_by(root)
        .map_err(|why| format!("This project has no id to record a branch under ({why})."))?;
    let tip = read_tip(host, repo, branch)?;
    if !crate::forge::createdbranches::is_commit_id(&tip) {
        return Err(format!("The forge did not give a commit for {branch}."));
    }
    crate::forge::createdbranches::record_via(
        config_root,
        &project_id,
        chat,
        &[crate::forge::createdbranches::CreatedBranch {
            host: host.clone(),
            repo: repo.clone(),
            repo_id: Some(*repo_id),
            branch: branch.clone(),
            tip: tip.clone(),
        }],
        Some(crate::forge::createdbranches::Via::Allow),
    )
    .map_err(|why| format!("purlis could not record the branch: {why}"))?;
    pressed(root, control);
    Ok(tip)
}

/// How many Let this chat push asks may wait for one chat at once: past it a push raises no
/// new ask, and git's reason says asks are already waiting.
pub const MAX_PUSH_ASKS_WAITING: usize = 3;

/// How many new asks one push may raise.
pub const MAX_PUSH_ASKS_PER_PUSH: usize = 1;

/// Who the record names as the owner of `(host, repo id, branch)`.
pub type OwnerOf = Box<dyn Fn(&str, u64, &str) -> Option<String> + Send + Sync>;

/// **The route's ask to let a chat push a branch it has no record of**
/// ([`crate::forge::proxy::BranchAsk`]) for one chat of one project: raises an ask for a branch
/// ([`Control::LetChatPush`], one per chat and branch however often it is refused), and says
/// in git's output that it is waiting in the window, **in every `[policy] consents` mode** (ADR
/// 0125, amended 2026-11-27, §10). **At most [`MAX_PUSH_ASKS_PER_PUSH`] new ask
/// for a push and [`MAX_PUSH_ASKS_WAITING`] waiting for the chat**: past either, nothing new is
/// raised and the chat is told asks are already waiting. **Nothing here answers it**: only the
/// person's press does ([`let_chat_push`], [`keep_push_blocked`]).
pub struct PushAsks {
    /// The project's folder.
    pub root: PathBuf,
    /// The id the route records this chat's pushes under.
    pub chat: String,
    /// The chat's session while it is open, read at each ask: a chat that has ended asks nothing.
    pub session: Box<dyn Fn() -> Option<u32> + Send + Sync>,
    /// Who the record names as the owner of `(host, repo_id, branch)`, for the ask to say: a
    /// chat's name, or `None` where nobody owns it.
    pub owner_of: OwnerOf,
}

impl crate::forge::proxy::BranchAsk for PushAsks {
    fn ask(
        &self,
        _: &crate::forge::proxy::ChatId,
        branches: &[crate::forge::proxy::NotYours],
    ) -> Option<String> {
        // Raised in every `[policy] consents` mode, unlike the other window-only refusals
        // ([`asks`]): the refusal stands whatever the mode, and the press is its only way out.
        let session = (self.session)()?;
        let mut waiting: Vec<&crate::forge::proxy::NotYours> = Vec::new();
        let mut kept: Vec<&crate::forge::proxy::NotYours> = Vec::new();
        let mut raised_now = 0;
        let mut held_back = false;
        for branch in branches {
            let control = Control::LetChatPush {
                chat: self.chat.clone(),
                host: branch.host.clone(),
                repo: branch.repo.clone(),
                repo_id: branch.repo_id,
                branch: branch.branch.clone(),
            };
            if is_kept_blocked(&self.root, session, &control) {
                kept.push(branch);
            } else if is_waiting(&self.root, session, &control) {
                waiting.push(branch);
            } else {
                if raised_now >= MAX_PUSH_ASKS_PER_PUSH {
                    held_back = true;
                    continue;
                }
                // The owner is read before the lock: it reads a file.
                let owner = (self.owner_of)(&branch.host, branch.repo_id, &branch.branch)
                    .filter(|chat| *chat != self.chat);
                if !raise_push_capped(&self.root, session, control, owner, MAX_PUSH_ASKS_WAITING) {
                    held_back = true;
                    continue;
                }
                raised_now += 1;
                waiting.push(branch);
            }
        }
        let names = |of: &[&crate::forge::proxy::NotYours]| match of {
            [one] => format!("{} in {}", one.branch, one.repo),
            _ => format!("{} branches", of.len()),
        };
        Some(if !waiting.is_empty() {
            format!(
                "purlis: the ask Let this chat push it is waiting in the window for {}; push \
                 again once the person has pressed Allow{}",
                names(&waiting),
                if held_back {
                    ". No ask was raised for the other branches: asks are already waiting"
                } else {
                    ""
                }
            )
        } else if !kept.is_empty() {
            format!(
                "purlis: the person kept {} blocked for this chat; ask the person to push it",
                names(&kept)
            )
        } else {
            "purlis: asks to let this chat push a branch are already waiting in the window, so \
             no new one was raised; push again once the person has answered them"
                .to_owned()
        })
    }
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
