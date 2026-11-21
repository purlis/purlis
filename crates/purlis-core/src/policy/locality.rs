// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What counts as outside any plane** (M75-07b, spec §6; review round 4, SEC4-SF1).
//!
//! A process is plane-less only when its working directory, the folders its command names (`git
//! -C`'s, a `cd`'s), the repos it names and every folder of its ancestry are all outside every
//! plane. Otherwise it belongs to a plane, and is never given the local `strict` of a process with
//! no host to ask.
//!
//! **A plane is anchored** when [`answers::anchored_roots`] holds it: the registry of planes where
//! this process can read it, and the app's non-secret list of anchored planes where it cannot
//! (`purlis hook pretooluse` has no seal key, so the registry reads as unreadable there). **With
//! no keyring and no list**, the same test runs against the planes found on disk
//! (`plane::find_root`'s upward search for the project file, a linked worktree's main tree
//! included), never the registry alone, and **a plane found on disk is looked up in the anchored
//! list by its root** (a worktree cut outside the plane is still the plane's). One found only on
//! disk is judged by the shipped default with the seal gap declared: nothing can be sealed for
//! it.
//!
//! **Ancestry is not implemented in the hook** ([`Whereabouts::ancestry`] is empty there): a
//! process the app did not start is judged by its own folder, the folders and repos its command
//! names, and never by what started it.

use std::path::{Path, PathBuf};

use crate::secrets::answers;

use super::decide::{Decision, Request, decide_locally};

/// What the registry cannot tell where the keyring works but the plane was never anchored.
pub const NOT_ANCHORED_NOTICE: &str = "This plane is not anchored on this machine, so purlis cannot \
     seal your answers for it and the shipped default applies. Nothing you answer is kept.";

/// **Where a process is**, as the host or the hook reads it.
#[derive(Debug, Clone, Copy)]
pub struct Whereabouts<'a> {
    /// The process's working directory.
    pub cwd: &'a Path,
    /// The folders the command acts on ([`reach_of`]): each `git -C`, `--git-dir` and
    /// `--work-tree`, each `cd`.
    pub targets: &'a [PathBuf],
    /// The repos the command names, `owner/name` in lower case ([`reach_of`]): `-R`, `--repo`,
    /// `GH_REPO`, a request's address. The working directory's own remotes are always read too.
    pub repos: &'a [String],
    /// The working directory of each process above it, up to the app's chat.
    pub ancestry: &'a [PathBuf],
}

impl Whereabouts<'_> {
    fn paths(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.cwd)
            .chain(self.targets.iter().map(PathBuf::as_path))
            .chain(self.ancestry.iter().map(PathBuf::as_path))
    }
}

/// Which plane, if any, a process belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Locality {
    /// Nothing of it is in a plane.
    PlaneLess,
    /// Something of it is in the plane at `root`.
    InPlane {
        /// The plane's root, by its real path.
        root: PathBuf,
        /// Whether the anchored planes hold it. Not anchored: found on disk alone.
        anchored: bool,
    },
}

/// **Whether `whereabouts` is outside every plane**, by the anchored planes and by the planes on
/// disk. An anchored plane is preferred where several are named, whichever way names it: a
/// folder inside it, a plane found on disk that the anchored list holds by its root, or a repo
/// whose remote is one of an anchored plane's.
pub fn locate(whereabouts: &Whereabouts<'_>) -> Locality {
    if let Some(root) = whereabouts
        .paths()
        .find_map(answers::anchored_root_containing)
    {
        return Locality::InPlane {
            root,
            anchored: true,
        };
    }
    // Never the registry alone: where it is empty, unreadable or has no keyring behind it, a
    // plane is still the plane a project file marks above a path.
    let found: Vec<PathBuf> = whereabouts
        .paths()
        .filter_map(|path| {
            let real = crate::sandbox::real(path);
            let root = crate::plane::find_root(&real).ok()?;
            Some(crate::sandbox::real(&root))
        })
        .collect();
    // A linked worktree cut outside the plane is found by its main tree's plane; that root is
    // asked of the anchored list too, never taken as unanchored for being found (F3).
    if let Some(root) = found.iter().find(|root| answers::plane_is_anchored(root)) {
        return Locality::InPlane {
            root: root.clone(),
            anchored: true,
        };
    }
    let mut repos: Vec<String> = whereabouts.repos.to_vec();
    repos.extend(remotes_of(whereabouts.cwd));
    if !repos.is_empty()
        && let Some(root) = anchored_plane_of_a_repo(&repos)
    {
        return Locality::InPlane {
            root,
            anchored: true,
        };
    }
    match found.into_iter().next() {
        Some(root) => Locality::InPlane {
            root,
            anchored: false,
        },
        None => Locality::PlaneLess,
    }
}

