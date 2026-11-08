// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Bringing a target in, against real git: two clones of one plane, and no forge.

use std::path::Path;

use super::*;

const PLANE_WS: &str = "workspaces/alpha";

use super::super::twoclones::{Two, commit, read, run, write};

const CHARTER: &str = "# alpha\n\n## Vision\n\nShip it\n\n## Context\n\nOne.\n\nTwo.\n\nThree.\n\n## Sessions\n\nNo session records yet.\n";

fn with_a_workspace() -> Two {
    Two::new(&[(&format!("{PLANE_WS}/workspace.md"), CHARTER)])
}

/// A pointer line each side wrote its own way, as two records leave it.
fn pointer(dir: &Path, line: &str) {
    let path = format!("{PLANE_WS}/workspace.md");
    let text = crate::mdsection::replace(&read(dir, &path), "Sessions", line);
    write(dir, &path, &text);
}

#[test]
fn what_each_side_wrote_in_a_charter_merges_around_the_pointer() {
    let two = with_a_workspace();
    let path = format!("{PLANE_WS}/workspace.md");
    let mine = read(&two.mine, &path).replace("One.", "One, mine.");
    write(&two.mine, &path, &mine);
    pointer(&two.mine, "1 session record — mine");
    commit(&two.mine, "mine");
    let theirs = read(&two.theirs, &path).replace("Three.", "Three, theirs.");
    write(&two.theirs, &path, &theirs);
    pointer(&two.theirs, "1 session record — theirs");
    two.land_theirs("theirs");

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Moved),
        BroughtIn::Moved(1)
    );

    let text = read(&two.mine, &path);
    assert!(
        text.contains("One, mine.") && text.contains("Three, theirs."),
        "{text}"
    );
    assert!(
        text.contains(crate::sessionrecord::none_yet(&two.mine).as_str()),
        "{text}"
    );
    assert!(
        !text.contains("<<<<<<<") && !text.contains(SET_ASIDE),
        "{text}"
    );
}

#[test]
fn a_conflict_in_a_charters_own_words_is_a_persons_and_nothing_moves() {
    let two = with_a_workspace();
    let path = format!("{PLANE_WS}/workspace.md");
    write(
        &two.mine,
        &path,
        &read(&two.mine, &path).replace("Ship it", "Ship it now"),
    );
    commit(&two.mine, "mine");
    write(
        &two.theirs,
        &path,
        &read(&two.theirs, &path).replace("Ship it", "Ship it later"),
    );
    two.land_theirs("theirs");
    let before = two.head();

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Moved),
        BroughtIn::Conflict(vec![path.clone()])
    );

    assert_eq!(two.head(), before);
    assert_eq!(run(&two.mine, &["status", "--porcelain"]), "");
    assert!(read(&two.mine, &path).contains("Ship it now"));
}

#[test]
fn a_file_with_uncommitted_work_in_the_way_waits_and_loses_nothing() {
    let two = with_a_workspace();
    write(&two.mine, "mine.md", "mine\n");
    commit(&two.mine, "mine");
    write(&two.mine, "shared.md", "not committed\n");
    write(&two.theirs, "shared.md", "theirs\n");
    two.land_theirs("theirs");
    let before = two.head();

    let came = bring_in(&two.mine, "release", false, When::Moved);

    assert!(matches!(came, BroughtIn::Waiting(_)), "{came:?}");
    assert_eq!(two.head(), before);
    assert_eq!(read(&two.mine, "shared.md"), "not committed\n");
}

#[test]
fn nothing_of_its_own_moves_onto_the_target_with_no_commit() {
    let two = with_a_workspace();
    write(&two.theirs, "theirs.md", "theirs\n");
    two.land_theirs("theirs");

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Moved),
        BroughtIn::Moved(1)
    );

    assert_eq!(two.head(), run(&two.mine, &["rev-parse", "origin/release"]));
}

#[test]
fn nothing_incoming_is_nothing() {
    let two = with_a_workspace();
    write(&two.mine, "mine.md", "mine\n");
    commit(&two.mine, "mine");
    assert_eq!(
        bring_in(&two.mine, "release", false, When::Moved),
        BroughtIn::Nothing
    );
}

