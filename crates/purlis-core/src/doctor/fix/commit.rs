// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One commit of a fix's own paths**: what `rename-plane` and `project-skills` share. A fix
//! that rewrites committed files for everyone checks the project can take a commit of its own
//! ([`committable`]), changes files in the working tree recording each in [`Work`], and commits
//! exactly those paths ([`commit_paths`]) through purlis's git runner, which runs no hooks.

use std::path::Path;

/// How long the fix waits for a save running in this process to let go of the project.
pub(super) const CLAIM_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// What the fix changed so far: each path it touched (relative to the project, as git spells
/// it), the ones it created, and the lines it says.
#[derive(Default)]
pub(super) struct Work {
    pub(super) touched: Vec<String>,
    pub(super) created: Vec<String>,
    pub(super) said: Vec<String>,
}

impl Work {
    pub(super) fn touch(&mut self, path: &str) {
        if !self.touched.iter().any(|p| p == path) {
            self.touched.push(path.to_owned());
        }
    }

    /// Put every touched path back as `HEAD` has it, and remove the paths the fix created.
    /// Answers what it did, for the refusal.
    pub(super) fn undo(&self, root: &Path) -> String {
        if self.touched.is_empty() {
            return "nothing was written".to_owned();
        }
        let mut args = vec!["reset", "-q", "--"];
        args.extend(self.touched.iter().map(String::as_str));
        let reset = git(root, &args);
        let in_head: Vec<&str> = self
            .touched
            .iter()
            .filter(|p| !self.created.contains(p))
            .map(String::as_str)
            .collect();
        let mut restored = reset.is_ok();
        if !in_head.is_empty() {
            let mut args = vec!["checkout", "-q", "HEAD", "--"];
            args.extend(in_head);
            restored &= git(root, &args).is_ok();
        }
        for path in &self.created {
            let at = root.join(path);
            if at.exists() {
                restored &= std::fs::remove_file(at).is_ok();
            }
        }
        if restored {
            "every file it changed was put back as it was".to_owned()
        } else {
            format!(
                "not every file could be put back — `git status` shows what is left; `git \
                 checkout HEAD -- {}` restores them",
                self.touched.join(" ")
            )
        }
    }
}

/// Why a fix that makes one commit of its own (`what`, as "the rename") may not make it in the
/// project at `root`, or `Ok`: the project must be the top of a git repository, and hold no
/// tracked change that is not committed, so the commit holds that fix's change and nothing else.
pub(super) fn committable(root: &Path, what: &str) -> Result<(), String> {
    let top = git(root, &["rev-parse", "--show-toplevel"]).map_err(|_| {
        format!(
            "this project is not a git repository, so there is no commit to make {what} in; \
             nothing was written"
        )
    })?;
    let same = |a: &Path, b: &Path| match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same(Path::new(top.trim()), root) {
        return Err(format!(
            "this project is inside the git repository at {}, not at its top, so a commit there \
             would carry more than this project; nothing was written",
            top.trim()
        ));
    }
    in_the_middle(root, what)?;
    let status = git(root, &["status", "--porcelain=v1", "-uno"])
        .map_err(|why| format!("git could not say whether this project has changes ({why})"))?;
    let changed: Vec<&str> = status.lines().filter(|l| !l.trim().is_empty()).collect();
    if !changed.is_empty() {
        let shown: Vec<String> = changed
            .iter()
            .take(5)
            .map(|l| l.get(3..).unwrap_or(l).to_owned())
            .collect();
        let more = changed.len().saturating_sub(shown.len());
        let more = if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        };
        return Err(format!(
            "this project has changes that are not committed ({}{more}), and {what} must be a \
             commit of its own; nothing was written. Save or commit them first, then run the fix \
             again",
            shown.join(", ")
        ));
    }
    Ok(())
}

/// The git operations a repository can be left in the middle of, each by the file or folder in
/// its git directory that says so, and how a person says it.
const UNDER_WAY: [(&str, &str); 5] = [
    ("MERGE_HEAD", "a merge"),
    ("CHERRY_PICK_HEAD", "a cherry-pick"),
    ("REVERT_HEAD", "a revert"),
    ("rebase-merge", "a rebase"),
    ("rebase-apply", "a rebase"),
];

/// Why `what` may not be committed while the repository at `root` is in the middle of something,
/// or `Ok`: on a detached HEAD the commit would belong to no branch, and in the middle of a merge,
/// a rebase or a cherry-pick it would become part of that.
fn in_the_middle(root: &Path, what: &str) -> Result<(), String> {
    if git(root, &["symbolic-ref", "-q", "HEAD"]).is_err() {
        return Err(format!(
            "this project's git is on no branch (a detached HEAD), so {what} would be a commit \
             no branch holds; nothing was written. Check out a branch, then run the fix again"
        ));
    }
    for (marker, doing) in UNDER_WAY {
        let at = git(root, &["rev-parse", "--git-path", marker])
            .map_err(|why| format!("git could not say where its own files are ({why})"))?;
        let at = Path::new(at.trim());
        let at = if at.is_absolute() {
            at.to_path_buf()
        } else {
            root.join(at)
        };
        if at.symlink_metadata().is_ok() {
            return Err(format!(
                "this project's git is in the middle of {doing}, and {what} must not become part \
                 of it; nothing was written. Finish or abort it, then run the fix again"
            ));
        }
    }
    Ok(())
}

