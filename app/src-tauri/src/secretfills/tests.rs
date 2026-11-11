// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One notice for a secret a chat asks purlis to fill (#5115): a press answers exactly what it
//! was shown, Allow once is audited and writes nothing, Keep blocked fills nothing and isn't
//! asked again, and only Allow once and Keep blocked can be pressed.

use std::cell::RefCell;
use std::path::Path;

use purlis_session_protocol::browsing::FieldKind;

use super::*;

/// Chat `session`, as the app knows it: its number and the id its record has (ADR 0066).
fn chat(session: u32) -> Chat<'static> {
    Chat {
        session,
        id: match session {
            7 => Some("01JX0000000000000000000007"),
            8 => Some("01JX0000000000000000000008"),
            _ => None,
        },
    }
}

/// The id of chat `session`, as a grant keeps it.
fn chat_id(session: u32) -> Option<String> {
    chat(session).id.map(str::to_owned)
}

fn wait(origin: &str) -> FillWait {
    FillWait {
        id: 0,
        session: 7,
        chat: chat_id(7),
        browser: "b1".into(),
        vault: "qa".into(),
        key: "dash-api-key".into(),
        origin: origin.into(),
        page: Some(format!("{origin}/settings")),
        label: "API key".into(),
        warnings: Vec::new(),
        code: false,
        since: 1,
        persona: None,
        card: false,
        payment: false,
        multi_line: false,
        line: None,
        // As the notice is offered to a chat that runs as no persona.
        no_always: Some(NO_PERSONA.to_owned()),
        vault_too: false,
        reads_as: None,
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
    assert!(fills.kept(7, None, "qa", "dash-api-key", "https://dash.example.test"));
    assert!(
        !fills.kept(7, None, "qa", "dash-api-key", "https://other.example.test"),
        "another site is asked about"
    );
    assert!(!fills.kept(8, None, "qa", "dash-api-key", "https://dash.example.test"));
}

/// The person's ruling of 2026-11-07: a persona's Keep blocked holds for all its chats until
/// purlis restarts, so a new chat starts no fresh tries.
#[test]
fn keep_blocked_holds_for_every_chat_of_the_persona_after_its_chat_closes() {
    let fills = Fills::default();
    let shown = FillWait {
        persona: Some("qa".into()),
        ..wait("https://dash.example.test")
    };
    let (id, _heard) = fills.raise(shown.clone()).expect("raised");
    answered(
        &fills,
        &Audited::default(),
        &press(id, Press::Keep, &shown.digest()),
    )
    .expect("kept");
    fills.forget(7);

    let key = ("qa", "dash-api-key", "https://dash.example.test");
    assert!(fills.kept(8, Some("qa"), key.0, key.1, key.2));
    assert!(!fills.kept(8, Some("ops"), key.0, key.1, key.2));
    assert!(!fills.kept(8, None, key.0, key.1, key.2));
}

#[test]
fn two_kept_blocked_end_a_personas_fill_notices_in_every_chat() {
    let fills = Fills::default();
    let audited = Audited::default();
    for origin in ["https://a.example.test", "https://b.example.test"] {
        let shown = FillWait {
            persona: Some("qa".into()),
            ..wait(origin)
        };
        let (id, _heard) = fills.raise(shown.clone()).expect("raised");
        answered(&fills, &audited, &press(id, Press::Keep, &shown.digest())).expect("kept");
    }
    fills.forget(7);
    assert!(fills.kept(9, Some("qa"), "qa", "another-key", "https://c.example.test"));
    assert!(!fills.kept(
        9,
        Some("ops"),
        "qa",
        "another-key",
        "https://c.example.test"
    ));
}

#[test]
fn two_kept_blocked_end_a_chats_fill_notices_where_it_runs_as_no_persona() {
    let fills = Fills::default();
    let audited = Audited::default();
    for origin in ["https://a.example.test", "https://b.example.test"] {
        let shown = wait(origin);
        let (id, _heard) = fills.raise(shown.clone()).expect("raised");
        answered(&fills, &audited, &press(id, Press::Keep, &shown.digest())).expect("kept");
    }
    assert!(fills.kept(7, None, "qa", "another-key", "https://c.example.test"));
    assert!(!fills.kept(8, None, "qa", "another-key", "https://c.example.test"));
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
    assert!(!fills.granted(chat(7), "qa", "dash-api-key", "https://shop.example.test"));
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
fn a_chat_with_no_persona_that_closes_takes_its_notice_and_its_kept_with_it() {
    let fills = Fills::default();
    let (_, heard) = fills
        .raise(wait("https://dash.example.test"))
        .expect("raised");
    fills.lock().kept.push((
        Whose::Chat(7),
        "qa".into(),
        "k".into(),
        "https://dash.example.test".into(),
    ));

    fills.forget(7);

    assert_eq!(heard.try_recv(), Ok(Heard::Gone));
    assert!(fills.waiting().is_empty());
    assert!(!fills.kept(7, None, "qa", "k", "https://dash.example.test"));
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
    // **A vault use** (#6178): said on its row, and its Allow asks once more.
    let risk = shown.risk.clone().expect("a vault use");
    assert_eq!(risk.class, crate::asking::RiskClass::VaultUse);
    assert_eq!(
        risk.why,
        "Hands dash-api-key from vault qa to a field on dash.example.test"
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

/// #5122, SF-12: a fill into a browser on a pool's runner warns that the value goes to that
/// machine, naming the pool.
#[test]
fn a_fill_on_a_runners_browser_warns_that_the_secret_goes_to_that_runner() {
    let warned = runner_warning("builds");

    assert!(warned.contains("pool builds's runner"), "{warned}");
    assert!(warned.contains("goes to that machine"), "{warned}");
}

/// #5122, ADR 0089: a fill on a runner is asked about only where the runner is shown to be the
/// person's own, for a chat in its sandbox there, and refused in words before anyone is asked
/// otherwise.
#[test]
fn a_fill_on_a_runner_not_shown_to_be_the_persons_own_is_refused_before_anyone_is_asked() {
    let config = tempfile::tempdir().unwrap();

    let unknown = runner_bound(false, Some(config.path()), "builds", None);
    let sandboxless = runner_bound(true, Some(config.path()), "builds", None);
    let no_pools = runner_bound(false, None, "builds", None);

    for refused in [unknown, sandboxless, no_pools] {
        let why = refused.expect_err("refused");
        assert!(why.contains("builds"), "{why}");
    }
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
fn a_2fa_codes_notice_says_so_and_a_press_on_the_values_notice_never_allows_it() {
    let dir = tempfile::tempdir().expect("a folder");
    let locks = purlis_core::sandbox::policy::Locks::of(dir.path());
    let mut code = wait("https://dash.example.test");
    code.key = "dash-2fa".into();
    code.code = true;

    let shown = crate::asking::secret_fill(&code, &locks);
    assert!(shown.fill.expect("a fill").code);
    assert!(
        shown.says.contains("2FA code from dash-2fa"),
        "{}",
        shown.says
    );

    // The value's notice and the code's are told apart by what a press names.
    let fills = Fills::default();
    let (id, heard) = fills.raise(code.clone()).expect("raised");
    let audited = Audited::default();
    let mut value = code.clone();
    value.code = false;
    let refused = answered(&fills, &audited, &press(id, Press::Once, &value.digest()));
    assert!(refused.is_err(), "{refused:?}");
    assert!(heard.try_recv().is_err(), "the call still waits");

    let pressed =
        answered(&fills, &audited, &press(id, Press::Once, &code.digest())).expect("allowed");
    let Pressed::Done { said } = pressed else {
        panic!("{pressed:?}");
    };
    assert!(said.contains("A 2FA code from dash-2fa"), "{said}");
    assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
    // Audited as the seed's approval for that one site, as any key's is (SF-11).
    assert_eq!(
        audited.into_inner(),
        vec![(
            "secret-fill".to_owned(),
            "qa/dash-2fa -> https://dash.example.test".to_owned(),
            true
        )]
    );
}

#[test]
fn allow_for_this_chat_says_a_code_goes_in_and_holds_the_seed_for_that_site_alone() {
    let fills = Fills::default();
    let mut code = wait("https://dash.example.test");
    code.key = "dash-2fa".into();
    code.code = true;
    let (id, _heard) = fills.raise(code.clone()).expect("raised");

    let pressed = answered(
        &fills,
        &Audited::default(),
        &press(id, Press::Chat, &code.digest()),
    )
    .expect("allowed for the chat");

    let Pressed::Done { said } = pressed else {
        panic!("{pressed:?}");
    };
    assert!(said.contains("A 2FA code from dash-2fa"), "{said}");
    assert!(fills.code_granted(chat(7), "qa", "dash-2fa", "https://dash.example.test"));
    assert!(!fills.code_granted(chat(7), "qa", "dash-2fa", "https://other.example.test"));
    assert!(
        !fills.granted(chat(7), "qa", "dash-2fa", "https://dash.example.test"),
        "a code's Allow holds no value"
    );
    fills.forget(7);
    assert!(
        !fills.code_granted(chat(7), "qa", "dash-2fa", "https://dash.example.test"),
        "it goes with the chat"
    );
}

#[test]
fn a_grant_for_a_key_holds_values_or_codes_never_both() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let origin = "https://dash.example.test";
    let fills = Fills::default();
    // Always allow pressed for a value: its line, allowed here.
    let shown = offered_in(dir.path(), "qa", "by-line", origin);
    let (id, _heard) = fills.raise(shown.clone()).expect("raised");
    answered_in(
        dir.path(),
        &fills,
        &Audited::default(),
        &press(id, Press::Always, &shown.digest()),
    )
    .expect("always");
    fills
        .lock()
        .granted
        .push((7, chat_id(7), "qa".into(), "value".into(), origin.into()));
    fills
        .lock()
        .codes_granted
        .push((7, chat_id(7), "qa".into(), "code".into(), origin.into()));
    let asked = |key: &str, code: bool| FillAsked {
        code,
        ..asked_into(key, FieldKind::OneLine)
    };
    let held = |key: &str, code: bool| {
        granted(
            dir.path(),
            &fills,
            chat(7),
            Some("qa"),
            &asked(key, code),
            origin,
        )
    };

    assert!(held("value", false));
    assert!(!held("value", true), "a value's Allow holds no code");
    assert!(held("code", true));
    assert!(!held("code", false), "a code's Allow holds no value");
    assert!(held("by-line", false));
    assert!(!held("by-line", true), "an Always line holds no code");
    assert!(
        !granted(
            dir.path(),
            &fills,
            chat(8),
            Some("qa"),
            &asked("code", true),
            origin
        ),
        "for that chat alone"
    );
}

#[test]
fn a_2fa_code_is_not_offered_always_and_a_press_of_it_writes_nothing() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let one = offered(
        dir.path(),
        FillWait {
            persona: Some("qa".into()),
            code: true,
            no_always: None,
            ..wait("https://dash.example.test")
        },
    );
    assert_eq!(one.line, None);
    let locks = purlis_core::sandbox::policy::Locks::of(dir.path());
    let options: Vec<String> = crate::asking::secret_fill(&one, &locks)
        .options
        .into_iter()
        .map(|option| option.label)
        .collect();
    assert_eq!(
        options,
        ["Allow once", "Allow for this chat", "Keep blocked"]
    );

    let fills = Fills::default();
    let (id, heard) = fills.raise(one.clone()).expect("raised");
    let before = file_of(dir.path());
    assert!(
        answered_in(
            dir.path(),
            &fills,
            &Audited::default(),
            &press(id, Press::Always, &one.digest())
        )
        .is_err()
    );
    assert_eq!(file_of(dir.path()), before, "nothing written");
    assert!(heard.try_recv().is_err(), "the call still waits");
}

/// A project whose vault qa holds a value and a seed, and a chat that reads it.
fn vault_with_a_seed() -> (tempfile::TempDir, brokered::Asker) {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::write(
        dir.path().join("vaults.json"),
        r#"{"vaults":{"qa":{"provider":"plain-file","config":{"file":"q.json"}}}}"#,
    )
    .expect("a registry");
    std::fs::write(
        dir.path().join("q.json"),
        r#"{"dash-api-key":"fixture-key-5118","dash-2fa":"GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"}"#,
    )
    .expect("a vault");
    let asker = brokered::Asker {
        root: dir.path().to_path_buf(),
        env: Env::of(&[("PATH", "/usr/bin:/bin")]),
        chat: 7,
        persona: None,
        folder: None,
        confines: None,
        harness: None,
        ends_seen: None,
    };
    (dir, asker)
}

#[test]
fn a_fill_asked_as_a_code_reads_the_code_and_one_asked_as_a_value_never_reads_a_seed() {
    let (_dir, asker) = vault_with_a_seed();
    let asked = |key: &str, code: bool| FillAsked {
        code,
        ..asked_into(key, FieldKind::OneLine)
    };

    let code = read_now(&asker, &asked("dash-2fa", true))
        .expect("a code")
        .expect("the vault holds it");
    assert_eq!(code.expose().len(), 6);
    assert!(code.expose().chars().all(|c| c.is_ascii_digit()));

    let Err(refused) = read_now(&asker, &asked("dash-2fa", false)) else {
        panic!("a seed was read as a value");
    };
    assert!(refused.contains("2FA seed"), "{refused}");

    let value = read_now(&asker, &asked("dash-api-key", false))
        .expect("a value")
        .expect("the vault holds it");
    assert_eq!(value.expose(), "fixture-key-5118");
    assert!(read_now(&asker, &asked("dash-api-key", true)).is_err());
}

#[test]
fn a_key_the_person_marked_as_no_seed_is_read_for_the_fill_they_allowed_and_a_new_value_is_not() {
    let (dir, asker) = vault_with_a_seed();
    let asked = FillAsked {
        code: false,
        ..asked_into("app", FieldKind::OneLine)
    };
    let holding = |value: &str| {
        std::fs::write(
            dir.path().join("q.json"),
            serde_json::json!({ "app": value }).to_string(),
        )
        .expect("a vault");
    };
    holding("abcdefghijklmnop");
    assert!(read_now(&asker, &asked).is_err());

    let ctx = purlis_core::secrets::Ctx::new(dir.path(), asker.env.clone());
    // The window's own call makes the mark: this test stands for the person's press.
    crate::vaults::mark_no_seed(&ctx, "qa", "app").expect("marked");
    let value = read_now(&asker, &asked)
        .expect("a value")
        .expect("the vault holds it");
    assert_eq!(value.expose(), "abcdefghijklmnop");

    holding("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
    assert!(read_now(&asker, &asked).is_err());
}

/// #5272: a login follows the same value rules as a fill, the person's mark included: what it holds
/// counts for the value it sealed alone, and an authenticator's address goes in under no mark.
#[test]
fn a_login_reads_a_key_the_person_marked_as_no_seed_as_a_fill_does() {
    use purlis_core::sandbox::tools::login::{Login, PASS_VAR};
    use purlis_core::sandbox::tools::{Handed, Tool};
    let (dir, asker) = vault_with_a_seed();
    let tool = Tool {
        login: Some(Login {
            url: "https://shop.example.test/login".into(),
            origin: "https://shop.example.test".into(),
            form: None,
        }),
        ..Tool::one("shop-login", "qa", PASS_VAR, Handed::File, "app")
    };
    let holding = |value: &str| {
        std::fs::write(
            dir.path().join("q.json"),
            serde_json::json!({ "app": value }).to_string(),
        )
        .expect("a vault");
    };
    let read = || brokered::granted_login(&asker, &tool, None);
    holding("abcdefghijklmnop");
    let refused = read().expect_err("no mark yet");
    assert!(refused.contains("2FA seed"), "{refused}");

    let ctx = purlis_core::secrets::Ctx::new(dir.path(), asker.env.clone());
    // The window's own call makes the mark: this test stands for the person's press.
    crate::vaults::mark_no_seed(&ctx, "qa", "app").expect("marked");
    let (login, _) = read().expect("marked, it goes in");
    assert_eq!(login.password.expose(), "abcdefghijklmnop");

    holding("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
    let refused = read().expect_err("the mark was for another value");
    assert!(refused.contains("value has changed"), "{refused}");

    let address = format!("my 2fa {}%3A//totp/shop?secret=JBSWY3DPEHPK3PXP", "otpauth");
    holding(&address);
    let refused = read().expect_err("no address goes in");
    assert!(refused.contains("otpauth address"), "{refused}");
    assert!(!refused.contains("JBSW"), "{refused}");
}

#[test]
fn no_value_is_read_for_a_browser_the_person_has_taken_over() {
    let browsers = crate::chatbrowsers::ChatBrowsers::default();
    let asked = asked_into("dash-api-key", FieldKind::OneLine);
    assert!(not_taken_over(&browsers, &asked).is_ok());

    browsers.driven(&asked.program, true, 1000);

    let told = not_taken_over(&browsers, &asked).expect_err("refused");
    assert!(told.contains("taken b1 over"), "{told}");
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
    assert!(fills.granted(chat(7), "qa", "dash-api-key", "https://dash.example.test"));
    assert!(!fills.granted(chat(7), "qa", "dash-api-key", "https://other.example.test"));
    assert!(!fills.granted(chat(7), "qa", "another-key", "https://dash.example.test"));
    assert!(!fills.granted(chat(8), "qa", "dash-api-key", "https://dash.example.test"));
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
        !fills.granted(chat(7), "qa", "dash-api-key", "https://dash.example.test"),
        "it goes with the chat"
    );
}

/// An Allow for this chat is bound to the chat's id as well as its number: a later chat dealt the
/// same number never reads it, a code's grant neither.
#[test]
fn a_grant_is_never_read_by_another_chat_dealt_its_number() {
    let fills = Fills::default();
    let audited = Audited::default();
    for code in [false, true] {
        let shown = FillWait {
            persona: Some("qa".into()),
            code,
            no_always: None,
            ..wait("https://dash.example.test")
        };
        let (id, _heard) = fills.raise(shown.clone()).expect("raised");
        answered(&fills, &audited, &press(id, Press::Chat, &shown.digest())).expect("allowed");
    }
    let dealt_again = Chat {
        session: 7,
        id: Some("01JX000000000000000000007B"),
    };
    let on = ("qa", "dash-api-key", "https://dash.example.test");
    assert!(fills.granted(chat(7), on.0, on.1, on.2));
    assert!(fills.code_granted(chat(7), on.0, on.1, on.2));
    assert!(!fills.granted(dealt_again, on.0, on.1, on.2));
    assert!(!fills.code_granted(dealt_again, on.0, on.1, on.2));
    assert!(fills.granted_origins(dealt_again, on.0, on.1).is_empty());
}

/// #5230, SF-15: a form may be sent to the other sites this key is approved for in this chat, never
/// the site it was filled on again, another chat's, or a site that isn't `https`.
#[test]
fn a_fill_is_approved_for_the_other_sites_its_key_is_allowed_on_in_this_chat() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let fills = Fills::default();
    let grant = |chat: u32, key: &str, origin: &str| {
        fills
            .lock()
            .granted
            .push((chat, chat_id(chat), "qa".into(), key.into(), origin.into()));
    };
    grant(7, "dash-api-key", "https://dash.example.test");
    grant(7, "dash-api-key", "https://sso.example.test");
    grant(7, "dash-api-key", "http://localhost:3000");
    grant(7, "another-key", "https://other.example.test");
    grant(8, "dash-api-key", "https://elsewhere.example.test");
    let asked = FillAsked {
        browser: "b1".into(),
        program: "program-1".into(),
        target: "e12".into(),
        vault: "qa".into(),
        key: "dash-api-key".into(),
        code: false,
        field: FieldSaid {
            origin: Some("https://dash.example.test".into()),
            label: "API key".into(),
            kind: FieldKind::OneLine,
            by_get: Some(false),
            payment: Some(false),
        },
        page: None,
        pool: None,
    };

    let also = approved_elsewhere(
        dir.path(),
        &fills,
        chat(7),
        None,
        &asked,
        "https://dash.example.test",
    );

    assert_eq!(also, ["https://sso.example.test"]);
}

#[test]
fn allow_for_this_chat_fills_the_next_ask_without_a_notice_and_never_a_cards() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let fills = Fills::default();
    fills.lock().granted.push((
        7,
        chat_id(7),
        "qa".into(),
        "dash-api-key".into(),
        "https://dash.example.test".into(),
    ));
    let asked = |key: &str| FillAsked {
        browser: "b1".into(),
        program: "program-1".into(),
        target: "e12".into(),
        vault: "qa".into(),
        key: key.into(),
        field: purlis_session_protocol::browsing::FieldSaid {
            origin: Some("https://dash.example.test".into()),
            label: "API key".into(),
            kind: FieldKind::OneLine,
            by_get: Some(false),
            payment: Some(false),
        },
        code: false,
        page: None,
        pool: None,
    };
    let origin = "https://dash.example.test";
    assert!(granted(
        dir.path(),
        &fills,
        chat(7),
        Some("qa"),
        &asked("dash-api-key"),
        origin
    ));
    assert!(!granted(
        dir.path(),
        &fills,
        chat(8),
        Some("qa"),
        &asked("dash-api-key"),
        origin
    ));
    assert!(!granted(
        dir.path(),
        &fills,
        chat(7),
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
        chat(7),
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

/// The steward's decision of 2026-11-06: a field the page marks for payment is shown as a card
/// or payment value, offered Allow once and Keep blocked alone, with no line.
#[test]
fn a_payment_field_is_offered_allow_once_alone_and_shows_no_line() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let one = FillWait {
        payment: true,
        ..offered_in(
            dir.path(),
            "qa",
            "dash-api-key",
            "https://shop.example.test",
        )
    };
    let locks = purlis_core::sandbox::policy::Locks::of(dir.path());

    let shown = crate::asking::secret_fill(&one, &locks);

    let options: Vec<&str> = shown.options.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(options, ["Allow once", "Keep blocked"]);
    let fill = shown.fill.expect("a fill");
    assert!(fill.card, "said as a card or payment value");
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
    assert!(!fills.granted(chat(7), "qa", "dash-api-key", "https://dash.example.test"));
}

/// A fill of key `key` into a field of `kind` whose frame is on dash.example.test.
fn asked_into(key: &str, kind: FieldKind) -> FillAsked {
    FillAsked {
        browser: "b1".into(),
        program: "program-1".into(),
        target: "e12".into(),
        vault: "qa".into(),
        key: key.into(),
        field: purlis_session_protocol::browsing::FieldSaid {
            origin: Some("https://dash.example.test".into()),
            label: "Comment".into(),
            kind,
            by_get: Some(false),
            payment: Some(false),
        },
        code: false,
        page: None,
        pool: None,
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
        chat_id(7),
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
                chat(7),
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
            chat(7),
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
    assert!(!fills.granted(chat(7), "qa", "dash-api-key", "https://dash.example.test"));
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
        chat_id(7),
        "qa".into(),
        "dash-api-key".into(),
        "http://localhost:3000".into(),
    ));
    let mut asked = asked_into("dash-api-key", FieldKind::OneLine);
    asked.field.origin = Some("http://localhost:3000".into());
    assert!(asks(
        dir.path(),
        &fills,
        chat(7),
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

// ---- A fill's key is judged at the press (#5831) ----

/// A fill whose key became a link into a vault its persona may not use while its notice waited:
/// Always is allowed nothing, writes no line, grants the persona no vault, and the person reads why
/// on the notice, told as you; the chat hears nothing yet.
#[test]
fn a_fill_linked_out_of_reach_while_its_notice_waits_is_refused_at_the_press() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let root = dir.path();
    let fills = Fills::default();
    let shown = FillWait {
        reads_as: Some("qa".into()),
        ..offered_in(root, "prod", "deploy-token", "https://dash.example.test")
    };
    assert!(
        shown.vault_too,
        "Always would grant qa vault prod: {shown:?}"
    );
    let (id, heard) = fills.raise(shown.clone()).expect("raised");
    // While it waits, prod's deploy-token is linked into vault `ops-only`, which qa may not use.
    std::fs::write(
        root.join("vaults.json"),
        r#"{"vaults":{
            "qa":{"provider":"plain-file","config":{"file":"q.json"},"persona":"qa"},
            "prod":{"provider":"plain-file","config":{"file":"o.json"},"persona":"ops"},
            "ops-only":{"provider":"plain-file","config":{"file":"x.json"},"persona":"ops"}
        }}"#,
    )
    .expect("a registry");
    std::fs::write(root.join("x.json"), r#"{"deploy-token":"n0t-yours"}"#).expect("ops-only");
    let ctx = Ctx::new(root, Env::from_process());
    let prod = purlis_core::secrets::cmd::provider(&ctx, "prod").expect("vault prod");
    purlis_core::secrets::links::link(&ctx, &prod, "deploy-token", "ops-only", "deploy-token")
        .expect("linked");
    let before = file_of(root);
    let audited = Audited::default();

    let said = answered_in(
        root,
        &fills,
        &audited,
        &press(id, Press::Always, &shown.digest()),
    )
    .map(|_| ())
    .expect_err("refused");

    assert!(said.starts_with("Nothing was allowed: "), "{said}");
    assert!(
        said.contains("deploy-token in vault prod reads an entry of a vault qa may not use"),
        "{said}"
    );
    assert!(!said.contains("ops-only"), "names no other vault: {said}");
    assert!(!said.contains("n0t-yours"), "never the value: {said}");
    assert!(!said.contains("The person"), "the person reads you: {said}");
    assert!(said.contains("You can link it"), "{said}");
    assert_eq!(file_of(root), before, "no line written");
    assert!(audited.borrow().is_empty(), "nothing allowed is audited");
    assert!(
        purlis_core::sandbox::local::granted_vaults(root).is_empty(),
        "no vault granted"
    );
    // The notice stays, showing why, as for every press refused; Keep blocked still ends it.
    assert_eq!(fills.waiting().len(), 1, "the notice stays");
    assert!(heard.try_recv().is_err(), "the chat still waits");
}

/// A press on a narrowed chat's fill is judged as the persona it was started as (#5831): a chat
/// started as ops, narrowed to qa's grants, is allowed a fill whose key links into a vault ops may
/// use.
#[test]
fn a_narrowed_chats_fill_press_is_judged_as_the_persona_it_was_started_as() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let root = dir.path();
    let fills = Fills::default();
    std::fs::write(root.join("q.json"), "{}").expect("vault qa");
    let shown = FillWait {
        reads_as: Some("ops".into()),
        ..offered_in(root, "qa", "deploy-token", "https://dash.example.test")
    };
    let (id, heard) = fills.raise(shown.clone()).expect("raised");
    // While it waits, qa's deploy-token is linked into vault prod, which ops may use and qa may
    // not.
    std::fs::write(root.join("o.json"), r#"{"deploy-token":"t"}"#).expect("vault prod");
    let ctx = Ctx::new(root, Env::from_process());
    let qa = purlis_core::secrets::cmd::provider(&ctx, "qa").expect("vault qa");
    purlis_core::secrets::links::link(&ctx, &qa, "deploy-token", "prod", "deploy-token")
        .expect("linked");

    answered_in(
        root,
        &fills,
        &Audited::default(),
        &press(id, Press::Once, &shown.digest()),
    )
    .expect("allowed");

    assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
}
