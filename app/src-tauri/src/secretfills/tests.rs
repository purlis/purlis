// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One notice for a secret a chat asks purlis to fill (#5115): a press answers exactly what it
//! was shown, Allow once is audited and writes nothing, Keep blocked fills nothing and isn't
//! asked again, and only Allow once and Keep blocked can be pressed.

use std::cell::RefCell;
use std::path::Path;

use super::*;

fn wait(origin: &str) -> FillWait {
    FillWait {
        id: 0,
        session: 7,
        browser: "b1".into(),
        vault: "qa".into(),
        key: "dash-api-key".into(),
        origin: origin.into(),
        page: Some(format!("{origin}/settings")),
        label: "API key".into(),
        warnings: Vec::new(),
        since: 1,
    }
}

fn press(id: u32, press: Press, shown: &str) -> Asked<'_> {
    Asked {
        session: 7,
        id,
        press,
        shown,
        reason: None,
    }
}

type Audited = RefCell<Vec<(String, String, bool)>>;

fn answered(fills: &Fills, audited: &Audited, asked: &Asked<'_>) -> Result<Pressed, String> {
    let dir = tempfile::tempdir().expect("a folder");
    answered_in(dir.path(), fills, audited, asked)
}

fn answered_in(
    root: &Path,
    fills: &Fills,
    audited: &Audited,
    asked: &Asked<'_>,
) -> Result<Pressed, String> {
    answer(root, fills, asked, &|_, one| {
        audited
            .borrow_mut()
            .push((one.what.to_owned(), one.target.to_owned(), one.granted));
        Ok(())
    })
}

#[test]
fn allow_once_tells_the_waiting_call_and_is_audited_naming_the_key_and_the_site() {
    let fills = Fills::default();
    let shown = wait("https://dash.example.test");
    let (id, heard) = fills.raise(shown.clone()).expect("raised");
    let audited = Audited::default();

    let pressed =
        answered(&fills, &audited, &press(id, Press::Once, &shown.digest())).expect("allowed");

    assert!(matches!(pressed, Pressed::Done { .. }), "{pressed:?}");
    assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
    assert_eq!(
        audited.into_inner(),
        [(
            "secret-fill".to_owned(),
            "qa/dash-api-key -> https://dash.example.test".to_owned(),
            true
        )]
    );
    assert!(fills.waiting().is_empty(), "the notice went");
}

#[test]
fn keep_blocked_tells_the_waiting_call_and_that_key_on_that_site_isnt_asked_again() {
    let fills = Fills::default();
    let shown = wait("https://dash.example.test");
    let (id, heard) = fills.raise(shown.clone()).expect("raised");
    let audited = Audited::default();

    answered(&fills, &audited, &press(id, Press::Keep, &shown.digest())).expect("kept");

    assert_eq!(heard.try_recv(), Ok(Heard::Kept(None)));
    assert!(audited.into_inner().is_empty(), "nothing was allowed");
    assert!(fills.kept(7, "qa", "dash-api-key", "https://dash.example.test"));
    assert!(
        !fills.kept(7, "qa", "dash-api-key", "https://other.example.test"),
        "another site is asked about"
    );
    assert!(!fills.kept(8, "qa", "dash-api-key", "https://dash.example.test"));
}

#[test]
fn two_kept_blocked_end_a_chats_fill_notices() {
    let fills = Fills::default();
    let audited = Audited::default();
    for origin in ["https://a.example.test", "https://b.example.test"] {
        let shown = wait(origin);
        let (id, _heard) = fills.raise(shown.clone()).expect("raised");
        answered(&fills, &audited, &press(id, Press::Keep, &shown.digest())).expect("kept");
    }
    assert!(fills.kept(7, "qa", "another-key", "https://c.example.test"));
}

#[test]
fn a_press_on_a_notice_drawn_otherwise_allows_nothing() {
    let fills = Fills::default();
    let shown = wait("https://dash.example.test");
    let (id, heard) = fills.raise(shown).expect("raised");
    let audited = Audited::default();
    let other = wait("https://evil.example.test").digest();

    let refused = answered(&fills, &audited, &press(id, Press::Once, &other)).unwrap_err();

    assert!(refused.contains("changed"), "{refused}");
    assert!(heard.try_recv().is_err(), "the call still waits");
    assert!(audited.into_inner().is_empty());
}

