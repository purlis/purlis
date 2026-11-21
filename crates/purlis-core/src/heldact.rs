// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Held forge acts** (one login, slice F; ADR 0128 D8): a chat holds no forge credential, so the
//! acts that are hard to undo are not refused dead. The chat **asks**, the person **presses** in
//! Asks, and the **app performs the act with the person's sign-in**. A chat never performs one.
//!
//! This generalises the merge hold ([`crate::mergehold`], which still holds merges, now also on
//! the proxy's request) to the other four acts:
//!
//! | Act | The ask shows | What the press does |
//! |---|---|---|
//! | **Tag** | repo, name, **the commit**, and **whether it is on the default branch** | creates the tag at that commit. An existing tag is never moved: refused, not held |
//! | **Release** | the same, plus the title, the notes and whether it makes the tag | makes the tag at the commit shown, then the release **against the tag**, with no `target_commitish` |
//! | **CI-file push** | the branch, the new tip, what it was compared with and **the changed lines of the CI files** | pushes the held tip: on GitHub with **the second credential** ([`WorkflowCredential`]), on GitLab with the everyday sign-in |
//! | **Fork** | the source (a recorded repo) and the person's own account as the only place it goes | forks into that account, and says what it made |
//!
//! # The ask is what happens
//!
//! **Read by the app, never by the chat.** A request ([`Requested`]) carries only the names a chat
//! used. [`prepare`] reads the rest from the forge and the clone, once, into a [`Prepared`]: the
//! exact [`Act`] and what the person is [`Shown`] of it. A name that moves is pinned to what it
//! was when the ask was made: a release's `target_commitish: main` becomes the commit `main` was
//! at, so a branch that moves afterwards changes nothing about what is released.
//!
//! **Bound to what was shown.** The window hands a press the digest of the row it drew
//! ([`Prepared::digest`], over the act and everything shown). A press performs only [`HeldActs`]'s
//! own copy of the act: there is no path by which a caller names another commit, tag, tip or
//! account at the press. And it **reads the forge again before it performs**: a read that gives
//! another act (a tag that now exists, a base that moved, a repository that is another) retires
//! the ask for one at what is true now, and nothing is performed.
//!
//! **Two presses**, as a merge takes ([`PRESSES`]): the first asks once more of the facts as the
//! forge says them now; the second performs, and only if nothing the row showed has changed
//! between them. **A CI-file push is never approved by default and never remembered**: no grant
//! exists for any held act ([`SOURCE`]; [`crate::askrules::Store::make`] refuses one), and a
//! press takes an [`Answerer`] made from the **window's own connection** (ADR 0128 D5): a press
//! from `charter inbox`, a shell or a socket is refused ([`NotDone::NotTheWindow`]). The second
//! press names the nonce the first returned ([`Pressed::AsksOnceMore`]), so a double click of
//! the first performs nothing and a double click of the second performs once.
//!
//! **"purlis decides" never applies.** Nothing here has a path by which a policy, a rule or a mode
//! presses. **Unattended, nothing is performed**: the act is refused with [`NOBODY_HERE`], and
//! queued for a connected desktop ([`HeldActs::take_queued`]).
//!
//! # The second credential
//!
//! On GitHub a push that changes a workflow needs the `workflow` scope, which lives on **a second
//! credential, made by its own sign-in, used only for a held push and for no route of a chat**.
//! Slice C makes it; [`WorkflowCredential`] is the seam, and until it lands [`NoWorkflowCredential`]
//! says it is not there. Where it is not, the CI-file act is not offered and the chat and the
//! window are told why ([`Refused::NoWorkflowCredential`]).
//!
//! # How the proxy hands a request over
//!
//! [`recognise`] reads a request as one of these acts ([`Recognised`]); the proxy then answers the
//! chat with [`said::asked`] (a 403 with one sentence) and calls [`HeldActs::ask`] (or, for a
//! merge, [`crate::mergehold::asked`]) with what the request named. No harness output is read.
//!
//! **Tier** (ADR 0069): memory only, in the app. A relaunch holds none and the chat's next try
//! asks again. Nothing is written to the plane, a clone or the machine.

use std::sync::{Mutex, PoisonError};

use crate::forge::Kind;
use crate::harness::asks::{Answerer, HumanScope};
use crate::mergehold::{Base, HeadSha, Repo};
use crate::secrets::keyring::Secret;

mod ci;
mod holds;
mod prepare;
mod recognise;
pub mod said;
mod tag;

pub use ci::{CiDiff, GITLAB_DEFAULT, GitTrees, MOST_DIFF, PushCheck, check_push};
pub use holds::{Asked, HeldActs, NotDone, Performed, Pressed, Waiting};
pub use prepare::{pinned, prepare};
pub use recognise::{Call, Recognised, Recorded, Requested, Why, recognise};
pub use tag::{InvalidTag, TagName};

/// The source a held act is listed under, and the one a rule made from an answer may never name.
pub const SOURCE: &str = "held-act";

/// The trace reason a held act is refused to the chat with: an ask is raised for it.
pub const REASON: &str = "held-act";

