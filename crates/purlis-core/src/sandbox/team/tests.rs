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

/// What the app's own fetch of the upstream found there: `commit`, kept where no chat writes.
fn fetched(root: &Path, commit: &str) {
    crate::sandbox::local::keep_fetched_upstream(root, commit).unwrap();
}

/// The commit and author an off on the fetched upstream is named by.
fn upstream(off: &TeamOff) -> Option<&Arrived> {
    match off {
        TeamOff::Asks(Arrival::Upstream(arrived))
        | TeamOff::Accepted(Arrival::Upstream(arrived))
        | TeamOff::KeptOn(Arrival::Upstream(arrived)) => Some(arrived),
        _ => None,
    }
}

#[test]
fn a_team_s_off_on_the_fetched_upstream_waits_for_each_person_s_accept_naming_its_commit_and_author()
 {
    let root = project(OFF, "Ada Lovelace");
    let sha = head(root.path());
    fetched(root.path(), &sha);
    // Until the person answers, their chats run at their own level, or the floor.
    let off = team_off(root.path());
    assert!(matches!(off, TeamOff::Asks(_)), "{off:?}");
    let arrived = upstream(&off).expect("named");
    assert_eq!(arrived.commit, sha);
    assert_eq!(arrived.author, "Ada Lovelace");
    let said = Arrival::Upstream(arrived.clone()).said();
    assert!(said.contains(&sha[..7]), "{said}");
    assert!(said.contains("Ada Lovelace"), "{said}");
    assert_eq!(allowed_levels(root.path()).clamp(None), Floor);
    assert_eq!(level_for(root.path(), None), Floor);

    // Accepted: no sandbox, from each chat's next start.
    answer(root.path(), true).unwrap();
    assert!(matches!(team_off(root.path()), TeamOff::Accepted(_)));
    assert_eq!(allowed_levels(root.path()).clamp(None), Off);
    assert_eq!(level_for(root.path(), None), Off);
    // A level the person chose themselves still stands.
    assert_eq!(allowed_levels(root.path()).clamp(Some(On)), On);

    // Changed their mind in Settings: kept on.
    answer(root.path(), false).unwrap();
    assert!(matches!(team_off(root.path()), TeamOff::KeptOn(_)));
    assert_eq!(allowed_levels(root.path()).clamp(None), Floor);
}

#[test]
fn a_local_commit_not_on_the_fetched_upstream_is_called_local_with_no_author() {
    let root = project("[sandbox]\nmode = \"on\"\n", "Ada");
    let before = head(root.path());
    commit(root.path(), OFF, "Ada Lovelace");
    let sha = head(root.path());
    let said_of = |off: TeamOff| match off {
        TeamOff::Asks(arrival) => {
            assert_eq!(arrival, Arrival::Local);
            arrival.said()
        }
        other => panic!("{other:?}"),
    };
    let quiet = |said: &str| {
        assert!(said.contains("local"), "{said}");
        assert!(said.contains("upstream"), "{said}");
        assert!(!said.contains("Ada"), "{said}");
        assert!(!said.contains(&sha[..7]), "{said}");
    };
    // Nothing fetched by the app: local.
    quiet(&said_of(team_off(root.path())));
    // The app's fetch found the upstream before the off: still local.
    fetched(root.path(), &before);
    quiet(&said_of(team_off(root.path())));
    // A remote-tracking ref in the clone is no proof: anything in the clone may write one.
    let moved = crate::testgit::run(
        root.path(),
        &["update-ref", "refs/remotes/origin/main", &sha],
    );
    assert!(moved.ok(), "{moved:?}");
    quiet(&said_of(team_off(root.path())));
    // Nor is an object that stands in for the upstream's.
    let grafted = crate::testgit::run(root.path(), &["replace", "--graft", &before, &sha]);
    assert!(grafted.ok(), "{grafted:?}");
    quiet(&said_of(team_off(root.path())));
    // The answer is still the person's to give, and it holds.
    answer(root.path(), false).unwrap();
    assert!(matches!(
        team_off(root.path()),
        TeamOff::KeptOn(Arrival::Local)
    ));
}