#[test]
fn a_todo_done_on_one_side_loses_its_line_in_the_rebuilt_index() {
    let index = format!("{PLANE_WS}/todos/MEMORY.md");
    let (old, kept) = ("20260101-000000-old.md", "20260102-000000-kept.md");
    let two = Two::new(&[
        (&format!("{PLANE_WS}/workspace.md"), CHARTER),
        (
            &index,
            &format!("# Todos\n\n- [old]({old})\n- [kept]({kept})\n"),
        ),
        (&format!("{PLANE_WS}/todos/{old}"), "# old\n"),
        (&format!("{PLANE_WS}/todos/{kept}"), "# kept\n"),
    ]);
    // Mine: a new todo. Theirs: the old one done, and a new one.
    write(
        &two.mine,
        &format!("{PLANE_WS}/todos/20260103-000000-mine.md"),
        "# mine\n",
    );
    write(
        &two.mine,
        &index,
        &format!(
            "{}- [mine](20260103-000000-mine.md)\n",
            read(&two.mine, &index)
        ),
    );
    commit(&two.mine, "mine");
    std::fs::remove_file(two.theirs.join(PLANE_WS).join("todos").join(old)).unwrap();
    write(
        &two.theirs,
        &format!("{PLANE_WS}/todos/20260104-000000-theirs.md"),
        "# theirs\n",
    );
    write(
        &two.theirs,
        &index,
        &format!("# Todos\n\n- [kept]({kept})\n- [theirs](20260104-000000-theirs.md)\n"),
    );
    two.land_theirs("theirs");

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Moved),
        BroughtIn::Moved(1)
    );

    assert_eq!(
        read(&two.mine, &index),
        format!(
            "# Todos\n\n- [kept]({kept})\n- [mine](20260103-000000-mine.md)\n\
             - [theirs](20260104-000000-theirs.md)\n"
        )
    );
}

#[test]
fn the_plane_roots_own_session_index_is_rebuilt_too() {
    let two = Two::new(&[("sessions/index.md", "# Sessions — plane root\n\n")]);
    let record = |dir: &Path, name: &str, title: &str| {
        write(
            dir,
            &format!("sessions/{name}"),
            &format!("---\ntitle: {title}\n---\n"),
        );
        write(
            dir,
            "sessions/index.md",
            &format!("# Sessions — plane root\n\n- {title}\n"),
        );
    };
    record(&two.mine, "20260101-000000-mine.md", "mine");
    commit(&two.mine, "mine");
    record(&two.theirs, "20260102-000000-theirs.md", "theirs");
    two.land_theirs("theirs");

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Moved),
        BroughtIn::Moved(1)
    );

    let index = read(&two.mine, "sessions/index.md");
    let lines: Vec<&str> = index.lines().filter(|l| l.starts_with("- ")).collect();
    assert_eq!(lines.len(), 2, "{index}");
    assert!(
        lines[0].contains("theirs") && lines[1].contains("mine"),
        "{index}"
    );
}

#[test]
fn managed_merge_rules_that_differ_settle_as_this_builds_and_keep_a_persons_lines() {
    let block = "# >>> charter merge rules (managed by charter) >>>\nold merge=union\n# <<< charter merge rules <<<\n";
    let old = format!("first.bin binary\nsecond.bin binary\nthird.bin binary\n{block}");
    let two = Two::new(&[(".gitattributes", &old)]);
    write(
        &two.mine,
        ".gitattributes",
        &format!(
            "mine.bin binary\n{}",
            old.replace("old merge", "older merge")
        ),
    );
    commit(&two.mine, "mine");
    write(
        &two.theirs,
        ".gitattributes",
        &format!(
            "{}theirs.bin binary\n",
            old.replace("old merge", "newer merge")
        ),
    );
    two.land_theirs("theirs");

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Moved),
        BroughtIn::Moved(1)
    );

    let text = read(&two.mine, ".gitattributes");
    assert!(
        text.contains("mine.bin binary") && text.contains("theirs.bin binary"),
        "{text}"
    );
    assert!(
        text.contains("workspaces/*/todos/MEMORY.md merge=union"),
        "{text}"
    );
    // The remote's rule this build does not write stays, as a newer build's would; this
    // machine's own stale one goes. The block keeps its place, under the person's first lines.
    assert!(!text.contains("older merge"), "{text}");
    assert!(text.contains("newer merge=union"), "{text}");
    let at = |line: &str| text.find(line).unwrap_or_else(|| panic!("{line}: {text}"));
    assert!(
        at("third.bin binary") < at("# >>> charter merge rules"),
        "{text}"
    );
    assert!(
        at("# <<< charter merge rules") < at("theirs.bin binary"),
        "{text}"
    );
    assert!(!text.contains(RULES_SET_ASIDE), "{text}");
}

