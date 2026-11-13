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
}

impl Control {
    /// The control's label, as the window draws it.
    pub fn label(&self) -> &'static str {
        match self {
            Self::TrustClone { .. } => TRUST_THIS_CLONE,
        }
    }

    /// What the ask says, in purlis's words.
    pub fn says(&self) -> String {
        match self {
            Self::TrustClone { folder, place } => format!(
                "{TRUST_SO_CHATS_CAN_USE_GITHUB}: {folder} was not cloned by purlis, and its \
                 remote is {place}. Trust it only if you made this clone yourself."
            ),
        }
    }
}

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

/// **Raises chat `session`'s refusal in the project at `root`, whose way out is `control`**:
/// one ask per chat and control. A chat's earlier Trust this clone for the same folder at
/// another place goes: its remote changed, so that ask's press would trust nothing, and the new
/// ask replaces it rather than stacking. Answers whether it is new.
pub fn raise(root: &Path, session: u32, control: Control) -> bool {
    let new = {
        let mut raised = RAISED.lock().unwrap_or_else(|e| e.into_inner());
        let Control::TrustClone { folder, .. } = &control;
        raised.retain(|one| {
            let Control::TrustClone { folder: theirs, .. } = &one.control;
            !(one.root == root
                && one.session == session
                && theirs == folder
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
pub fn look_at_clones(
    root: &Path,
    config_root: &Path,
) -> Vec<crate::forge::gitcredential::vouch::Unvouched> {
    let clones = crate::forge::gitcredential::vouch::unvouched_clones(root, config_root);
    if settle_clones(root, clones.clone()) {
        tell(root);
    }
    clones
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
