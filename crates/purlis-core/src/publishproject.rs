// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Publish a project that lives only on this machine to GitHub or GitLab** (#1820, spec #1810,
//! ruling Q9): the action beside the window's "This project lives only on this machine".
//!
//! A project lives only here when its folder is no repository yet, or is the top of one that
//! names no remote ([`lives_only_here`]): the project purlis keeps on this machine, or one made
//! with "Only this Mac". Nothing of it is anywhere else, so nothing backs it up and nobody else
//! can open it.
//!
//! Publishing asks what creating on a forge asks (#1819's form): an owner, a name and who can
//! read it, private unless the person says public. Then, with each stage said as it runs
//! ([`STAGES`], in [`words`]):
//!
//! 1. **Checking**: the folder still lives only here, purlis may write it, and the owner is
//!    looked up again on the forge, as a creation does.
//! 2. **Committing**: the folder becomes a repository on [`BRANCH`] when it is not one, and what
//!    is not committed yet is committed, as the person's own git identity or their forge
//!    account's.
//! 3. **Creating the repo**: an empty repo on the forge, through the person's purlis sign-in.
//! 4. **Pushing**: the project's history, scanned first for anything credential-shaped, through
//!    purlis's own git credential helper (#752); the repo becomes the project's `origin`, and
//!    the push record holds it as the remote the project's saves push to.
//!
//! **Cancel or a failure takes back what this run made, and only that**: the repo it made on
//! the forge, the `origin` it named, and the repository when this run made the folder one.
//! A commit of the person's own work in a repository that was already there is kept: it is
//! their files, as a save would commit them, and nothing outside this machine. A repo the forge
//! would not delete is kept, empty, and named, and a Retry pushes into it, as a creation's does
//! ([`crate::createproject`]).
//!
//! **No chat reaches this**: its only caller is a command of the window's alone, which asks the
//! forge as the person in the window, and the push's helper is named only for git the window
//! runs.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::createproject::{
    Author, BRANCH, Created, Plan, Refused, Stage, commit_as, forge_said, git_in, git_words,
    name_refusal, repo_left, said_with,
};
use crate::forge::{Caller, ForgeBackend, Kind, MadeRepo, NewRepo, OwnerKind};
use crate::worktree::git;

/// The stages a publish runs, in order: what the window lists.
pub const STAGES: [Stage; 4] = [
    Stage::Checking,
    Stage::Committing,
    Stage::CreatingRepo,
    Stage::Pushing,
];

/// What the window says while `stage` of a publish runs, on `kind`.
pub fn words(stage: Stage, kind: Kind) -> String {
    match stage {
        Stage::Committing => "Committing what is not committed yet".to_string(),
        Stage::Pushing => "Pushing the project's history".to_string(),
        other => other.words(kind),
    }
}

/// The message of the commit a publish makes of what was not committed yet.
pub fn commit_message(name: &str) -> String {
    format!("Publish {name} with purlis")
}

/// What a publish says before the paths where a file it would commit is not the one file on
/// disk ([`crate::planegit::ondisk`]).
const NOT_ONE_FILE: &str = "Not published: a file in the project is not the one file on disk";

/// The folder `root` as git sees it: the top of the repository it is in, or `None` outside one.
fn top_of(root: &Path) -> Option<PathBuf> {
    let run = git::run(root, &["rev-parse", "--show-toplevel"], git::READ).ok()?;
    if !run.ok() {
        return None;
    }
    let top = run.line().trim().to_string();
    (!top.is_empty()).then(|| PathBuf::from(top))
}

/// Whether `root` is the top of a repository of its own.
fn is_its_own_repo(root: &Path) -> bool {
    let (Some(top), Ok(root)) = (top_of(root), root.canonicalize()) else {
        return false;
    };
    top.canonicalize().is_ok_and(|top| top == root)
}

/// **Whether the project at `root` lives only on this machine**: its folder is no repository
/// yet, or the top of one that names no remote. A folder inside another repository is that
/// repository's, which is not this project's to publish, so it is not offered.
pub fn lives_only_here(root: &Path) -> bool {
    if top_of(root).is_none() {
        return root.is_dir();
    }
    if !is_its_own_repo(root) {
        return false;
    }
    git::run(root, &["remote"], git::READ).is_ok_and(|run| run.ok() && run.out.trim().is_empty())
}

