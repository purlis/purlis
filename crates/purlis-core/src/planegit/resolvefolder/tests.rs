// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Resolving off to the side, against real git: two clones of one plane whose words conflict.

use super::*;
use crate::planegit::tests::twoclones::{Two, commit, read, run, write};

const SHARED: &str = "one\ntwo\nthree\n";

/// Two machines that each changed `shared.md`'s first line their own way, and the remote's
/// fetched: a merge stops on it.
fn conflicting() -> Two {
    let two = Two::new(&[("shared.md", SHARED), ("other.md", "other\n")]);
    write(&two.mine, "shared.md", &SHARED.replace("one", "one, mine"));
    write(&two.mine, "mine.md", "mine\n");
    commit(&two.mine, "mine");
    write(
        &two.theirs,
        "shared.md",
        &SHARED.replace("one", "one, theirs"),
    );
    write(&two.theirs, "theirs.md", "theirs\n");
    two.land_theirs("theirs");
    two
}

fn said(_: Say) {}

fn shown(files: &[&str]) -> Vec<String> {
    files.iter().map(|f| (*f).to_owned()).collect()
}

fn root_is_as_it_was(two: &Two, head: &str) {
    assert_eq!(two.head(), head, "HEAD never moved");
    assert!(
        crate::gitstate::stopped(&two.mine).is_none(),
        "nothing stopped at the root"
    );
    assert!(!two.mine.join(".git/MERGE_HEAD").exists());
    assert!(!two.mine.join(".git/rebase-merge").exists());
    assert_eq!(
        read(&two.mine, "shared.md"),
        SHARED.replace("one", "one, mine")
    );
}

#[test]
fn a_conflict_is_settled_in_a_resolve_folder_and_the_root_never_stops() {
    let two = conflicting();
    let before = two.head();

    assert_eq!(cut(&two.mine, "release"), Ok(vec!["shared.md".to_owned()]));

    let folder = folder(&two.mine);
    root_is_as_it_was(&two, &before);
    // The folder holds the merge: the remote's file in, and the person's conflict marked, with
    // git's own unmerged sides in its index.
    assert_eq!(read(&folder, "theirs.md"), "theirs\n");
    assert!(read(&folder, "shared.md").contains("<<<<<<<"));
    assert_eq!(
        crate::gitstate::unmerged(&folder),
        vec!["shared.md".to_owned()]
    );
    assert_eq!(
        conflicts(&two.mine),
        Some(Conflicts {
            marked: vec!["shared.md".into()],
            by_hand: Vec::new(),
            keyed: Vec::new(),
        })
    );
    assert_eq!(sides(&two.mine), Some(("theirs".into(), "mine".into())));
    // A save at the root never takes the folder in.
    assert!(
        !crate::planegit::standing(&two.mine)
            .changed
            .iter()
            .any(|path| path.contains("resolve")),
        "{:?}",
        crate::planegit::standing(&two.mine).changed
    );

    // Markers left: refused, and nothing moved.
    let refused = carry_on(&two.mine, &shown(&["shared.md"]), &[], &mut said);
    assert!(
        matches!(&refused, Settled::Refused(why) if why.contains("conflict markers")),
        "{refused:?}"
    );
    root_is_as_it_was(&two, &before);

    write(&folder, "shared.md", "one, both\ntwo\nthree\n");
    assert_eq!(
        carry_on(&two.mine, &shown(&["shared.md"]), &[], &mut said),
        Settled::Replayed
    );

    // The root moved once, onto a merge of both, and the folder, its branch and record are gone.
    assert_eq!(read(&two.mine, "shared.md"), "one, both\ntwo\nthree\n");
    assert_eq!(read(&two.mine, "theirs.md"), "theirs\n");
    assert_eq!(
        run(&two.mine, &["rev-list", "--parents", "-1", "HEAD"])
            .split(' ')
            .count(),
        3,
        "a merge of both sides"
    );
    assert_eq!(run(&two.mine, &["rev-parse", "HEAD^1"]), before);
    assert_eq!(
        run(&two.mine, &["log", "-1", "--format=%s"]),
        "Bring release into this machine's saves"
    );
    assert!(!folder.exists());
    assert!(resolving(&two.mine).is_none());
    assert!(
        crate::testgit::run(&two.mine, &["rev-parse", "--verify", "-q", BRANCH])
            .out
            .is_empty()
    );
    assert_eq!(run(&two.mine, &["status", "--porcelain"]), "");
}

