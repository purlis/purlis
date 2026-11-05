// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #5406: **discarding a hunk puts back exactly that hunk**, with `git diff` as the oracle, run
//! by the test with no global or system config, as in `a_comparison_matches_git_diff.rs`. The
//! write goes through `files::discard`, whose every path the window can reach is here: the
//! stale check, the guards, the one Human edit, and the repo whose config names a program for
//! every place git runs one.

use crate::support;

use std::path::{Path, PathBuf};

use purlis_core::files::{self, Against, Branch, Discard, Discarded, FileDiff, Hunk, Target};
use purlis_core::humanedit::{EditKind, EditedFile, EditedLines};
use purlis_core::worktree;

fn reader() -> files::Reader {
    files::Reader::new(
        std::env::current_exe().expect("the test binary"),
        [
            "a_discard_matches_git::reader_child",
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

fn write(at: &Path, path: &str, text: impl AsRef<[u8]>) {
    let file = at.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

/// A clone whose `main` holds `files`, and a piece cut from it.
fn cut(f: &support::Fixture, files: &[(&str, &[u8])]) -> PathBuf {
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

/// What `git diff` says the working tree changed against `main`, renames found: for each file its
/// status, then its `-U0` hunk lines. The index is a scratch copy holding the working tree, so
/// neither the piece's index nor an untracked file is in the way.
fn git_says(piece: &Path) -> Vec<String> {
    let scratch = tempfile::tempdir().unwrap();
    let index = scratch.path().join("index");
    let git = |args: &[&str]| {
        let mut cmd = support::unsigned();
        cmd.arg("-C").arg(piece).args(args);
        cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_INDEX_FILE", &index)
            .env_remove("GIT_DIR");
        let out = purlis_core::forklock::output(&mut cmd).expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    git(&["add", "-A"]);
    let patch = git(&[
        "diff",
        "--cached",
        "-M",
        "-U0",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "main",
    ]);
    patch
        .lines()
        .filter(|line| {
            !line.starts_with("index ")
                && !line.starts_with("--- ")
                && !line.starts_with("+++ ")
                && !line.starts_with("similarity ")
        })
        .map(|line| match line.split_once(" @@") {
            Some((numbers, _)) if numbers.starts_with("@@") => format!("{numbers} @@"),
            _ => line.to_string(),
        })
        .collect()
}

/// What the file's diff draws: the stamp the window holds, and the hunks.
fn opened(f: &support::Fixture, path: &str) -> (String, Vec<Hunk>) {
    let shown = files::what_changed(&reader(), &f.plane, piece_of(f), path).unwrap();
    let hunks = match shown.diff {
        FileDiff::Text { hunks, .. } => hunks,
        _ => Vec::new(),
    };
    (shown.stamp.expect("a stamp"), hunks)
}

/// Discards the `nth` hunk of `path`, as the window would having just drawn it.
fn discard_nth(f: &support::Fixture, path: &str, nth: usize) -> Discarded {
    let (stamp, hunks) = opened(f, path);
    files::discard(
        &reader(),
        &f.plane,
        piece_of(f),
        &Discard {
            path: path.into(),
            target: Target::Hunk(hunks[nth]),
            against: Against::Opened(stamp),
            turn: None,
        },
    )
    .unwrap()
}

fn discard_whole(f: &support::Fixture, path: &str) -> Discarded {
    let (stamp, _) = opened(f, path);
    files::discard(
        &reader(),
        &f.plane,
        piece_of(f),
        &Discard {
            path: path.into(),
            target: Target::Whole,
            against: Against::Opened(stamp),
            turn: None,
        },
    )
    .unwrap()
}

const TEN: &[u8] = b"a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n";

#[test]
fn a_discard_puts_back_one_hunk_and_leaves_the_others() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("ten.txt", TEN)]);
    write(&piece, "ten.txt", "a\nB\nc\nd\ne\nf\ng\nh\nI\nj\n");
    assert_eq!(
        git_says(&piece)[1..],
        ["@@ -2 +2 @@", "-b", "+B", "@@ -9 +9 @@", "-i", "+I"]
    );

    assert!(matches!(
        discard_nth(&f, "ten.txt", 0),
        Discarded::Done { .. }
    ));

    assert_eq!(git_says(&piece)[1..], ["@@ -9 +9 @@", "-i", "+I"]);
    assert_eq!(
        std::fs::read(piece.join("ten.txt")).unwrap(),
        b"a\nb\nc\nd\ne\nf\ng\nh\nI\nj\n"
    );
}

#[test]
fn a_hunk_that_added_lines_and_a_hunk_that_removed_lines_each_go_back_with_the_numbers_shifted() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("ten.txt", TEN)]);
    write(&piece, "ten.txt", "new\nmore\na\nb\nc\nd\nf\ng\nh\ni\nj\n");
    assert_eq!(
        git_says(&piece)[1..],
        ["@@ -0,0 +1,2 @@", "+new", "+more", "@@ -5 +6,0 @@", "-e"]
    );

    discard_nth(&f, "ten.txt", 1);
    assert_eq!(git_says(&piece)[1..], ["@@ -0,0 +1,2 @@", "+new", "+more"]);

    discard_nth(&f, "ten.txt", 0);
    assert_eq!(git_says(&piece), Vec::<String>::new());
    assert_eq!(std::fs::read(piece.join("ten.txt")).unwrap(), TEN);
}

