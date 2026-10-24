// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Create a project on GitHub or GitLab, in one step** (#1819, spec #1810, ruling Q8): the
//! welcome's "Create a project" when the person picks a forge rather than "only this Mac".
//!
//! The person gives a name, an owner (themselves or one of their organisations or groups), who
//! can read it (private unless they say public) and a stack template (FR-17) or none. Then, with
//! each stage said as it runs ([`Stage`]):
//!
//! 1. **Checking**: the owner is looked up again on the forge, so the owner that reaches it is
//!    one the forge lists for this account now, never only the window's word for it.
//! 2. **Laying out**: the project is scaffolded in its folder, `~/Projects/<name>` unless the
//!    person picked another place, as `purlis init` makes one, with the template laid in.
//! 3. **Committing**: a repository with one commit, on `main`.
//! 4. **Creating the repo**: an empty repo on the forge, through the person's purlis sign-in
//!    ([`crate::forge::Making`]).
//! 5. **Pushing**: the first commit, through purlis's own git credential helper (#752), with no
//!    other helper asked.
//!
//! The window then opens the folder through the same open-and-trust question as every other
//! open (ADR 0035). Nothing here opens or trusts anything.
//!
//! **Cancel or a failure at any stage takes back what this run made, and only that**: the repo
//! it made on the forge ([`crate::forge::Making::delete_created_repo`], which takes nothing but
//! the create's own answer), then the folders it made. Nothing is ever deleted that this run did
//! not make.
//!
//! **A repo the forge would not delete is kept, empty** (dispatcher ruling, 2026-10-12): purlis
//! does not ask GitHub for `delete_repo`, which would let it delete any repo the person can, so on
//! GitHub a repo made before a failed or cancelled push usually stays. It is named, with where to
//! delete it by hand, and handed back as [`Refused::kept`] or [`Created::CalledOff`]'s `kept`. A
//! Retry of the same ask passes it as [`Plan::kept`], and the creation pushes into that repo
//! instead of making another.
//!
//! The remote is made as late as it can be: everything that can fail on this machine fails
//! before a repo exists, so the stage after it is only the push.
//!
//! **No chat reaches this.** The only caller is a command of the window's alone, which asks the
//! forge as the person in the window ([`crate::forge::Caller::window`]); the push's credential
//! helper is named only for git the window runs, and refuses in a chat's process tree.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::forge::{
    Caller, Failure, ForgeBackend, ForgeError, Kind, MadeRepo, NewRepo, OwnerKind, RepoOwner,
};
use crate::worktree::git;

/// The branch the first commit is on.
pub const BRANCH: &str = "main";

/// The longest name either forge takes for a repo's path.
const MAX_NAME: usize = 100;

/// Why `name` cannot be a new project's name, or `None` when it can: letters, digits, `.`, `-`
/// and `_`, starting with a letter or a digit, at most 100 long, and not ending in `.git` or
/// `.atom`. Both forges take every name this allows, and it is a folder name on every system.
pub fn name_refusal(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("Give the project a name.".to_string());
    }
    if name.len() > MAX_NAME {
        return Some(format!(
            "A project's name is at most {MAX_NAME} characters long."
        ));
    }
    let usable = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    let starts = name.starts_with(|c: char| c.is_ascii_alphanumeric());
    let lower = name.to_ascii_lowercase();
    if !usable || !starts || lower.ends_with(".git") || lower.ends_with(".atom") {
        return Some(
            "A project's name is letters, digits, dots, dashes and underscores, starting with a \
             letter or a digit, like my-project."
                .to_string(),
        );
    }
    None
}

/// The folder a new project goes into: `<parent>/<name>`, where `parent` is the place the person
/// picked last, or `~/Projects` (ruling Q7, as "Get a project" does).
pub fn folder_for(name: &str, parent: Option<&Path>, home: &Path) -> PathBuf {
    parent
        .map_or_else(
            || home.join(crate::getproject::DEFAULT_PARENT),
            Path::to_path_buf,
        )
        .join(name)
}

/// A stage of a creation, in the order they run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Checking,
    LayingOut,
    Committing,
    CreatingRepo,
    Pushing,
}