/// **What a rebase would have done** (#5338's design choice, kept true): a line of commits that
/// each wrote the pointer stops on every one when replayed with the line settled to what is true
/// (taking the commit's own line would stop once, and leave the count wrong), and the merge stops
/// on none.
#[test]
fn a_line_of_saves_each_writing_the_pointer_stops_a_rebase_on_each_and_a_merge_on_none() {
    let two = with_a_workspace();
    for n in 1..=3 {
        pointer(&two.mine, &format!("{n} session records — mine"));
        commit(&two.mine, &format!("mine {n}"));
    }
    pointer(&two.theirs, "1 session record — theirs");
    two.land_theirs("theirs");

    let tried = tempfile::tempdir().unwrap();
    let clone = tried.path().join("clone");
    run(
        tried.path(),
        &[
            "clone",
            "-q",
            two.mine.to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    run(&clone, &["config", "user.name", "Clone"]);
    run(&clone, &["config", "user.email", "clone@example.invalid"]);
    let theirs_head = run(&two.mine, &["rev-parse", "origin/release"]);
    run(
        &clone,
        &["fetch", "-q", two.mine.to_str().unwrap(), &theirs_head],
    );
    let mut stops = 0;
    let mut replayed = crate::testgit::run(&clone, &["rebase", &theirs_head]).ok();
    while !replayed && stops < 10 {
        stops += 1;
        run(
            &clone,
            &[
                "checkout",
                "--theirs",
                "--",
                &format!("{PLANE_WS}/workspace.md"),
            ],
        );
        pointer(&clone, &format!("{} session records — both", stops + 1));
        run(&clone, &["add", "-A"]);
        let on = ["-c", "core.editor=true", "rebase", "--continue"];
        replayed = crate::testgit::run(&clone, &on).ok();
    }
    assert_eq!(stops, 3, "the rebase stops once for each commit");

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Moved),
        BroughtIn::Moved(1)
    );
}

/// **A move that cannot finish does not start** (qw346): under a chat's sandbox, what the target
/// changes in a file the chat may not write leaves the plane where it was, for the window's look.
#[test]
fn a_move_a_chats_sandbox_would_stop_waits_for_the_window() {
    let two = Two::new(&[("Cargo.toml", "[package]\n")]);
    write(&two.mine, "mine.md", "mine\n");
    commit(&two.mine, "mine");
    write(&two.theirs, "Cargo.toml", "[package]\nname = \"theirs\"\n");
    two.land_theirs("theirs");
    let before = two.head();

    let _stood_in = crate::sandbox::held::stand_in(&["Cargo.toml"]);
    let came = bring_in(&two.mine, "release", false, When::Moved);

    assert!(
        matches!(&came, BroughtIn::Waiting(why) if why.contains("from the window")),
        "{came:?}"
    );
    assert_eq!(two.head(), before);
    assert_eq!(read(&two.mine, "Cargo.toml"), "[package]\n");
}

