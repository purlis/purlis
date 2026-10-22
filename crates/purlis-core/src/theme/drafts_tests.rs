// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Theme drafts and the hard floors at the engine's interface (#2332): a draft in, what the
//! window draws and what Keep, Revert and the moves leave on disk out.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::drafts::{self, Draft, KeptTo, Place};
use super::*;

/// A one-appearance dark theme called `name` that sets `tokens`.
fn dark(name: &str, tokens: Value) -> Value {
    json!({
        "name": name,
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    })
}

fn put(folder: &Path, document: &Value) {
    std::fs::create_dir_all(folder).unwrap();
    std::fs::write(folder.join(THEME_JSON), document.to_string()).unwrap();
}

/// What breaks a hard floor and no consent floor: a light sunken ground under Classic's light
/// words. (A ground an ask or a state mark is drawn on is held by the consent floors first.)
fn unreadable() -> Value {
    json!({ "surface.sunken": "#f2f2f2" })
}

/// What the person is shown of a Keep of `draft`, as it stands now.
fn seen(places: &Places, draft: &Draft) -> drafts::Seen {
    drafts::Seen {
        kept_to: draft.kept_to.clone(),
        replaces: drafts::replaces(&places.plane, draft),
        changed: drafts::changed(&places.plane, draft),
    }
}

/// A config home and a plane with the workspaces `alpha` and `beta`.
struct Places {
    _home: tempfile::TempDir,
    config: PathBuf,
    plane: PathBuf,
}

impl Places {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config");
        let plane = home.path().join("plane");
        std::fs::create_dir_all(plane.join("workspaces/alpha")).unwrap();
        std::fs::create_dir_all(plane.join("workspaces/beta")).unwrap();
        std::fs::create_dir_all(plane.join(".purlis")).unwrap();
        std::fs::create_dir_all(&config).unwrap();
        Self {
            _home: home,
            config,
            plane,
        }
    }

    fn at<'a>(&'a self, workspace: Option<&'a str>) -> Place<'a> {
        Place {
            plane: &self.plane,
            workspace,
        }
    }

    fn workspace_themes(&self, name: &str) -> PathBuf {
        self.plane.join("workspaces").join(name).join("themes")
    }

    fn start(&self, workspace: Option<&str>, chat: &str, folder: &str) -> PathBuf {
        drafts::start(Some(&self.config), self.at(workspace), chat, folder)
            .unwrap()
            .path
    }

    fn drafts(&self, workspace: Option<&str>, chats: &[&str]) -> Vec<Draft> {
        drafts::at(Some(&self.config), self.at(workspace), chats)
    }
}

fn chat() -> String {
    crate::reopen::mint()
}

#[test]
fn purlis_classic_holds_every_hard_floor_in_every_appearance() {
    for one in check_every(classic()) {
        assert_eq!(one.hard, [], "{}", one.appearance);
    }
}

#[test]
fn a_light_window_under_light_words_breaks_the_primary_text_floor_and_says_which() {
    let theme = load(&dark("Glare", unreadable())).theme;
    let broken = drafts::broken(&theme);
    assert!(!broken.is_empty());
    assert!(
        broken
            .iter()
            .all(|one| one.broken.floor == HardFloor::PrimaryText),
        "{broken:?}"
    );
    let said = broken[0].said();
    assert!(
        said.starts_with(
            "primary text is readable on every surface: text.primary on surface.sunken"
        ),
        "{said}"
    );
    assert!(said.ends_with("(dark)"), "{said}");
}

#[test]
fn a_focus_ring_lost_on_the_window_breaks_its_floor() {
    let base = built_in_for(Appearance::Dark).value("surface.base");
    let theme = load(&dark("Lost", json!({ "focus.ring": base }))).theme;
    assert!(
        drafts::broken(&theme)
            .iter()
            .any(|one| one.broken.floor == HardFloor::FocusRing)
    );
}

#[test]
fn a_draft_lives_in_the_git_ignored_state_folder_named_by_its_chat() {
    let places = Places::new();
    let id = chat();
    let path = places.start(Some("alpha"), &id, "dusk");
    let state = crate::plane::state_dir(&places.plane);
    assert_eq!(path, state.join(drafts::DRAFTS).join(&id).join("dusk"));
    assert!(path.join(THEME_JSON).is_file());
    // Nothing in a workspace or the project's themes is written.
    assert!(!places.workspace_themes("alpha").exists());
    assert!(!places.plane.join("themes").exists());
}

