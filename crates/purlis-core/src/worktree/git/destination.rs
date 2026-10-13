// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where a network call goes, read the way git reads it, for the network log (#1853, #1828).
//!
//! - **A push that names no remote** goes to `branch.<branch>.pushRemote`, else
//!   `remote.pushDefault`, else `branch.<branch>.remote`, else `origin`: git's own order. A
//!   fetch's default (`ls-remote --get-url` with no name) reads only the last two.
//! - **A URL given on the command line** is rewritten as git rewrites it: the longest
//!   `url.<base>.insteadOf` prefix it starts with is replaced by its base, and a push takes the
//!   longest `url.<base>.pushInsteadOf` first, falling back to `insteadOf` where none matches.
//!
//! Both are read from config only (`git config --get-regexp`), which connects to nothing.

/// The remote a push that names none goes to, given the branch checked out (`None` when HEAD
/// is detached) and `config`, the `(key, value)` entries git lists, in the order it read them
/// (the last of a key wins). Sections and variables are lowercased as git lists them; the
/// branch name is compared as written.
pub(super) fn push_remote(branch: Option<&str>, config: &[(String, String)]) -> String {
    let last = |wanted: &str| {
        config
            .iter()
            .rev()
            .find(|(key, value)| key == wanted && !value.is_empty())
            .map(|(_, value)| value.clone())
    };
    let of_branch = |variable: &str| branch.and_then(|b| last(&format!("branch.{b}.{variable}")));
    of_branch("pushremote")
        .or_else(|| last("remote.pushdefault"))
        .or_else(|| of_branch("remote"))
        .unwrap_or_else(|| "origin".to_owned())
}

/// `url` as git reaches it once its rewrites are applied: `config` holds the
/// `url.<base>.insteadof` and `url.<base>.pushinsteadof` entries, in the order git read them.
/// For a push, the longest matching `pushInsteadOf` wins and `insteadOf` is the fallback; for
/// anything else only `insteadOf` is read. Of two equally long prefixes the first read wins.
pub(super) fn rewritten(url: &str, config: &[(String, String)], push: bool) -> String {
    let longest = |variable: &str| {
        let mut best: Option<(&str, usize)> = None;
        for (key, prefix) in config {
            let Some(base) = key
                .strip_prefix("url.")
                .and_then(|rest| rest.strip_suffix(variable))
                .and_then(|rest| rest.strip_suffix('.'))
            else {
                continue;
            };
            if url.starts_with(prefix.as_str()) && best.is_none_or(|(_, len)| prefix.len() > len) {
                best = Some((base, prefix.len()));
            }
        }
        best.map(|(base, len)| format!("{base}{}", &url[len..]))
    };
    push.then(|| longest("pushinsteadof"))
        .flatten()
        .or_else(|| longest("insteadof"))
        .unwrap_or_else(|| url.to_owned())
}

/// The `(key, value)` entries of `git config -z --get-regexp`: `key\nvalue\0` each.
pub(super) fn entries(listed: &str) -> Vec<(String, String)> {
    listed
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let (key, value) = entry.split_once('\n').unwrap_or((entry, ""));
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn a_push_naming_no_remote_goes_where_git_sends_it() {
        let all = config(&[
            ("branch.main.remote", "fetched"),
            ("remote.pushdefault", "default"),
            ("branch.main.pushremote", "pushed"),
        ]);
        assert_eq!(push_remote(Some("main"), &all), "pushed");
        // Without the branch's own push remote, the repository's push default.
        let all = config(&[
            ("branch.main.remote", "fetched"),
            ("remote.pushdefault", "default"),
        ]);
        assert_eq!(push_remote(Some("main"), &all), "default");
        // Then the branch's remote, then `origin`.
        let all = config(&[("branch.main.remote", "fetched")]);
        assert_eq!(push_remote(Some("main"), &all), "fetched");
        assert_eq!(push_remote(Some("main"), &[]), "origin");
        // A detached HEAD reads no branch's keys, and another branch's are not this one's.
        let all = config(&[
            ("branch.main.pushremote", "pushed"),
            ("branch.Main.remote", "other"),
        ]);
        assert_eq!(push_remote(None, &all), "origin");
        assert_eq!(push_remote(Some("dev"), &all), "origin");
        // The last of a key wins, as git reads it.
        let all = config(&[
            ("remote.pushdefault", "first"),
            ("remote.pushdefault", "second"),
        ]);
        assert_eq!(push_remote(Some("main"), &all), "second");
        // A branch name with dots in it is still that branch.
        let all = config(&[("branch.release.1.pushremote", "rel")]);
        assert_eq!(push_remote(Some("release.1"), &all), "rel");
    }

    #[test]
    fn a_given_url_is_rewritten_as_git_rewrites_it_and_a_push_reads_its_own_rewrite_first() {
        let all = config(&[
            ("url.https://short.invalid/.insteadof", "https://a.invalid/"),
            (
                "url.https://long.invalid/x/.insteadof",
                "https://a.invalid/org/",
            ),
            (
                "url.https://push.invalid/.pushinsteadof",
                "https://a.invalid/",
            ),
            (
                "url.https://dots.in.base.invalid/.insteadof",
                "https://b.invalid/",
            ),
        ]);
        let url = "https://a.invalid/org/repo.git";
        // The longest matching prefix wins.
        assert_eq!(
            rewritten(url, &all, false),
            "https://long.invalid/x/repo.git"
        );
        // A push takes its own rewrite first.
        assert_eq!(
            rewritten(url, &all, true),
            "https://push.invalid/org/repo.git"
        );
        // A push with no push rewrite that matches falls back to insteadOf.
        assert_eq!(
            rewritten("https://b.invalid/r.git", &all, true),
            "https://dots.in.base.invalid/r.git"
        );
        // Nothing matches: the URL as given.
        assert_eq!(
            rewritten("https://c.invalid/r.git", &all, true),
            "https://c.invalid/r.git"
        );
        // Of two equally long prefixes, the first read.
        let tie = config(&[
            ("url.https://one.invalid/.insteadof", "https://a.invalid/"),
            ("url.https://two.invalid/.insteadof", "https://a.invalid/"),
        ]);
        assert_eq!(
            rewritten("https://a.invalid/r", &tie, false),
            "https://one.invalid/r"
        );
    }

    #[test]
    fn config_entries_read_as_git_lists_them() {
        assert_eq!(
            entries("remote.pushdefault\nup\0branch.main.remote\norigin\0"),
            config(&[
                ("remote.pushdefault", "up"),
                ("branch.main.remote", "origin")
            ])
        );
        assert!(entries("").is_empty());
    }
}