/// The anchored plane whose remote is one of `repos` (`owner/name`, lower case).
fn anchored_plane_of_a_repo(repos: &[String]) -> Option<PathBuf> {
    answers::anchored_roots()
        .into_iter()
        .find(|root| remotes_of(root).iter().any(|remote| repos.contains(remote)))
}

/// The repos (`owner/name`, lower case) that the remotes of the git folder at or above `folder`
/// name, read from its config: a linked worktree's `.git` file leads to the main tree's. Empty
/// where there is no git folder or its config does not read.
pub fn remotes_of(folder: &Path) -> Vec<String> {
    let real = crate::sandbox::real(folder);
    let Some(git) = real
        .ancestors()
        .map(|dir| dir.join(".git"))
        .find(|git| git.exists())
    else {
        return Vec::new();
    };
    let mut dir = git.clone();
    if git.is_file() {
        let Ok(text) = std::fs::read_to_string(&git) else {
            return Vec::new();
        };
        let Some(gitdir) = text.trim().strip_prefix("gitdir:") else {
            return Vec::new();
        };
        dir = git.parent().unwrap_or(&real).join(gitdir.trim());
        if let Ok(common) = std::fs::read_to_string(dir.join("commondir")) {
            dir = dir.join(common.trim());
        }
    }
    let Ok(config) = std::fs::read_to_string(dir.join("config")) else {
        return Vec::new();
    };
    config
        .lines()
        .filter_map(|line| line.trim().strip_prefix("url"))
        .filter_map(|rest| rest.trim_start().strip_prefix('='))
        .filter_map(crate::forge::namespace_of)
        .map(|namespace| namespace.to_ascii_lowercase())
        .collect()
}

/// **What is to be done with an ask from a process** (see [`judge`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judged {
    /// Outside every plane: the shipped default under `strict`, decided in the asking process.
    Locally(Decision),
    /// In a plane found on disk that nothing is sealed for: the shipped default decides, and
    /// `gap` says why nothing the person loosened or kept applies.
    FoundPlane {
        /// The decision.
        decision: Decision,
        /// The seal gap, in the words the window and doctor use.
        gap: &'static str,
    },
    /// In an anchored plane: its host decides, and no host means a refusal, never a local answer.
    AskTheHost {
        /// The plane's root.
        root: PathBuf,
    },
}

/// **Which decision a process that is no chat's gets** (spec §6): the shipped `strict` outside
/// every plane, the shipped default with the seal gap declared in a plane found only on disk, and
/// the plane's host for one that is anchored.
pub fn judge(whereabouts: &Whereabouts<'_>, request: &Request<'_>) -> Judged {
    match locate(whereabouts) {
        Locality::PlaneLess => Judged::Locally(decide_locally(request)),
        Locality::InPlane {
            anchored: false, ..
        } => Judged::FoundPlane {
            decision: decide_locally(request),
            gap: if answers::registry_reads() {
                NOT_ANCHORED_NOTICE
            } else {
                answers::NO_KEYRING_NOTICE
            },
        },
        Locality::InPlane { root, .. } => Judged::AskTheHost { root },
    }
}