#[test]
fn a_draft_is_drawn_only_where_its_chat_works() {
    let places = Places::new();
    let id = chat();
    places.start(Some("alpha"), &id, "dusk");
    let here = places.drafts(Some("alpha"), &[&id]);
    assert_eq!(
        here,
        [Draft {
            chat: id.clone(),
            folder: "dusk".into(),
            kept_to: KeptTo::Workspace("alpha".into()),
        }]
    );
    // Another workspace and the plane root have no chat of it, so no draft.
    assert_eq!(places.drafts(Some("beta"), &[]), []);
    assert_eq!(places.drafts(None, &[]), []);
}

#[test]
fn an_id_that_is_not_a_chat_s_has_no_drafts_and_starts_none() {
    let places = Places::new();
    assert_eq!(places.drafts(Some("alpha"), &["../alpha"]), []);
    assert!(drafts::start(Some(&places.config), places.at(None), "not-a-chat", "dusk").is_err());
}

#[test]
fn a_new_draft_extends_the_default_purlis_and_is_drawn() {
    let places = Places::new();
    let id = chat();
    let path = places.start(None, &id, "dusk");
    // A new theme starts as the default (#3034).
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(path.join(THEME_JSON)).unwrap()).unwrap();
    assert_eq!(written["extends"], DEFAULT);
    let draft = places.drafts(None, &[&id]).remove(0);
    let read = drafts::read(Some(&places.config), places.at(None), &draft, true);
    let drafted = drafts::drafted(read, None, None, Appearance::Dark, Contrast::Standard);
    assert!(drafted.broken.is_empty() && drafted.refused.is_empty());
    assert!(drafted.drawn.is_some() && drafted.good.is_some());
    // Drawn as Purlis, its parent, and not a fallback.
    let drawn = drafted.drawn.unwrap();
    let theme = drawn.theme.as_ref().expect("a theme is drawn");
    assert_eq!(
        theme.value("surface.base"),
        purlis().first().value("surface.base")
    );
    assert_eq!(theme.fonts, purlis().first().fonts);
}

#[test]
fn a_draft_starts_as_a_copy_of_the_folder_it_is_a_draft_of_and_is_kept_there() {
    let places = Places::new();
    put(
        &places.plane.join("themes/team"),
        &dark("Team", json!({ "accent.base": "#4477dd" })),
    );
    let id = chat();
    let path = places.start(Some("alpha"), &id, "team");
    let copied = std::fs::read_to_string(path.join(THEME_JSON)).unwrap();
    assert!(copied.contains("#4477dd"), "{copied}");
    let draft = places.drafts(Some("alpha"), &[&id]).remove(0);
    assert_eq!(draft.kept_to, KeptTo::Project);
}

#[test]
fn a_draft_that_breaks_a_hard_floor_is_not_drawn_and_the_last_good_version_stays() {
    let places = Places::new();
    let id = chat();
    let path = places.start(Some("alpha"), &id, "dusk");
    let draft = places.drafts(Some("alpha"), &[&id]).remove(0);
    let read = || {
        drafts::read(
            Some(&places.config),
            places.at(Some("alpha")),
            &draft,
            false,
        )
    };
    put(&path, &dark("Dusk", json!({ "accent.base": "#4477dd" })));
    let good = drafts::drafted(read(), None, None, Appearance::Dark, Contrast::Standard);
    let last = good.good.expect("drawn");
    put(&path, &dark("Dusk", unreadable()));
    let bad = drafts::drafted(
        read(),
        Some(&last),
        None,
        Appearance::Dark,
        Contrast::Standard,
    );
    assert!(!bad.broken.is_empty());
    let drawn = bad.drawn.expect("the last good version").theme.unwrap();
    assert_eq!(drawn.value("accent.base"), "#4477dd");
    assert_ne!(drawn.value("surface.sunken"), "#f2f2f2");
    // With no good version, nothing of the draft is drawn: the kept theme is.
    let first = drafts::drafted(read(), None, None, Appearance::Dark, Contrast::Standard);
    assert!(first.drawn.is_none() && !first.broken.is_empty());
}