#[test]
fn only_allow_once_and_keep_blocked_can_be_pressed() {
    let fills = Fills::default();
    let shown = wait("https://dash.example.test");
    let (id, heard) = fills.raise(shown.clone()).expect("raised");
    let audited = Audited::default();
    for not_offered in [Press::Chat, Press::Always] {
        assert!(answered(&fills, &audited, &press(id, not_offered, &shown.digest())).is_err());
    }
    assert!(heard.try_recv().is_err(), "the call still waits");
    assert!(audited.into_inner().is_empty());
}

#[test]
fn allow_once_is_refused_where_policy_removes_that_scope() {
    use purlis_core::sandbox::policy::{Locks, set_for_this_test};
    let dir = tempfile::tempdir().expect("a folder");
    set_for_this_test(Locks::parse(
        r#"{"sandbox": {"allow-scopes": ["chat", "you"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let fills = Fills::default();
    let shown = wait("https://dash.example.test");
    let (id, heard) = fills.raise(shown.clone()).expect("raised");
    let audited = Audited::default();

    let refused = answered_in(
        dir.path(),
        &fills,
        &audited,
        &press(id, Press::Once, &shown.digest()),
    );

    assert!(refused.is_err(), "{refused:?}");
    assert!(heard.try_recv().is_err());
    assert!(audited.into_inner().is_empty());
}

#[test]
fn one_notice_waits_for_each_chat() {
    let fills = Fills::default();
    fills
        .raise(wait("https://dash.example.test"))
        .expect("raised");
    assert!(fills.raise(wait("https://other.example.test")).is_err());
}

#[test]
fn a_chat_that_closes_takes_its_notice_and_its_kept_with_it() {
    let fills = Fills::default();
    let (_, heard) = fills
        .raise(wait("https://dash.example.test"))
        .expect("raised");
    fills.lock().kept.push((
        7,
        "qa".into(),
        "k".into(),
        "https://dash.example.test".into(),
    ));

    fills.forget(7);

    assert_eq!(heard.try_recv(), Ok(Heard::Gone));
    assert!(fills.waiting().is_empty());
    assert!(!fills.kept(7, "qa", "k", "https://dash.example.test"));
}

#[test]
fn the_notice_names_the_key_the_vault_and_the_site_and_offers_once_and_keep() {
    let dir = tempfile::tempdir().expect("a folder");
    let locks = purlis_core::sandbox::policy::Locks::of(dir.path());
    let mut one = wait("https://dash.example.test");
    one.id = 3;
    one.warnings = vec!["Vault qa isn't tagged for dev.".into()];

    let shown = crate::asking::secret_fill(&one, &locks);

    let options: Vec<&str> = shown.options.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(options, ["Allow once", "Keep blocked"]);
    let fill = shown.fill.expect("drawn as a fill");
    assert_eq!(fill.site, "dash.example.test");
    assert_eq!(fill.key, "dash-api-key");
    assert_eq!(fill.vault, "qa");
    assert_eq!(fill.label, "API key");
    assert_eq!(fill.warnings, ["Vault qa isn't tagged for dev."]);
    assert_eq!(
        shown.answer,
        crate::asking::AnswerPath::SecretFill {
            id: 3,
            shown: one.digest()
        }
    );
    // A site that isn't https says its scheme.
    let local = crate::asking::secret_fill(&wait("http://localhost:3000"), &locks);
    assert_eq!(local.fill.expect("a fill").site, "http://localhost:3000");
}

#[test]
fn a_field_in_a_frame_of_another_site_than_its_page_is_warned_of() {
    let warned = frame_warning(
        "https://pay.example.test",
        Some("https://shop.example.test/checkout"),
    )
    .expect("warned");
    assert!(warned.contains("https://pay.example.test"), "{warned}");
    assert!(warned.contains("https://shop.example.test"), "{warned}");
    assert_eq!(
        frame_warning(
            "https://shop.example.test",
            Some("https://shop.example.test/a")
        ),
        None
    );
    assert_eq!(frame_warning("https://shop.example.test", None), None);
}

/// #5122, SF-12: a fill into a browser on a pool's runner warns that the value goes to that
/// machine, naming the pool.
#[test]
fn a_fill_on_a_runners_browser_warns_that_the_secret_goes_to_that_runner() {
    let warned = runner_warning("builds");

    assert!(warned.contains("pool builds's runner"), "{warned}");
    assert!(warned.contains("goes to that machine"), "{warned}");
}
