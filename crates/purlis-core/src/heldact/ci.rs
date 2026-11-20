// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which pushes touch CI files, and what they change** (ADR 0128 D8.3).
//!
//! A pipeline file runs code as the person (GitLab) or with the repository's secrets (both), so a
//! push that changes one is held for the person's press. **What decides it is the tree diff of the
//! new tip against the base**, never a ref's name or a commit's message:
//!
//! - the base is **the tip the forge has of that branch**, or, for a branch the forge has no tip
//!   for, **the default branch's tip** as the forge reports it for this push, so a new branch that
//!   adds or changes a workflow against the default branch is in the diff;
//! - the same diff covers **every other way a ref moves**: a REST `POST …/git/refs`, a `PATCH`,
//!   GraphQL's `createRef` and `updateRef`. Their target is a commit, and it is diffed the same
//!   way: its **tree** against the base's, wherever the objects came from. GitHub shares objects
//!   between a repository and its forks, so a ref can be created at a commit nobody pushed to this
//!   repository; a commit's own change from its parent says nothing of how its tree differs from
//!   the base's.
//! - **a diff that cannot be computed refuses the push** ([`PushCheck::Refuse`]): an object the
//!   clone does not have, a diff too large to show, a path the app cannot read, a `local:` include
//!   whose path is a variable. It is never allowed and never guessed.
//!
//! CI files are `.github/workflows/**` on GitHub, and on GitLab the project's pipeline file
//! (`.gitlab-ci.yml`, or the project's `ci_config_path` where it names a file of this repository),
//! every file its `include:` names in this repository (a `local:` path, a glob of paths, a plain
//! string), and every file a job's `trigger: include:` names. An include of another project, a
//! remote address, a template or a component is the forge's to serve, not this push's: it is named
//! beside the diff so the person reads that the pipeline also runs it.

use super::Trees;
use crate::forge::Kind;
use crate::mergehold::HeadSha;

/// The most of a diff the ask shows. A larger one cannot be shown whole, and what cannot be shown
/// is refused.
pub const MOST_DIFF: usize = 256 * 1024;

/// The deepest chain of `local:` includes read, and the most files read for one push.
const DEEPEST: usize = 8;
const MOST_FILES: usize = 64;

/// The pipeline file GitLab reads where the project names none.
pub const GITLAB_DEFAULT: &str = ".gitlab-ci.yml";

/// **What a push changes in CI files**, as the ask shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CiDiff {
    /// The commit the new tip was compared with.
    pub against: HeadSha,
    /// The CI files the push adds, changes or removes, sorted.
    pub files: Vec<String>,
    /// The changed lines of those files, as git writes a patch. Only CI files are in it.
    pub diff: String,
    /// What the pipeline runs that this repository does not hold (an include of another project,
    /// a remote address, a template, a component), as the pipeline names it, sorted.
    pub outside: Vec<String>,
}

/// What a push's tree diff says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushCheck {
    /// The push changes no CI file: the proxy lets it through its ordinary rules.
    NoCiFiles,
    /// The push changes CI files: it is held, with this diff.
    Hold(CiDiff),
    /// The diff could not be computed, or cannot be shown: the push is refused, for this reason.
    Refuse(String),
}

/// **Whether pushing `tip`, as `against`'s successor, touches CI files** on a forge of `kind`.
///
/// `ci_config` is the project's own `ci_config_path` as the forge reports it for this push (GitLab
/// only; `None` where the project sets none), read at each push and never cached.
pub fn check_push(
    kind: Kind,
    ci_config: Option<&str>,
    trees: &dyn Trees,
    against: &HeadSha,
    tip: &HeadSha,
) -> PushCheck {
    let changed = match trees.changed(against, tip) {
        Ok(changed) => changed,
        Err(why) => {
            return PushCheck::Refuse(format!(
                "purlis could not compare the push with {}: {why}",
                short(against)
            ));
        }
    };
    if changed.is_empty() {
        return PushCheck::NoCiFiles;
    }
    let (matcher, outside) = match kind {
        Kind::GitHub => (Matcher::always(), Vec::new()),
        Kind::GitLab => match gitlab_matcher(ci_config, trees, against, tip) {
            Ok(found) => found,
            Err(why) => return PushCheck::Refuse(why),
        },
    };
    let mut files: Vec<String> = changed
        .into_iter()
        .filter(|path| matcher.matches(path))
        .collect();
    files.sort();
    files.dedup();
    if files.is_empty() {
        return PushCheck::NoCiFiles;
    }
    let diff = match trees.patch(against, tip, &files) {
        Ok(diff) => diff,
        Err(why) => {
            return PushCheck::Refuse(format!(
                "purlis could not read what the push changes in {}: {why}",
                files.join(", ")
            ));
        }
    };
    if diff.len() > MOST_DIFF {
        return PushCheck::Refuse(format!(
            "the push changes {} by more than purlis can show ({} KiB at most), so it is not \
             held and not allowed",
            files.join(", "),
            MOST_DIFF / 1024
        ));
    }
    if diff.lines().any(|line| line.starts_with("Binary files ")) {
        // What git would not show as text is what a chat can hide a change in: never held on a
        // diff the person cannot read.
        return PushCheck::Refuse(format!(
            "the push changes {} in a way git shows as binary, so it cannot be shown and is not \
             allowed",
            files.join(", ")
        ));
    }
    if diff.trim().is_empty() {
        // A change git shows no lines for (a mode change, a binary file): nothing to read, so
        // nothing the person could approve knowingly.
        return PushCheck::Refuse(format!(
            "the push changes {} in a way that has no lines to show, so it is not held and not \
             allowed",
            files.join(", ")
        ));
    }
    PushCheck::Hold(CiDiff {
        against: against.clone(),
        files,
        diff,
        outside,
    })
}