#[test]
fn a_history_rewritten_under_an_unchanged_off_asks_nobody_again() {
    let root = project(OFF, "Ada");
    answer(root.path(), false).unwrap();
    // Rewritten: the off now sits in another commit, by another name.
    let amended = crate::testgit::run(
        root.path(),
        &[
            "-c",
            "user.name=Someone Else",
            "-c",
            "user.email=else@example.invalid",
            "commit",
            "-q",
            "--amend",
            "--reset-author",
            "-m",
            "rewritten",
        ],
    );
    assert!(amended.ok(), "{amended:?}");
    assert!(matches!(team_off(root.path()), TeamOff::KeptOn(_)));
    // And again, in a commit of its own on top.
    commit(root.path(), &format!("{OFF}# again\n"), "Someone Else");
    assert!(matches!(team_off(root.path()), TeamOff::KeptOn(_)));
    // An accept holds the same way.
    answer(root.path(), true).unwrap();
    commit(root.path(), &format!("{OFF}# and again\n"), "Someone Else");
    assert!(matches!(team_off(root.path()), TeamOff::Accepted(_)));
}

#[test]
fn the_answer_is_kept_where_no_chat_writes_and_an_off_seen_to_leave_and_return_asks_again() {
    let root = project(OFF, "Ada");
    answer(root.path(), true).unwrap();
    // Not in the project: a chat could write there.
    assert!(!root.path().join(crate::profiles::LOCAL_FILE).exists());
    assert_eq!(
        crate::sandbox::local::team_off_answer(root.path()).map(|one| one.accepted),
        Some(true)
    );
    // Turned on, then off again in a later commit: asked again.
    commit(root.path(), "[sandbox]\nmode = \"on\"\n", "Bob");
    assert!(matches!(team_off(root.path()), TeamOff::No));
    assert_eq!(crate::sandbox::local::team_off_answer(root.path()), None);
    commit(root.path(), OFF, "Bob");
    assert!(matches!(team_off(root.path()), TeamOff::Asks(_)));
    assert_eq!(allowed_levels(root.path()).clamp(None), Floor);

    // The same where the file on disk says on for a while, with no commit.
    answer(root.path(), true).unwrap();
    std::fs::write(
        root.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n",
    )
    .unwrap();
    assert!(matches!(team_off(root.path()), TeamOff::No));
    std::fs::write(root.path().join("charter.toml"), OFF).unwrap();
    assert!(matches!(team_off(root.path()), TeamOff::Asks(_)));
}

#[test]
fn an_off_not_committed_is_not_answered() {
    // An off nobody committed (a working-tree edit) asks nobody and turns nothing off.
    let loose = project("[sandbox]\nmode = \"on\"\n", "Ada");
    std::fs::write(loose.path().join("charter.toml"), OFF).unwrap();
    assert!(matches!(team_off(loose.path()), TeamOff::NotCommitted));
    assert_eq!(allowed_levels(loose.path()).clamp(None), Floor);
    assert!(answer(loose.path(), true).is_err());
}

#[test]
fn never_with_off_is_refused_and_asks_nobody() {
    let root = project(NEVER_OFF, "Ada");
    assert!(matches!(team_off(root.path()), TeamOff::No));
    assert_eq!(allowed_levels(root.path()).clamp(None), On);
    assert!(answer(root.path(), true).is_err());
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
fn a_mode_changed_in_another_table_names_the_commit_the_off_arrived_in() {
    // Review of #2139: only `[sandbox].mode` says when the team's off arrived.
    let root = project(OFF, "Ada");
    let first = head(root.path());
    answer(root.path(), true).unwrap();
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
    fetched(root.path(), &head(root.path()));
    let off = team_off(root.path());
    assert!(matches!(off, TeamOff::Accepted(_)), "{off:?}");
    let arrived = upstream(&off).expect("named");
    assert_eq!(arrived.commit, first);
    assert_eq!(arrived.author, "Ada");
}

/// #2071: in a project that turned the full sandbox on, a person who chose nothing runs at it,
/// so their level, not the bound's default, is what may take a chat below it.
#[test]
fn in_a_project_at_the_full_sandbox_a_person_who_chose_nothing_runs_at_it() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n",
    )
    .unwrap();
    let bound = allowed_levels(root.path());
    assert_eq!(bound.clamp(None), On);
    assert_eq!(bound.clamp(Some(Floor)), Floor);
    assert_eq!(bound.lowest, Off);
}

