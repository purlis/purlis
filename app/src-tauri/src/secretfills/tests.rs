// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One notice for a secret a chat asks purlis to fill (#5115): a press answers exactly what it
//! was shown, Allow once is audited and writes nothing, Keep blocked fills nothing and isn't
//! asked again, and only Allow once and Keep blocked can be pressed.

use std::cell::RefCell;
use std::path::Path;

use purlis_session_protocol::browsing::FieldKind;

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
        persona: None,
        card: false,
        multi_line: false,
        line: None,
        // As the notice is offered to a chat that runs as no persona.
        no_always: Some(NO_PERSONA.to_owned()),
        vault_too: false,
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
fn a_cards_key_takes_allow_once_and_keep_blocked_alone() {
    let fills = Fills::default();
    let shown = FillWait {
        card: true,
        ..wait("https://shop.example.test")
    };
    let (id, heard) = fills.raise(shown.clone()).expect("raised");
    let audited = Audited::default();
    for not_offered in [Press::Chat, Press::Always] {
        let refused = answered(&fills, &audited, &press(id, not_offered, &shown.digest()))
            .expect_err("a card's key is allowed once alone");
        assert!(refused.contains("card"), "{refused}");
    }
    assert!(heard.try_recv().is_err(), "the call still waits");
    assert!(audited.into_inner().is_empty());
    assert!(!fills.granted(7, "qa", "dash-api-key", "https://shop.example.test"));
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
fn the_notice_names_the_key_the_vault_and_the_site_and_offers_once_this_chat_and_keep() {
    let dir = tempfile::tempdir().expect("a folder");
    let locks = purlis_core::sandbox::policy::Locks::of(dir.path());
    let mut one = wait("https://dash.example.test");
    one.id = 3;
    one.warnings = vec!["Vault qa isn't tagged for dev.".into()];

    let shown = crate::asking::secret_fill(&one, &locks);

    let options: Vec<&str> = shown.options.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(
        options,
        ["Allow once", "Allow for this chat", "Keep blocked"]
    );
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

const VAULTS: &str = r#"{"vaults":{
    "qa":{"provider":"plain-file","config":{"file":"q.json"},"persona":"qa"},
    "pay":{"provider":"plain-file","config":{"file":"p.json"},"persona":"qa"},
    "prod":{"provider":"plain-file","config":{"file":"o.json"},"persona":"ops"}
}}"#;

/// A project whose settings are `file`, with the vaults above.
fn project(file: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::write(dir.path().join("vaults.json"), VAULTS).expect("a registry");
    std::fs::write(purlis_core::names::manifest(dir.path()), file).expect("a project file");
    dir
}

fn file_of(root: &Path) -> String {
    std::fs::read_to_string(purlis_core::names::manifest(root)).expect("the project file")
}

/// A chat of persona qa's fill of `key` of vault `vault` on `origin`, as the notice offers it in
/// the project at `root`.
fn offered_in(root: &Path, vault: &str, key: &str, origin: &str) -> FillWait {
    offered(
        root,
        FillWait {
            persona: Some("qa".into()),
            vault: vault.into(),
            key: key.into(),
            no_always: None,
            ..wait(origin)
        },
    )
}

#[test]
fn allow_for_this_chat_holds_that_key_on_that_site_for_that_chat_alone() {
    let fills = Fills::default();
    let shown = wait("https://dash.example.test");
    let (id, heard) = fills.raise(shown.clone()).expect("raised");
    let audited = Audited::default();

    let pressed = answered(&fills, &audited, &press(id, Press::Chat, &shown.digest()))
        .expect("allowed for the chat");

    assert!(matches!(pressed, Pressed::Done { .. }), "{pressed:?}");
    assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
    assert!(fills.granted(7, "qa", "dash-api-key", "https://dash.example.test"));
    assert!(!fills.granted(7, "qa", "dash-api-key", "https://other.example.test"));
    assert!(!fills.granted(7, "qa", "another-key", "https://dash.example.test"));
    assert!(!fills.granted(8, "qa", "dash-api-key", "https://dash.example.test"));
    assert_eq!(
        audited.into_inner(),
        [(
            "secret-fill".to_owned(),
            "qa/dash-api-key -> https://dash.example.test".to_owned(),
            true
        )]
    );
    fills.forget(7);
    assert!(
        !fills.granted(7, "qa", "dash-api-key", "https://dash.example.test"),
        "it goes with the chat"
    );
}

