// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::time::{Duration, SystemTime};

fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    (dir, root)
}

const CHAT: &str = "01K6H1BERNATED00000000000B";
const OTHER: &str = "01K6H1BERNATED00000000000C";

fn file_of(root: &Path, id: &str) -> PathBuf {
    dir(root).join(format!("{id}.screen"))
}

#[test]
fn a_kept_screen_reads_back_as_it_was_kept() {
    let (_dir, root) = project();
    keep(&root, CHAT, "\x1b[1mDone.\x1b[0m\r\n").unwrap();
    assert_eq!(
        kept(&root, CHAT).as_deref(),
        Some("\x1b[1mDone.\x1b[0m\r\n")
    );
}

#[test]
fn nothing_kept_reads_as_none() {
    let (_dir, root) = project();
    assert_eq!(kept(&root, CHAT), None);
}

#[test]
fn a_name_that_is_not_a_ulid_is_refused() {
    let (_dir, root) = project();
    assert!(keep(&root, "../escape", "x").is_err());
    assert_eq!(kept(&root, "../escape"), None);
}

#[test]
fn a_screen_of_one_mebibyte_is_kept_and_one_byte_more_is_not() {
    let (_dir, root) = project();
    assert_eq!(
        MOST_BYTES,
        1024 * 1024,
        "spec D3, in binary units (ADR 0082)"
    );
    keep(&root, CHAT, &"x".repeat(MOST_BYTES)).unwrap();
    assert!(kept(&root, CHAT).is_some());
    assert!(keep(&root, OTHER, &"x".repeat(MOST_BYTES + 1)).is_err());
    assert!(
        !file_of(&root, OTHER).exists(),
        "a refused screen leaves no file"
    );
}

#[test]
fn a_refused_screen_takes_the_one_kept_before_with_it() {
    // A chat hibernated again keeps what it showed now or nothing, never an older screen.
    let (_dir, root) = project();
    keep(&root, CHAT, "older").unwrap();
    assert!(keep(&root, CHAT, &"x".repeat(MOST_BYTES + 1)).is_err());
    assert_eq!(kept(&root, CHAT), None);
    assert!(!file_of(&root, CHAT).exists());
}

#[test]
fn a_write_that_fails_leaves_no_file() {
    let (_dir, root) = project();
    keep(&root, OTHER, "x").unwrap();
    // Something that is no file stands where the screen goes: the rename onto it fails, and
    // neither the screen nor its temp file is left.
    std::fs::create_dir_all(file_of(&root, CHAT).join("in the way")).unwrap();
    assert!(keep(&root, CHAT, "never").is_err());
    assert_eq!(kept(&root, CHAT), None);
    let files: Vec<_> = std::fs::read_dir(dir(&root))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.metadata().unwrap().is_file())
        .map(|entry| entry.file_name())
        .collect();
    assert_eq!(files, [std::ffi::OsString::from(format!("{OTHER}.screen"))]);
}

#[test]
fn a_file_past_the_bound_reads_as_none() {
    let (_dir, root) = project();
    keep(&root, CHAT, "x").unwrap();
    std::fs::write(file_of(&root, CHAT), "x".repeat(MOST_BYTES + 1)).unwrap();
    assert_eq!(kept(&root, CHAT), None);
}

#[test]
fn a_file_that_is_not_text_reads_as_none() {
    let (_dir, root) = project();
    keep(&root, CHAT, "x").unwrap();
    std::fs::write(file_of(&root, CHAT), [0xff, 0xfe, 0x00, 0xc3]).unwrap();
    assert_eq!(kept(&root, CHAT), None);
}

#[cfg(unix)]
#[test]
fn a_kept_screen_is_private() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_dir, root) = project();
    keep(&root, CHAT, "x").unwrap();
    let file = std::fs::metadata(file_of(&root, CHAT)).unwrap();
    assert_eq!(file.permissions().mode() & 0o777, 0o600);
    let folder = std::fs::metadata(dir(&root)).unwrap();
    assert_eq!(folder.permissions().mode() & 0o777, 0o700);
}