/// The name Publish suggests for the repo of the project at `root`: its folder's name, with
/// what a repo's name cannot hold made a dash, or `my-project` when nothing usable is left.
pub fn suggested_name(root: &Path) -> String {
    let folder = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let usable: String = folder
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let usable = usable
        .trim_start_matches(|c: char| !c.is_ascii_alphanumeric())
        .chars()
        .take(100)
        .collect::<String>();
    if name_refusal(&usable).is_none() {
        usable
    } else {
        "my-project".to_string()
    }
}

/// The work on this machine a publish does besides asking the forge. The window's is
/// [`OnThisMachine`]; a test stands in for it.
pub trait Hands {
    /// Makes `root` a repository on [`BRANCH`] when it is not one, and commits what is not
    /// committed yet. Answers whether it made the repository.
    fn commit(&self, root: &Path, plan: &Plan, author: &Author) -> Result<bool, String>;
    /// Names `made` as `origin` and pushes the branch the project is on there, recording it as
    /// the project's remote.
    fn push(&self, root: &Path, plan: &Plan, made: &MadeRepo) -> Result<(), String>;
    /// Takes back what [`Hands::commit`] and [`Hands::push`] did: `origin` when it names
    /// `named`, and the repository itself when `made_repo`.
    fn take_back(
        &self,
        root: &Path,
        made_repo: bool,
        named: Option<&MadeRepo>,
    ) -> Result<(), String>;
}

/// Publishes the project at `plan.folder` as `plan` asks, as `caller` on `forge`, with `hands`
/// doing the work on this machine, telling `progress` as each stage starts, and stopping at the
/// next stage once `called_off` is raised. See the module doc.
pub fn publish(
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
        hands,
        called_off,
        made_repo: false,
        pushing: false,
        repo: None,
        kept: None,
    };
    match run.stages(progress) {
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

/// One publish as it runs: what it made so far, so a failure or a Cancel takes it back.
struct Run<'a> {
    plan: &'a Plan,
    forge: &'a dyn ForgeBackend,
    caller: &'a Caller,
    hands: &'a dyn Hands,
    called_off: &'a AtomicBool,
    /// Whether this run made the folder a repository.
    made_repo: bool,
    /// Whether the push started, so `origin` may name the repo.
    pushing: bool,
    repo: Option<MadeRepo>,
    kept: Option<MadeRepo>,
}