#[test]
fn a_last_line_that_gained_its_line_end_goes_back_without_it() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("no-eol.txt", b"a\nb")]);
    write(&piece, "no-eol.txt", "a\nb\n");

    discard_nth(&f, "no-eol.txt", 0);

    assert_eq!(git_says(&piece), Vec::<String>::new());
    assert_eq!(std::fs::read(piece.join("no-eol.txt")).unwrap(), b"a\nb");
}

#[test]
fn bytes_that_are_not_text_elsewhere_in_the_file_are_kept_as_they_are() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let latin = b"caf\xe9\nb\nc\nd\ne\nf\ng\nh\n";
    let piece = cut(&f, &[("latin.txt", latin)]);
    write(&piece, "latin.txt", b"caf\xe9\nb\nc\nd\ne\nf\ng\nH\n");

    discard_nth(&f, "latin.txt", 0);

    assert_eq!(std::fs::read(piece.join("latin.txt")).unwrap(), latin);
}

#[test]
fn an_added_file_goes_when_its_hunk_is_discarded() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("keep.txt", b"kept\n")]);
    write(&piece, "src/new.txt", "new\nfile\n");
    assert_eq!(
        git_says(&piece)[0],
        "diff --git a/src/new.txt b/src/new.txt"
    );

    discard_nth(&f, "src/new.txt", 0);

    assert_eq!(git_says(&piece), Vec::<String>::new());
    assert!(!piece.join("src/new.txt").exists());
}

#[test]
fn a_deleted_file_comes_back_whole_with_its_folder_and_its_mode() {
    purlis_core::unsteered!();
    use std::os::unix::fs::PermissionsExt as _;
    let f = support::plane_with_clone("thing");
    write(&f.clone, "bin/run.sh", "#!/bin/sh\necho hi\n");
    let file = f.clone.join("bin/run.sh");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    let piece = cut(&f, &[("keep.txt", b"kept\n")]);
    std::fs::remove_dir_all(piece.join("bin")).unwrap();

    discard_nth(&f, "bin/run.sh", 0);

    assert_eq!(git_says(&piece), Vec::<String>::new());
    assert_eq!(
        std::fs::read(piece.join("bin/run.sh")).unwrap(),
        b"#!/bin/sh\necho hi\n"
    );
    let mode = std::fs::metadata(piece.join("bin/run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o100, 0o100, "it came back not executable: {mode:o}");
}

#[test]
fn a_deleted_file_in_a_folder_that_stays_comes_back() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(
        &f,
        &[("src/gone.txt", b"gone\n"), ("src/kept.txt", b"kept\n")],
    );
    std::fs::remove_file(piece.join("src/gone.txt")).unwrap();

    discard_nth(&f, "src/gone.txt", 0);

    assert_eq!(git_says(&piece), Vec::<String>::new());
    assert_eq!(
        std::fs::read(piece.join("src/gone.txt")).unwrap(),
        b"gone\n"
    );
}

#[test]
fn a_path_that_is_gone_and_no_change_lists_is_said_to_be_gone() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    cut(&f, &[("keep.txt", b"kept\n")]);

    let refused =
        files::what_changed(&reader(), &f.plane, piece_of(&f), "never-was.txt").unwrap_err();

    assert!(
        refused
            .to_string()
            .contains("not in the branch's folder any more"),
        "{refused}"
    );
}