#[cfg(unix)]
#[test]
fn a_link_in_the_store_is_never_read() {
    let (_dir, root) = project();
    keep(&root, OTHER, "x").unwrap();
    let elsewhere = root.join("secret.txt");
    std::fs::write(&elsewhere, "not a screen").unwrap();
    std::os::unix::fs::symlink(&elsewhere, file_of(&root, CHAT)).unwrap();
    assert_eq!(kept(&root, CHAT), None);
}

#[test]
fn forgetting_a_chat_takes_its_screen_alone() {
    let (_dir, root) = project();
    keep(&root, CHAT, "a").unwrap();
    keep(&root, OTHER, "b").unwrap();
    forget(&root, CHAT);
    assert_eq!(kept(&root, CHAT), None);
    assert_eq!(kept(&root, OTHER).as_deref(), Some("b"));
}

#[test]
fn the_sweep_takes_a_screen_no_hibernated_chat_names() {
    let (_dir, root) = project();
    keep(&root, CHAT, "kept").unwrap();
    keep(&root, OTHER, "orphan").unwrap();
    assert_eq!(sweep(&root, |id| id == CHAT, SystemTime::now()), 1);
    assert_eq!(kept(&root, CHAT).as_deref(), Some("kept"));
    assert_eq!(kept(&root, OTHER), None);
}

#[test]
fn the_sweep_takes_a_screen_thirty_days_old_even_of_a_hibernated_chat() {
    let (_dir, root) = project();
    keep(&root, CHAT, "old").unwrap();
    let later = SystemTime::now() + crate::retention::KEEP_FOR + Duration::from_secs(60);
    assert_eq!(sweep(&root, |_| true, SystemTime::now()), 0);
    assert_eq!(sweep(&root, |_| true, later), 1);
    assert_eq!(kept(&root, CHAT), None);
}

#[test]
fn the_sweep_takes_a_killed_write_s_temp_once_it_is_old() {
    let (_dir, root) = project();
    keep(&root, CHAT, "kept").unwrap();
    let temp = dir(&root).join(format!(".purlis-generated.{CHAT}.screen.123.ab.tmp"));
    std::fs::write(&temp, "half").unwrap();
    assert_eq!(
        sweep(&root, |_| true, SystemTime::now()),
        0,
        "a young one may be a write"
    );
    assert_eq!(
        sweep(&root, |_| true, SystemTime::now() + Duration::from_secs(60)),
        1
    );
    assert!(!temp.exists());
    assert!(kept(&root, CHAT).is_some());
}

#[test]
fn the_sweep_leaves_a_file_purlis_never_writes() {
    let (_dir, root) = project();
    keep(&root, CHAT, "x").unwrap();
    let stranger = dir(&root).join("notes.txt");
    std::fs::write(&stranger, "mine").unwrap();
    sweep(&root, |_| false, SystemTime::now());
    assert!(stranger.exists());
}

#[test]
fn a_project_s_open_takes_every_screen_while_no_hibernation_outlives_a_restart() {
    // SC-4e (#7634) makes the hibernated state durable and the open's sweep read it.
    let (_dir, root) = project();
    keep(&root, CHAT, "a").unwrap();
    keep(&root, OTHER, "b").unwrap();
    crate::retention::on_open(&root, SystemTime::now());
    assert_eq!(kept(&root, CHAT), None);
    assert_eq!(kept(&root, OTHER), None);
}

/// A snapshot as the engine takes it with `history` lines above a screen of `rows`, each line
/// `width` bytes and its own colour: `None` is all `lines` of it.
fn engine(lines: u32, rows: u32, width: usize) -> impl FnMut(Option<u32>) -> Option<String> {
    move |history: Option<u32>| {
        let above = history.map_or(lines, |history| history.min(lines));
        let first = lines - above;
        Some(
            (first..lines + rows)
                .map(|line| format!("\x1b[3{}m{:width$}\x1b[0m\r\n", line % 8, line))
                .collect(),
        )
    }
}

#[test]
fn a_screen_that_fits_is_taken_whole_in_one_look() {
    let mut asked = Vec::new();
    let mut take = engine(100, 24, 80);
    let shown = fitted(|history| {
        asked.push(history);
        take(history)
    })
    .unwrap();
    assert_eq!(asked, [None]);
    assert_eq!(shown.lines().count(), 124);
}