fn short(sha: &HeadSha) -> &str {
    &sha.as_str()[..12]
}

/// Which paths are CI files.
struct Matcher {
    /// A path that starts with one of these.
    prefixes: Vec<String>,
    /// A path that is exactly one of these.
    exact: Vec<String>,
    /// A path a glob matches.
    globs: Vec<String>,
    /// Every YAML file: set where a pipeline's includes cannot all be resolved (a YAML merge
    /// key builds one), so no changed YAML file can be said not to be one the pipeline runs.
    any_yaml: bool,
}

/// Files beside the workflows that decide what runs or who approves it, on both forges.
const ALWAYS: &[&str] = &[
    ".github/dependabot.yml",
    ".github/dependabot.yaml",
    "renovate.json",
    "renovate.json5",
    ".renovaterc",
    ".renovaterc.json",
    ".renovaterc.json5",
    ".github/renovate.json",
    ".github/renovate.json5",
    ".gitlab/renovate.json",
    "CODEOWNERS",
    ".github/CODEOWNERS",
    "docs/CODEOWNERS",
    ".gitlab/CODEOWNERS",
];

impl Matcher {
    /// What runs, or decides who may approve what runs, on either forge: workflows and the
    /// actions they use, dependency bots' configuration (a bot's pull request is merged by
    /// rules the file sets), and who owns a path.
    fn always() -> Matcher {
        Matcher {
            prefixes: vec![
                ".github/workflows/".to_owned(),
                ".github/actions/".to_owned(),
            ],
            exact: ALWAYS.iter().map(|path| (*path).to_owned()).collect(),
            globs: Vec::new(),
            any_yaml: false,
        }
    }

    fn matches(&self, path: &str) -> bool {
        self.prefixes.iter().any(|prefix| path.starts_with(prefix))
            || self.exact.iter().any(|exact| exact == path)
            || self.globs.iter().any(|glob| glob_matches(glob, path))
            || (self.any_yaml && (path.ends_with(".yml") || path.ends_with(".yaml")))
    }
}

/// What GitLab's `include:` names, in this repository and outside it.
#[derive(Debug, Default)]
struct Includes {
    local: Vec<String>,
    outside: Vec<String>,
    /// An include a YAML merge key (`<<`) builds: it cannot be read from the tree as parsed.
    unresolved: bool,
}

