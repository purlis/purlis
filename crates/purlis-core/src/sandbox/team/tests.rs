// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The team's say over how far a person may turn the sandbox down (#2071, PR C).

use super::*;
use crate::sandbox::personal::Choice::{Floor, Off, On};

/// A project at a fresh git repository, its manifest holding `text`, committed by `author`.
fn project(text: &str, author: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let made = crate::testgit::run(root.path(), &["init", "-q"]);
    assert!(made.ok(), "{made:?}");
    commit(root.path(), text, author);
    root
}

fn commit(root: &Path, text: &str, author: &str) {
    std::fs::write(root.join("charter.toml"), text).unwrap();
    crate::testgit::run(root, &["add", "charter.toml"]);
    let ran = crate::testgit::run(
        root,
        &[
            "-c",
            &format!("user.name={author}"),
            "-c",
            "user.email=someone@example.invalid",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "a change",
        ],
    );
    assert!(ran.ok(), "{ran:?}");
}

fn head(root: &Path) -> String {
    crate::testgit::run(root, &["rev-parse", "HEAD"])
        .out
        .trim()
        .to_owned()
}

const OFF: &str = "[sandbox]\nmode = \"off\"\n";
const NEVER: &str = "[sandbox]\nmode = \"on\"\npolicy = \"never\"\n";
const NEVER_OFF: &str = "[sandbox]\nmode = \"off\"\npolicy = \"never\"\n";

#[test]
fn the_policy_is_never_or_local_and_absent_is_local() {
    let read = |text: &str| Team::of_text(text);
    assert_eq!(read("").policy, Policy::Local);
    assert_eq!(read("[sandbox]\nmode = \"on\"\n").policy, Policy::Local);
    assert_eq!(
        read("[sandbox]\npolicy = \"local\"\n").policy,
        Policy::Local
    );
    assert_eq!(read(NEVER).policy, Policy::Never);
    // A word that is not one is read as the stricter: a typo never loosens anyone's sandbox.
    for typo in ["\"nevr\"", "\"team-off\"", "true", "1"] {
        assert_eq!(
            read(&format!("[sandbox]\npolicy = {typo}\n")).policy,
            Policy::Never,
            "{typo}"
        );
    }
    assert!(read(OFF).off);
    assert!(!read(NEVER_OFF).off, "never forbids the team's off");
    assert!(!read("[sandbox]\nmode = \"on\"\n").off);
}

#[test]
fn under_local_each_person_takes_their_own_level_and_the_floor_by_default() {
    let bound = Bound::local(false);
    assert_eq!(bound.clamp(None), Floor);
    for chosen in [On, Floor, Off] {
        assert_eq!(bound.clamp(Some(chosen)), chosen);
    }
    assert!(bound.why.is_none());
}

#[test]
fn under_never_nobody_runs_below_the_full_sandbox_and_the_sentence_names_the_policy() {
    let root = project(NEVER, "Ada");
    let bound = allowed_levels(root.path());
    for chosen in [None, Some(On), Some(Floor), Some(Off)] {
        assert_eq!(bound.clamp(chosen), On, "{chosen:?}");
    }
    let why = bound.why.clone().expect("a sentence");
    assert!(why.contains("policy = \"never\""), "{why}");
    // Start without the sandbox is refused, by the same sentence.
    let locks = crate::sandbox::policy::Locks::of(root.path());
    assert!(locks.forbids_opt_out());
    assert_eq!(locks.opt_out_refused().as_deref(), Some(why.as_str()));
}