#[test]
fn a_draft_is_drawn_with_the_consent_floors_held() {
    let places = Places::new();
    let id = chat();
    let path = places.start(None, &id, "dusk");
    let accent = built_in_for(Appearance::Dark).value("accent.base");
    put(&path, &dark("Dusk", json!({ "danger.base": accent })));
    let draft = places.drafts(None, &[&id]).remove(0);
    let read = drafts::read(Some(&places.config), places.at(None), &draft, true);
    let drawn = drafts::drafted(read, None, None, Appearance::Dark, Contrast::Standard)
        .drawn
        .unwrap();
    assert!(drawn.held.is_some());
    assert_ne!(drawn.theme.unwrap().value("danger.base"), accent);
}

#[test]
fn a_draft_that_is_not_a_theme_is_refused_and_says_why() {
    let places = Places::new();
    let id = chat();
    let path = places.start(None, &id, "dusk");
    std::fs::write(path.join(THEME_JSON), "not json").unwrap();
    let draft = places.drafts(None, &[&id]).remove(0);
    let read = drafts::read(Some(&places.config), places.at(None), &draft, true);
    let drafted = drafts::drafted(read, None, None, Appearance::Dark, Contrast::Standard);
    assert!(drafted.drawn.is_none());
    assert!(!drafted.refused.is_empty());
}

#[cfg(unix)]
#[test]
fn a_draft_folder_that_is_a_link_is_not_read() {
    let places = Places::new();
    let id = chat();
    let elsewhere = places.plane.join("elsewhere");
    put(&elsewhere, &dark("Elsewhere", json!({})));
    let chat_folder = drafts::folder_of(&places.plane, &id).unwrap();
    std::fs::create_dir_all(&chat_folder).unwrap();
    std::os::unix::fs::symlink(&elsewhere, chat_folder.join("dusk")).unwrap();
    let draft = Draft {
        chat: id.clone(),
        folder: "dusk".into(),
        kept_to: KeptTo::Project,
    };
    let read = drafts::read(Some(&places.config), places.at(None), &draft, true);
    let refused = match read {
        Ok(resolved) => resolved.refused,
        Err(_) => true,
    };
    assert!(refused);
    assert!(
        drafts::keep(
            Some(&places.config),
            places.at(None),
            &draft,
            &seen(&places, &draft)
        )
        .is_err()
    );
}

#[test]
fn revert_deletes_the_draft_and_nothing_else() {
    let places = Places::new();
    let id = chat();
    put(
        &places.workspace_themes("alpha").join("dusk"),
        &dark("Dusk", json!({})),
    );
    places.start(Some("alpha"), &id, "dusk");
    let draft = places.drafts(Some("alpha"), &[&id]).remove(0);
    drafts::revert(&places.plane, &draft).unwrap();
    assert_eq!(places.drafts(Some("alpha"), &[&id]), []);
    assert!(
        places
            .workspace_themes("alpha")
            .join("dusk/theme.json")
            .is_file()
    );
    // Twice is nothing more.
    drafts::revert(&places.plane, &draft).unwrap();
}

#[test]
fn keep_copies_the_draft_into_the_real_folder_and_deletes_it() {
    let places = Places::new();
    let id = chat();
    put(
        &places.workspace_themes("alpha").join("dusk"),
        &dark("Dusk", json!({})),
    );
    let path = places.start(Some("alpha"), &id, "dusk");
    put(&path, &dark("Dusk", json!({ "accent.base": "#4477dd" })));
    let draft = places.drafts(Some("alpha"), &[&id]).remove(0);
    assert!(drafts::replaces(&places.plane, &draft));
    // Keep does what the person was shown, or nothing: here, that it replaces the folder there.
    let shown_none = drafts::Seen {
        replaces: false,
        ..seen(&places, &draft)
    };
    let why = drafts::keep(
        Some(&places.config),
        places.at(Some("alpha")),
        &draft,
        &shown_none,
    )
    .unwrap_err();
    assert!(why.contains("has changed since you were shown it"), "{why}");
    let shown_project = drafts::Seen {
        kept_to: KeptTo::Project,
        ..seen(&places, &draft)
    };
    assert!(
        drafts::keep(
            Some(&places.config),
            places.at(Some("alpha")),
            &draft,
            &shown_project
        )
        .is_err()
    );
    let kept = drafts::keep(
        Some(&places.config),
        places.at(Some("alpha")),
        &draft,
        &seen(&places, &draft),
    )
    .unwrap();
    assert_eq!(kept.kept_to, KeptTo::Workspace("alpha".into()));
    let text =
        std::fs::read_to_string(places.workspace_themes("alpha").join("dusk/theme.json")).unwrap();
    assert!(text.contains("#4477dd"), "{text}");
    assert_eq!(places.drafts(Some("alpha"), &[&id]), []);
}