#[test]
fn a_quit_mid_resolve_leaves_the_root_untouched_and_the_folder_is_found_again() {
    let two = conflicting();
    let before = two.head();
    cut(&two.mine, "release").unwrap();
    write(&folder(&two.mine), "shared.md", "one, both\ntwo\nthree\n");

    // A quit: nothing in this process is kept, and what is on disk is all the next start reads.
    root_is_as_it_was(&two, &before);
    let found = resolving(&two.mine).expect("the resolve is found again");
    assert_eq!(found.conflicted, vec!["shared.md".to_owned()]);
    assert_eq!(found.ours, before);

    assert_eq!(
        carry_on(&two.mine, &shown(&["shared.md"]), &[], &mut said),
        Settled::Replayed
    );
    assert_eq!(read(&two.mine, "shared.md"), "one, both\ntwo\nthree\n");
}

#[test]
fn a_folder_left_by_a_cut_a_quit_stopped_is_removed_by_the_next_resolve() {
    let two = conflicting();
    cut(&two.mine, "release").unwrap();
    // The record never written: the quit came before the folder was ready.
    std::fs::remove_file(two.mine.join(".git").join(RECORD)).unwrap();
    assert!(resolving(&two.mine).is_none());

    assert_eq!(cut(&two.mine, "release"), Ok(vec!["shared.md".to_owned()]));
    assert!(resolving(&two.mine).is_some());
}

#[test]
fn a_chat_writing_at_the_root_during_a_resolve_is_kept_and_a_clash_is_named() {
    let two = conflicting();
    cut(&two.mine, "release").unwrap();
    write(&folder(&two.mine), "shared.md", "one, both\ntwo\nthree\n");
    // A chat writes at the root meanwhile: a new file, and a file the merge changes.
    write(&two.mine, "chat.md", "a chat's words\n");
    write(&two.mine, "theirs.md", "a chat wrote here first\n");

    let refused = carry_on(&two.mine, &shown(&["shared.md"]), &[], &mut said);

    assert!(
        matches!(&refused, Settled::Refused(why) if why.contains("`theirs.md`")),
        "{refused:?}"
    );
    assert_eq!(read(&two.mine, "theirs.md"), "a chat wrote here first\n");
    assert!(resolving(&two.mine).is_some(), "the folder is kept");

    // The chat's clash undone: Continue goes on, and the chat's own file stays as it wrote it.
    std::fs::remove_file(two.mine.join("theirs.md")).unwrap();
    assert_eq!(
        carry_on(&two.mine, &shown(&["shared.md"]), &[], &mut said),
        Settled::Replayed
    );
    assert_eq!(read(&two.mine, "chat.md"), "a chat's words\n");
    assert_eq!(read(&two.mine, "theirs.md"), "theirs\n");
}

#[test]
fn a_save_made_at_the_root_during_a_resolve_is_carried_onto_the_merge() {
    let two = conflicting();
    cut(&two.mine, "release").unwrap();
    write(&folder(&two.mine), "shared.md", "one, both\ntwo\nthree\n");
    write(&two.mine, "later.md", "saved while resolving\n");
    commit(&two.mine, "later");

    assert_eq!(
        carry_on(&two.mine, &shown(&["shared.md"]), &[], &mut said),
        Settled::Replayed
    );

    assert_eq!(run(&two.mine, &["log", "-1", "--format=%s"]), "later");
    assert_eq!(
        run(&two.mine, &["log", "-1", "--format=%s", "HEAD^"]),
        "Bring release into this machine's saves"
    );
    assert_eq!(read(&two.mine, "shared.md"), "one, both\ntwo\nthree\n");
    assert_eq!(read(&two.mine, "later.md"), "saved while resolving\n");
}

