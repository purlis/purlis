// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A request's head, fetched for its review** (RC-2's kind (f), #704; ADR 0084 §1–§2).
//!
//! A review of a request compares the merge base of its target branch and its head with its
//! head, and git can compare only objects it has. So the head is fetched into the workspace's
//! clone, under [`NAMESPACE`], and compared there as two refs ([`super::Comparison::Refs`]).
//!
//! **Through the runner's network entry point, and nothing else** ([`git::run_network`]): the
//! fetch takes the runner's one fixed set of overrides, so it authenticates with the credential
//! the caller names or none, never with a program the repo's configuration names, and the
//! network log lists it as a git call. Nothing here adds an override of its own. What the call
//! asks for is fixed too: no tags, no submodules, no `FETCH_HEAD`, no automatic maintenance,
//! and no configured refspec (`--refmap=`), so the one ref it writes is the review's.
//!
//! **Named, never given as text git reads as an option.** The address, the request's ref and
//! the review's key are each checked before git is started: the address cannot start with `-`
//! and is passed after `--`, the request's ref is a full ref git accepts, and the key is a short
//! plain name.
//!
//! **Removed when nothing needs it** ([`keep_only`], [`forget`]). The refs are the clone's own
//! git (tier None, ADR 0084's tiers table), and the only record of a review's head that purlis
//! keeps: what decides that a review is still needed (an open Review tab, a draft) hands the
//! keys it still needs, and every other ref under [`NAMESPACE`] is deleted.

use std::path::Path;

use crate::worktree::{self, git};

/// Where a request's fetched head is kept in the workspace's clone: `refs/purlis/review/<key>`.
pub const NAMESPACE: &str = "refs/purlis/review/";

/// The longest key a review's ref takes.
pub const LONGEST_KEY: usize = 64;

/// What refused a review fetch, or the removal of its refs.
#[derive(Debug, thiserror::Error)]
pub enum Refused {
    #[error(transparent)]
    Clone(#[from] worktree::Refusal),
    #[error(
        "'{0}' cannot name a review: use letters, digits, '.', '_' and '-', starting with a \
         letter or a digit, at most 64 of them"
    )]
    BadKey(String),
    #[error("'{0}' is not an address purlis fetches a request from")]
    BadAddress(String),
    #[error("'{0}' is not a full ref git accepts, such as refs/pull/7/head")]
    BadHead(String),
    #[error(transparent)]
    Git(#[from] git::GitUnavailable),
    #[error("git did not fetch the request's head: {0}")]
    NotFetched(String),
    #[error("git did not change the review's refs: {0}")]
    NotChanged(String),
}

/// A request's head, fetched: the ref it is kept under and the commit it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// `refs/purlis/review/<key>`: what [`super::Comparison::Refs`] is handed as `to`.
    pub reference: String,
    /// The commit, by its full hex id.
    pub commit: String,
}

/// The ref a review's head is kept under.
pub fn reference(key: &str) -> Result<String, Refused> {
    if !key_ok(key) {
        return Err(Refused::BadKey(key.to_string()));
    }
    Ok(format!("{NAMESPACE}{key}"))
}

/// A key is a short plain name: one path component, nothing git or a shell reads specially.
fn key_ok(key: &str) -> bool {
    key.len() <= LONGEST_KEY
        && key
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !key.contains("..")
        && !key.ends_with(".lock")
        && !key.ends_with('.')
}

/// The request's ref as the fetch's source: a full ref (`refs/…`), as git checks one, and no
/// pattern.
fn head_ok(head: &str) -> bool {
    head.starts_with("refs/")
        && !head.contains(['*', ':', '+', '^', '~', '?', '[', '\\', ' ', '\t', '\n'])
        && !head.contains("..")
        && !head.contains("@{")
        && !head.ends_with('/')
        && !head.ends_with(".lock")
        && !head.ends_with('.')
        && head
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.'))
        && !head.chars().any(char::is_control)
}

/// Fetch the request's head `head` (a full ref on the far side, `refs/pull/7/head`) from
/// `address` into the clone of `repo` in workspace `ws`, under [`reference`]`(key)`, with the
/// credential helper `helper` alone, or none. A head already fetched under `key` is moved to
/// where the request's head is now.
pub fn fetch(
    plane: &Path,
    ws: &str,
    repo: &str,
    address: &str,
    head: &str,
    key: &str,
    helper: Option<&str>,
) -> Result<Fetched, Refused> {
    let reference = reference(key)?;
    if address.is_empty() || address.starts_with('-') || address.contains(['\n', '\0']) {
        return Err(Refused::BadAddress(address.to_string()));
    }
    if !head_ok(head) {
        return Err(Refused::BadHead(head.to_string()));
    }
    let clone = worktree::clone_dir(plane, ws, repo)?;
    let refspec = format!("+{head}:{reference}");
    let fetched = git::run_network(
        &clone,
        helper,
        &[
            "fetch",
            "--quiet",
            "--no-tags",
            "--no-recurse-submodules",
            "--no-write-fetch-head",
            "--no-auto-maintenance",
            "--refmap=",
            "--",
            address,
            &refspec,
        ],
    )?;
    if !fetched.ok() {
        return Err(Refused::NotFetched(said(&fetched)));
    }
    let named = git::run(
        &clone,
        &["rev-parse", "--verify", "--end-of-options", &reference],
        git::READ,
    )?;
    if !named.ok() {
        return Err(Refused::NotFetched(said(&named)));
    }
    Ok(Fetched {
        reference,
        commit: named.out.trim().to_string(),
    })
}