#[test]
fn a_draft_that_breaks_a_hard_floor_is_not_kept() {
    let places = Places::new();
    let id = chat();
    let path = places.start(None, &id, "dusk");
    put(&path, &dark("Dusk", unreadable()));
    let draft = places.drafts(None, &[&id]).remove(0);
    let why = drafts::keep(
        Some(&places.config),
        places.at(None),
        &draft,
        &seen(&places, &draft),
    )
    .unwrap_err();
    assert!(why.contains("hard floor"), "{why}");
    assert!(!places.plane.join("themes/dusk").exists());
}

#[test]
fn use_in_the_whole_project_moves_a_workspace_s_theme_up_and_never_over_one() {
    let places = Places::new();
    put(
        &places.workspace_themes("alpha").join("dusk"),
        &dark("Dusk", json!({})),
    );
    let moved = drafts::move_to_project(&places.plane, "alpha", "dusk").unwrap();
    assert_eq!(moved.path, places.plane.join("themes/dusk"));
    assert!(!places.workspace_themes("alpha").join("dusk").exists());
    put(
        &places.workspace_themes("alpha").join("dusk"),
        &dark("Dusk 2", json!({})),
    );
    let why = drafts::move_to_project(&places.plane, "alpha", "dusk").unwrap_err();
    assert!(why.contains("already"), "{why}");
    assert!(places.workspace_themes("alpha").join("dusk").exists());
}

#[test]
fn copy_to_my_machine_carries_only_the_font_files_approved_with_the_project() {
    let places = Places::new();
    let folder = places.plane.join("themes/dusk");
    put(&folder, &dark("Dusk", json!({})));
    std::fs::create_dir_all(folder.join("fonts")).unwrap();
    std::fs::write(folder.join("fonts/face.woff2"), b"wOF2 not approved").unwrap();
    let copied = drafts::copy_to_machine(&places.config, &places.plane, None, "dusk").unwrap();
    assert!(copied.path.join(THEME_JSON).is_file());
    assert!(!copied.path.join("fonts/face.woff2").exists());
    assert_eq!(copied.left_out.len(), 1, "{:?}", copied.left_out);
    // The project's folder stays where it is.
    assert!(folder.join(THEME_JSON).is_file());
    let why = drafts::copy_to_machine(&places.config, &places.plane, None, "dusk").unwrap_err();
    assert!(why.contains("already"), "{why}");
}

#[test]
fn the_watch_holds_the_drafts_of_the_chats_the_app_has_open_and_no_other() {
    use super::reload::{Reload, Watched};
    let places = Places::new();
    let (open, other) = (chat(), chat());
    // No draft yet: the nearest folder on the way to the drafts is watched for the next step.
    let watched = Watched::default().with_drafts(&drafts::watched(&places.plane, &[&open]));
    let made = places.plane.join(".purlis").join(drafts::DRAFTS);
    assert_eq!(
        watched.decide([made.as_path()], false),
        Reload::Redraw { survey: false }
    );
    let elsewhere = places.plane.join(".purlis/other");
    assert_eq!(
        watched.decide([elsewhere.as_path()], false),
        Reload::Nothing
    );
    // The open chat's draft is watched; a folder of another name is not looked into.
    let saved = places.start(Some("alpha"), &open, "dawn").join(THEME_JSON);
    places.start(Some("alpha"), &other, "dusk");
    let level = drafts::watched(&places.plane, &[&open]);
    assert_eq!(level.len(), 2);
    let watched = Watched::default().with_drafts(&level);
    assert_eq!(
        watched.decide([saved.as_path()], false),
        Reload::Redraw { survey: false }
    );
    assert!(
        watched
            .folders()
            .all(|folder| !folder.to_string_lossy().contains(&other)),
        "{:?}",
        watched.folders().collect::<Vec<_>>()
    );
}

