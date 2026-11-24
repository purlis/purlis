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
    // The remotes of every folder it is in or names, never the working directory's alone (R2-5).
    let mut repos: Vec<String> = whereabouts.repos.to_vec();
    for path in whereabouts.paths() {
        repos.extend(remotes_of(path));
    }
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

/// The anchored plane that owns a repo among `repos` (`owner/name`, lower case): by the remotes of
/// the plane's own folder and of every repo it owns ([`owned_repo_folders`]), never the root's
/// alone (R2-1).
fn anchored_plane_of_a_repo(repos: &[String]) -> Option<PathBuf> {
    answers::anchored_roots().into_iter().find(|root| {
        std::iter::once(root.clone())
            .chain(owned_repo_folders(root))
            .any(|folder| {
                remotes_of(&folder)
                    .iter()
                    .any(|remote| repos.contains(remote))
            })
    })
}

/// The git folders a plane owns, under `workspaces/<ws>/`: a workspace's repo folders, its guest
/// checkouts (`repos/<repo>`) and its branch folders (`.worktrees/<repo>/<piece>`).
fn owned_repo_folders(root: &Path) -> Vec<PathBuf> {
    let children = |dir: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| path.is_dir())
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut folders = Vec::new();
    for workspace in children(&root.join("workspaces")) {
        for child in children(&workspace) {
            match child.file_name().and_then(|name| name.to_str()) {
                Some("repos") => folders.extend(children(&child)),
                Some(".worktrees") => {
                    for repo in children(&child) {
                        folders.extend(children(&repo));
                    }
                }
                _ if child.join(".git").exists() => folders.push(child),
                _ => {}
            }
        }
    }
    folders
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
            // No chat started it, so the host holds no start record of a mode.
            "harness_mode" => "unknown".into(),
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

/// What [`reach_of`] could not read: a folder or repo the shell would expand, a command it could
/// not tokenize, or a merge it could not find in a command of its own. Never read as outside every
/// plane: a caller refuses where any anchored plane exists.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Unreadable(pub String);

/// The characters that make a word one the shell expands or redirects on, so its value is not the
/// word.
const EXPANDED: [char; 14] = [
    '$', '{', '}', '*', '?', '[', ']', '`', '\\', '(', ')', '<', '>', '~',
];

/// The shells: a program that runs a string or a script file the reader does not follow.
const SHELLS: [&str; 9] = [
    "sh", "bash", "zsh", "dash", "ksh", "ash", "fish", "csh", "tcsh",
];

/// What a word can only be to the shell that runs it, in a command's own text: a substitution, a
/// process substitution or a here-document or here-string, whose words are not top-level segments.
const SUBSTITUTIONS: [&str; 5] = ["$(", "`", "<(", ">(", "<<"];

/// The builtins that put a variable in the environment of what runs after, by name and value.
const DECLARERS: [&str; 5] = ["export", "declare", "typeset", "readonly", "local"];

/// The short options of `gh` and `glab`'s merge that take a value: pflag gives the rest of a
/// cluster, or else the next word, to the first of them it meets.
const VALUE_SHORTS: [char; 5] = ['b', 't', 'A', 'F', 'm'];

/// What [`reach_of`] has read so far.
#[derive(Default)]
struct Read {
    reach: Reach,
    /// Every `GH_REPO` seen, the process's and each assignment's, never the last alone (R2-2).
    gh_repos: Vec<String>,
    /// A `cd` was seen, and `CDPATH` is set somewhere.
    cd: bool,
    cdpath: bool,
    /// A segment is a merge the release floor reads, or a forge call whose repo was parsed.
    merge_found: bool,
}

impl Read {
    /// A variable set for what runs: the ones that move where a command acts.
    fn assigned(&mut self, name: &str, value: &str, here: &Path) -> Result<(), Unreadable> {
        match name {
            "GH_REPO" => self.gh_repos.push(value.to_owned()),
            "GIT_DIR" | "GIT_WORK_TREE" => {
                literal(value, name)?;
                self.reach.targets.push(here.join(value));
            }
            "CDPATH" if !value.is_empty() => self.cdpath = true,
            _ => {}
        }
        Ok(())
    }
}

