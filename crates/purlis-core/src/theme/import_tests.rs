// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Import and export at the engine's interface (#2334): each source kind through its mapping
//! table into a theme the engine draws, every key nothing took listed, and a theme folder's zip
//! giving the folder back.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use super::*;

const EMBER: &[u8] = include_bytes!("import-fixtures/ember.jsonc");
const HARBOR: &[u8] = include_bytes!("import-fixtures/harbor.yaml");
const HARBOR_TINTED: &[u8] = include_bytes!("import-fixtures/harbor-tinted.yaml");
const DUNE: &[u8] = include_bytes!("import-fixtures/dune.json");
const MOSS: &[u8] = include_bytes!("import-fixtures/moss");
const TIDE: &[u8] = include_bytes!("import-fixtures/tide.itermcolors");
const TIDE_BOTH: &[u8] = include_bytes!("import-fixtures/tide-both.itermcolors");

/// The theme an import wrote, as the engine reads it: drawn, with nothing left out.
fn drawn(imported: &Imported) -> Theme {
    let document: Value = serde_json::from_slice(&imported.files[THEME_JSON]).expect("JSON");
    assert_eq!(
        document["$schema"], SCHEMA_URL,
        "it names the published schema"
    );
    let resolved = Library::new().load(None, &document);
    assert!(!resolved.refused, "refused: {:?}", resolved.complaints);
    assert!(
        resolved.complaints.is_empty(),
        "the engine left out: {:?}",
        resolved.complaints
    );
    resolved.theme
}

fn keys(imported: &Imported) -> Vec<&str> {
    imported.left_out.iter().map(|it| it.key.as_str()).collect()
}

#[test]
fn a_vs_code_theme_with_comments_and_trailing_commas_imports_through_its_table() {
    let imported = import(SourceKind::Vscode, "ember.jsonc", EMBER).expect("imported");

    let theme = drawn(&imported);
    assert_eq!(imported.name, "Ember");
    assert_eq!(imported.folder, "ember");
    assert_eq!(theme.appearances.len(), 1);
    let dark = theme.first();
    assert_eq!(dark.appearance, Appearance::Dark);
    assert_eq!(dark.value("surface.base"), "#1c1a19");
    assert_eq!(dark.value("window.background"), "#1c1a19");
    assert_eq!(
        dark.value("terminal.background"),
        "#1c1a19",
        "falls to the editor's"
    );
    assert_eq!(dark.value("text.primary"), "#e8e2d9");
    assert_eq!(dark.value("accent.base"), "#e0814b");
    assert_eq!(dark.value("terminal.ansi.red"), "#d9594c");
    assert_eq!(dark.value("selection.background"), "#3a332e");
    assert_eq!(dark.value("syntax.comment"), "#7a716a");
    assert!(dark.typed.flag("syntax.comment.italic"));
    assert_eq!(
        dark.value("syntax.keyword"),
        "#e0814b",
        "keyword.control is keyword's"
    );
    assert!(dark.typed.flag("syntax.keyword.bold"));
    assert_eq!(
        dark.value("syntax.operator"),
        "#d0c8bd",
        "its own scope is nearer"
    );
    assert_eq!(dark.value("syntax.string"), "#a8c97f");
    assert_eq!(dark.value("markdown.heading"), "#f0b35c");
    // A token the source does not fill is Purlis Classic's.
    let classic = super::classic().first();
    assert_eq!(dark.value("danger.base"), classic.value("danger.base"));
}

#[test]
fn every_key_of_a_vs_code_theme_nothing_took_is_listed_with_why() {
    let imported = import(SourceKind::Vscode, "ember.jsonc", EMBER).expect("imported");

    assert_eq!(
        imported.left_out,
        vec![
            LeftOut::new("semanticHighlighting", "theme.json has no place for it"),
            LeftOut::new("colors.editorBracketMatch.border", "is not a hex colour"),
            LeftOut::new(
                "tokenColors meta.embedded string",
                "no token takes its scopes"
            ),
            LeftOut::new("colors.editorRuler.foreground", "no token takes it"),
        ]
    );
}

#[test]
fn a_vs_code_theme_s_type_says_its_appearance_and_high_contrast() {
    let light =
        br##"{ "name": "Day", "type": "light", "colors": { "editor.background": "#101010" } }"##;
    let hc = br##"{ "name": "Loud", "type": "hc", "colors": {} }"##;

    let day = drawn(&import(SourceKind::Vscode, "day.json", light).expect("imported"));
    let loud = drawn(&import(SourceKind::Vscode, "loud.json", hc).expect("imported"));

    assert_eq!(
        day.first().appearance,
        Appearance::Light,
        "its type wins over the background"
    );
    assert_eq!(loud.first().appearance, Appearance::Dark);
    assert!(loud.first().high_contrast);
}

#[test]
fn a_vs_code_theme_that_names_other_files_says_import_does_not_read_them() {
    let source =
        br#"{ "name": "Half", "include": "./base.json", "tokenColors": "./syntax.tmTheme" }"#;

    let imported = import(SourceKind::Vscode, "half.json", source).expect("imported");

    drawn(&imported);
    assert_eq!(keys(&imported), ["include", "tokenColors"]);
    assert!(imported.left_out[0].why.contains("does not read"));
}

#[test]
fn a_base16_scheme_keeps_its_colours_as_the_palette() {
    let imported = import(SourceKind::Base16, "harbor.yaml", HARBOR).expect("imported");

    let theme = drawn(&imported);
    let document: Value = serde_json::from_slice(&imported.files[THEME_JSON]).expect("JSON");
    assert_eq!(document["palette"]["base0d"], "#61afef");
    assert_eq!(
        document["appearances"][0]["tokens"]["accent.base"],
        "base0d"
    );
    let dark = theme.first();
    assert_eq!(dark.appearance, Appearance::Dark, "from its background");
    assert_eq!(dark.value("surface.base"), "#1b2024");
    assert_eq!(dark.value("text.primary"), "#c7d3da");
    assert_eq!(
        dark.value("terminal.ansi.bright-red"),
        "#e06c75",
        "base16 repeats the normal"
    );
    assert_eq!(dark.value("syntax.keyword"), "#c678dd");
    assert_eq!(imported.name, "Harbor");
}

#[test]
fn a_base16_scheme_s_unused_colour_and_its_author_are_listed() {
    let imported = import(SourceKind::Base16, "harbor.yaml", HARBOR).expect("imported");

    assert_eq!(
        imported.left_out,
        vec![
            LeftOut::new("author", "theme.json has no place for it"),
            LeftOut::new("base06", "no token takes it"),
        ]
    );
}

#[test]
fn the_tinted_form_of_a_scheme_imports_as_the_classic_one_does() {
    let classic = import(SourceKind::Base16, "harbor.yaml", HARBOR).expect("imported");
    let tinted = import(SourceKind::Base16, "harbor-tinted.yaml", HARBOR_TINTED).expect("imported");

    assert_eq!(tinted.files, classic.files);
}

#[test]
fn a_base24_scheme_takes_its_own_bright_colours_and_deeper_backgrounds() {
    let imported = import(SourceKind::Base24, "dune.json", DUNE).expect("imported");

    let theme = drawn(&imported);
    let light = theme.first();
    assert_eq!(light.appearance, Appearance::Light, "its variant says");
    assert_eq!(light.value("terminal.ansi.red"), "#b5473a");
    assert_eq!(light.value("terminal.ansi.bright-red"), "#c9554a");
    assert_eq!(light.value("surface.deep"), "#fffcf6");
    assert_eq!(keys(&imported), ["base06"]);
}

#[test]
fn a_base24_scheme_read_as_base16_lists_its_extra_colours() {
    let imported = import(SourceKind::Base16, "dune.json", DUNE).expect("imported");

    drawn(&imported);
    let extra: Vec<&str> = keys(&imported)
        .into_iter()
        .filter(|it| it.starts_with("palette.base1"))
        .collect();
    assert_eq!(extra.len(), 8, "{:?}", imported.left_out);
    assert!(keys(&imported).contains(&"system"));
}

