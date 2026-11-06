// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #5411: **Undo a turn puts back exactly what the turn changed**, and a hunk discarded on a turn's
//! diff goes back as the turn's start had it, with git as the oracle, run by the test with no
//! global or system config. Through `files::undo_turn`, `files::turn_changed` and
//! `files::discard`: the turn offered, the refusals, the files put back of every kind, and the repo
//! whose config names a program for every place git runs one. The idle check is the host's, under
//! the run's state lock (`app/src-tauri/src/humanedits.rs`).

use crate::support;

use std::path::{Path, PathBuf};

use purlis_core::files::{
    self, Against, Branch, Discard, Discarded, FileDiff, InTurn, Target, TurnChanged, TurnKey,
};
use purlis_core::humanedit::EditKind;
use purlis_core::worktree;

fn reader() -> files::Reader {
    files::Reader::new(
        std::env::current_exe().expect("the test binary"),
        [
            "a_turn_undo_matches_git::reader_child",
            "--exact",
            "--nocapture",
            "--test-threads=1",
            files::READ_ARG,
        ]
        .map(std::ffi::OsString::from),
    )
}

/// The reader's child: answers the one question it was asked and exits.
#[test]
fn reader_child() {
    purlis_core::unsteered!();
    if let Some(code) = files::serve_if_asked() {
        std::process::exit(code);
    }
}

const CHAT: &str = "01J9CHAT0000000000000000UN";

fn write(at: &Path, path: &str, text: impl AsRef<[u8]>) {
    let file = at.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

fn read(at: &Path, path: &str) -> Option<String> {
    std::fs::read_to_string(at.join(path)).ok()
}

/// A clone whose `main` holds `files`, and a piece cut from it.
fn cut(f: &support::Fixture, files: &[(&str, &str)]) -> PathBuf {
    for (path, text) in files {
        write(&f.clone, path, text);
    }
    support::git(&f.clone, &["add", "-A"]);
    support::git(&f.clone, &["commit", "-q", "-m", "files"]);
    worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path
}

fn piece_of(f: &support::Fixture) -> Branch<'_> {
    Branch::piece(&f.ws, &f.repo, "piece")
}

fn turn(number: u32) -> TurnKey {
    TurnKey {
        chat: CHAT.into(),
        number,
    }
}

/// git, with no config but the repo's, in `dir`; `index` a scratch index of its own.
fn git(dir: &Path, args: &[&str], index: Option<&Path>) -> String {
    let mut cmd = support::unsigned();
    cmd.arg("-C").arg(dir).args(args);
    cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("GIT_DIR");
    if let Some(index) = index {
        cmd.env("GIT_INDEX_FILE", index);
    }
    let out = purlis_core::forklock::output(&mut cmd).expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// What git says the folder's files are now against `of`, renames found: the oracle an undo is
/// read by. The working tree is staged in a scratch index, so the piece's own is not touched.
fn against(piece: &Path, of: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let index = scratch.path().join("index");
    git(piece, &["read-tree", "HEAD"], Some(&index));
    git(piece, &["add", "-A", "."], Some(&index));
    git(
        piece,
        &["diff", "--cached", "-M", "--name-status", of],
        Some(&index),
    )
}

fn start_of(number: u32) -> String {
    format!("refs/purlis/turns/{CHAT}/{number}/start")
}

/// One turn of the chat: `edit` is what its agent does.
fn a_turn(f: &support::Fixture, edit: impl FnOnce()) -> u32 {
    let number = files::turns::began(&f.plane, piece_of(f), CHAT)
        .unwrap()
        .expect("a turn begins");
    edit();
    files::turns::ended(&f.plane, piece_of(f), CHAT).unwrap();
    number
}

/// What the file diff reads of `path` across `number`.
fn across(f: &support::Fixture, path: &str, number: u32) -> TurnChanged {
    files::turn_changed(&reader(), &f.plane, piece_of(f), path, Some(&turn(number))).unwrap()
}

fn undo_offered(f: &support::Fixture, path: &str, number: u32) -> Option<Vec<String>> {
    match across(f, path, number) {
        TurnChanged::Of { undo, .. } => undo,
        TurnChanged::NoTurn => panic!("no turn"),
    }
}

/// The agent's turn of every kind of change: a file edited, one added, one deleted, one renamed,
/// some of it committed, some not.
fn every_kind(piece: &Path) {
    write(piece, "edited.txt", "one\nTWO\nthree\n");
    write(piece, "added/new.txt", "new\n");
    std::fs::remove_file(piece.join("deleted.txt")).unwrap();
    support::git(piece, &["mv", "old.txt", "moved.txt"]);
    support::git(piece, &["commit", "-q", "-am", "the agent's commit"]);
    write(piece, "uncommitted.txt", "the agent's\n");
}

const FILES: &[(&str, &str)] = &[
    ("edited.txt", "one\ntwo\nthree\n"),
    ("deleted.txt", "deleted\n"),
    (
        "old.txt",
        "a file long enough\nto be found\nas a rename\nby git\n",
    ),
    ("untouched.txt", "untouched\n"),
];

#[test]
fn an_undo_puts_back_every_file_the_turn_changed_and_leaves_the_rest_and_the_branch_alone() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, FILES);
    // The operator's own edit before the turn is the turn's start, and so is kept.
    write(&piece, "untouched.txt", "the operator's, before\n");
    let number = a_turn(&f, || every_kind(&piece));
    let head = git(&piece, &["rev-parse", "HEAD"], None);
    let staged = git(&piece, &["diff", "--cached", "--name-status"], None);
    // And after the turn, one of the operator's in a file the turn did not touch.
    write(&piece, "elsewhere.txt", "the operator's, after\n");

    let offered = undo_offered(&f, "edited.txt", number).expect("undo is offered");
    assert_eq!(
        offered,
        [
            "added/new.txt",
            "deleted.txt",
            "edited.txt",
            "moved.txt",
            "old.txt",
            "uncommitted.txt"
        ]
    );
    let undone = files::undo_turn(&reader(), &f.plane, piece_of(&f), &turn(number)).unwrap();

    // The folder is the turn's start again, but for the edit made after it.
    assert_eq!(against(&piece, &start_of(number)), "A\telsewhere.txt\n");
    assert_eq!(
        read(&piece, "untouched.txt").as_deref(),
        Some("the operator's, before\n")
    );
    assert!(!piece.join("added").exists() || !piece.join("added/new.txt").exists());
    // Nothing committed, no branch moved, and the folder's index as the turn left it.
    assert_eq!(git(&piece, &["rev-parse", "HEAD"], None), head);
    assert_eq!(
        git(&piece, &["diff", "--cached", "--name-status"], None),
        staged
    );
    // One edit, of every file it wrote, and their lines, not their contents.
    assert_eq!(undone.unmarked, None);
    assert_eq!(undone.edit.kind, EditKind::UndoneTurn);
    let paths: Vec<&str> = undone.edit.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, offered);
    let edited = &undone.edit.files[2];
    assert_eq!(edited.lines.len(), 1);
    assert_eq!((edited.lines[0].start, edited.lines[0].count), (2, 1));
}

