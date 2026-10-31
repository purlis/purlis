// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a profile is drawn with (#2922): its glyph, its own SVG, or its kind's monogram.

use super::*;

/// One profile drawn alone, as an answer of one row draws it.
struct Lone {
    glyph: Option<&'static str>,
    svg: Option<SafeSvg>,
    monogram: String,
}

/// What the profile `name` of `kind` is drawn with in an answer of its own.
fn of(name: &str, kind: &str, icon: Option<&str>, config_root: Option<&Path>) -> Lone {
    let mut answer = Answer::default();
    let drawn = answer.drawn(name, kind, icon, config_root);
    Lone {
        glyph: drawn.glyph,
        svg: drawn.own.and_then(|key| answer.svgs.remove(&key)),
        monogram: drawn.monogram,
    }
}

const PLAIN: &str = r#"<svg viewBox="0 0 24 24"><path d="M1 1h22"/></svg>"#;

/// A config home whose icons folder holds `files`.
fn home(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let icons = folder(dir.path());
    std::fs::create_dir_all(&icons).unwrap();
    for (name, text) in files {
        std::fs::write(icons.join(name), text).unwrap();
    }
    dir
}

#[test]
fn a_built_in_harness_is_drawn_with_its_glyph() {
    for (kind, glyph, letters) in [
        ("claude", "spark", "C"),
        ("opencode", "braces", "OC"),
        ("codex", "terminal", "CX"),
    ] {
        let icon = of(kind, kind, None, None);
        assert_eq!(icon.glyph, Some(glyph), "{kind}");
        assert_eq!(icon.svg, None);
        assert_eq!(icon.monogram, letters);
    }
}

#[test]
fn every_glyph_a_kind_is_drawn_with_is_in_the_icon_set() {
    for kind in crate::profiles::KINDS {
        let glyph = glyph_of_kind(kind.word).expect("every built-in kind has a glyph");
        assert!(crate::theme::vocabulary::GLYPHS.contains(&glyph), "{glyph}");
    }
}

#[test]
fn a_profile_that_names_no_icon_shows_its_kind_s_monogram() {
    let icon = of("claude-work", "claude", None, None);
    assert_eq!(icon.glyph, None);
    assert_eq!(icon.svg, None);
    assert_eq!(icon.monogram, "C");
    assert_eq!(monogram("shelly"), "S");
    assert_eq!(monogram("-"), "?");
}

#[test]
fn a_profile_can_name_a_glyph() {
    let icon = of("work", "codex", Some("spark"), None);
    assert_eq!(icon.glyph, Some("spark"));
    assert_eq!(icon.monogram, "CX");
}

#[test]
fn a_glyph_purlis_does_not_have_shows_the_monogram() {
    let icon = of("claude", "claude", Some("comet"), None);
    assert_eq!(icon.glyph, None);
    assert_eq!(icon.svg, None);
    assert_eq!(icon.monogram, "C");
}

#[test]
fn a_profile_can_name_an_svg_in_its_icons_folder() {
    let dir = home(&[("mine.svg", PLAIN)]);
    let icon = of("work", "opencode", Some("mine.svg"), Some(dir.path()));
    assert_eq!(icon.glyph, None);
    let svg = icon.svg.expect("the file is drawn");
    assert_eq!(svg.view_box, "0 0 24 24");
    assert_eq!(svg.shapes[0].attributes["d"], "M1 1h22");
    assert_eq!(icon.monogram, "OC");
}

#[test]
fn an_svg_purlis_cannot_draw_shows_the_monogram() {
    let dir = home(&[("bad.svg", "<html/>")]);
    for named in ["bad.svg", "missing.svg"] {
        let icon = of("work", "claude", Some(named), Some(dir.path()));
        assert_eq!((icon.glyph, icon.svg), (None, None), "{named}");
        assert_eq!(icon.monogram, "C");
    }
    // With no config home, nothing is read.
    let icon = of("work", "claude", Some("mine.svg"), None);
    assert_eq!(icon.svg, None);
}