#[test]
fn an_iterm_preset_imports_its_components_and_alpha() {
    let imported = import(SourceKind::Iterm, "Tide.itermcolors", TIDE).expect("imported");

    let theme = drawn(&imported);
    assert_eq!(imported.name, "Tide");
    let dark = theme.first();
    assert_eq!(dark.appearance, Appearance::Dark);
    assert_eq!(dark.value("terminal.background"), "#10141e");
    assert_eq!(dark.value("terminal.ansi.blue"), "#3366cc");
    // No link colour: its blue, walked three tenths toward its words (#e6e6e6) to read on the
    // window at the words' floor (#2589).
    assert_eq!(
        dark.value("markdown.link"),
        tenth("#3366cc", "#e6e6e6", 3),
        "no link colour: its blue"
    );
    assert_eq!(dark.value("terminal.selection"), "#33406680");
    assert_eq!(
        imported.left_out,
        vec![
            LeftOut::new("Name", "is not a colour"),
            LeftOut::new("Badge Color", "no token takes it"),
        ]
    );
}

#[test]
fn an_iterm_preset_with_dark_and_light_colours_has_both_appearances() {
    let imported = import(SourceKind::Iterm, "both.itermcolors", TIDE_BOTH).expect("imported");

    let theme = drawn(&imported);
    let ids: Vec<(&str, Appearance)> = theme
        .appearances
        .iter()
        .map(|one| (one.id.as_str(), one.appearance))
        .collect();
    assert_eq!(
        ids,
        [("dark", Appearance::Dark), ("light", Appearance::Light)]
    );
    assert_eq!(theme.appearances[0].value("surface.base"), "#0d0d0d");
    assert_eq!(theme.appearances[1].value("surface.base"), "#fafafa");
    assert_eq!(
        theme.appearances[1].value("terminal.ansi.blue"),
        "#3366cc",
        "a plain colour is in both"
    );
    assert!(imported.left_out.is_empty(), "{:?}", imported.left_out);
}

#[test]
fn a_ghostty_theme_imports_its_palette_and_lists_the_rest() {
    let imported = import(SourceKind::Ghostty, "moss", MOSS).expect("imported");

    let theme = drawn(&imported);
    assert_eq!(imported.name, "moss");
    let dark = theme.first();
    assert_eq!(dark.value("terminal.background"), "#1a201c");
    assert_eq!(dark.value("terminal.ansi.bright-cyan"), "#72cbbb");
    assert_eq!(dark.value("selection.text"), "#e8eee8");
    let document: Value = serde_json::from_slice(&imported.files[THEME_JSON]).expect("JSON");
    assert_eq!(document["palette"]["palette-4"], "#6fa3c8");
    assert_eq!(
        imported.left_out,
        vec![
            LeftOut::new("font-family", "is not a colour a theme takes"),
            LeftOut::new("palette 16", "no token takes it"),
        ]
    );
}

#[test]
fn each_fixture_s_kind_is_told_from_its_name_and_its_words() {
    assert_eq!(detect("ember.jsonc", EMBER), Some(SourceKind::Vscode));
    assert_eq!(detect("harbor.yaml", HARBOR), Some(SourceKind::Base16));
    assert_eq!(detect("dune.json", DUNE), Some(SourceKind::Base24));
    assert_eq!(detect("moss", MOSS), Some(SourceKind::Ghostty));
    assert_eq!(detect("Tide.itermcolors", TIDE), Some(SourceKind::Iterm));
    assert_eq!(
        detect("tide", TIDE),
        Some(SourceKind::Iterm),
        "from its words alone"
    );
    assert_eq!(detect("harbor", HARBOR), Some(SourceKind::Base16));
    assert_eq!(detect("a.zip", b"PK\x03\x04"), Some(SourceKind::Zip));
    assert_eq!(detect("notes.txt", b"nothing a theme says"), None);
}

#[test]
fn a_source_that_breaks_its_grammar_is_refused_with_where() {
    let list = b"scheme: \"Odd\"\nbase00:\n  - \"101010\"\n";
    let ghostty = b"background = 101010\njust words\n";

    let yaml = import(SourceKind::Base16, "odd.yaml", list).expect_err("refused");
    let words = import(SourceKind::Ghostty, "odd", ghostty).expect_err("refused");
    let json = import(SourceKind::Vscode, "odd.json", b"{ nope").expect_err("refused");

    assert!(yaml.contains("line 3"), "{yaml}");
    assert!(words.contains("line 2"), "{words}");
    assert!(json.contains("JSON with comments"), "{json}");
}

#[test]
fn a_source_past_the_cap_is_refused_before_it_is_read() {
    let big = vec![b' '; MOST_SOURCE_BYTES as usize + 1];

    let refused = import(SourceKind::Vscode, "big.json", &big).expect_err("refused");

    assert!(refused.contains("larger than"), "{refused}");
}

#[test]
fn a_theme_s_folder_is_named_from_its_name() {
    assert_eq!(
        folder_name("Solarized Dark (Patched)"),
        "solarized-dark-patched"
    );
    assert_eq!(folder_name("1984"), "theme-1984");
    assert_eq!(folder_name("✨"), "imported-theme");
    assert!(folder_name(&"a".repeat(80)).len() <= 32);
}

#[test]
fn every_table_names_only_purlis_tokens_and_the_docs_print_each() {
    let docs = include_str!("../../docs/themes.md");
    for kind in SourceKind::ALL.iter().filter(|it| **it != SourceKind::Zip) {
        for (token, keys) in table(*kind) {
            assert!(vocabulary::is_token(token), "{token} in {kind:?}");
            assert!(!keys.is_empty());
        }
        assert!(
            docs.contains(&table_markdown(*kind)),
            "docs/themes.md does not print the {} table as the engine has it:\n{}",
            kind.name(),
            table_markdown(*kind)
        );
    }
    assert!(docs.contains(&scopes_markdown()), "{}", scopes_markdown());
    for (token, _) in SCOPES {
        assert!(vocabulary::is_token(token), "{token}");
    }
}

// ------------------------------------------------------------------------------------------
// Export and the zip
// ------------------------------------------------------------------------------------------

/// A theme folder `name` in `dir` with a theme, a licence, a font and an icon set.
fn a_folder(dir: &Path, name: &str) -> std::path::PathBuf {
    let folder = dir.join(name);
    std::fs::create_dir_all(folder.join("fonts")).expect("fonts");
    std::fs::create_dir_all(folder.join("icons")).expect("icons");
    std::fs::write(
        folder.join(THEME_JSON),
        r##"{ "name": "Kept", "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {} }] }"##,
    )
    .expect("theme.json");
    std::fs::write(folder.join("LICENSE"), "made up for a test\n").expect("licence");
    std::fs::write(folder.join("fonts/ui.woff2"), b"wOF2 and some bytes").expect("font");
    std::fs::write(folder.join("icons/set.json"), "{}").expect("icons");
    folder
}

#[test]
fn an_exported_folder_unpacks_to_the_same_files_under_its_name() {
    let dir = tempfile::tempdir().expect("a directory");
    let folder = a_folder(dir.path(), "kept");

    let packed = export(&folder).expect("exported");
    let (name, files) = unpack(&packed.bytes).expect("unpacked");

    assert_eq!(name, "kept");
    assert_eq!(packed.files, 4);
    let (on_disk, left_out) = gather(&folder).expect("gathered");
    assert_eq!(files, on_disk);
    assert!(left_out.is_empty());
    assert_eq!(
        files.keys().map(String::as_str).collect::<Vec<_>>(),
        ["LICENSE", "fonts/ui.woff2", "icons/set.json", "theme.json"]
    );
}

