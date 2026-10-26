// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The forge seam: one trait per area, each implemented once per forge (ADR 0070 §1).
//!
//! Every forge operation charter's core performs is a method here, with a GitHub body in
//! [`super::github`] and a GitLab body in [`super::gitlab`]. Nothing outside those two
//! matches on the forge to build a request. `crates/purlis-core/docs/forges.md` holds the
//! parity table, and the contract suite (`tests/forge_contract.rs`) runs every method on both
//! forges; its parity test fails when a method here has no contract case.
//!
//! No method has a default body, so a method added to an area does not compile until both
//! backends answer it.
//!
//! # One error type per discipline
//!
//! Every **strict** method fails with [`ForgeError`], the forge's or the transport's own words.
//! The two **permissive** methods, the status line's, never fail: every failure is `Ok(None)`,
//! and [`Raised`] marks only an answer shaped where Python raised (`gl-refresh`'s contract).
//! `checks_at` folds every failure into `UNKNOWN` with its reason, because a check that could
//! not be read is one of its five values. A `ForgeError` carries its kind from ADR 0070's
//! closed set ([`super::Failure`]) beside its words: the native transport tells it from the
//! HTTP status, and a CLI refusal is `Unrecognised`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::checks::{Annotation, Checks, FailedCheck};
use super::cli::Cli;
use super::pr::{
    AutoMerge, Gate, Image, MergeAs, MergedAt, Opened, Pinned, Pr, Ready, Request, State,
};
use super::transport::{Call, NoAnswer, Reply, Transport};
use super::{Forge, ForgeError, Kind, Raised};

/// Who a forge call is made for (ADR 0070 §4). It rides on every call, reads included.
///
/// It names the surface that asked, whether a person is waiting, **who** is asking (a human,
/// or a chat by its id) and, once FW-1's sign-in exists, the account. The credential a call
/// gets is decided from this and the account alone ([`super::route::route`]).
///
/// **Its fields are private, and nothing builds one from data.** A `Caller` is made by the code
/// that knows who asked: [`Caller::window`] by the window, [`Caller::command`] by a `charter`
/// command, [`Caller::chat`] and [`Caller::mcp`] by the host for a chat's own connection. It
/// has no `Deserialize`, so an MCP tool call's arguments can never name its principal (ADR
/// 0070 §4). And it grants nothing by itself: the sign-in token is reachable only through a
/// [`super::route::Resolver`] the host was given a sign-in for, and a chat or MCP `Caller`
/// never resolves to it however the resolver was built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    surface: Surface,
    priority: Priority,
    principal: Principal,
    account: Option<Account>,
}

/// The surface a call was made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
    /// The app's window.
    Window,
    /// A `charter` command.
    Command,
    /// An MCP tool a chat called, served on the chat's own connection.
    Mcp,
    /// A trigger: something that fired with nobody at the keyboard.
    Trigger,
}

impl Surface {
    /// Every surface. A `match`, so a new surface does not compile until it is listed, and
    /// the routing tests that walk this list cover it.
    pub fn all() -> [Surface; 4] {
        let every = [
            Surface::Window,
            Surface::Command,
            Surface::Mcp,
            Surface::Trigger,
        ];
        for surface in every {
            match surface {
                Surface::Window | Surface::Command | Surface::Mcp | Surface::Trigger => {}
            }
        }
        every
    }
}

/// Whether a person is waiting on the answer. The request budget holds a background call back
/// once its account's hour is spent, and never a person's foreground one; a chat's or an MCP
/// call is admitted as background whatever it carries ([`super::budget::admission`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Foreground,
    Background,
}

/// Who a call is made by.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Principal {
    /// A person, through the window or a command they typed.
    Human,
    /// A chat, by its id. Never resolves to the human's sign-in token (FI3).
    Chat(String),
}

/// One forge sign-in: a kind, a host and a login (FI2). A repo is bound to one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Account {
    pub kind: Kind,
    pub host: String,
    pub login: String,
}

impl Account {
    /// A name for this account that is safe as one path segment: `github-github.com-octocat`.
    pub fn key(&self) -> String {
        let clean = |s: &str| {
            s.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        };
        format!(
            "{}-{}-{}",
            self.kind.word(),
            clean(&self.host),
            clean(&self.login)
        )
    }
}

impl Caller {
    /// A `charter` command a person is waiting on. Always the CLI transport (ADR 0070 ruling 5).
    pub fn command() -> Caller {
        Caller {
            surface: Surface::Command,
            priority: Priority::Foreground,
            principal: Principal::Human,
            account: None,
        }
    }

    /// The window, with a person waiting.
    pub fn window() -> Caller {
        Caller {
            surface: Surface::Window,
            ..Caller::command()
        }
    }

    /// A trigger that fired with nobody at the keyboard, on a human's behalf. It never resolves
    /// to the sign-in token: nobody is there to have asked for it.
    pub fn trigger() -> Caller {
        Caller {
            surface: Surface::Trigger,
            priority: Priority::Background,
            ..Caller::command()
        }
    }

    /// A chat, by its id, running a `charter` command.
    pub fn chat(id: &str) -> Caller {
        Caller {
            principal: Principal::Chat(id.to_string()),
            ..Caller::command()
        }
    }

    /// An MCP tool chat `id` called. There is no human MCP caller: the host serves MCP on a
    /// chat's own connection only.
    pub fn mcp(id: &str) -> Caller {
        Caller {
            surface: Surface::Mcp,
            ..Caller::chat(id)
        }
    }

    /// The same caller, with nobody waiting: a refresh.
    pub fn background(self) -> Caller {
        Caller {
            priority: Priority::Background,
            ..self
        }
    }