fn gitlab_matcher(
    ci_config: Option<&str>,
    trees: &dyn Trees,
    against: &HeadSha,
    tip: &HeadSha,
) -> Result<(Matcher, Vec<String>), String> {
    let mut roots = vec![GITLAB_DEFAULT.to_owned()];
    if let Some(path) = ci_config {
        // `path@group/project`, an address and a file this repository does not hold are not one of
        // its files; a path it does hold is.
        if !path.contains('@') && !path.contains("://") && !path.is_empty() {
            roots.push(path.trim_start_matches('/').to_owned());
        }
    }
    let mut matcher = Matcher::always();
    matcher.exact.extend(roots.iter().cloned());
    let mut outside = Vec::new();
    // Read at the tip and at the base: an include the push removes is a change of the root file,
    // and a file the base included is still one a push of it changes.
    for commit in [tip, against] {
        let mut queue: Vec<(String, usize)> = roots.iter().map(|root| (root.clone(), 0)).collect();
        let mut read = 0;
        while let Some((path, depth)) = queue.pop() {
            if read >= MOST_FILES || depth > DEEPEST {
                return Err(format!(
                    "the pipeline of this project includes more files than purlis reads ({MOST_FILES}), so the push is not held and not allowed"
                ));
            }
            let Some(text) = trees.read(commit, &path).map_err(|why| {
                format!("purlis could not read {path} at {}: {why}", short(commit))
            })?
            else {
                continue;
            };
            read += 1;
            let found = includes_of(&text).map_err(|why| {
                format!(
                    "purlis could not read the includes of {path} at {}: {why}, so the push is not held and not allowed",
                    short(commit)
                )
            })?;
            outside.extend(found.outside);
            if found.unresolved {
                matcher.any_yaml = true;
                outside.push(
                    "includes a YAML merge key builds, which purlis cannot resolve: every YAML \
                     file of the push is read as a pipeline file"
                        .to_owned(),
                );
            }
            for local in found.local {
                if local.contains(['*', '?']) {
                    // A glob names the files of the tree it matches: each is read in turn, for
                    // the includes it makes.
                    let files = trees.files(commit).map_err(|why| {
                        format!(
                            "purlis could not list the files at {}: {why}",
                            short(commit)
                        )
                    })?;
                    for file in files.into_iter().filter(|file| glob_matches(&local, file)) {
                        if !matcher.exact.contains(&file) {
                            matcher.exact.push(file.clone());
                            queue.push((file, depth + 1));
                        }
                    }
                    if !matcher.globs.contains(&local) {
                        matcher.globs.push(local);
                    }
                    continue;
                }
                if !matcher.exact.contains(&local) {
                    matcher.exact.push(local.clone());
                    queue.push((local, depth + 1));
                }
            }
        }
    }
    outside.sort();
    outside.dedup();
    Ok((matcher, outside))
}

/// **What a pipeline file includes**: `include:` at the top, and `trigger: include:` of a job.
/// A `local:` path with a variable in it cannot be resolved, and is an error.
fn includes_of(text: &str) -> Result<Includes, String> {
    use yaml_rust2::{Yaml, YamlLoader};
    let docs = YamlLoader::load_from_str(text).map_err(|why| why.to_string())?;
    let mut found = Includes::default();
    for doc in &docs {
        let Yaml::Hash(top) = doc else { continue };
        if top.contains_key(&Yaml::String("<<".to_owned())) {
            found.unresolved = true;
        }
        for (key, value) in top {
            if key.as_str() == Some("include") {
                one_include(value, &mut found)?;
            } else if let Yaml::Hash(job) = value
                && let Some(Yaml::Hash(trigger)) = job.get(&Yaml::String("trigger".to_owned()))
                && let Some(include) = trigger.get(&Yaml::String("include".to_owned()))
            {
                one_include(include, &mut found)?;
            }
        }
    }
    Ok(found)
}

fn one_include(value: &yaml_rust2::Yaml, found: &mut Includes) -> Result<(), String> {
    use yaml_rust2::Yaml;
    match value {
        Yaml::String(text) => string_include(text, found),
        Yaml::Array(items) => items.iter().try_for_each(|item| one_include(item, found)),
        Yaml::Hash(map) => {
            if map.contains_key(&Yaml::String("<<".to_owned())) {
                found.unresolved = true;
            }
            let get = |name: &str| map.get(&Yaml::String(name.to_owned()));
            if let Some(local) = get("local") {
                match local {
                    Yaml::String(path) => local_include(path, found)?,
                    Yaml::Array(paths) => {
                        for path in paths {
                            match path.as_str() {
                                Some(path) => local_include(path, found)?,
                                None => return Err("a local include is not a path".to_owned()),
                            }
                        }
                    }
                    _ => return Err("a local include is not a path".to_owned()),
                }
            }
            for (name, what) in [
                ("remote", "a remote address"),
                ("template", "a template"),
                ("component", "a component"),
                ("project", "another project"),
            ] {
                if let Some(Yaml::String(named)) = get(name) {
                    found.outside.push(format!("{what}: {named}"));
                }
            }
            if get("artifact").is_some() {
                found
                    .outside
                    .push("a pipeline another job generates".to_owned());
            }
            Ok(())
        }
        _ => Err("an include is not a path or a list".to_owned()),
    }
}

fn string_include(text: &str, found: &mut Includes) -> Result<(), String> {
    if text.starts_with("http://") || text.starts_with("https://") {
        found.outside.push(format!("a remote address: {text}"));
        Ok(())
    } else {
        local_include(text, found)
    }
}

fn local_include(path: &str, found: &mut Includes) -> Result<(), String> {
    if path.contains('$') {
        return Err(format!(
            "the include {path} has a variable in its path, which purlis cannot resolve"
        ));
    }
    found.local.push(path.trim_start_matches('/').to_owned());
    Ok(())
}