#[test]
fn an_icon_file_is_named_inside_the_icons_folder_only() {
    for bad in [
        "../profiles.json",
        "/etc/passwd.svg",
        "sub/mine.svg",
        "..svg",
        ".hidden.svg",
        "a..b.svg",
        "mine.png",
        ".svg",
        "mine\\x.svg",
    ] {
        assert!(!file_name_ok(bad), "{bad}");
        assert_eq!(named(bad), Named::Neither, "{bad}");
    }
    for good in ["mine.svg", "Work-2.SVG", "a.b_c.svg"] {
        assert!(file_name_ok(good), "{good}");
    }
    assert!(!file_name_ok(&format!("{}.svg", "a".repeat(MOST_NAME))));
}

/// What [`read_file`] says of a file it does not read.
const UNREAD: &str = "the icon file is not a plain file an icon may be read from";

/// `read` run on a thread of its own, so a read that waited would fail the test rather than
/// hang it.
fn within_a_moment<T: Send + 'static>(read: impl FnOnce() -> T + Send + 'static) -> T {
    let (sent, got) = std::sync::mpsc::channel();
    std::thread::spawn(move || sent.send(read()));
    got.recv_timeout(std::time::Duration::from_secs(10))
        .expect("the read came back without waiting on the file")
}

#[cfg(unix)]
#[test]
fn an_icon_file_that_is_a_link_or_no_plain_file_is_not_read() {
    let dir = home(&[]);
    let outside = dir.path().join("outside.svg");
    std::fs::write(&outside, PLAIN).unwrap();
    std::os::unix::fs::symlink(&outside, folder(dir.path()).join("link.svg")).unwrap();
    std::fs::create_dir(folder(dir.path()).join("dir.svg")).unwrap();
    for name in ["link.svg", "dir.svg"] {
        assert_eq!(
            read_file(dir.path(), name).expect_err(name).0,
            UNREAD,
            "{name}"
        );
    }
    assert_eq!(
        read_file(dir.path(), "missing.svg")
            .expect_err("not there")
            .0,
        "the icon file is not there"
    );
}

#[cfg(unix)]
#[test]
fn an_icon_file_with_another_name_is_not_read() {
    let dir = home(&[]);
    let elsewhere = dir.path().join("elsewhere.svg");
    std::fs::write(&elsewhere, PLAIN).unwrap();
    std::fs::hard_link(&elsewhere, folder(dir.path()).join("linked.svg")).unwrap();
    assert_eq!(
        read_file(dir.path(), "linked.svg")
            .expect_err("two names")
            .0,
        UNREAD
    );
}

#[cfg(unix)]
#[test]
fn an_icons_folder_that_is_a_link_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("mine.svg"), PLAIN).unwrap();
    let config = crate::machine::dir(dir.path());
    std::fs::create_dir_all(&config).unwrap();
    std::os::unix::fs::symlink(&elsewhere, folder(dir.path())).unwrap();
    assert_eq!(
        read_file(dir.path(), "mine.svg").expect_err("a link").0,
        UNREAD
    );
}

/// **A pipe in the icons folder is never waited on**: the file is opened without waiting and
/// read only when it is a plain file, so the start options, which draw every profile's icon,
/// still load.
#[cfg(unix)]
#[test]
fn a_pipe_in_the_icons_folder_never_holds_up_the_read() {
    let dir = home(&[]);
    let pipe = folder(dir.path()).join("pipe.svg");
    // `mkfifo(1)`, which macOS and Linux both have: rustix makes no pipe on macOS.
    let made = crate::forklock::status(std::process::Command::new("mkfifo").arg(&pipe))
        .expect("mkfifo runs");
    assert!(made.success(), "a pipe is made");
    let root = dir.path().to_path_buf();
    let said = within_a_moment(move || read_file(&root, "pipe.svg").map(drop));
    assert_eq!(said.expect_err("a pipe").0, UNREAD);
    let root = dir.path().to_path_buf();
    let icon = within_a_moment(move || of("work", "claude", Some("pipe.svg"), Some(&root)));
    assert_eq!((icon.glyph, icon.svg), (None, None));
    assert_eq!(icon.monogram, "C");
}

