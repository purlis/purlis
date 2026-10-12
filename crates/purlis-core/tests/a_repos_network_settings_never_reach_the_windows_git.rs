// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's network git uses only your own network settings, never a repository's.
//!
//! A chat can write the config of the repository it works in, and the window's network git
//! (`run_network`) reads that config. So the fixture plants, in the repository's own config, one
//! network setting at a time: a proxy, TLS verification off, a CA bundle, an extra header for one
//! address, and a rewrite of the forge's address to another host. For each, the window's git
//! either does not use it, or is refused with a sentence naming the kind of setting and not its
//! value.
//!
//! **Whether git uses a setting** is asked of git itself, on the window's own command line:
//! `git config --get-urlmatch <key> <url>` answers with the value git's HTTP transport would use
//! for that URL, by the same matching. Nothing here reaches a network. The control asks plain git
//! the same and sees the planted value.
//!
//! Two layouts: a clone whose `.git` is a folder, and a work tree whose git directory is kept
//! beside it.

mod support;

use std::path::{Path, PathBuf};

use purlis_core::worktree::git;

/// The forge's address, as the window's git would fetch it. Never connected to.
const URL: &str = "https://forge.example/o/r.git";

#[derive(Clone, Copy)]
enum Layout {
    Clone,
    KeptBeside,
}

struct Repo {
    _dir: tempfile::TempDir,
    work: PathBuf,
}

fn repo(layout: Layout) -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let case = std::fs::canonicalize(dir.path()).unwrap();
    let work = case.join("work");
    match layout {
        Layout::Clone => {
            std::fs::create_dir_all(&work).unwrap();
            support::git(&work, &["init", "-q", "-b", "main", "."]);
        }
        Layout::KeptBeside => {
            let store = case.join("store");
            support::git(
                &case,
                &[
                    "init",
                    "-q",
                    "-b",
                    "main",
                    &format!("--separate-git-dir={}", store.display()),
                    "work",
                ],
            );
        }
    }
    Repo { _dir: dir, work }
}

/// What the window's git answers for `key` at [`URL`]: `Ok(value)`, or `Err(sentence)` where the
/// call was refused before git ran.
fn windows_value(work: &Path, key: &str) -> Result<String, String> {
    let run =
        git::run_network(work, None, &["config", "--get-urlmatch", key, URL]).expect("git starts");
    match run.code {
        // `--get-urlmatch` exits 1 where nothing is set.
        Some(0 | 1) => Ok(run.out.trim_end_matches('\n').to_string()),
        _ => Err(run.err),
    }
}

/// What plain git, with no global or system config, answers for the same.
fn plain_value(work: &Path, key: &str) -> String {
    let mut cmd = support::unsigned();
    cmd.arg("-C")
        .arg(work)
        .args(["config", "--get-urlmatch", key, URL])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    let out = purlis_core::forklock::output(&mut cmd).expect("git runs");
    String::from_utf8_lossy(&out.stdout)
        .trim_end_matches('\n')
        .to_string()
}

/// The planted settings: the key planted, its value, the key git's transport reads it as, and a
/// word the refusal names its kind by.
const PLANTED: [(&str, &str, &str, &str); 4] = [
    ("http.proxy", "http://127.0.0.1:9", "http.proxy", "proxy"),
    ("http.sslVerify", "false", "http.sslverify", "TLS"),
    (
        "http.sslCAInfo",
        "/planted/ca.pem",
        "http.sslcainfo",
        "certificate",
    ),
    (
        "http.https://forge.example/.extraHeader",
        "X-Planted: yes",
        "http.extraheader",
        "header",
    ),
];