#[test]
fn a_save_writes_the_draft_of_the_chat_the_app_names_and_says_what_is_drawn() {
    let places = Places::new();
    let (mine, theirs) = (chat(), chat());
    let theirs_path = places.start(Some("alpha"), &theirs, "dusk");
    put(
        &theirs_path,
        &dark("Theirs", json!({ "accent.base": "#4477dd" })),
    );
    let files = |document: &Value| {
        [(THEME_JSON.to_owned(), document.to_string())]
            .into_iter()
            .collect()
    };
    let saved = drafts::save(
        Some(&places.config),
        places.at(Some("alpha")),
        &mine,
        "dusk",
        &files(&dark("Mine", unreadable())),
    )
    .unwrap();
    assert_eq!(
        saved.path,
        drafts::folder_of(&places.plane, &mine)
            .unwrap()
            .join("dusk")
    );
    assert!(!saved.broken.is_empty(), "the window does not draw it");
    // The other chat's draft of the same name is as it was.
    let theirs_now = std::fs::read_to_string(theirs_path.join(THEME_JSON)).unwrap();
    assert!(theirs_now.contains("#4477dd"), "{theirs_now}");
    // A save hands over theme files, and nothing else.
    let mut more: std::collections::BTreeMap<String, String> = files(&dark("Mine", json!({})));
    more.insert("fonts/face.woff2".into(), "wOF2".into());
    assert!(
        drafts::save(
            Some(&places.config),
            places.at(Some("alpha")),
            &mine,
            "dusk",
            &more
        )
        .is_err()
    );
}

#[test]
fn keep_carries_only_the_font_files_the_theme_names_and_the_project_approved() {
    let places = Places::new();
    let id = chat();
    let path = places.start(None, &id, "dusk");
    put(&path, &dark("Dusk", json!({})));
    std::fs::create_dir_all(path.join("fonts")).unwrap();
    std::fs::write(path.join("fonts/stray.woff2"), b"wOF2 named by nothing").unwrap();
    let draft = places.drafts(None, &[&id]).remove(0);
    let kept = drafts::keep(
        Some(&places.config),
        places.at(None),
        &draft,
        &seen(&places, &draft),
    )
    .unwrap();
    assert!(kept.path.join(THEME_JSON).is_file());
    assert!(!kept.path.join("fonts/stray.woff2").exists());
}