#[test]
fn an_icon_file_past_the_cap_is_not_read() {
    let big = format!(
        "<svg viewBox=\"0 0 1 1\"><path d=\"M0 0\"/>{}</svg>",
        " ".repeat(safesvg::MOST_BYTES as usize)
    );
    let dir = home(&[("big.svg", &big)]);
    assert_eq!(
        read_file(dir.path(), "big.svg").expect_err("too big").0,
        TOO_LARGE
    );
}

/// An icon file of about `paths` x 15 KB once read: each path's `d` that long.
fn sized(paths: usize) -> String {
    let path = format!("<path d=\"M0 0{}\"/>", "h1".repeat(7_500));
    format!("<svg viewBox=\"0 0 24 24\">{}</svg>", path.repeat(paths))
}

/// **An answer that lists many profiles holds their icons to one budget** (#2922): a file is
/// read and sent once however many rows name it, and the answer's SVGs together stay within
/// [`MOST_ANSWER`], so a row past it shows its monogram.
#[test]
fn many_profiles_naming_icons_keep_the_answer_within_its_budget() {
    // Each just under the file's cap: four fit the answer's budget, and eight do not.
    let one = sized(4);
    let names: Vec<String> = (0..8).map(|at| format!("icon-{at}.svg")).collect();
    let files: Vec<(&str, &str)> = names
        .iter()
        .map(|name| (name.as_str(), one.as_str()))
        .chain([("shared.svg", one.as_str())])
        .collect();
    let dir = home(&files);

    // Three hundred profiles naming one icon: it is read once and sent once.
    let mut answer = Answer::default();
    let rows: Vec<Drawn> = (0..300)
        .map(|at| {
            answer.drawn(
                &format!("p{at}"),
                "claude",
                Some("shared.svg"),
                Some(dir.path()),
            )
        })
        .collect();
    assert!(
        rows.iter()
            .all(|row| row.own.as_deref() == Some("shared.svg"))
    );
    assert_eq!(answer.svgs.len(), 1);
    let sent = serde_json::to_string(&(&rows, &answer.svgs)).unwrap();
    assert!(sent.len() < MOST_ANSWER + 300 * 64, "{} bytes", sent.len());

    // Rows naming several icons: those that fit are sent, and the rest show the monogram.
    let mut answer = Answer::default();
    let rows: Vec<Drawn> = names
        .iter()
        .cycle()
        .take(200)
        .enumerate()
        .map(|(at, file)| answer.drawn(&format!("p{at}"), "codex", Some(file), Some(dir.path())))
        .collect();
    assert!(answer.spent() <= MOST_ANSWER);
    assert!(
        answer.svgs.len() < names.len(),
        "the budget holds some back"
    );
    assert!(!answer.svgs.is_empty());
    for row in &rows {
        match &row.own {
            Some(file) => assert!(answer.svgs.contains_key(file)),
            None => assert_eq!(row.monogram, "CX"),
        }
    }
    let sent = serde_json::to_string(&(&rows, &answer.svgs)).unwrap();
    assert!(
        sent.len() < MOST_ANSWER * 2 + 200 * 64,
        "{} bytes",
        sent.len()
    );
}

#[test]
fn an_answer_draws_a_built_in_and_a_glyph_as_a_lone_profile_is_drawn() {
    let mut answer = Answer::default();
    let built_in = answer.drawn("claude", "claude", None, None);
    assert_eq!((built_in.glyph, built_in.own), (Some("spark"), None));
    let named = answer.drawn("work", "opencode", Some("terminal"), None);
    assert_eq!(named.glyph, Some("terminal"));
    assert_eq!(answer.drawn("w", "opencode", None, None).monogram, "OC");
}