/// **The request a process that is no chat's makes** (spec §6): the action and resource it asks
/// about, and the context a host would make for it where none can: unattended, no sandbox of
/// purlis's, arriving by a hook. Only the fields the action's context declares.
pub fn process_request<'a>(action: &'a str, resource: (&'a str, &'a str)) -> Request<'a> {
    let mut context = serde_json::Map::new();
    for field in super::context_fields(action) {
        let value: serde_json::Value = match field.as_str() {
            "sandbox" => "off".into(),
            "attended" => false.into(),
            "harness_mode" => "default".into(),
            "depth" => 0.into(),
            "via" => "hook".into(),
            _ => continue,
        };
        context.insert(field.clone(), value);
    }
    Request {
        chat: crate::enforcement::UNKNOWN_CHAT,
        of: super::levels::Chat {
            workspace: None,
            persona: None,
        },
        action,
        resource,
        context: serde_json::Value::Object(context),
    }
}

/// What a command reaches, as its own words say ([`reach_of`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reach {
    /// The folders it acts on: `git -C`, `--git-dir`, `--work-tree`, `cd`, `pushd`, each against
    /// the folder it is run from.
    pub targets: Vec<PathBuf>,
    /// The repos it names, `owner/name` in lower case.
    pub repos: Vec<String>,
}

/// What [`reach_of`] could not read: a folder or repo the shell would expand, or a command it
/// could not tokenize. Never read as outside every plane: a caller refuses.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Unreadable(pub String);

/// The characters that make a word one the shell expands or redirects on, so its value is not the
/// word.
const EXPANDED: [char; 14] = [
    '$', '{', '}', '*', '?', '[', ']', '`', '\\', '(', ')', '<', '>', '~',
];

/// **What a command reaches** (review F2): read per segment with the tokenizer the merge hold
/// uses ([`crate::shellseg::segment_argv_parsed`]), never by whitespace. A folder comes from
/// `git`'s own global options (`-C`, `--git-dir`, `--work-tree`, as git composes them), from
/// `GIT_DIR` and `GIT_WORK_TREE`, and from `cd` and `pushd`, never from another tool's `-C`; a
/// repo from `gh` and `glab`'s `-R` and `--repo`, a request's URL, and `GH_REPO` (in the
/// segment's assignments or in `env`, the process's own).
///
/// **Fails closed**: a word the shell would expand, a `cd` with no folder or to `-`, or a command
/// that would not tokenize is [`Unreadable`].
pub fn reach_of(
    command: &str,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Reach, Unreadable> {
    let (segments, parsed) = crate::shellseg::segment_argv_parsed(command);
    if !parsed {
        return Err(Unreadable(
            "the command could not be read word by word".to_owned(),
        ));
    }
    let mut reach = Reach::default();
    let mut here = cwd.to_path_buf();
    // The process's own environment names a target as an assignment in front would.
    for var in ["GIT_DIR", "GIT_WORK_TREE"] {
        if let Some(value) = env(var).filter(|value| !value.is_empty()) {
            literal(&value, var)?;
            reach.targets.push(here.join(value));
        }
    }
    let mut gh_repo = env("GH_REPO").filter(|value| !value.is_empty());
    for toks in &segments {
        let invocation = crate::shellwrap::split_env_chdir(toks);
        for assignment in &invocation.env {
            let Some((name, value)) = assignment.split_once('=') else {
                continue;
            };
            match name {
                "GH_REPO" => gh_repo = Some(value.to_owned()),
                "GIT_DIR" | "GIT_WORK_TREE" => {
                    literal(value, name)?;
                    reach.targets.push(here.join(value));
                }
                _ => {}
            }
        }
        let args = invocation.argv.get(1..).unwrap_or(&[]);
        match crate::shellwrap::base_lower(&invocation.prog).as_str() {
            "cd" | "pushd" => {
                let folder = args
                    .iter()
                    .find(|arg| !arg.starts_with('-') || arg.as_str() == "-")
                    .map(String::as_str)
                    .unwrap_or_default();
                literal(folder, "the folder `cd` goes to")?;
                if folder == "-" {
                    return Err(Unreadable("`cd -` goes to a folder not named".to_owned()));
                }
                here = here.join(folder);
                reach.targets.push(here.clone());
            }
            "git" => git_options(args, &here, &mut reach)?,
            "gh" | "glab" => forge_options(args, &mut reach)?,
            _ => {}
        }
    }
    if let Some(named) = gh_repo {
        literal(&named, "GH_REPO")?;
        reach.repos.push(repo_named(&named));
    }
    Ok(reach)
}

/// `Ok` for a plain word; the refusal naming `what` for one the shell would expand, or none.
fn literal(word: &str, what: &str) -> Result<(), Unreadable> {
    if word.is_empty() || word.contains(EXPANDED) {
        Err(Unreadable(format!(
            "{what} is not a plain word the shell leaves as written"
        )))
    } else {
        Ok(())
    }
}

/// The folders of `git`'s global options in `args` (those before its subcommand), as git
/// composes them: each `-C` is against the one before.
fn git_options(args: &[String], here: &Path, reach: &mut Reach) -> Result<(), Unreadable> {
    let mut at = here.to_path_buf();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let next = args.get(i + 1).map(String::as_str).unwrap_or_default();
        match arg {
            "-C" => {
                literal(next, "the folder `git -C` names")?;
                at = at.join(next);
                reach.targets.push(at.clone());
                i += 1;
            }
            "-c" | "--namespace" | "--exec-path" | "--super-prefix" => i += 1,
            "--git-dir" | "--work-tree" => {
                literal(next, arg)?;
                reach.targets.push(at.join(next));
                i += 1;
            }
            _ if arg.starts_with("--git-dir=") || arg.starts_with("--work-tree=") => {
                let folder = arg.split_once('=').map_or("", |(_, value)| value);
                literal(folder, arg)?;
                reach.targets.push(at.join(folder));
            }
            // The first word that is no option is the subcommand: git reads no global option
            // after it.
            _ if !arg.starts_with('-') => break,
            _ => {}
        }
        i += 1;
    }
    Ok(())
}