#[test]
fn undo_is_offered_on_the_latest_turn_not_yet_undone_and_then_on_the_one_before() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, FILES);
    let first = a_turn(&f, || write(&piece, "edited.txt", "first\n"));
    // Turns a second apart, as a model's round trip keeps them.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let second = a_turn(&f, || write(&piece, "edited.txt", "second\n"));

    assert_eq!(undo_offered(&f, "edited.txt", first), None);
    assert!(undo_offered(&f, "edited.txt", second).is_some());
    let refused = files::undo_turn(&reader(), &f.plane, piece_of(&f), &turn(first)).unwrap_err();
    assert!(refused.to_string().contains("that is turn 2"), "{refused}");
    assert_eq!(read(&piece, "edited.txt").as_deref(), Some("second\n"));

    files::undo_turn(&reader(), &f.plane, piece_of(&f), &turn(second)).unwrap();
    assert_eq!(read(&piece, "edited.txt").as_deref(), Some("first\n"));
    assert_eq!(undo_offered(&f, "edited.txt", second), None);
    assert!(undo_offered(&f, "edited.txt", first).is_some());
    let again = files::undo_turn(&reader(), &f.plane, piece_of(&f), &turn(second)).unwrap_err();
    assert!(again.to_string().contains("that is turn 1"), "{again}");

    files::undo_turn(&reader(), &f.plane, piece_of(&f), &turn(first)).unwrap();
    assert_eq!(
        read(&piece, "edited.txt").as_deref(),
        Some("one\ntwo\nthree\n")
    );
    assert_eq!(undo_offered(&f, "edited.txt", first), None);
}

#[test]
fn a_file_changed_since_the_turn_ended_refuses_the_undo_by_its_name_and_nothing_is_written() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, FILES);
    let number = a_turn(&f, || {
        write(&piece, "edited.txt", "the agent's\n");
        write(&piece, "untouched.txt", "the agent's too\n");
    });
    write(&piece, "untouched.txt", "the operator's, after\n");

    let refused = files::undo_turn(&reader(), &f.plane, piece_of(&f), &turn(number)).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "The turn was not undone: 'untouched.txt' changed since the turn ended. Nothing was \
         changed."
    );
    assert_eq!(read(&piece, "edited.txt").as_deref(), Some("the agent's\n"));
    assert_eq!(
        read(&piece, "untouched.txt").as_deref(),
        Some("the operator's, after\n")
    );
    // Still the turn offered: the operator can put the file back and undo.
    assert!(undo_offered(&f, "edited.txt", number).is_some());

    // A file the turn deleted that is back, and one it added that is gone, are changes too.
    write(&piece, "untouched.txt", "the agent's too\n");
    let deleting = a_turn(&f, || {
        std::fs::remove_file(piece.join("deleted.txt")).unwrap()
    });
    write(&piece, "deleted.txt", "the operator's\n");
    let refused = files::undo_turn(&reader(), &f.plane, piece_of(&f), &turn(deleting)).unwrap_err();
    assert!(refused.to_string().contains("'deleted.txt'"), "{refused}");
}