impl Stage {
    /// Every stage, in order: what the window lists.
    pub const ALL: [Stage; 5] = [
        Stage::Checking,
        Stage::LayingOut,
        Stage::Committing,
        Stage::CreatingRepo,
        Stage::Pushing,
    ];

    /// What the window says while this stage runs, on `kind`.
    pub fn words(self, kind: Kind) -> String {
        match self {
            Stage::Checking => format!("Checking the owner on {}", kind.display()),
            Stage::LayingOut => "Laying out the project".to_string(),
            Stage::Committing => "Making the first commit".to_string(),
            Stage::CreatingRepo => format!("Creating the repo on {}", kind.display()),
            Stage::Pushing => "Pushing the first commit".to_string(),
        }
    }
}

/// What the person asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub kind: Kind,
    /// The forge's host: the person's signed-in account's.
    pub host: String,
    /// The owner's name, as the window's picker showed it: looked up again on the forge.
    pub owner: String,
    pub name: String,
    /// Public when `false`: private unless the person said otherwise.
    pub private: bool,
    /// A project template's id (FR-17), or `None` for an empty project.
    pub template: Option<String>,
    /// Where the project goes: absolute, and not there yet or an empty folder.
    pub folder: PathBuf,
    /// A repo an earlier run of this same ask made and could not take back: this run pushes into
    /// it instead of making one. Only ever a creation's own answer ([`Refused::kept`]), never the
    /// window's word.
    pub kept: Option<MadeRepo>,
}

/// The work on this machine: what a creation does besides asking the forge. The window's is
/// [`OnThisMachine`]; a test stands in for it.
pub trait Hands {
    /// Scaffold the project at `folder`, which is there and empty, and lay `plan`'s template in.
    fn lay_out(&self, folder: &Path, plan: &Plan) -> Result<(), String>;
    /// Make `folder` a repository on [`BRANCH`] with one commit of everything in it, as
    /// `author` when the person's git names nobody.
    fn commit(&self, folder: &Path, plan: &Plan, author: &Author) -> Result<(), String>;
    /// Push the commit to `made`, as `origin`.
    fn push(&self, folder: &Path, plan: &Plan, made: &MadeRepo) -> Result<(), String>;
}

/// Who the first commit is by when the person's git names nobody: their forge account, with
/// the forge's no-reply address, so no address of theirs is published that they did not choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    pub name: String,
    pub email: String,
}

impl Author {
    /// The account `you` on `kind` at `host`, with that forge's no-reply address.
    pub fn of(kind: Kind, host: &str, you: &RepoOwner) -> Author {
        let id = you.id.as_ref().map(|id| id.0.as_str()).unwrap_or_default();
        let email = match (kind, id.is_empty()) {
            (Kind::GitHub, false) => format!("{id}+{}@users.noreply.{host}", you.name),
            (Kind::GitHub, true) => format!("{}@users.noreply.{host}", you.name),
            (Kind::GitLab, false) => format!("{id}-{}@users.noreply.{host}", you.name),
            (Kind::GitLab, true) => format!("{}@users.noreply.{host}", you.name),
        };
        Author {
            name: you.name.clone(),
            email,
        }
    }
}

/// What came of a creation that was not refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Created {
    /// The project is in this folder, its first commit pushed to this repo: open it.
    Made { folder: PathBuf, repo: MadeRepo },
    /// The person called it off. `left` names what could not be taken back, each with where to
    /// take it back by hand; it is empty when nothing was left behind. `kept` is the repo the
    /// forge would not delete, which a Retry pushes into.
    CalledOff {
        left: Vec<String>,
        kept: Option<MadeRepo>,
    },
}

/// A creation that failed: the stage it failed at, and the sentence the window says, which ends
/// with what was taken back.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{sentence}")]
pub struct Refused {
    pub stage: Stage,
    pub sentence: String,
    /// What could not be taken back (see [`Created::CalledOff`]).
    pub left: Vec<String>,
    /// The repo the forge would not delete, kept empty: a Retry of the same ask pushes into it
    /// ([`Plan::kept`]). Boxed, so a refusal stays small.
    pub kept: Option<Box<MadeRepo>>,
}

