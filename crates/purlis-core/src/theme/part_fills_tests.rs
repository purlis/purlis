// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every part's words are read on the fill it sets** (#3099): a part that takes a fill and
//! words declares the words it is drawn in and, where its stylesheet gives it a ground of its
//! own, that fill, so the floors read its words on what is under them, from the one rule
//! [`super::drawn`] keeps, never a family's own.

use serde_json::json;

use super::super::vocabulary::TOKENS;
use super::super::{ResolvedTheme, load};
use super::{MANIFEST, Prop, fallen, family};

/// Every part but a root that takes both a fill and words.
fn filled_with_words() -> Vec<(&'static str, &'static str)> {
    MANIFEST
        .iter()
        .flat_map(|family| {
            family.parts.iter().skip(1).filter_map(move |part| {
                (part.properties.contains(&Prop::Fill) && part.properties.contains(&Prop::Text))
                    .then_some((family.name, part.name))
            })
        })
        .collect()
}

/// A part's declared words and fill.
fn declared_of(family_name: &str, part: &str) -> (Option<&'static str>, Option<&'static str>) {
    let family = family(family_name).expect("a family");
    let at = family
        .parts
        .iter()
        .position(|one| one.name == part)
        .expect("a part");
    (family.own_words(at), family.own_fill(at))
}

#[test]
fn every_part_that_takes_a_fill_and_words_declares_the_words_it_is_drawn_in() {
    let missing: Vec<String> = filled_with_words()
        .into_iter()
        .filter(|(family, part)| declared_of(family, part).0.is_none())
        .map(|(family, part)| format!("{family}.{part}"))
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
}

/// A stylesheet with its comments left out.
fn uncommented(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("/*") {
        out.push_str(&rest[..at]);
        rest = rest[at..]
            .find("*/")
            .map_or("", |end| &rest[at + end + 2..]);
    }
    out.push_str(rest);
    out
}

/// What the rules `selectors` open in `css` declare for `property`, the last that declares it
/// winning as the cascade has it, as a token: `None` for none, and for `transparent`.
fn drawn_as(css: &str, selectors: &[&str], property: &str) -> Option<String> {
    let value = selectors
        .iter()
        .filter_map(|selector| {
            let at = css
                .find(&format!("\n{selector} {{"))
                .unwrap_or_else(|| panic!("no rule {selector}"));
            let body = &css[at..];
            let body = &body[..body.find('}').expect("the rule ends")];
            body.lines().map(str::trim).find_map(|line| {
                line.strip_prefix(&format!("{property}: "))
                    .map(|value| value.trim_end_matches(';'))
            })
        })
        .next_back()?;
    let name = value.strip_prefix("var(--")?.strip_suffix(')')?;
    TOKENS
        .iter()
        .find(|token| token.replace('.', "-") == name)
        .map(|token| (*token).to_owned())
}

/// **Where a part is drawn** (#3099): its family and part, the stylesheet, the rules that draw
/// the part itself at rest, the rules of what it stands in (a tab's counts button around its
/// count), and the surface it stands on where no rule of its stylesheet paints that (a
/// switcher's row, on the popover it is drawn in).
struct Drawn {
    family: &'static str,
    part: &'static str,
    /// The stylesheet, from `app/src`.
    css: &'static str,
    own: &'static [&'static str],
    around: &'static [&'static str],
    on: Option<&'static str>,
}

/// **The parts drawn otherwise than by their own rule** (#5573): a part whose rule is
/// `[data-scope="<family>"][data-part="<part>"]`, in either order, in one stylesheet under
/// `app/src/ui`, standing on nothing named, is found by [`drawn`] and needs no row here.
const APART: &[Drawn] = &[
    Drawn {
        family: "tabs",
        part: "closer",
        css: "ui/tabs/tabs.css",
        own: &[
            "[data-scope=\"tabs\"][data-part=\"tab\"] [data-part=\"closer\"][data-scope=\"tabs\"]",
        ],
        around: &[],
        on: None,
    },
    Drawn {
        family: "tabs",
        part: "count",
        css: "App.css",
        own: &[".tasks-menu-summary .count"],
        // The summary of a chat's tasks menu, on the menu's own surface (#5341).
        around: &[".tasks-menu-summary"],
        on: Some("surface.overlay"),
    },
    Drawn {
        family: "popover",
        part: "footer",
        css: "ui/popover/popover.css",
        own: &["[data-scope=\"popover\"][data-part=\"footer\"][data-of=\"keys\"]"],
        around: &[],
        on: None,
    },
    Drawn {
        family: "titlebar",
        part: "pin",
        css: "ui/titlebar/titlebar.css",
        own: &["[data-part=\"pin\"][data-scope=\"titlebar\"]"],
        around: &[],
        on: Some("surface.overlay"),
    },
    Drawn {
        family: "titlebar",
        part: "row",
        css: "ui/titlebar/titlebar.css",
        own: &["[data-part=\"row\"][data-scope=\"titlebar\"]"],
        around: &[],
        on: Some("surface.overlay"),
    },
];

