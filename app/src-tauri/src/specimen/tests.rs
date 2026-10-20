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
    handed: Mutex<Vec<Handed>>,
    refuses: Option<String>,
    turns: Turns,
    spacing: Spacing,
}

impl Recording {
    /// The themes it was handed, in order.
    fn themes(&self) -> Vec<WindowTheme> {
        self.handed
            .lock()
            .unwrap()
            .iter()
            .map(|one| one.theme.clone())
            .collect()
    }
}

impl Painter for Recording {
    fn turns(&self) -> &Turns {
        &self.turns
    }

    fn spacing(&self) -> &Spacing {
        &self.spacing
    }

    fn paint(&self, handed: &Handed, _: &Turn<'_>) -> Result<Picture, String> {
        if let Some(why) = &self.refuses {
            return Err(why.clone());
        }
        self.handed.lock().unwrap().push(handed.clone());
        Ok(Picture {
            png: handed.theme.name.as_bytes().to_vec(),
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
        options: purlis_core::theme::Values::new(),
    }
}

fn ask(chat: u32, specimens: Vec<Specimen>) -> ThemePreviewAsk {
    ThemePreviewAsk { chat, specimens }
}

/// Chat 7 is open, at the root of a plane no theme folder is in.
const OPEN: &dyn Fn(u32) -> Option<ChatAt> = &|chat| {
    (chat == 7).then(|| ChatAt {
        plane: std::env::temp_dir().join("purlis-specimen-no-plane"),
        workspace: None,
        id: None,
    })
};

/// [`answered`] with no config home: the built-in themes only.
fn answered_here(painter: Option<&dyn Painter>, asks: &ThemePreviewAsk) -> Answer {
    answered(OPEN, None, painter, asks)
}

#[test]
fn each_specimen_is_drawn_in_the_theme_the_engine_resolves_in_the_order_asked() {
    let painter = Recording::default();
    let answer = answered_here(
        Some(&painter),
        &ask(
            7,
            vec![
                specimen("purlis-classic", Some(Appearance::Light)),
                specimen("purlis-classic", Some(Appearance::Dark)),
            ],
        ),
    );
    assert_eq!(
        painter.themes(),
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
    let answer = answered_here(
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
    let answer = answered_here(Some(&painter), &ask(7, vec![specimen("nord", None)]));
    assert!(
        matches!(answer, Answer::No { ref why } if why.contains("no theme `nord` within this chat's reach")),
        "{answer:?}"
    );
    assert!(painter.handed.lock().unwrap().is_empty());
}

#[test]
fn none_or_more_than_two_specimens_are_refused() {
    let painter = Recording::default();
    for specimens in [Vec::new(), vec![specimen("purlis-classic", None); 3]] {
        let answer = answered_here(Some(&painter), &ask(7, specimens));
        assert!(
            matches!(answer, Answer::No { ref why } if why.contains("side by side")),
            "{answer:?}"
        );
    }
}

#[test]
fn with_no_painter_or_a_failed_one_the_chat_is_told_why() {
    let answer = answered_here(None, &ask(7, vec![specimen("purlis-classic", None)]));
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
    let answer = answered_here(
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
    let script = creation_script(&Handed {
        theme: theme.clone(),
        tint: Some("teal".to_owned()),
        faces: vec![("ab12".to_owned(), b"wOF2".to_vec())],
    });
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
        serde_json::json!({
            "theme": theme,
            "width": 1280,
            "height": 800,
            "tint": "teal",
            "faces": { "ab12": "d09GMg==" },
        })
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

#[test]
fn a_gallery_picture_asked_while_a_chat_is_drawn_is_answered_busy_and_draws_nothing() {
    let painter = Recording::default();
    let chat = painter
        .turns()
        .chat(std::time::Duration::ZERO)
        .expect("free");
    let asked = painted(
        Some(&painter),
        None,
        &[specimen("purlis-classic", None)],
        Who::Gallery,
    );
    assert_eq!(asked, Err(BUSY.to_owned()));
    assert!(painter.handed.lock().unwrap().is_empty());
    drop(chat);
    assert!(
        painted(
            Some(&painter),
            None,
            &[specimen("purlis-classic", None)],
            Who::Gallery
        )
        .is_ok()
    );
}

#[test]
fn a_chat_asked_while_a_gallery_picture_is_drawn_is_drawn_once_it_gives_way() {
    let painter = Recording::default();
    std::thread::scope(|scope| {
        let gallery = painter.turns().gallery().expect("free");
        // The gallery's page, still drawing: it gives way as soon as the chat waits.
        let painting = scope.spawn(move || {
            let (_page, drawn) = mpsc::channel::<()>();
            let waited = gallery.wait(&drawn, Instant::now() + std::time::Duration::from_secs(30));
            drop(gallery);
            waited
        });
        let answer = answered_here(
            Some(&painter),
            &ask(7, vec![specimen("purlis-classic", None)]),
        );
        assert!(matches!(answer, Answer::Previewed { .. }), "{answer:?}");
        assert_eq!(painting.join().unwrap(), Err(Waited::GaveWay));
    });
}

/// A config home, and a plane whose workspace `alpha` is pink and has a theme folder of the
/// person's own, `typed`, whose terminal font is one face; `beta` has its own `other`.
struct Places {
    _home: tempfile::TempDir,
    config: std::path::PathBuf,
    plane: std::path::PathBuf,
    face: Vec<u8>,
}

impl Places {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config");
        let plane = home.path().join("plane");
        for dir in [
            ".purlis",
            "workspaces/alpha",
            "workspaces/beta/themes/other",
        ] {
            std::fs::create_dir_all(plane.join(dir)).unwrap();
        }
        std::fs::write(
            plane.join("workspaces/alpha/workspace.json"),
            r#"{"name": "alpha", "settings": {"theme": {"colour": "pink"}}}"#,
        )
        .unwrap();
        let theme = |name: &str| {
            serde_json::json!({
                "name": name,
                "extends": "purlis-classic",
                "appearances": [{ "id": "dark", "appearance": "dark",
                    "tokens": { "surface.base": "#181820" } }],
                "fonts": { "terminal": { "family": "Mono", "faces": [
                    { "file": "fonts/Mono.woff2" }
                ] } }
            })
            .to_string()
        };
        let mut face = b"wOF2".to_vec();
        face.extend_from_slice(b"the person's own face, #2536");
        let typed = purlis_core::machine::dir(&config).join("themes/typed");
        std::fs::create_dir_all(typed.join("fonts")).unwrap();
        std::fs::write(typed.join("theme.json"), theme("Typed")).unwrap();
        std::fs::write(typed.join("fonts/Mono.woff2"), &face).unwrap();
        std::fs::write(
            plane.join("workspaces/beta/themes/other/theme.json"),
            r#"{"name": "Other", "appearances": [{"id": "dark", "appearance": "dark"}]}"#,
        )
        .unwrap();
        Self {
            _home: home,
            config,
            plane,
            face,
        }
    }

    /// Chat 7, open in `workspace`.
    fn chat(&self, workspace: Option<&str>) -> impl Fn(u32) -> Option<ChatAt> + '_ {
        let workspace = workspace.map(str::to_owned);
        move |chat| {
            (chat == 7).then(|| ChatAt {
                plane: self.plane.clone(),
                workspace: workspace.clone(),
                id: None,
            })
        }
    }
}

#[test]
fn a_chat_s_specimen_is_its_workspace_s_theme_tinted_as_there_in_the_theme_s_own_fonts() {
    let places = Places::new();
    let painter = Recording::default();
    let answer = answered(
        &places.chat(Some("alpha")),
        Some(places.config.clone()),
        Some(&painter),
        &ask(7, vec![specimen("machine:typed", None)]),
    );
    assert!(matches!(answer, Answer::Previewed { .. }), "{answer:?}");
    let handed = painter.handed.lock().unwrap();
    let [one] = handed.as_slice() else {
        panic!("{handed:?}");
    };
    assert_eq!(one.theme.values["surface.base"], "#181820");
    assert_eq!(one.tint.as_deref(), Some("pink"));
    let terminal = &one.theme.fonts.terminal;
    assert_eq!(
        one.faces,
        [(terminal.faces[0].asset.clone(), places.face.clone())]
    );
    // And at the plane root it is drawn untinted.
    drop(handed);
    let root = Recording::default();
    answered(
        &places.chat(None),
        Some(places.config.clone()),
        Some(&root),
        &ask(7, vec![specimen("typed", None)]),
    );
    assert_eq!(root.handed.lock().unwrap()[0].tint, None);
}

#[test]
fn another_workspace_s_theme_is_beyond_a_chat_s_reach_and_nothing_is_drawn() {
    let places = Places::new();
    let painter = Recording::default();
    for id in ["other", "workspace:beta:other"] {
        let answer = answered(
            &places.chat(Some("alpha")),
            Some(places.config.clone()),
            Some(&painter),
            &ask(7, vec![specimen(id, None)]),
        );
        assert!(matches!(answer, Answer::No { .. }), "{id}: {answer:?}");
    }
    assert!(painter.handed.lock().unwrap().is_empty());
}

#[test]
fn a_specimen_asked_for_while_another_chat_s_is_drawn_is_refused_at_once() {
    let painter = Recording::default();
    let held = painter
        .turns()
        .chat(std::time::Duration::ZERO)
        .expect("free");
    let answer = answered_here(
        Some(&painter),
        &ask(7, vec![specimen("purlis-classic", None)]),
    );
    assert_eq!(
        answer,
        Answer::No {
            why: BUSY.to_owned()
        }
    );
    drop(held);
    assert!(painter.themes().is_empty());
    // An ask answered busy took no turn, so it is not spaced: the next is drawn.
    let answer = answered_here(
        Some(&painter),
        &ask(7, vec![specimen("purlis-classic", None)]),
    );
    assert!(matches!(answer, Answer::Previewed { .. }), "{answer:?}");
}

#[test]
fn a_chat_asking_again_at_once_is_spaced_so_the_gallery_is_drawn_between() {
    let painter = Recording::default();
    let one = || {
        answered_here(
            Some(&painter),
            &ask(7, vec![specimen("purlis-classic", None)]),
        )
    };
    assert!(matches!(one(), Answer::Previewed { .. }));
    let again = one();
    assert!(
        matches!(again, Answer::No { ref why } if why.contains("ask again in")),
        "{again:?}"
    );
    // Meanwhile Settings › Theme's picture is drawn: the chat holds no turn and waits for none.
    assert!(
        painted(
            Some(&painter),
            None,
            &[specimen("purlis-classic", None)],
            Who::Gallery
        )
        .is_ok()
    );
    // Another chat is not held back by it.
    let other = answered(
        &|chat| {
            (chat == 8).then(|| ChatAt {
                plane: std::env::temp_dir().join("purlis-specimen-no-plane"),
                workspace: None,
                id: None,
            })
        },
        None,
        Some(&painter),
        &ask(8, vec![specimen("purlis-classic", None)]),
    );
    assert!(matches!(other, Answer::Previewed { .. }), "{other:?}");
    assert_eq!(painter.themes().len(), 3);
}

#[test]
fn a_line_written_past_the_tool_is_held_to_its_bounds_and_nothing_is_drawn() {
    let places = Places::new();
    let painter = Recording::default();
    let many: purlis_core::theme::Values = (0..=themepreview::MOST_OPTIONS)
        .map(|n| (format!("o{n}"), "on".to_owned()))
        .collect();
    for specimens in [
        vec![specimen("/etc/passwd", None)],
        vec![Specimen {
            options: many,
            ..specimen("typed", None)
        }],
        vec![Specimen {
            high_contrast: true,
            ..specimen("purlis", None)
        }],
    ] {
        let answer = answered(
            &places.chat(Some("alpha")),
            Some(places.config.clone()),
            Some(&painter),
            &ask(7, specimens),
        );
        assert!(matches!(answer, Answer::No { .. }), "{answer:?}");
    }
    assert!(painter.themes().is_empty());
}

#[test]
fn a_chat_with_no_id_reaches_no_draft_and_no_workspace_folder() {
    let places = Places::new();
    // Where a chat purlis has no record of works is the plane root, with no drafts.
    std::fs::create_dir_all(places.plane.join("workspaces/alpha/themes/client")).unwrap();
    std::fs::write(
        places
            .plane
            .join("workspaces/alpha/themes/client/theme.json"),
        r#"{"name": "Client", "appearances": [{"id": "dark", "appearance": "dark"}]}"#,
    )
    .unwrap();
    let no_id = |chat: u32| {
        (chat == 7).then(|| ChatAt {
            plane: places.plane.clone(),
            workspace: None,
            id: None,
        })
    };
    let painter = Recording::default();
    for id in [
        "draft:client",
        "client",
        "workspace:client",
        "workspace:alpha:client",
    ] {
        let answer = answered(
            &no_id,
            Some(places.config.clone()),
            Some(&painter),
            &ask(7, vec![specimen(id, None)]),
        );
        assert!(matches!(answer, Answer::No { .. }), "{id}: {answer:?}");
    }
    assert!(painter.themes().is_empty());
}

#[test]
fn a_theme_whose_fonts_are_past_the_cap_is_handed_to_no_page() {
    let places = Places::new();
    let folder = purlis_core::machine::dir(&places.config).join("themes/heavy");
    std::fs::create_dir_all(folder.join("fonts")).unwrap();
    let mut listed = Vec::new();
    for n in 0..9u8 {
        let mut bytes = b"wOF2".to_vec();
        bytes.resize(2_000_000, n);
        std::fs::write(folder.join(format!("fonts/F{n}.woff2")), &bytes).unwrap();
        listed.push(serde_json::json!({
            "file": format!("fonts/F{n}.woff2"), "weight": 100 + u16::from(n % 8) * 100
        }));
    }
    let theme = serde_json::json!({
        "name": "Heavy", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "appearance": "dark" }],
        "fonts": {
            "ui": { "family": "Heavy", "faces": listed[..8] },
            "mono": { "family": "Heavy Mono", "faces": [listed[8]] },
        }
    });
    std::fs::write(folder.join("theme.json"), theme.to_string()).unwrap();
    let painter = Recording::default();
    let answer = answered(
        &places.chat(None),
        Some(places.config.clone()),
        Some(&painter),
        &ask(7, vec![specimen("heavy", None)]),
    );
    assert!(
        matches!(answer, Answer::No { ref why } if why.contains("more than 16 MiB")),
        "{answer:?}"
    );
    assert!(painter.handed.lock().unwrap().is_empty());
}