/// What a nothing-left creation that was refused or called off says after its reason.
pub const NOTHING_LEFT: &str = "Nothing was left behind.";

/// Creates the project `plan` asks for, as `caller` on `forge`, with `hands` doing the work on
/// this machine, telling `progress` as each stage starts, and stopping at the next stage once
/// `called_off` is raised. See the module doc for the stages and what is taken back.
pub fn create(
    plan: &Plan,
    forge: &dyn ForgeBackend,
    caller: &Caller,
    hands: &dyn Hands,
    called_off: &AtomicBool,
    progress: &mut dyn FnMut(Stage),
) -> Result<Created, Refused> {
    let mut run = Run {
        plan,
        forge,
        caller,
        called_off,
        folder: None,
        repo: None,
        kept: None,
    };
    match run.stages(hands, progress) {
        Ok(Some(made)) => Ok(made),
        Ok(None) => {
            let left = run.take_back();
            Ok(Created::CalledOff {
                left,
                kept: run.kept,
            })
        }
        Err((stage, why)) => {
            let called = called_off.load(Ordering::SeqCst);
            let left = run.take_back();
            if called {
                return Ok(Created::CalledOff {
                    left,
                    kept: run.kept,
                });
            }
            Err(Refused {
                stage,
                sentence: said_with(&why, &left),
                left,
                kept: run.kept.map(Box::new),
            })
        }
    }
}

/// `why`, then what was taken back.
pub(crate) fn said_with(why: &str, left: &[String]) -> String {
    let why = why.trim_end();
    let why = if why.ends_with(['.', '!', '?']) {
        why.to_string()
    } else {
        format!("{why}.")
    };
    if left.is_empty() {
        format!("{why} {NOTHING_LEFT}")
    } else {
        format!("{why} {}", left.join(" "))
    }
}

/// One creation as it runs: what it made so far, so a failure or a Cancel takes it back.
struct Run<'a> {
    plan: &'a Plan,
    forge: &'a dyn ForgeBackend,
    caller: &'a Caller,
    called_off: &'a AtomicBool,
    folder: Option<MadeFolder>,
    repo: Option<MadeRepo>,
    /// The repo the forge would not delete, once [`Run::take_back`] has tried.
    kept: Option<MadeRepo>,
}