/// Every path git tracks in the project.
pub(super) fn tracked(root: &Path) -> Result<Vec<String>, String> {
    let out = git(root, &["ls-files", "-z"])
        .map_err(|why| format!("git could not list this project's files ({why})"))?;
    Ok(out
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect())
}

/// `text` with `requires` holding `feature` and `schema` at least 2, in the file's own layout,
/// or `None` when it already says both: how a fix marks a project that a build without
/// `feature` must open read-only.
pub(super) fn with_feature_named(text: &str, feature: &str) -> Result<Option<String>, String> {
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| format!("it is not valid TOML ({})", e.message()))?;
    let mut changed = false;
    let schema = doc.get("schema").and_then(toml_edit::Item::as_integer);
    if schema.is_none_or(|s| s < crate::compat::SCHEMA) {
        if doc.get("schema").is_some() && schema.is_none() {
            return Err("its `schema` is not a number".to_owned());
        }
        doc["schema"] = toml_edit::value(crate::compat::SCHEMA);
        changed = true;
    }
    let mut entry = toml_edit::InlineTable::new();
    entry.insert("feature", feature.into());
    entry.insert("since", env!("CARGO_PKG_VERSION").into());
    match doc.get_mut("requires") {
        None => {
            let mut list = toml_edit::Array::new();
            list.push(entry);
            doc["requires"] = toml_edit::value(list);
            changed = true;
        }
        Some(item) => {
            let Some(list) = item.as_array_mut() else {
                return Err("its `requires` is not a list".to_owned());
            };
            let there = list.iter().any(|e| {
                e.as_inline_table()
                    .and_then(|t| t.get("feature"))
                    .and_then(toml_edit::Value::as_str)
                    == Some(feature)
            });
            if !there {
                list.push(entry);
                changed = true;
            }
        }
    }
    Ok(changed.then(|| doc.to_string()))
}

/// The text of `rel` in the project at `root`, read with no link followed on the way to it or
/// at it: a fix reads only the project's own files.
pub(super) fn read(root: &Path, rel: &str) -> Result<String, String> {
    crate::contain::read_text_no_link(root, &root.join(rel))
        .map_err(|e| format!("could not read {rel} ({e})"))
}

/// `rel` in the project at `root` replaced whole with `text`, refused where it or a folder on the
/// way is a link ([`crate::rewrite::replace`]).
pub(super) fn write(root: &Path, rel: &str, text: &str) -> Result<(), String> {
    crate::rewrite::replace(
        root,
        &root.join(rel),
        text.as_bytes(),
        crate::rewrite::Mode::Kept,
    )
    .map_err(|e| format!("could not write {rel} ({e})"))
}

/// Stage exactly `paths` and commit them with the message `body`; purlis's git runner runs no
/// hooks. A path `git mv` took away is staged already, and is no pathspec `add` can match, so
/// only the paths there now are added. Answers the short sha, or `None` when the commit was made
/// and git could not name it. `Err` only when no commit was made.
pub(super) fn commit_paths(
    root: &Path,
    paths: &[&str],
    body: &str,
) -> Result<Option<String>, String> {
    let there: Vec<&str> = paths
        .iter()
        .copied()
        .filter(|p| root.join(p).symlink_metadata().is_ok())
        .collect();
    let mut args = vec!["add", "--"];
    args.extend(&there);
    let add = || {
        git(root, &args)
            .map(drop)
            .map_err(|why| format!("could not stage the change ({why})"))
    };
    // A folder a fix moves carries files it did not write, so each file committed is the one
    // name it has on disk, as a save's are (`planegit::ondisk`). With no path there, the add
    // stages nothing, and a look before it would be of the whole tree.
    if there.is_empty() {
        add()?;
    } else {
        crate::planegit::ondisk::add_checked(
            root,
            &there,
            "could not stage the change: a file in it is not the one file on disk",
            add,
        )?;
    }
    git(root, &["commit", "-q", "-m", body])
        .map_err(|why| format!("git did not make the commit ({why})"))?;
    Ok(git(root, &["rev-parse", "--short", "HEAD"])
        .ok()
        .map(|sha| sha.trim().to_owned()))
}

/// `git <args>` in `root`: its stdout, or what it said when it failed.
pub(super) fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let run = crate::worktree::git::run(root, args, crate::worktree::git::NETWORK)
        .map_err(|e| e.to_string())?;
    if run.ok() {
        Ok(run.out)
    } else if run.code.is_none() {
        Err("git did not answer in time".to_owned())
    } else {
        let said = run.err.trim();
        Err(if said.is_empty() {
            format!("git exited {}", run.code.unwrap_or(-1))
        } else {
            crate::shown::short(said)
        })
    }
}

#[cfg(test)]
mod tests;