/// GitLab's glob for a `local:` include: `*` is any run of characters within one folder, `**`
/// any run across folders, `?` one character.
fn glob_matches(glob: &str, path: &str) -> bool {
    fn at(glob: &[u8], path: &[u8]) -> bool {
        match glob.split_first() {
            None => path.is_empty(),
            Some((b'*', rest)) => {
                if let Some(rest) = rest.strip_prefix(b"*") {
                    let rest = rest.strip_prefix(b"/").unwrap_or(rest);
                    (0..=path.len()).any(|skip| at(rest, &path[skip..]))
                } else {
                    (0..=path.len())
                        .take_while(|skip| *skip == 0 || path[skip - 1] != b'/')
                        .any(|skip| at(rest, &path[skip..]))
                }
            }
            Some((b'?', rest)) => path
                .split_first()
                .is_some_and(|(c, tail)| *c != b'/' && at(rest, tail)),
            Some((c, rest)) => path
                .split_first()
                .is_some_and(|(d, tail)| c == d && at(rest, tail)),
        }
    }
    at(glob.trim_start_matches('/').as_bytes(), path.as_bytes())
}

/// **The trees of a clone, read by git** (the standard tool): the diff of two commits' trees, the
/// patch of some paths of it, and a file at a commit.
pub struct GitTrees {
    clone: std::path::PathBuf,
}

impl GitTrees {
    /// Trees of the clone at `clone`.
    pub fn of(clone: impl Into<std::path::PathBuf>) -> GitTrees {
        GitTrees {
            clone: clone.into(),
        }
    }

    /// git, read with nothing of the clone's own to say about what it shows: **replace refs are
    /// not followed** (a `refs/replace/<sha>` makes git read another object for a commit's id, so
    /// a tree diff of the real commits would be empty), and every call that shows a patch is
    /// `--text` (a `-diff` attribute in `.git/info/attributes` or a `.gitattributes` makes git
    /// print "Binary files differ" for a file whose lines are the change).
    fn git(&self, args: &[&str]) -> Result<String, String> {
        use crate::worktree::git;
        let mut argv = vec![
            "--no-replace-objects",
            "-c",
            "core.attributesFile=/dev/null",
        ];
        argv.extend_from_slice(args);
        let done = git::run(&self.clone, &argv, git::READ).map_err(|why| why.to_string())?;
        if done.ok() {
            Ok(done.out)
        } else {
            Err(done
                .err
                .trim()
                .lines()
                .next()
                .unwrap_or("git failed")
                .to_owned())
        }
    }
}

impl Trees for GitTrees {
    fn changed(&self, against: &HeadSha, tip: &HeadSha) -> Result<Vec<String>, String> {
        // Both objects must be here: a commit the clone lacks has no tree to compare.
        for sha in [against, tip] {
            self.git(&["cat-file", "-e", &format!("{}^{{commit}}", sha.as_str())])
                .map_err(|why| format!("the clone has no commit {}: {why}", short(sha)))?;
        }
        let out = self.git(&[
            "diff-tree",
            "-r",
            "-z",
            "--raw",
            "--text",
            "--no-renames",
            "--no-textconv",
            "--no-ext-diff",
            against.as_str(),
            tip.as_str(),
        ])?;
        let mut paths = Vec::new();
        let mut tokens = out.split('\0').filter(|token| !token.is_empty());
        while let Some(meta) = tokens.next() {
            if !meta.starts_with(':') {
                return Err("git's diff was not in the form purlis reads".to_owned());
            }
            let path = tokens
                .next()
                .ok_or_else(|| "git's diff ended inside an entry".to_owned())?;
            if path.contains('\u{fffd}') {
                return Err("a changed path is not text purlis can read".to_owned());
            }
            paths.push(path.to_owned());
        }
        Ok(paths)
    }

    fn patch(&self, against: &HeadSha, tip: &HeadSha, paths: &[String]) -> Result<String, String> {
        let mut args = vec![
            "--literal-pathspecs",
            "diff-tree",
            "-r",
            "-p",
            "--text",
            "--no-renames",
            "--no-textconv",
            "--no-ext-diff",
            "--no-color",
            "--unified=3",
            against.as_str(),
            tip.as_str(),
            "--",
        ];
        args.extend(paths.iter().map(String::as_str));
        self.git(&args)
    }

    fn files(&self, commit: &HeadSha) -> Result<Vec<String>, String> {
        let out = self.git(&["ls-tree", "-r", "-z", "--name-only", commit.as_str()])?;
        Ok(out
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect())
    }

    fn read(&self, commit: &HeadSha, path: &str) -> Result<Option<String>, String> {
        let at = format!("{}:{path}", commit.as_str());
        let listed = self.git(&[
            "--literal-pathspecs",
            "ls-tree",
            "-z",
            commit.as_str(),
            "--",
            path,
        ])?;
        if listed.is_empty() {
            return Ok(None);
        }
        self.git(&["cat-file", "blob", &at]).map(Some)
    }
}
