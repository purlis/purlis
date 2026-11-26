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
//!   clone does not have, a diff too large to show, a path the app cannot read. It is never allowed and never guessed.
//!
//! **Which paths are CI files is decided by the path alone, never by reading a file.** A push is
//! held if it adds, changes, deletes or renames:
//!
//! - on GitHub, anything under `.github/` (workflows, local actions, reusable workflows and what
//!   they reference) and any file named `action.yml` or `action.yaml`, in any case, anywhere;
//! - on GitLab, **any file whose name ends in `.yml` or `.yaml`, in any case, anywhere in the
//!   tree**, and the project's configured pipeline file (`ci_config_path`, read from the forge at
//!   each push) whatever it is called. A GitLab `local:` include must be a YAML file, so this
//!   covers every include without working out the include graph. The cost is an ask for a YAML-only
//!   edit that no pipeline includes.
//!
//! Five review rounds each found inputs on which the YAML reader here and GitLab's loader disagree
//! (tags, merge keys, document markers, line breaks, interpolation of keys), so the include graph
//! is not parsed and no pipeline file is read to decide anything.

use super::Trees;
use crate::forge::Kind;
use crate::mergehold::HeadSha;

/// The most of a diff the ask shows. A larger one cannot be shown whole, and what cannot be shown
/// is refused.
pub const MOST_DIFF: usize = 256 * 1024;

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
    /// How many other files the same push changes, which are not CI files and are not shown.
    pub others: usize,
    /// The message of the commit the push makes, as it will be sent.
    pub message: String,
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
    check_push_since(kind, ci_config, trees, against, None, tip)
}

/// **Whether pushing `tip` puts CI files on the forge that differ from `against`'s**, the default
/// branch's tip now, for a branch the push creates as for one that is there.
///
/// A CI file of `tip` that is not what `against` holds is a change, however the branch came by it:
/// a rebase onto the current default branch brings that branch's CI files in, and so is clean,
/// while a tip built on an older commit, with that commit's CI files, is held. The one exception
/// is `already`, the branch's old tip on the forge, for a push to a branch that is there: a file
/// the push leaves as it was at `already` is not a change the push makes, whatever it differs from
/// `against` by. Where there is no `already` (a new branch) nothing is excepted.
pub fn check_push_since(
    kind: Kind,
    ci_config: Option<&str>,
    trees: &dyn Trees,
    against: &HeadSha,
    already: Option<&HeadSha>,
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
    let matcher = match kind {
        Kind::GitHub => Matcher::github(),
        Kind::GitLab => Matcher::gitlab(ci_config),
    };
    let total = changed.len();
    let mut files: Vec<String> = changed
        .into_iter()
        .filter(|path| matcher.matches(path))
        .collect();
    files.sort();
    files.dedup();
    if let Some(already) = already
        && !files.is_empty()
    {
        let moved = match trees.changed(already, tip) {
            Ok(moved) => moved,
            Err(why) => {
                return PushCheck::Refuse(format!(
                    "purlis could not compare the push with the branch's tip {}: {why}",
                    short(already)
                ));
            }
        };
        files.retain(|path| moved.contains(path));
    }
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
        // A change git shows no lines at all for (an empty file added, say): nothing to read, so
        // nothing the person could approve knowingly. A chmod-only change to a `.yml` is not this
        // case: git prints `old mode` / `new mode` header lines, so it is held and shown.
        return PushCheck::Refuse(format!(
            "the push changes {} in a way that has no lines to show, so it is not held and not \
             allowed",
            files.join(", ")
        ));
    }
    let others = total.saturating_sub(files.len());
    let message = match trees.message(tip) {
        Ok(message) => message,
        Err(why) => {
            return PushCheck::Refuse(format!(
                "purlis could not read the commit message of {}: {why}",
                short(tip)
            ));
        }
    };
    PushCheck::Hold(CiDiff {
        against: against.clone(),
        files,
        diff,
        others,
        message,
    })
}

fn short(sha: &HeadSha) -> &str {
    &sha.as_str()[..12]
}

/// Which paths are CI files: **a rule on the path alone** (ADR 0124, amended 2026-11-21). No file
/// is read to decide it: five rounds each found another input on which a YAML reader here and
/// GitLab's loader disagree, so what a pipeline includes is never worked out.
struct Matcher {
    /// A path that starts with one of these.
    prefixes: Vec<&'static str>,
    /// A path that is exactly one of these.
    exact: Vec<String>,
    /// Every file whose name ends in `.yml` or `.yaml`, in any case, anywhere in the tree. A
    /// GitLab `local:` include must be one, so this covers every include without reading any.
    any_yaml: bool,
    /// A file named `action.yml` or `action.yaml`, in any case, anywhere: a local action.
    any_action: bool,
}

