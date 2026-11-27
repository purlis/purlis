// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Who owns what a chat acts on, as the host holds it** (#7748; ADR 0125, amended 2026-11-27,
//! §5; spec §3.2): `context.owner`, computed by [`owner`] once, for the real actor, and carried in
//! the request's context. It is never derived in Cedar from entity attributes. Spec §6.5 judges a
//! dispatched chat's act again for every asker up its chain, and each asker's evaluation reads the
//! value the child's did.
//!
//! # The values
//!
//! [`Owner`]: `"self"` (the actor), `"task"` (a chat down the actor's own dispatch chain, at any
//! depth), `"asker"` (one up it, at any depth), `"other"` (any other chat, a sibling or cousin
//! included) or `"none"` (no chat: the person's, or not recorded). **Lineage is the actor's own
//! line, up and down.** A sibling or cousin is reached only through a common ancestor, so it is
//! never on it.
//!
//! # The records
//!
//! There are two ownerships, kept apart ([`Owned`]):
//!
//! - a **forge branch** is owned by the chat the forge route recorded creating it (`push`,
//!   `merge`);
//! - a **local branch folder** is owned by the chat whose claim on it the app recorded first
//!   (`commit`, `update`, `discard`).
//!
//! The actor's line comes from the app's record of who dispatched whom. All three are **owner
//! records** ([`answers::OWNER`]) in the sealed answer store. Only the app can seal a record, and
//! the store's manifest and counter are anchored in the keyring, so a line put back counts for
//! nothing. A record that is missing, edited or forged fails its seal and is no record: its
//! resource's owner reads `"none"`, never `"self"`. While the store lists such a record, no new
//! record is made at all, and every commit lists it again (Q339.8). No record is read from a
//! file a chat can write: not the piece log, which is the one-writer mutex's (protocol, §7),
//! not the dispatch records, and not `forge-created-branches.jsonl`. In any process but the app
//! nothing is verified, and every owner reads `"none"`.
//!
//! A port's server and a browser are owned by the chat that opened them, by the host's own
//! registry. That registry is held in the app's memory, and its caller hands [`owner`] the holder.
//!
//! # How a record moves
//!
//! **A record, once made, moves only by hand-over or the person's press.** A second chat's record
//! of the same branch or folder changes nothing ([`record_owner`]). A finished task's hand-over
//! gives everything it owns to the chat that dispatched it ([`hand_over`]). The person's press of
//! "Let this chat push it" gives one to the chat named ([`let_this_chat`]). Nothing else in purlis
//! writes an owner record: [`answers::Draft::add`] and [`answers::Draft::remove`] refuse the kind.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::secrets::answers::{self, Press, Standing};

/// **The value of `context.owner`**: what the owner of a resource is to the chat that acts on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Owner {
    /// `"self"`: the actor owns it.
    Actor,
    /// `"task"`: a chat down the actor's own dispatch chain, at any depth.
    Task,
    /// `"asker"`: a chat up the actor's own dispatch chain, at any depth.
    Asker,
    /// `"other"`: any other chat, a sibling or cousin included.
    Other,
    /// `"none"`: no chat. The person's, or not recorded.
    Nobody,
}

impl Owner {
    /// Every value, in the schema's order.
    pub const ALL: [Self; 5] = [
        Self::Actor,
        Self::Task,
        Self::Asker,
        Self::Other,
        Self::Nobody,
    ];

    /// The word the context carries.
    pub fn word(self) -> &'static str {
        match self {
            Self::Actor => "self",
            Self::Task => "task",
            Self::Asker => "asker",
            Self::Other => "other",
            Self::Nobody => "none",
        }
    }
}

/// **What a chat can own, by an owner record.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owned<'a> {
    /// A forge branch, as the forge route keys it: `(host, repo path, branch)`.
    Branch {
        host: &'a str,
        repo: &'a str,
        branch: &'a str,
    },
    /// A local branch folder, by its real path.
    Folder(&'a Path),
}

impl Owned<'_> {
    fn key(&self) -> String {
        match self {
            Self::Branch { host, repo, branch } => format!("branch {host}/{repo}#{branch}"),
            Self::Folder(path) => format!("folder {}", path.display()),
        }
    }
}

/// **One owner record**, as its entry in the sealed store spells it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "fact", rename_all = "kebab-case", deny_unknown_fields)]
enum Fact {
    /// `chat` owns `of` ([`Owned::key`]).
    Owns { of: String, chat: String },
    /// `task` was dispatched by `by`.
    Asked { task: String, by: String },
}

impl Fact {
    fn text(&self) -> String {
        serde_json::to_string(self).expect("an owner record is plain strings")
    }
}

