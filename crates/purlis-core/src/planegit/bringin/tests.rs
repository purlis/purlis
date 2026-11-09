// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Bringing a target in, against real git: two clones of one plane, and no forge.

use std::path::Path;

use super::*;

const PLANE_WS: &str = "workspaces/alpha";

use super::super::tests::twoclones::{Two, commit, read, run, write};

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
        BroughtIn::Conflict(Conflicted {
            files: vec![path.clone()],
            keys: BTreeMap::new(),
        })
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

/// Append a memory's file and its index line to a store, as `remember` does.
fn remember(dir: &Path, store: &str, file: &str, title: &str) {
    write(dir, &format!("{store}/{file}"), &format!("# {title}\n"));
    let index = format!("{store}/MEMORY.md");
    let text = read(dir, &index);
    write(dir, &index, &format!("{text}- [{title}]({file})\n"));
}

/// **A memory index is rebuilt, not stitched** (#5824 F4): the forge ignores `merge=union`, so
/// two machines that each remembered something conflict on the index. It is written from the
/// memory files of the merged tree: a memory forgotten on one side stays gone, one both sides
/// added appears once, and the lines keep the order this machine had, the other's after them.
#[test]
fn a_memory_index_both_sides_added_to_is_rebuilt_not_stitched() {
    let store = "personas/devops/memory";
    let index = format!("{store}/MEMORY.md");
    let header = "# Memory Index — devops\n\nOne line per memory.\n";
    let two = Two::new(&[
        (
            &index,
            &format!("{header}- [Alpha](alpha.md)\n- [Gone](gone.md)\n"),
        ),
        (&format!("{store}/alpha.md"), "# Alpha\n"),
        (&format!("{store}/gone.md"), "# Gone\n"),
    ]);
    remember(&two.mine, store, "zeta.md", "Zeta");
    remember(&two.mine, store, "both.md", "Both");
    commit(&two.mine, "mine");
    std::fs::remove_file(two.theirs.join(store).join("gone.md")).unwrap();
    write(
        &two.theirs,
        &index,
        &format!("{header}- [Alpha](alpha.md)\n"),
    );
    remember(&two.theirs, store, "beta.md", "Beta");
    remember(&two.theirs, store, "both.md", "Both");
    two.land_theirs("theirs");

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Needed),
        BroughtIn::Moved(1)
    );

    assert_eq!(
        read(&two.mine, &index),
        format!(
            "{header}- [Alpha](alpha.md)\n- [Zeta](zeta.md)\n- [Both](both.md)\n\
             - [Beta](beta.md)\n"
        )
    );
}

/// A workspace's memory files start with the time they were written, so its index is rebuilt
/// in the order of their names; a title one side changed is kept.
#[test]
fn a_workspace_memory_index_is_rebuilt_in_time_order_with_a_retitle_kept() {
    let store = format!("{PLANE_WS}/memory");
    let index = format!("{store}/MEMORY.md");
    let (first, mine, theirs) = (
        "20260101-000000-first.md",
        "20260103-000000-mine.md",
        "20260102-000000-theirs.md",
    );
    let two = Two::new(&[
        (
            &index,
            &format!("# alpha — task memory\n\n- [First]({first})\n"),
        ),
        (&format!("{store}/{first}"), "# First\n"),
    ]);
    remember(&two.mine, &store, mine, "Mine");
    commit(&two.mine, "mine");
    write(
        &two.theirs,
        &index,
        &format!("# alpha — task memory\n\n- [First, retitled]({first})\n"),
    );
    remember(&two.theirs, &store, theirs, "Theirs");
    two.land_theirs("theirs");

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Moved),
        BroughtIn::Moved(1)
    );

    assert_eq!(
        read(&two.mine, &index),
        format!(
            "# alpha — task memory\n\n- [First, retitled]({first})\n- [Theirs]({theirs})\n\
             - [Mine]({mine})\n"
        )
    );
}

/// A `charter.toml` with a comment, a sandbox and its hosts as the writer before #5824 left them:
/// one line, that every Always allow appended to.
fn settings(hosts: &str) -> String {
    format!(
        "# The project's settings.\n[plane]\nname = \"fixture\"\n\n[sandbox]\nmode = \"on\" # on \
         for everyone\nhosts = [{hosts}]\n"
    )
}

