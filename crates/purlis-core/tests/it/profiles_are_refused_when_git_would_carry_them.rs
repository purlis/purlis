// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter.local.toml` is gitignored, and charter does not trust its own line.
//!
//! The reason it cannot: an ignore rule does not apply to a tracked path. `git add -f`
//! commits the file, a fresh clone materialises it, and every machine that pulls runs the
//! commands in it — on a click, with no prompt in between. So while git tracks the file, or
//! WOULD commit it, every profile it declares is refused by name.
//!
//! An answer git could not give refuses too: an unknown is not a pass (ADR 0009).

use std::fs;
use std::path::Path;

use purlis_core::profiles;

use crate::support;

fn git(dir: &Path, args: &[&str]) {
    let out = purlis_core::forklock::output(
        support::unsigned()
            .args(args)
            .current_dir(dir)
            // Hermetic: the operator's own global config is not this test's business, and it
            // broke the suite once — a `commit.gpgsign` pointing at a 1Password signer failed
            // the commit with "failed to fill whole buffer" and reddened two tests that are
            // about git's answer, not about signing.
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@e")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@e"),
    )
    .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

/// A plane that is a git repository, with a local file and whatever `.gitignore` is given.
fn repo(ignore: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("charter.toml"), "").unwrap();
    fs::write(
        dir.path().join(profiles::LOCAL_FILE),
        "[harness.work]\nkind = \"claude\"\ncommand = [\"claude\"]\n",
    )
    .unwrap();
    if !ignore.is_empty() {
        fs::write(dir.path().join(".gitignore"), ignore).unwrap();
    }
    git(dir.path(), &["init", "-q"]);
    dir
}

#[test]
fn a_plane_that_is_not_a_git_repository_has_nothing_to_commit_to_and_passes() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(profiles::LOCAL_FILE), "").unwrap();

    assert!(profiles::ignore_check(dir.path()).passes());
}

#[test]
fn a_plane_with_no_local_file_declares_nothing_and_passes() {
    purlis_core::unsteered!();
    let dir = repo("/charter.local.toml\n");
    fs::remove_file(dir.path().join(profiles::LOCAL_FILE)).unwrap();

    assert!(profiles::ignore_check(dir.path()).passes());
}

#[test]
fn an_ignored_untracked_local_file_is_the_state_the_feature_wants_and_passes() {
    purlis_core::unsteered!();
    let dir = repo("/charter.local.toml\n");

    assert!(profiles::ignore_check(dir.path()).passes());
}

#[test]
fn a_local_file_git_would_commit_refuses_every_profile_in_it() {
    purlis_core::unsteered!();
    // No `.gitignore` line: git reports it as `??`, which means the next `git add .` takes
    // it, and then every clone has it.
    let dir = repo("");

    let check = profiles::ignore_check(dir.path());

    assert_eq!(
        check.reason,
        "git would commit charter.local.toml, so purlis reads nothing in it until it is \
         ignored — purlis doctor --fix local-ignore adds /charter.local.toml to .gitignore."
    );
    assert_eq!(check.fix, "purlis reinit");
}

#[test]
fn a_tracked_local_file_is_refused_and_told_that_reinit_alone_will_not_fix_it() {
    purlis_core::unsteered!();
    // One fix per state: `charter reinit` adds the ignore line, and an ignore rule does not
    // apply to a path git already tracks.
    let dir = repo("/charter.local.toml\n");
    git(dir.path(), &["add", "-f", profiles::LOCAL_FILE]);
    git(dir.path(), &["commit", "-qm", "forced"]);

    let check = profiles::ignore_check(dir.path());

    assert_eq!(
        check.reason,
        "git tracks charter.local.toml, so what it says would reach every clone of this \
         plane — purlis reads nothing in it until it is untracked: git rm --cached \
         charter.local.toml, commit that removal, then purlis reinit."
    );
    assert_eq!(
        check.fix,
        "git rm --cached charter.local.toml, commit that removal, then purlis reinit"
    );
}

#[test]
fn a_file_staged_for_removal_but_not_yet_committed_is_still_tracked() {
    purlis_core::unsteered!();
    // `git rm --cached` leaves `D ` beside `!!` until the removal is committed, and the file
    // is in HEAD until then — so a clone still carries it.
    let dir = repo("/charter.local.toml\n");
    git(dir.path(), &["add", "-f", profiles::LOCAL_FILE]);
    git(dir.path(), &["commit", "-qm", "forced"]);
    git(dir.path(), &["rm", "-q", "--cached", profiles::LOCAL_FILE]);

    assert!(
        profiles::ignore_check(dir.path())
            .reason
            .starts_with("git tracks charter.local.toml,"),
        "a removal that is not committed read as untracked"
    );
}

