// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::time::Duration;

fn lock(size: u64, age: f64) -> IndexLock {
    IndexLock {
        path: PathBuf::from("/p/.git/index.lock"),
        size,
        age,
    }
}

#[test]
fn an_age_is_coarse_and_never_negative() {
    assert_eq!(age_phrase(-5.0), "0s");
    assert_eq!(age_phrase(f64::NAN), "0s");
    assert_eq!(age_phrase(4.0), "4s");
    assert_eq!(age_phrase(59.9), "59s");
    // Each unit starts exactly at its boundary, as Python's `secs < 60` has it.
    assert_eq!(age_phrase(60.0), "1m");
    assert_eq!(age_phrase(3600.0), "1h");
    assert_eq!(age_phrase(86400.0), "1d");
    assert_eq!(age_phrase(12.0 * 60.0), "12m");
    assert_eq!(age_phrase(23.0 * 3600.0), "23h");
    assert_eq!(age_phrase(3.0 * 86400.0), "3d");
}

#[test]
fn only_an_empty_lock_that_has_sat_there_is_called_a_crash() {
    assert!(lock(0, STALE_AFTER).crashed());
    // A lock that got as far as holding an index is a git that was working.
    assert!(!lock(1, STALE_AFTER * 10.0).crashed());
    // And a young one may still be running, so charter does not send anyone to `rm`.
    assert!(!lock(0, STALE_AFTER - 1.0).crashed());
}

#[test]
fn the_description_carries_the_path_the_size_and_the_age() {
    let said = lock(0, STALE_AFTER).describe();
    assert!(said.contains("/p/.git/index.lock"), "{said}");
    assert!(said.contains("0 bytes"), "{said}");
    assert!(said.contains("15m old"), "{said}");
    assert!(said.contains("crashed"), "{said}");
    assert!(lock(4, 1.0).describe().contains("may still be holding it"));
}

#[test]
fn the_remedy_asks_who_holds_it_before_it_offers_the_rm() {
    let [first, second] = lock(0, STALE_AFTER).remedy();
    assert!(first.starts_with("who holds it:"), "{first}");
    assert!(second.contains("rm -f /p/.git/index.lock"), "{second}");
}

#[test]
fn a_real_lock_is_found_with_its_size_and_a_directory_without_one_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let git_dir = dir.path().join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();

    assert_eq!(find(&git_dir), None, "no lock, no answer");
    assert_eq!(
        find(&dir.path().join("nowhere")),
        None,
        "nor for a non-path"
    );

    std::fs::write(git_dir.join(LOCK_NAME), "").unwrap();
    let found = find(&git_dir).expect("the lock");
    assert_eq!(found.size, 0);
    assert!(found.age >= 0.0 && found.age < 60.0, "{}", found.age);
    assert!(found.path.ends_with("index.lock"));
}

#[test]
fn git_is_quoted_rather_than_paraphrased_and_a_silent_failure_says_nothing() {
    let run = Run {
        code: Some(128),
        out: String::new(),
        err: "\n  fatal: Unable to create '/p/.git/index.lock': File exists.\nsecond\n".into(),
    };
    assert_eq!(
        said(&run),
        "fatal: Unable to create '/p/.git/index.lock': File exists."
    );
    assert_eq!(
        said(&Run {
            code: None,
            out: String::new(),
            err: "  \n\n".into()
        }),
        ""
    );
}

#[test]
fn the_age_is_read_from_the_file_rather_than_assumed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(LOCK_NAME);
    std::fs::write(&path, "x").unwrap();
    let back = SystemTime::now() - Duration::from_secs(3600);
    // `set_times` is stable and needs no `unsafe`, which this workspace forbids.
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(back))
        .unwrap();

    let found = find(dir.path()).expect("the lock");
    assert_eq!(age_phrase(found.age), "1h", "{}", found.age);
    assert_eq!(found.size, 1);
}

// ----------------------------------------------------------------------------------- //
// `stopped`: one check for every save, the pull and `charter sync` (#433)               //
// ----------------------------------------------------------------------------------- //

fn git(dir: &Path, args: &[&str]) -> Run {
    crate::testgit::run(dir, args)
}