/// The sentence a held act is refused with where nobody is present to approve it.
pub const NOBODY_HERE: &str = "Held: nobody is here to approve it.";

/// How many presses a held act takes: the merge's two (ADR 0124, amended 2026-11-14).
pub const PRESSES: u8 = crate::mergehold::PRESSES;

/// The most held acts one chat has waiting at once: one more is not asked.
pub const AT_MOST_PER_CHAT: usize = 3;

/// The most held acts queued for a desktop that is not connected.
pub const MOST_QUEUED: usize = 16;

/// What a CI-file push was compared with.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Against {
    /// The tip the forge has of the branch.
    BranchTip,
    /// The default branch's tip, for a branch the forge has no tip for.
    DefaultBranch(Base),
}

/// **Exactly what will happen**, as the app will do it. Every field is a fact read from the forge
/// or the clone, or a name the chat gave that the app has checked. Nothing in it can move: a
/// commit is a commit, never a branch's name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Act {
    /// Create `name` at `target`.
    Tag {
        repo: Repo,
        name: TagName,
        target: HeadSha,
    },
    /// Make the release of `tag` at `target`: the tag first where `creates_tag`, then the release
    /// against it. The release request never carries a `target_commitish`.
    Release {
        repo: Repo,
        tag: TagName,
        target: HeadSha,
        creates_tag: bool,
        title: String,
        notes: String,
        draft: bool,
        prerelease: bool,
    },
    /// Push `tip` to `branch`, which changes the CI files of `diff`.
    CiPush {
        repo: Repo,
        branch: Base,
        tip: HeadSha,
        against: Against,
        base: HeadSha,
        diff: CiDiff,
    },
    /// Fork `source` (the forge's id `source_id`) into `into`, the person's own account.
    Fork {
        source: Repo,
        source_id: String,
        into: String,
    },
}

impl Act {
    /// The repo the act is of.
    pub fn repo(&self) -> &Repo {
        match self {
            Act::Tag { repo, .. } | Act::Release { repo, .. } | Act::CiPush { repo, .. } => repo,
            Act::Fork { source, .. } => source,
        }
    }

    /// The word for the kind of act.
    pub fn word(&self) -> &'static str {
        match self {
            Act::Tag { .. } => "tag",
            Act::Release { .. } => "release",
            Act::CiPush { .. } => "CI-file push",
            Act::Fork { .. } => "fork",
        }
    }
}

/// One line of an ask: a label and what it says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Row {
    pub label: String,
    pub value: String,
}

/// A long text an ask shows whole: a release's notes, a CI file's diff.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Body {
    pub label: String,
    pub text: String,
}

/// **What the person is shown** of an act: the window draws this and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Shown {
    /// The act's word.
    pub kind: &'static str,
    /// The repo, as the forge names it.
    pub repo: String,
    pub rows: Vec<Row>,
    pub body: Option<Body>,
}

/// **An act, and what is shown of it**, read once from the forge and the clone.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Prepared {
    pub act: Act,
    pub shown: Shown,
}

impl Prepared {
    /// **The digest of what the person is shown**: the act and each thing drawn of it. The window
    /// hands it back with a press, which performs only while the ask is still this.
    pub fn digest(&self) -> String {
        use sha2::Digest;
        // Serialising these types cannot fail: they hold strings, bools and lists.
        let canonical = serde_json::to_vec(self).unwrap_or_default();
        sha2::Sha256::digest(&canonical)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

/// **The trees of a clone**: the pushed commit's tree against the base's, as [`ci`] reads them.
pub trait Trees: Send + Sync {
    /// Every path whose content differs between the trees of `against` and `tip`, added, changed
    /// or removed, with a rename read as the removal and the addition it is. An object the clone
    /// lacks is an error: there is no tree to compare.
    fn changed(&self, against: &HeadSha, tip: &HeadSha) -> Result<Vec<String>, String>;
    /// The patch of `paths` between the trees.
    fn patch(&self, against: &HeadSha, tip: &HeadSha, paths: &[String]) -> Result<String, String>;
    /// Every file of the tree of `commit`, as a path.
    fn files(&self, commit: &HeadSha) -> Result<Vec<String>, String>;
    /// The text of `path` at `commit`, or `None` where the commit has no such file.
    fn read(&self, commit: &HeadSha, path: &str) -> Result<Option<String>, String>;
}

/// The person's own account on a forge, and what a fork of it made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forked {
    /// The fork's path, `account/name`.
    pub path: String,
    /// The forge's id for it.
    pub id: String,
}

/// **The forge, as a held act needs it**, always as the person: the app's own sign-in, never a
/// chat's. The reads say what an ask shows; the performs are what a press does and nothing else.
pub trait HeldForge: Send + Sync {
    /// The forge's kind.
    fn kind(&self) -> Kind;
    /// The forge's host, as the credential for it is kept.
    fn host(&self) -> String;

    // What an ask reads.