/// **The owner records the app sealed**, as one decision reads them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Records {
    /// What each resource's record names as its owner, by [`Owned::key`]. A key two records
    /// name with different chats is left out: neither is believed.
    owns: BTreeMap<String, String>,
    /// Who dispatched each task, by the task's stable id.
    asked: BTreeMap<String, String>,
}

impl Records {
    /// The owner records of a plane's standing: only what its verified store holds
    /// ([`Standing::entries`]). A plane that is fresh, in the deny state or unverifiable here
    /// holds none, and every owner reads `"none"`.
    pub fn of(standing: &Standing) -> Self {
        let mut owns: BTreeMap<String, Option<String>> = BTreeMap::new();
        let mut asked: BTreeMap<String, Option<String>> = BTreeMap::new();
        let facts = standing
            .entries
            .iter()
            .filter(|entry| entry.kind == answers::OWNER)
            .filter_map(|entry| serde_json::from_str::<Fact>(&entry.text).ok());
        for fact in facts {
            let (map, key, chat) = match fact {
                Fact::Owns { of, chat } => (&mut owns, of, chat),
                Fact::Asked { task, by } => (&mut asked, task, by),
            };
            if chat.is_empty() {
                continue;
            }
            map.entry(key)
                .and_modify(|held| {
                    if held.as_deref() != Some(chat.as_str()) {
                        *held = None;
                    }
                })
                .or_insert(Some(chat));
        }
        let settled = |map: BTreeMap<String, Option<String>>| {
            map.into_iter()
                .filter_map(|(key, chat)| Some((key, chat?)))
                .collect()
        };
        Self {
            owns: settled(owns),
            asked: settled(asked),
        }
    }

    /// The owner records of the plane at `root`, as this process can verify them.
    pub fn read(root: &Path) -> Self {
        Self::of(&answers::standing(root))
    }

    /// The chat a record names as the owner of `owned`, where one does.
    pub fn holder(&self, owned: &Owned<'_>) -> Option<&str> {
        self.owns.get(&owned.key()).map(String::as_str)
    }

    /// The chat that dispatched `task`, where a record says one did.
    pub fn asker_of(&self, task: &str) -> Option<&str> {
        self.asked.get(task).map(String::as_str)
    }

    /// Whether `above` is up `chat`'s dispatch chain, at any depth. A loop no dispatch made ends
    /// the walk.
    fn is_up_the_chain(&self, chat: &str, above: &str) -> bool {
        let mut seen = BTreeSet::new();
        let mut at = chat;
        while let Some(asker) = self.asker_of(at) {
            if asker == above {
                return true;
            }
            if !seen.insert(asker) {
                return false;
            }
            at = asker;
        }
        false
    }
}

/// **`context.owner`: the one host function** (#7748). It gives what `holder`, the chat a host
/// record names as the owner of the resource, is to `actor`, the real actor, along the actor's own
/// line in `records`. The caller computes it once for the real actor and hands the same value to
/// every asker's evaluation up the chain (spec §6.5): it is never computed again for an asker.
///
/// `holder` is what [`Records::holder`] read, or the host registry's opener of a port's server or
/// a browser. No holder, or an empty one, is `"none"`.
pub fn owner(actor: &str, holder: Option<&str>, records: &Records) -> Owner {
    let Some(holder) = holder.filter(|holder| !holder.is_empty()) else {
        return Owner::Nobody;
    };
    if holder == actor {
        Owner::Actor
    } else if records.is_up_the_chain(holder, actor) {
        Owner::Task
    } else if records.is_up_the_chain(actor, holder) {
        Owner::Asker
    } else {
        Owner::Other
    }
}

/// **What an act touches, for its `context.owner`**, as the enforcement point that asks names it
/// to the host ([`crate::policy::decide::Sealed::touched`]). Nothing named reads `"none"`: the
/// merge hold (its resource is a pull request, whose head branch is not read yet: #7035) and the
/// forge operations ask that way today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Touched {
    /// A forge branch or a branch folder, which an owner record keys ([`Owned`]).
    Recorded(String),
    /// A port's server or a browser, by the chat the host's registry says opened it.
    OpenedBy(String),
}

impl From<Owned<'_>> for Touched {
    fn from(owned: Owned<'_>) -> Self {
        Self::Recorded(owned.key())
    }
}

/// **`context.owner` for one request**: [`owner`] for `actor`, the asking chat's stable id, over
/// the records `standing` verified, of what `touched` names. No actor or nothing named is
/// `"none"`, never `"self"`.
pub fn of_request(standing: &Standing, actor: Option<&str>, touched: Option<&Touched>) -> Owner {
    let (Some(actor), Some(touched)) = (actor.filter(|actor| !actor.is_empty()), touched) else {
        return Owner::Nobody;
    };
    let records = Records::of(standing);
    let holder = match touched {
        Touched::Recorded(key) => records.owns.get(key).map(String::as_str),
        Touched::OpenedBy(chat) => Some(chat.as_str()),
    };
    owner(actor, holder, &records)
}