#[test]
fn an_import_of_an_exported_zip_is_the_folder_it_was() {
    let dir = tempfile::tempdir().expect("a directory");
    let folder = a_folder(dir.path(), "kept");
    let packed = export(&folder).expect("exported");

    let imported = import(SourceKind::Zip, "kept.zip", &packed.bytes).expect("imported");

    assert_eq!(imported.folder, "kept");
    assert_eq!(imported.name, "Kept");
    assert_eq!(imported.files, gather(&folder).expect("gathered").0);
    assert!(imported.left_out.is_empty());
}

#[test]
fn the_same_folder_is_the_same_zip_every_time() {
    let dir = tempfile::tempdir().expect("a directory");
    let folder = a_folder(dir.path(), "kept");

    assert_eq!(
        export(&folder).expect("once").bytes,
        export(&folder).expect("twice").bytes
    );
}

#[test]
fn what_a_theme_folder_does_not_keep_is_listed_and_not_packed() {
    let dir = tempfile::tempdir().expect("a directory");
    let folder = a_folder(dir.path(), "kept");
    std::fs::write(folder.join(".DS_Store"), "x").expect("hidden");
    std::fs::create_dir_all(folder.join("drafts")).expect("another folder");
    std::fs::write(folder.join("drafts/old.json"), "{}").expect("a draft");
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path(), folder.join("elsewhere")).expect("a link");

    let packed = export(&folder).expect("exported");

    let left: Vec<&str> = packed.left_out.iter().map(|it| it.key.as_str()).collect();
    let mut expected = vec![".DS_Store", "drafts"];
    if cfg!(unix) {
        expected.push("elsewhere");
    }
    assert_eq!(left, expected);
    assert_eq!(packed.files, 4);
}

#[test]
fn a_folder_without_a_theme_json_is_not_exported() {
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::create_dir_all(dir.path().join("empty")).expect("a folder");

    let refused = export(&dir.path().join("empty")).expect_err("refused");

    assert!(refused.contains("no theme.json"), "{refused}");
}

/// A zip holding `entries`, each a name and its bytes, written as the zip crate writes any.
fn a_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in entries {
        writer.start_file(*name, options).expect("an entry");
        writer.write_all(bytes).expect("its bytes");
    }
    writer.finish().expect("a zip").into_inner()
}

#[test]
fn a_zip_whose_entries_climb_out_or_start_at_the_root_is_refused_whole() {
    let theme: &[u8] = b"{}";
    for bad in [
        "kept/../escape.json",
        "/etc/kept/theme.json",
        "kept/fonts/../../x",
        "C:/kept/theme.json",
        "kept\\theme.json",
    ] {
        let zip = a_zip(&[("kept/theme.json", theme), (bad, b"x")]);

        let refused = unpack(&zip).expect_err(bad);

        assert!(refused.contains("in the zip"), "{bad}: {refused}");
    }
}

#[test]
fn a_zip_holding_a_link_is_refused() {
    use std::io::Write;
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    writer
        .start_file("kept/theme.json", options)
        .expect("an entry");
    writer.write_all(b"{}").expect("its bytes");
    writer
        .add_symlink("kept/LICENSE", "/etc/passwd", options)
        .expect("a link");
    let zip = writer.finish().expect("a zip").into_inner();

    let refused = unpack(&zip).expect_err("refused");

    assert!(refused.contains("is a link"), "{refused}");
}

#[test]
fn a_zip_with_two_folders_a_stray_file_or_no_theme_json_is_refused() {
    let theme: &[u8] = b"{}";
    let two = a_zip(&[("kept/theme.json", theme), ("other/theme.json", theme)]);
    let deep = a_zip(&[
        ("kept/theme.json", theme),
        ("kept/fonts/more/x.woff2", b"x"),
    ]);
    let none = a_zip(&[("kept/LICENSE", b"x")]);

    assert!(unpack(&two).expect_err("two").contains("one folder"));
    assert!(unpack(&deep).expect_err("deep").contains("keeps a file"));
    assert!(unpack(&none).expect_err("none").contains("no theme.json"));
}

#[test]
fn a_zip_entry_past_the_cap_is_refused() {
    let big = vec![b'a'; MOST_FILE_BYTES as usize + 1];
    let zip = a_zip(&[("kept/theme.json", b"{}"), ("kept/fonts/big.woff2", &big)]);

    let refused = unpack(&zip).expect_err("refused");

    assert!(refused.contains("larger than"), "{refused}");
}

#[test]
fn a_zip_with_too_many_files_is_refused() {
    let names: Vec<String> = (0..=MOST_FILES)
        .map(|at| format!("kept/icons/{at}.json"))
        .collect();
    let mut entries: Vec<(&str, &[u8])> = vec![("kept/theme.json", b"{}")];
    entries.extend(names.iter().map(|it| (it.as_str(), &b"{}"[..])));

    let refused = unpack(&a_zip(&entries)).expect_err("refused");

    assert!(refused.contains("more than"), "{refused}");
}

#[test]
fn a_staged_folder_goes_in_place_and_never_over_one_unless_asked() {
    let dir = tempfile::tempdir().expect("a directory");
    let themes = dir.path().join("themes");
    let files = Files::from([(THEME_JSON.to_owned(), b"{\"name\":\"One\"}".to_vec())]);

    let first = stage(&themes, "one", &files, false)
        .expect("staged")
        .commit()
        .expect("in place");
    let again = stage(&themes, "one", &files, false).expect_err("there already");
    let newer = Files::from([(THEME_JSON.to_owned(), b"{\"name\":\"Two\"}".to_vec())]);
    stage(&themes, "one", &newer, true)
        .expect("staged")
        .commit()
        .expect("replaced");

    assert_eq!(first, themes.join("one"));
    assert_eq!(again, NotWritten::Exists(themes.join("one")));
    assert_eq!(
        std::fs::read(themes.join("one").join(THEME_JSON)).expect("read"),
        b"{\"name\":\"Two\"}"
    );
    let left: Vec<_> = std::fs::read_dir(&themes)
        .expect("listed")
        .flatten()
        .collect();
    assert_eq!(left.len(), 1, "nothing staged or moved aside is left");
}

#[test]
fn a_staged_folder_dropped_leaves_nothing() {
    let dir = tempfile::tempdir().expect("a directory");
    let files = Files::from([(THEME_JSON.to_owned(), b"{}".to_vec())]);

    let staged = stage(dir.path(), "one", &files, false).expect("staged");
    let path = staged.path().to_path_buf();
    assert!(path.join(THEME_JSON).is_file());
    drop(staged);

    assert!(!path.exists());
    assert!(!dir.path().join("one").exists());
}

#[cfg(unix)]
#[test]
fn a_link_where_the_folder_goes_is_never_replaced() {
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::create_dir_all(dir.path().join("real")).expect("a folder");
    std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("one")).expect("a link");
    let files = Files::from([(THEME_JSON.to_owned(), b"{}".to_vec())]);

    let refused = stage(dir.path(), "one", &files, true).expect_err("refused");

    assert!(matches!(refused, NotWritten::Failed(why) if why.contains("is a link")));
}

// ------------------------------------------------------------------------------------------
// Review round: the move in, nesting, ids, and what a zip can claim
// ------------------------------------------------------------------------------------------

#[test]
fn a_folder_that_appears_after_the_stage_is_never_replaced_without_force() {
    let dir = tempfile::tempdir().expect("a directory");
    let files = Files::from([(THEME_JSON.to_owned(), b"{}".to_vec())]);
    let staged = stage(dir.path(), "one", &files, false).expect("staged");
    std::fs::create_dir(dir.path().join("one")).expect("an empty folder meanwhile");

    let refused = staged.commit().expect_err("refused");

    assert_eq!(refused, NotWritten::Exists(dir.path().join("one")));
    assert_eq!(
        std::fs::read_dir(dir.path().join("one"))
            .expect("listed")
            .count(),
        0,
        "the folder there is as it was"
    );
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .expect("listed")
        .flatten()
        .collect();
    assert_eq!(names.len(), 1, "the stage is gone");
}

