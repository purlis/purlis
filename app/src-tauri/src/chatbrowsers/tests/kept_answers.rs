// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the person answered survives a restart (#7190): a chat's posture answer for that chat's
//! life, and a pool login opt-in on this machine. "A restart" is a second `ChatBrowsers` over the
//! same folders, as after a quit.

use super::*;
use purlis_core::poollogins::Bound;

const CHAT: &str = "01JA0POSTURE0000000000000A";
const ANOTHER: &str = "01JA0POSTURE0000000000000B";

/// The browsers of a window over `root`'s state and `config`, chat 3 known as `chat`.
fn window(root: &Path, config: &Path, chat: &str) -> ChatBrowsers {
    let browsers = ChatBrowsers::default().keeping_answers(root, Some(config));
    browsers.identify(3, Some(chat.into()));
    browsers
}

fn asked(browsers: &ChatBrowsers, hosted: &Arc<Hosted>) -> bool {
    answered(browsers, hosted, &ask(3, click())).1
}

fn bound(definition: &str, image: &str) -> Bound {
    Bound {
        pool: "builds".into(),
        definition: definition.into(),
        image: image.into(),
        runner: "01JB0RUNNER0000000000000000".into(),
        runner_key: "ab".repeat(32),
        runner_image: image.into(),
        pod: String::new(),
        engine: "this machine's Docker".into(),
    }
}

#[test]
fn a_chats_answer_survives_a_restart_and_the_chat_is_not_asked_again() {
    let (root, config) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hosted = Arc::new(Hosted::default());
    let first = window(root.path(), config.path(), CHAT);
    assert!(asked(&first, &hosted));
    assert!(first.allow(3, true));

    let restarted = window(root.path(), config.path(), CHAT);

    assert_eq!(restarted.posture(3), Posture::Allowed { open_web: true });
    assert!(!asked(&restarted, &hosted), "no Ask: the chat's answer stands");
    assert_eq!(hosted.started.lock().unwrap().len(), 1, "its browser starts");
}

#[test]
fn a_kept_off_answer_survives_too() {
    let (root, config) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hosted = Arc::new(Hosted::default());
    let first = window(root.path(), config.path(), CHAT);
    asked(&first, &hosted);
    assert!(first.keep_off(3));

    assert_eq!(window(root.path(), config.path(), CHAT).posture(3), Posture::Off);
}

#[test]
fn another_chat_even_one_that_takes_the_same_number_still_asks() {
    let (root, config) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hosted = Arc::new(Hosted::default());
    let first = window(root.path(), config.path(), CHAT);
    asked(&first, &hosted);
    first.allow(3, false);

    let new_chat = window(root.path(), config.path(), ANOTHER);

    assert_eq!(new_chat.posture(3), Posture::Unasked);
    assert!(asked(&new_chat, &hosted), "a new chat asks");
}

#[test]
fn an_answer_given_before_the_chats_identity_was_known_is_kept_once_it_is() {
    let (root, config) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hosted = Arc::new(Hosted::default());
    let first = ChatBrowsers::default().keeping_answers(root.path(), Some(config.path()));
    asked(&first, &hosted);
    first.allow(3, false);
    assert!(!purlis_core::browserposture::path(root.path()).exists());

    first.identify(3, Some(CHAT.into()));

    assert_eq!(
        window(root.path(), config.path(), CHAT).posture(3),
        Posture::Allowed { open_web: false }
    );
}

#[test]
fn the_answer_ends_with_the_chat() {
    let (root, config) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hosted = Arc::new(Hosted::default());
    let first = window(root.path(), config.path(), CHAT);
    asked(&first, &hosted);
    first.allow(3, false);
    let driver: Arc<dyn Driver> = hosted.clone();

    first.forget(Some(&driver), 3, "Chat 3");

    assert_eq!(window(root.path(), config.path(), CHAT).posture(3), Posture::Unasked);
    assert!(
        !purlis_core::browserposture::path(root.path()).exists(),
        "nothing of it is left"
    );
}

#[test]
fn narrowing_a_chats_reach_still_ends_its_browsers_and_the_kept_answer_stands() {
    let (root, config) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hosted = Arc::new(Hosted::default());
    let first = window(root.path(), config.path(), CHAT);
    asked(&first, &hosted);
    first.allow(3, false);
    assert!(!asked(&first, &hosted));
    assert_eq!(hosted.started.lock().unwrap().len(), 1);
    let driver: Arc<dyn Driver> = hosted.clone();

    let ended = first.narrowed(Some(&driver), 3);

    assert_eq!(ended.ended, 1, "its browser ends");
    assert_eq!(hosted.closed.lock().unwrap().len(), 1);
    assert_eq!(
        window(root.path(), config.path(), CHAT).posture(3),
        Posture::Allowed { open_web: false }
    );
}

