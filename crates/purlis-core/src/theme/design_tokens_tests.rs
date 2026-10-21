// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The design-tokens export and import (#2926): every export holds to the format's official
//! 2025.10 schema, and exporting then importing gives the same theme folder.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::design_tokens::{self, SCHEMA_ID, VENDOR};
use super::draft7::Schemas;
use super::import::Files;
use super::{SourceKind, THEME_JSON};

/// The vendored format, schema and test suite.
fn vendored() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/theme/dtcg-2025.10")
}

fn schemas() -> Schemas {
    let text = std::fs::read_to_string(vendored().join("format.json")).unwrap();
    Schemas::bundle(&serde_json::from_str(&text).unwrap())
}

/// Why `bytes` does not hold to the format's schema: empty when it does.
fn schema_errors(bytes: &[u8]) -> Vec<String> {
    let document: Value = serde_json::from_slice(bytes).unwrap();
    schemas().validate(SCHEMA_ID, &document)
}

/// `theme.json` and the files beside it.
fn folder(theme: &Value, beside: &[(&str, &[u8])]) -> Files {
    let mut files = Files::from([(
        THEME_JSON.to_owned(),
        serde_json::to_vec_pretty(theme).unwrap(),
    )]);
    for (path, bytes) in beside {
        files.insert((*path).to_owned(), bytes.to_vec());
    }
    files
}

/// Exports `files` as `name`, checks the schema holds, imports it back through the engine's
/// import, and says what came back.
fn round_trip(name: &str, files: &Files) -> super::Imported {
    let bytes = design_tokens::pack(name, files).unwrap();
    let errors = schema_errors(&bytes);
    assert!(
        errors.is_empty(),
        "{name}'s export breaks the 2025.10 schema: {errors:#?}"
    );
    super::import(
        SourceKind::DesignTokens,
        &format!("{name}.tokens.json"),
        &bytes,
    )
    .unwrap()
}

/// That `back` is `files` again: the same `theme.json`, read as JSON, and the same files beside.
fn assert_same(files: &Files, back: &Files) {
    let json = |files: &Files| -> Value { serde_json::from_slice(&files[THEME_JSON]).unwrap() };
    assert_eq!(json(back), json(files), "theme.json changed on the way");
    assert_eq!(
        serde_json::to_string(&json(back)).unwrap(),
        serde_json::to_string(&json(files)).unwrap(),
        "theme.json's keys moved on the way"
    );
    let others = |files: &Files| -> Vec<(String, Vec<u8>)> {
        files
            .iter()
            .filter(|(path, _)| path.as_str() != THEME_JSON)
            .map(|(path, bytes)| (path.clone(), bytes.clone()))
            .collect()
    };
    assert_eq!(
        others(back),
        others(files),
        "the files beside theme.json changed"
    );
}

/// That the engine reads both folders as the same theme.
fn assert_same_theme(files: &Files, back: &Files) {
    let read = |files: &Files| {
        let document: Value = serde_json::from_slice(&files[THEME_JSON]).unwrap();
        super::load(&document)
    };
    let before = read(files);
    assert!(!before.refused, "{:?}", before.complaints);
    assert_eq!(read(back), before);
}

#[test]
fn the_validator_gives_every_official_fixture_the_verdict_its_manifest_gives() {
    let suite = vendored().join("test-suite");
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(suite.join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["version"], "2025.10");
    assert_eq!(manifest["schemaId"], SCHEMA_ID);
    let schemas = schemas();
    let tests = manifest["tests"].as_array().unwrap();
    assert!(tests.len() > 200, "the suite is whole");
    let mut wrong = Vec::new();
    let mut checked = 0;
    for test in tests {
        // As the suite's own schema tests do: a fixture that needs its references resolved
        // first is no test of a schema.
        let features = test["features"].as_array().cloned().unwrap_or_default();
        if features.iter().any(|it| it == "preprocessing-required") {
            continue;
        }
        checked += 1;
        let input = suite.join(test["input"].as_str().unwrap());
        let document: Value =
            serde_json::from_str(&std::fs::read_to_string(&input).unwrap()).unwrap();
        let valid = schemas.validate(SCHEMA_ID, &document).is_empty();
        let expected = test["type"] == "PositiveEvaluationTest";
        if valid != expected {
            wrong.push(test["id"].as_str().unwrap().to_owned());
        }
    }
    assert!(checked > 150, "only {checked} fixtures checked");
    assert!(
        wrong.is_empty(),
        "verdicts that differ from the suite's: {wrong:?}"
    );
}

