// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project's theme (charter-app#273, ADR 0048): Local over Shared, and an extension's theme
//! only while that extension is on in the project — each expected answer written out.

use super::*;
use crate::extension::project::{Choices, Installed, resolve as extensions};

fn approved(id: &str) -> Installed {
    Installed {
        id: id.to_owned(),
        approved: true,
        name: id.to_owned(),
        settings: Vec::new(),
    }
}

/// The theme for a project whose two files are `shared` and `local`, on a machine with
/// `installed`, where the approved extensions provide `offered`.
fn theme(
    installed: &[Installed],
    offered: Option<&[Offered]>,
    shared: &str,
    local: &str,
) -> Resolved {
    let on = extensions(installed, &Choices::from_text(Some(shared), Some(local)));
    // The project has no theme folders: a folder's name reaches none (#2323).
    let said = Said::from_text(Some(shared), Some(local)).with_folders(Vec::new(), Vec::new());
    resolve(&on, offered, &said)
}

#[test]
fn a_project_that_picks_nothing_leaves_the_window_its_own_theme() {
    let got = theme(&[], None, "", "");
    assert_eq!(got.draws, None);
    assert_eq!(got.source, Source::Default);
    assert_eq!(got.why, None);
}

#[test]
fn local_overrides_shared() {
    let got = theme(
        &[],
        None,
        "[theme]\nuse = \"purlis-light\"\n",
        "[theme]\nuse = \"system\"\n",
    );
    assert_eq!((got.draws, got.source), (Some(Pick::System), Source::Local));

    let got = theme(&[], None, "[theme]\nuse = \"purlis-light\"\n", "");
    assert_eq!(
        (got.draws, got.source),
        (Some(Pick::BuiltIn("purlis-light")), Source::Shared)
    );
}