#[test]
fn with_force_a_folder_that_appears_after_the_stage_is_exchanged_and_removed() {
    let dir = tempfile::tempdir().expect("a directory");
    let files = Files::from([(THEME_JSON.to_owned(), b"{\"name\":\"New\"}".to_vec())]);
    let staged = stage(dir.path(), "one", &files, true).expect("staged");
    std::fs::create_dir(dir.path().join("one")).expect("a folder meanwhile");
    std::fs::write(dir.path().join("one/old.json"), "{}").expect("an old file");

    let put = staged.commit().expect("in place");

    assert_eq!(put, dir.path().join("one"));
    assert_eq!(
        std::fs::read(put.join(THEME_JSON)).expect("read"),
        b"{\"name\":\"New\"}"
    );
    assert!(!put.join("old.json").exists());
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .expect("listed")
        .flatten()
        .collect();
    assert_eq!(names.len(), 1, "nothing is left beside it");
}

#[test]
fn a_stage_an_import_left_behind_an_hour_ago_is_swept() {
    let dir = tempfile::tempdir().expect("a directory");
    let left = dir.path().join(".gone.importing-1");
    let fresh = dir.path().join(".busy.importing-2");
    std::fs::create_dir(&left).expect("a stale stage");
    std::fs::create_dir(&fresh).expect("a fresh stage");
    let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 60 * 60);
    std::fs::File::open(&left)
        .and_then(|it| it.set_modified(long_ago))
        .expect("aged");
    let files = Files::from([(THEME_JSON.to_owned(), b"{}".to_vec())]);

    drop(stage(dir.path(), "one", &files, false).expect("staged"));

    assert!(!left.exists(), "the stale one is swept");
    assert!(fresh.exists(), "one another import may be writing is not");
}

/// A property list nesting `depth` dictionaries.
fn nested_plist(depth: usize) -> Vec<u8> {
    let mut xml = String::from("<?xml version=\"1.0\"?>\n<plist version=\"1.0\">\n");
    for _ in 0..depth {
        xml.push_str("<dict><key>k</key>");
    }
    xml.push_str("<string>x</string>");
    for _ in 0..depth {
        xml.push_str("</dict>");
    }
    xml.push_str("</plist>\n");
    xml.into_bytes()
}

#[test]
fn a_property_list_nested_deeper_than_a_preset_is_refused_before_it_is_parsed() {
    let deep = nested_plist(9);
    let very_deep = nested_plist(30_000);

    let refused = import(SourceKind::Iterm, "deep.itermcolors", &deep).expect_err("refused");
    let very = import(SourceKind::Iterm, "deeper.itermcolors", &very_deep);

    assert!(refused.contains("deep"), "{refused}");
    assert!(very_deep.len() as u64 <= MOST_SOURCE_BYTES);
    assert!(very.expect_err("refused").contains("deep"));
    assert!(
        import(SourceKind::Iterm, "tide.itermcolors", TIDE).is_ok(),
        "a preset nests two"
    );
}

#[test]
fn a_reserved_or_ungrammatical_id_is_never_a_theme_folder() {
    let dir = tempfile::tempdir().expect("a directory");
    let files = Files::from([(THEME_JSON.to_owned(), b"{}".to_vec())]);
    for id in [
        "purlis-classic",
        "purlis-dark",
        "purlis-light",
        "charter-dark",
        "mine",
        "system",
    ] {
        assert!(is_reserved(id), "{id}");
        assert!(
            matches!(
                stage(dir.path(), id, &files, true),
                Err(NotWritten::Failed(_))
            ),
            "{id}"
        );
        let zip = a_zip(&[(&format!("{id}/theme.json"), b"{}")]);
        assert!(
            unpack(&zip).expect_err(id).contains("keeps for its own"),
            "{id}"
        );
    }
    for id in ["Kept", "kept theme", "9lives", &"a".repeat(33)] {
        assert!(!is_folder_id(id), "{id}");
        let zip = a_zip(&[(&format!("{id}/theme.json"), b"{}")]);
        assert!(
            unpack(&zip)
                .expect_err(id)
                .contains("not a theme folder's id"),
            "{id}"
        );
    }
    let named = br#"{ "name": "Purlis Classic", "colors": {} }"#;
    let refused = import(SourceKind::Vscode, "classic.json", named).expect_err("reserved");
    assert!(refused.contains("keeps for its own"), "{refused}");
    std::fs::create_dir_all(dir.path().join("mine")).expect("a folder");
    std::fs::write(dir.path().join("mine").join(THEME_JSON), "{}").expect("a theme");
    assert!(export(&dir.path().join("mine")).is_err());
}

/// Every offset of `signature` in `bytes`.
fn offsets(bytes: &[u8], signature: &[u8; 4]) -> Vec<usize> {
    bytes
        .windows(4)
        .enumerate()
        .filter(|(_, it)| *it == signature)
        .map(|(at, _)| at)
        .collect()
}

#[test]
fn an_entry_that_declares_less_than_it_holds_is_refused() {
    let big = vec![b'a'; MOST_FILE_BYTES as usize + 10];
    let mut zip = a_zip(&[("kept/theme.json", b"{}"), ("kept/fonts/big.woff2", &big)]);
    // The second entry's uncompressed size, in its local header and its central one.
    let local = offsets(&zip, b"PK\x03\x04")[1];
    let central = offsets(&zip, b"PK\x01\x02")[1];
    zip[local + 22..local + 26].copy_from_slice(&100u32.to_le_bytes());
    zip[central + 24..central + 28].copy_from_slice(&100u32.to_le_bytes());

    let refused = unpack(&zip).expect_err("refused");

    assert!(refused.contains("larger than"), "{refused}");
}

#[test]
fn a_zip_past_the_folder_s_total_is_refused() {
    let one = vec![0u8; MOST_FILE_BYTES as usize];
    let names: Vec<String> = (0..9).map(|at| format!("kept/fonts/{at}.woff2")).collect();
    let mut entries: Vec<(&str, &[u8])> = vec![("kept/theme.json", b"{}")];
    entries.extend(names.iter().map(|it| (it.as_str(), one.as_slice())));

    let refused = unpack(&a_zip(&entries)).expect_err("refused");

    assert!(refused.contains("MiB together"), "{refused}");
}

#[test]
fn two_names_one_filesystem_writes_as_one_file_are_refused() {
    let theme: &[u8] = b"{}";
    let case = a_zip(&[
        ("kept/theme.json", theme),
        ("kept/LICENSE", b"a"),
        ("kept/license", b"b"),
    ]);
    let accents = a_zip(&[
        ("kept/theme.json", theme),
        ("kept/caf\u{e9}", b"a"),
        ("kept/cafe\u{301}", b"b"),
    ]);
    let mut twice = a_zip(&[
        ("kept/theme.json", theme),
        ("kept/aaaa", b"a"),
        ("kept/bbbb", b"b"),
    ]);
    for at in offsets(&twice, b"PK\x03\x04")
        .into_iter()
        .chain(offsets(&twice, b"PK\x01\x02"))
    {
        let name = twice
            .windows(4)
            .skip(at)
            .position(|it| it == b"bbbb")
            .map(|it| it + at);
        if let Some(name) = name.filter(|it| it - at < 64) {
            twice[name..name + 4].copy_from_slice(b"aaaa");
        }
    }

    assert!(unpack(&case).expect_err("case").contains("other case"));
    assert!(
        unpack(&accents)
            .expect_err("accents")
            .contains("other case")
    );
    assert!(unpack(&twice).is_err(), "an entry named twice");
}