impl Run<'_> {
    fn off(&self) -> bool {
        self.called_off.load(Ordering::SeqCst)
    }

    fn stages(
        &mut self,
        progress: &mut dyn FnMut(Stage),
    ) -> Result<Option<Created>, (Stage, String)> {
        let plan = self.plan;
        let kind = plan.kind;
        let root = &plan.folder;

        progress(Stage::Checking);
        if let Some(why) = name_refusal(&plan.name) {
            return Err((Stage::Checking, why));
        }
        if !lives_only_here(root) {
            return Err((
                Stage::Checking,
                format!(
                    "{} is no longer a project that lives only on this machine: it has a remote \
                     already, or is inside another repository. Nothing was published",
                    root.display()
                ),
            ));
        }
        if let Some(why) = crate::compat::read_only(root) {
            return Err((
                Stage::Checking,
                format!("purlis opens this project read-only, so it publishes nothing: {why}"),
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

        progress(Stage::Committing);
        let author = Author::of(kind, &plan.host, &you);
        // Set before the commit, so a commit that fails after it started the repository still has
        // that repository taken back; the commit's own answer replaces it.
        self.made_repo = !is_its_own_repo(root);
        self.hands
            .commit(root, plan, &author)
            .map(|made| self.made_repo = made)
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
        self.pushing = true;
        self.hands
            .push(root, plan, &repo)
            .map_err(|why| (Stage::Pushing, why))?;
        if self.off() {
            return Ok(None);
        }
        Ok(Some(Created::Made {
            folder: root.clone(),
            repo,
        }))
    }

    /// Takes back what this run made: the repo on the forge first, then what it did on this
    /// machine. What could not be taken back, each with where to do it by hand.
    fn take_back(&mut self) -> Vec<String> {
        let mut left = Vec::new();
        let named = self.repo.clone().filter(|_| self.pushing);
        if let Some(repo) = self.repo.take()
            && let Err(why) = self.forge.delete_created_repo(self.caller, &repo)
        {
            left.push(repo_left(self.plan.kind, &repo, &why));
            self.kept = Some(repo);
        }
        if (self.made_repo || named.is_some())
            && let Err(why) =
                self.hands
                    .take_back(&self.plan.folder, self.made_repo, named.as_ref())
        {
            left.push(why);
        }
        left
    }
}

/// The window's [`Hands`]: the window's git, purlis's own credential helper for the push, and
/// the push record.
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

    /// The credential helper the push gets: purlis's own for the account on the project's host,
    /// and no other (#752); `None` for a test's local repository, which asks for none. Without a
    /// sign-in nothing is pushed, rather than another helper asked.
    fn helper(&self, plan: &Plan) -> Result<Option<String>, String> {
        #[cfg(any(test, feature = "fenced"))]
        if self.push_to.is_some() {
            return Ok(None);
        }
        crate::forge::gitcredential::for_the_window(&plan.host)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "purlis has no sign-in to {} to push with. Sign in in {}, then Retry",
                    plan.host,
                    crate::forge::signin::SIGN_IN_AT
                )
            })
    }

    /// Where the push goes: the stand-in's repository in a test, else the repo's HTTPS URL.
    fn push_url(&self, made: &MadeRepo) -> String {
        #[cfg(any(test, feature = "fenced"))]
        if let Some(to) = &self.push_to {
            return to.display().to_string();
        }
        made.clone_url().to_string()
    }
}

/// The branch the project is on, or why it cannot be pushed.
fn branch_of(root: &Path) -> Result<String, String> {
    let run = git::run(
        root,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        git::READ,
    )
    .map_err(|unavailable| format!("purlis could not start git: {unavailable}"))?;
    let branch = run.line().trim().to_string();
    if !run.ok() || branch.is_empty() {
        return Err(
            "The project is on no branch (a detached HEAD), so purlis cannot tell which history \
             to publish. Check out a branch, then Retry"
                .to_string(),
        );
    }
    Ok(branch)
}

/// **What the push would publish, scanned before it is sent**: the whole history of `branch`,
/// since the remote has nothing yet. The pushes purlis makes run no hooks, so this is the scan
/// a `pre-push` would have made. It refuses on a credential only, as a save's push does.
fn scanned(root: &Path, head: &str, branch: &str, url: &str) -> Result<(), String> {
    let theirs = "0".repeat(head.len().max(40));
    let updates = format!("HEAD {head} refs/heads/{branch} {theirs}\n");
    let scan = crate::diffscan::pushed(root, "origin", url, &updates)
        .map_err(|why| format!("purlis could not scan what the push would send: {why}"))?;
    let found: Vec<String> = scan
        .refused
        .iter()
        .filter(|finding| !crate::secretshape::is_personal(finding.rule))
        .take(5)
        .map(|finding| {
            format!(
                "{}:{} ({}, {})",
                finding.path, finding.line, finding.kind, finding.masked
            )
        })
        .collect();
    if found.is_empty() {
        return Ok(());
    }
    Err(format!(
        "Not published: the project's history holds what looks like a secret: {}. Take it out \
         of the history, or move it to a vault, then Retry",
        found.join("; ")
    ))
}