/// **A saved old id draws the renamed theme** (#1838): a project file, a Local file or a workspace
/// that still names a built-in by the id it had before the rename picks that built-in, by its
/// purlis id, with nothing ignored. The old ids come from the name module, the one place that
/// spells them.
#[test]
fn an_old_built_in_id_is_read_as_the_renamed_theme() {
    use crate::names::{THEME_DARK, THEME_LIGHT};
    for renamed in [THEME_DARK, THEME_LIGHT] {
        for old in renamed.reads.iter().chain(renamed.history) {
            let picks = format!("[theme]\nuse = \"{old}\"\n");
            for (shared, local, source) in [
                (picks.as_str(), "", Source::Shared),
                ("", picks.as_str(), Source::Local),
            ] {
                let got = theme(&[], None, shared, local);
                assert_eq!(
                    (got.draws, got.picked.clone(), got.source),
                    (
                        Some(Pick::BuiltIn(renamed.write)),
                        Some(Pick::BuiltIn(renamed.write)),
                        source
                    ),
                    "{old} in the {source:?} file"
                );
                assert!(got.ignored.is_empty(), "{old}: {:?}", got.ignored);
                assert_eq!(got.why, None);
            }
            let manifest = format!(r#"{{"settings": {{"theme": {{"use": "{old}"}}}}}}"#);
            let got = in_alpha(&[], None, "", &manifest, "");
            assert_eq!(
                (got.draws, got.source),
                (Some(Pick::BuiltIn(renamed.write)), Source::Workspace)
            );
            assert_eq!(
                Pick::parse(old).map(|pick| pick.value()),
                Some(renamed.write.to_owned())
            );
        }
    }
    assert_eq!(FALLBACK, crate::theme::PURLIS);
}

// -------------------------------------------------------------------------------------
// An extension's theme, and when it falls back
// -------------------------------------------------------------------------------------

const PICKS_SOLARIZED: &str = "[theme]\nuse = \"solarized/Solarized Dark\"\n";

fn solarized() -> Pick {
    Pick::Extension {
        id: "solarized".to_owned(),
        name: "Solarized Dark".to_owned(),
    }
}

fn offered() -> Vec<Offered> {
    vec![Offered {
        id: "solarized".to_owned(),
        name: "Solarized Dark".to_owned(),
    }]
}

#[test]
fn an_extension_theme_is_drawn_while_its_extension_is_on_in_the_project() {
    let got = theme(
        &[approved("solarized")],
        Some(&offered()),
        PICKS_SOLARIZED,
        "",
    );
    assert_eq!(got.picked, Some(solarized()));
    assert_eq!(got.draws, Some(solarized()));
    assert_eq!(got.why, None);
}

#[test]
fn an_extension_theme_whose_extension_the_project_turned_off_falls_back_with_a_reason() {
    let got = theme(
        &[approved("solarized")],
        Some(&offered()),
        PICKS_SOLARIZED,
        "[extensions.solarized]\nenabled = false\n",
    );
    assert_eq!(got.picked, Some(solarized()));
    assert_eq!(got.draws, Some(Pick::BuiltIn("purlis")));
    assert_eq!(
        got.why.as_deref(),
        Some(
            "charter.toml picks “Solarized Dark” from solarized, but solarized is off in this \
             project — so the built-in theme, Purlis, is drawn"
        )
    );
}

#[test]
fn an_extension_theme_this_machine_has_not_approved_falls_back_with_a_reason() {
    let unapproved = Installed {
        approved: false,
        ..approved("solarized")
    };
    let got = theme(&[unapproved], None, "", PICKS_SOLARIZED);
    assert_eq!(got.source, Source::Local);
    assert_eq!(got.draws, Some(Pick::BuiltIn("purlis")));
    assert_eq!(
        got.why.as_deref(),
        Some(
            "charter.local.toml picks “Solarized Dark” from solarized, but this machine has not \
             approved solarized — so the built-in theme, Purlis, is drawn"
        )
    );
}

#[test]
fn an_extension_theme_from_an_extension_not_installed_falls_back_with_a_reason() {
    let got = theme(&[], None, PICKS_SOLARIZED, "");
    assert_eq!(got.draws, Some(Pick::BuiltIn("purlis")));
    assert_eq!(
        got.why.as_deref(),
        Some(
            "charter.toml picks “Solarized Dark” from solarized, but solarized is not installed \
             on this machine — so the built-in theme, Purlis, is drawn"
        )
    );
}

#[test]
fn a_theme_the_extension_does_not_contribute_falls_back_when_the_survey_says_so() {
    let got = theme(
        &[approved("solarized")],
        Some(&offered()),
        "[theme]\nuse = \"solarized/Solarized Light\"\n",
        "",
    );
    assert_eq!(got.draws, Some(Pick::BuiltIn("purlis")));
    assert_eq!(
        got.why.as_deref(),
        Some(
            "charter.toml picks “Solarized Light” from solarized, but solarized contributes no \
             theme called “Solarized Light” — so the built-in theme, Purlis, is drawn"
        )
    );
    // Asked without a survey, the pick stands: the window only holds what a survey found.
    let cheap = theme(
        &[approved("solarized")],
        None,
        "[theme]\nuse = \"solarized/Solarized Light\"\n",
        "",
    );
    assert_eq!(cheap.why, None);
}

#[test]
fn local_turning_the_extension_back_on_brings_shareds_pick_back() {
    let got = theme(
        &[approved("solarized")],
        Some(&offered()),
        "[theme]\nuse = \"solarized/Solarized Dark\"\n[extensions.solarized]\nenabled = false\n",
        "[extensions.solarized]\nenabled = true\n",
    );
    assert_eq!((got.draws, got.source), (Some(solarized()), Source::Shared));
}

// -------------------------------------------------------------------------------------
// What is not a pick
// -------------------------------------------------------------------------------------

#[test]
fn a_value_that_is_not_a_pick_is_ignored_and_the_next_file_down_is_used() {
    let got = theme(
        &[],
        None,
        "[theme]\nuse = \"purlis-light\"\n",
        "[theme]\nuse = \"purple\"\n",
    );
    assert_eq!(
        (got.draws, got.source),
        (Some(Pick::BuiltIn("purlis-light")), Source::Shared)
    );
    assert_eq!(
        got.ignored,
        [Ignored {
            source: Source::Local,
            why: "charter.local.toml sets theme.use to \"purple\", which names no theme folder \
                  this layer reaches — so charter.toml's pick is used"
                .to_owned(),
        }]
    );
}

#[test]
fn a_value_that_is_not_text_leaves_the_window_its_own_theme_when_nothing_else_picks() {
    let got = theme(&[], None, "[theme]\nuse = 3\n", "");
    assert_eq!((got.draws, got.source), (None, Source::Default));
    assert_eq!(
        got.ignored[0].why,
        "charter.toml sets theme.use to 3, which is not purlis-dark, purlis-light, system, a theme \
         folder's name or <extension>/<theme> — so the window keeps its own theme"
    );
}

#[test]
fn a_pick_is_read_back_as_it_was_written() {
    for value in [
        "purlis-dark",
        "purlis-light",
        "system",
        "solarized/Solarized Dark",
    ] {
        assert_eq!(
            Pick::parse(value).map(|pick| pick.value()).as_deref(),
            Some(value)
        );
    }
    // A theme folder, by its folder's name (#2323).
    assert_eq!(Pick::parse("dusk"), Some(Pick::Folder("dusk".to_owned())));
    for value in [
        "",
        "Dark",
        "/Solarized",
        "solarized/",
        "../x/y",
        "solarized/  ",
    ] {
        assert_eq!(Pick::parse(value), None, "{value:?}");
    }
}

/// #1145: a pick names a theme by the rule an extension's manifest holds a theme's name to, so a
/// name no extension can contribute is not a pick, for the colour theme and the icon theme.
#[test]
fn a_pick_whose_theme_name_purlis_would_not_draw_is_not_a_pick() {
    let longest = format!("solarized/{}", "L".repeat(40));
    assert!(Pick::parse(&longest).is_some());
    assert!(Pick::parse_icons(&longest).is_some());
    for value in [
        format!("solarized/{}", "L".repeat(41)),
        "solarized/Dark\u{202e}".to_owned(),
        "solarized/Dark\u{200b}".to_owned(),
        "solarized/Da\u{7}rk".to_owned(),
    ] {
        assert_eq!(Pick::parse(&value), None, "{value:?}");
        assert_eq!(Pick::parse_icons(&value), None, "{value:?}");
    }
}

// -------------------------------------------------------------------------------------
// A workspace's theme and colour (charter-app#281)
// -------------------------------------------------------------------------------------

/// The theme in the workspace `alpha`, whose `workspace.json` is `manifest`, of a project whose
/// two files are `shared` and `local` — each layer read by the one resolver.
fn in_alpha(
    installed: &[Installed],
    offered: Option<&[Offered]>,
    shared: &str,
    manifest: &str,
    local: &str,
) -> Resolved {
    let choices =
        Choices::from_text(Some(shared), Some(local)).in_workspace("alpha", Some(manifest));
    let on = extensions(installed, &choices);
    resolve(
        &on,
        offered,
        &Said::from_text(Some(shared), Some(local))
            .in_workspace("alpha", Some(manifest))
            .with_folders(Vec::new(), Vec::new()),
    )
}

const ALPHA_LIGHT: &str = r#"{"settings": {"theme": {"use": "purlis-light"}}}"#;

#[test]
fn a_workspaces_pick_wins_over_shared_and_over_the_local_file() {
    let got = in_alpha(&[], None, "[theme]\nuse = \"system\"\n", ALPHA_LIGHT, "");
    assert_eq!(
        (got.draws, got.source),
        (Some(Pick::BuiltIn("purlis-light")), Source::Workspace)
    );

    let got = in_alpha(
        &[],
        None,
        "[theme]\nuse = \"system\"\n",
        ALPHA_LIGHT,
        "[theme]\nuse = \"purlis-dark\"\n",
    );
    // The theme levels' order (#2323): the workspace's pick comes before the local file's.
    assert_eq!(
        (got.draws, got.source),
        (Some(Pick::BuiltIn("purlis-light")), Source::Workspace)
    );
}

#[test]
fn a_workspace_that_picks_nothing_leaves_the_projects_pick() {
    let got = in_alpha(
        &[],
        None,
        "[theme]\nuse = \"system\"\n",
        r#"{"name": "alpha"}"#,
        "",
    );
    assert_eq!(
        (got.draws, got.source),
        (Some(Pick::System), Source::Shared)
    );
    assert_eq!(got.colour, None);
}

#[test]
fn a_workspaces_extension_theme_falls_back_when_the_workspace_turned_that_extension_off() {
    let got = in_alpha(
        &[approved("solarized")],
        Some(&offered()),
        "",
        r#"{"settings": {"theme": {"use": "solarized/Solarized Dark"},
            "extensions": {"solarized": {"enabled": false}}}}"#,
        "",
    );
    assert_eq!(got.draws, Some(Pick::BuiltIn("purlis")));
    assert_eq!(
        got.why.as_deref(),
        Some(
            "workspaces/alpha/workspace.json picks “Solarized Dark” from solarized, but \
             solarized is off in this workspace — so the built-in theme, Purlis, is drawn"
        )
    );
    // And the project's pick, when the workspace turned its extension off, says the same.
    let got = in_alpha(
        &[approved("solarized")],
        Some(&offered()),
        PICKS_SOLARIZED,
        r#"{"settings": {"extensions": {"solarized": {"enabled": false}}}}"#,
        "",
    );
    assert_eq!(
        got.why.as_deref(),
        Some(
            "charter.toml picks “Solarized Dark” from solarized, but solarized is off in this \
             workspace — so the built-in theme, Purlis, is drawn"
        )
    );
}