/// **Records that `chat` owns `owned`**: the app's record of a forge branch the route saw it
/// create, or of the first claim on a branch folder. **The first record stands.** Where one is
/// already sealed, this changes nothing, whoever asks: a record moves only by [`hand_over`] or
/// [`let_this_chat`]. Returns whether a record was made. Refused where this process cannot seal
/// (it is not the app), where the plane is in the deny state, and **while the store lists an owner
/// record that does not load** (Q339.8): the key a lost record named can't be read, so a second
/// chat could otherwise be recorded as the owner in its place.
pub fn record_owner(root: &Path, owned: &Owned<'_>, chat: &str) -> io::Result<bool> {
    let key = owned.key();
    record_once(
        root,
        chat,
        |records| records.owns.contains_key(&key),
        || Fact::Owns {
            of: key.clone(),
            chat: chat.to_owned(),
        },
    )
}

/// **Records that `asker` dispatched `task`**, from the app's own dispatch. The first record
/// stands, and it is refused while an owner record is lost, as in [`record_owner`]: a chat is
/// dispatched once.
pub fn record_dispatch(root: &Path, task: &str, asker: &str) -> io::Result<bool> {
    if asker.is_empty() || task == asker {
        return Ok(false);
    }
    record_once(
        root,
        task,
        |records| records.asked.contains_key(task),
        || Fact::Asked {
            task: task.to_owned(),
            by: asker.to_owned(),
        },
    )
}

/// Why a new owner record is refused while one is lost (Q339.8).
const LOST: &str = "an owner record in this project's sealed answers is missing or was changed \
                    outside purlis, so purlis records no new owner of a branch, folder or task \
                    here: nothing was recorded";

fn record_once(
    root: &Path,
    chat: &str,
    held: impl Fn(&Records) -> bool,
    fact: impl Fn() -> Fact,
) -> io::Result<bool> {
    if chat.is_empty() {
        return Ok(false);
    }
    let mut made = false;
    answers::commit(root, |draft| {
        // A record deleted or edited by hand names a key no one can read now, so no key is known
        // to be free: no new record is made while one is lost (Q339.8).
        if draft.owner_records_lost() {
            draft.refuse(LOST);
            return;
        }
        if held(&records_in(draft)) {
            return;
        }
        draft.add_owner_record(&fact().text());
        made = true;
    })?;
    Ok(made)
}

/// **A finished task's hand-over** (ADR 0125 §5): everything `task` owns, its forge branches and
/// its branch folders, passes to the chat that dispatched it, by the dispatch record. The caller
/// hands over only where `guards-git-take-up-child` permits it, once the task has ended (its report
/// sent, or its `worktree done`); for an unfinished task the rule decides nothing and no caller
/// hands anything over. Returns how many records moved: none where no dispatch record names the
/// task's asker.
pub fn hand_over(root: &Path, task: &str) -> io::Result<usize> {
    let mut moved = 0;
    answers::commit(root, |draft| {
        let records = records_in(draft);
        let Some(asker) = records.asker_of(task).map(str::to_owned) else {
            return;
        };
        for (of, chat) in &records.owns {
            if chat != task {
                continue;
            }
            draft.take_out_owner_record(
                &Fact::Owns {
                    of: of.clone(),
                    chat: chat.clone(),
                }
                .text(),
            );
            draft.add_owner_record(
                &Fact::Owns {
                    of: of.clone(),
                    chat: asker.clone(),
                }
                .text(),
            );
            moved += 1;
        }
    })?;
    Ok(moved)
}

/// **The person's press of "Let this chat push it"**: `owned` passes to `chat`, whoever owned it.
/// Only a [`Press`] reaches it, so no chat's request does.
pub fn let_this_chat(_press: &Press, root: &Path, owned: &Owned<'_>, chat: &str) -> io::Result<()> {
    if chat.is_empty() {
        return Err(io::Error::other(
            "a branch is let to a chat by its id, and none was named, so nothing was changed",
        ));
    }
    let key = owned.key();
    answers::commit(root, |draft| {
        let records = records_in(draft);
        if let Some(held) = records.owns.get(&key) {
            draft.take_out_owner_record(
                &Fact::Owns {
                    of: key.clone(),
                    chat: held.clone(),
                }
                .text(),
            );
        }
        draft.add_owner_record(
            &Fact::Owns {
                of: key.clone(),
                chat: chat.to_owned(),
            }
            .text(),
        );
    })?;
    Ok(())
}

/// The owner records a commit's draft holds now.
fn records_in(draft: &answers::Draft) -> Records {
    let mut standing = Standing::fresh();
    standing.entries = draft.entries().to_vec();
    Records::of(&standing)
}

#[cfg(test)]
#[path = "owner_tests.rs"]
mod tests;