#[test]
fn allow_for_this_chat_fills_the_next_ask_without_a_notice_and_never_a_cards() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let fills = Fills::default();
    fills.lock().granted.push((
        7,
        "qa".into(),
        "dash-api-key".into(),
        "https://dash.example.test".into(),
    ));
    let asked = |key: &str| FillAsked {
        browser: "b1".into(),
        target: "e12".into(),
        vault: "qa".into(),
        key: key.into(),
        field: purlis_session_protocol::browsing::FieldSaid {
            origin: Some("https://dash.example.test".into()),
            label: "API key".into(),
            kind: FieldKind::OneLine,
        },
        page: None,
    };
    let origin = "https://dash.example.test";
    assert!(granted(
        dir.path(),
        &fills,
        7,
        Some("qa"),
        &asked("dash-api-key"),
        origin
    ));
    assert!(!granted(
        dir.path(),
        &fills,
        8,
        Some("qa"),
        &asked("dash-api-key"),
        origin
    ));
    assert!(!granted(
        dir.path(),
        &fills,
        7,
        Some("qa"),
        &asked("other"),
        origin
    ));

    // A line of any persona's marking the key a card's asks each time, whatever was allowed.
    std::fs::write(
        purlis_core::names::manifest(dir.path()),
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.ops.tools.dash]\nvault = \"qa\"\nkey = \"dash-api-key\"\nfill = \"https://elsewhere.example.test\"\ncard = true\n",
    )
    .expect("a card's mark");
    assert!(!granted(
        dir.path(),
        &fills,
        7,
        Some("qa"),
        &asked("dash-api-key"),
        origin
    ));
}

#[test]
fn a_cards_key_is_offered_allow_once_alone_and_shows_no_line() {
    let dir = project(
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.qa.tools.visa]\nvault = \"pay\"\nkey = \"visa\"\nfill = \"https://shop.example.test\"\ncard = true\n",
    );
    let one = offered_in(dir.path(), "pay", "visa", "https://other-shop.example.test");
    assert!(one.card, "marked on every origin");
    let locks = purlis_core::sandbox::policy::Locks::of(dir.path());

    let shown = crate::asking::secret_fill(&one, &locks);

    let options: Vec<&str> = shown.options.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(options, ["Allow once", "Keep blocked"]);
    let fill = shown.fill.expect("a fill");
    assert!(fill.card);
    assert_eq!(fill.line, None);
}

#[test]
fn always_shows_the_line_it_writes_before_the_press() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let one = offered_in(
        dir.path(),
        "qa",
        "dash-api-key",
        "https://dash.example.test",
    );
    assert_eq!(one.no_always, None);
    let locks = purlis_core::sandbox::policy::Locks::of(dir.path());

    let shown = crate::asking::secret_fill(&one, &locks);

    let options: Vec<&str> = shown.options.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(
        options,
        [
            "Allow once",
            "Allow for this chat",
            "Always allow",
            "Keep blocked"
        ]
    );
    let fill = shown.fill.expect("a fill");
    assert_eq!(
        fill.line.as_deref(),
        Some(
            r#"dash-api-key = { vault = "qa", key = "dash-api-key", fill = "https://dash.example.test" }"#
        )
    );
    assert_eq!(fill.under.as_deref(), Some("[sandbox.personas.qa] tools"));
}

#[test]
fn always_writes_the_one_line_allows_it_here_and_the_next_fill_asks_nothing() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let fills = Fills::default();
    let shown = offered_in(
        dir.path(),
        "qa",
        "dash-api-key",
        "https://dash.example.test",
    );
    let (id, heard) = fills.raise(shown.clone()).expect("raised");
    let audited = Audited::default();

    let pressed = answered_in(
        dir.path(),
        &fills,
        &audited,
        &press(id, Press::Always, &shown.digest()),
    )
    .expect("allowed");

    assert!(matches!(pressed, Pressed::Done { .. }), "{pressed:?}");
    assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
    assert!(
        file_of(dir.path()).contains(r#"fill = "https://dash.example.test""#),
        "{}",
        file_of(dir.path())
    );
    let lines = purlis_core::secretfill::lines(
        &Ctx::new(dir.path(), Env::of(&[])),
        Some("qa"),
        "qa",
        "dash-api-key",
        "https://dash.example.test",
    );
    assert!(lines.always, "allowed here in the same press");
    assert!(
        !purlis_core::secretfill::lines(
            &Ctx::new(dir.path(), Env::of(&[])),
            Some("qa"),
            "qa",
            "dash-api-key",
            "https://other.example.test"
        )
        .always,
        "that origin alone"
    );
    assert!(
        audited
            .into_inner()
            .iter()
            .any(|(what, target, _)| what == "secret-fill"
                && target == "qa/dash-api-key -> https://dash.example.test")
    );
}

