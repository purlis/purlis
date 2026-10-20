// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::Mutex;

use purlis_core::hookwire::{Answer, ThemePreviewAsk};
use purlis_core::theme::Appearance;
use purlis_core::themepreview::Specimen;

use super::*;

/// A painter that draws nothing and remembers what it was handed.
#[derive(Default)]
struct Recording {
    handed: Mutex<Vec<WindowTheme>>,
    refuses: Option<String>,
}

impl Painter for Recording {
    fn paint(&self, theme: &WindowTheme) -> Result<Picture, String> {
        if let Some(why) = &self.refuses {
            return Err(why.clone());
        }
        self.handed.lock().unwrap().push(theme.clone());
        Ok(Picture {
            png: theme.name.as_bytes().to_vec(),
            width: 2560,
            height: 1600,
        })
    }
}

fn specimen(theme: &str, appearance: Option<Appearance>) -> Specimen {
    Specimen {
        theme: theme.to_owned(),
        appearance,
        high_contrast: false,
    }
}

fn ask(chat: u32, specimens: Vec<Specimen>) -> ThemePreviewAsk {
    ThemePreviewAsk { chat, specimens }
}

const OPEN: &dyn Fn(u32) -> bool = &|chat| chat == 7;

#[test]
fn each_specimen_is_drawn_in_the_theme_the_engine_resolves_in_the_order_asked() {
    let painter = Recording::default();
    let answer = answered(
        OPEN,
        Some(&painter),
        &ask(
            7,
            vec![
                specimen("purlis-classic", Some(Appearance::Light)),
                specimen("purlis-classic", Some(Appearance::Dark)),
            ],
        ),
    );
    let handed = painter.handed.lock().unwrap();
    assert_eq!(
        *handed,
        [
            WindowTheme::from(purlis_core::theme::built_in("purlis-light").unwrap()),
            WindowTheme::from(purlis_core::theme::built_in("purlis-dark").unwrap()),
        ]
    );
    let Answer::Previewed { painted } = answer else {
        panic!("{answer:?}");
    };
    assert_eq!(
        painted
            .iter()
            .map(|p| (p.theme.as_str(), p.appearance, p.png.clone()))
            .collect::<Vec<_>>(),
        [
            (
                "purlis-classic",
                Appearance::Light,
                b"purlis-light".to_vec()
            ),
            ("purlis-classic", Appearance::Dark, b"purlis-dark".to_vec()),
        ]
    );
    assert!(painted.iter().all(|p| (p.width, p.height) == (2560, 1600)));
}

#[test]
fn a_chat_this_window_does_not_have_open_is_drawn_nothing() {
    let painter = Recording::default();
    let answer = answered(
        OPEN,
        Some(&painter),
        &ask(8, vec![specimen("purlis-classic", None)]),
    );
    assert!(
        matches!(answer, Answer::No { ref why } if why.contains("chat 8")),
        "{answer:?}"
    );
    assert!(painter.handed.lock().unwrap().is_empty());
}

#[test]
fn a_theme_this_purlis_does_not_have_is_refused_and_nothing_is_drawn() {
    let painter = Recording::default();
    let answer = answered(OPEN, Some(&painter), &ask(7, vec![specimen("nord", None)]));
    assert!(
        matches!(answer, Answer::No { ref why } if why.contains("no theme `nord`")),
        "{answer:?}"
    );
    assert!(painter.handed.lock().unwrap().is_empty());
}

#[test]
fn none_or_more_than_two_specimens_are_refused() {
    let painter = Recording::default();
    for specimens in [Vec::new(), vec![specimen("purlis-classic", None); 3]] {
        let answer = answered(OPEN, Some(&painter), &ask(7, specimens));
        assert!(
            matches!(answer, Answer::No { ref why } if why.contains("side by side")),
            "{answer:?}"
        );
    }
}

#[test]
fn with_no_painter_or_a_failed_one_the_chat_is_told_why() {
    let answer = answered(OPEN, None, &ask(7, vec![specimen("purlis-classic", None)]));
    assert_eq!(
        answer,
        Answer::No {
            why: NO_PAINTER.to_owned()
        }
    );
    let failing = Recording {
        refuses: Some(over_budget()),
        ..Recording::default()
    };
    let answer = answered(
        OPEN,
        Some(&failing),
        &ask(7, vec![specimen("purlis-classic", None)]),
    );
    assert!(
        matches!(answer, Answer::No { ref why } if why.contains("budget of 5 s")),
        "{answer:?}"
    );
}

#[test]
fn the_page_is_handed_the_built_ins_and_the_one_theme_and_nothing_else() {
    // The privacy half that is the core's: the creation script is made from the theme and the
    // compiled-in built-ins alone, so nothing of the person's can be in it. Held as the script's
    // two globals, each with exactly these keys.
    let theme = WindowTheme::from(purlis_core::theme::built_in("purlis-light").unwrap());
    let script = creation_script(&theme);
    let globals: Vec<&str> = script
        .lines()
        .filter_map(|line| line.strip_prefix("Object.defineProperty(window, \""))
        .filter_map(|rest| rest.split('"').next())
        .collect();
    assert_eq!(globals, ["__CHARTER_AT_CREATION__", GIVEN]);
    let value = |name: &str| -> serde_json::Value {
        let line = script
            .lines()
            .find(|line| line.contains(&format!("\"{name}\"")))
            .unwrap();
        let from = line.find("Object.freeze(").unwrap() + "Object.freeze(".len();
        let to = line.rfind(") })").unwrap();
        serde_json::from_str(&line[from..to]).unwrap()
    };
    let at_creation = value("__CHARTER_AT_CREATION__");
    assert_eq!(
        at_creation.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["built_in"]
    );
    assert_eq!(
        at_creation["built_in"],
        serde_json::to_value(
            purlis_core::theme::built_ins()
                .iter()
                .map(WindowTheme::from)
                .collect::<Vec<_>>()
        )
        .unwrap()
    );
    let given = value(GIVEN);
    assert_eq!(
        given,
        serde_json::json!({ "theme": theme, "width": 1280, "height": 800 })
    );
}

#[test]
fn a_specimen_window_is_none_of_purlis_s_own_windows() {
    for n in [0, 1, 42] {
        assert!(
            !crate::windows::is_charter_window(&label(n)),
            "{}",
            label(n)
        );
    }
}