fn each_planted_setting_is_ignored_or_refused_in(layout: Layout) {
    for (key, value, read_as, kind) in PLANTED {
        let repo = repo(layout);
        support::git(&repo.work, &["config", key, value]);
        // The control: git reads what was planted.
        assert_eq!(
            plain_value(&repo.work, read_as),
            value,
            "the plant of {key}"
        );

        match windows_value(&repo.work, read_as) {
            Ok(seen) => assert_ne!(seen, value, "the window's git used the repo's {key}"),
            Err(said) => {
                assert!(
                    said.contains("this repository's own git config") && said.contains(kind),
                    "{key}: {said}"
                );
                assert!(
                    !said.contains(value),
                    "the refusal repeats the value: {said}"
                );
            }
        }
    }
}

#[test]
fn a_clones_network_settings_are_ignored_or_refused() {
    purlis_core::unsteered!();
    each_planted_setting_is_ignored_or_refused_in(Layout::Clone);
}

#[test]
fn a_kept_beside_git_directorys_network_settings_are_ignored_or_refused() {
    purlis_core::unsteered!();
    each_planted_setting_is_ignored_or_refused_in(Layout::KeptBeside);
}

/// TLS verification off and an extra header, the two a command line can always give back as git's
/// own defaults: ignored, not refused, wherever your own config says nothing of them.
#[test]
fn tls_off_and_an_extra_header_are_held_where_your_config_is_silent() {
    purlis_core::unsteered!();
    let repo = repo(Layout::KeptBeside);
    let yours = |key: &str| {
        let run = git::run(
            Path::new("/"),
            &["config", "--global", "--get-regexp", key],
            git::READ,
        )
        .expect("git starts");
        run.out.is_empty()
    };
    support::git(&repo.work, &["config", "http.sslVerify", "false"]);
    support::git(
        &repo.work,
        &[
            "config",
            "http.https://forge.example/.extraHeader",
            "X-Planted: yes",
        ],
    );
    if yours("^http\\..*sslverify$") {
        assert_eq!(
            windows_value(&repo.work, "http.sslverify"),
            Ok("true".into())
        );
    }
    if yours("^http\\..*extraheader$") {
        assert_eq!(
            windows_value(&repo.work, "http.extraheader"),
            Ok(String::new())
        );
    }
}

/// A rewrite of the forge's address to another host: the window's git is refused before it
/// connects anywhere, so the credential it carries is never offered at the other host. The
/// control: plain git rewrites the address (`ls-remote --get-url` prints it and connects
/// nowhere). Each call is also told not to use HTTPS, so that none connects even where a runner
/// did not refuse it.
fn a_rewrite_to_another_host_is_refused_in(layout: Layout) {
    let repo = repo(layout);
    support::git(
        &repo.work,
        &[
            "config",
            "url.https://elsewhere.example/.insteadOf",
            "https://forge.example/",
        ],
    );
    let mut control = support::unsigned();
    control
        .arg("-C")
        .arg(&repo.work)
        .args(["ls-remote", "--get-url", URL])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    let out = purlis_core::forklock::output(&mut control).expect("git runs");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "https://elsewhere.example/o/r.git",
        "the plant of the rewrite"
    );

    let offline = ["-c", "protocol.https.allow=never"];
    for verb in [
        &["ls-remote", "--get-url", URL][..],
        &["ls-remote", URL],
        &["fetch", URL],
        &["push", URL, "HEAD:main"],
    ] {
        let args: Vec<&str> = offline.iter().chain(verb).copied().collect();
        let run = git::run_network(&repo.work, Some("!false"), &args).expect("git starts");
        assert!(
            !run.ok()
                && run
                    .err
                    .contains("rewrites a network address to another host"),
            "{args:?}: {run:?}"
        );
        assert!(!run.err.contains("elsewhere"), "{args:?}: {run:?}");
    }
}

#[test]
fn a_clones_rewrite_to_another_host_is_refused() {
    purlis_core::unsteered!();
    a_rewrite_to_another_host_is_refused_in(Layout::Clone);
}

#[test]
fn a_kept_beside_git_directorys_rewrite_to_another_host_is_refused() {
    purlis_core::unsteered!();
    a_rewrite_to_another_host_is_refused_in(Layout::KeptBeside);
}