#[test]
fn an_answer_git_could_not_give_refuses_because_an_unknown_is_not_a_pass() {
    purlis_core::unsteered!();
    // ADR 0009. There is no git on the PATH this check is given, so it cannot look — which
    // is not the same as looking and finding the file ignored.
    let dir = repo("/charter.local.toml\n");

    let check = profiles::ignore_check_with(dir.path(), Path::new("/definitely/not/git"));

    assert!(
        check
            .reason
            .starts_with("git could not say whether charter.local.toml is ignored (")
            && check.reason.ends_with(
                "), so purlis reads nothing in it — an unknown is not a pass. Run git status \
             --ignored -- charter.local.toml in the plane to see what git says."
            ),
        "{:?}",
        check.reason
    );
    assert!(check.fix.starts_with(
        "run git status --ignored -- charter.local.toml in the plane by hand; git said: "
    ));
}

#[test]
fn a_refusing_check_moves_every_declared_profile_to_refused_and_leaves_the_built_ins() {
    purlis_core::unsteered!();
    // Each of the three sentences says "charter reads nothing in it", so a surface that
    // asked must show them refused — not as ordinary rows with a warning underneath.
    let dir = repo("");
    let set = profiles::current(dir.path());
    assert!(
        set.get("work").is_some(),
        "the profile reads fine on its own"
    );

    let guarded = profiles::with_ignore_check(set, &profiles::ignore_check(dir.path()));

    assert!(guarded.get("work").is_none());
    assert_eq!(
        guarded
            .profiles()
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["claude", "opencode", "codex"]
    );
    assert!(
        guarded
            .refused
            .iter()
            .any(|r| r.name == "work" && r.reason.starts_with("git would commit"))
    );
}

#[test]
fn a_replacement_refused_by_the_git_check_does_not_let_its_built_in_stand_in() {
    purlis_core::unsteered!();
    // Ruling 19: the operator said how `claude` runs here, and the built-in standing in
    // would run the command they replaced.
    let dir = repo("");
    fs::write(
        dir.path().join(profiles::LOCAL_FILE),
        "[harness.claude]\nkind = \"claude\"\ncommand = [\"~/.local/bin/claude\"]\n",
    )
    .unwrap();

    let guarded = profiles::with_ignore_check(
        profiles::current(dir.path()),
        &profiles::ignore_check(dir.path()),
    );

    assert_eq!(
        guarded
            .profiles()
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["opencode", "codex"],
        "the built-in `claude` stood in for the refused replacement"
    );
}

/// A stand-in `git` in `dir` that runs `body` whatever it is asked.
///
/// Through `stand_in::program`, never `fs::write` here: these tests run it the instant it is
/// written, and written from this process that lost to `ETXTBSY` on a green branch
/// (charter-app#81). The reasoning is in that crate.
fn a_git_that(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
    stand_in::program(dir, name, &format!("#!/bin/sh\n{body}\n"))
}

#[test]
fn a_git_that_refuses_the_repository_is_not_taken_for_one_that_found_none() {
    purlis_core::unsteered!();
    // Only git's own "not a git repository" at exit 128 is a plane with nothing to commit
    // to. Exit 128 is also what git answers when it refuses to look — a repository owned by
    // someone else, `safe.directory` unset — and that is an unknown, not a pass. Turning the
    // `&&` into `||` (nightly run 35430920851) passed the suite and waved every declared
    // profile through on a plane git would not read.
    let dir = repo("");
    let refusing = a_git_that(
        dir.path(),
        "git-that-refuses",
        "echo \"fatal: detected dubious ownership in repository at '$PWD'\" >&2\nexit 128",
    );

    let check = profiles::ignore_check_with(dir.path(), &refusing);

    assert!(!check.passes(), "a refusing git read as no repository");
    assert!(
        check.reason.starts_with(
            "git could not say whether charter.local.toml is ignored (fatal: \
                          detected dubious ownership"
        ),
        "{:?}",
        check.reason
    );
}