#[test]
fn a_renamed_files_hunk_goes_back_to_its_old_lines_and_it_keeps_its_new_name() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let long: String = (0..40).map(|n| format!("line {n} with words\n")).collect();
    let piece = cut(&f, &[("old.txt", long.as_bytes())]);
    std::fs::remove_file(piece.join("old.txt")).unwrap();
    write(
        &piece,
        "moved/new.txt",
        long.replace("line 3 with words", "LINE 3")
            .replace("line 30 with words", "LINE 30"),
    );
    assert!(git_says(&piece).contains(&"rename to moved/new.txt".to_string()));

    discard_nth(&f, "moved/new.txt", 0);

    let said = git_says(&piece);
    assert!(
        said.contains(&"rename from old.txt".to_string()),
        "{said:?}"
    );
    assert!(
        said.contains(&"rename to moved/new.txt".to_string()),
        "{said:?}"
    );
    assert_eq!(
        said[3..],
        ["@@ -31 +31 @@", "-line 30 with words", "+LINE 30"]
    );
    assert!(!piece.join("old.txt").exists());
}

#[test]
fn a_binary_files_change_is_discarded_whole() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("logo.bin", b"\x89PNG\x00one")]);
    write(&piece, "logo.bin", b"\x89PNG\x00two");
    write(&piece, "added.bin", b"\x00added");

    let Discarded::Done { edit, .. } = discard_whole(&f, "logo.bin") else {
        panic!("not discarded");
    };
    assert_eq!(edit.files[0].lines, Vec::<EditedLines>::new());
    assert_eq!(
        std::fs::read(piece.join("logo.bin")).unwrap(),
        b"\x89PNG\x00one"
    );
    discard_whole(&f, "added.bin");

    assert_eq!(git_says(&piece), Vec::<String>::new());
}

#[test]
fn a_discard_makes_one_human_edit_with_its_file_and_lines_and_no_contents() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("ten.txt", TEN)]);
    write(&piece, "ten.txt", "a\nb\nc\nd\ne\nf\ng\nh\nSECRET\nj\n");

    let Discarded::Done { edit, now } = discard_nth(&f, "ten.txt", 0) else {
        panic!("not discarded");
    };

    assert_eq!(edit.kind, EditKind::DiscardedHunk);
    assert_eq!(edit.repo, f.repo);
    assert_eq!(edit.piece.as_deref(), Some("piece"));
    assert_eq!(
        edit.files,
        [EditedFile {
            path: "ten.txt".into(),
            lines: vec![EditedLines { start: 9, count: 1 }],
        }]
    );
    assert!(!edit.body().to_string().contains("SECRET"));
    // Nothing is left of the file's change, so the diff has nothing to draw.
    assert_eq!(now, None);
}

#[test]
fn what_a_discard_answers_is_the_diff_without_the_hunk() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("ten.txt", TEN)]);
    write(&piece, "ten.txt", "a\nB\nc\nd\ne\nf\ng\nh\nI\nj\n");

    let Discarded::Done { now, .. } = discard_nth(&f, "ten.txt", 0) else {
        panic!("not discarded");
    };

    let FileDiff::Text { hunks, .. } = now.expect("a change is left").diff else {
        panic!("not text");
    };
    assert_eq!(
        hunks,
        [Hunk {
            old_start: 9,
            old_lines: 1,
            new_start: 9,
            new_lines: 1
        }]
    );
}