#[test]
fn the_vendored_schema_is_the_one_its_readme_names() {
    use sha2::Digest as _;
    let bytes = std::fs::read(vendored().join("format.json")).unwrap();
    let digest = crate::extension::hex(&sha2::Sha256::digest(&bytes));
    let readme = std::fs::read_to_string(vendored().join("README.md")).unwrap();
    assert!(
        readme.contains(&digest),
        "README.md names the schema's sha-256 {digest}"
    );
    for licence in ["LICENSE.md", "LICENSE-W3C-BSD"] {
        assert!(
            vendored().join(licence).is_file(),
            "{licence} travels with it"
        );
    }
}

#[test]
fn purlis_classic_exports_to_the_schema_and_comes_back_the_same() {
    let files = Files::from([(
        THEME_JSON.to_owned(),
        super::CLASSIC_THEME_JSON.as_bytes().to_vec(),
    )]);
    let back = round_trip("purlis-classic-copy", &files);
    assert_eq!(back.folder, "purlis-classic-copy");
    assert_eq!(back.name, "Purlis Classic");
    assert_same(&files, &back.files);
    assert_same_theme(&files, &back.files);
    assert!(back.left_out.is_empty(), "{:?}", back.left_out);
}

#[test]
fn purlis_exports_with_its_fonts_icons_and_licence_and_comes_back_the_same() {
    let files = super::purlis_files();
    assert!(files.keys().any(|path| path.starts_with("fonts/")));
    let back = round_trip("purlis-copy", &files);
    assert_same(&files, &back.files);
    assert!(back.left_out.is_empty(), "{:?}", back.left_out);
}

#[test]
fn purlis_s_grey_tiers_are_tokens_and_come_back_as_its_palette_names() {
    let files = super::purlis_files();
    let bytes = design_tokens::pack("purlis-copy", &files).unwrap();
    let document: Value = serde_json::from_slice(&bytes).unwrap();
    for (tier, name) in [
        ("primary", "ink"),
        ("secondary", "body"),
        ("muted", "label"),
        ("quiet", "quiet"),
        ("faint", "faint"),
    ] {
        let token = &document["appearance"]["dark"]["text"][tier];
        assert_eq!(token["$type"], "color", "text.{tier}");
        assert_eq!(
            token["$value"],
            format!("{{palette.{name}}}"),
            "text.{tier}"
        );
    }
    assert_eq!(
        document["palette"]["quiet"]["$value"]["hex"], "#91909d",
        "the palette's grey is a colour with its hex"
    );
}

/// A theme with component parts: every kind of value a part takes.
fn parts_theme() -> Value {
    json!({
        "$schema": super::SCHEMA_URL,
        "name": "Pill",
        "extends": "purlis-classic",
        "palette": { "lavender": "#b4a7f5" },
        "appearances": [
            { "id": "dark", "appearance": "dark", "tokens": { "accent.base": "lavender" } },
            { "id": "light", "appearance": "light", "tokens": { "accent.base": "#5b4bc4" } }
        ],
        "parts": {
            "button.root.corner-radius": "full",
            "button.root.primary.hover.fill": "accent.base",
            "button.root.ghost.fill": "transparent",
            "button.root.padding": [4, "space.3"],
            "button.root.border-width": 0,
            "button.root.shadow": "raised",
            "button.label.font-weight": 600,
            "button.root.secondary.corner-radius": [0, "full", "radius.md", 6]
        }
    })
}

#[test]
fn a_theme_s_component_parts_come_back_the_same() {
    let files = folder(&parts_theme(), &[]);
    let back = round_trip("pill", &files);
    assert_same(&files, &back.files);
    assert_same_theme(&files, &back.files);
    let document: Value =
        serde_json::from_slice(&design_tokens::pack("pill", &files).unwrap()).unwrap();
    assert_eq!(
        document["$extensions"][VENDOR]["theme"]["parts"],
        parts_theme()["parts"],
        "parts travel as the theme writes them"
    );
}