#[test]
fn an_encrypted_entry_is_refused() {
    let mut zip = a_zip(&[("kept/theme.json", b"{}")]);
    let local = offsets(&zip, b"PK\x03\x04")[0];
    let central = offsets(&zip, b"PK\x01\x02")[0];
    zip[local + 6] |= 1;
    zip[central + 8] |= 1;

    let refused = unpack(&zip).expect_err("refused");

    assert!(refused.contains("encrypted"), "{refused}");
}

#[test]
fn a_zip_whose_end_lists_too_many_entries_is_refused_before_it_is_read() {
    let mut zip = a_zip(&[("kept/theme.json", b"{}")]);
    let end = *offsets(&zip, b"PK\x05\x06").last().expect("an end");
    zip[end + 8..end + 10].copy_from_slice(&1000u16.to_le_bytes());
    zip[end + 10..end + 12].copy_from_slice(&1000u16.to_le_bytes());

    let refused = unpack(&zip).expect_err("refused");

    assert!(refused.contains("lists 1000 entries"), "{refused}");
}

#[test]
fn a_key_a_nearer_key_shadowed_says_which_token_it_lost() {
    let source = br##"{ "name": "Near", "colors": { "input.background": "#101010", "dropdown.background": "#202020" } }"##;

    let imported = import(SourceKind::Vscode, "near.json", source).expect("imported");

    assert_eq!(
        imported.left_out,
        vec![LeftOut::new(
            "colors.dropdown.background",
            "a nearer key gave control.base its value"
        )]
    );
}

/// A property list that nests `depth` lists, each opened as `open` and closed as `close`.
fn nested_with(depth: usize, open: &str, close: &str) -> Vec<u8> {
    let mut xml = String::from("<?xml version=\"1.0\"?>\n<plist version=\"1.0\">\n");
    xml.push_str(&open.repeat(depth));
    xml.push_str(&close.repeat(depth));
    xml.push_str("</plist>\n");
    xml.into_bytes()
}

#[test]
fn nesting_hidden_by_a_prefix_or_a_comment_is_counted_as_the_reader_reads_it() {
    let prefixed = nested_with(40_000, "<a:array>", "</a:array>");
    let commented = nested_with(30_000, "<array><!--</array>-->", "</array>");
    let in_cdata = nested_with(9, "<array><![CDATA[</array>]]>", "</array>");

    for (what, xml) in [
        ("prefixed", prefixed),
        ("commented", commented),
        ("cdata", in_cdata),
    ] {
        assert!(xml.len() as u64 <= MOST_SOURCE_BYTES, "{what}");
        let refused = import(SourceKind::Iterm, "deep.itermcolors", &xml).expect_err(what);
        assert!(!refused.is_empty(), "{what}");
    }
}

#[test]
fn what_the_macos_finder_adds_to_a_zip_is_passed_over() {
    let zip = a_zip(&[
        ("kept/theme.json", b"{}"),
        ("kept/.DS_Store", b"x"),
        ("__MACOSX/kept/._theme.json", b"x"),
    ]);

    let (name, files) = unpack(&zip).expect("unpacked");

    assert_eq!(name, "kept");
    assert_eq!(
        files.keys().map(String::as_str).collect::<Vec<_>>(),
        [THEME_JSON]
    );
}

#[test]
fn an_end_record_inside_a_comment_is_not_the_zip_s_end() {
    let zip = a_zip(&[("kept/theme.json", b"{}")]);
    // A comment that holds a record listing 1000 entries: the real end is the one before it.
    let mut fake = b"PK\x05\x06".to_vec();
    fake.extend_from_slice(&[0, 0, 0, 0]);
    fake.extend_from_slice(&1000u16.to_le_bytes());
    fake.extend_from_slice(&1000u16.to_le_bytes());
    // Its own comment's length runs past the end of the file, so it is no end record.
    fake.extend_from_slice(&[0; 8]);
    fake.extend_from_slice(&5u16.to_le_bytes());
    let mut with_comment = zip.clone();
    let end = *offsets(&zip, b"PK\x05\x06").last().expect("an end");
    with_comment[end + 20..end + 22].copy_from_slice(&(fake.len() as u16).to_le_bytes());
    with_comment.extend_from_slice(&fake);

    let (name, _) = unpack(&with_comment).expect("unpacked");

    assert_eq!(name, "kept");
}

#[cfg(unix)]
#[test]
fn a_link_swapped_in_at_the_target_under_force_is_put_back_and_refused() {
    let dir = tempfile::tempdir().expect("a directory");
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).expect("a folder");
    std::fs::write(elsewhere.join("kept.txt"), "mine").expect("a file");
    let files = Files::from([(THEME_JSON.to_owned(), b"{}".to_vec())]);
    let staged = stage(dir.path(), "one", &files, true).expect("staged");
    let stage_path = staged.path().to_path_buf();
    std::os::unix::fs::symlink(&elsewhere, dir.path().join("one")).expect("a link meanwhile");

    let refused = staged.commit().expect_err("refused");

    assert!(matches!(refused, NotWritten::Failed(why) if why.contains("not a folder")));
    let link = std::fs::symlink_metadata(dir.path().join("one")).expect("still there");
    assert!(link.file_type().is_symlink(), "the link is put back");
    assert_eq!(
        std::fs::read_to_string(elsewhere.join("kept.txt")).expect("read"),
        "mine"
    );
    assert!(!stage_path.exists(), "the stage is gone");
}

#[test]
fn bytes_after_a_zip_s_end_refuse_it() {
    let mut zip = a_zip(&[("kept/theme.json", b"{}")]);
    zip.extend_from_slice(b"trailing bytes");

    let refused = unpack(&zip).expect_err("refused");

    assert!(refused.contains("does not end"), "{refused}");
}

#[test]
fn what_the_finder_adds_is_not_counted_against_the_entries_a_theme_folder_may_have() {
    // 64 files, a Finder shadow entry for each, and its folder file: past the theme's own cap
    // only when the Finder's entries are counted.
    let names: Vec<String> = (0..MOST_FILES - 1)
        .map(|at| format!("kept/icons/{at}.json"))
        .collect();
    let shadows: Vec<String> = names.iter().map(|it| format!("__MACOSX/{it}")).collect();
    let mut entries: Vec<(&str, &[u8])> =
        vec![("kept/theme.json", b"{}"), ("kept/.DS_Store", b"x")];
    entries.extend(names.iter().map(|it| (it.as_str(), &b"{}"[..])));
    entries.extend(shadows.iter().map(|it| (it.as_str(), &b"x"[..])));

    let (_, files) = unpack(&a_zip(&entries)).expect("unpacked");

    assert_eq!(files.len(), MOST_FILES);
}

// ------------------------------------------------------------------------------------------
// A staged import is read only to check (#2587)
// ------------------------------------------------------------------------------------------

/// What [`served`] answers for a digest no accepted theme registered.
const NEVER_REGISTERED: &str = "that is not a font of a theme purlis read";

/// A woff2 file no other test has: its digest is registered only by what this test reads.
fn a_font_of_its_own(tag: &str) -> Vec<u8> {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |it| it.as_nanos());
    format!(
        "wOF2 a staged font for {tag}, {} at {at}",
        std::process::id()
    )
    .into_bytes()
}

/// The digest [`served`] knows a font's bytes by.
fn digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A theme folder's files: a `theme.json` whose UI font is `fonts/ui.woff2`, holding `font`,
/// with `tokens` in its one appearance.
fn with_a_font(font: &[u8], tokens: &Value) -> Files {
    let theme = serde_json::json!({
        "name": "Inked",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }],
        "fonts": { "ui": { "family": "Inked Sans", "faces": [{ "file": "fonts/ui.woff2" }] } }
    });
    Files::from([
        (THEME_JSON.to_owned(), theme.to_string().into_bytes()),
        ("fonts/ui.woff2".to_owned(), font.to_vec()),
    ])
}

