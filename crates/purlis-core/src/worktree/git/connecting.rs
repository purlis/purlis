// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Which git calls can reach another repository, and so run only through the runner's network
//! entry point ([`super::run_network`]) (#1853).
//!
//! The one-credential rule, the network settings held to your own and the network log all live
//! on that entry point. A connecting verb started through any other runner would skip all three,
//! so the runner itself refuses one there, rather than leave it to review that no caller does.
//! A repository on this machine is reached through the network entry too: it holds the
//! same rule, gives no credential to a folder, and does not list one in the network log.
//!
//! **Kept on every runner: the local-only lookups** the network log itself uses, which ask
//! git where a remote is and connect to nothing: `ls-remote --get-url`, and `remote` reading
//! or writing its own config (`get-url`, `add` without a fetch, `set-url` and the rest).

/// The verbs that can reach another repository whatever they are told: they fetch, push,
/// clone, or speak a transport. `remote-<name>` is every remote helper.
const CONNECTING: [&str; 15] = [
    "fetch",
    "pull",
    "push",
    "clone",
    "fetch-pack",
    "send-pack",
    "request-pull",
    "http-fetch",
    "http-push",
    "backfill",
    "maintenance",
    "imap-send",
    "send-email",
    "svn",
    "p4",
];

/// What `git submodule` does that fetches or clones a submodule.
const SUBMODULE_FETCHES: [&str; 2] = ["update", "add"];

/// What `git lfs` does that reaches nothing but this machine.
const LFS_STAYS_HERE: [&str; 6] = ["version", "env", "ls-files", "status", "pointer", "fsck"];

/// Whether a call whose verb starts `from` can reach another repository, and so is run only
/// through the network entry point. `ls-remote --get-url` and a `remote` that only reads or
/// writes its config connect to nothing.
pub(super) fn connects(from: &[&str]) -> bool {
    let Some((verb, rest)) = from.split_first() else {
        return false;
    };
    let sub = || rest.iter().copied().find(|word| !word.starts_with('-'));
    match *verb {
        "ls-remote" => !rest.contains(&"--get-url"),
        "remote" => {
            super::reaches_a_remote(from)
                || (sub() == Some("add") && rest.iter().any(|w| matches!(*w, "-f" | "--fetch")))
        }
        "submodule" => sub().is_some_and(|sub| SUBMODULE_FETCHES.contains(&sub)),
        "archive" => rest.iter().any(|word| word.starts_with("--remote")),
        // Whatever git-lfs may send or fetch: only what reads this machine alone stays off it.
        "lfs" => sub().is_none_or(|sub| !LFS_STAYS_HERE.contains(&sub)),
        verb if verb.starts_with("remote-") => true,
        verb if verb.starts_with("cvs") => true,
        verb => CONNECTING.contains(&verb),
    }
}

/// What a connecting call started outside the network entry point answers.
pub(super) const OFF_THE_NETWORK_ENTRY: &str = "this git command can reach another repository, \
    which purlis runs only through its network entry point, so git was not run here";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_verb_that_can_reach_another_repository_connects() {
        for asks in [
            &["fetch", "origin"][..],
            &["fetch", "--prune"],
            &["pull", "--ff-only"],
            &["push", "-q", "origin", "main"],
            &["clone", "--", "/a/folder", "/b"],
            &["clone", "https://example.invalid/x.git"],
            &["ls-remote", "origin"],
            &["lfs", "push", "origin", "main"],
            &["lfs", "pull"],
            &["lfs", "fetch", "--all"],
            &["ls-remote", "--heads", "https://example.invalid/x.git"],
            &["remote", "update"],
            &["remote", "show", "origin"],
            &["remote", "prune", "origin"],
            &["remote", "set-head", "origin", "-a"],
            &["remote", "add", "-f", "up", "https://example.invalid/x.git"],
            &[
                "remote",
                "add",
                "--fetch",
                "up",
                "https://example.invalid/x.git",
            ],
            &["submodule", "update", "--init"],
            &["submodule", "add", "https://example.invalid/x.git"],
            &["archive", "--remote=origin", "HEAD"],
            &["archive", "--remote", "origin", "HEAD"],
            &["fetch-pack", "https://example.invalid/x.git"],
            &["send-pack", "https://example.invalid/x.git"],
            &["request-pull", "v1", "https://example.invalid/x.git"],
            &["remote-https", "origin", "https://example.invalid/x.git"],
            &["remote-ext", "origin", "sh -c x"],
            &["backfill"],
            &["maintenance", "run", "--task=prefetch"],
            &["svn", "fetch"],
            &["cvsimport"],
        ] {
            assert!(connects(asks), "{asks:?}");
        }
    }

    #[test]
    fn the_local_only_lookups_connect_to_nothing() {
        for reads in [
            &["ls-remote", "--get-url"][..],
            &["ls-remote", "--get-url", "origin"],
            &["remote", "get-url", "--push", "origin"],
            &["remote", "add", "origin", "https://example.invalid/x.git"],
            &[
                "remote",
                "set-url",
                "origin",
                "https://example.invalid/x.git",
            ],
            &["remote", "remove", "origin"],
            &["remote", "-v"],
            &["remote"],
            &["submodule", "status"],
            &["submodule"],
            &["archive", "HEAD"],
            &["status"],
            &["config", "--get", "remote.origin.url"],
            &[],
        ] {
            assert!(!connects(reads), "{reads:?}");
        }
    }
}