/// The keys of the reviews whose heads the clone of `repo` keeps, sorted.
pub fn kept(plane: &Path, ws: &str, repo: &str) -> Result<Vec<String>, Refused> {
    let clone = worktree::clone_dir(plane, ws, repo)?;
    let listed = git::run(
        &clone,
        &["for-each-ref", "--format=%(refname)", NAMESPACE],
        git::READ,
    )?;
    if !listed.ok() {
        return Err(Refused::NotChanged(said(&listed)));
    }
    let mut keys: Vec<String> = listed
        .out
        .lines()
        .filter_map(|line| line.strip_prefix(NAMESPACE))
        .map(str::to_string)
        .collect();
    keys.sort();
    Ok(keys)
}

/// Delete the review head kept under `key`. Nothing kept there is not an error.
pub fn forget(plane: &Path, ws: &str, repo: &str, key: &str) -> Result<(), Refused> {
    let reference = reference(key)?;
    let clone = worktree::clone_dir(plane, ws, repo)?;
    delete(&clone, &[reference])
}

/// Delete every review head the clone of `repo` keeps but `needed`, and answer the keys deleted.
/// What a review still needs (an open Review tab, a draft) is the caller's to say.
pub fn keep_only(
    plane: &Path,
    ws: &str,
    repo: &str,
    needed: &[&str],
) -> Result<Vec<String>, Refused> {
    let unneeded: Vec<String> = kept(plane, ws, repo)?
        .into_iter()
        .filter(|key| !needed.contains(&key.as_str()))
        .collect();
    if unneeded.is_empty() {
        return Ok(unneeded);
    }
    let clone = worktree::clone_dir(plane, ws, repo)?;
    let references: Vec<String> = unneeded
        .iter()
        .map(|key| format!("{NAMESPACE}{key}"))
        .collect();
    delete(&clone, &references)?;
    Ok(unneeded)
}

/// Delete `references` in one transaction, each only where it is a ref of [`NAMESPACE`].
fn delete(clone: &Path, references: &[String]) -> Result<(), Refused> {
    let mut input = String::from("start\n");
    for reference in references {
        debug_assert!(reference.starts_with(NAMESPACE));
        input.push_str(&format!("delete {reference}\n"));
    }
    input.push_str("prepare\ncommit\n");
    let deleted = git::run_with_input(
        clone,
        &["update-ref", "--stdin"],
        input.into_bytes(),
        git::READ,
    )?;
    if deleted.code != Some(0) {
        return Err(Refused::NotChanged(
            String::from_utf8_lossy(&deleted.err).trim().to_string(),
        ));
    }
    Ok(())
}

/// What git said of a failed call, on one line.
fn said(run: &git::Run) -> String {
    let text = if run.err.trim().is_empty() {
        &run.out
    } else {
        &run.err
    };
    text.trim().lines().last().unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_one_short_plain_name() {
        for good in ["github-7", "pr.12", "a", "x_y-z"] {
            assert!(key_ok(good), "{good}");
        }
        let long = "a".repeat(LONGEST_KEY + 1);
        for bad in [
            "", "-x", ".x", "a/b", "a..b", "a.lock", "a.", "a b", "a:b", "a\nb", &long,
        ] {
            assert!(!key_ok(bad), "{bad:?}");
            assert!(matches!(reference(bad), Err(Refused::BadKey(_))), "{bad:?}");
        }
        assert_eq!(reference("pr-7").unwrap(), "refs/purlis/review/pr-7");
    }

    #[test]
    fn a_head_is_a_full_ref_and_never_a_pattern_or_a_refspec() {
        for good in [
            "refs/pull/7/head",
            "refs/merge-requests/3/head",
            "refs/heads/main",
        ] {
            assert!(head_ok(good), "{good}");
        }
        for bad in [
            "main",
            "refs/pull/*/head",
            "refs/pull/7/head:refs/heads/main",
            "+refs/pull/7/head",
            "refs/pull/../head",
            "refs//x",
            "refs/.hidden/x",
            "refs/x.lock",
            "refs/x/",
            "refs/x@{1}",
            "refs/x\n",
            "--upload-pack=x",
        ] {
            assert!(!head_ok(bad), "{bad:?}");
        }
    }

    #[test]
    fn an_address_that_git_would_read_as_an_option_is_refused_before_anything_runs() {
        let plane = tempfile::tempdir().unwrap();
        for address in ["", "--upload-pack=touch x", "-u", "a\nb"] {
            assert!(
                matches!(
                    fetch(
                        plane.path(),
                        "alpha",
                        "thing",
                        address,
                        "refs/pull/7/head",
                        "pr-7",
                        None
                    ),
                    Err(Refused::BadAddress(_))
                ),
                "{address:?}"
            );
        }
    }
}