#[test]
fn always_is_held_back_by_proposed_changes_off_and_once_stays() {
    use purlis_core::sandbox::policy::{Locks, set_for_this_test};
    let dir = project("[sandbox]\nmode = \"on\"\n");
    set_for_this_test(Locks::parse(
        r#"{"sandbox": {"proposed-changes": "off"}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let fills = Fills::default();
    let shown = offered_in(
        dir.path(),
        "qa",
        "dash-api-key",
        "https://dash.example.test",
    );
    assert!(shown.no_always.is_some(), "{shown:?}");
    let options: Vec<String> = crate::asking::secret_fill(&shown, &Locks::of(dir.path()))
        .options
        .into_iter()
        .map(|o| o.label)
        .collect();
    assert!(!options.iter().any(|o| o == "Always allow"), "{options:?}");
    let (id, _heard) = fills.raise(shown.clone()).expect("raised");
    let audited = Audited::default();
    let before = file_of(dir.path());

    assert!(
        answered_in(
            dir.path(),
            &fills,
            &audited,
            &press(id, Press::Always, &shown.digest())
        )
        .is_err()
    );
    assert_eq!(file_of(dir.path()), before, "nothing written");
    assert!(audited.borrow().is_empty());
    answered_in(
        dir.path(),
        &fills,
        &audited,
        &press(id, Press::Once, &shown.digest()),
    )
    .expect("once stays");
}

#[test]
fn a_cards_mark_added_while_the_notice_is_up_draws_it_again_before_a_press_counts() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let fills = Fills::default();
    let shown = offered_in(
        dir.path(),
        "qa",
        "dash-api-key",
        "https://dash.example.test",
    );
    let (id, heard) = fills.raise(shown.clone()).expect("raised");
    std::fs::write(
        purlis_core::names::manifest(dir.path()),
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.qa.tools.dash]\nvault = \"qa\"\nkey = \"dash-api-key\"\nfill = \"https://dash.example.test\"\ncard = true\n",
    )
    .expect("marked");
    let audited = Audited::default();

    let pressed = answered_in(
        dir.path(),
        &fills,
        &audited,
        &press(id, Press::Chat, &shown.digest()),
    )
    .expect("drawn again");

    assert!(matches!(pressed, Pressed::DrawnAgain { .. }), "{pressed:?}");
    assert!(heard.try_recv().is_err(), "the call still waits");
    assert!(fills.waiting()[0].card);
    assert!(audited.into_inner().is_empty());
    assert!(!fills.granted(7, "qa", "dash-api-key", "https://dash.example.test"));
}

/// A fill of key `key` into a field of `kind` whose frame is on dash.example.test.
fn asked_into(key: &str, kind: FieldKind) -> FillAsked {
    FillAsked {
        browser: "b1".into(),
        target: "e12".into(),
        vault: "qa".into(),
        key: key.into(),
        field: purlis_session_protocol::browsing::FieldSaid {
            origin: Some("https://dash.example.test".into()),
            label: "Comment".into(),
            kind,
        },
        page: None,
    }
}

#[test]
fn a_multi_line_box_or_an_editable_area_asks_under_always_and_this_chat_and_a_one_line_field_does_not()
 {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let fills = Fills::default();
    let shown = offered_in(
        dir.path(),
        "qa",
        "dash-api-key",
        "https://dash.example.test",
    );
    let (id, _heard) = fills.raise(shown.clone()).expect("raised");
    answered_in(
        dir.path(),
        &fills,
        &Audited::default(),
        &press(id, Press::Always, &shown.digest()),
    )
    .expect("always");
    fills.lock().granted.push((
        7,
        "qa".into(),
        "dash-api-key".into(),
        "https://dash.example.test".into(),
    ));
    let origin = "https://dash.example.test";

    for kind in [FieldKind::MultiLine, FieldKind::Editable] {
        assert!(
            asks(
                dir.path(),
                &fills,
                7,
                Some("qa"),
                &asked_into("dash-api-key", kind),
                origin
            ),
            "{kind:?} asks each time"
        );
    }
    assert!(
        !asks(
            dir.path(),
            &fills,
            7,
            Some("qa"),
            &asked_into("dash-api-key", FieldKind::OneLine),
            origin
        ),
        "a one-line field under the same grant fills without asking"
    );
}