/// **Host allows merge as a set** (#5824 F5, D4): two machines that each allowed a host made a
/// request that could not merge, on the one line both appended to. The list is merged as a set:
/// both machines' hosts, without the one this machine removed, written one per line, sorted, with
/// the file's comments kept.
#[test]
fn hosts_both_machines_allowed_merge_as_a_set_and_a_removal_holds() {
    let two = Two::new(&[("charter.toml", &settings("\"x.example\", \"y.example\""))]);
    write(
        &two.mine,
        "charter.toml",
        &settings("\"x.example\", \"a.example\""),
    );
    commit(&two.mine, "mine: a allowed, y removed");
    write(
        &two.theirs,
        "charter.toml",
        &settings("\"x.example\", \"y.example\", \"b.example\""),
    );
    two.land_theirs("theirs: b allowed");
    // A chat's sandbox holds `charter.toml`, so the window makes this move; it stands in here
    // for a chat running the test.
    let _the_window = crate::sandbox::held::stand_in(&[]);

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Needed),
        BroughtIn::Moved(1)
    );

    assert_eq!(
        read(&two.mine, "charter.toml"),
        "# The project's settings.\n[plane]\nname = \"fixture\"\n\n[sandbox]\nmode = \"on\" # on \
         for everyone\nhosts = [\n    \"a.example\",\n    \"b.example\",\n    \"x.example\",\n]\n"
    );
}

/// **The rest of `charter.toml` merges key by key** (#5824 F6): a key only one machine changed
/// takes that machine's value, whatever else conflicted on the lines around it.
#[test]
fn a_key_only_one_machine_changed_in_charter_toml_takes_that_side() {
    let base = "[plane]\nname = \"fixture\"\n\n[sandbox]\nmode = \"on\"\nhosts = [\"x.example\"]\n";
    let two = Two::new(&[("charter.toml", base)]);
    write(
        &two.mine,
        "charter.toml",
        "[plane]\nname = \"fixture\"\nautosave = true # mine\n\n[sandbox]\nmode = \"on\"\nhosts = \
         [\"x.example\", \"a.example\"]\n",
    );
    commit(&two.mine, "mine");
    write(
        &two.theirs,
        "charter.toml",
        "[plane]\nname = \"fixture\"\n\n[sandbox]\nmode = \"off\" # theirs\nhosts = [\"x.example\", \
         \"b.example\"]\n\n[theme]\nname = \"dusk\"\n",
    );
    two.land_theirs("theirs");
    let _the_window = crate::sandbox::held::stand_in(&[]);

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Needed),
        BroughtIn::Moved(1)
    );

    assert_eq!(
        read(&two.mine, "charter.toml"),
        "[plane]\nname = \"fixture\"\nautosave = true # mine\n\n[sandbox]\nmode = \"off\" # theirs\n\
         hosts = [\n    \"a.example\",\n    \"b.example\",\n    \"x.example\",\n]\n\n[theme]\nname \
         = \"dusk\"\n"
    );
}

/// A key both machines changed, each its own way, is a person's conflict: the save names the file
/// and the key, nothing moves, and no marker is written into the TOML.
#[test]
fn a_key_both_machines_changed_in_charter_toml_is_a_persons_conflict_named_by_its_key() {
    let base = "[sandbox]\nmode = \"on\"\nhosts = [\"x.example\"]\n";
    let two = Two::new(&[("charter.toml", base)]);
    write(
        &two.mine,
        "charter.toml",
        "[sandbox]\nmode = \"off\"\nhosts = [\"x.example\", \"a.example\"]\n",
    );
    commit(&two.mine, "mine");
    write(
        &two.theirs,
        "charter.toml",
        "[sandbox]\nmode = \"ask\"\nhosts = [\"x.example\", \"b.example\"]\n",
    );
    two.land_theirs("theirs");
    let before = two.head();
    let _the_window = crate::sandbox::held::stand_in(&[]);

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Needed),
        BroughtIn::Conflict(Conflicted {
            files: vec!["charter.toml".to_owned()],
            keys: BTreeMap::from([("charter.toml".to_owned(), vec!["sandbox.mode".to_owned()])]),
        })
    );

    assert_eq!(two.head(), before);
    assert_eq!(run(&two.mine, &["status", "--porcelain"]), "");
    assert!(!read(&two.mine, "charter.toml").contains("<<<<<<<"));
    let planned = plan(&two.mine, "refs/remotes/origin/release", None).unwrap();
    assert!(!planned.marked.contains_key("charter.toml"));
}

/// What a save records of a conflict and the window reads back, by way of the push record.
fn as_a_save_records_it(two: &Two, conflict: Conflicted) -> super::super::Standing {
    let head = two.head();
    super::super::record_push(
        &two.mine,
        super::super::PushResult::conflicted("release", conflict),
        &head,
    );
    super::super::standing(&two.mine)
}

fn quiet() -> impl FnMut(crate::repocmd::Say) {
    |_| {}
}