/// **An answer reads each file once, whatever case a row spells it in, and stops reading once
/// it can hold no more** (#2922).
#[test]
fn an_answer_reads_a_file_once_in_any_case_and_no_more_than_it_may() {
    let dir = home(&[("Mine.svg", PLAIN)]);
    let mut answer = Answer::default();
    let spellings = ["Mine.svg", "mine.svg", "MINE.SVG", "mInE.sVg"];
    let rows: Vec<Drawn> = spellings
        .iter()
        .map(|file| answer.drawn("work", "claude", Some(file), Some(dir.path())))
        .collect();
    assert_eq!(answer.svgs.len(), 1, "one file, read and sent once");
    assert_eq!(answer.asked.len(), 1);
    // The first row's spelling is the key; on a disk that tells case apart the others name no
    // file of their own, and on one that does not they are the same file: either way, one read.
    assert_eq!(rows[0].own.as_deref(), Some("Mine.svg"));
    for row in &rows[1..] {
        assert_eq!(row.own, rows[0].own);
    }

    // Past the most files an answer reads, a row names none and nothing more is read.
    let mut answer = Answer::default();
    for at in 0..(MOST_FILES + 10) {
        answer.drawn("w", "codex", Some(&format!("f{at}.svg")), Some(dir.path()));
    }
    assert_eq!(answer.asked.len(), MOST_FILES);

    // A spent budget reads nothing more.
    let mut answer = Answer {
        spent: MOST_ANSWER,
        ..Answer::default()
    };
    let row = answer.drawn("w", "codex", Some("Mine.svg"), Some(dir.path()));
    assert_eq!((row.own, answer.asked.len()), (None, 0));
}

/// **Why it fell back, in a fixed sentence** (#3137): the file missing, too large, not a plain
/// file, its contents refused, or a name purlis draws nothing for.
#[test]
fn an_icon_that_fell_back_says_why() {
    let big = format!(
        "<svg viewBox=\"0 0 1 1\"><path d=\"M0 0\"/>{}</svg>",
        " ".repeat(safesvg::MOST_BYTES as usize)
    );
    let dir = home(&[("big.svg", &big), ("refused.svg", "<svg>not closed")]);
    std::fs::create_dir(folder(dir.path()).join("dir.svg")).unwrap();
    let root = Some(dir.path());
    assert_eq!(
        fell_back(Some("missing.svg"), root),
        Some("the icon file is not there")
    );
    assert_eq!(fell_back(Some("big.svg"), root), Some(TOO_LARGE));
    assert_eq!(fell_back(Some("dir.svg"), root), Some(UNREAD));
    assert_eq!(
        fell_back(Some("refused.svg"), root),
        Some("the file is not well-formed SVG")
    );
    assert_eq!(fell_back(Some("no-such-glyph"), root), Some(NO_SUCH_ICON));
    assert_eq!(fell_back(Some("mine.svg"), None), Some(NO_CONFIG_HOME));
}

#[test]
fn an_icon_that_draws_or_is_not_named_has_nothing_to_say() {
    let dir = home(&[("mine.svg", PLAIN)]);
    let root = Some(dir.path());
    assert_eq!(fell_back(None, root), None);
    assert_eq!(fell_back(Some("spark"), None), None);
    assert_eq!(fell_back(Some("mine.svg"), root), None);
}

/// Whatever a file holds, what is said is one of the sentences purlis wrote.
#[test]
fn why_an_icon_fell_back_never_repeats_the_file() {
    let dir = home(&[("words.svg", "SECRET-WORDS<svg SECRET-WORDS>")]);
    let said = fell_back(Some("words.svg"), Some(dir.path())).expect("refused");
    assert!(!said.contains("SECRET"), "{said}");
}