#[test]
fn a_multi_line_notice_takes_allow_once_and_keep_blocked_alone_and_says_so() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let one = offered(
        dir.path(),
        FillWait {
            persona: Some("qa".into()),
            multi_line: true,
            no_always: None,
            ..wait("https://dash.example.test")
        },
    );
    assert_eq!(
        one.line, None,
        "nothing to write for a field that asks each time"
    );
    let locks = purlis_core::sandbox::policy::Locks::of(dir.path());
    let shown = crate::asking::secret_fill(&one, &locks);
    let options: Vec<&str> = shown.options.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(options, ["Allow once", "Keep blocked"]);
    assert!(shown.fill.expect("a fill").multi_line);

    let fills = Fills::default();
    let (id, heard) = fills.raise(one.clone()).expect("raised");
    let audited = Audited::default();
    for not_offered in [Press::Chat, Press::Always] {
        assert!(
            answered_in(
                dir.path(),
                &fills,
                &audited,
                &press(id, not_offered, &one.digest())
            )
            .is_err()
        );
    }
    assert!(heard.try_recv().is_err(), "the call still waits");
    answered_in(
        dir.path(),
        &fills,
        &audited,
        &press(id, Press::Once, &one.digest()),
    )
    .expect("once");
    assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
    assert!(!fills.granted(7, "qa", "dash-api-key", "https://dash.example.test"));
}

#[test]
fn always_on_another_personas_vault_says_so_and_allows_that_vault_too() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let fills = Fills::default();
    let shown = offered_in(
        dir.path(),
        "prod",
        "deploy-token",
        "https://dash.example.test",
    );
    assert!(shown.vault_too, "{shown:?}");
    let locks = purlis_core::sandbox::policy::Locks::of(dir.path());
    assert!(
        crate::asking::secret_fill(&shown, &locks)
            .fill
            .expect("a fill")
            .vault_too
    );
    let (id, _heard) = fills.raise(shown.clone()).expect("raised");

    answered_in(
        dir.path(),
        &fills,
        &Audited::default(),
        &press(id, Press::Always, &shown.digest()),
    )
    .expect("allowed");

    assert!(
        purlis_core::sandbox::local::granted_vaults(dir.path())
            .iter()
            .any(|grant| grant.vault == "prod" && grant.persona == "qa")
    );
}

#[test]
fn a_site_on_this_machine_is_asked_about_each_time() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let fills = Fills::default();
    fills.lock().granted.push((
        7,
        "qa".into(),
        "dash-api-key".into(),
        "http://localhost:3000".into(),
    ));
    let mut asked = asked_into("dash-api-key", FieldKind::OneLine);
    asked.field.origin = Some("http://localhost:3000".into());
    assert!(asks(
        dir.path(),
        &fills,
        7,
        Some("qa"),
        &asked,
        "http://localhost:3000"
    ));
    let one = offered_in(dir.path(), "qa", "dash-api-key", "http://localhost:3000");
    let options: Vec<String> =
        crate::asking::secret_fill(&one, &purlis_core::sandbox::policy::Locks::of(dir.path()))
            .options
            .into_iter()
            .map(|o| o.label)
            .collect();
    assert_eq!(options, ["Allow once", "Keep blocked"]);
}

#[test]
fn allow_once_isnt_drawn_again_for_a_change_only_always_would_write() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let fills = Fills::default();
    let shown = offered_in(
        dir.path(),
        "qa",
        "dash-api-key",
        "https://dash.example.test",
    );
    let (id, heard) = fills.raise(shown.clone()).expect("raised");
    // Another tool takes the name the line would have had.
    std::fs::write(
        purlis_core::names::manifest(dir.path()),
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.qa.tools.dash-api-key]\nvault = \"qa\"\nkey = \"other\"\nfill = \"https://elsewhere.example.test\"\n",
    )
    .expect("changed");

    let pressed = answered_in(
        dir.path(),
        &fills,
        &Audited::default(),
        &press(id, Press::Once, &shown.digest()),
    )
    .expect("allowed once");

    assert!(matches!(pressed, Pressed::Done { .. }), "{pressed:?}");
    assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
}
