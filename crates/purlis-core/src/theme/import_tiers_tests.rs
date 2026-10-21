// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The grey tiers through each import table** (#2941): a source's own quiet and disabled greys
//! go straight to `text.quiet` and `text.faint`, and where the source has no such grey the tier
//! follows the imported muted (ADR 0097's amendment of 2026-10-21). One test per table, at the
//! engine's interface: a source in, the theme the engine draws out.

use serde_json::Value;

use super::*;

const HARBOR: &[u8] = include_bytes!("import-fixtures/harbor.yaml");
const DUNE: &[u8] = include_bytes!("import-fixtures/dune.json");
const MOSS: &[u8] = include_bytes!("import-fixtures/moss");
const TIDE: &[u8] = include_bytes!("import-fixtures/tide.itermcolors");

/// The theme an import wrote, drawn by the engine with nothing refused or left out.
fn drawn(imported: &Imported) -> Theme {
    let document: Value = serde_json::from_slice(&imported.files[THEME_JSON]).expect("JSON");
    let resolved = Library::new().load(None, &document);
    assert!(!resolved.refused, "refused: {:?}", resolved.complaints);
    assert!(resolved.complaints.is_empty(), "{:?}", resolved.complaints);
    resolved.theme
}

/// A token's value in the first appearance, as `theme.json` holds it.
fn written(imported: &Imported, token: &str) -> Value {
    let document: Value = serde_json::from_slice(&imported.files[THEME_JSON]).expect("JSON");
    document["appearances"][0]["tokens"][token].clone()
}

/// The contrast findings the check has for `token`, as it is and in every workspace tint.
fn findings_for(theme: &Theme, token: &str) -> Vec<String> {
    check_every(theme)
        .into_iter()
        .flat_map(|checked| checked.report.findings)
        .filter_map(|finding| match finding {
            Finding::Contrast { pair, .. } if pair.front == token => {
                Some(format!("{} on {}", pair.front, pair.back))
            }
            _ => None,
        })
        .collect()
}

/// How a tier reads on the window, laid over it when it has alpha.
fn reads(look: &ResolvedTheme, token: &str) -> f64 {
    let window = look.value("surface.base");
    check::contrast(&check::onto(look.value(token), window), window)
}

/// The tiers never invert: muted reads at least as much as quiet, and quiet as faint.
fn in_order(theme: &Theme) {
    for look in &theme.appearances {
        let (muted, quiet, faint) = (
            reads(look, "text.muted"),
            reads(look, "text.quiet"),
            reads(look, "text.faint"),
        );
        assert!(
            muted >= quiet,
            "{}: muted {muted} under quiet {quiet}",
            look.id
        );
        assert!(
            quiet >= faint,
            "{}: quiet {quiet} under faint {faint}",
            look.id
        );
    }
}

/// The source has neither grey: quiet and faint are written nowhere and draw in its muted.
fn follows_muted(imported: &Imported) {
    assert_eq!(written(imported, "text.quiet"), Value::Null);
    assert_eq!(written(imported, "text.faint"), Value::Null);
    let theme = drawn(imported);
    for look in &theme.appearances {
        let muted = look.value("text.muted");
        assert_eq!(look.value("text.quiet"), muted, "{}", look.id);
        assert_eq!(look.value("text.faint"), muted, "{}", look.id);
    }
}

#[test]
fn a_vs_code_theme_s_line_numbers_are_quiet_and_its_disabled_words_faint() {
    let both = br##"{ "name": "Greys", "type": "dark", "colors": {
        "editor.background": "#1e1e1e", "editor.foreground": "#d4d4d4",
        "editorLineNumber.foreground": "#9a9a9a", "disabledForeground": "#7a7a7a" } }"##;
    let numbers = br##"{ "name": "Numbers", "type": "dark", "colors": {
        "editor.background": "#1e1e1e", "editorLineNumber.foreground": "#9a9a9a" } }"##;
    let neither = br##"{ "name": "Plain", "type": "dark", "colors": {
        "editor.background": "#1e1e1e", "editor.foreground": "#d4d4d4" } }"##;

    let both = import(SourceKind::Vscode, "greys.json", both).expect("imported");
    let theme = drawn(&both);
    let dark = theme.first();
    assert_eq!(
        dark.value("text.quiet"),
        "#9a9a9a",
        "editorLineNumber.foreground"
    );
    assert_eq!(dark.value("text.faint"), "#7a7a7a", "disabledForeground");
    assert_eq!(
        dark.value("text.muted"),
        "#9a9a9a",
        "muted is the line numbers' grey before the disabled one"
    );
    assert!(both.left_out.is_empty(), "{:?}", both.left_out);
    in_order(&theme);

    // Dim line numbers and a louder disabled grey: muted takes the louder, so it never reads
    // under either, and faint is left out to follow quiet rather than out-shout it.
    let dim = br##"{ "name": "Dim", "type": "dark", "colors": {
        "editor.background": "#1e1e1e", "editor.foreground": "#d4d4d4",
        "editorLineNumber.foreground": "#5a5a5a", "disabledForeground": "#8a8a8a" } }"##;
    let dim = import(SourceKind::Vscode, "dim.json", dim).expect("imported");
    assert_eq!(
        written(&dim, "text.faint"),
        Value::Null,
        "faint follows quiet"
    );
    let theme = drawn(&dim);
    let dark = theme.first();
    assert_eq!(dark.value("text.muted"), "#8a8a8a", "the louder grey");
    assert_eq!(dark.value("text.quiet"), "#5a5a5a");
    assert_eq!(dark.value("text.faint"), "#5a5a5a");
    assert!(
        reads(dark, "text.muted") >= 4.5,
        "muted reads on the editor"
    );
    in_order(&theme);

    // Line numbers and no disabled grey: faint follows quiet.
    let numbers = drawn(&import(SourceKind::Vscode, "numbers.json", numbers).expect("imported"));
    assert_eq!(numbers.first().value("text.faint"), "#9a9a9a");

    // Neither: both follow the theme's muted, Classic's here.
    let plain = import(SourceKind::Vscode, "plain.json", neither).expect("imported");
    follows_muted(&plain);
}