/// The window's sources, `app/src`.
fn app_src() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src")
}

/// Every stylesheet under `app/src/ui`, as its path from `app/src` and its text, comments left
/// out.
fn family_sheets() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, into: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("a folder") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(&path, into);
            } else if path.extension().is_some_and(|ext| ext == "css") {
                into.push(path);
            }
        }
    }
    let root = app_src();
    let mut paths = Vec::new();
    walk(&root.join("ui"), &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("a stylesheet");
            let from = path.strip_prefix(&root).expect("under app/src");
            (from.display().to_string(), uncommented(&text))
        })
        .collect()
}

/// **Where a part was found drawn**: its family and part, the stylesheet's path from `app/src`
/// and its text with comments left out, and the rules [`Drawn`] names.
struct Found {
    family: &'static str,
    part: &'static str,
    path: String,
    css: String,
    own: Vec<String>,
    around: Vec<String>,
    on: Option<&'static str>,
}

/// Where each part that takes a fill and words is drawn, in the manifest's order: its row in
/// [`APART`], or else the one stylesheet under `app/src/ui` with a rule of its own.
fn drawn() -> Vec<Found> {
    let owned = |selectors: &[&str]| selectors.iter().map(|one| (*one).to_owned()).collect();
    let sheets = family_sheets();
    filled_with_words()
        .into_iter()
        .map(|(family, part)| {
            if let Some(apart) = APART
                .iter()
                .find(|one| one.family == family && one.part == part)
            {
                let text = std::fs::read_to_string(app_src().join(apart.css))
                    .unwrap_or_else(|_| panic!("{family}.{part}: no {}", apart.css));
                return Found {
                    family,
                    part,
                    path: apart.css.to_owned(),
                    css: uncommented(&text),
                    own: owned(apart.own),
                    around: owned(apart.around),
                    on: apart.on,
                };
            }
            let orders = [
                format!("[data-scope=\"{family}\"][data-part=\"{part}\"]"),
                format!("[data-part=\"{part}\"][data-scope=\"{family}\"]"),
            ];
            let mut found: Vec<(&String, &String, &String)> = sheets
                .iter()
                .flat_map(|(path, css)| {
                    orders
                        .iter()
                        .filter(|selector| css.contains(&format!("\n{selector} {{")))
                        .map(move |selector| (path, css, selector))
                })
                .collect();
            let (Some((path, css, selector)), None) = (found.pop(), found.pop()) else {
                panic!(
                    "{family}.{part}: no single rule of its own in a stylesheet under \
                     app/src/ui; draw it in one, or give it a row in APART"
                )
            };
            Found {
                family,
                part,
                path: path.clone(),
                css: css.clone(),
                own: vec![selector.clone()],
                around: Vec::new(),
                on: None,
            }
        })
        .collect()
}

#[test]
fn every_part_drawn_apart_is_one_that_takes_a_fill_and_words() {
    let filled = filled_with_words();
    let stale: Vec<String> = APART
        .iter()
        .filter(|one| !filled.contains(&(one.family, one.part)))
        .map(|one| format!("{}.{}", one.family, one.part))
        .collect();
    assert!(stale.is_empty(), "{stale:?}");
}

/// A theme over Purlis Classic that sets `tokens` and `parts`, as read.
fn shaped(tokens: serde_json::Value, parts: serde_json::Value) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": tokens }],
        "parts": parts,
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded.theme.first().clone()
}

/// What fell, as `<part> <floor>`.
fn fell_in(theme: &ResolvedTheme) -> Vec<String> {
    fallen(theme)
        .into_iter()
        .map(|one| format!("{} {}", one.part, one.floor))
        .collect()
}