/// Files beside the workflows that decide what runs or who approves it, on both forges.
const ALWAYS: &[&str] = &[
    "renovate.json",
    "renovate.json5",
    ".renovaterc",
    ".renovaterc.json",
    ".renovaterc.json5",
    ".gitlab/renovate.json",
    ".gitlab/renovate.json5",
    "CODEOWNERS",
    "docs/CODEOWNERS",
    ".gitlab/CODEOWNERS",
];

/// GitLab checks `File.basename(location).match?(/.+\.(yml|yaml)$/i)`, and Ruby's `$` matches at
/// any line end, so a basename is YAML if any `\n`-separated line of it ends in `.yml` or
/// `.yaml` (`ci/a.yml\nb` is). A name with a line break is then refused by the control-character
/// check at prepare time.
fn has_yaml_name(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.split('\n').any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.ends_with(".yml") || lower.ends_with(".yaml")
    })
}

impl Matcher {
    /// **GitHub**: everything under `.github/` (a workflow can reference a local action, a
    /// reusable workflow, a script or a template there, and Dependabot's and CODEOWNERS' files
    /// are there too), and a file named `action.yml` or `action.yaml` anywhere, which a
    /// `uses: ./path` can name. Dependency bots' configuration and CODEOWNERS elsewhere.
    fn github() -> Matcher {
        Matcher {
            prefixes: vec![".github/"],
            // `.github` itself (a symlink or a gitlink there) is an entry too.
            exact: ALWAYS
                .iter()
                .chain(&[".github"])
                .map(|path| (*path).to_owned())
                .collect(),
            any_yaml: false,
            any_action: true,
        }
    }

    /// **GitLab**: any `.yml` or `.yaml` file anywhere, the default pipeline file, and the
    /// project's configured pipeline file, whatever it is called. Beside the files both forges
    /// hold.
    fn gitlab(ci_config: Option<&str>) -> Matcher {
        let mut matcher = Matcher::github();
        matcher.any_yaml = true;
        matcher.exact.push(GITLAB_DEFAULT.to_owned());
        if let Some(path) = ci_config {
            // Skipped: `path@group/project` and an address, files this repository does not
            // hold. Every in-repo form of `ci_config_path` is YAML-named and so held by
            // `any_yaml` anyway; the exact entry only keeps the intent visible.
            if !path.contains('@') && !path.contains("://") && !path.is_empty() {
                matcher.exact.push(path.trim_start_matches('/').to_owned());
            }
        }
        matcher
    }

    fn matches(&self, path: &str) -> bool {
        let lower = path.to_ascii_lowercase();
        self.prefixes.iter().any(|prefix| lower.starts_with(prefix))
            || self
                .exact
                .iter()
                .any(|exact| exact.eq_ignore_ascii_case(path))
            || (self.any_yaml && has_yaml_name(path))
            || (self.any_action
                && path.rsplit('/').next().is_some_and(|name| {
                    name.eq_ignore_ascii_case("action.yml")
                        || name.eq_ignore_ascii_case("action.yaml")
                }))
    }
}

/// The most bytes of a listing of a tree's files, or of the paths a diff changes.
pub(super) const MOST_LISTING: usize = 16 * 1024 * 1024;

/// **The trees of a clone, read by git** (the standard tool): the diff of two commits' trees, the
/// patch of some paths of it, and a file at a commit.
pub struct GitTrees {
    clone: std::path::PathBuf,
}

impl GitTrees {
    /// Trees of the repository at `store`, **which only purlis writes**: the quarantine
    /// ([`super::Quarantine`]) or a test's own repository. **Never a chat's clone**, whose objects
    /// a chat can overwrite so that this shows other lines than the commit the forge receives;
    /// `no_chat_clone_is_read_for_a_held_act` fails on any other use in the source.
    pub fn of_trusted_store(store: impl Into<std::path::PathBuf>) -> GitTrees {
        let clone = store;
        GitTrees {
            clone: clone.into(),
        }
    }
}

impl GitTrees {
    /// git, read with nothing of the clone's own to say about what it shows: **replace refs are
    /// not followed** (a `refs/replace/<sha>` makes git read another object for a commit's id, so
    /// a tree diff of the real commits would be empty), and every call that shows a patch is
    /// `--text` (a `-diff` attribute in `.git/info/attributes` or a `.gitattributes` makes git
    /// print "Binary files differ" for a file whose lines are the change).
    ///
    /// Its output is held to `most` bytes: past them git is stopped and the answer is an error, so
    /// a huge patch, listing or blob is never held whole.
    pub(super) fn git_capped(&self, args: &[&str], most: usize) -> Result<String, String> {
        self.git_capped_bytes(args, most)
            .map(|out| String::from_utf8_lossy(&out).into_owned())
    }