/// The hunks of `path` drawn across `number`, and what they were drawn from.
fn drawn(f: &support::Fixture, path: &str, number: u32) -> (String, Vec<files::Hunk>, bool) {
    let TurnChanged::Of {
        file: InTurn::Changed(shown),
        ..
    } = across(f, path, number)
    else {
        panic!("{path} is the turn's");
    };
    let FileDiff::Text { hunks, .. } = shown.diff else {
        panic!("{path} is text");
    };
    (shown.stamp.expect("a stamp"), hunks, shown.since_turn_ended)
}

fn discard_in_turn(f: &support::Fixture, path: &str, number: u32, nth: usize) -> Discarded {
    let (stamp, hunks, _) = drawn(f, path, number);
    files::discard(
        &reader(),
        &f.plane,
        piece_of(f),
        &Discard {
            path: path.into(),
            target: Target::Hunk(hunks[nth]),
            against: Against::Opened(stamp),
            turn: Some(turn(number)),
        },
    )
    .unwrap()
}

const TEN: &str = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n";

#[test]
fn a_hunk_discarded_on_a_turns_diff_goes_back_as_the_turn_started_and_the_rest_stay_discardable() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("ten.txt", TEN)]);
    // Before the turn, the operator's: the base does not have it, the turn's start does.
    write(&piece, "ten.txt", "A\nb\nc\nd\ne\nf\ng\nh\ni\nj\n");
    let number = a_turn(&f, || {
        write(&piece, "ten.txt", "A\nb\nC\nd\ne\nf\ng\nH\ni\nj\n");
    });
    let (_, hunks, since) = drawn(&f, "ten.txt", number);
    assert_eq!(hunks.len(), 2);
    assert!(
        !since,
        "as the turn ended it, it is drawn from the turn's refs"
    );

    let Discarded::Done { edit, now } = discard_in_turn(&f, "ten.txt", number, 0) else {
        panic!("it is discarded");
    };
    assert_eq!(edit.kind, EditKind::DiscardedHunk);
    assert_eq!(
        read(&piece, "ten.txt").as_deref(),
        Some("A\nb\nc\nd\ne\nf\ng\nH\ni\nj\n")
    );
    // What the window draws next: the one hunk left, from the turn's start to the file now.
    let now = now.expect("a hunk is left");
    assert!(now.since_turn_ended);
    let FileDiff::Text { hunks, .. } = &now.diff else {
        panic!("text");
    };
    assert_eq!(hunks.len(), 1);
    assert_eq!(drawn(&f, "ten.txt", number).1, *hunks);

    // And that one can be discarded too, as the turn's start had it.
    discard_in_turn(&f, "ten.txt", number, 0);
    assert_eq!(against(&piece, &start_of(number)), "");
    // The file changed since the turn ended, so the turn cannot be undone over it.
    let refused = files::undo_turn(&reader(), &f.plane, piece_of(&f), &turn(number)).unwrap_err();
    assert!(refused.to_string().contains("'ten.txt'"), "{refused}");
}

#[test]
fn a_file_the_turn_added_goes_and_one_it_deleted_comes_back_when_their_hunks_are_discarded() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, FILES);
    let number = a_turn(&f, || {
        write(&piece, "added.txt", "new\n");
        std::fs::remove_file(piece.join("deleted.txt")).unwrap();
    });
    discard_in_turn(&f, "added.txt", number, 0);
    discard_in_turn(&f, "deleted.txt", number, 0);
    assert_eq!(against(&piece, &start_of(number)), "");
}