#[test]
fn a_footer_and_a_key_drawn_in_their_own_fill_fall_back_and_the_check_names_them() {
    for part in ["footer", "key"] {
        let theme = shaped(
            json!({}),
            json!({
                format!("popover.{part}.fill"): "surface.raised",
                format!("popover.{part}.text"): "surface.raised",
            }),
        );
        let fell = fell_in(&theme);
        assert!(
            fell.iter()
                .any(|one| one.contains(&format!("the {part}'s words contrast 4.5:1"))),
            "{part}: {fell:?}"
        );
        let report = super::super::check(&theme);
        assert!(
            report.findings.iter().any(|one| matches!(
                one,
                super::super::check::Finding::PartFallsBack { floor, .. }
                    if floor.contains(&format!("the {part}'s words"))
            )),
            "{part}: {:?}",
            report.findings
        );
    }
}

#[test]
fn a_key_s_words_are_read_on_its_raised_cap_not_on_the_popover() {
    // A light cap in a dark theme: the primary words read on the popover's surface, but not on
    // the key's own raised cap, which is what is under them.
    let theme = shaped(
        json!({ "surface.raised": "#eeeeee" }),
        json!({ "popover.key.text": "text.primary" }),
    );
    let fell = fell_in(&theme);
    assert!(
        fell.iter()
            .any(|one| one.contains("the key's words contrast 4.5:1")),
        "{fell:?}"
    );
}

#[test]
fn a_tab_s_closer_is_held_as_a_mark() {
    // A faint × that reads as a mark (3:1) but not as words (4.5:1) is drawn as written.
    let theme = shaped(json!({}), json!({ "tabs.closer.text": "text.faint" }));
    let fell = fell_in(&theme);
    assert!(!fell.iter().any(|one| one.contains("closer")), "{fell:?}");
}

/// Selectors as `&str`s.
fn as_strs(selectors: &[String]) -> Vec<&str> {
    selectors.iter().map(String::as_str).collect()
}

#[test]
fn every_part_that_takes_a_fill_and_words_is_drawn_where_its_stylesheet_says() {
    for one in drawn() {
        let named = format!("{}.{} in {}", one.family, one.part, one.path);
        let (css, own, around) = (&one.css, &as_strs(&one.own), &as_strs(&one.around));
        let family = family(one.family).expect("a family");
        let at = family
            .parts
            .iter()
            .position(|part| part.name == one.part)
            .expect("a part");
        let painted = |selectors: &[&str]| {
            drawn_as(css, selectors, "background-color")
                .or_else(|| drawn_as(css, selectors, "background"))
        };
        // Its words: its own colour, or the colour of what it stands in.
        let colour = drawn_as(css, own, "color").or_else(|| drawn_as(css, around, "color"));
        assert_eq!(
            family.own_words(at).map(str::to_owned),
            colour,
            "{named}'s words"
        );
        // A fill is what the part paints itself, and only that: a fallback paints it again.
        assert_eq!(
            family.own_fill(at).map(str::to_owned),
            painted(own),
            "{named}'s own fill"
        );
        // What it stands on otherwise is read, never painted.
        let under = family.stands_on(at).map(|grounds| grounds.join(" "));
        assert_eq!(
            under,
            painted(around).or(one.on.map(str::to_owned)),
            "{named}'s ground"
        );
    }
}

#[test]
fn a_fallback_paints_only_the_fills_a_part_paints_itself() {
    // A tab whose words vanish on its fill falls back; the count inside it, which paints no
    // ground of its own, is not given its chip's sunken pill.
    let theme = shaped(
        json!({}),
        json!({ "tabs.tab.fill": "surface.base", "tabs.tab.text": "surface.base" }),
    );
    assert!(!fell_in(&theme).is_empty(), "the tab falls back");
    let sheet = super::stylesheet(&theme);
    assert!(
        !sheet.lines().any(
            |line| line.contains("[data-scope=\"tabs\"][data-part=\"count\"]")
                && line.contains("background-color")
        ),
        "{sheet}"
    );
    // The key's cap, which the key paints, is painted again by a popover's fallback.
    let popover = shaped(
        json!({}),
        json!({ "popover.key.fill": "surface.raised", "popover.key.text": "surface.raised" }),
    );
    assert!(
        super::stylesheet(&popover).lines().any(|line| line
            .contains("[data-scope=\"popover\"][data-part=\"key\"]")
            && line.contains("background-color: var(--surface-raised)")),
        "the key's cap is painted again"
    );
}