    /// The same caller, asking as `account`.
    pub fn as_account(self, account: Account) -> Caller {
        Caller {
            account: Some(account),
            ..self
        }
    }

    pub fn surface(&self) -> Surface {
        self.surface
    }

    pub fn priority(&self) -> Priority {
        self.priority
    }

    pub fn principal(&self) -> &Principal {
        &self.principal
    }

    pub fn account(&self) -> Option<&Account> {
        self.account.as_ref()
    }

    /// Whether a chat is asking, by its principal or its surface.
    pub fn is_a_chat(&self) -> bool {
        matches!(self.principal, Principal::Chat(_)) || self.surface == Surface::Mcp
    }
}

/// Repositories: what an owner has, what an account reaches, and what a repo holds.
pub trait Repos {
    /// Every repo under `owner`, as the owner exposes them. Strict: a failure is an error,
    /// never an empty list.
    fn owned(&self, caller: &Caller, owner: &Owner) -> Result<Vec<RepoRecord>, ForgeError>;

    /// Every repo under `owner` that the account reaches, private ones included (ADR 0055).
    /// Strict.
    fn reachable(&self, caller: &Caller, owner: &Owner) -> Result<Vec<RepoRecord>, ForgeError>;

    /// The top-level file names of `repo` at `git_ref`, or its default branch. Strict.
    fn top_level(
        &self,
        caller: &Caller,
        repo: &RepoRecord,
        git_ref: Option<&str>,
    ) -> Result<Vec<String>, ForgeError>;

    /// Whether `repo` is public, whether this account can open an issue there (FI14), and
    /// whether the forge refuses a push that carries a secret (SQ-8). Read before anything is
    /// sent to it: `charter ws todo promote` names the first two first (ADR 0088 §5), and
    /// `charter doctor` names the first and the last for the project's own remote.
    /// Both forges address it by its `path_with_namespace`, so a record the inventory read,
    /// which holds no forge id, is enough. Strict.
    fn about(&self, caller: &Caller, repo: &RepoRecord) -> Result<About, ForgeError>;
}

/// Making a repo for a project the person creates on their forge, and taking back the one a run
/// made when the run is called off or fails (#1819, spec #1810).
///
/// **Only the repo this run made can be deleted.** [`MadeRepo`] has private fields and no
/// `Deserialize`, so the only way to one is [`Making::create_repo`]'s answer, and
/// [`Making::delete_created_repo`] takes nothing else: no name, no path, no id typed by anyone.
/// Each forge's body also checks the forge still holds that same repo, by the id the forge gave
/// it, before it deletes anything.
pub trait Making {
    /// Who this account can make a repo under: itself first, then each organisation (GitHub) or
    /// group (GitLab) it belongs to, in the forge's order. Strict.
    fn owners_to_create_under(&self, caller: &Caller) -> Result<Vec<RepoOwner>, ForgeError>;

    /// Make an empty repo: no README, no licence, no first commit, so the project's own first
    /// commit is pushed onto nothing. Strict: a name the owner already has is
    /// [`super::Failure::Conflict`].
    fn create_repo(&self, caller: &Caller, new: &NewRepo) -> Result<MadeRepo, ForgeError>;

    /// Delete `made`, the repo [`Making::create_repo`] answered in this run, once the forge
    /// shows it still holds that same repo. A repo already gone is `Ok`. Strict.
    fn delete_created_repo(&self, caller: &Caller, made: &MadeRepo) -> Result<(), ForgeError>;
}

/// Whose a new repo is: the account's own, or one of its organisations' or groups'.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoOwner {
    /// The account's login, the organisation's login, or the group's full path (`grp/sub`).
    pub name: String,
    /// What the forge calls it for a person: the account's or the group's display name, else
    /// `name`.
    pub title: String,
    pub kind: OwnerKind,
    /// The forge's own id for the owner: GitLab makes a repo in a group by its namespace id.
    pub id: Option<ForgeRef>,
}

/// Whether a [`RepoOwner`] is the account itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerKind {
    /// The signed-in account.
    You,
    /// A GitHub organisation, or a GitLab group or subgroup.
    Organisation,
}

/// A repo to make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRepo {
    pub owner: RepoOwner,
    /// Its name, and the last part of its path.
    pub name: String,
    /// Who can read it. Only [`Visibility::Private`] and [`Visibility::Public`] are asked for.
    pub visibility: Visibility,
}

/// The repo a [`Making::create_repo`] made, as the forge answered: the only thing
/// [`Making::delete_created_repo`] takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MadeRepo {
    id: ForgeRef,
    path: String,
    web_url: String,
    clone_url: String,
}

impl MadeRepo {
    /// The made repo, from its forge's answer: only a backend builds one.
    pub(super) fn answered(id: ForgeRef, path: String, web_url: String, clone_url: String) -> Self {
        MadeRepo {
            id,
            path,
            web_url,
            clone_url,
        }
    }

    /// The forge's own id for it.
    pub fn id(&self) -> &ForgeRef {
        &self.id
    }

    /// `owner/name`, or the full namespace path on GitLab.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Its page on the forge.
    pub fn web_url(&self) -> &str {
        &self.web_url
    }

    /// Its HTTPS clone URL: where the first commit is pushed.
    pub fn clone_url(&self) -> &str {
        &self.clone_url
    }

    /// A made repo for a test that never asks a forge.
    #[cfg(any(test, feature = "fenced"))]
    pub fn for_a_test(id: &str, path: &str, clone_url: &str) -> Self {
        MadeRepo::answered(
            ForgeRef(id.to_string()),
            path.to_string(),
            format!("https://forge.invalid/{path}"),
            clone_url.to_string(),
        )
    }
}