#[test]
fn a_file_changed_on_disk_since_it_was_opened_is_stale_and_is_left_alone() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("ten.txt", TEN)]);
    write(&piece, "ten.txt", "a\nB\nc\nd\ne\nf\ng\nh\ni\nj\n");
    let (stamp, hunks) = opened(&f, "ten.txt");
    let newer = "a\nB\nc\nd\ne\nf\ng\nh\ni\nJ, written since\n";
    write(&piece, "ten.txt", newer);

    let answer = files::discard(
        &reader(),
        &f.plane,
        piece_of(&f),
        &Discard {
            path: "ten.txt".into(),
            target: Target::Hunk(hunks[0]),
            against: Against::Opened(stamp),
            turn: None,
        },
    )
    .unwrap();

    assert_eq!(answer, Discarded::Stale);
    assert_eq!(
        std::fs::read_to_string(piece.join("ten.txt")).unwrap(),
        newer
    );
}

#[test]
fn overwrite_writes_what_was_opened_less_the_hunk_over_the_newer_file_and_merges_nothing() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("ten.txt", TEN)]);
    let drawn = "a\nB\nc\nd\ne\nf\ng\nh\nI\nj\n";
    write(&piece, "ten.txt", drawn);
    let (_, hunks) = opened(&f, "ten.txt");
    write(
        &piece,
        "ten.txt",
        "a\nB\nc\nd\ne\nf\ng\nh\nI\nJ, written since\n",
    );

    files::discard(
        &reader(),
        &f.plane,
        piece_of(&f),
        &Discard {
            path: "ten.txt".into(),
            target: Target::Hunk(hunks[0]),
            against: Against::Overwrite {
                head: Some(drawn.into()),
            },
            turn: None,
        },
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(piece.join("ten.txt")).unwrap(),
        "a\nb\nc\nd\ne\nf\ng\nh\nI\nj\n"
    );
}

#[test]
fn overwrite_of_a_binary_file_restores_it_whatever_it_became() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("logo.bin", b"\x00one")]);
    write(&piece, "logo.bin", b"\x00two");
    write(&piece, "logo.bin", b"\x00three");

    files::discard(
        &reader(),
        &f.plane,
        piece_of(&f),
        &Discard {
            path: "logo.bin".into(),
            target: Target::Whole,
            against: Against::Overwrite { head: None },
            turn: None,
        },
    )
    .unwrap();

    assert_eq!(git_says(&piece), Vec::<String>::new());
}

#[test]
fn a_hunk_the_file_does_not_have_is_stale_and_writes_nothing() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("ten.txt", TEN)]);
    let drawn = "a\nB\nc\nd\ne\nf\ng\nh\ni\nj\n";
    write(&piece, "ten.txt", drawn);
    let (stamp, _) = opened(&f, "ten.txt");

    let answer = files::discard(
        &reader(),
        &f.plane,
        piece_of(&f),
        &Discard {
            path: "ten.txt".into(),
            target: Target::Hunk(Hunk {
                old_start: 1,
                old_lines: 3,
                new_start: 1,
                new_lines: 3,
            }),
            against: Against::Opened(stamp),
            turn: None,
        },
    )
    .unwrap();

    assert_eq!(answer, Discarded::Stale);
    assert_eq!(
        std::fs::read_to_string(piece.join("ten.txt")).unwrap(),
        drawn
    );
}

#[test]
fn line_ends_the_working_tree_converts_stay_as_the_file_has_them() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    write(&f.clone, ".gitattributes", "*.txt text eol=crlf\n");
    let piece = cut(&f, &[("crlf.txt", b"a\nb\nc\nd\ne\nf\ng\nh\n")]);
    write(
        &piece,
        "crlf.txt",
        "A\r\nb\r\nc\r\nd\r\ne\r\nf\r\ng\r\nH\r\n",
    );

    discard_nth(&f, "crlf.txt", 0);

    assert_eq!(
        std::fs::read(piece.join("crlf.txt")).unwrap(),
        b"a\r\nb\r\nc\r\nd\r\ne\r\nf\r\ng\r\nH\r\n"
    );
    assert_eq!(git_says(&piece)[1..], ["@@ -8 +8 @@", "-h", "+H"]);
}