#[test]
fn an_answer_that_rests_on_a_personas_always_is_not_kept() {
    let (root, config) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hosted = Arc::new(Hosted::default());
    let first = window(root.path(), config.path(), CHAT);
    let as_persona = as_persona("steward", None, None);
    answered_as(&first, &hosted, &ask(3, click()), &as_persona);
    assert_eq!(first.allow_always("steward", false), [3]);
    first.identify(3, Some(CHAT.into()));

    assert_eq!(
        window(root.path(), config.path(), CHAT).posture(3),
        Posture::Unasked,
        "Always is followed as it is now, never copied"
    );
    assert!(!purlis_core::browserposture::path(root.path()).exists());
}

#[test]
fn what_is_kept_is_written_under_this_machines_state_and_config_never_the_plane() {
    let (root, config) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let hosted = Arc::new(Hosted::default());
    let first = window(root.path(), config.path(), CHAT);
    asked(&first, &hosted);
    first.allow(3, true);
    let _ = first.pool_login(3, "steward", &bound("d1", "i1"), "shop", "id-shop", false, 1);
    first.answer_pool_login(3, true);

    let mut written = Vec::new();
    for folder in [root.path(), config.path()] {
        let mut walk = vec![folder.to_path_buf()];
        while let Some(dir) = walk.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                match entry.file_type().unwrap().is_dir() {
                    true => walk.push(entry.path()),
                    false => written.push(entry.path()),
                }
            }
        }
    }
    written.sort();
    let mut expected = vec![
        purlis_core::browserposture::path(root.path()),
        config.path().join(purlis_core::poollogins::IN_CONFIG),
    ];
    expected.sort();
    assert_eq!(
        written, expected,
        "only this clone's state folder and this machine's config folder, nothing of the plane"
    );
}

#[test]
fn a_pool_login_opt_in_survives_a_restart_and_a_changed_pool_asks_again() {
    let (root, config) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let first = window(root.path(), config.path(), CHAT);
    first
        .pool_login(3, "steward", &bound("d1", "i1"), "shop", "id-shop", false, 1)
        .unwrap_err();
    first.answer_pool_login(3, true);

    let restarted = window(root.path(), config.path(), CHAT);

    assert_eq!(
        restarted.pool_login(3, "steward", &bound("d1", "i1"), "shop", "id-shop", false, 2),
        Ok(()),
        "no Ask"
    );
    let (why, raised) = restarted
        .pool_login(3, "steward", &bound("d2", "i1"), "shop", "id-shop", false, 3)
        .unwrap_err();
    assert!(raised, "a changed definition asks again: {why}");
    let (_, raised) = restarted
        .pool_login(4, "steward", &bound("d1", "i2"), "shop", "id-shop", false, 4)
        .unwrap_err();
    assert!(raised, "a changed image asks again");
}

#[test]
fn the_granted_list_names_an_opt_in_and_remove_makes_the_next_login_ask() {
    let (root, config) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let first = window(root.path(), config.path(), CHAT);
    first
        .pool_login(3, "steward", &bound("d1", "i1"), "shop", "id-shop", false, 1)
        .unwrap_err();
    first.answer_pool_login(3, true);

    let restarted = window(root.path(), config.path(), CHAT);
    assert_eq!(restarted.pool_login_grants().len(), 1);
    assert!(restarted.remove_pool_login_grant("steward", "id-shop", "builds"));

    assert!(restarted.pool_login_grants().is_empty());
    let after = window(root.path(), config.path(), CHAT);
    let (_, raised) = after
        .pool_login(3, "steward", &bound("d1", "i1"), "shop", "id-shop", false, 5)
        .unwrap_err();
    assert!(raised, "removed: the next login asks");
}

#[test]
fn a_runners_browser_is_not_among_the_addresses_a_quit_keeps_because_it_outlives_the_quit() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    let driver: Arc<dyn Driver> = hosted.clone();
    *hosted.at.lock().unwrap() = Some("https://on-the-runner.example.test/".into());
    let on_builds = |browse: &BrowseAsk| {
        browsers.answer(
            Some(&driver),
            Path::new("/p"),
            Some("builds"),
            browse,
            &|| reach(),
            false,
            &AsPersona::default,
            &logins,
            &no_fills,
            &|| Err("no folder here".to_owned()),
            1000,
        )
    };
    on_builds(&ask(3, click()));
    browsers.allow(3, false);
    on_builds(&ask(3, click()));
    *hosted.at.lock().unwrap() = Some("https://on-this-machine.example.test/".into());
    allowed_and_acting(&browsers, &hosted, 4);

    assert_eq!(
        browsers.open_addresses(),
        ["https://on-this-machine.example.test/"],
        "this machine's browser ends with the quit and is offered; the runner's runs on"
    );
}