#[test]
fn no_program_the_repos_config_names_runs_for_an_undo_or_a_discard_on_a_turn() {
    purlis_core::unsteered!();
    use std::os::unix::fs::PermissionsExt as _;
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("ten.txt", TEN), ("other.txt", "other\n")]);
    let ran = tempfile::tempdir().unwrap();
    for (key, place) in [
        ("diff.external", "external"),
        ("diff.x.command", "driver"),
        ("diff.x.textconv", "textconv"),
        ("filter.x.clean", "clean"),
        ("filter.x.smudge", "smudge"),
        ("filter.x.process", "process"),
        ("core.fsmonitor", "fsmonitor"),
        ("core.pager", "pager"),
        ("core.hooksPath", "hooks"),
        ("core.askPass", "askpass"),
        ("core.sshCommand", "ssh"),
        ("gpg.program", "gpg"),
        ("core.editor", "editor"),
    ] {
        let script = ran.path().join(format!("{place}.sh"));
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\ntouch '{}'\ncat\n",
                ran.path().join(format!("ran-{place}")).display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        support::git(&f.clone, &["config", key, &script.display().to_string()]);
    }
    write(&piece, ".gitattributes", "* diff=x\n*.txt filter=x\n");
    let number = a_turn(&f, || {
        write(&piece, "ten.txt", "a\nB\nc\nd\ne\nf\ng\nh\ni\nJ\n");
        write(&piece, "other.txt", "the agent's\n");
    });

    // A discard on the turn's diff, and then the undo of the turn, in the repo that names them
    // all: the file the turn changed is put back first, as the undo needs every one as it ended.
    discard_in_turn(&f, "ten.txt", number, 0);
    write(&piece, "ten.txt", "a\nB\nc\nd\ne\nf\ng\nh\ni\nJ\n");
    files::undo_turn(&reader(), &f.plane, piece_of(&f), &turn(number)).unwrap();
    assert_eq!(read(&piece, "ten.txt").as_deref(), Some(TEN));
    assert_eq!(read(&piece, "other.txt").as_deref(), Some("other\n"));

    let ran_any: Vec<String> = std::fs::read_dir(ran.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("ran-"))
        .collect();
    assert!(ran_any.is_empty(), "programs ran: {ran_any:?}");

    // And the fixture is hostile for real: git itself, asked for a plain diff there, runs one.
    write(&piece, "ten.txt", "changed for git to diff\n");
    let mut cmd = support::unsigned();
    cmd.arg("-C").arg(&piece).args(["diff", "HEAD"]);
    cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_PAGER", "cat");
    let _ = purlis_core::forklock::output(&mut cmd);
    assert!(
        ran.path().join("ran-driver").exists() || ran.path().join("ran-external").exists(),
        "git ran none of the fixture's programs: the fixture proves nothing"
    );
}

#[test]
fn after_an_undo_the_file_diff_opens_on_the_turn_now_offered() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, FILES);
    let first = a_turn(&f, || write(&piece, "edited.txt", "first\n"));
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let second = a_turn(&f, || write(&piece, "edited.txt", "second\n"));
    files::undo_turn(&reader(), &f.plane, piece_of(&f), &turn(second)).unwrap();

    let opened =
        files::turn_changed(&reader(), &f.plane, piece_of(&f), "edited.txt", None).unwrap();
    let TurnChanged::Of {
        number,
        turns,
        undo,
        ..
    } = opened
    else {
        panic!("a turn");
    };
    assert_eq!(number, first);
    assert!(undo.is_some(), "Undo is offered on the turn it opens on");
    // The undone turn is still listed, to be reviewed.
    assert_eq!(turns, [second, first]);
}

fn executable(at: &Path, path: &str) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(at.join(path))
        .unwrap()
        .permissions()
        .mode()
        & 0o111
        != 0
}

fn set_executable(at: &Path, path: &str, on: bool) {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = if on { 0o755 } else { 0o644 };
    std::fs::set_permissions(at.join(path), std::fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn an_undo_puts_back_the_executable_bit_the_turn_changed() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, FILES);
    let number = a_turn(&f, || {
        set_executable(&piece, "untouched.txt", true);
        write(&piece, "edited.txt", "the agent's\n");
        set_executable(&piece, "edited.txt", true);
    });
    assert!(
        undo_offered(&f, "edited.txt", number)
            .expect("undo is offered")
            .contains(&"untouched.txt".to_string())
    );

    files::undo_turn(&reader(), &f.plane, piece_of(&f), &turn(number)).unwrap();

    assert!(!executable(&piece, "untouched.txt"));
    assert!(!executable(&piece, "edited.txt"));
    assert_eq!(against(&piece, &start_of(number)), "");
}

#[test]
fn a_files_mode_changed_since_the_turn_ended_refuses_the_undo() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, FILES);
    let number = a_turn(&f, || write(&piece, "edited.txt", "the agent's\n"));
    set_executable(&piece, "edited.txt", true);

    let refused = files::undo_turn(&reader(), &f.plane, piece_of(&f), &turn(number)).unwrap_err();
    assert!(
        refused
            .to_string()
            .contains("'edited.txt' changed since the turn ended"),
        "{refused}"
    );
    assert_eq!(read(&piece, "edited.txt").as_deref(), Some("the agent's\n"));
}