impl Hands for OnThisMachine {
    fn commit(&self, root: &Path, plan: &Plan, author: &Author) -> Result<bool, String> {
        let made = !is_its_own_repo(root);
        if made {
            git_in(
                root,
                &["init", "-q", "-b", BRANCH],
                "make the project a repository",
            )?;
        }
        let stages = crate::planegit::save_stages();
        let stages: Vec<&str> = stages.iter().map(String::as_str).collect();
        // Each file committed is the one name it has on disk, as a save's are: staged from one
        // open descriptor and asked again once staged, so a refused file's bytes reach no
        // commit, no push and no object store (`planegit::ondisk`).
        crate::planegit::ondisk::add_checked(
            root,
            &crate::planegit::ondisk::pathspec_of(&stages),
            NOT_ONE_FILE,
            "git could not add the project's files",
        )?;
        let born = git::run(
            root,
            &["rev-parse", "--verify", "--quiet", "HEAD"],
            git::READ,
        )
        .is_ok_and(|run| run.ok());
        let staged = git::run(root, &["diff", "--cached", "--quiet"], git::READ)
            .map_err(|unavailable| format!("purlis could not start git: {unavailable}"))?;
        match staged.code {
            Some(0) if born => Ok(made),
            Some(0) => {
                Err("The project has no file git keeps, so there is nothing to publish".to_string())
            }
            Some(1) => {
                commit_as(
                    root,
                    &commit_message(&plan.name),
                    author,
                    "commit the project",
                )?;
                Ok(made)
            }
            _ => Err(format!(
                "git could not tell what is staged: {}",
                git_words(&staged)
            )),
        }
    }

    fn push(&self, root: &Path, plan: &Plan, made: &MadeRepo) -> Result<(), String> {
        let helper = self.helper(plan)?;
        let branch = branch_of(root)?;
        let url = self.push_url(made);
        // The commit scanned is the commit pushed, by its id (#4515).
        let name = crate::worktree::name::as_ref(&branch);
        let head =
            git::commit_at(root, &name).ok_or("The project has no commit to publish".to_owned())?;
        scanned(root, &head, &branch, &url)?;
        git_in(root, &["remote", "add", "origin", &url], "name the remote")?;
        let refspec = git::push_refspec(&head, &name);
        let args = ["push", "-q", "origin", refspec.as_str()];
        let Some(helper) = helper else {
            crate::createproject::pushed_to_a_folder(root, &args, "push the project")?;
            crate::createproject::follows_origin(root, &branch);
            return Ok(());
        };
        // Its Git LFS objects first: a hook would have sent them, and purlis's git runs none
        // (#4399).
        crate::lfsupload::before_push(&crate::lfsupload::Push {
            dir: root,
            helper: Some(&helper),
            args: &args,
            tip: &head,
            remote: "origin",
        })?;
        let run = git::run_network(root, Some(&helper), &args)
            .map_err(|unavailable| format!("purlis could not start git: {unavailable}"))?;
        if !run.ok() {
            return Err(format!(
                "git could not push the project to {}: {}",
                made.web_url(),
                git_words(&run)
            ));
        }
        crate::createproject::follows_origin(root, &branch);
        // The project's remote from here on: an automatic save pushes to it with purlis's
        // helper, and to no other.
        crate::forge::gitcredential::record_push_remote(root, &url);
        Ok(())
    }

    fn take_back(
        &self,
        root: &Path,
        made_repo: bool,
        named: Option<&MadeRepo>,
    ) -> Result<(), String> {
        if made_repo {
            // The repository this run started, with the one commit it made: the files stay.
            let dot_git = root.join(".git");
            return match std::fs::symlink_metadata(&dot_git) {
                Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&dot_git).map_err(|err| {
                    format!(
                        "purlis could not remove the repository it started at {} ({err}). \
                         Delete that folder yourself.",
                        dot_git.display()
                    )
                }),
                _ => Ok(()),
            };
        }
        let Some(made) = named else {
            return Ok(());
        };
        let url = self.push_url(made);
        let origin = git::run(root, &["remote", "get-url", "origin"], git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|run| run.line().trim().to_string());
        if origin.as_deref() != Some(url.as_str()) {
            return Ok(());
        }
        git_in(root, &["remote", "remove", "origin"], "forget the remote").map_err(|why| {
            format!(
                "{why}. Remove it yourself with git remote remove origin in {}.",
                root.display()
            )
        })
    }
}

#[cfg(test)]
mod tests;