#[test]
fn a_workspace_value_that_is_not_a_pick_is_named_by_its_path_in_workspace_json() {
    let got = in_alpha(
        &[],
        None,
        "[theme]\nuse = \"purlis-light\"\n",
        r#"{"settings": {"theme": {"use": "dark"}}}"#,
        "",
    );
    assert_eq!(
        (got.draws, got.source),
        (Some(Pick::BuiltIn("purlis-light")), Source::Shared)
    );
    assert_eq!(
        got.ignored,
        [Ignored {
            source: Source::Workspace,
            why: "workspaces/alpha/workspace.json sets settings.theme.use to \"dark\", which \
                  names no theme folder this layer reaches — so charter.toml's pick is used"
                .to_owned(),
        }]
    );
}

#[test]
fn a_workspaces_colour_is_a_palette_name_or_a_hex_colour() {
    for (written, colour) in [
        ("teal", Colour::Palette("teal")),
        ("#3fa0c0", Colour::Custom("#3fa0c0".to_owned())),
        ("#3FA0C0", Colour::Custom("#3FA0C0".to_owned())),
    ] {
        let manifest = format!(r#"{{"settings": {{"theme": {{"colour": "{written}"}}}}}}"#);
        let got = in_alpha(&[], None, "", &manifest, "");
        assert_eq!(got.colour, Some(colour.clone()), "{written}");
        assert_eq!(colour.value(), written);
        // A colour picks no theme: the window keeps the one the project draws.
        assert_eq!(got.draws, None);
        assert!(got.ignored.is_empty(), "{:?}", got.ignored);
    }
    assert_eq!(PALETTE.len(), 8);
}

#[test]
fn a_colour_that_is_neither_is_ignored_with_a_reason_and_the_workspace_has_none() {
    for written in [r#""mauve""#, r##""#fff""##, "200"] {
        let manifest = format!(r#"{{"settings": {{"theme": {{"colour": {written}}}}}}}"#);
        let got = in_alpha(&[], None, "", &manifest, "");
        assert_eq!(got.colour, None, "{written}");
        assert_eq!(
            got.ignored,
            [Ignored {
                source: Source::Workspace,
                why: format!(
                    "workspaces/alpha/workspace.json sets settings.theme.colour to {written}, \
                     which is not red, orange, yellow, green, teal, blue, purple, pink or \
                     #rrggbb — so this workspace has no colour"
                ),
            }]
        );
    }
}

#[test]
fn only_a_workspace_has_a_colour() {
    // A project's files pick a theme and nothing else: `[theme] colour` there is refused, not
    // read (charter-app#281 — the colour is what tells one workspace from another).
    let got = theme(&[], None, "[theme]\ncolour = \"teal\"\n", "");
    assert_eq!(got.colour, None);
    assert_eq!(
        refusals("[theme]\ncolour = \"teal\"\n", "charter.toml"),
        [
            "theme.colour in charter.toml is not read — [theme] holds use, appearance, options \
          and icons and nothing else; a colour is a workspace's"
        ]
    );
}

#[test]
fn a_workspaces_theme_table_is_refused_in_the_readers_words() {
    let top: toml::Table =
        toml::from_str("[theme]\nuse = \"Dark!\"\ncolour = \"mauve\"\nfont = \"x\"\n").unwrap();
    assert_eq!(
        refusals_in_workspace(&top, "workspaces/alpha/workspace.json"),
        [
            "settings.theme.use in workspaces/alpha/workspace.json is \"Dark!\", which is not \
             purlis-dark, purlis-light, system, a theme folder's name or <extension>/<theme>",
            "settings.theme.colour in workspaces/alpha/workspace.json is \"mauve\", which is not \
             red, orange, yellow, green, teal, blue, purple, pink or #rrggbb",
            "settings.theme.font in workspaces/alpha/workspace.json is not read — a workspace's \
             theme holds use, appearance, options, icons and colour and nothing else",
        ]
    );
    let ok: toml::Table =
        toml::from_str("[theme]\nuse = \"system\"\ncolour = \"#00aa11\"\n").unwrap();
    assert!(refusals_in_workspace(&ok, "workspaces/alpha/workspace.json").is_empty());
    let not_a_table: toml::Table = toml::from_str("theme = \"teal\"\n").unwrap();
    assert_eq!(
        refusals_in_workspace(&not_a_table, "workspaces/alpha/workspace.json"),
        [
            "settings.theme in workspaces/alpha/workspace.json is not an object — write \
          {\"use\": \"<theme>\", \"colour\": \"<colour>\"}"
        ]
    );
}

#[test]
fn a_workspaces_theme_is_read_from_its_manifest_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "schema = 1\n").unwrap();
    let ws = dir.path().join("workspaces/alpha");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(
        ws.join("workspace.json"),
        r#"{"name": "alpha", "settings": {"theme": {"use": "purlis-light", "colour": "pink"}}}"#,
    )
    .unwrap();

    let said = Said::read_in(dir.path(), Some("alpha"));
    let got = resolve(&[], None, &said);
    assert_eq!(
        (got.draws, got.colour),
        (
            Some(Pick::BuiltIn("purlis-light")),
            Some(Colour::Palette("pink"))
        )
    );
    assert_eq!(
        said.workspace_file().as_deref(),
        Some("workspaces/alpha/workspace.json")
    );
    let alpha = crate::workspaces::Plane::open(dir.path())
        .workspace("alpha")
        .unwrap();
    assert_eq!(colour_of(&alpha), Some(Colour::Palette("pink")));
    // Outside every workspace there is no workspace layer.
    let got = resolve(&[], None, &Said::read_in(dir.path(), None));
    assert_eq!((got.draws, got.colour), (None, None));
}