#[test]
fn a_staged_import_that_goes_in_place_leaves_no_font_registered_under_the_stage() {
    let dir = tempfile::tempdir().expect("a directory");
    let themes = dir.path().join("themes");
    let font = a_font_of_its_own("one that goes in place");
    let asset = digest(&font);

    let staged = stage(
        &themes,
        "inked",
        &with_a_font(&font, &serde_json::json!({})),
        false,
    )
    .expect("staged");
    let read = staged.read();
    assert!(!read.refused, "{:?}", read.complaints);
    assert_eq!(read.theme.first().fonts.ui.faces[0].asset, asset);
    assert_eq!(served(&asset), Err(NEVER_REGISTERED.to_owned()));
    let path = staged.commit().expect("in place");
    assert_eq!(served(&asset), Err(NEVER_REGISTERED.to_owned()));

    // The folder in place is a theme like any other: reading it to draw it serves its font, from
    // where it is now.
    let drawn = Library::new()
        .reading_named_fonts()
        .load_folder(&themes, "inked");
    assert!(!drawn.refused, "{:?}", drawn.complaints);
    assert_eq!(path, themes.join("inked"));
    assert_eq!(served(&asset), Ok(font));
}

#[test]
fn a_staged_import_that_fails_leaves_no_font_registered_under_the_stage() {
    let dir = tempfile::tempdir().expect("a directory");
    let themes = dir.path().join("themes");

    // The theme is drawn, but a folder of its name is there and --force was not given.
    let font = a_font_of_its_own("one whose folder is there");
    let asset = digest(&font);
    let staged = stage(
        &themes,
        "inked",
        &with_a_font(&font, &serde_json::json!({})),
        false,
    )
    .expect("staged");
    let stage_path = staged.path().to_path_buf();
    std::fs::create_dir_all(themes.join("inked")).expect("a folder there meanwhile");
    let read = staged.read();
    assert!(!read.refused, "{:?}", read.complaints);
    assert_eq!(served(&asset), Err(NEVER_REGISTERED.to_owned()));
    assert_eq!(
        staged.commit(),
        Err(NotWritten::Exists(themes.join("inked")))
    );
    assert!(!stage_path.exists());
    assert_eq!(served(&asset), Err(NEVER_REGISTERED.to_owned()));

    // The theme is refused, so nothing is put in place and the stage goes.
    let font = a_font_of_its_own("one the engine refuses");
    let asset = digest(&font);
    let staged = stage(
        &themes,
        "refused",
        &with_a_font(
            &font,
            &serde_json::json!({ "surface.base": "not a colour" }),
        ),
        false,
    )
    .expect("staged");
    let stage_path = staged.path().to_path_buf();
    assert!(staged.read().refused);
    drop(staged);
    assert!(!stage_path.exists());
    assert_eq!(served(&asset), Err(NEVER_REGISTERED.to_owned()));
}

// ------------------------------------------------------------------------------------------
// Names Windows cannot hold (#2588)
// ------------------------------------------------------------------------------------------

#[test]
fn every_device_name_and_a_name_ending_in_a_dot_or_a_space_is_not_held() {
    use portable::{NotHeld, not_held};
    let mut devices: Vec<String> = ["con", "prn", "aux", "nul"]
        .iter()
        .map(|it| (*it).to_owned())
        .collect();
    devices.extend(["conin$", "conout$"].map(str::to_owned));
    for n in 0..=9 {
        devices.push(format!("com{n}"));
        devices.push(format!("lpt{n}"));
    }
    devices.extend(["com¹", "lpt³"].map(str::to_owned));
    for device in &devices {
        for name in [
            device.clone(),
            device.to_ascii_uppercase(),
            format!("{device}.json"),
            format!("{}.tar.gz", device.to_ascii_uppercase()),
            format!("{device} .txt"),
        ] {
            assert_eq!(not_held(&name), Some(NotHeld::Device), "{name}");
        }
    }
    assert_eq!(not_held("LICENSE."), Some(NotHeld::Dot));
    assert_eq!(not_held("notes "), Some(NotHeld::Space));
    for held in [
        "console",
        "com10",
        "lpt01",
        "conin",
        "lpt",
        "nul-theme",
        "auxiliary",
        "icon.json",
        ".con",
    ] {
        assert_eq!(not_held(held), None, "{held}");
    }
    assert!(!is_folder_name("con"));
    assert!(!is_folder_name("lpt9"));
    assert!(is_folder_name("console"));
}

#[test]
fn a_zip_naming_what_windows_cannot_hold_is_refused_with_a_sentence() {
    let theme: &[u8] = b"{}";
    for (bad, said) in [
        ("kept/fonts/CON.woff2", "fonts/CON.woff2"),
        ("kept/aux.md", "aux.md"),
        ("kept/Nul", "Nul"),
        ("kept/icons/lpt1.json", "lpt1.json"),
        ("kept/LICENSE.", "LICENSE."),
        ("kept/notes ", "notes "),
    ] {
        let zip = a_zip(&[("kept/theme.json", theme), (bad, b"x")]);

        let refused = unpack(&zip).expect_err(bad);

        assert!(refused.contains("Windows"), "{bad}: {refused}");
        assert!(refused.contains(&format!("{bad} in the zip")), "{refused}");
        assert!(
            refused.contains(said.rsplit('/').next().unwrap_or(said)),
            "{refused}"
        );
        // One sentence: no full stop but the one a name may end in.
        let without_names = refused.replace(said, "");
        assert_eq!(
            without_names.matches(". ").count(),
            0,
            "one sentence: {refused}"
        );
    }
    let named = a_zip(&[("com3/theme.json", theme)]);
    let refused = unpack(&named).expect_err("a device as the folder");
    assert!(refused.contains("Windows keeps for a device"), "{refused}");
    let imported = import(SourceKind::Zip, "com3.zip", &named).expect_err("refused");
    assert!(imported.contains("Windows"), "{imported}");
}

#[test]
fn a_theme_whose_name_would_be_a_device_is_not_imported() {
    let named = br#"{ "name": "Aux", "colors": {} }"#;

    let refused = import(SourceKind::Vscode, "aux.json", named).expect_err("refused");

    assert!(
        refused.contains("aux, a name Windows keeps for a device"),
        "{refused}"
    );
}

#[test]
fn a_stage_or_an_export_of_what_windows_cannot_hold_is_refused_or_not_carried() {
    let dir = tempfile::tempdir().expect("a directory");
    let theme = Files::from([(THEME_JSON.to_owned(), b"{}".to_vec())]);
    let Err(NotWritten::Failed(why)) = stage(dir.path(), "nul", &theme, false) else {
        panic!("a device as the folder is staged");
    };
    assert!(why.contains("Windows"), "{why}");
    let mut with_a_device = theme.clone();
    with_a_device.insert("icons/prn.json".to_owned(), b"{}".to_vec());
    let Err(NotWritten::Failed(why)) = stage(dir.path(), "kept", &with_a_device, false) else {
        panic!("a device inside is staged");
    };
    assert!(why.contains("prn.json") && why.contains("Windows"), "{why}");

    let folder = a_folder(dir.path(), "kept");
    std::fs::write(folder.join("aux.txt"), "x").expect("a file");
    std::fs::write(folder.join("icons/con.json"), "{}").expect("a file");
    let packed = export(&folder).expect("exported");
    let left: Vec<(&str, bool)> = packed
        .left_out
        .iter()
        .map(|it| (it.key.as_str(), it.why.contains("Windows")))
        .collect();
    assert_eq!(left, [("aux.txt", true), ("icons/con.json", true)]);
    assert_eq!(packed.files, 4);

    let device = a_folder(dir.path(), "lpt2");
    let refused = export(&device).expect_err("a device as the folder");
    assert!(refused.contains("Windows"), "{refused}");
}