#[test]
fn abort_removes_the_folder_and_changes_nothing_at_the_root() {
    let two = conflicting();
    let before = two.head();
    cut(&two.mine, "release").unwrap();
    write(&folder(&two.mine), "shared.md", "half settled\n");

    assert_eq!(abort(&two.mine), Settled::Aborted);

    root_is_as_it_was(&two, &before);
    assert!(!folder(&two.mine).exists());
    assert!(resolving(&two.mine).is_none());
    assert_eq!(run(&two.mine, &["status", "--porcelain"]), "");
    assert!(matches!(abort(&two.mine), Settled::Refused(_)));
}

#[test]
fn a_file_settled_outside_the_conflict_is_refused_and_nothing_moves() {
    let two = conflicting();
    let before = two.head();
    cut(&two.mine, "release").unwrap();
    let folder = folder(&two.mine);
    write(&folder, "shared.md", "one, both\ntwo\nthree\n");
    write(&folder, "other.md", "changed in the folder\n");
    run(&folder, &["add", "other.md"]);

    let refused = carry_on(&two.mine, &shown(&["shared.md"]), &[], &mut said);

    assert!(
        matches!(&refused, Settled::Refused(why) if why.contains("`other.md`")),
        "{refused:?}"
    );
    root_is_as_it_was(&two, &before);
}

#[test]
fn a_binary_conflict_is_settled_by_hand_in_the_folder() {
    let two = Two::new(&[("blob.bin", "seed\0")]);
    write(&two.mine, "blob.bin", "mine\0");
    commit(&two.mine, "mine");
    write(&two.theirs, "blob.bin", "theirs\0");
    two.land_theirs("theirs");
    cut(&two.mine, "release").unwrap();
    let folder = folder(&two.mine);
    assert_eq!(
        conflicts(&two.mine).unwrap().by_hand,
        vec!["blob.bin".to_owned()]
    );

    let refused = carry_on(&two.mine, &[], &[], &mut said);
    assert!(
        matches!(&refused, Settled::Refused(why) if why.contains("git checkout --theirs")),
        "{refused:?}"
    );

    // This machine's side kept: there is a merge to commit.
    run(&folder, &["checkout", "--ours", "--", "blob.bin"]);
    run(&folder, &["add", "--", "blob.bin"]);
    assert_eq!(carry_on(&two.mine, &[], &[], &mut said), Settled::Replayed);
    assert_eq!(read(&two.mine, "blob.bin"), "mine\0");
}

#[test]
fn skip_drops_this_machines_side_where_what_is_settled_brings_nothing() {
    let two = conflicting();
    cut(&two.mine, "release").unwrap();
    let folder = folder(&two.mine);
    write(&folder, "shared.md", &SHARED.replace("one", "one, theirs"));
    std::fs::remove_file(folder.join("mine.md")).unwrap();
    run(&folder, &["rm", "-q", "--cached", "mine.md"]);

    // `mine.md` is outside the conflict: refused.
    assert!(matches!(skip(&two.mine), Settled::Refused(_)));

    run(&folder, &["reset", "-q", "--", "mine.md"]);
    run(&folder, &["checkout", "--", "mine.md"]);
    let refused = skip(&two.mine);
    assert!(
        matches!(&refused, Settled::Refused(why) if why.contains("Skip drops only")),
        "{refused:?}"
    );
}

#[test]
fn resolve_replays_a_push_mode_planes_commits_with_no_rebase_and_moves_once() {
    let two = Two::new(&[("shared.md", SHARED)]);
    write(&two.mine, "mine.md", "mine\n");
    commit(&two.mine, "mine");
    write(&two.theirs, "theirs.md", "theirs\n");
    two.land_theirs("theirs");
    write(&two.mine, "unsaved.md", "a chat's words\n");

    assert_eq!(start_fetched(&two.mine, &mut said), Settled::Replayed);

    assert_eq!(
        run(&two.mine, &["log", "--format=%s", "-2"]),
        "mine\ntheirs"
    );
    assert_eq!(read(&two.mine, "unsaved.md"), "a chat's words\n");
    assert!(!folder(&two.mine).exists());
}