/// **A request that merges as it is is left** (#5338 review): a moved target with nothing that
/// conflicts and nothing purlis generates to rebuild makes no merge commit, so the request keeps
/// its head and a repository that keeps its history linear can still take it.
#[test]
fn a_request_that_merges_as_it_is_is_left_where_it_is() {
    let two = with_a_workspace();
    write(&two.mine, "mine.md", "mine\n");
    commit(&two.mine, "mine");
    write(&two.theirs, "theirs.md", "theirs\n");
    two.land_theirs("theirs");
    let before = two.head();

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Needed),
        BroughtIn::Nothing
    );
    assert_eq!(two.head(), before);

    // The forge saying it cannot merge brings it in all the same.
    assert_eq!(
        bring_in(&two.mine, "release", false, When::Moved),
        BroughtIn::Moved(1)
    );
    assert_ne!(two.head(), before);
}

/// What git's merge would stop on is needed, and is brought in.
#[test]
fn a_pointer_both_sides_wrote_is_needed_and_brought_in() {
    let two = with_a_workspace();
    pointer(&two.mine, "1 session record — mine");
    commit(&two.mine, "mine");
    pointer(&two.theirs, "1 session record — theirs");
    two.land_theirs("theirs");

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Needed),
        BroughtIn::Moved(1)
    );
    let parents = run(&two.mine, &["rev-list", "--parents", "-n", "1", "HEAD"]);
    assert_eq!(parents.split(' ').count(), 3, "a merge: {parents}");
}

/// **A merged request's newer saves are carried, not merged** (#5338 review): the target holds
/// the request as a copy (a squash or rebase merge), so a merge with the plane's history would
/// bring the original commit back beside it. One commit on the target carries what was saved
/// since, and the original is no longer in the plane's history.
#[test]
fn a_merged_requests_newer_saves_are_carried_onto_the_target_as_one_commit() {
    let two = with_a_workspace();
    let path = format!("{PLANE_WS}/workspace.md");
    pointer(&two.mine, "1 session record — the request");
    commit(&two.mine, "the request");
    let request = two.head();
    // Its copy lands on the target, with a later change of the pointer of its own.
    pointer(&two.theirs, "1 session record — the request");
    commit(&two.theirs, "the request, squashed");
    pointer(&two.theirs, "2 session records — theirs");
    two.land_theirs("theirs");
    // Saved since the request, and conflicting with the target on the pointer.
    write(
        &two.mine,
        &path,
        &read(&two.mine, &path).replace("One.", "One, since."),
    );
    pointer(&two.mine, "2 session records — mine");
    commit(&two.mine, "since");

    assert_eq!(
        bring_in(&two.mine, "release", false, When::MergedAt(&request)),
        BroughtIn::Moved(2)
    );

    let target = run(&two.mine, &["rev-parse", "origin/release"]);
    let parents = run(&two.mine, &["rev-list", "--parents", "-n", "1", "HEAD"]);
    assert_eq!(
        parents,
        format!("{} {target}", two.head()),
        "one parent, the target"
    );
    let original = crate::testgit::run(
        &two.mine,
        &["merge-base", "--is-ancestor", &request, "HEAD"],
    );
    assert!(!original.ok(), "the request's original came back");
    let text = read(&two.mine, &path);
    assert!(text.contains("One, since."), "{text}");
    assert!(!text.contains("<<<<<<<"), "{text}");
}

/// Markers left in a person's file name the sides as a person reads them.
#[test]
fn a_persons_conflict_is_marked_with_the_sides_names() {
    let two = with_a_workspace();
    let path = format!("{PLANE_WS}/workspace.md");
    write(
        &two.mine,
        &path,
        &read(&two.mine, &path).replace("Ship it", "Ship it now"),
    );
    commit(&two.mine, "mine");
    write(
        &two.theirs,
        &path,
        &read(&two.theirs, &path).replace("Ship it", "Ship it later"),
    );
    two.land_theirs("theirs");

    let planned = plan(&two.mine, "refs/remotes/origin/release", None).unwrap();
    let text = &planned.marked[&path];
    assert!(text.contains("<<<<<<< HEAD\n"), "{text}");
    assert!(text.contains(">>>>>>> origin/release\n"), "{text}");
}