/// A commit of HEAD's tree on `parent`, made without moving HEAD: a commit of the upstream's
/// that this clone has fetched but not checked out.
fn commit_on(root: &Path, parent: &str, message: &str) -> String {
    let made = crate::testgit::run(
        root,
        &[
            "-c",
            "user.name=Upstream",
            "-c",
            "user.email=upstream@example.invalid",
            "commit-tree",
            "HEAD^{tree}",
            "-p",
            parent,
            "-m",
            message,
        ],
    );
    assert!(made.ok(), "{made:?}");
    made.out.trim().to_owned()
}

/// Where the clone keeps `id` as a loose object.
fn loose(root: &Path, id: &str) -> std::path::PathBuf {
    root.join(".git/objects").join(&id[..2]).join(&id[2..])
}

/// The object kept under `id` is replaced by `bytes`, as written to the clone's own store.
fn substitute(root: &Path, id: &str, bytes: &[u8]) {
    let at = loose(root, id);
    assert!(at.exists(), "{at:?}");
    std::fs::remove_file(&at).unwrap();
    std::fs::write(&at, bytes).unwrap();
}

/// A commit like `id`, but naming `parent` as its parent, written to the store under its own id:
/// the loose object's bytes.
fn forged(root: &Path, id: &str, parent: &str) -> Vec<u8> {
    let read = crate::testgit::run(root, &["cat-file", "commit", id]);
    assert!(read.ok(), "{read:?}");
    let text: String = read
        .out
        .lines()
        .map(|line| {
            if line.starts_with("parent ") {
                format!("parent {parent}\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect();
    let file = root.join(".git/forged");
    std::fs::write(&file, text).unwrap();
    let written = crate::testgit::run(
        root,
        &["hash-object", "-w", "-t", "commit", file.to_str().unwrap()],
    );
    assert!(written.ok(), "{written:?}");
    std::fs::read(loose(root, written.out.trim())).unwrap()
}

#[test]
fn an_off_further_down_the_fetched_upstream_s_history_is_named() {
    let root = project(OFF, "Ada Lovelace");
    let sha = head(root.path());
    let next = commit_on(root.path(), &sha, "next");
    let upstream_head = commit_on(root.path(), &next, "and next");
    fetched(root.path(), &upstream_head);
    let off = team_off(root.path());
    let arrived = upstream(&off).expect("named");
    assert_eq!(arrived.commit, sha);
    assert_eq!(arrived.author, "Ada Lovelace");
}

#[test]
fn a_substituted_or_damaged_object_on_the_way_to_the_fetched_upstream_calls_the_off_local() {
    // The upstream went on from the project's first commit; the off is a local commit beside it.
    let root = project("[sandbox]\nmode = \"on\"\n", "Ada");
    let before = head(root.path());
    let between = commit_on(root.path(), &before, "between");
    let upstream_head = commit_on(root.path(), &between, "upstream");
    commit(root.path(), OFF, "Ada Lovelace");
    let sha = head(root.path());
    fetched(root.path(), &upstream_head);
    assert!(matches!(
        team_off(root.path()),
        TeamOff::Asks(Arrival::Local)
    ));

    // The commit between them, in the clone's store, now names the off as its parent.
    let bytes = forged(root.path(), &between, &sha);
    substitute(root.path(), &between, &bytes);
    let off = team_off(root.path());
    assert!(matches!(off, TeamOff::Asks(Arrival::Local)), "{off:?}");

    // A damaged object reads as local too.
    substitute(root.path(), &between, b"not an object");
    let off = team_off(root.path());
    assert!(matches!(off, TeamOff::Asks(Arrival::Local)), "{off:?}");
}

#[test]
fn a_substituted_object_on_an_off_the_upstream_does_hold_calls_it_local() {
    // The off is on the upstream, but a commit on the way to it reads other than it was written.
    let root = project(OFF, "Ada Lovelace");
    let sha = head(root.path());
    let next = commit_on(root.path(), &sha, "next");
    let upstream_head = commit_on(root.path(), &next, "and next");
    fetched(root.path(), &upstream_head);
    assert!(upstream(&team_off(root.path())).is_some());
    // Same parent, other bytes: a message changed under the same id.
    let reworded = {
        let read = crate::testgit::run(root.path(), &["cat-file", "commit", &next]);
        let file = root.path().join(".git/reworded");
        std::fs::write(&file, read.out.replace("next", "other")).unwrap();
        let written = crate::testgit::run(
            root.path(),
            &["hash-object", "-w", "-t", "commit", file.to_str().unwrap()],
        );
        assert!(written.ok(), "{written:?}");
        std::fs::read(loose(root.path(), written.out.trim())).unwrap()
    };
    substitute(root.path(), &next, &reworded);
    let off = team_off(root.path());
    assert!(matches!(off, TeamOff::Asks(Arrival::Local)), "{off:?}");
}

#[test]
fn a_grafts_file_calls_the_off_local() {
    let root = project("[sandbox]\nmode = \"on\"\n", "Ada");
    let before = head(root.path());
    let upstream_head = commit_on(root.path(), &before, "upstream");
    commit(root.path(), OFF, "Ada Lovelace");
    let sha = head(root.path());
    fetched(root.path(), &upstream_head);
    // A graft gives the upstream's commit the off as its parent.
    let info = root.path().join(".git/info");
    std::fs::create_dir_all(&info).unwrap();
    std::fs::write(info.join("grafts"), format!("{upstream_head} {sha}\n")).unwrap();
    let off = team_off(root.path());
    assert!(matches!(off, TeamOff::Asks(Arrival::Local)), "{off:?}");
    // And with the graft gone, the off is local still: it never was on the upstream.
    std::fs::remove_file(info.join("grafts")).unwrap();
    let off = team_off(root.path());
    assert!(matches!(off, TeamOff::Asks(Arrival::Local)), "{off:?}");
}

/// A project whose upstream's last commit, by Bob, changed the manifest without turning the
/// sandbox off, and a local commit on top that did: the root, the upstream's commit and the
/// local one.
fn an_off_on_top_of_the_upstream() -> (tempfile::TempDir, String, String) {
    let root = project("[sandbox]\nmode = \"on\"\n", "Ada");
    commit(root.path(), "[sandbox]\nmode = \"on\"\n# reviewed\n", "Bob");
    let upstream_head = head(root.path());
    fetched(root.path(), &upstream_head);
    commit(root.path(), OFF, "Eve");
    let local = head(root.path());
    assert!(matches!(
        team_off(root.path()),
        TeamOff::Asks(Arrival::Local)
    ));
    (root, upstream_head, local)
}

/// The id `spec` names, as `rev-parse` reads it.
fn id_of(root: &Path, spec: &str) -> String {
    let read = crate::testgit::run(root, &["rev-parse", spec]);
    assert!(read.ok(), "{read:?}");
    read.out.trim().to_owned()
}

#[test]
fn a_substituted_tree_of_an_upstream_commit_never_names_it_as_the_off_s() {
    let (root, upstream_head, local) = an_off_on_top_of_the_upstream();
    // The upstream commit's tree, in the clone's store, now holds the local commit's manifest.
    let tree = id_of(root.path(), &format!("{upstream_head}^{{tree}}"));
    let bytes = std::fs::read(loose(
        root.path(),
        &id_of(root.path(), &format!("{local}^{{tree}}")),
    ))
    .unwrap();
    substitute(root.path(), &tree, &bytes);
    let off = team_off(root.path());
    assert!(matches!(off, TeamOff::Asks(Arrival::Local)), "{off:?}");
    assert!(!Arrival::Local.said().contains("Bob"));
}

#[test]
fn a_substituted_manifest_of_an_upstream_commit_never_names_it_as_the_off_s() {
    let (root, upstream_head, local) = an_off_on_top_of_the_upstream();
    // The upstream commit's manifest, in the clone's store, now reads as the local one's.
    let manifest = id_of(root.path(), &format!("{upstream_head}:charter.toml"));
    let bytes = std::fs::read(loose(
        root.path(),
        &id_of(root.path(), &format!("{local}:charter.toml")),
    ))
    .unwrap();
    substitute(root.path(), &manifest, &bytes);
    let off = team_off(root.path());
    assert!(matches!(off, TeamOff::Asks(Arrival::Local)), "{off:?}");
}