impl Run<'_> {
    /// Every stage, in order. `Ok(None)` once the person called it off between stages.
    fn stages(
        &mut self,
        hands: &dyn Hands,
        progress: &mut dyn FnMut(Stage),
    ) -> Result<Option<Created>, (Stage, String)> {
        let plan = self.plan;
        let kind = plan.kind;

        progress(Stage::Checking);
        if let Some(why) = name_refusal(&plan.name) {
            return Err((Stage::Checking, why));
        }
        if let Some(template) = &plan.template
            && crate::template::named(template).is_none()
        {
            return Err((
                Stage::Checking,
                format!("purlis has no project template named {template}"),
            ));
        }
        let owners = self
            .forge
            .owners_to_create_under(self.caller)
            .map_err(|e| (Stage::Checking, forge_said(plan, &e, Stage::Checking)))?;
        let Some(owner) = owners
            .iter()
            .find(|o| o.name.eq_ignore_ascii_case(&plan.owner))
            .cloned()
        else {
            return Err((
                Stage::Checking,
                format!(
                    "{} does not list {} among the owners your account can make a repo under. \
                     Pick another owner, then Retry",
                    kind.display(),
                    plan.owner
                ),
            ));
        };
        let you = owners
            .iter()
            .find(|o| o.kind == OwnerKind::You)
            .cloned()
            .unwrap_or_else(|| owner.clone());
        if self.off() {
            return Ok(None);
        }

        progress(Stage::LayingOut);
        let made = MadeFolder::prepare(&plan.folder).map_err(|why| (Stage::LayingOut, why))?;
        self.folder = Some(made);
        hands
            .lay_out(&plan.folder, plan)
            .map_err(|why| (Stage::LayingOut, why))?;
        if self.off() {
            return Ok(None);
        }

        progress(Stage::Committing);
        hands
            .commit(&plan.folder, plan, &Author::of(kind, &plan.host, &you))
            .map_err(|why| (Stage::Committing, why))?;
        if self.off() {
            return Ok(None);
        }

        progress(Stage::CreatingRepo);
        let new = NewRepo {
            owner,
            name: plan.name.clone(),
            visibility: if plan.private {
                crate::forge::backend::Visibility::Private
            } else {
                crate::forge::backend::Visibility::Public
            },
        };
        // A Retry pushes into the repo an earlier run kept, rather than making another under
        // the same name, which the forge would refuse as taken.
        let repo = match &plan.kept {
            Some(kept) => kept.clone(),
            None => self.forge.create_repo(self.caller, &new).map_err(|e| {
                (
                    Stage::CreatingRepo,
                    forge_said(plan, &e, Stage::CreatingRepo),
                )
            })?,
        };
        self.repo = Some(repo.clone());
        if self.off() {
            return Ok(None);
        }

        progress(Stage::Pushing);
        hands
            .push(&plan.folder, plan, &repo)
            .map_err(|why| (Stage::Pushing, why))?;
        if self.off() {
            return Ok(None);
        }
        Ok(Some(Created::Made {
            folder: plan.folder.clone(),
            repo,
        }))
    }

    fn off(&self) -> bool {
        self.called_off.load(Ordering::SeqCst)
    }

    /// Takes back what this run made: the repo on the forge first, then the folders. What could
    /// not be taken back, each with where to do it by hand.
    fn take_back(&mut self) -> Vec<String> {
        let mut left = Vec::new();
        if let Some(repo) = self.repo.take()
            && let Err(why) = self.forge.delete_created_repo(self.caller, &repo)
        {
            left.push(repo_left(self.plan.kind, &repo, &why));
            self.kept = Some(repo);
        }
        if let Some(folder) = self.folder.take()
            && let Err(why) = folder.undo(&self.plan.folder)
        {
            left.push(why);
        }
        left
    }
}

/// What a repo the forge would not delete says: which, why, and where to delete it by hand.
pub(crate) fn repo_left(kind: Kind, repo: &MadeRepo, why: &ForgeError) -> String {
    let where_ = match kind {
        Kind::GitHub => format!(
            "{}/settings, under Danger Zone, Delete this repository",
            repo.web_url()
        ),
        Kind::GitLab => format!("{}/edit, under Advanced, Delete project", repo.web_url()),
    };
    format!(
        "purlis made {} and could not delete it ({}), so it is kept, empty: Retry pushes the \
         project into it. To remove it instead, delete it yourself at {where_}.",
        repo.web_url(),
        why.said()
    )
}

/// A forge's refusal at `stage`, as a sentence naming the fix.
pub(crate) fn forge_said(plan: &Plan, why: &ForgeError, stage: Stage) -> String {
    let forge = plan.kind.display();
    let full = format!("{}/{}", plan.owner, plan.name);
    match why.failure() {
        Failure::Conflict if stage == Stage::CreatingRepo => {
            format!("{forge} already has a repo named {full}. Pick another name, then Retry")
        }
        Failure::Auth => format!(
            "{forge} refused purlis's sign-in on {}. Sign in again in {}, then Retry",
            plan.host,
            crate::forge::signin::SIGN_IN_AT
        ),
        Failure::Forbidden if stage == Stage::CreatingRepo => format!(
            "{forge} did not let your account make a repo under {}. Ask an owner of {} for the \
             right, or pick another owner, then Retry ({})",
            plan.owner,
            plan.owner,
            why.said()
        ),
        Failure::Transport => format!(
            "purlis could not reach {}. Check your connection, then Retry ({})",
            plan.host,
            why.said()
        ),
        _ => format!("{forge} said: {}", why.said()),
    }
}

/// What a creation made before the layout ran, so a failed or called-off one can take it away:
/// the folders it made, or what is now inside the empty folder that was there.
struct MadeFolder {
    /// The highest folder this run made, which holds everything else it made; `None` when the
    /// project's folder itself was there already, empty.
    top: Option<PathBuf>,
}