#[test]
fn resolve_cuts_a_folder_for_a_conflict_and_a_second_press_finds_the_same_one() {
    let two = conflicting();
    let before = two.head();

    assert_eq!(
        start_fetched(&two.mine, &mut said),
        Settled::Stopped(vec!["shared.md".to_owned()])
    );
    root_is_as_it_was(&two, &before);
    write(&folder(&two.mine), "shared.md", "settled already\n");

    assert_eq!(
        start_fetched(&two.mine, &mut said),
        Settled::Stopped(vec!["shared.md".to_owned()])
    );
    assert_eq!(read(&folder(&two.mine), "shared.md"), "settled already\n");
    let standing = crate::planegit::standing(&two.mine);
    assert_eq!(standing.stopped_by, Some(crate::planegit::RESOLVING));
    assert_eq!(standing.conflicts, vec!["shared.md".to_owned()]);
}

#[test]
fn resolve_in_a_request_mode_cuts_a_folder_and_brings_in_by_itself_what_does_not_conflict() {
    let two = conflicting();
    write(
        &two.mine,
        "charter.toml",
        "[plane]\nname = \"fixture\"\nbranch = \"release\"\nmode = \"pr\"\n",
    );
    commit(&two.mine, "request mode");

    assert_eq!(
        start_fetched(&two.mine, &mut said),
        Settled::Stopped(vec!["shared.md".to_owned()])
    );
    write(&folder(&two.mine), "shared.md", "one, both\ntwo\nthree\n");
    assert_eq!(
        carry_on(&two.mine, &shown(&["shared.md"]), &[], &mut said),
        Settled::Replayed
    );
    assert!(read(&two.mine, "charter.toml").contains("mode = \"pr\""));
}