#[test]
fn a_status_line_charter_cannot_read_is_an_unknown_and_not_a_tracked_file() {
    purlis_core::unsteered!();
    // A status code outside the ones that mean "tracked" is git saying something charter has
    // not measured. It refuses either way — but as an UNKNOWN, with git's own line quoted
    // and the fix that goes with it, not as "git tracks charter.local.toml" and a
    // `git rm --cached` that would not help. The `&&` beside the tracked codes survived
    // being turned into `||` (the same run).
    let dir = repo("");
    let odd = a_git_that(
        dir.path(),
        "git-that-says-zz",
        "echo 'ZZ charter.local.toml'",
    );

    let check = profiles::ignore_check_with(dir.path(), &odd);

    assert_eq!(
        check.reason,
        "git could not say whether charter.local.toml is ignored (git status printed \"ZZ \
         charter.local.toml\"), so purlis reads nothing in it — an unknown is not a pass. \
         Run git status --ignored -- charter.local.toml in the plane to see what git says."
    );
}

#[test]
fn a_git_that_never_answers_refuses_too_because_a_hang_is_not_a_pass() {
    purlis_core::unsteered!();
    // The unknown branch was only ever driven by a git that EXITS. A git that hangs is the
    // one that would turn "an unknown is not a pass" into a pass, and it was unexercised —
    // found in review. It waits two seconds rather than the real thirty: at thirty it was half
    // of purlis-core's test time, paid again for every mutant the nightly tests. And the git
    // "hangs" for twenty seconds, not for ever: long enough that only a deadline answers
    // first, short enough that a check which stopped keeping its deadline FAILS here inside a
    // mutation run's timeout instead of outliving it as a TIMEOUT.
    let dir = repo("/charter.local.toml\n");
    //
    // It waits as a shell runs any command, in a child, and says which process that child is.
    let said = dir.path().join("what-the-hanging-git-started");
    let hanging = a_git_that(
        dir.path(),
        "git-that-hangs",
        &format!(
            "sleep 20 &\necho $! > '{0}.part'\nmv '{0}.part' '{0}'\nwait",
            said.display()
        ),
    );

    let check =
        profiles::ignore_check_within(dir.path(), &hanging, std::time::Duration::from_secs(2));

    // Ended at the deadline with what it started. Ending the script alone left its `sleep`
    // running for twenty seconds, and a suite that ended meanwhile was failed for it by CI's
    // orphan check, with no test named (qw375).
    // The id is waited for rather than read once: on a loaded machine the git may not have
    // written it yet. One that never wrote it fails the test: with no id there is nothing to
    // hold, and the check below would pass whether or not the group was ended.
    #[cfg(unix)]
    {
        let started = stand_in::pid_named_in(&said, std::time::Duration::from_secs(5))
            .expect("the git said which process it started");
        assert!(
            stand_in::is_gone(started, std::time::Duration::from_secs(10)),
            "what the git started was left running"
        );
    }

    assert!(
        check
            .reason
            .contains("git could not say whether charter.local.toml is ignored"),
        "{:?}",
        check.reason
    );
    assert!(
        check.reason.contains("did not answer within 2 seconds"),
        "{:?}",
        check.reason
    );
}

/// A git that answers `answer` and writes one line to `asked` each time it is run.
fn a_git_that_counts(dir: &Path, asked: &Path, answer: &str) -> std::path::PathBuf {
    a_git_that(
        dir,
        "git-that-counts",
        &format!("echo asked >> '{}'\necho '{answer}'", asked.display()),
    )
}

/// A plane with a local file, for a stand-in git that answers without a repository.
fn a_plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("charter.toml"), "").unwrap();
    fs::write(dir.path().join(profiles::LOCAL_FILE), "").unwrap();
    dir
}

fn times_asked(asked: &Path) -> usize {
    fs::read_to_string(asked).map_or(0, |text| text.lines().count())
}

#[test]
fn one_start_asks_git_once_and_every_reader_in_it_gets_that_answer() {
    purlis_core::unsteered!();
    // #2179: a New tab read the Local file's git state seven times or so, each a `git status`
    // of 140 ms on the macOS runner. Inside one start the answer is asked once and given to
    // every reader, so each reader decides as before, on the same answer.
    let dir = a_plane();
    let asked = dir.path().join("asked");
    let git = a_git_that_counts(dir.path(), &asked, "?? charter.local.toml");

    let alone = profiles::ignore_check_with(dir.path(), &git);
    let (first, second, third) = profiles::git_asked_once(|| {
        (
            profiles::ignore_check_with(dir.path(), &git),
            profiles::ignore_check_with(dir.path(), &git),
            profiles::ignore_check_with(dir.path(), &git),
        )
    });

    assert!(!alone.passes(), "a file git would commit still refuses");
    assert_eq!([&first, &second, &third], [&alone, &alone, &alone]);
    assert_eq!(
        times_asked(&asked),
        2,
        "once alone, once for the three inside"
    );
}