    /// [`Self::git_capped`], the answer as git wrote it.
    fn git_capped_bytes(&self, args: &[&str], most: usize) -> Result<Vec<u8>, String> {
        use crate::worktree::git;
        let mut argv = vec![
            "--no-replace-objects",
            "-c",
            "core.attributesFile=/dev/null",
        ];
        argv.extend_from_slice(args);
        let done = git::run_capped_bytes(&self.clone, &argv, git::READ, most)
            .map_err(|why| why.to_string())?;
        let Some(done) = done else {
            return Err(format!(
                "git's answer is more than {most} bytes, more than purlis can show"
            ));
        };
        if done.code == Some(0) {
            Ok(done.out)
        } else {
            Err(String::from_utf8_lossy(&done.err)
                .trim()
                .lines()
                .next()
                .unwrap_or("git failed")
                .to_owned())
        }
    }
}

/// **No objects to read**: the trees of a held act that has no push's quarantine, which is every
/// act but a CI-file push. Every question is an error, never a read of a clone.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoClone;

impl Trees for NoClone {
    fn changed(&self, _: &HeadSha, _: &HeadSha) -> Result<Vec<String>, String> {
        Err("no clone".to_owned())
    }
    fn patch(&self, _: &HeadSha, _: &HeadSha, _: &[String]) -> Result<String, String> {
        Err("no clone".to_owned())
    }
}

impl Trees for GitTrees {
    fn changed(&self, against: &HeadSha, tip: &HeadSha) -> Result<Vec<String>, String> {
        // Both objects must be here: a commit the clone lacks has no tree to compare.
        for sha in [against, tip] {
            self.git_capped(
                &["cat-file", "-e", &format!("{}^{{commit}}", sha.as_str())],
                1024,
            )
            .map_err(|why| format!("the clone has no commit {}: {why}", short(sha)))?;
        }
        let out = self.git_capped(
            &[
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
            ],
            MOST_LISTING,
        )?;
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
        // Stopped past what can be shown, never buffered whole first.
        self.git_capped(&args, MOST_DIFF + 1).and_then(|out| {
            if out.len() > MOST_DIFF {
                Err(format!(
                    "the diff is more than purlis can show ({} KiB at most)",
                    MOST_DIFF / 1024
                ))
            } else {
                Ok(out)
            }
        })
    }

    fn message(&self, tip: &HeadSha) -> Result<String, String> {
        super::TipFiles::message(self, tip)
    }

    fn missing(&self, base: &HeadSha, tip: &HeadSha) -> Result<usize, String> {
        self.commits_missing(base, tip)
    }
}

impl GitTrees {
    /// How many commits `base` reaches that `tip` does not, as git counts them. A commit either
    /// side lacks is an error, never a count.
    pub(super) fn commits_missing(&self, base: &HeadSha, tip: &HeadSha) -> Result<usize, String> {
        let out = self
            .git_capped(
                &["rev-list", "--count", base.as_str(), "--not", tip.as_str()],
                64,
            )
            .map_err(|why| {
                format!(
                    "purlis could not tell whether {} contains {}: {why}",
                    short(tip),
                    short(base)
                )
            })?;
        out.trim()
            .parse()
            .map_err(|_| "git's count of commits was not a number".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::Matcher;

    #[test]
    fn a_basename_with_a_line_ending_in_yaml_is_yaml_to_gitlab_whatever_follows_the_line_break() {
        let gitlab = Matcher::gitlab(None);
        for path in [
            "ci/a.yml\nb",
            "ci/a.YAML\nb\nc",
            "ci/x\na.yml",
            "ci/a.yml\n",
        ] {
            assert!(gitlab.matches(path), "{path:?}");
        }
        for path in ["ci/a.txt\nb", "ci.yml/b", "ci/yml\n", "ci/a.yml.txt"] {
            assert!(!gitlab.matches(path), "{path:?}");
        }
    }

    #[test]
    fn dot_github_itself_and_the_always_list_match_in_any_case_on_both_forges() {
        for matcher in [Matcher::github(), Matcher::gitlab(None)] {
            for path in [
                ".github",
                ".GitHub",
                ".GITHUB/workflows/x",
                "codeowners",
                "Docs/CodeOwners",
                "RENOVATE.JSON5",
                ".gitlab/Renovate.json5",
                ".gitlab/renovate.json5",
            ] {
                assert!(matcher.matches(path), "{path}");
            }
            assert!(
                !matcher.matches("docs/.github"),
                "nested .github is not the entry"
            );
            assert!(!matcher.matches(".githubx"));
        }
    }
}
