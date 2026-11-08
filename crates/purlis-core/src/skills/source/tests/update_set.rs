// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Several updates accepted at once (#5354): one commit, all or nothing, one Undo; and an update
//! that gains a capability lands only where it was accepted with it.

use super::*;
use crate::skills::source::update::Gained;
use crate::skills::source::update::set::{NotUndone, Updating, undo_update_set, update_set};
use crate::testgit::run;

const DEPLOY_V2: &[u8] = b"---\nname: deploy\ndescription: Ships it, then skills:notify.\n---\n\nRun scripts/ship.sh. Then say so.\n";
const NOTIFY_V2: &[u8] =
    b"---\nname: notify\ndescription: Tells the team.\n---\n\nPost to https://chat.example.com, briefly.\n";

/// A plane in commit mode, its first commit made.
fn git_plane() -> tempfile::TempDir {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".gitignore"),
        ".charter/\n.purlis/\nskills.local/\nskills.local.lock\ncharter.local.toml\n",
    )
    .unwrap();
    run(root, &["init", "-q", "-b", "main", "."]);
    run(root, &["config", "user.name", "Fixture"]);
    run(root, &["config", "user.email", "fixture@example.invalid"]);
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "one"]);
    tmp
}

/// `deploy` and `notify` installed at the project level from the good remote.
fn installed(root: &Path) -> (Upstream, String) {
    let (remote, first) = good_remote();
    let source = remote.source();
    let review = review(root, &source, &["deploy".to_owned()], &Level::project()).unwrap();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &review.accepted(), None)
    })
    .0
    .unwrap();
    (remote, first)
}

/// What accepting the update of `name` with every kind it gains says.
fn accepted_whole(root: &Path, name: &str) -> Updating {
    let diff = update_review(root, &Level::project(), name).unwrap();
    Updating {
        level: Level::project(),
        name: name.to_owned(),
        accepted: diff.accepted(),
        grant: diff.grant.clone(),
        allowed: diff.gained.clone(),
        held: None,
    }
}

fn read(root: &Path, path: &str) -> Vec<u8> {
    std::fs::read(root.join(path)).unwrap()
}

#[test]
fn two_updates_land_as_one_commit_and_one_undo_puts_both_back() {
    let tmp = git_plane();
    let root = tmp.path();
    let (remote, first) = installed(root);
    let lock_before = read(root, lock::FILE);
    let tree = remote.tree(&[
        ("skills/deploy/SKILL.md", "100644", DEPLOY_V2),
        ("skills/deploy/scripts/ship.sh", "100755", SHIP),
        ("skills/notify/SKILL.md", "100644", NOTIFY_V2),
    ]);
    let second = remote.commit(&tree, Some(&first));
    let set = [
        accepted_whole(root, "deploy"),
        accepted_whole(root, "notify"),
    ];
    assert!(set.iter().all(|one| one.allowed.is_empty()), "{set:?}");
    let head_before = run(root, &["rev-parse", "HEAD"]).out;

    let (undo, _) = on(root, |cx| update_set(cx, &set));
    let undo = undo.unwrap();
    assert_eq!(undo.count(), 2);
    assert_eq!(read(root, "skills/deploy/SKILL.md"), DEPLOY_V2);
    assert_eq!(read(root, "skills/notify/SKILL.md"), NOTIFY_V2);
    let lock = lock::read(root, Kept::Shared).unwrap().lock;
    assert!(lock.skills.iter().all(|one| one.pin.as_str() == second));
    // One commit after the one before, holding both and the lockfile.
    let parent = run(root, &["rev-parse", "HEAD~1"]).out;
    assert_eq!(parent, head_before);
    let subject = run(root, &["log", "-1", "--format=%s"]).out;
    assert!(subject.trim().ends_with("skill update 2"), "{subject}");
    let mut paths: Vec<String> = run(root, &["show", "--name-only", "--format=", "HEAD"])
        .out
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        [
            "skills.lock",
            "skills/deploy/SKILL.md",
            "skills/notify/SKILL.md"
        ]
    );

    on(root, |cx| undo_update_set(cx, &undo)).0.unwrap();
    assert_eq!(read(root, "skills/deploy/SKILL.md"), DEPLOY);
    assert_eq!(read(root, "skills/notify/SKILL.md"), NOTIFY);
    assert_eq!(read(root, lock::FILE), lock_before);
    let subject = run(root, &["log", "-1", "--format=%s"]).out;
    assert!(subject.trim().ends_with("skill update undo 2"), "{subject}");
    let status = run(root, &["status", "--porcelain"]).out;
    assert_eq!(status.trim(), "");
    assert_eq!(updates(root, None).offers.len(), 2);
}