    /// The commit `rev` names in `repo` now: a commit, a branch or a tag, as the forge resolves it.
    fn rev_commit(&self, repo: &Repo, rev: &str) -> Result<HeadSha, String>;
    /// The default branch and its tip.
    fn default_branch(&self, repo: &Repo) -> Result<(Base, HeadSha), String>;
    /// The tip of `branch`, or `None` where the forge has no such branch.
    fn branch_tip(&self, repo: &Repo, branch: &Base) -> Result<Option<HeadSha>, String>;
    /// Whether `commit` is on `branch`: reachable from its tip.
    fn on_branch(&self, repo: &Repo, branch: &Base, commit: &HeadSha) -> Result<bool, String>;
    /// The commit a tag is at, or `None` where there is no such tag.
    fn tag_at(&self, repo: &Repo, name: &TagName) -> Result<Option<HeadSha>, String>;
    /// The project's own `ci_config_path` (GitLab), read now.
    fn ci_config_path(&self, repo: &Repo) -> Result<Option<String>, String>;
    /// The forge's id for `repo`: what a fork is bound to, so a repo deleted and made again under
    /// the name is not the one the person was asked about.
    fn source_id(&self, repo: &Repo) -> Result<String, String>;
    /// The person's login on this forge.
    fn account(&self) -> Result<String, String>;
    /// Whether the host and the organization allow the person to fork `repo`.
    fn may_fork(&self, repo: &Repo) -> Result<bool, String>;

    // What a press does.

    /// Creates the tag `name` at `at`. **A tag that exists is an error and is never moved.**
    fn create_tag(&self, repo: &Repo, name: &TagName, at: &HeadSha) -> Result<(), String>;
    /// Creates the release of the existing tag `tag`. **The request has no `target_commitish`**:
    /// the tag is the only thing it names.
    fn create_release(
        &self,
        repo: &Repo,
        tag: &TagName,
        title: &str,
        notes: &str,
        draft: bool,
        prerelease: bool,
    ) -> Result<(), String>;
    /// Forks `source` into the person's own account, and says what it made.
    fn make_fork(&self, source: &Repo, source_id: &str) -> Result<Forked, String>;
    /// Pushes `tip` to `branch` of `repo`: with `workflow`, the second credential, on GitHub, and
    /// with the everyday sign-in (`None`) on GitLab, which has no such scope. The app holds
    /// the commit objects of `tip`; this sends exactly them, and updates only `branch`, which
    /// where the forge has a tip for it must still be `base` (a lease), and where it has none
    /// must still not exist.
    fn push_tip(
        &self,
        repo: &Repo,
        branch: &Base,
        tip: &HeadSha,
        existing: Option<&HeadSha>,
        workflow: Option<&Secret>,
    ) -> Result<(), String>;
}

/// Why the second credential is not there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable(pub String);

/// **The second credential** (ADR 0128 D1.1): the `workflow`-scoped token made by its own sign-in,
/// used only for a held CI-file push. Slice C provides it, in the keyring; **it is never injected
/// on a chat's route**. Reading it hands back a [`Secret`], whose `Debug` never prints it.
pub trait WorkflowCredential: Send + Sync {
    /// Whether the credential for `host` can be had, without reading it: what decides whether the
    /// act is offered.
    fn available(&self, host: &str) -> Result<(), Unavailable>;
    /// The credential, for one push.
    fn secret(&self, host: &str) -> Result<Secret, Unavailable>;
}

/// The stub until slice C lands: no second sign-in has been made.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoWorkflowCredential;

impl WorkflowCredential for NoWorkflowCredential {
    fn available(&self, _: &str) -> Result<(), Unavailable> {
        Err(Unavailable(
            "pushing a CI file needs a second sign-in that grants the workflow permission, \
             and this account has not made it"
                .to_owned(),
        ))
    }

    fn secret(&self, host: &str) -> Result<Secret, Unavailable> {
        self.available(host).map(|()| Secret::new(String::new()))
    }
}

/// Everything an ask and a press read and use.
pub struct World<'a> {
    pub forge: &'a dyn HeldForge,
    pub trees: &'a dyn Trees,
    pub workflow: &'a dyn WorkflowCredential,
    pub recorded: &'a dyn Recorded,
}

/// **Why an act is not offered**: nothing is asked, and the chat and the window are told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The repo is not one the person recorded for this chat.
    NotRecorded(Repo),
    /// The tag exists, at this commit. It is never moved.
    TagExists { name: TagName, at: HeadSha },
    /// A request names a commit the forge does not resolve, in its words.
    NoSuchCommit { rev: String, why: String },
    /// The CI-file push touches no CI file, so it is not a held act.
    NoCiFiles,
    /// The second credential is not there (GitHub only).
    NoWorkflowCredential(String),
    /// A title or notes too long, or text purlis will not show, in these words.
    Text(&'static str),
    /// The push's diff could not be computed or shown.
    CiDiff(String),
    /// The host or the organization forbids the person forking the repo.
    ForkNotAllowed(Repo),
    /// The forge did not say, in its words.
    Forge(String),
    /// Nobody is here to approve it.
    Unattended { queued: bool },
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&said::refused(self))
    }
}

/// Memory shared by the store's lock.
fn locked<T>(lock: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The human scope an answer came through.
pub(crate) fn scope_of(by: Answerer) -> HumanScope {
    by.scope()
}