/// **What a command reaches** (review F2): read per segment with the tokenizer the merge hold
/// uses ([`crate::shellseg::segment_argv_parsed`]), never by whitespace. A folder comes from
/// `git`'s own global options (`-C`, `--git-dir`, `--work-tree`, as git composes them), from
/// `GIT_DIR` and `GIT_WORK_TREE` (an assignment, `export` or `declare -x`, or the process's own),
/// from `cd` and `pushd`, and from a wrapper's own chdir (`env -C`, `sudo -D`), never from another
/// tool's `-C`; a repo from `gh` and `glab`'s `-R` and `--repo` (read as pflag reads them), a
/// request's URL, an API path, and every `GH_REPO` (in an assignment, in `env`, or the process's
/// own).
///
/// **Fails closed** (R2-3, R2-4): a word the shell would expand, a `cd` with no folder or to `-`
/// or with `CDPATH` set, a repo that cannot be named, a command that would not tokenize, **a
/// command text with a substitution or a here-document in it, a segment that is a shell, `eval`,
/// `xargs` or another string runner, and a held command that no top-level segment merges** are all
/// [`Unreadable`]: the floor ([`crate::floorguard`]) follows `sh -c`, `eval`, substitutions and
/// aliases to what it holds, and this reader follows none of them, so where it cannot see the
/// command it does not say where the command goes.
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
    if SUBSTITUTIONS.iter().any(|mark| command.contains(mark)) {
        return Err(Unreadable(
            "the command has a substitution or a here-document, whose words are not read"
                .to_owned(),
        ));
    }
    let mut read = Read::default();
    let mut here = cwd.to_path_buf();
    // The process's own environment names a target as an assignment in front would.
    for var in ["GIT_DIR", "GIT_WORK_TREE", "GH_REPO", "CDPATH"] {
        if let Some(value) = env(var).filter(|value| !value.is_empty()) {
            read.assigned(var, &value, &here)?;
        }
    }
    for toks in &segments {
        let invocation = crate::shellwrap::split_env_chdir(toks);
        let base = crate::shellwrap::base_lower(&invocation.prog);
        if runs_a_string(&invocation, &base) {
            return Err(Unreadable(format!(
                "`{}` runs a string or a script whose words are not read",
                invocation.prog
            )));
        }
        for assignment in &invocation.env {
            if let Some((name, value)) = assignment.split_once('=') {
                read.assigned(name, value, &here)?;
            }
        }
        if !invocation.chdir.is_empty() {
            literal(
                &invocation.chdir,
                "the folder a wrapper moves the command to",
            )?;
            read.reach.targets.push(here.join(&invocation.chdir));
        }
        let args = invocation.argv.get(1..).unwrap_or(&[]);
        match base.as_str() {
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
                read.cd = true;
                here = here.join(folder);
                read.reach.targets.push(here.clone());
            }
            name if DECLARERS.contains(&name) => {
                for arg in args {
                    if let Some((name, value)) = arg.split_once('=') {
                        read.assigned(name, value, &here)?;
                    }
                }
            }
            "git" => git_options(args, &here, &mut read.reach)?,
            "gh" | "glab" => read.merge_found |= forge_options(&base, args, &mut read.reach)?,
            _ => {}
        }
        if crate::mergehold::segment_merges(toks) {
            read.merge_found = true;
        }
    }
    if read.cd && read.cdpath {
        return Err(Unreadable(
            "`cd` with CDPATH set goes to a folder not named".to_owned(),
        ));
    }
    if !read.merge_found && crate::mergehold::held(command).is_some() {
        return Err(Unreadable(
            "the held command is not in a command of its own that can be read".to_owned(),
        ));
    }
    for named in &read.gh_repos {
        literal(named, "GH_REPO")?;
        let repo = repo_named(named)
            .ok_or_else(|| Unreadable("GH_REPO does not name an owner and a repo".to_owned()))?;
        read.reach.repos.push(repo);
    }
    Ok(read.reach)
}