#[test]
fn a_file_turned_into_a_link_is_refused_and_stays_a_link() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("file.txt", b"text\n"), ("other.txt", b"other\n")]);
    std::fs::remove_file(piece.join("file.txt")).unwrap();
    std::os::unix::fs::symlink("other.txt", piece.join("file.txt")).unwrap();

    let refused = files::discard(
        &reader(),
        &f.plane,
        piece_of(&f),
        &Discard {
            path: "file.txt".into(),
            target: Target::Whole,
            against: Against::Overwrite { head: None },
            turn: None,
        },
    )
    .unwrap_err();

    assert!(refused.to_string().contains("file.txt"), "{refused}");
    assert!(piece.join("file.txt").is_symlink());
}

#[test]
fn a_path_out_of_the_branch_or_into_git_is_refused_and_nothing_is_written() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    cut(&f, &[("ten.txt", TEN)]);
    for path in ["../outside.txt", "/etc/hosts", ".git/config", ""] {
        let refused = files::discard(
            &reader(),
            &f.plane,
            piece_of(&f),
            &Discard {
                path: path.into(),
                target: Target::Whole,
                against: Against::Overwrite { head: None },
                turn: None,
            },
        );
        assert!(refused.is_err(), "{path} was taken");
    }
}

#[test]
fn a_folder_swapped_for_a_link_is_never_written_through() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("src/lib.txt", b"base\n")]);
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "lib.txt", "OUTSIDE\n");
    std::fs::remove_dir_all(piece.join("src")).unwrap();
    std::os::unix::fs::symlink(outside.path(), piece.join("src")).unwrap();

    let refused = files::discard(
        &reader(),
        &f.plane,
        piece_of(&f),
        &Discard {
            path: "src/lib.txt".into(),
            target: Target::Hunk(Hunk {
                old_start: 1,
                old_lines: 1,
                new_start: 0,
                new_lines: 0,
            }),
            against: Against::Overwrite {
                head: Some(String::new()),
            },
            turn: None,
        },
    );

    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(
        std::fs::read_to_string(outside.path().join("lib.txt")).unwrap(),
        "OUTSIDE\n"
    );
}

#[test]
fn no_program_the_repos_config_names_runs_for_a_discard() {
    purlis_core::unsteered!();
    use std::os::unix::fs::PermissionsExt as _;
    let f = support::plane_with_clone("thing");
    let piece = cut(&f, &[("ten.txt", TEN), ("filtered.txt", b"base\n")]);
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
    write(
        &piece,
        ".gitattributes",
        "* diff=x\nfiltered.txt filter=x\n",
    );
    write(&piece, "ten.txt", "a\nB\nc\nd\ne\nf\ng\nh\ni\nj\n");
    write(&piece, "filtered.txt", "changed\n");

    // The discard itself, end to end, in the repo that names them all.
    assert!(matches!(
        discard_nth(&f, "ten.txt", 0),
        Discarded::Done { .. }
    ));
    assert_eq!(std::fs::read(piece.join("ten.txt")).unwrap(), TEN);
    // A file the repo filters: its blob is not what its file holds, and purlis runs no filter to
    // turn one into the other, so it says so rather than write the blob's bytes.
    let (stamp, hunks) = opened(&f, "filtered.txt");
    let refused = files::discard(
        &reader(),
        &f.plane,
        piece_of(&f),
        &Discard {
            path: "filtered.txt".into(),
            target: Target::Hunk(hunks[0]),
            against: Against::Opened(stamp),
            turn: None,
        },
    )
    .unwrap_err();
    assert!(refused.to_string().contains("filter"), "{refused}");
    assert_eq!(
        std::fs::read_to_string(piece.join("filtered.txt")).unwrap(),
        "changed\n"
    );

    let ran_any: Vec<String> = std::fs::read_dir(ran.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("ran-"))
        .collect();
    assert!(ran_any.is_empty(), "programs ran: {ran_any:?}");

    // And the fixture is hostile for real: git itself, asked for a plain diff there, runs one.
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