/// **Continue settles a `charter.toml` key conflict** (#5824 F6 review): the save names the key
/// beside the file, never inside the path, so the files the window hands Continue are the ones
/// git stopped in, and Continue carries the merge on once the person has settled the key.
#[test]
fn continue_settles_a_key_conflict_in_charter_toml_that_a_save_named() {
    let two = Two::new(&[("charter.toml", "[sandbox]\nmode = \"on\"\n")]);
    write(&two.mine, "charter.toml", "[sandbox]\nmode = \"off\"\n");
    commit(&two.mine, "mine");
    write(&two.theirs, "charter.toml", "[sandbox]\nmode = \"ask\"\n");
    two.land_theirs("theirs");
    let _the_window = crate::sandbox::held::stand_in(&[]);

    let BroughtIn::Conflict(conflict) = bring_in(&two.mine, "release", false, When::Needed) else {
        panic!("a key both machines changed is a conflict");
    };
    assert_eq!(conflict.files, vec!["charter.toml".to_owned()]);
    assert_eq!(
        conflict.keys,
        BTreeMap::from([("charter.toml".to_owned(), vec!["sandbox.mode".to_owned()])])
    );
    let standing = as_a_save_records_it(&two, conflict);
    assert_eq!(standing.conflicts, vec!["charter.toml".to_owned()]);
    assert_eq!(
        standing.conflict_keys,
        BTreeMap::from([("charter.toml".to_owned(), vec!["sandbox.mode".to_owned()])])
    );

    use super::super::resolvefolder as folder;
    assert_eq!(
        folder::cut(&two.mine, "release"),
        Ok(vec!["charter.toml".to_owned()])
    );
    // What SavingResolve hands Continue: the files the stop named and the standing's, and the
    // person's choice for the key, never markers in the file (#5852).
    let shown = standing.conflicts;
    let ours = folder::Chosen {
        file: "charter.toml".into(),
        key: "sandbox.mode".into(),
        side: folder::Side::Ours,
    };

    assert_eq!(
        super::super::resolve::carry_on(&two.mine, &shown, &[ours], &mut quiet()),
        super::super::resolve::Settled::Replayed
    );
    assert!(read(&two.mine, "charter.toml").contains("mode = \"off\""));
    assert_eq!(
        run(&two.mine, &["log", "-1", "--format=%P"])
            .split(' ')
            .count(),
        2
    );
}

/// **A key conflict is said as one** (#5824 F6 review, docs/ui-copy.md): both machines changed a
/// key, not "the same lines".
#[test]
fn a_key_conflict_is_said_as_both_machines_changing_that_key() {
    let said = said_conflict(
        "release",
        &Conflicted {
            files: vec!["charter.toml".into(), "notes.md".into()],
            keys: BTreeMap::from([(
                "charter.toml".to_owned(),
                vec!["sandbox.mode".to_owned(), "theme.name".to_owned()],
            )]),
        },
    );

    assert_eq!(
        said,
        "release on the remote and this machine both changed `sandbox.mode` and `theme.name` in \
         charter.toml, and the same lines in notes.md. Nothing was changed here: press Resolve \
         in the Saving view to settle them"
    );
    assert_eq!(
        said_conflict(
            "release",
            &Conflicted {
                files: vec!["notes.md".into()],
                keys: BTreeMap::new(),
            },
        ),
        "release on the remote changed the same lines as this machine in notes.md. Nothing was \
         changed here: press Resolve in the Saving view to settle it"
    );
}

/// **A memory a person deleted at Resolve has no line** (#5824 F4 review): this machine forgot
/// a memory the other retitled and edited, so the file is the person's to settle. Once they
/// keep the deletion and press Continue, the index follows the tree they settled.
#[test]
fn continue_rebuilds_the_memory_index_from_the_tree_the_person_settled() {
    let store = "personas/devops/memory";
    let index = format!("{store}/MEMORY.md");
    let two = Two::new(&[
        (&index, "# M\n- [Alpha](alpha.md)\n- [Kept](kept.md)\n"),
        (&format!("{store}/alpha.md"), "# Alpha\n"),
        (&format!("{store}/kept.md"), "# Kept\n"),
    ]);
    std::fs::remove_file(two.mine.join(store).join("alpha.md")).unwrap();
    write(&two.mine, &index, "# M\n- [Kept](kept.md)\n");
    commit(&two.mine, "mine: alpha forgotten");
    write(
        &two.theirs,
        &format!("{store}/alpha.md"),
        "# Alpha\n\nMore.\n",
    );
    write(
        &two.theirs,
        &index,
        "# M\n- [Alpha, more](alpha.md)\n- [Kept](kept.md)\n",
    );
    two.land_theirs("theirs: alpha edited");
    let alpha = format!("{store}/alpha.md");
    assert_eq!(
        bring_in(&two.mine, "release", false, When::Needed),
        BroughtIn::Conflict(Conflicted {
            files: vec![alpha.clone()],
            keys: BTreeMap::new(),
        })
    );

    assert_eq!(
        super::super::resolvefolder::cut(&two.mine, "release"),
        Ok(vec![alpha.clone()])
    );
    run(
        &super::super::resolvefolder::path(&two.mine),
        &["rm", "-q", "--", &alpha],
    );

    assert_eq!(
        super::super::resolve::carry_on(&two.mine, &[alpha], &[], &mut quiet()),
        super::super::resolve::Settled::Replayed
    );
    assert_eq!(read(&two.mine, &index), "# M\n- [Kept](kept.md)\n");
    assert_eq!(run(&two.mine, &["status", "--porcelain"]), "");
}