/// A repo on `main` whose README.md was changed on `side` and on `main` both, so a
/// merge, rebase, cherry-pick or revert of the one onto the other conflicts in it.
fn diverged() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for args in [
        &["init", "-q", "-b", "main", "."][..],
        &["config", "user.name", "Fixture"],
        &["config", "user.email", "fixture@example.invalid"],
    ] {
        assert!(git(root, args).ok());
    }
    let commit = |text: &str| {
        std::fs::write(root.join("README.md"), text).unwrap();
        assert!(git(root, &["add", "-A"]).ok());
        assert!(git(root, &["commit", "-q", "-m", text.trim()]).ok());
    };
    commit("one\n");
    assert!(git(root, &["checkout", "-q", "-b", "side"]).ok());
    commit("side\n");
    assert!(git(root, &["checkout", "-q", "main"]).ok());
    commit("main\n");
    dir
}

#[test]
fn a_clean_tree_has_stopped_nowhere() {
    let dir = diverged();
    assert_eq!(stopped(dir.path()), None);
}

#[test]
fn a_conflicted_merge_is_stopped_with_its_file_and_its_way_out() {
    let dir = diverged();
    assert!(!git(dir.path(), &["merge", "side"]).ok());

    let got = stopped(dir.path()).expect("stopped");

    assert_eq!(got.operation, Some(Operation::Merge));
    assert_eq!(got.unmerged, vec!["README.md".to_string()]);
    let why = got.why();
    assert!(why.contains("a merge is stopped part-way"), "{why}");
    assert!(why.contains("README.md"), "{why}");
    assert!(why.contains("git merge --abort"), "{why}");
}

#[test]
fn a_rebase_stopped_part_way_is_a_rebase() {
    let dir = diverged();
    assert!(!git(dir.path(), &["rebase", "side"]).ok());

    let got = stopped(dir.path()).expect("stopped");

    assert_eq!(got.operation, Some(Operation::Rebase));
    assert!(got.why().contains("git rebase --abort"), "{}", got.why());
}

#[test]
fn a_revert_that_conflicted_is_a_revert() {
    let dir = diverged();
    // Reverting `one` on top of `main` conflicts: `main` changed the line `one` added.
    assert!(!git(dir.path(), &["revert", "--no-edit", "main~1"]).ok());
    assert!(dir.path().join(".git/REVERT_HEAD").exists());

    let got = stopped(dir.path()).expect("stopped");

    assert_eq!(got.operation, Some(Operation::Revert));
    assert!(got.why().contains("git revert --abort"), "{}", got.why());
}

#[test]
fn a_bisect_under_way_is_stopped_even_with_nothing_unmerged() {
    let dir = diverged();
    assert!(git(dir.path(), &["bisect", "start"]).ok());
    assert!(dir.path().join(".git/BISECT_LOG").exists());

    let got = stopped(dir.path()).expect("stopped");

    assert_eq!(got.operation, Some(Operation::Bisect));
    assert!(got.unmerged.is_empty());
    assert!(got.why().contains("git bisect reset"), "{}", got.why());
}

#[test]
fn every_marker_git_leaves_is_one_operation() {
    let dir = tempfile::tempdir().unwrap();
    for (marker, op) in [
        ("MERGE_HEAD", Operation::Merge),
        ("rebase-merge", Operation::Rebase),
        ("rebase-apply", Operation::Rebase),
        ("REBASE_HEAD", Operation::Rebase),
        ("CHERRY_PICK_HEAD", Operation::CherryPick),
        ("REVERT_HEAD", Operation::Revert),
        ("BISECT_LOG", Operation::Bisect),
    ] {
        let git_dir = dir.path().join(marker.to_lowercase());
        std::fs::create_dir_all(&git_dir).unwrap();
        assert_eq!(Operation::in_progress(&git_dir), None, "{marker}");
        std::fs::write(git_dir.join(marker), "").unwrap();
        assert_eq!(Operation::in_progress(&git_dir), Some(op), "{marker}");
    }
    let am = dir.path().join("am");
    std::fs::create_dir_all(am.join("rebase-apply")).unwrap();
    std::fs::write(am.join("rebase-apply/applying"), "").unwrap();
    assert_eq!(Operation::in_progress(&am), Some(Operation::Am));
}