impl MadeFolder {
    /// Checks `into` is a place a project can go, and makes it and the folders above it.
    fn prepare(into: &Path) -> Result<MadeFolder, String> {
        if !into.is_absolute() {
            return Err(format!(
                "{} is not a full path. Choose the folder with Choose a folder…",
                into.display()
            ));
        }
        match std::fs::symlink_metadata(into) {
            Ok(meta) => {
                let empty = meta.is_dir()
                    && std::fs::read_dir(into).is_ok_and(|mut entries| entries.next().is_none());
                if !empty {
                    return Err(format!(
                        "{} is already there and is not an empty folder. Pick another name or \
                         another folder, then Retry",
                        into.display()
                    ));
                }
                Ok(MadeFolder { top: None })
            }
            Err(_) => {
                let top = into
                    .ancestors()
                    .take_while(|folder| std::fs::symlink_metadata(folder).is_err())
                    .last()
                    .unwrap_or(into)
                    .to_path_buf();
                std::fs::create_dir_all(into).map_err(|err| {
                    format!(
                        "purlis could not make {}: {err}. Choose another folder, then Retry",
                        into.display()
                    )
                })?;
                Ok(MadeFolder { top: Some(top) })
            }
        }
    }

    /// Takes away what this run made; why it could not, naming the folder.
    fn undo(&self, into: &Path) -> Result<(), String> {
        let gone = match &self.top {
            Some(top) => remove(top),
            None => std::fs::read_dir(into).and_then(|entries| {
                entries
                    .flatten()
                    .try_for_each(|entry| remove(&entry.path()))
            }),
        };
        gone.map_err(|err| {
            format!(
                "purlis could not remove {} ({err}). Delete that folder yourself.",
                self.top.as_deref().unwrap_or(into).display()
            )
        })
    }
}

/// Removes `path`, a folder with everything in it or a file, never following a link: a link is
/// removed as itself.
fn remove(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

/// The window's [`Hands`]: `purlis init`'s scaffold and the template on this machine, the
/// window's git, and purlis's own credential helper for the push.
pub struct OnThisMachine {
    /// In a test or a scenario build: a local repository the push goes to in place of the
    /// forge's URL, with no credential.
    #[cfg(any(test, feature = "fenced"))]
    pub push_to: Option<PathBuf>,
}

impl OnThisMachine {
    /// The window's own: pushes to the repo the forge made.
    pub fn for_the_window() -> OnThisMachine {
        OnThisMachine {
            #[cfg(any(test, feature = "fenced"))]
            push_to: None,
        }
    }
}

/// The message of the first commit.
fn first_commit_message(plan: &Plan) -> String {
    format!("Start {} with purlis", plan.name)
}

/// `git <args>` in `folder`, or why it failed in git's own words.
pub(crate) fn git_in(folder: &Path, args: &[&str], what: &str) -> Result<(), String> {
    let run = git::run(folder, args, git::READ)
        .map_err(|unavailable| format!("purlis could not start git: {unavailable}"))?;
    if run.ok() {
        Ok(())
    } else {
        Err(format!("git could not {what}: {}", git_words(&run)))
    }
}

/// [`git_in`] for a push given no credential helper (a test's repository on this machine), through the network
/// entry point as every push is (#1853), with no credential at all.
pub(crate) fn pushed_to_a_folder(folder: &Path, args: &[&str], what: &str) -> Result<(), String> {
    let run = git::run_network(folder, None, args)
        .map_err(|unavailable| format!("purlis could not start git: {unavailable}"))?;
    if run.ok() {
        Ok(())
    } else {
        Err(format!("git could not {what}: {}", git_words(&run)))
    }
}

/// The last thing git said, without its `fatal: `.
pub(crate) fn git_words(run: &git::Run) -> String {
    let said = if run.err.trim().is_empty() {
        &run.out
    } else {
        &run.err
    };
    said.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.strip_prefix("fatal: ").unwrap_or(line).to_string())
        .unwrap_or_else(|| "it stopped without saying why".to_string())
}