#[test]
fn a_team_s_off_waits_for_each_person_s_accept_naming_its_commit_and_author() {
    let root = project(OFF, "Ada Lovelace");
    let sha = head(root.path());
    // Until the person answers, their chats run at their own level, or the floor.
    match team_off(root.path()) {
        TeamOff::Asks(arrived) => {
            assert_eq!(arrived.commit, sha);
            assert_eq!(arrived.author, "Ada Lovelace");
            let said = arrived.said();
            assert!(said.contains(&sha[..7]), "{said}");
            assert!(said.contains("Ada Lovelace"), "{said}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(allowed_levels(root.path()).clamp(None), Floor);
    assert_eq!(level_for(root.path(), None), Floor);

    // Accepted: no sandbox, from each chat's next start.
    answer(root.path(), &sha, true).unwrap();
    assert!(matches!(team_off(root.path()), TeamOff::Accepted(_)));
    assert_eq!(allowed_levels(root.path()).clamp(None), Off);
    assert_eq!(level_for(root.path(), None), Off);
    // A level the person chose themselves still stands.
    assert_eq!(allowed_levels(root.path()).clamp(Some(On)), On);

    // Changed their mind in Settings: kept on.
    answer(root.path(), &sha, false).unwrap();
    assert!(matches!(team_off(root.path()), TeamOff::KeptOn(_)));
    assert_eq!(allowed_levels(root.path()).clamp(None), Floor);
}

#[test]
fn the_accept_is_kept_where_no_chat_writes_and_a_new_off_asks_again() {
    let root = project(OFF, "Ada");
    let first = head(root.path());
    answer(root.path(), &first, true).unwrap();
    // Not in the project: a chat could write there.
    assert!(!root.path().join(crate::profiles::LOCAL_FILE).exists());
    assert_eq!(
        crate::sandbox::local::team_off_answer(root.path()).map(|one| one.commit),
        Some(first.clone())
    );
    // Turned on, then off again in a later commit: asked again.
    commit(root.path(), "[sandbox]\nmode = \"on\"\n", "Bob");
    assert!(matches!(team_off(root.path()), TeamOff::No));
    commit(root.path(), OFF, "Bob");
    match team_off(root.path()) {
        TeamOff::Asks(arrived) => assert_eq!(arrived.author, "Bob"),
        other => panic!("{other:?}"),
    }
    assert_eq!(allowed_levels(root.path()).clamp(None), Floor);
}

#[test]
fn an_answer_for_another_commit_or_an_off_not_committed_is_refused() {
    let root = project(OFF, "Ada");
    assert!(
        answer(
            root.path(),
            "0000000000000000000000000000000000000000",
            true
        )
        .is_err()
    );
    // An off nobody committed (a working-tree edit) asks nobody and turns nothing off.
    let loose = project("[sandbox]\nmode = \"on\"\n", "Ada");
    std::fs::write(loose.path().join("charter.toml"), OFF).unwrap();
    assert!(matches!(team_off(loose.path()), TeamOff::NotCommitted));
    assert_eq!(allowed_levels(loose.path()).clamp(None), Floor);
    assert!(answer(loose.path(), &head(loose.path()), true).is_err());
}

#[test]
fn never_with_off_is_refused_and_asks_nobody() {
    let root = project(NEVER_OFF, "Ada");
    assert!(matches!(team_off(root.path()), TeamOff::No));
    assert_eq!(allowed_levels(root.path()).clamp(None), On);
    assert!(answer(root.path(), &head(root.path()), true).is_err());
}

#[test]
fn under_never_the_personal_switch_is_refused_naming_the_policy() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("charter.toml"), NEVER).unwrap();
    for lower in [Floor, Off] {
        let why = crate::sandbox::personal::set_by_you(root.path(), None, Some(lower))
            .expect_err("refused");
        assert!(why.contains("policy = \"never\""), "{why}");
        let why = crate::sandbox::personal::set_by_you(root.path(), Some("qa"), Some(lower))
            .expect_err("refused");
        assert!(why.contains("policy = \"never\""), "{why}");
    }
    // The full sandbox, and taking a level out, are still yours to set.
    crate::sandbox::personal::set_by_you(root.path(), None, Some(On)).unwrap();
    crate::sandbox::personal::set_by_you(root.path(), None, None).unwrap();
    // Without a policy, every level is.
    let free = tempfile::tempdir().unwrap();
    crate::sandbox::personal::set_by_you(free.path(), None, Some(Off)).unwrap();
}

#[test]
fn a_mode_changed_in_another_table_asks_nobody_again() {
    // Review of #2139: only `[sandbox].mode` says when the team's off arrived.
    let root = project(OFF, "Ada");
    let first = head(root.path());
    answer(root.path(), &first, true).unwrap();
    commit(
        root.path(),
        "[plane]\nmode = \"pr\"\n\n[sandbox]\nmode = \"off\"\n",
        "Bob",
    );
    commit(
        root.path(),
        "[plane]\nmode = \"off\"\n\n[repos.app]\nmode = \"pr\"\n\n[sandbox]\nmode = \"off\"\n",
        "Bob",
    );
    match team_off(root.path()) {
        TeamOff::Accepted(arrived) => {
            assert_eq!(arrived.commit, first);
            assert_eq!(arrived.author, "Ada");
        }
        other => panic!("{other:?}"),
    }
}