/// The repos `gh` and `glab` name in `args`: `-R`, `--repo`, and a request's address.
fn forge_options(args: &[String], reach: &mut Reach) -> Result<(), Unreadable> {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "-R" || arg == "--repo" {
            let named = args.get(i + 1).map(String::as_str).unwrap_or_default();
            literal(named, "the repo `-R` names")?;
            reach.repos.push(repo_named(named));
            i += 1;
        } else if let Some(named) = arg.strip_prefix("--repo=") {
            literal(named, "the repo `--repo` names")?;
            reach.repos.push(repo_named(named));
        } else if arg.contains("://") {
            reach.repos.push(repo_named(arg));
        }
        i += 1;
    }
    Ok(())
}

/// `owner/name` (lower case) of a repo as a command spells it: `owner/name`, `host/owner/name`,
/// or a request's address (`https://host/owner/name/pull/5`; a GitLab one's `/-/merge_requests/5`).
fn repo_named(word: &str) -> String {
    let word = word.trim().trim_end_matches(".git");
    let path = word.split_once("://").map_or(word, |(_, rest)| {
        rest.split_once('/').map_or("", |(_, path)| path)
    });
    let mut parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if let Some(cut) = parts
        .iter()
        .position(|part| matches!(*part, "pull" | "pulls" | "-" | "merge_requests" | "issues"))
    {
        parts.truncate(cut);
    }
    // `host/owner/name`, as `GH_REPO` and `-R` allow: a host has a dot, and is first of three.
    if !word.contains("://") && parts.len() == 3 && parts[0].contains('.') {
        parts.remove(0);
    }
    parts.join("/").to_ascii_lowercase()
}

#[cfg(test)]
#[path = "locality_tests.rs"]
mod tests;