/// What [`Repos::about`] answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct About {
    pub visibility: Visibility,
    pub issues: Issues,
    pub push_protection: PushProtection,
    /// Whether the account may push to the repo: GitHub's `permissions.push`, a GitLab access
    /// level of Developer or above. `None` where the answer does not say.
    pub can_push: Option<bool>,
    /// Whether opening one of the repo's uploads needs a sign-in that can read the repo:
    /// GitLab's `enforce_auth_checks_on_uploads`. `None` where the forge does not say, and
    /// always on GitHub, which keeps no uploads of a repo's own (#2026).
    pub uploads_need_sign_in: Option<bool>,
}

/// Whether the forge itself refuses a push that carries a secret: GitHub's push protection,
/// GitLab's secret push protection. Each forge shows the setting only to an account that may
/// change it, so an answer without it is `Unknown`, never `Off`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushProtection {
    On,
    Off,
    Unknown,
}

impl PushProtection {
    /// Read off a forge's on/off answer, `None` being silence.
    pub fn of(on: Option<bool>) -> PushProtection {
        match on {
            Some(true) => PushProtection::On,
            Some(false) => PushProtection::Off,
            None => PushProtection::Unknown,
        }
    }
}

/// Who can read a repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// Everyone.
    Public,
    /// Everyone signed in to the instance: GitLab's `internal`, GitHub Enterprise's.
    Internal,
    /// Only those given access.
    Private,
}

impl Visibility {
    /// The forge's own word for it.
    pub fn word(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Internal => "internal",
            Visibility::Private => "private",
        }
    }

    /// Who can read what is sent there, as charter says it before it sends.
    pub fn readers(self) -> &'static str {
        match self {
            Visibility::Public => "PUBLIC: everyone can read what is sent",
            Visibility::Internal => "internal: everyone signed in to the instance can read it",
            Visibility::Private => "private",
        }
    }

    /// Read off a forge's `visibility` field. A word charter does not know is `None`, never a
    /// guess: a promote must not tell the operator a repo is private when it is not.
    pub fn parse(word: &str) -> Option<Visibility> {
        [
            Visibility::Public,
            Visibility::Internal,
            Visibility::Private,
        ]
        .into_iter()
        .find(|v| v.word() == word)
    }
}

/// Whether this account can open an issue in a repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issues {
    /// It can.
    Open,
    /// The repo has its issues turned off.
    Off,
    /// The repo takes issues, but not from this account.
    NoRight,
    /// The repo is archived: read-only, so it takes no issue at all.
    Archived,
}

/// The FI6 layer-2 label for workspace `ws`, as forge `kind` spells it: `ws:<name>` on GitHub,
/// the scoped `charter::ws::<name>` on GitLab, so a second workspace's label replaces it there.
pub fn workspace_label(kind: Kind, ws: &str) -> String {
    match kind {
        Kind::GitHub => format!("ws:{ws}"),
        Kind::GitLab => format!("charter::ws::{ws}"),
    }
}

/// Work items: issues, epics and the rest of FI4 (ADR 0070 §1, ADR 0088 §6). FW-5 brings the
/// area with its first method; FW-6a and FW-6b map each forge's reads onto the neutral model.
pub trait WorkItems {
    /// Issue `number` of the repo at `path` (GitLab's `iid`), as `caller`, in the neutral model:
    /// its fields, how it stands to other items, and the tracker's own boards that hold it.
    /// What a forge lacks is left empty, and its capability says so ([`Capability`]). Strict.
    fn read(
        &self,
        caller: &Caller,
        path: &str,
        number: u64,
    ) -> Result<crate::work::WorkItem, ForgeError>;

    /// Open an issue in the repo at `path`, as `caller`. The answer names it by its tracker key,
    /// with the forge's own id beside it. Strict.
    fn create(
        &self,
        caller: &Caller,
        path: &str,
        new: &NewWorkItem,
    ) -> Result<crate::work::WorkItem, ForgeError>;

    /// The issues of the repo at `path` that match `words`, open and closed alike, at most
    /// `limit` of them, in the forge's own order: what may already cover an issue about to be
    /// filed (`purlis report`, #806). Never a pull or merge request. Each is mapped as
    /// [`WorkItems::create`] maps its answer. Strict.
    fn search(
        &self,
        caller: &Caller,
        path: &str,
        words: &str,
        limit: u8,
    ) -> Result<Vec<crate::work::WorkItem>, ForgeError>;
}

/// A new issue, in the neutral model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewWorkItem {
    pub title: String,
    pub body: String,
    /// The workspace whose FI6 layer-2 label the issue carries, when that label is on for the
    /// repo, spelled by [`workspace_label`].
    pub workspace_label: Option<String>,
}