#[test]
fn an_update_that_gains_a_script_lands_only_when_accepted_with_it() {
    let tmp = plane();
    let root = tmp.path();
    let (remote, first) = installed(root);
    let lock_before = read(root, lock::FILE);
    let tree = remote.tree(&[
        ("skills/deploy/SKILL.md", "100644", DEPLOY),
        ("skills/deploy/scripts/ship.sh", "100755", SHIP),
        ("skills/notify/SKILL.md", "100644", NOTIFY),
        (
            "skills/notify/scripts/ping.sh",
            "100755",
            b"#!/bin/sh\necho hi\n",
        ),
    ]);
    remote.commit(&tree, Some(&first));
    let whole = accepted_whole(root, "notify");
    assert_eq!(whole.allowed, [Gained::Script]);
    let unticked = Updating {
        allowed: Vec::new(),
        ..whole.clone()
    };

    let (refused, _) = on(root, |cx| update_set(cx, &[unticked]));
    let why = refused.unwrap_err();
    assert!(why.contains("open it and tick it on its own"), "{why}");
    assert!(!root.join("skills/notify/scripts/ping.sh").exists());
    assert_eq!(read(root, lock::FILE), lock_before);

    on(root, |cx| update_set(cx, &[whole])).0.unwrap();
    assert!(root.join("skills/notify/scripts/ping.sh").is_file());
}

#[test]
fn an_update_that_fails_part_way_takes_back_every_one_written_before_it() {
    let tmp = plane();
    let root = tmp.path();
    let (remote, first) = installed(root);
    let lock_before = read(root, lock::FILE);
    let tree = remote.tree(&[
        ("skills/deploy/SKILL.md", "100644", DEPLOY_V2),
        ("skills/deploy/scripts/ship.sh", "100755", SHIP),
        ("skills/deploy/notes.md", "100644", b"Notes.\n"),
        ("skills/notify/SKILL.md", "100644", NOTIFY_V2),
    ]);
    remote.commit(&tree, Some(&first));
    let set = [
        accepted_whole(root, "notify"),
        accepted_whole(root, "deploy"),
    ];
    // Where deploy's new file goes is a folder: its write fails after notify's landed.
    std::fs::create_dir_all(root.join("skills/deploy/notes.md")).unwrap();

    let (answer, _) = on(root, |cx| update_set(cx, &set));
    let why = answer.unwrap_err();
    assert!(why.contains("so no skill was updated"), "{why}");
    assert_eq!(read(root, "skills/notify/SKILL.md"), NOTIFY);
    assert_eq!(read(root, "skills/deploy/SKILL.md"), DEPLOY);
    assert_eq!(read(root, lock::FILE), lock_before);
}

#[test]
fn an_undo_is_refused_whole_where_a_skill_was_edited_since() {
    let tmp = plane();
    let root = tmp.path();
    let (remote, first) = installed(root);
    let tree = remote.tree(&[
        ("skills/deploy/SKILL.md", "100644", DEPLOY_V2),
        ("skills/deploy/scripts/ship.sh", "100755", SHIP),
        ("skills/notify/SKILL.md", "100644", NOTIFY_V2),
    ]);
    remote.commit(&tree, Some(&first));
    let set = [
        accepted_whole(root, "deploy"),
        accepted_whole(root, "notify"),
    ];
    let undo = on(root, |cx| update_set(cx, &set)).0.unwrap();
    std::fs::write(
        root.join("skills/notify/SKILL.md"),
        "---\nname: notify\ndescription: mine\n---\n",
    )
    .unwrap();

    let (answer, _) = on(root, |cx| undo_update_set(cx, &undo));
    let Err(NotUndone::Refused(why)) = answer else {
        panic!("{answer:?}");
    };
    assert!(why.contains("notify changed since it was updated"), "{why}");
    assert_eq!(read(root, "skills/deploy/SKILL.md"), DEPLOY_V2);
}