#[test]
fn a_theme_that_names_only_its_muted_keeps_its_quiet_tiers_following_it() {
    let theme = json!({
        "name": "Muted",
        "extends": "purlis",
        "appearances": [
            { "id": "dark", "appearance": "dark", "tokens": { "text.muted": "#9a98a6" } }
        ]
    });
    let files = folder(&theme, &[]);
    let back = round_trip("muted", &files);
    assert_same(&files, &back.files);
    let written: Value = serde_json::from_slice(&back.files[THEME_JSON]).unwrap();
    let tokens = written["appearances"][0]["tokens"].as_object().unwrap();
    assert_eq!(
        tokens.len(),
        1,
        "no tier was written that the theme left to follow"
    );
}

/// A theme that uses every kind of value `theme.json` has.
fn every_value() -> Value {
    json!({
        "$schema": super::SCHEMA_URL,
        "name": "Every value",
        "palette": { "ink": "#E6E4EC", "wash": "#a99cf024", "short": "#fff", "bad.name": "#000000" },
        "appearances": [
            {
                "id": "night",
                "appearance": "dark",
                "high-contrast": true,
                "tokens": {
                    "text.primary": "ink",
                    "accent.surface": "wash",
                    "surface.base": "#121215ff",
                    "danger.wash": "#f0a9a81f",
                    "caret": "platform",
                    "syntax.keyword": "#c3baf5",
                    "syntax.keyword.italic": true,
                    "terminal.cursor-style": "bar",
                    "terminal.line-height": 1.2,
                    "radius.md": 8,
                    "font-weight.medium": 500,
                    "line-height.body": 1.5,
                    "not.a.token": "#123456",
                    "not.a.token.deeper": "#654321"
                },
                "motion": {
                    "duration.quick": 120,
                    "easing.standard": [0.2, 0, 0, 1],
                    "easing.enter": [1.4, 0, 0.2, 1]
                }
            },
            { "id": "day", "appearance": "light", "tokens": {} },
            "not an appearance"
        ],
        "compact": { "space.4": 5 },
        "options": [ { "id": "warm", "kind": "toggle", "on": { "tokens": { "accent.base": "#e5c08a" } } } ],
        "tint": { "on": false },
        "icons": "icons/icons.json"
    })
}

#[test]
fn every_kind_of_value_a_theme_holds_comes_back_as_written() {
    let files = folder(
        &every_value(),
        &[
            ("icons/icons.json", b"{\"views\":{}}"),
            ("LICENSE", b"MIT\n"),
        ],
    );
    let back = round_trip("every-value", &files);
    assert_same(&files, &back.files);
}

#[test]
fn what_a_token_type_holds_is_a_token_and_the_rest_is_kept_under_the_vendor_key() {
    let bytes = design_tokens::pack("every-value", &folder(&every_value(), &[])).unwrap();
    let document: Value = serde_json::from_slice(&bytes).unwrap();
    let night = &document["appearance"]["night"];
    assert_eq!(
        night["radius"]["md"]["$value"],
        json!({ "value": 8, "unit": "px" })
    );
    assert_eq!(night["font-weight"]["medium"]["$type"], "fontWeight");
    assert_eq!(night["line-height"]["body"]["$type"], "number");
    assert_eq!(night["terminal"]["line-height"]["$type"], "number");
    assert_eq!(
        night["motion"]["duration"]["quick"]["$value"],
        json!({ "value": 120, "unit": "ms" })
    );
    assert_eq!(
        night["motion"]["easing"]["standard"]["$type"],
        "cubicBezier"
    );
    // A name that is a token and a group's start is the group's $root.
    assert_eq!(
        night["not"]["a"]["token"]["$root"]["$value"]["hex"],
        "#123456"
    );
    assert_eq!(
        night["not"]["a"]["token"]["deeper"]["$value"]["hex"],
        "#654321"
    );
    assert_eq!(
        night["danger"]["wash"]["$value"],
        json!({ "colorSpace": "srgb", "components": [240.0 / 255.0, 169.0 / 255.0, 168.0 / 255.0],
                "alpha": 31.0 / 255.0, "hex": "#f0a9a8" })
    );
    let kept = &night["$extensions"][VENDOR];
    assert_eq!(kept["appearance"], "dark");
    assert_eq!(kept["high-contrast"], true);
    assert_eq!(kept["tokens"]["caret"], "platform");
    assert_eq!(kept["tokens"]["syntax.keyword.italic"], true);
    assert_eq!(kept["tokens"]["terminal.cursor-style"], "bar");
    assert_eq!(
        kept["motion"]["easing.enter"],
        json!([1.4, 0, 0.2, 1]),
        "x past 1 is no cubicBezier"
    );
    assert_eq!(
        document["palette"]["ink"]["$extensions"][VENDOR]["hex"],
        "#E6E4EC"
    );
    assert_eq!(document["compact"]["space"]["4"]["$value"]["value"], 5);
}