/// Pull requests, merge requests on GitLab: opening, reading, merging and their checks.
pub trait Requests {
    /// Open a request from `head` into `base` in the repo at `path`, or update charter's own
    /// open one (ADR 0051).
    fn open_or_update(
        &self,
        caller: &Caller,
        path: &str,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<Opened, ForgeError>;

    /// Where `pr` stands: open, merged or closed.
    fn state(&self, caller: &Caller, path: &str, pr: &Pr) -> Result<State, ForgeError>;

    /// The description `pr` holds, whole: GitHub's `body`, GitLab's `description`. A request
    /// with none is the empty string (ADR 0060: `charter change push` splices its block in).
    fn body(&self, caller: &Caller, path: &str, pr: &Pr) -> Result<String, ForgeError>;

    /// Replace `pr`'s description with `body`, and nothing else about it: not its title, its
    /// base or its state (ADR 0060).
    fn set_body(&self, caller: &Caller, path: &str, pr: &Pr, body: &str) -> Result<(), ForgeError>;

    /// The newest request whose head is `branch`, in any state (ADR 0060).
    fn by_head(
        &self,
        caller: &Caller,
        path: &str,
        branch: &str,
    ) -> Result<Option<Request>, ForgeError>;

    /// Ask the forge to merge `pr` once its checks pass, only at `head_sha` (ADR 0051).
    fn request_auto_merge(
        &self,
        caller: &Caller,
        path: &str,
        pr: &Pr,
        head_sha: &str,
    ) -> Result<AutoMerge, ForgeError>;

    /// Merge `pr` now, only when the forge says it is the request `at` names (the commit the
    /// save pushed, from the save's own branch, into its configured target), by the repo's own
    /// method, when the forge says it may merge and `gate` lets it through: how every `pr-merge` save's request is merged
    /// ([`Fallback::MergeWhenReady`], ADR 0051, amended 2026-10-13).
    ///
    /// Under [`Gate::Checks`] purlis reads every check at that commit itself. A failed one,
    /// required or not, is [`Ready::Red`] with its name. A running one is [`Ready::Waiting`],
    /// and so is a head with no check at all until it is `settled` **and** nothing the
    /// repository shows says a check will run on it. A conflict or a review the branch
    /// requires is [`Ready::Refused`]. Nothing is merged in any of these. A branch that lands
    /// through a merge queue or train is put in it once the checks pass ([`Ready::Queued`]).
    ///
    /// Under [`Gate::Person`] the checks are passed over and nothing else is.
    fn merge_when_ready(
        &self,
        caller: &Caller,
        path: &str,
        pr: &Pr,
        at: &Pinned,
        gate: Gate,
    ) -> Result<Ready, ForgeError>;

    /// Whether `pr` lands through its target branch's queue: GitHub's merge queue, GitLab's
    /// merge train (ADR 0060, ruling Q16).
    fn lands_through_queue(&self, caller: &Caller, path: &str, pr: &Pr)
    -> Result<bool, ForgeError>;

    /// Merge `pr` now, as `how` says, and only while its head is `head_sha`: the forge's own
    /// guard refuses a head that moved (ADR 0060 D3). It never asks the forge to merge later;
    /// [`MergedAt::Later`] is a forge that did so anyway and could not be undone.
    fn merge_at(
        &self,
        caller: &Caller,
        path: &str,
        pr: &Pr,
        head_sha: &str,
        how: &MergeAs,
    ) -> Result<MergedAt, ForgeError>;

    /// Put `pr` in its target branch's queue, only while its head is `head_sha` (ruling Q16).
    /// `Ok` once the forge's answer names the entry.
    fn enqueue_at(
        &self,
        caller: &Caller,
        path: &str,
        pr: &Pr,
        head_sha: &str,
        how: &MergeAs,
    ) -> Result<(), ForgeError>;

    /// The checks at exactly `sha` (ADR 0060). `request` is the request's number: GitLab reads
    /// a merge request's own pipelines, and GitHub reads a commit's checks with no request at
    /// all, so its body does not use it.
    fn checks_at(&self, caller: &Caller, path: &str, sha: &str, request: u64) -> Checks;

    /// What one failed check found, the first few ([`super::checks::ANNOTATIONS_READ`]): a
    /// GitHub check run's annotations, failures first, or the failed jobs of a GitLab
    /// pipeline. Read by [`FailedCheck::run`]; a check with none (a commit status) has no
    /// findings to read, and answers the empty list without asking. Every field is whoever
    /// wrote the check's, bounded and made printable ([`super::checks::Annotation`]).
    fn annotations(
        &self,
        caller: &Caller,
        path: &str,
        check: &FailedCheck,
    ) -> Result<Vec<Annotation>, ForgeError>;

    /// The open request on `branch`, as the forge's own field holds it. Permissive: every
    /// failure is `Ok(None)`, and a shape Python raised on is `Err(Raised)` (`gl-refresh`).
    fn open_on_branch(
        &self,
        caller: &Caller,
        path: &str,
        branch: &str,
    ) -> Result<Option<Value>, Raised>;

    /// The branch's last CI result as one of `cistate::CI_STATES`. Permissive, as above.
    fn ci_word(&self, caller: &Caller, path: &str, branch: &str) -> Result<Option<String>, Raised>;

    /// Keep `images` where `pr`'s page can show them, and answer the address of each, in
    /// order (browser proof, #2026). GitHub has no API for a comment's attachments, so a
    /// commit of only the pictures goes on [`super::pr::proof_branch`], whose first commit has
    /// no parent; GitLab keeps each as one of the project's uploads. Where either is is
    /// [`super::pr::proof_kept_at`], which the person is shown first.
    fn keep_images(
        &self,
        caller: &Caller,
        path: &str,
        pr: &Pr,
        images: &[Image],
    ) -> Result<Vec<String>, ForgeError>;

    /// Post `body` as a comment on `pr`, and answer the comment's page (#2026).
    fn comment(
        &self,
        caller: &Caller,
        path: &str,
        pr: &Pr,
        body: &str,
    ) -> Result<String, ForgeError>;
}

/// What a forge can do, for one reach (ADR 0070 §2).
pub trait Capabilities {
    /// Whether `what` is there at `at`. Silence never reads as yes: what charter has not
    /// asked about is [`Support::Unknown`], and an unknown capability takes its fallback.
    fn support(&self, caller: &Caller, at: &Reach, what: Capability) -> Support;
}

/// What a capability question is about: the account's instance, one owner, or one repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    Instance,
    Owner(String),
    Repo(String),
}

/// A forge's own identifier for a thing (a GitHub node id, a GitLab global id), carried
/// opaquely so that a round trip never re-derives it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForgeRef(pub String);

/// Whose repos a listing asks for: a GitHub organisation or user, or a GitLab group by its
/// full path (`grp/sub`), as the plane's forge config declares it; or, for
/// [`Repos::reachable`] alone, [`Owner::every`] one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner(String, bool);

