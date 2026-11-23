// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The forge's tips are read incrementally and within limits** (ADR 0128, amended 2026-11-23):
//! `FromForge` against a local bare repository standing in for the forge. Each tip lands in a
//! private ref of the quarantine, which is what git negotiates `have` lines from, so a later
//! chunk is sent only what is new; a run past its deadline or its size budget is refused.

use std::path::{Path, PathBuf};
use std::time::Duration;

use purlis_core::heldact::{BaseObjects, FetchLimits, FromForge};
use purlis_core::mergehold::HeadSha;

use crate::support::git;

fn sha_of(root: &Path, rev: &str) -> HeadSha {
    let out = git(root, &["rev-parse", rev]);
    HeadSha::try_from(String::from_utf8(out.stdout).unwrap().trim()).unwrap()
}

struct Scene {
    _dir: tempfile::TempDir,
    forge: PathBuf,
    quarantine: PathBuf,
    /// The tip of each of the forge's branches, `main` first.
    tips: Vec<HeadSha>,
}

/// A forge with `main` and two more branches, and an empty repository to fetch into.
fn scene() -> Scene {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let work = root.join("work");
    std::fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "-b", "main", "."]);
    let mut tips = Vec::new();
    for (i, branch) in ["main", "one", "two"].into_iter().enumerate() {
        if i > 0 {
            git(&work, &["checkout", "-q", "-b", branch, "main"]);
        }
        std::fs::write(work.join(format!("{branch}.txt")), format!("{branch}\n")).unwrap();
        git(&work, &["add", "-A"]);
        git(&work, &["commit", "-q", "-m", branch]);
        tips.push(sha_of(&work, "HEAD"));
    }
    let forge = root.join("forge.git");
    git(
        &root,
        &[
            "clone",
            "-q",
            "--bare",
            &work.to_string_lossy(),
            &forge.to_string_lossy(),
        ],
    );
    let quarantine = root.join("quarantine");
    std::fs::create_dir_all(&quarantine).unwrap();
    git(&quarantine, &["init", "-q", "."]);
    Scene {
        _dir: dir,
        forge,
        quarantine,
        tips,
    }
}

fn from(path: &Path) -> FromForge {
    FromForge {
        url: path.to_string_lossy().into_owned(),
        helper: None,
        cache: None,
    }
}

fn refs(quarantine: &Path, prefix: &str) -> Vec<String> {
    let out = git(quarantine, &["for-each-ref", "--format=%(refname)", prefix]);
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn has(quarantine: &Path, sha: &HeadSha) -> bool {
    crate::support::git_tried(
        quarantine,
        &["cat-file", "-e", &format!("{}^{{commit}}", sha.as_str())],
    )
    .status
    .success()
}

fn bogus() -> HeadSha {
    HeadSha::try_from("0123456789abcdef0123456789abcdef01234567").unwrap()
}

#[test]
fn the_forge_s_branch_tips_are_listed_as_full_ids() {
    purlis_core::unsteered!();
    let s = scene();
    let mut listed = from(&s.forge).advertised(&s.quarantine).expect("it lists");
    listed.sort();
    let mut expected: Vec<String> = s.tips.iter().map(|t| t.as_str().to_owned()).collect();
    expected.sort();
    assert_eq!(listed, expected);
}

#[test]
fn an_address_with_user_information_lists_nothing_and_fetches_nothing() {
    purlis_core::unsteered!();
    let s = scene();
    let forge = FromForge {
        url: format!("user:secret@{}", s.forge.to_string_lossy()),
        helper: None,
        cache: None,
    };
    assert!(forge.advertised(&s.quarantine).is_none());
    assert!(forge.fetch(&s.quarantine, &s.tips[0]).is_err());
    assert!(forge.fetch_all(&s.quarantine, &s.tips).is_err());
    assert!(refs(&s.quarantine, "refs/quarantine").is_empty());
}

#[test]
fn a_base_is_fetched_into_a_private_ref_so_that_later_fetches_negotiate_from_it() {
    purlis_core::unsteered!();
    let s = scene();
    from(&s.forge).fetch(&s.quarantine, &s.tips[0]).unwrap();
    assert!(has(&s.quarantine, &s.tips[0]));
    assert_eq!(
        refs(&s.quarantine, "refs/quarantine/base"),
        vec![format!("refs/quarantine/base/{}", s.tips[0].as_str())]
    );
}

#[test]
fn every_tip_is_fetched_into_a_private_ref_whatever_the_chunk_size() {
    purlis_core::unsteered!();
    for per_fetch in [1, 2, 256] {
        let s = scene();
        let limits = FetchLimits {
            per_fetch,
            ..FetchLimits::DEFAULT
        };
        from(&s.forge)
            .fetch_all_within(&s.quarantine, &s.tips, &limits)
            .unwrap();
        let mut held = refs(&s.quarantine, "refs/quarantine/tips");
        held.sort();
        let mut expected: Vec<String> = s
            .tips
            .iter()
            .map(|t| format!("refs/quarantine/tips/{}", t.as_str()))
            .collect();
        expected.sort();
        assert_eq!(held, expected, "per_fetch {per_fetch}");
        assert!(s.tips.iter().all(|t| has(&s.quarantine, t)));
    }
}

#[test]
fn a_tip_deleted_since_it_was_listed_is_skipped_and_the_others_are_fetched() {
    purlis_core::unsteered!();
    let s = scene();
    let mut tips = vec![bogus()];
    tips.extend(s.tips.iter().cloned());
    // One chunk holds the vanished tip: it fails whole, and is taken again tip by tip.
    from(&s.forge)
        .fetch_all_within(&s.quarantine, &tips, &FetchLimits::DEFAULT)
        .unwrap();
    assert!(s.tips.iter().all(|t| has(&s.quarantine, t)));
    assert!(!has(&s.quarantine, &bogus()));
}

#[test]
fn a_run_that_fetches_no_tip_at_all_is_an_error() {
    purlis_core::unsteered!();
    let s = scene();
    let refused = from(&s.forge).fetch_all_within(&s.quarantine, &[bogus()], &FetchLimits::DEFAULT);
    assert!(refused.is_err(), "{refused:?}");
}

#[test]
fn a_run_past_its_deadline_is_refused_with_the_reason() {
    purlis_core::unsteered!();
    let s = scene();
    let limits = FetchLimits {
        deadline: Duration::ZERO,
        ..FetchLimits::DEFAULT
    };
    let why = from(&s.forge)
        .fetch_all_within(&s.quarantine, &s.tips, &limits)
        .unwrap_err();
    assert!(why.contains("took longer than"), "{why}");
    assert!(refs(&s.quarantine, "refs/quarantine/tips").is_empty());
}

#[test]
fn a_run_past_its_size_budget_is_refused_with_the_reason() {
    purlis_core::unsteered!();
    let s = scene();
    let limits = FetchLimits {
        growth: 0,
        ..FetchLimits::DEFAULT
    };
    let why = from(&s.forge)
        .fetch_all_within(&s.quarantine, &s.tips, &limits)
        .unwrap_err();
    assert!(why.contains("MiB beyond the base"), "{why}");
}