// -------------------------------------------------------------------------------------
// The icon theme (FM-3, #1106): `[theme] icons`, in the same order as `use`
// -------------------------------------------------------------------------------------

/// The icon theme for a project whose two files are `shared` and `local`.
fn icons(
    installed: &[Installed],
    offered: Option<&[Offered]>,
    shared: &str,
    local: &str,
) -> Resolved {
    let on = extensions(installed, &Choices::from_text(Some(shared), Some(local)));
    resolve_icons(&on, offered, &Said::from_text(Some(shared), Some(local)))
}

fn seti() -> Pick {
    Pick::Extension {
        id: "seti".to_owned(),
        name: "Seti".to_owned(),
    }
}

const PICKS_SETI: &str = "[theme]\nicons = \"seti/Seti\"\n";

#[test]
fn a_project_that_picks_no_icon_theme_draws_purliss_own() {
    let got = icons(&[], None, "[theme]\nuse = \"purlis-light\"\n", "");
    assert_eq!((got.picked, got.draws), (None, None));
    assert_eq!(got.source, Source::Default);
    assert_eq!(ICONS_FALLBACK, "purlis-icons");
    assert_eq!(ICONS_FALLBACK, crate::names::ICON_THEME.write);
}

/// **A saved old icon theme id draws the renamed icon theme** (#1854), as an old colour theme id
/// does (#1838): a project file, a Local file or a workspace that still names the built-in icon
/// theme by its id from before the rename picks it, by its purlis id, with nothing ignored and
/// no refusal. The old ids come from the name module, the one place that spells them.
#[test]
fn an_old_built_in_icon_theme_id_is_read_as_the_renamed_icon_theme() {
    use crate::names::ICON_THEME;
    let renamed = Pick::BuiltIn(ICON_THEME.write);
    assert!(!ICON_THEME.reads.is_empty());
    for old in ICON_THEME.reads.iter().chain(ICON_THEME.history) {
        let picks = format!("[theme]\nicons = \"{old}\"\n");
        for (shared, local, source) in [
            (picks.as_str(), "", Source::Shared),
            ("", picks.as_str(), Source::Local),
        ] {
            let got = icons(&[], None, shared, local);
            assert_eq!(
                (got.draws, got.picked.clone(), got.source),
                (Some(renamed.clone()), Some(renamed.clone()), source),
                "{old} in the {source:?} file"
            );
            assert!(got.ignored.is_empty(), "{old}: {:?}", got.ignored);
            assert_eq!(got.why, None);
        }
        let on = extensions(
            &[],
            &Choices::from_text(Some(""), Some("")).in_workspace("alpha", Some("{}")),
        );
        let manifest = format!(r#"{{"settings": {{"theme": {{"icons": "{old}"}}}}}}"#);
        let said = Said::from_text(Some(""), Some("")).in_workspace("alpha", Some(&manifest));
        let got = resolve_icons(&on, None, &said);
        assert_eq!(
            (got.draws, got.source),
            (Some(renamed.clone()), Source::Workspace)
        );
        assert_eq!(
            Pick::parse_icons(old).map(|pick| pick.value()),
            Some(ICON_THEME.write.to_owned())
        );
        // An old icon theme id is an icon theme, never a colour theme.
        assert_eq!(Pick::parse(old), None);
        assert!(refusals(&picks, "charter.toml").is_empty(), "{old}");
    }
}

#[test]
fn the_icon_theme_is_picked_local_over_workspace_over_shared_apart_from_the_colours() {
    let got = icons(
        &[approved("seti")],
        None,
        "[theme]\nuse = \"purlis-light\"\nicons = \"purlis-icons\"\n",
        PICKS_SETI,
    );
    assert_eq!((got.draws, got.source), (Some(seti()), Source::Local));

    let on = extensions(
        &[approved("seti")],
        &Choices::from_text(Some(""), Some("")).in_workspace("alpha", Some("{}")),
    );
    let said = Said::from_text(Some(PICKS_SETI), Some("")).in_workspace(
        "alpha",
        Some(r#"{"settings": {"theme": {"icons": "purlis-icons"}}}"#),
    );
    let got = resolve_icons(&on, None, &said);
    assert_eq!(
        (got.draws, got.source),
        (Some(Pick::BuiltIn("purlis-icons")), Source::Workspace)
    );
    // The colour theme is its own key: picking icons picks no colours.
    assert_eq!(resolve(&on, None, &said).picked, None);
}

#[test]
fn an_extensions_icon_theme_is_drawn_only_while_it_is_on_and_contributes_it() {
    let offered = [Offered {
        id: "seti".to_owned(),
        name: "Seti".to_owned(),
    }];
    let got = icons(&[approved("seti")], Some(&offered), PICKS_SETI, "");
    assert_eq!((got.draws, got.why), (Some(seti()), None));

    let got = icons(
        &[approved("seti")],
        Some(&offered),
        PICKS_SETI,
        "[extensions.seti]\nenabled = false\n",
    );
    assert_eq!(got.draws, Some(Pick::BuiltIn("purlis-icons")));
    assert_eq!(
        got.why.as_deref(),
        Some(
            "charter.toml picks “Seti” from seti, but seti is off in this project — so the \
             built-in purlis-icons is drawn"
        )
    );

    let got = icons(&[approved("seti")], Some(&[]), PICKS_SETI, "");
    assert_eq!(
        got.why.as_deref(),
        Some(
            "charter.toml picks “Seti” from seti, but seti contributes no icon theme called \
             “Seti” — so the built-in purlis-icons is drawn"
        )
    );
}

#[test]
fn a_colour_theme_is_not_an_icon_theme() {
    let got = icons(&[], None, "[theme]\nicons = \"purlis-dark\"\n", "");
    assert_eq!(got.draws, None);
    assert_eq!(
        got.ignored
            .iter()
            .map(|it| it.why.as_str())
            .collect::<Vec<_>>(),
        [
            "charter.toml sets theme.icons to \"purlis-dark\", which is not purlis-icons or \
          <extension>/<icon theme> — so purlis's own icons are drawn"
        ]
    );
}

#[test]
fn a_projects_theme_table_holds_icons_too() {
    assert!(
        refusals(
            "[theme]\nuse = \"system\"\nicons = \"seti/Seti\"\n",
            "charter.toml"
        )
        .is_empty()
    );
    assert_eq!(
        refusals("[theme]\nicons = \"system\"\n", "charter.toml"),
        [
            "theme.icons in charter.toml is \"system\", which is not purlis-icons or \
          <extension>/<icon theme>"
        ]
    );
    let top: toml::Table = toml::from_str("[theme]\nicons = \"purlis-icons\"\n").unwrap();
    assert!(refusals_in_workspace(&top, "workspaces/alpha/workspace.json").is_empty());
}