/// **A store only the other machine changed is rebuilt too** (#5824 F4 review: never merged
/// by lines): its index is written from the merged tree's memories, so a line for a memory no
/// longer there is gone, whatever git's merge of the index would have left.
#[test]
fn a_memory_index_only_the_other_machine_changed_is_rebuilt() {
    let store = "personas/devops/memory";
    let index = format!("{store}/MEMORY.md");
    let two = Two::new(&[
        (&index, "# M\n- [Alpha](alpha.md)\n"),
        (&format!("{store}/alpha.md"), "# Alpha\n"),
    ]);
    write(&two.mine, "notes.md", "mine\n");
    commit(&two.mine, "mine: elsewhere");
    // The other machine lost the file and not its line, as a hand edit leaves it.
    std::fs::remove_file(two.theirs.join(store).join("alpha.md")).unwrap();
    two.land_theirs("theirs: alpha gone");

    assert_eq!(
        bring_in(&two.mine, "release", false, When::Moved),
        BroughtIn::Moved(1)
    );

    assert_eq!(read(&two.mine, &index), "# M\n");
}

/// The memory files a store holds, each with a heading.
fn held(names: &[&str]) -> Vec<(String, String)> {
    names
        .iter()
        .map(|name| ((*name).to_owned(), format!("# {name}\n")))
        .collect()
}

/// **A rebuild indexes only what a side listed** (#5824 F4 review): a memory file no side ever
/// put on its index is the state `optimize` reports, and a rebuild does not hide it by listing
/// it. A line either side took off since the base stays off, this machine's own drop included.
#[test]
fn a_merged_index_lists_only_what_a_side_listed_and_neither_dropped() {
    let base = "# M\n- [A](a.md)\n- [Mine dropped](mine-dropped.md)\n\
                - [Theirs dropped](theirs-dropped.md)\n";
    let ours = "# M\n- [A](a.md)\n- [Theirs dropped](theirs-dropped.md)\n- [New](new.md)\n";
    let theirs = "# M\n- [A](a.md)\n- [Mine dropped](mine-dropped.md)\n";
    let files = held(&[
        "a.md",
        "mine-dropped.md",
        "theirs-dropped.md",
        "new.md",
        "unlisted.md",
    ]);

    let text = crate::memstore::index_merged("# fallback\n", [base, ours, theirs], &files, false);

    assert_eq!(text, "# M\n- [A](a.md)\n- [New](new.md)\n");
}

/// **A rebuild keeps what a person wrote by hand** (#5824 F4 review): a heading between the
/// lines and a link to another folder are not a memory's line, and stay where they were. The
/// other machine's new line goes where it wrote it.
#[test]
fn a_merged_index_keeps_a_heading_between_lines_and_a_link_to_another_folder() {
    let base = "# M\n\n## Projects\n- [A](a.md)\n\n## People\n- [B](b.md)\n\
                - [Shared](../../_shared/memory/x.md)\n";
    let ours = "# M\n\n## Projects\n- [A](a.md)\n- [C](c.md)\n\n## People\n- [B](b.md)\n\
                - [Shared](../../_shared/memory/x.md)\n";
    let theirs = "# M\n\n## Projects\n- [A, retitled](a.md)\n\n## People\n- [B](b.md)\n\
                  - [D](d.md)\n- [Shared](../../_shared/memory/x.md)\n";
    let files = held(&["a.md", "b.md", "c.md", "d.md"]);

    let text = crate::memstore::index_merged("# fallback\n", [base, ours, theirs], &files, false);

    assert_eq!(
        text,
        "# M\n\n## Projects\n- [A, retitled](a.md)\n- [C](c.md)\n\n## People\n- [B](b.md)\n\
         - [D](d.md)\n- [Shared](../../_shared/memory/x.md)\n"
    );
}