/// **A key both machines changed in `charter.toml` is chosen by key, and no marker is ever
/// written into it** (#5852): the folder's settings file has the sets merged and this machine's
/// value standing in, Continue waits for a choice, and the choice is what lands, whatever a chat
/// wrote into the folder's copy.
#[test]
fn a_charter_toml_key_conflict_is_settled_in_the_resolve_folder_with_no_marker() {
    let seed = "[plane]\nname = \"fixture\"\nbranch = \"release\"\n\n[sandbox]\nmode = \"project\"\nhosts = [\"a.example\"]\n";
    let two = Two::new(&[]);
    write(&two.mine, "charter.toml", seed);
    commit(&two.mine, "settings");
    run(&two.mine, &["push", "-q", "origin", "release"]);
    run(&two.theirs, &["pull", "-q", "origin", "release"]);
    write(
        &two.mine,
        "charter.toml",
        &seed
            .replace("\"project\"", "\"workspace\"")
            .replace("[\"a.example\"]", "[\"a.example\", \"mine.example\"]"),
    );
    commit(&two.mine, "mine");
    write(
        &two.theirs,
        "charter.toml",
        &seed
            .replace("\"project\"", "\"off\"")
            .replace("[\"a.example\"]", "[\"a.example\", \"theirs.example\"]"),
    );
    two.land_theirs("theirs");
    // The window, under no chat's sandbox, whatever runs these tests.
    let _the_window = crate::sandbox::held::stand_in(&[]);

    assert_eq!(
        cut(&two.mine, "release"),
        Ok(vec!["charter.toml".to_owned()])
    );

    let folder = folder(&two.mine);
    let written = read(&folder, "charter.toml");
    assert!(
        !written.contains("<<<<<<<") && !written.contains(">>>>>>>"),
        "{written}"
    );
    assert!(written.contains("mode = \"workspace\""), "{written}");
    assert!(
        written.contains("theirs.example") && written.contains("mine.example"),
        "{written}"
    );
    assert_eq!(
        conflicts(&two.mine),
        Some(Conflicts {
            marked: Vec::new(),
            by_hand: Vec::new(),
            keyed: vec!["charter.toml".into()],
        })
    );
    assert_eq!(
        resolving(&two.mine).unwrap().keys,
        vec![ResolvingKey {
            file: "charter.toml".into(),
            key: "sandbox.mode".into(),
            ours: Some("\"workspace\"".into()),
            theirs: Some("\"off\"".into()),
        }]
    );

    // The standing names the file still to settle, as the Saving view lists it.
    assert_eq!(
        crate::planegit::standing(&two.mine).conflicts,
        vec!["charter.toml".to_owned()]
    );

    // No choice yet: refused, naming the key.
    let refused = carry_on(&two.mine, &[], &[], &mut said);
    assert!(
        matches!(&refused, Settled::Refused(why) if why.contains("`sandbox.mode`")),
        "{refused:?}"
    );

    // A chat writes the folder's copy: what lands is still the person's choice.
    write(
        &folder,
        "charter.toml",
        "[sandbox]\nmode = \"off\"\nhosts = [\"evil.example\"]\n",
    );
    let theirs = Chosen {
        file: "charter.toml".into(),
        key: "sandbox.mode".into(),
        side: Side::Theirs,
    };
    assert_eq!(
        carry_on(&two.mine, &[], &[theirs], &mut said),
        Settled::Replayed
    );

    let landed = read(&two.mine, "charter.toml");
    assert!(landed.contains("mode = \"off\""), "{landed}");
    assert!(
        landed.contains("mine.example") && landed.contains("theirs.example"),
        "{landed}"
    );
    assert!(!landed.contains("evil.example"), "{landed}");
    assert!(!landed.contains("<<<<<<<"), "{landed}");
}

/// **A resolve folder is held only while its gitfile is the one cut for it**: it names this
/// plane's own `.git/worktrees/<folder>` entry, and that entry names it back. Repointed (here at
/// the root's own `.git`), nothing runs git in it: Continue and Skip refuse and say so, and
/// Abort removes the folder without asking git there. The root is never touched.
#[test]
fn a_resolve_folder_whose_gitfile_is_repointed_is_refused_and_the_root_is_untouched() {
    let two = conflicting();
    let before = two.head();
    cut(&two.mine, "release").unwrap();
    let folder = folder(&two.mine);
    write(&folder, "shared.md", "one, both\ntwo\nthree\n");
    let root_git = two.mine.join(".git");
    std::fs::write(
        folder.join(".git"),
        format!("gitdir: {}\n", root_git.display()),
    )
    .unwrap();
    let index = std::fs::read(root_git.join("index")).unwrap();

    assert!(resolving(&two.mine).is_none());
    // Still shown as open, so the window offers Abort, with nothing read from the folder.
    assert_eq!(conflicts(&two.mine), Some(Conflicts::default()));
    for refused in [
        carry_on(&two.mine, &shown(&["shared.md"]), &[], &mut said),
        skip(&two.mine),
    ] {
        assert!(
            matches!(&refused, Settled::Refused(why) if why.contains("not the one purlis made")),
            "{refused:?}"
        );
    }
    root_is_as_it_was(&two, &before);
    assert_eq!(
        std::fs::read(root_git.join("index")).unwrap(),
        index,
        "the root's index moved"
    );

    assert_eq!(abort(&two.mine), Settled::Aborted);
    assert!(!folder.exists());
    root_is_as_it_was(&two, &before);
    assert_eq!(
        std::fs::read(root_git.join("index")).unwrap(),
        index,
        "the root's index moved"
    );
    assert_eq!(run(&two.mine, &["status", "--porcelain"]), "");
    assert_eq!(
        run(&two.mine, &["worktree", "list", "--porcelain"])
            .matches("worktree ")
            .count(),
        1
    );
}