#[test]
fn a_base16_scheme_s_invisibles_are_faint_and_its_quiet_follows_muted() {
    let imported = import(SourceKind::Base16, "harbor.yaml", HARBOR).expect("imported");

    // base03 is the scheme's grey for invisibles: faint takes it, walked to the mark floor.
    let faint = written(&imported, "text.faint");
    assert_ne!(faint, Value::Null, "faint is written");
    assert_eq!(written(&imported, "text.quiet"), Value::Null);
    let theme = drawn(&imported);
    let dark = theme.first();
    assert_eq!(dark.value("text.quiet"), dark.value("text.muted"));
    assert_ne!(dark.value("text.faint"), dark.value("text.muted"));
    assert!(
        check::contrast(dark.value("text.faint"), dark.value("surface.base"))
            < check::contrast(dark.value("text.muted"), dark.value("surface.base")),
        "faint is fainter than muted"
    );
    assert_eq!(findings_for(&theme, "text.faint"), Vec::<String>::new());
    // base03 #5c6a73 is 2.94 on the window: under the mark floor, so it is walked toward base04.
    assert_ne!(faint, "base03");
    in_order(&theme);
}

#[test]
fn a_base16_scheme_s_faint_that_would_read_louder_than_muted_follows_it() {
    // A mid-grey selected row muted cannot clear stops muted's walk early; faint, held only to
    // the mark floor, would walk on to base05, the words' own colour.
    let scheme = b"scheme: \"Slate\"\nauthor: \"purlis tests\"\n\
base00: \"202020\"\nbase01: \"2a2a2a\"\nbase02: \"6c6c6c\"\nbase03: \"484848\"\n\
base04: \"808080\"\nbase05: \"c8c8c8\"\nbase06: \"e0e0e0\"\nbase07: \"f0f0f0\"\n\
base08: \"e06c75\"\nbase09: \"d19a66\"\nbase0A: \"e5c07b\"\nbase0B: \"98c379\"\n\
base0C: \"56b6c2\"\nbase0D: \"61afef\"\nbase0E: \"c678dd\"\nbase0F: \"be5046\"\n";

    let imported = import(SourceKind::Base16, "slate.yaml", scheme).expect("imported");

    assert_eq!(written(&imported, "text.faint"), Value::Null, "left out");
    let theme = drawn(&imported);
    let dark = theme.first();
    assert_eq!(dark.value("text.faint"), dark.value("text.muted"));
    assert_ne!(dark.value("text.faint"), dark.value("text.primary"));
    in_order(&theme);
}

#[test]
fn a_base24_scheme_s_invisibles_are_faint_and_its_quiet_follows_muted() {
    let imported = import(SourceKind::Base24, "dune.json", DUNE).expect("imported");

    assert_ne!(
        written(&imported, "text.faint"),
        Value::Null,
        "faint is written"
    );
    assert_eq!(written(&imported, "text.quiet"), Value::Null);
    let theme = drawn(&imported);
    let light = theme.first();
    assert_eq!(light.value("text.quiet"), light.value("text.muted"));
    assert_ne!(light.value("text.faint"), light.value("text.muted"));
    assert!(
        reads(light, "text.faint") < reads(light, "text.muted"),
        "faint is fainter than muted"
    );
    in_order(&theme);
    assert_eq!(findings_for(&theme, "text.faint"), Vec::<String>::new());
}

#[test]
fn an_iterm_preset_has_no_quiet_or_disabled_grey_and_its_tiers_follow_muted() {
    let imported = import(SourceKind::Iterm, "Tide.itermcolors", TIDE).expect("imported");

    follows_muted(&imported);
    in_order(&drawn(&imported));
}

#[test]
fn a_ghostty_theme_has_no_quiet_or_disabled_grey_and_its_tiers_follow_muted() {
    let imported = import(SourceKind::Ghostty, "moss", MOSS).expect("imported");

    follows_muted(&imported);
    in_order(&drawn(&imported));
}