/// Commits what is staged in `folder` with `message` (`what` names the act in a failure): as
/// the person's own git identity when their git has one, as `author` otherwise, with its
/// forge's no-reply address.
pub(crate) fn commit_as(
    folder: &Path,
    message: &str,
    author: &Author,
    what: &str,
) -> Result<(), String> {
    let named = git::run(folder, &["config", "--get", "user.email"], git::READ)
        .is_ok_and(|run| run.ok() && !run.out.trim().is_empty());
    let name = format!("user.name={}", author.name);
    let email = format!("user.email={}", author.email);
    let mut args: Vec<&str> = Vec::new();
    if !named {
        args.extend(["-c", name.as_str(), "-c", email.as_str()]);
    }
    args.extend(["commit", "-q", "-m", message]);
    git_in(folder, &args, what)
}

impl Hands for OnThisMachine {
    fn lay_out(&self, folder: &Path, plan: &Plan) -> Result<(), String> {
        let root = folder
            .canonicalize()
            .map_err(|err| format!("purlis cannot read {} ({err})", folder.display()))?;
        let host = (plan.host != plan.kind.default_host()).then(|| plan.host.clone());
        let outcome = crate::scaffold::init(
            &crate::plane::Place {
                root: root.clone(),
                is_plane: false,
            },
            &crate::scaffold::InitArgs {
                owner: plan.owner.clone(),
                host,
                ..crate::scaffold::InitArgs::for_the_app(false, None, Some(plan.kind))
            },
        );
        if outcome.code != 0 {
            return Err(crate::scaffold::Say::in_full(&outcome.said));
        }
        if let Some(id) = &plan.template {
            let template = crate::template::named(id)
                .ok_or_else(|| format!("purlis has no project template named {id}"))?;
            crate::template::apply(&root, template, None)?;
        }
        Ok(())
    }

    fn commit(&self, folder: &Path, plan: &Plan, author: &Author) -> Result<(), String> {
        git_in(
            folder,
            &["init", "-q", "-b", BRANCH],
            "make the project a repository",
        )?;
        // The folder was empty when it was checked, but a file can land in it before the add:
        // each file committed is the one name it has on disk, as a save's are
        // (`planegit::ondisk`).
        crate::planegit::ondisk::add_checked(
            folder,
            &[],
            "Not created: a file in the new project is not the one file on disk",
            || git_in(folder, &["add", "-A"], "add the project's files"),
        )?;
        commit_as(
            folder,
            &first_commit_message(plan),
            author,
            "make the first commit",
        )
    }

    fn push(&self, folder: &Path, plan: &Plan, made: &MadeRepo) -> Result<(), String> {
        #[cfg(any(test, feature = "fenced"))]
        if let Some(to) = &self.push_to {
            let to = to.display().to_string();
            git_in(folder, &["remote", "add", "origin", &to], "name the remote")?;
            return pushed_to_a_folder(
                folder,
                &["push", "-q", "--set-upstream", "origin", BRANCH],
                "push the first commit",
            );
        }
        // purlis's own helper for the account on this host, and no other (#752): without a
        // sign-in, nothing is pushed rather than another helper asked.
        let helper = crate::forge::gitcredential::for_the_window(&plan.host).ok_or_else(|| {
            format!(
                "purlis has no sign-in to {} to push with. Sign in in {}, then Retry",
                plan.host,
                crate::forge::signin::SIGN_IN_AT
            )
        })?;
        git_in(
            folder,
            &["remote", "add", "origin", made.clone_url()],
            "name the remote",
        )?;
        let run = git::run_network(
            folder,
            Some(&helper),
            &["push", "-q", "--set-upstream", "origin", BRANCH],
        )
        .map_err(|unavailable| format!("purlis could not start git: {unavailable}"))?;
        if run.ok() {
            // The remote the person made, recorded as this project's: an automatic save pushes
            // to it with purlis's helper, and to no other (#752's push record).
            crate::forge::gitcredential::record_push_remote(folder, made.clone_url());
            Ok(())
        } else {
            Err(format!(
                "git could not push the first commit to {}: {}",
                made.web_url(),
                git_words(&run)
            ))
        }
    }
}

#[cfg(test)]
mod tests;