impl Owner {
    pub fn new(owner: impl Into<String>) -> Owner {
        Owner(owner.into(), false)
    }

    /// Every owner: what an account reaches anywhere, its own repos, its organisations' or
    /// groups', and those it collaborates on (the repo list, #1818). Only
    /// [`Repos::reachable`] takes it; it names no owner a listing could be asked of.
    pub fn every() -> Owner {
        Owner("every owner".to_string(), true)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `repo` is under this owner. A subgroup's repos are under its group's path, so
    /// they are held too. Case does not matter, as on both forges.
    pub fn holds(&self, repo: &RepoRecord) -> bool {
        self.1
            || repo
                .path_with_namespace
                .to_lowercase()
                .starts_with(&format!("{}/", self.0.to_lowercase()))
    }
}

/// One repo as a forge lists it, in neutral fields (ADR 0070 §1). The field names are the
/// keys the inventory writes (`inventory::record`), so a record reads the same in both.
///
/// A field the forge left out, or gave as null, empty or not a string, is empty here: `""`,
/// `[]`, or `None` for the two that can be absent. The one exception is `description`: a
/// truthy non-string there holds what Python's `str()` prints of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRecord {
    /// The forge's own id for the repo. GitLab addresses a repo's tree by it.
    pub id: Option<ForgeRef>,
    /// The repo's name in its path: GitHub's `name`, GitLab's `path` (else its `name`).
    pub name: String,
    /// `owner/name` on GitHub, the full namespace path on GitLab.
    pub path_with_namespace: String,
    pub default_branch: Option<String>,
    pub description: String,
    /// The repo's page: GitHub's `html_url`, GitLab's `web_url`.
    pub web_url: String,
    /// GitHub's `ssh_url`, GitLab's `ssh_url_to_repo`.
    pub ssh_url: String,
    pub topics: Vec<String>,
    /// The forge that listed it.
    pub forge: Kind,
}

/// One thing a forge may or may not do for one repo. Not an extension's capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    AutoMerge,
    MergeQueue,
    SubIssues,
    IssueTypes,
    Epics,
    Iterations,
    Boards,
    Dependencies,
    /// Why a closed issue was closed: completed, not planned, a duplicate.
    CloseReasons,
    /// A status of the item's own, apart from any board's: GitLab's work item status. Without
    /// it, an item's status is a board's ([`crate::work::WorkItem::status`]).
    ItemStatus,
}

impl Capability {
    /// Every capability, for a test that walks them.
    pub const ALL: [Capability; 10] = [
        Capability::AutoMerge,
        Capability::MergeQueue,
        Capability::SubIssues,
        Capability::IssueTypes,
        Capability::Epics,
        Capability::Iterations,
        Capability::Boards,
        Capability::Dependencies,
        Capability::CloseReasons,
        Capability::ItemStatus,
    ];

    /// What charter does instead when this capability is unavailable or unknown.
    pub fn fallback(self) -> Fallback {
        match self {
            // No auto-merge: purlis merges it itself once the forge's own gates allow it.
            Capability::AutoMerge => Fallback::MergeWhenReady,
            // No merge queue: auto-merge, with charter keeping the landing order (ADR 0060).
            Capability::MergeQueue => Fallback::AutoMergeInLandingOrder,
            // No sub-issues or dependencies: a link block in the body.
            Capability::SubIssues | Capability::Dependencies => Fallback::ParentLinkInBody,
            // No issue types, epics or iterations: labels and milestones (FW-6b).
            Capability::IssueTypes | Capability::Epics | Capability::Iterations => {
                Fallback::LabelsAndMilestones
            }
            // No boards, or no reason a closed item was closed: neither is shown.
            Capability::Boards | Capability::CloseReasons => Fallback::Hidden,
            // No status of its own: the item shows its board's.
            Capability::ItemStatus => Fallback::BoardStatus,
        }
    }
}

/// Whether a capability is there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Support {
    Available,
    Unavailable(Unavailable),
    Unknown(UnknownWhy),
}

/// What charter does about one capability: use it, or take a fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taken {
    Feature,
    Fallback(Fallback),
}

impl Support {
    /// What charter does about `what` given this answer. **An `Unknown` capability takes its
    /// fallback** (ADR 0070 ruling 8): only `Available` uses the feature.
    pub fn taken(&self, what: Capability) -> Taken {
        match self {
            Support::Available => Taken::Feature,
            Support::Unavailable(u) => Taken::Fallback(u.fallback),
            Support::Unknown(_) => Taken::Fallback(what.fallback()),
        }
    }
}

/// Why charter does not know whether a capability is there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnknownWhy {
    /// It depends on a repo's settings, an owner's plan or a host's version, which charter has
    /// not asked: no probe answers it, or the probe's answer leaves the question it turns on
    /// unasked.
    NotProbed,
    /// Charter asked, and got no answer it reads (#802): the forge refused the probe (a
    /// licence shown to an administrator only, a plan shown to its owners), the transport gave
    /// none (a timeout, no credential), or the answer named nothing charter knows.
    Unanswered,
}

/// A capability that is not there, why, and what charter does instead. The same value is what
/// [`Capabilities::support`] answers and what an operation fails with
/// ([`super::Failure::Unavailable`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable {
    pub what: Capability,
    pub reason: Reason,
    pub fallback: Fallback,
}