#[test]
fn a_colour_is_read_as_hex_from_its_hex_or_its_srgb_components() {
    let theme = |value: Value| {
        json!({
            "appearance": { "dark": { "surface": { "base": { "$type": "color", "$value": value } } } }
        })
    };
    let read = |document: Value| {
        design_tokens::unpack("x.tokens.json", document.to_string().as_bytes()).map(|read| {
            let written: Value = serde_json::from_slice(&read.files[THEME_JSON]).unwrap();
            written["appearances"][0]["tokens"]["surface.base"].clone()
        })
    };
    assert_eq!(
        read(theme(
            json!({ "colorSpace": "srgb", "components": [1, 0.5, 0] })
        ))
        .unwrap(),
        "#ff8000"
    );
    assert_eq!(
        read(theme(
            json!({ "colorSpace": "oklch", "components": [0.5, 0.1, 30], "hex": "#aa3311" })
        ))
        .unwrap(),
        "#aa3311",
        "another space's hex is the colour purlis reads"
    );
    let refused = read(theme(
        json!({ "colorSpace": "oklch", "components": [0.5, 0.1, 30] }),
    ))
    .unwrap_err();
    assert!(
        refused.contains("appearance.dark.surface.base") && refused.contains("hex"),
        "{refused}"
    );
    assert_eq!(
        read(theme(
            json!({ "colorSpace": "srgb", "components": ["none", 0, 0], "hex": "#000000" })
        ))
        .unwrap(),
        "#000000",
        "a component that is none leaves the colour to its hex"
    );
    let refused = read(theme(
        json!({ "colorSpace": "srgb", "components": ["none", 0, 0] }),
    ))
    .unwrap_err();
    assert!(refused.contains("hex"), "{refused}");
    let refused = read(theme(
        json!({ "colorSpace": "srgb", "components": [1, 1, 1], "hex": "#000000" }),
    ))
    .unwrap_err();
    assert!(refused.contains("two colours"), "{refused}");
}

#[test]
fn a_design_tool_s_file_reads_by_the_same_rules() {
    let document = json!({
        "palette": { "$type": "color", "night": { "$value": { "colorSpace": "srgb", "components": [0.0706, 0.0706, 0.0824], "hex": "#121215" } } },
        "base": { "grey": { "900": { "$type": "color", "$value": "{palette.night}" } } },
        "surface": { "base": { "$type": "color", "$value": "{palette.night}" } },
        "text": { "primary": { "$type": "color", "$value": "{base.grey.900}" } },
        "brand": { "$type": "color", "$value": { "colorSpace": "srgb", "components": [1, 0, 0] } },
        "duration": { "quick": { "$type": "duration", "$value": { "value": 0.2, "unit": "s" } } }
    });
    let read =
        design_tokens::unpack("Night Owl.tokens.json", document.to_string().as_bytes()).unwrap();
    assert_eq!(read.name, "Night Owl");
    assert_eq!(read.folder, "night-owl");
    let written: Value = serde_json::from_slice(&read.files[THEME_JSON]).unwrap();
    assert_eq!(written["$schema"], super::SCHEMA_URL);
    assert_eq!(written["palette"], json!({ "night": "#121215" }));
    let appearance = &written["appearances"][0];
    assert_eq!(appearance["appearance"], "dark");
    assert_eq!(appearance["tokens"]["surface.base"], "night");
    assert_eq!(
        appearance["tokens"]["text.primary"], "#121215",
        "a reference outside the palette is followed"
    );
    assert_eq!(appearance["motion"]["duration.quick"], 200);
    let left: Vec<&str> = read.left_out.iter().map(|one| one.key.as_str()).collect();
    assert_eq!(left, ["base.grey.900", "brand"]);
}