/// A git that answers whatever `answer` holds when it is run, and counts its runs in `asked`.
fn a_git_that_answers_from(dir: &Path, asked: &Path, answer: &Path) -> std::path::PathBuf {
    a_git_that(
        dir,
        "git-that-answers-from",
        &format!(
            "echo asked >> '{}'\ncat '{}'",
            asked.display(),
            answer.display()
        ),
    )
}

/// What a stand-in git prints for a local file it ignores, and for one it would commit.
const IGNORED: &str = "!! charter.local.toml\n";
const COMMITTABLE: &str = "?? charter.local.toml\n";

#[test]
fn an_answer_is_kept_for_its_start_alone_and_never_for_the_next() {
    purlis_core::unsteered!();
    // Kept for one start, never for the run: git's answer changes when the person runs
    // `git add -f` or `git rm --cached`, and the next start asks again and decides by it.
    let dir = a_plane();
    let asked = dir.path().join("asked");
    let answer = dir.path().join("answer");
    fs::write(&answer, IGNORED).unwrap();
    let git = a_git_that_answers_from(dir.path(), &asked, &answer);

    let first = profiles::git_asked_once(|| profiles::ignore_check_with(dir.path(), &git));
    fs::write(&answer, COMMITTABLE).unwrap();
    let second = profiles::git_asked_once(|| profiles::ignore_check_with(dir.path(), &git));

    assert!(first.passes(), "{first:?}");
    assert!(
        !second.passes(),
        "the next start kept the last one's answer"
    );
    assert_eq!(times_asked(&asked), 2);
}

#[test]
fn a_local_file_changed_during_a_start_is_asked_about_again() {
    purlis_core::unsteered!();
    // A reader later in the start never judges content newer than its answer: the file changed
    // after the first reader's answer, so the second reader asks git again, and refuses.
    let dir = a_plane();
    let asked = dir.path().join("asked");
    let answer = dir.path().join("answer");
    fs::write(&answer, IGNORED).unwrap();
    let git = a_git_that_answers_from(dir.path(), &asked, &answer);

    let (first, second) = profiles::git_asked_once(|| {
        let first = profiles::ignore_check_with(dir.path(), &git);
        fs::write(
            dir.path().join(profiles::LOCAL_FILE),
            "[harness.work]\nkind = \"claude\"\ncommand = [\"claude\"]\n",
        )
        .unwrap();
        fs::write(&answer, COMMITTABLE).unwrap();
        (first, profiles::ignore_check_with(dir.path(), &git))
    });

    assert!(first.passes(), "{first:?}");
    assert!(
        !second.passes(),
        "a changed file was judged by the answer before the change"
    );
    assert_eq!(times_asked(&asked), 2);
}

#[test]
fn a_start_that_panics_keeps_no_answer_for_the_next() {
    purlis_core::unsteered!();
    let dir = a_plane();
    let asked = dir.path().join("asked");
    let answer = dir.path().join("answer");
    fs::write(&answer, IGNORED).unwrap();
    let git = a_git_that_answers_from(dir.path(), &asked, &answer);

    let panicked = std::panic::catch_unwind(|| {
        profiles::git_asked_once(|| {
            profiles::ignore_check_with(dir.path(), &git);
            panic!("a start that fails part-way");
        })
    });
    fs::write(&answer, COMMITTABLE).unwrap();
    let after = profiles::ignore_check_with(dir.path(), &git);

    assert!(panicked.is_err());
    assert!(
        !after.passes(),
        "the answer of a start that panicked outlived it"
    );
    assert_eq!(times_asked(&asked), 2);
}

#[test]
fn a_start_asks_again_for_a_file_that_appeared_during_it_or_another_plane() {
    purlis_core::unsteered!();
    // An absent file passes without asking git; one written during the start is asked about,
    // and another plane's answer is its own.
    let one = a_plane();
    let other = a_plane();
    fs::remove_file(one.path().join(profiles::LOCAL_FILE)).unwrap();
    let asked = other.path().join("asked");
    let git = a_git_that_counts(other.path(), &asked, "?? charter.local.toml");

    let (absent, written, elsewhere) = profiles::git_asked_once(|| {
        let absent = profiles::ignore_check_with(one.path(), &git);
        fs::write(one.path().join(profiles::LOCAL_FILE), "").unwrap();
        let written = profiles::ignore_check_with(one.path(), &git);
        let elsewhere = profiles::ignore_check_with(other.path(), &git);
        (absent, written, elsewhere)
    });

    assert!(absent.passes());
    assert!(!written.passes());
    assert!(!elsewhere.passes());
    assert_eq!(times_asked(&asked), 2);
}