#[test]
fn the_folder_reader_refuses_a_device_name_and_a_font_or_icon_set_named_so() {
    let dir = tempfile::tempdir().expect("a directory");
    let themes = dir.path().join("themes");
    let refused = Library::new().load_folder(&themes, "aux");
    assert!(refused.refused);
    assert!(
        refused.complaints[0].said.contains("Windows"),
        "{refused:?}"
    );

    let font = a_font_of_its_own("a device font");
    let mut files = with_a_font(&font, &serde_json::json!({}));
    let mut theme: Value = serde_json::from_slice(&files[THEME_JSON]).expect("JSON");
    theme["fonts"]["ui"]["faces"][0]["file"] = Value::from("fonts/NUL.woff2");
    files.insert(THEME_JSON.to_owned(), theme.to_string().into_bytes());
    let folder = themes.join("inked");
    std::fs::create_dir_all(folder.join("fonts")).expect("a folder");
    for (path, bytes) in &files {
        std::fs::write(folder.join(path), bytes).expect("a file");
    }
    let read = Library::new()
        .reading_named_fonts()
        .load_folder(&themes, "inked");
    assert!(read.refused);
    let said: Vec<&str> = read.complaints.iter().map(|it| it.said.as_str()).collect();
    assert!(
        said.iter()
            .any(|it| it.contains("NUL.woff2") && it.contains("Windows")),
        "{said:?}"
    );

    theme.as_object_mut().expect("an object").remove("fonts");
    theme["icons"] = Value::from("icons/com1.json");
    std::fs::write(folder.join(THEME_JSON), theme.to_string()).expect("a theme");
    let read = Library::new()
        .reading_named_fonts()
        .load_folder(&themes, "inked");
    assert!(read.refused);
    assert!(
        read.complaints
            .iter()
            .any(|it| it.said.contains("com1.json") && it.said.contains("Windows")),
        "{:?}",
        read.complaints
    );
}

// ------------------------------------------------------------------------------------------
// The recipes (#2589)
// ------------------------------------------------------------------------------------------

/// Each shipped fixture, its kind and its file name.
const FIXTURES: &[(SourceKind, &str, &[u8])] = &[
    (SourceKind::Vscode, "ember.jsonc", EMBER),
    (SourceKind::Base16, "harbor.yaml", HARBOR),
    (SourceKind::Base24, "dune.json", DUNE),
    (SourceKind::Iterm, "Tide.itermcolors", TIDE),
    (SourceKind::Iterm, "both.itermcolors", TIDE_BOTH),
    (SourceKind::Ghostty, "moss", MOSS),
];

/// What the theme check finds in `theme`, as one line each, untinted and tinted alike.
fn findings(theme: &Theme) -> Vec<String> {
    check_every(theme)
        .into_iter()
        .flat_map(|checked| checked.report.findings)
        .map(|finding| match finding {
            Finding::Contrast { pair, .. } => format!("{} on {}", pair.front, pair.back),
            Finding::Ansi { token, .. } => format!("{token} on terminal.background"),
            other => format!("{other:?}"),
        })
        .collect()
}

/// Each token's value in the appearance the import wrote first, as `theme.json` holds it.
fn written(imported: &Imported, token: &str) -> Value {
    let document: Value = serde_json::from_slice(&imported.files[THEME_JSON]).expect("JSON");
    document["appearances"][0]["tokens"][token].clone()
}

#[test]
fn every_shipped_fixture_imports_with_no_finding_its_own_palette_could_avoid() {
    for (kind, name, bytes) in FIXTURES {
        let imported = import(*kind, name, bytes).expect("imported");
        let found = findings(&drawn(&imported));
        // A terminal's own sixteen colours are what it draws, and are taken as they are: the
        // findings left are those, in the theme as it is and in each workspace tint.
        let expected: &[&str] = match *name {
            "Tide.itermcolors" => &["terminal.ansi.black on terminal.background"],
            // A VS Code theme's comment is drawn as its author wrote it, on the diff's washes it
            // does not name (#4424): the check says so, and the import does not walk it.
            "ember.jsonc" => &[
                "syntax.comment on diff.deleted-text",
                "syntax.comment on diff.inserted-text",
            ],
            "moss" => &[
                "terminal.ansi.black on terminal.background",
                "terminal.ansi.bright-black on terminal.background",
            ],
            _ => &[],
        };
        let distinct: BTreeSet<&str> = found.iter().map(String::as_str).collect();
        assert_eq!(
            distinct,
            expected.iter().copied().collect::<BTreeSet<_>>(),
            "{name}"
        );
    }
}

#[test]
fn a_terminal_s_own_ansi_colours_are_taken_as_they_are() {
    let imported = import(SourceKind::Ghostty, "moss", MOSS).expect("imported");

    assert_eq!(written(&imported, "terminal.ansi.black"), "palette-0");
    assert_eq!(
        written(&imported, "terminal.ansi.bright-black"),
        "palette-8"
    );
}

#[test]
fn base16_s_row_under_the_pointer_is_halfway_between_base01_and_base02() {
    let imported = import(SourceKind::Base16, "harbor.yaml", HARBOR).expect("imported");

    let dark = drawn(&imported);
    let dark = dark.first();
    // base01 #232a2f and base02 #2e373d, mixed half and half and rounded as the screen does.
    assert_eq!(dark.value("surface.hover"), "#293136");
    assert_eq!(written(&imported, "list.selected"), "base02");
    assert_ne!(dark.value("surface.hover"), dark.value("list.selected"));
    assert_ne!(dark.value("surface.hover"), dark.value("surface.raised"));
}

#[test]
fn selected_text_is_the_foreground_when_the_source_colours_only_the_selection() {
    let harbor = import(SourceKind::Base16, "harbor.yaml", HARBOR).expect("imported");
    assert_eq!(written(&harbor, "selection.text"), "base05");
    assert!(
        !harbor.left_out.iter().any(|it| it.key == "base05"),
        "base05 gave a token its value"
    );

    let tide = import(SourceKind::Iterm, "Tide.itermcolors", TIDE).expect("imported");
    assert_eq!(written(&tide, "selection.text"), "foreground");

    let ember = import(SourceKind::Vscode, "ember.jsonc", EMBER).expect("imported");
    assert_eq!(
        written(&ember, "selection.text"),
        "#e8e2d9",
        "editor.foreground"
    );

    // Selected text the source colours itself is its own.
    let moss = import(SourceKind::Ghostty, "moss", MOSS).expect("imported");
    assert_eq!(written(&moss, "selection.text"), "selection-foreground");
}

#[test]
fn selected_text_is_left_to_the_platform_with_the_selection() {
    let source = b"background = 1a201c\nforeground = d6ddd6\npalette = 4=#6fa3c8\n";
    let imported = import(SourceKind::Ghostty, "plain", source).expect("imported");

    assert_eq!(written(&imported, "selection.text"), Value::Null);
    let found = findings(&drawn(&imported));
    assert!(
        !found.iter().any(|it| it == "SelectionTextAlone"),
        "{found:?}"
    );
}

/// The point a [`Recipe::Toward`] walk from `from` toward `to` is at after `step` tenths.
fn tenth(from: &str, to: &str, step: u32) -> String {
    check::mix(to, from, f64::from(step) / f64::from(TOWARD_STEPS))
}

#[test]
fn a_walk_takes_the_nearest_tenth_that_reads_and_no_further() {
    let imported = import(SourceKind::Base16, "harbor.yaml", HARBOR).expect("imported");
    let theme = drawn(&imported);
    let dark = theme.first();

    // base03 #5c6a73 is muted words at 2.94 on the window: the walk goes on to base04 #8a9aa4
    // and from there toward base05 #c7d3da, and stops at the first tenth every pair clears.
    let base04_reached = (1..=TOWARD_STEPS)
        .map(|step| tenth("#8a9aa4", "#c7d3da", step))
        .collect::<Vec<_>>();
    let muted = dark.value("text.muted");
    let at = base04_reached
        .iter()
        .position(|it| it == muted)
        .expect("text.muted is on the way from base04 to base05");
    // Read as the check reads it: as it is and in every workspace tint.
    let clears = |theme: &ResolvedTheme| {
        !check(theme).findings.iter().any(|finding| {
            matches!(finding, Finding::Contrast { pair, .. } if pair.front == "text.muted")
        })
    };
    assert!(clears(dark));
    let mut nearer = dark.clone();
    let before = if at == 0 {
        "#8a9aa4".to_owned()
    } else {
        base04_reached[at - 1].clone()
    };
    nearer.values.insert("text.muted", before);
    assert!(!clears(&nearer), "a nearer tenth would have read too");
}