#[test]
fn a_scrollback_past_the_bound_is_taken_again_with_fewer_lines_until_it_fits() {
    // About 3 MiB of history: what is kept is at most 1 MiB, the newest lines, each whole.
    let (lines, rows, width) = (30_000, 24, 100);
    let shown = fitted(engine(lines, rows, width)).unwrap();
    assert!(shown.len() <= MOST_BYTES, "{}", shown.len());
    assert!(
        shown.len() > MOST_BYTES / 2,
        "keeps most of what fits: {}",
        shown.len()
    );
    // Every line is whole, so it redraws validly: each begins with its colour and ends with a
    // reset and a newline, and the screen's own last line is the last one kept.
    for line in shown.split_terminator("\r\n") {
        assert!(
            line.starts_with("\x1b[3") && line.ends_with("\x1b[0m"),
            "{line:?}"
        );
    }
    let last = format!("{:width$}", lines + rows - 1);
    assert!(shown.ends_with(&format!("{last}\x1b[0m\r\n")));
}

#[test]
fn a_screen_past_the_bound_with_no_history_at_all_is_not_kept() {
    // 48 rows of 40,000 bytes each: the screen alone is past it.
    assert_eq!(fitted(engine(0, 48, 40_000)), None);
}

#[test]
fn a_screen_that_cannot_be_taken_is_not_kept() {
    assert_eq!(fitted(|_| None), None);
}

/// A terminal of the engine purlis runs, `columns` wide, fed `lines` lines each of a different
/// colour in every cell, so its snapshot is many times its text.
fn a_long_coloured_scrollback(lines: u32, columns: u16) -> crate::engine::AlacrittyEngine {
    use crate::engine::{AlacrittyEngine, Engine, Size};
    let mut terminal = AlacrittyEngine::new(Size { columns, rows: 24 }, 5_000);
    for line in 0..lines {
        let cells: String = (0..columns)
            .map(|cell| format!("\x1b[3{}m{}", (u32::from(cell) + line) % 8, line % 10))
            .collect();
        terminal.advance(format!("{cells}\x1b[0m\r\n").as_bytes());
    }
    terminal
}

#[test]
fn the_engine_s_scrollback_past_a_mebibyte_is_kept_within_it_and_redraws_into_a_blank_terminal() {
    use crate::engine::{AlacrittyEngine, Engine, Size};
    let mut terminal = a_long_coloured_scrollback(5_000, 200);
    assert!(
        terminal.snapshot().len() > MOST_BYTES,
        "the case is past the bound"
    );

    let shown = fitted(|history| {
        let snapshot = match history {
            None => terminal.snapshot(),
            Some(history) => terminal.snapshot_within(history as usize),
        };
        String::from_utf8(snapshot).ok()
    })
    .expect("kept");

    assert!(shown.len() <= MOST_BYTES, "{}", shown.len());
    // Drawn into a blank terminal, it shows what the chat's screen showed, and its history is
    // the newest lines, each whole.
    let mut blank = AlacrittyEngine::new(
        Size {
            columns: 200,
            rows: 24,
        },
        5_000,
    );
    blank.advance(shown.as_bytes());
    assert_eq!(blank.screen().lines, terminal.screen().lines);
    assert!(blank.history() > 0 && blank.history() < terminal.history());
}

#[test]
fn an_open_alternate_screen_is_kept_alone() {
    use crate::engine::{AlacrittyEngine, Engine, Size};
    let mut terminal = a_long_coloured_scrollback(100, 40);
    terminal.advance(b"\x1b[?1049h\x1b[2J\x1b[Hthe editor's own screen");
    let shown = fitted(|_| String::from_utf8(terminal.snapshot()).ok()).expect("kept");
    let mut blank = AlacrittyEngine::new(
        Size {
            columns: 40,
            rows: 24,
        },
        5_000,
    );
    blank.advance(shown.as_bytes());
    assert_eq!(blank.screen().lines[0], "the editor's own screen");
    assert_eq!(blank.history(), 0, "none of the scrollback behind it");
}