#[test]
fn an_update_that_brings_a_skill_along_takes_it_away_again_on_undo() {
    let tmp = plane();
    let root = tmp.path();
    let (remote, first) = installed(root);
    let tree = remote.tree(&[
        ("skills/deploy/SKILL.md", "100644", DEPLOY),
        ("skills/deploy/scripts/ship.sh", "100755", SHIP),
        (
            "skills/notify/SKILL.md",
            "100644",
            b"---\nname: notify\ndescription: Tells the team.\n---\n\nThen skills:format.\n",
        ),
        (
            "skills/format/SKILL.md",
            "100644",
            b"---\nname: format\ndescription: Formats.\n---\n",
        ),
    ]);
    remote.commit(&tree, Some(&first));
    let set = [accepted_whole(root, "notify")];
    let undo = on(root, |cx| update_set(cx, &set)).0.unwrap();
    assert!(root.join("skills/format/SKILL.md").is_file());

    on(root, |cx| undo_update_set(cx, &undo)).0.unwrap();
    assert!(!root.join("skills/format").exists());
    let lock = lock::read(root, Kept::Shared).unwrap().lock;
    assert!(lock.skills.iter().all(|one| one.name != "format"));
}

#[test]
fn an_undo_leaves_a_skill_installed_after_the_update_as_it_is() {
    let tmp = plane();
    let root = tmp.path();
    let (remote, first) = installed(root);
    let format: &[u8] = b"---\nname: format\ndescription: Formats.\n---\n";
    let tree = remote.tree(&[
        ("skills/deploy/SKILL.md", "100644", DEPLOY),
        ("skills/deploy/scripts/ship.sh", "100755", SHIP),
        ("skills/notify/SKILL.md", "100644", NOTIFY_V2),
        ("skills/format/SKILL.md", "100644", format),
    ]);
    remote.commit(&tree, Some(&first));
    let undo = on(root, |cx| update_set(cx, &[accepted_whole(root, "notify")]))
        .0
        .unwrap();
    // Installed after the update: no Undo of the update takes it out.
    let source = remote.source();
    let review = review(root, &source, &["format".to_owned()], &Level::project()).unwrap();
    on(root, |cx| {
        install(cx, &source, &Level::project(), &review.accepted(), None)
    })
    .0
    .unwrap();

    on(root, |cx| undo_update_set(cx, &undo)).0.unwrap();
    assert_eq!(read(root, "skills/notify/SKILL.md"), NOTIFY);
    assert_eq!(read(root, "skills/format/SKILL.md"), format);
    let lock = lock::read(root, Kept::Shared).unwrap().lock;
    assert!(lock.skills.iter().any(|one| one.name == "format"));
}

#[test]
fn an_update_that_gains_a_hook_is_refused_unless_accepted_with_it() {
    let tmp = plane();
    let root = tmp.path();
    let (remote, first) = installed(root);
    let hooked: &[u8] = b"---\nname: notify\ndescription: Tells the team.\nhooks:\n  Stop:\n    - command: ./x.sh\n---\n\nPost to https://chat.example.com.\n";
    let tree = remote.tree(&[
        ("skills/deploy/SKILL.md", "100644", DEPLOY),
        ("skills/deploy/scripts/ship.sh", "100755", SHIP),
        ("skills/notify/SKILL.md", "100644", hooked),
    ]);
    remote.commit(&tree, Some(&first));
    let whole = accepted_whole(root, "notify");
    assert_eq!(whole.allowed, [Gained::Hook]);
    let unticked = Updating {
        allowed: vec![Gained::Script],
        ..whole
    };
    let (answer, _) = on(root, |cx| update_set(cx, &[unticked]));
    assert!(answer.unwrap_err().contains("(a hook)"));
    assert_eq!(read(root, "skills/notify/SKILL.md"), NOTIFY);
}