#[test]
fn base16_s_ansi_black_takes_the_nearest_step_up_the_scale_the_terminal_shows() {
    let imported = import(SourceKind::Base16, "harbor.yaml", HARBOR).expect("imported");
    let theme = drawn(&imported);
    let dark = theme.first();

    let black = dark.value("terminal.ansi.black");
    assert_ne!(black, dark.value("terminal.background"));
    // From base00 to base01 to base02 nothing clears 1.5; a tenth of the way on to base03 does.
    assert_eq!(black, tenth("#2e373d", "#5c6a73", 2));
    assert!(check::contrast(black, "#1b2024") >= 1.5);
    assert!(check::contrast(&tenth("#2e373d", "#5c6a73", 1), "#1b2024") < 1.5);
}

#[test]
fn a_hue_that_reads_already_stays_the_source_s_palette_colour() {
    let imported = import(SourceKind::Base16, "harbor.yaml", HARBOR).expect("imported");

    assert_eq!(written(&imported, "accent.base"), "base0d");
    assert_eq!(written(&imported, "syntax.keyword"), "base0e");
    assert_eq!(written(&imported, "terminal.ansi.red"), "base08");
}

#[test]
fn a_light_scheme_s_hues_walk_toward_its_darker_words() {
    let imported = import(SourceKind::Base24, "dune.json", DUNE).expect("imported");
    let theme = drawn(&imported);
    let light = theme.first();

    // base0B #56802c is markdown code at 4.13 on the window, under 4.5: a tenth or more of the
    // way toward base05 #3d382e reads.
    let code = light.value("markdown.code");
    assert!(
        (1..=TOWARD_STEPS).any(|step| tenth("#56802c", "#3d382e", step) == code),
        "{code}"
    );
    assert!(check::contrast(code, light.value("surface.base")) >= 4.5);
    assert!(
        check::contrast(code, "#ffffff") > check::contrast("#56802c", "#ffffff"),
        "darker than base0B"
    );
}

#[test]
fn every_import_is_drawn_through_the_consent_floors() {
    // A scheme whose every colour is one grey: no step on its scale reads, and the window
    // still draws purlis's own values where a consent floor breaks.
    let mut grey = String::from("scheme: \"Fog\"\n");
    for key in [
        "00", "01", "02", "03", "04", "05", "06", "07", "08", "09", "0A", "0B", "0C", "0D", "0E",
        "0F",
    ] {
        grey.push_str(&format!("base{key}: \"808080\"\n"));
    }
    let imported = import(SourceKind::Base16, "fog.yaml", grey.as_bytes()).expect("imported");
    let theme = drawn(&imported);

    let held = draw(theme.first()).held.expect("the floors hold");
    assert!(held.floors.contains(&Floor::Ask), "{held:?}");
    assert!(
        check_every(&theme).iter().all(|it| it.held.is_some()),
        "each combination says what it holds"
    );

    // A VS Code theme's colours are taken as they are: Ember's accent is too near purlis's own
    // danger red, which it does not name, and the window draws purlis's own accent there.
    let ember = import(SourceKind::Vscode, "ember.jsonc", EMBER).expect("imported");
    let ember = drawn(&ember);
    let shown = draw(ember.first());
    let held = shown.held.expect("the floors hold");
    // Its warm accent is near purlis's waiting amber too (#3130).
    assert_eq!(held.floors, [Floor::DangerApart, Floor::WaitingApart]);
    assert_eq!(
        shown.theme.expect("drawn").value("accent.base"),
        built_in_for(Appearance::Dark).value("accent.base")
    );
}

#[test]
fn each_kind_s_recipes_tune_only_purlis_tokens_and_the_docs_print_them() {
    let docs = include_str!("../../docs/themes.md");
    for kind in SourceKind::ALL.iter().filter(|it| **it != SourceKind::Zip) {
        for (token, recipe) in recipes(*kind) {
            assert!(vocabulary::is_token(token), "{token} in {kind:?}");
            if let Recipe::Fill { when, .. } = recipe {
                assert!(vocabulary::is_token(when), "{when}");
            }
        }
        assert!(
            docs.contains(&recipes_markdown(*kind)),
            "docs/themes.md does not print the {} recipes as the engine has them:\n{}",
            kind.name(),
            recipes_markdown(*kind)
        );
    }
    assert!(recipes(SourceKind::Zip).is_empty());
}

/// How far apart two hues must stay after tuning, in OKLab: the least a person can see.
const SEEN_APART: f64 = 0.02;

#[test]
fn the_state_and_icon_hues_stay_apart_from_each_other_after_tuning() {
    const STATES: &[&str] = &[
        "state.running",
        "state.waiting",
        "state.failed",
        "state.success",
        "state.unreadable",
    ];
    const ICONS: &[&str] = &[
        "icon.red",
        "icon.orange",
        "icon.yellow",
        "icon.green",
        "icon.teal",
        "icon.blue",
        "icon.purple",
        "icon.pink",
    ];
    for (kind, name, bytes) in FIXTURES {
        let imported = import(*kind, name, bytes).expect("imported");
        let theme = drawn(&imported);
        let rows = table(*kind);
        // The source key a token's row takes first, among those the source has: two tokens the
        // source gives one colour (danger and failed are both red) are not held apart.
        let key_of = |token: &str| {
            rows.iter()
                .find(|(it, _)| *it == token)
                .and_then(|(_, keys)| keys.first().copied())
        };
        for one in &theme.appearances {
            for group in [STATES, ICONS] {
                for (at, a) in group.iter().enumerate() {
                    for b in &group[at + 1..] {
                        let (Some(key_a), Some(key_b)) = (key_of(a), key_of(b)) else {
                            continue;
                        };
                        if key_a == key_b {
                            continue;
                        }
                        let apart = tint::distance(
                            tint::oklab_of(one.value(a)).expect("a colour"),
                            tint::oklab_of(one.value(b)).expect("a colour"),
                        );
                        assert!(
                            apart >= SEEN_APART,
                            "{name} {}: {a} {} and {b} {} are {apart:.3} apart",
                            one.id,
                            one.value(a),
                            one.value(b)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn a_terminal_colour_s_warning_names_the_palette_entry_that_holds_it() {
    let imported = import(SourceKind::Ghostty, "moss", MOSS).expect("imported");
    let theme = drawn(&imported);
    let checked = check_every(&theme);

    assert_eq!(
        imported
            .palette_entry("dark", "terminal.ansi.black")
            .as_deref(),
        Some("palette-0")
    );
    assert_eq!(
        imported.palette_entry("dark", "text.muted"),
        None,
        "walked: hex"
    );
    assert_eq!(imported.palette_entry("light", "terminal.ansi.black"), None);
    let said = warnings_said_naming(&theme, &checked, |appearance, token| {
        imported.palette_entry(appearance, token)
    });
    let black = said
        .iter()
        .find(|line| line.starts_with("terminal.ansi.black on terminal.background"))
        .expect("a warning");
    assert!(
        black.ends_with(
            "; it is the palette's `palette-0`: change `palette-0` in theme.json to lift it"
        ),
        "{black}"
    );
    // Without an entry, the line is the check's own.
    assert!(
        warnings_said(&theme, &checked)
            .iter()
            .all(|line| !line.contains("palette-0")),
        "{said:?}"
    );
}