#[test]
fn a_reference_round_a_circle_or_to_nothing_refuses_the_file() {
    let circle = json!({ "appearance": { "dark": { "surface": {
        "base": { "$type": "color", "$value": "{appearance.dark.surface.raised}" },
        "raised": { "$type": "color", "$value": "{appearance.dark.surface.base}" }
    } } } });
    let why = design_tokens::unpack("x.tokens.json", circle.to_string().as_bytes()).unwrap_err();
    assert!(why.contains("circle"), "{why}");
    let nowhere = json!({ "appearance": { "dark": { "surface": {
        "base": { "$type": "color", "$value": "{palette.gone}" }
    } } } });
    let why = design_tokens::unpack("x.tokens.json", nowhere.to_string().as_bytes()).unwrap_err();
    assert!(why.contains("no token"), "{why}");
}

#[test]
fn a_carried_file_is_held_to_a_theme_folder_s_places_and_bounds() {
    let carrying = |path: &str| json!({ "$extensions": { VENDOR: { "version": 1, "files": { path: "AAAA" } } } });
    for path in [
        "../escape",
        "theme.json",
        "deep/folder/file",
        ".hidden",
        "fonts/a/b",
    ] {
        let why = design_tokens::unpack("x.tokens.json", carrying(path).to_string().as_bytes())
            .unwrap_err();
        assert!(
            why.contains("not where a theme folder keeps a file"),
            "{path}: {why}"
        );
    }
    let read = design_tokens::unpack(
        "x.tokens.json",
        carrying("fonts/a.woff2").to_string().as_bytes(),
    )
    .unwrap();
    assert_eq!(read.files["fonts/a.woff2"], [0, 0, 0]);
    let twice = json!({ "$extensions": { VENDOR: { "version": 1,
        "files": { "fonts/A.woff2": "AAAA", "fonts/a.woff2": "AAAA" } } } });
    let why = design_tokens::unpack("x.tokens.json", twice.to_string().as_bytes()).unwrap_err();
    assert!(why.contains("same file"), "{why}");
    let newer = json!({ "$extensions": { VENDOR: { "version": 99 } } });
    let why = design_tokens::unpack("x.tokens.json", newer.to_string().as_bytes()).unwrap_err();
    assert!(why.contains("newer purlis"), "{why}");
    for version in [json!("2"), json!(1.5), json!(-1)] {
        let odd = json!({ "$extensions": { VENDOR: { "version": version } } });
        let why = design_tokens::unpack("x.tokens.json", odd.to_string().as_bytes()).unwrap_err();
        assert!(why.contains("not a whole number"), "{version}: {why}");
    }
}

#[test]
fn a_design_tokens_file_is_told_apart_from_the_other_kinds() {
    let bytes = design_tokens::pack("pill", &folder(&parts_theme(), &[])).unwrap();
    assert_eq!(
        super::detect("pill.tokens.json", &bytes),
        Some(SourceKind::DesignTokens)
    );
    assert_eq!(
        super::detect("pill.tokens", &bytes),
        Some(SourceKind::DesignTokens)
    );
    assert_eq!(
        super::detect("pill.json", &bytes),
        Some(SourceKind::DesignTokens)
    );
    let foreign = br#"{ "color": { "red": { "$type": "color", "$value": "{x}" } } }"#;
    assert_eq!(
        super::detect("tokens.json", foreign),
        Some(SourceKind::DesignTokens)
    );
    // A tokens file whose top group is `colors`, as design tools often write it, is no VS Code
    // theme.
    let colors = br##"{ "colors": { "$type": "color", "red": { "$value": { "colorSpace": "srgb", "components": [1, 0, 0], "hex": "#ff0000" } } } }"##;
    assert_eq!(
        super::detect("tokens.json", colors),
        Some(SourceKind::DesignTokens)
    );
    let vscode = br##"{ "type": "dark", "colors": { "editor.background": "#101010" } }"##;
    assert_eq!(
        super::detect("theme.json", vscode),
        Some(SourceKind::Vscode)
    );
    assert_eq!(
        SourceKind::named("design-tokens"),
        Some(SourceKind::DesignTokens)
    );
}

#[test]
fn a_reserved_or_unnamed_folder_is_not_exported() {
    let files = folder(&parts_theme(), &[]);
    assert!(design_tokens::pack("purlis", &files).is_err());
    assert!(design_tokens::pack("Not A Name", &files).is_err());
    let not_json = Files::from([(THEME_JSON.to_owned(), b"[1]".to_vec())]);
    assert!(design_tokens::pack("pill", &not_json).is_err());
}