/// Whether a segment runs words this reader does not follow: a shell, `eval`, `source`, a wrapper
/// that builds its command from input (`xargs`, `watch`, `flock`, `script`, `parallel`) or from a
/// string (`env -S`), or anything the shell wrappers read a script out of.
fn runs_a_string(invocation: &crate::shellwrap::Invocation, base: &str) -> bool {
    SHELLS.contains(&base)
        || matches!(base, "eval" | "source" | ".")
        || invocation.taken.iter().any(|word| {
            matches!(
                crate::shellwrap::base_lower(word).as_str(),
                "xargs" | "watch" | "flock" | "script" | "parallel" | "busybox"
            ) || word.starts_with("-S")
                || word.starts_with("--split-string")
        })
        || !crate::shellwrap::shell_scripts_of(&invocation.prog, &invocation.argv).is_empty()
        || crate::shellwrap::here_string_script_of(&invocation.prog, &invocation.argv).is_some()
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

/// The repos `gh` and `glab` name in `args`, read as pflag reads options: `-R owner/repo`,
/// `-Rowner/repo`, `-R=owner/repo`, a cluster (`-dR owner/repo`, `-dRowner/repo`), `--repo
/// owner/repo`, `--repo=owner/repo`, and a request's address. A repo that cannot be named is
/// [`Unreadable`]. `true` for an `api` call whose `repos/owner/name` path was read; any other
/// `api` call (graphql, a path with no repo) is [`Unreadable`], as is every `glab api`.
fn forge_options(base: &str, args: &[String], reach: &mut Reach) -> Result<bool, Unreadable> {
    let name = |word: &str, what: &str, reach: &mut Reach| -> Result<(), Unreadable> {
        literal(word, what)?;
        let repo = repo_named(word)
            .ok_or_else(|| Unreadable(format!("{what} does not name an owner and a repo")))?;
        reach.repos.push(repo);
        Ok(())
    };
    let mut words: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "--" {
            words.extend(args[i + 1..].iter().map(String::as_str));
            break;
        } else if let Some(long) = arg.strip_prefix("--") {
            match long.split_once('=') {
                Some(("repo", value)) => name(value, "the repo `--repo` names", reach)?,
                Some(_) => {}
                None if long == "repo" => {
                    i += 1;
                    let value = args.get(i).map(String::as_str).unwrap_or_default();
                    name(value, "the repo `--repo` names", reach)?;
                }
                None if crate::mergehold::VALUE_FLAGS.contains(&arg) => i += 1,
                None => {}
            }
        } else if arg.len() > 1 && arg.starts_with('-') {
            let letters = &arg[1..];
            for (at, letter) in letters.char_indices() {
                let rest = &letters[at + letter.len_utf8()..];
                if letter == 'R' {
                    let value = if rest.is_empty() {
                        i += 1;
                        args.get(i).map(String::as_str).unwrap_or_default()
                    } else {
                        rest.strip_prefix('=').unwrap_or(rest)
                    };
                    name(value, "the repo `-R` names", reach)?;
                    break;
                }
                if VALUE_SHORTS.contains(&letter) {
                    if rest.is_empty() {
                        i += 1;
                    }
                    break;
                }
            }
        } else if arg.contains("://") {
            name(arg, "the repo a request's address names", reach)?;
            words.push(arg);
        } else {
            words.push(arg);
        }
        i += 1;
    }
    if words.first() != Some(&"api") {
        return Ok(false);
    }
    if base == "glab" {
        return Err(Unreadable(
            "`glab api` names a project by its path or id, which is not read".to_owned(),
        ));
    }
    let endpoint = words[1..].iter().find_map(|word| {
        let path = word.trim_start_matches('/');
        path.strip_prefix("repos/")
            .map(|rest| rest.split('/').take(2).collect::<Vec<_>>().join("/"))
            .filter(|repo| repo.split('/').all(|part| !part.is_empty()) && repo.contains('/'))
    });
    let Some(repo) = endpoint else {
        return Err(Unreadable(
            "`gh api` is not a `repos/owner/name` path, so the repo it reaches is not read"
                .to_owned(),
        ));
    };
    name(&repo, "the repo an API path names", reach)?;
    Ok(true)
}

/// `owner/name` (lower case) of a repo as a command spells it: `owner/name`, `host/owner/name`,
/// `git@host:owner/name.git`, or a request's address (`https://host/owner/name/pull/5`; a GitLab
/// one's `/-/merge_requests/5`), by [`crate::forge::namespace_of`] for a URL. `None` where it does
/// not name at least an owner and a name.
fn repo_named(word: &str) -> Option<String> {
    let word = word.trim();
    let from_a_url = word.contains("://");
    let path = crate::forge::namespace_of(word)
        .unwrap_or_else(|| word.trim_end_matches(".git").to_owned());
    let mut parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if let Some(cut) = parts
        .iter()
        .position(|part| matches!(*part, "pull" | "pulls" | "-" | "merge_requests" | "issues"))
    {
        parts.truncate(cut);
    }
    // `host/owner/name`, as `GH_REPO` and `-R` allow: a host has a dot, and is first of three.
    if !from_a_url && parts.len() == 3 && parts[0].contains('.') {
        parts.remove(0);
    }
    (parts.len() >= 2).then(|| parts.join("/").to_ascii_lowercase())
}

#[cfg(test)]
#[path = "locality_tests.rs"]
mod tests;
