// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a profile is drawn with (#2922): its glyph, its own SVG, or its kind's monogram.

use super::*;

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

#[cfg(unix)]
#[test]
fn an_icon_file_that_is_a_link_or_no_plain_file_is_not_read() {
    let dir = home(&[]);
    let outside = dir.path().join("outside.svg");
    std::fs::write(&outside, PLAIN).unwrap();
    std::os::unix::fs::symlink(&outside, folder(dir.path()).join("link.svg")).unwrap();
    std::fs::create_dir(folder(dir.path()).join("dir.svg")).unwrap();
    assert_eq!(
        read_file(dir.path(), "link.svg").expect_err("a link").0,
        "the icon file is not a plain file"
    );
    assert_eq!(
        read_file(dir.path(), "dir.svg").expect_err("a folder").0,
        "the icon file is not a plain file"
    );
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
        "the file is larger than an icon may be"
    );
}