/// A `theme.json` of `bytes` bytes as JSON writes it inside a save: a theme with a name that long.
fn theme_of_json_bytes(bytes: usize) -> std::collections::BTreeMap<String, String> {
    let empty = |name: &str| {
        let text = json!({ "name": name, "appearances": [{ "id": "dark", "appearance": "dark" }] })
            .to_string();
        [(THEME_JSON.to_owned(), text)]
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let base = serde_json::to_string(&empty("")).unwrap().len();
    let files = empty(&"a".repeat(bytes - base));
    assert_eq!(serde_json::to_string(&files).unwrap().len(), bytes);
    files
}

#[test]
fn a_save_carries_what_one_line_of_the_hook_socket_holds_and_no_more() {
    let places = Places::new();
    let id = chat();
    let at = drafts::MOST_SAVED_JSON_BYTES;
    assert_eq!(drafts::too_large(&theme_of_json_bytes(at)), None);
    let over = theme_of_json_bytes(at + 1);
    let why = drafts::too_large(&over).expect("refused");
    assert!(why.contains("a save carries at most"), "{why}");
    let refused =
        drafts::save(Some(&places.config), places.at(None), &id, "dusk", &over).unwrap_err();
    assert_eq!(refused, why);
    // The whole ask, at the most a save carries, fits one line with room to spare.
    #[cfg(unix)]
    {
        let ask = crate::hookwire::Ask::ThemeDraft(Box::new(crate::hookwire::ThemeDraftAsk {
            chat: u32::MAX,
            folder: "a".repeat(32),
            files: Some(theme_of_json_bytes(at)),
        }));
        let line = serde_json::to_string(&ask).unwrap().len() as u64;
        assert!(line + 1024 < crate::hookwire::A_LINE_IS_AT_MOST, "{line}");
    }
}

#[test]
fn keep_asks_when_the_real_folder_changed_since_the_draft_started() {
    let places = Places::new();
    let id = chat();
    let real = places.workspace_themes("alpha").join("dusk");
    put(&real, &dark("Dusk", json!({})));
    let path = places.start(Some("alpha"), &id, "dusk");
    put(&path, &dark("Dusk", json!({ "accent.base": "#4477dd" })));
    let draft = places.drafts(Some("alpha"), &[&id]).remove(0);
    // As it started: nothing to ask about.
    assert!(!drafts::changed(&places.plane, &draft));
    // The person's own edit (or a pull, or another chat) changes the folder there.
    put(&real, &dark("Dusk", json!({ "accent.base": "#dd7744" })));
    assert!(drafts::changed(&places.plane, &draft));
    // A Keep shown as over an unchanged folder is refused, and nothing is written.
    let unchanged = drafts::Seen {
        changed: false,
        ..seen(&places, &draft)
    };
    let why = drafts::keep(
        Some(&places.config),
        places.at(Some("alpha")),
        &draft,
        &unchanged,
    )
    .unwrap_err();
    assert!(why.contains("has changed since you were shown it"), "{why}");
    let there = std::fs::read_to_string(real.join(THEME_JSON)).unwrap();
    assert!(there.contains("#dd7744"), "{there}");
    // Shown that it changed, the person's Keep replaces it.
    drafts::keep(
        Some(&places.config),
        places.at(Some("alpha")),
        &draft,
        &seen(&places, &draft),
    )
    .unwrap();
    let kept = std::fs::read_to_string(real.join(THEME_JSON)).unwrap();
    assert!(kept.contains("#4477dd"), "{kept}");
}

#[test]
fn a_new_theme_s_draft_is_changed_once_a_folder_of_its_name_is_made_there() {
    let places = Places::new();
    let id = chat();
    places.start(Some("alpha"), &id, "dusk");
    let draft = places.drafts(Some("alpha"), &[&id]).remove(0);
    assert!(!drafts::changed(&places.plane, &draft));
    put(
        &places.workspace_themes("alpha").join("dusk"),
        &dark("Dusk", json!({})),
    );
    assert!(drafts::changed(&places.plane, &draft));
}

#[test]
fn a_draft_with_no_record_of_its_start_counts_as_changed() {
    let places = Places::new();
    let id = chat();
    places.start(None, &id, "dusk");
    let draft = places.drafts(None, &[&id]).remove(0);
    let chat_folder = drafts::folder_of(&places.plane, &id).unwrap();
    std::fs::remove_file(chat_folder.join(".dusk.started")).unwrap();
    assert!(drafts::changed(&places.plane, &draft));
}

#[test]
fn revert_deletes_the_record_of_the_start_with_the_draft() {
    let places = Places::new();
    let id = chat();
    places.start(None, &id, "dusk");
    let draft = places.drafts(None, &[&id]).remove(0);
    drafts::revert(&places.plane, &draft).unwrap();
    assert!(!drafts::folder_of(&places.plane, &id).unwrap().exists());
}

#[test]
fn a_sweep_deletes_the_drafts_of_chats_that_are_not_open_and_no_other() {
    let places = Places::new();
    let (open, gone) = (chat(), chat());
    places.start(Some("alpha"), &open, "dawn");
    places.start(Some("alpha"), &gone, "dusk");
    // A name that is not a chat's id is not purlis's to delete.
    let stray = drafts::folder_of(&places.plane, &open)
        .unwrap()
        .with_file_name("not-a-chat");
    std::fs::create_dir_all(&stray).unwrap();
    assert_eq!(drafts::sweep(&places.plane, &[&open]), 1);
    assert_eq!(places.drafts(Some("alpha"), &[&gone]), []);
    assert_eq!(places.drafts(Some("alpha"), &[&open]).len(), 1);
    assert!(stray.is_dir());
    // Nothing more to sweep.
    assert_eq!(drafts::sweep(&places.plane, &[&open]), 0);
}

#[cfg(unix)]
#[test]
fn a_sweep_never_deletes_through_a_link() {
    let places = Places::new();
    let elsewhere = places.plane.join("elsewhere");
    put(&elsewhere.join("dusk"), &dark("Elsewhere", json!({})));
    let linked = drafts::folder_of(&places.plane, &chat()).unwrap();
    std::fs::create_dir_all(linked.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &linked).unwrap();
    assert_eq!(drafts::sweep(&places.plane, &[]), 0);
    assert!(elsewhere.join("dusk").join(THEME_JSON).is_file());
}

#[test]
fn a_sweep_deletes_only_this_clone_s_drafts_in_a_drafts_folder_two_planes_share() {
    // Two clones whose state folder is one (`$PURLIS_HOME`): each chat's drafts folder names
    // the clone whose app made it, and a sweep deletes only its own clone's.
    let (one, two) = (Places::new(), Places::new());
    let (mine, theirs) = (chat(), chat());
    one.start(None, &mine, "dawn");
    two.start(None, &theirs, "dusk");
    // The other clone's chat folder, where the shared state folder holds it: beside this one's.
    let shared = drafts::folder_of(&one.plane, &mine)
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::rename(
        drafts::folder_of(&two.plane, &theirs).unwrap(),
        shared.join(&theirs),
    )
    .unwrap();
    assert_eq!(drafts::sweep(&one.plane, &[]), 1);
    assert!(
        !shared.join(&mine).exists(),
        "this clone's gone chat's drafts are deleted"
    );
    assert!(
        shared.join(&theirs).join("dusk").is_dir(),
        "the other clone's stay"
    );
    // The other clone sweeps its own, where it now is.
    assert_eq!(drafts::sweep(&two.plane, &[]), 0);
}

#[test]
fn a_sweep_adopts_an_ownerless_folder_inside_the_plane_and_deletes_it_once_its_chat_is_gone() {
    // A chat's drafts folder made before folders named their clone (#2708) is this plane's,
    // where the state folder is the plane's own: the sweep adopts it (#2761).
    let places = Places::new();
    let (open, other) = (chat(), chat());
    places.start(None, &other, "dawn");
    let folder = drafts::folder_of(&places.plane, &open).unwrap();
    put(&folder.join("noon"), &dark("Noon", json!({})));
    assert!(!folder.join(".owner").exists());
    let owner = std::fs::read_to_string(
        drafts::folder_of(&places.plane, &other)
            .unwrap()
            .join(".owner"),
    )
    .unwrap();
    // Its chat open: adopted, and kept.
    assert_eq!(drafts::sweep(&places.plane, &[&open, &other]), 0);
    assert!(folder.join("noon").join(THEME_JSON).is_file());
    assert_eq!(
        std::fs::read_to_string(folder.join(".owner")).unwrap(),
        owner,
        "it names this clone now"
    );
    // Its chat gone: deleted, as any of this clone's.
    assert_eq!(drafts::sweep(&places.plane, &[&other]), 1);
    assert!(!folder.exists());
}

#[cfg(unix)]
#[test]
fn a_sweep_adopts_no_folder_whose_owner_record_is_a_link() {
    let places = Places::new();
    let id = chat();
    let folder = drafts::folder_of(&places.plane, &id).unwrap();
    put(&folder.join("noon"), &dark("Noon", json!({})));
    let target = places.plane.join("elsewhere-owner");
    std::os::unix::fs::symlink(&target, folder.join(".owner")).unwrap();
    assert_eq!(drafts::sweep(&places.plane, &[]), 0);
    assert!(folder.join("noon").join(THEME_JSON).is_file());
    assert!(!target.exists(), "nothing is written through the link");
}

#[test]
fn a_chat_has_drafts_only_while_its_folder_holds_a_theme_folder() {
    let places = Places::new();
    let (drafting, none) = (chat(), chat());
    assert!(!drafts::has_drafts(&places.plane, &drafting));
    places.start(Some("alpha"), &drafting, "dusk");
    assert!(drafts::has_drafts(&places.plane, &drafting));
    assert!(!drafts::has_drafts(&places.plane, &none));
    assert!(!drafts::has_drafts(&places.plane, "not-a-chat"));
    let draft = places.drafts(Some("alpha"), &[&drafting]).remove(0);
    drafts::revert(&places.plane, &draft).unwrap();
    assert!(!drafts::has_drafts(&places.plane, &drafting));
}

#[cfg(unix)]
#[test]
fn a_chat_whose_drafts_folder_is_a_link_has_no_drafts() {
    let places = Places::new();
    let id = chat();
    let elsewhere = places.plane.join("elsewhere");
    put(&elsewhere.join("dusk"), &dark("Elsewhere", json!({})));
    let linked = drafts::folder_of(&places.plane, &id).unwrap();
    std::fs::create_dir_all(linked.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &linked).unwrap();
    assert!(!drafts::has_drafts(&places.plane, &id));
}

#[cfg(unix)]
#[test]
fn a_draft_folder_that_is_a_link_is_no_draft_of_its_chat() {
    let places = Places::new();
    let id = chat();
    let elsewhere = places.plane.join("elsewhere");
    put(&elsewhere, &dark("Elsewhere", json!({})));
    let chat_folder = drafts::folder_of(&places.plane, &id).unwrap();
    std::fs::create_dir_all(&chat_folder).unwrap();
    std::os::unix::fs::symlink(&elsewhere, chat_folder.join("dusk")).unwrap();
    assert!(!drafts::has_drafts(&places.plane, &id));
}

#[cfg(unix)]
#[test]
fn a_chat_has_no_drafts_through_a_link_on_the_way_to_its_folder() {
    let places = Places::new();
    let id = chat();
    let real = drafts::folder_of(&places.plane, &id).unwrap();
    let drafts_folder = real.parent().unwrap().to_path_buf();
    let elsewhere = places.plane.join("elsewhere-drafts");
    put(
        &elsewhere.join(&id).join("dusk"),
        &dark("Elsewhere", json!({})),
    );
    std::fs::create_dir_all(drafts_folder.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &drafts_folder).unwrap();
    assert!(!drafts::has_drafts(&places.plane, &id));
}

#[test]
fn a_chat_with_more_stray_entries_than_are_looked_at_counts_as_having_drafts() {
    // Where `has_drafts` cannot tell, the close asks: a draft listed after more stray files than
    // it looks at is never closed quietly, whatever order the folder lists them in.
    let places = Places::new();
    let (drafting, cluttered) = (chat(), chat());
    places.start(Some("alpha"), &drafting, "dusk");
    let folder = drafts::folder_of(&places.plane, &drafting).unwrap();
    for n in 0..150 {
        std::fs::write(folder.join(format!("stray-{n}.txt")), "x").unwrap();
    }
    assert!(drafts::has_drafts(&places.plane, &drafting));
    let only_strays = drafts::folder_of(&places.plane, &cluttered).unwrap();
    std::fs::create_dir_all(&only_strays).unwrap();
    for n in 0..=drafts::MOST_ENTRIES_LOOKED_AT {
        std::fs::write(only_strays.join(format!("stray-{n}.txt")), "x").unwrap();
    }
    assert!(drafts::has_drafts(&places.plane, &cluttered));
}

#[test]
fn a_chat_with_a_few_stray_files_and_no_theme_folder_has_no_drafts() {
    let places = Places::new();
    let id = chat();
    let folder = drafts::folder_of(&places.plane, &id).unwrap();
    std::fs::create_dir_all(&folder).unwrap();
    for n in 0..3 {
        std::fs::write(folder.join(format!("stray-{n}.txt")), "x").unwrap();
    }
    assert!(!drafts::has_drafts(&places.plane, &id));
}

#[test]
fn a_start_record_that_is_not_a_plain_file_counts_as_changed() {
    let places = Places::new();
    let id = chat();
    places.start(None, &id, "dusk");
    let draft = places.drafts(None, &[&id]).remove(0);
    let record = drafts::folder_of(&places.plane, &id)
        .unwrap()
        .join(".dusk.started");
    std::fs::remove_file(&record).unwrap();
    std::fs::create_dir(&record).unwrap();
    assert!(drafts::changed(&places.plane, &draft));
}

#[cfg(unix)]
#[test]
fn a_start_record_that_is_a_link_is_not_read() {
    let places = Places::new();
    let id = chat();
    places.start(None, &id, "dusk");
    let draft = places.drafts(None, &[&id]).remove(0);
    let record = drafts::folder_of(&places.plane, &id)
        .unwrap()
        .join(".dusk.started");
    let elsewhere = places.plane.join("elsewhere.started");
    std::fs::rename(&record, &elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &record).unwrap();
    assert!(drafts::changed(&places.plane, &draft));
}