#[test]
fn a_token_where_an_appearance_goes_is_listed_not_made_an_appearance() {
    let black = json!({ "colorSpace": "srgb", "components": [0, 0, 0] });
    let document = json!({ "appearance": {
        "stray": { "$type": "color", "$value": black },
        "dark": { "surface": { "base": { "$type": "color", "$value": black } } }
    } });
    let read = design_tokens::unpack("x.tokens.json", document.to_string().as_bytes()).unwrap();
    let written: Value = serde_json::from_slice(&read.files[THEME_JSON]).unwrap();
    let ids: Vec<&str> = written["appearances"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|it| it["id"].as_str())
        .collect();
    assert_eq!(ids, ["dark"]);
    assert!(
        read.left_out
            .iter()
            .any(|one| one.key == "appearance.stray"),
        "{:?}",
        read.left_out
    );
}

/// What `run` answers, and the most this process's memory grew while it ran, sampled every
/// millisecond.
fn peak_growth<T>(run: impl FnOnce() -> T) -> (T, usize) {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let now = || memory_stats::memory_stats().map_or(0, |it| it.physical_mem);
    let before = now();
    let done = std::sync::Arc::new(AtomicBool::new(false));
    let most = std::sync::Arc::new(AtomicUsize::new(before));
    let sampler = {
        let (done, most) = (done.clone(), most.clone());
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                most.fetch_max(now(), Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        })
    };
    let answer = run();
    most.fetch_max(now(), Ordering::Relaxed);
    done.store(true, Ordering::Relaxed);
    sampler.join().unwrap();
    (answer, most.load(Ordering::Relaxed).saturating_sub(before))
}

/// A file of about `len` bytes: purlis's mark, and `filler` repeated in a list in its theme.
fn crafted(filler: &str, len: usize) -> Vec<u8> {
    let mut text = format!("{{\"$extensions\":{{\"{VENDOR}\":{{\"version\":1,\"theme\":{{\"x\":[");
    while text.len() + filler.len() + 8 < len {
        text.push_str(filler);
    }
    text.push_str("0]}}}}");
    text.into_bytes()
}

#[test]
fn imports_are_bounded_in_size_and_work() {
    let started = std::time::Instant::now();
    // At the byte cap, and past the count of values.
    let full = crafted(
        "0,",
        usize::try_from(design_tokens::MOST_BYTES).unwrap() - 16,
    );
    let why = design_tokens::unpack("x.tokens.json", &full).unwrap_err();
    assert!(why.contains("values"), "{why}");
    // At the byte cap and just within the count: read, and refused for the theme.json it makes.
    let filler = format!("\"{}\",", "a".repeat(62));
    let most = crafted(
        &filler,
        usize::try_from(design_tokens::MOST_BYTES).unwrap() - 16,
    );
    let (said, grew) = peak_growth(|| design_tokens::unpack("x.tokens.json", &most));
    let why = said.unwrap_err();
    assert!(why.contains("MiB"), "{why}");
    // The file is 24 MiB: reading it holds it parsed once, and a little more.
    assert!(
        grew < 128 * 1024 * 1024,
        "reading it took {} MiB more",
        grew / 1024 / 1024
    );
    // Within the count, but a theme.json past a theme folder's file cap.
    let long = format!("\"{}\",", "a".repeat(1024));
    let wide = crafted(&long, 3 * 1024 * 1024);
    let why = design_tokens::unpack("x.tokens.json", &wide).unwrap_err();
    assert!(why.contains("MiB"), "{why}");
    // More appearances than a theme may have.
    let many: Vec<Value> = (0..65).map(|at| json!(format!("a{at}"))).collect();
    let ordered = json!({ "$extensions": { VENDOR: { "version": 1, "appearances": many } } });
    let why = design_tokens::unpack("x.tokens.json", ordered.to_string().as_bytes()).unwrap_err();
    assert!(why.contains("appearances"), "{why}");
    // Many top-level keys, each read once.
    let keys: Vec<Value> = (0..100_000).map(|at| json!(format!("k{at}"))).collect();
    let listed = json!({ "$extensions": { VENDOR: { "version": 1, "keys": keys } } });
    design_tokens::unpack("x.tokens.json", listed.to_string().as_bytes()).unwrap();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "took {:?}",
        started.elapsed()
    );
}