impl Unavailable {
    /// `what` is unavailable for `reason`, with its capability's own fallback.
    pub fn because(what: Capability, reason: Reason) -> Unavailable {
        Unavailable {
            what,
            reason,
            fallback: what.fallback(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    NotOnThisForge,
    NeedsTier(Tier),
    HostTooOld(String),
    NoRight(String),
    /// W7: the twin ships one release after the first forge's.
    NotYetBuilt,
    /// The repo does not allow it: its setting is off, or its plan has none, which the forge
    /// does not tell apart. In the forge's or purlis's words.
    NotAllowed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    GitHubOrganisation,
    GitLabPremium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallback {
    AutoMergeInLandingOrder,
    /// purlis merges the request itself, at the commit it pushed, once the forge says it may
    /// merge and every check there has passed: never past a failing check or a required review.
    MergeWhenReady,
    LabelsAndMilestones,
    ParentLinkInBody,
    HumanClick,
    Hidden,
    /// The status of a board that holds the item: a Projects v2 board's `Status` field, the
    /// label list of a GitLab issue board.
    BoardStatus,
}

/// Everything a forge backend answers. It grows as each area lands (ADR 0070 §1).
pub trait ForgeBackend: Repos + Making + Requests + Capabilities + WorkItems + Send + Sync {}

impl<T: Repos + Making + Requests + Capabilities + WorkItems + Send + Sync> ForgeBackend for T {}

impl Forge {
    /// This forge's backend, over the transports this process has: in the window's process, the
    /// person's purlis sign-in for a window call and the CLI for every other
    /// ([`super::signin`]); anywhere else, the CLI.
    pub fn backend(&self) -> Box<dyn ForgeBackend> {
        match super::signin::transports(self) {
            Some(transports) => self.backend_for(transports),
            None => self.backend_for(Arc::new(ChatsLoseAnImport {
                forge: self.clone(),
                cli: Arc::new(Cli::default()),
                config_root: crate::machine::config_root(),
            })),
        }
    }

    /// This forge's backend over `transport`, for every caller.
    pub fn backend_over(&self, transport: Arc<dyn Transport>) -> Box<dyn ForgeBackend> {
        self.backend_for(Arc::new(Fixed(transport)))
    }

    /// This forge's backend, each call sent through the transport `transports` resolves for
    /// that call's [`Caller`].
    ///
    /// The backend keeps its probes' answers for [`PROBE_KEPT_FOR`], each for the caller that
    /// asked it ([`Probes`]): in this process only, never written anywhere.
    pub fn backend_for(&self, transports: Arc<dyn Transports>) -> Box<dyn ForgeBackend> {
        let asker = Asker {
            forge: self.clone(),
            transports: Arc::new(Kept::over(transports, PROBE_KEPT_FOR)),
        };
        match self.kind {
            Kind::GitHub => Box::new(super::github::GitHub(asker)),
            Kind::GitLab => Box::new(super::gitlab::GitLab(asker)),
        }
    }

    /// Refuse unless this forge can be asked as someone: its CLI installed and logged in.
    pub fn check_auth(&self) -> Result<(), ForgeError> {
        Cli::default().check_auth(self)
    }
}

/// Which transport one call takes: decided from its [`Caller`], once per request, by a rule
/// that does not change between the requests of one call (ADR 0070 §5).
pub trait Transports: Send + Sync {
    fn for_caller(&self, caller: &Caller) -> Result<Arc<dyn Transport>, ForgeError>;

    /// The probe answers these transports keep, if they keep any. Only a backend's own
    /// [`Kept`] does; any other asks every probe afresh.
    fn probes(&self) -> Option<&Probes> {
        None
    }
}

/// How long a probe's answer is kept (ADR 0070 §2: kept per account, with an expiry). A
/// host's version, its licence, an owner's kind and a namespace's plan change seldom, and a
/// change shows within this long.
pub const PROBE_KEPT_FOR: Duration = Duration::from_secs(15 * 60);

/// Whose answer a probe's is: the caller, by everything its credential is decided from (its
/// surface, its principal and its account; ADR 0070 §4), the host asked and the path asked.
/// Two hosts never share an answer, and neither do two accounts or a person and a chat on one
/// host.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ProbeKey {
    host: String,
    surface: Surface,
    principal: Principal,
    account: Option<Account>,
    path: String,
}

impl ProbeKey {
    fn of(host: &str, caller: &Caller, path: &str) -> ProbeKey {
        ProbeKey {
            host: host.to_string(),
            surface: caller.surface,
            principal: caller.principal.clone(),
            account: caller.account.clone(),
            path: path.to_string(),
        }
    }
}

/// The probe answers one backend holds, each for its caller, until it expires. **In memory
/// only**: nothing here is written to disk or handed to another backend, so an answer lives no
/// longer than the backend that asked it. Only an answer is kept: a refusal or a transport that
/// could not answer is asked again next time.
pub struct Probes {
    kept_for: Duration,
    answers: Mutex<HashMap<ProbeKey, (Instant, Value)>>,
}

impl Probes {
    fn new(kept_for: Duration) -> Probes {
        Probes {
            kept_for,
            answers: Mutex::new(HashMap::new()),
        }
    }

    fn get(&self, key: &ProbeKey) -> Option<Value> {
        let mut answers = self.answers.lock().unwrap_or_else(|e| e.into_inner());
        match answers.get(key) {
            Some((at, answer)) if at.elapsed() < self.kept_for => Some(answer.clone()),
            Some(_) => {
                answers.remove(key);
                None
            }
            None => None,
        }
    }

    fn keep(&self, key: ProbeKey, answer: &Value) {
        if self.kept_for.is_zero() {
            return;
        }
        self.answers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, (Instant::now(), answer.clone()));
    }
}

/// A backend's transports and the probe answers it keeps.
pub(super) struct Kept {
    transports: Arc<dyn Transports>,
    probes: Probes,
}

impl Kept {
    /// `transports`, keeping each probe answer for `kept_for`.
    pub(super) fn over(transports: Arc<dyn Transports>, kept_for: Duration) -> Kept {
        Kept {
            transports,
            probes: Probes::new(kept_for),
        }
    }
}

impl Transports for Kept {
    fn for_caller(&self, caller: &Caller) -> Result<Arc<dyn Transport>, ForgeError> {
        self.transports.for_caller(caller)
    }

    fn probes(&self) -> Option<&Probes> {
        Some(&self.probes)
    }
}

/// One transport for every caller: the CLI today, a recording in a test.
pub(super) struct Fixed(pub(super) Arc<dyn Transport>);

impl Transports for Fixed {
    fn for_caller(&self, _caller: &Caller) -> Result<Arc<dyn Transport>, ForgeError> {
        Ok(self.0.clone())
    }
}

/// The CLI's login for every caller, but a chat on a host whose CLI login the person imported
/// into purlis (ADR 0077 §5, ADR 0070 §4, V22c): that login is the person's now, so a chat there
/// gets no forge credential from purlis. The transports of a process the host handed no sign-ins
/// to, which is every process a chat runs (`signin::transports`, whose resolver says the same).
struct ChatsLoseAnImport {
    forge: Forge,
    cli: Arc<dyn Transport>,
    /// Where the accounts are kept; `None` on a machine with no home, which has none.
    config_root: Option<std::path::PathBuf>,
}

impl Transports for ChatsLoseAnImport {
    fn for_caller(&self, caller: &Caller) -> Result<Arc<dyn Transport>, ForgeError> {
        let imported = caller.is_a_chat()
            && self.config_root.as_deref().is_some_and(|root| {
                super::accounts::imported_on(root, self.forge.kind, &self.forge.host)
            });
        if imported {
            return Err(ForgeError::of(
                super::Failure::Forbidden,
                IMPORTED_LOGIN_IS_THE_PERSONS.to_string(),
            ));
        }
        Ok(self.cli.clone())
    }
}

/// Why a chat has no forge credential on a host whose CLI login was imported: the resolver's
/// words (`route::route`), said the same in every process.
pub(super) const IMPORTED_LOGIN_IS_THE_PERSONS: &str = "this host's CLI login was imported into purlis, so it is the human's now, and a chat has no \
     forge credential of its own here yet";

/// A forge and the transports its requests take: what both backends are built on.
pub(super) struct Asker {
    pub(super) forge: Forge,
    pub(super) transports: Arc<dyn Transports>,
}

impl Asker {
    pub(super) fn kind(&self) -> Kind {
        self.forge.kind
    }

    /// Send `call` for `caller`, through the transport resolved for it. A caller refused any
    /// credential has no answer, in the resolver's words.
    pub(super) fn send(&self, caller: &Caller, call: &Call) -> Result<Reply, NoAnswer> {
        let transport = self
            .transports
            .for_caller(caller)
            .map_err(|refused| NoAnswer::Refused(refused.to_string()))?;
        transport.send(&self.forge, call)
    }

    /// Send `call` and read its answer as JSON. Any failure is an error in the forge's own
    /// words; nothing here reads a failure as "none".
    pub(super) fn ask(
        &self,
        caller: &Caller,
        call: &Call,
        doing: &str,
    ) -> Result<Value, ForgeError> {
        let kind = self.kind();
        let answer = match self.send(caller, call) {
            Ok(answer) => answer,
            Err(NoAnswer::Timeout(why)) => {
                return Err(ForgeError::transport(format!("{doing}: {why}")));
            }
            Err(no @ (NoAnswer::Missing(_) | NoAnswer::Refused(_) | NoAnswer::HeldBack { .. })) => {
                return Err(no.error(no.said().to_string()));
            }
        };
        if !answer.ok() {
            return Err(ForgeError::of(
                answer.failure(),
                format!("{doing} failed: {}", answer.said(kind)),
            ));
        }
        serde_json::from_str(&answer.out).map_err(|e| {
            ForgeError::new(format!(
                "{doing}: {} answered malformed JSON: {e}",
                kind.cli()
            ))
        })
    }

    /// Send a write and keep the forge's refusal apart from a transport that could not answer:
    /// the outer error is "no answer", the inner one is the forge's own words.
    pub(super) fn said(
        &self,
        caller: &Caller,
        call: &Call,
    ) -> Result<Result<(), String>, ForgeError> {
        match self.send(caller, call) {
            Ok(answer) if answer.ok() => Ok(Ok(())),
            Ok(answer) => Ok(Err(answer.said(self.kind()))),
            Err(no) => Err(no.error(no.said().to_string())),
        }
    }

    /// As [`Asker::said`], with a success's JSON answer: `Ok(Ok(answer))`, `Ok(Err(words))` for
    /// the forge's refusal, and `Err` for a transport that could not answer or malformed JSON.
    pub(super) fn answered(
        &self,
        caller: &Caller,
        call: &Call,
        doing: &str,
    ) -> Result<Result<Value, String>, ForgeError> {
        match self.send(caller, call) {
            Ok(answer) if answer.ok() => serde_json::from_str(&answer.out).map(Ok).map_err(|e| {
                ForgeError::new(format!(
                    "{doing}: {} answered malformed JSON: {e}",
                    self.kind().cli()
                ))
            }),
            Ok(answer) => Ok(Err(answer.said(self.kind()))),
            Err(no) => Err(no.error(no.said().to_string())),
        }
    }

    /// One strict JSON GET: a failure raises and never reads as "empty". Python's
    /// `_api_strict`.
    pub(super) fn strict(
        &self,
        caller: &Caller,
        path: &str,
        what_failed: &str,
    ) -> Result<Value, ForgeError> {
        self.found(caller, path, what_failed, |_| false)
            .map(|found| found.unwrap_or(Value::Null))
    }

    /// As [`Asker::strict`], except that a refusal `missing` recognises is `None`: the one
    /// answer a caller reads as "not here, ask elsewhere". Every other failure still raises.
    pub(super) fn found(
        &self,
        caller: &Caller,
        path: &str,
        what_failed: &str,
        missing: impl Fn(&Reply) -> bool,
    ) -> Result<Option<Value>, ForgeError> {
        let answer = match self.send(caller, &Call::get(path, super::LIST_TIMEOUT)) {
            Ok(answer) => answer,
            Err(NoAnswer::Timeout(why)) => {
                return Err(ForgeError::transport(format!(
                    "{what_failed} ({path}) {why}"
                )));
            }
            Err(no @ (NoAnswer::Missing(_) | NoAnswer::Refused(_) | NoAnswer::HeldBack { .. })) => {
                return Err(no.error(no.said().to_string()));
            }
        };
        if !answer.ok() {
            if missing(&answer) {
                return Ok(None);
            }
            return Err(ForgeError::of(
                answer.failure(),
                format!(
                    "{what_failed} failed ({path}): {}",
                    answer.said(self.kind())
                ),
            ));
        }
        super::parse(self.kind(), &answer.out, path).map(Some)
    }

    /// One FG-2 probe (#802): a JSON GET whose answer settles a capability. An answer is kept
    /// for its caller ([`Probes`]) and served from there until it expires. A refusal, no answer
    /// from the transport, or an answer that is not JSON is [`UnknownWhy::Unanswered`], and is
    /// asked again next time.
    pub(super) fn probe(&self, caller: &Caller, path: &str) -> Result<Value, UnknownWhy> {
        let key = ProbeKey::of(&self.forge.host, caller, path);
        let probes = self.transports.probes();
        if let Some(answer) = probes.and_then(|probes| probes.get(&key)) {
            return Ok(answer);
        }
        let answer = self
            .send(caller, &Call::get(path, super::STATUS_TIMEOUT))
            .map_err(|_| UnknownWhy::Unanswered)?;
        if !answer.ok() || answer.out.trim().is_empty() {
            return Err(UnknownWhy::Unanswered);
        }
        let answer: Value =
            serde_json::from_str(&answer.out).map_err(|_| UnknownWhy::Unanswered)?;
        if let Some(probes) = probes {
            probes.keep(key, &answer);
        }
        Ok(answer)
    }

    /// Best-effort JSON GET. `None` on **every** failure. Python's `_api` on both backends.
    pub(super) fn best_effort(&self, caller: &Caller, path: &str) -> Option<Value> {
        let answer = self
            .send(caller, &Call::get(path, super::STATUS_TIMEOUT))
            .ok()?;
        if !answer.ok() || answer.out.trim().is_empty() {
            return None;
        }
        serde_json::from_str(&answer.out).ok()
    }
}

#[cfg(test)]
mod probe_key_tests {
    use super::*;

    #[test]
    fn a_kept_answer_is_one_hosts_and_one_callers() {
        let probes = Probes::new(PROBE_KEPT_FOR);
        let asked = ProbeKey::of("git.example.com", &Caller::window(), "metadata");
        probes.keep(asked.clone(), &serde_json::json!({"version": "19.4.0"}));
        assert!(probes.get(&asked).is_some());
        for other in [
            ProbeKey::of("git.example.org", &Caller::window(), "metadata"),
            ProbeKey::of("git.example.com", &Caller::chat("c1"), "metadata"),
            ProbeKey::of("git.example.com", &Caller::window(), "license"),
        ] {
            assert!(probes.get(&other).is_none(), "{other:?}");
        }
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;

    /// A config home whose accounts file holds one account on github.com, signed in by `method`.
    fn accounts_with(method: &str) -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let at = super::super::accounts::path(home.path());
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(
            &at,
            serde_json::json!({
                "version": 1,
                "accounts": [{
                    "id": "01J9IMPORTED",
                    "kind": "github",
                    "host": "github.com",
                    "login": "octocat",
                    "method": method,
                    "state": "signed-in",
                }],
            })
            .to_string(),
        )
        .unwrap();
        home
    }

    fn transports(home: &tempfile::TempDir, forge: Forge) -> ChatsLoseAnImport {
        ChatsLoseAnImport {
            forge,
            cli: Arc::new(Cli::default()),
            config_root: Some(home.path().to_path_buf()),
        }
    }

    /// V22c: a CLI login the person imported is theirs, so a chat on that host loses it even in
    /// a process the host handed no sign-ins to, which is every process a chat runs.
    #[test]
    fn a_chat_loses_the_cli_login_of_a_host_whose_login_was_imported() {
        let home = accounts_with("import");
        let github = transports(&home, Forge::default_of(Kind::GitHub));
        for chat in [Caller::chat("c1"), Caller::mcp("c1")] {
            let refused = github.for_caller(&chat).err().expect("a chat is refused");
            assert_eq!(refused.failure(), &super::super::Failure::Forbidden);
            assert!(
                refused.said().contains("imported into purlis"),
                "{refused:?}"
            );
        }
        for person in [Caller::window(), Caller::command(), Caller::trigger()] {
            assert!(github.for_caller(&person).is_ok(), "{person:?}");
        }
        // Another host keeps its CLI login for a chat.
        let gitlab = transports(&home, Forge::default_of(Kind::GitLab));
        assert!(gitlab.for_caller(&Caller::chat("c1")).is_ok());
    }

    #[test]
    fn a_chat_keeps_the_cli_login_of_a_host_signed_in_with_a_token() {
        let home = accounts_with("pat");
        let github = transports(&home, Forge::default_of(Kind::GitHub));
        assert!(github.for_caller(&Caller::chat("c1")).is_ok());
    }
}
