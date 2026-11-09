// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One notice for a login a chat suggests (#4595): a press answers exactly what it was shown,
//! Once and this chat write nothing, and Always blesses nothing nobody saw.

use std::cell::RefCell;
use std::path::Path;
use std::time::Instant;

use purlis_core::loginask::{Proposal, Suggested};
use purlis_core::sandbox::tools::Tool;

use super::*;

#[path = "tests/held.rs"]
mod held;

const VAULTS: &str = r#"{"vaults":{
    "qa":{"provider":"plain-file","config":{"file":"q.json"},"persona":"qa"},
    "prod":{"provider":"plain-file","config":{"file":"p.json"},"persona":"ops"}
}}"#;

/// What vault qa holds: each key a test suggests.
const QA_HOLDS: &str = r#"{"username":"u","password":"p"}"#;

/// The id of chat 7, the chat the tests raise their logins for.
const CHAT_7: Option<&str> = Some("01JX0000000000000000000007");

fn project(file: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::write(dir.path().join("vaults.json"), VAULTS).expect("a registry");
    // Vault qa holds every key the tests suggest, so no notice says one is missing.
    std::fs::write(dir.path().join("q.json"), QA_HOLDS).expect("vault qa");
    std::fs::write(purlis_core::names::manifest(dir.path()), file).expect("a project file");
    dir
}

fn suggested(site: &str) -> Suggested {
    Suggested {
        site: site.to_owned(),
        vault: "qa".to_owned(),
        user_key: Some("username".to_owned()),
        key: "password".to_owned(),
    }
}

/// Where a page's own form shows: on the page itself.
fn on_its_page(page: &str) -> Result<String, String> {
    Ok(page.to_owned())
}

/// [`super::judged`] for a page whose form is on its own origin.
fn judged(
    root: &Path,
    persona: &str,
    login: &str,
    suggested: &Suggested,
    held: &[Tool],
) -> Result<Judged, String> {
    super::judged(
        root,
        Whose {
            grants: persona,
            reads_as: persona,
        },
        login,
        suggested,
        &on_its_page,
        None,
        held,
    )
    .map(|judged| judged.expect("an undeclared login is asked about"))
}

fn raised(root: &Path, grants: &Grants, site: &str) -> (u32, Proposal, mpsc::Receiver<Heard>) {
    raised_finding(root, grants, site, &on_its_page)
}

/// [`raised`], the page followed to its form with `finds`.
fn raised_finding(
    root: &Path,
    grants: &Grants,
    site: &str,
    finds: FindsForm<'_>,
) -> (u32, Proposal, mpsc::Receiver<Heard>) {
    let judged = super::judged(
        root,
        Whose {
            grants: "qa",
            reads_as: "qa",
        },
        "shop-login",
        &suggested(site),
        finds,
        None,
        &[],
    )
    .expect("asked")
    .expect("an ask");
    let proposal = judged.proposal.clone();
    let (id, heard) = grants
        .raise(LoginWait {
            id: 0,
            session: 7,
            chat: CHAT_7.map(str::to_owned),
            proposal: judged.proposal,
            since: 1,
            vault_too: judged.vault_too,
            no_always: judged.no_always,
            pool: None,
            reads_as: "qa".to_owned(),
        })
        .expect("raised");
    (id, proposal, heard)
}

fn press(id: u32, press: Press, shown: &str) -> Asked<'_> {
    Asked {
        session: 7,
        id,
        press,
        shown,
        typed: None,
        reason: None,
    }
}

type Audited = RefCell<Vec<(String, String)>>;

fn answered(
    root: &Path,
    grants: &Grants,
    audited: &Audited,
    asked: &Asked<'_>,
) -> Result<Pressed, String> {
    answered_finding(root, grants, audited, asked, &on_its_page)
}

/// [`answered`], a typed page followed to its form with `finds`.
fn answered_finding(
    root: &Path,
    grants: &Grants,
    audited: &Audited,
    asked: &Asked<'_>,
    finds: FindsForm<'_>,
) -> Result<Pressed, String> {
    answer(root, grants, asked, finds, &|_, one| {
        audited
            .borrow_mut()
            .push((one.what.to_owned(), one.level.word().to_owned()));
        Ok(())
    })
}

fn file_of(root: &Path) -> String {
    std::fs::read_to_string(purlis_core::names::manifest(root)).expect("a project file")
}

fn tool_of(grants: &Grants, name: &str) -> Option<Tool> {
    grants.granted(7, CHAT_7, Some("qa"), name)
}

#[test]
fn allow_once_fills_what_was_confirmed_writes_nothing_and_goes_with_its_read() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let grants = Grants::default();
    let (id, proposal, heard) = raised(dir.path(), &grants, "https://shop.example.test/login");
    let before = file_of(dir.path());
    let audited = Audited::default();
    let done = answered(
        dir.path(),
        &grants,
        &audited,
        &press(id, Press::Once, &proposal.digest()),
    );
    assert!(matches!(done, Ok(Pressed::Done { .. })), "{done:?}");
    assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
    assert_eq!(file_of(dir.path()), before);
    assert_eq!(audited.borrow().len(), 1);
    let tool = tool_of(&grants, "shop-login").expect("held");
    assert_eq!(
        tool.login.expect("a login").origin,
        "https://shop.example.test"
    );
    assert!(grants.take(7, CHAT_7, Some("qa"), "shop-login").is_some());
    assert!(tool_of(&grants, "shop-login").is_none(), "used once");
    assert!(grants.take(7, CHAT_7, Some("qa"), "shop-login").is_none());
}

#[test]
fn allow_for_this_chat_stays_for_that_chat_and_persona_alone() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/login");
    let audited = Audited::default();
    answered(
        dir.path(),
        &grants,
        &audited,
        &press(id, Press::Chat, &proposal.digest()),
    )
    .expect("allowed");
    assert!(grants.take(7, CHAT_7, Some("qa"), "shop-login").is_some());
    assert!(tool_of(&grants, "shop-login").is_some());
    assert!(grants.granted(8, None, Some("qa"), "shop-login").is_none());
    assert!(
        grants
            .granted(7, CHAT_7, Some("ops"), "shop-login")
            .is_none()
    );
    grants.forget(7);
    assert!(
        tool_of(&grants, "shop-login").is_none(),
        "a later chat dealt 7 gets nothing"
    );
}

#[test]
fn a_grant_is_never_read_by_another_chat_dealt_its_number() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/login");
    let audited = Audited::default();
    answered(
        dir.path(),
        &grants,
        &audited,
        &press(id, Press::Chat, &proposal.digest()),
    )
    .expect("allowed");
    let another = Some("01JX0000000000000000000008");
    assert!(
        grants
            .granted(7, another, Some("qa"), "shop-login")
            .is_none()
    );
    assert!(grants.take(7, another, Some("qa"), "shop-login").is_none());
    assert!(grants.granted(7, None, Some("qa"), "shop-login").is_none());
    assert!(tool_of(&grants, "shop-login").is_some(), "still chat 7's");
}

#[test]
fn a_press_on_anything_but_what_was_drawn_changes_nothing() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/login");
    let audited = Audited::default();
    assert!(
        answered(
            dir.path(),
            &grants,
            &audited,
            &press(id, Press::Once, "other")
        )
        .is_err()
    );
    let wrong_chat = Asked {
        session: 8,
        ..press(id, Press::Once, "x")
    };
    assert!(answered(dir.path(), &grants, &audited, &wrong_chat).is_err());
    assert!(tool_of(&grants, "shop-login").is_none());
    assert!(audited.borrow().is_empty());
    assert_eq!(grants.waiting()[0].proposal, proposal);
}

#[test]
fn a_second_call_never_replaces_the_proposal_shown() {
    let dir = project("");
    let grants = Grants::default();
    let (_, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/login");
    let other = judged(
        dir.path(),
        "qa",
        "shop-login",
        &suggested("https://evil.example.test/"),
        &[],
    )
    .expect("judged");
    let refused = grants.raise(LoginWait {
        id: 0,
        session: 7,
        chat: CHAT_7.map(str::to_owned),
        proposal: other.proposal,
        since: 2,
        vault_too: false,
        no_always: None,
        pool: None,
        reads_as: "qa".to_owned(),
    });
    assert!(refused.is_err());
    assert_eq!(grants.waiting()[0].proposal, proposal);
}

#[test]
fn a_typed_domain_is_what_fills() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://evil.example.test/login");
    let audited = Audited::default();
    let digest = proposal.digest();
    let asked = Asked {
        typed: Some("shop.example.test"),
        ..press(id, Press::Once, &digest)
    };
    answered(dir.path(), &grants, &audited, &asked).expect("allowed");
    let login = tool_of(&grants, "shop-login")
        .and_then(|tool| tool.login)
        .expect("a login");
    assert_eq!(login.origin, "https://shop.example.test");
    assert_eq!(login.url, "https://shop.example.test/login");
}

#[test]
fn a_typed_domain_spelled_otherwise_is_drawn_again_before_anything_is_allowed() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, heard) = raised(dir.path(), &grants, "https://evil.example.test/login");
    let audited = Audited::default();
    let digest = proposal.digest();
    let asked = Asked {
        typed: Some("pаypal.example.test"),
        ..press(id, Press::Once, &digest)
    };
    let drawn = answered(dir.path(), &grants, &audited, &asked);
    assert!(matches!(drawn, Ok(Pressed::DrawnAgain { .. })), "{drawn:?}");
    assert!(tool_of(&grants, "shop-login").is_none());
    assert!(heard.try_recv().is_err(), "the chat still waits");
    let now = &grants.waiting()[0].proposal;
    assert!(now.origin.contains("xn--"), "{now:?}");
    assert!(!now.warnings.is_empty());
    assert_ne!(now.digest(), proposal.digest());
    // The old press no longer answers it.
    assert!(
        answered(
            dir.path(),
            &grants,
            &audited,
            &press(id, Press::Once, &proposal.digest())
        )
        .is_err()
    );
}

#[test]
fn a_typed_http_site_is_refused_and_the_ask_stays() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    let audited = Audited::default();
    let digest = proposal.digest();
    let asked = Asked {
        typed: Some("http://shop.example.test"),
        ..press(id, Press::Once, &digest)
    };
    assert!(answered(dir.path(), &grants, &audited, &asked).is_err());
    assert_eq!(grants.waiting().len(), 1);
}

#[test]
fn a_warning_that_came_up_while_the_notice_was_drawn_is_drawn_before_a_press_counts() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    let other = other_site(dir.path());
    let (other_id, _other_heard) = grants
        .raise(LoginWait {
            id: 0,
            session: 9,
            chat: None,
            proposal: judged(
                dir.path(),
                "qa",
                "other",
                &suggested("https://other.example.test/"),
                &[],
            )
            .expect("judged")
            .proposal,
            since: 2,
            vault_too: false,
            no_always: None,
            pool: None,
            reads_as: "qa".to_owned(),
        })
        .expect("raised");
    assert!(grants.allow(other_id, "qa", other, true));
    grants.take(9, None, Some("qa"), "other");
    let audited = Audited::default();
    let drawn = answered(
        dir.path(),
        &grants,
        &audited,
        &press(id, Press::Once, &proposal.digest()),
    );
    assert!(matches!(drawn, Ok(Pressed::DrawnAgain { .. })), "{drawn:?}");
    assert!(heard.try_recv().is_err());
    assert!(!grants.waiting()[0].proposal.warnings.is_empty());
}

/// A login of the same key on another site.
fn other_site(root: &Path) -> Tool {
    let other = purlis_core::loginask::proposal(
        root,
        "qa",
        "other",
        &suggested("https://other.example.test/"),
        &on_its_page,
        &[],
    )
    .expect("a proposal");
    other.tool().expect("a tool")
}

#[test]
fn a_vault_that_isnt_the_personas_is_said() {
    let dir = project("");
    let not_its = Suggested {
        vault: "prod".to_owned(),
        ..suggested("https://shop.example.test/")
    };
    let judged = judged(dir.path(), "qa", "shop-login", &not_its, &[]).expect("judged");
    assert!(judged.vault_too);
    assert!(
        judged
            .proposal
            .warnings
            .iter()
            .any(|one| matches!(one, loginask::Warning::NotItsVault { .. }))
    );
}

#[test]
fn a_key_the_vault_has_nothing_under_is_said_on_the_notice() {
    let dir = project("");
    let wrong = Suggested {
        key: "pasword".to_owned(),
        ..suggested("https://shop.example.test/")
    };
    let judged = judged(dir.path(), "qa", "shop-login", &wrong, &[]).expect("judged");
    let missing = loginask::Warning::NotInVault {
        vault: "qa".to_owned(),
        keys: vec!["pasword".to_owned()],
    };
    assert!(
        judged.proposal.warnings.contains(&missing),
        "{:?}",
        judged.proposal.warnings
    );
}

#[test]
fn a_key_put_in_the_vault_while_the_notice_is_up_is_no_reason_to_draw_it_again() {
    let dir = project("");
    std::fs::write(dir.path().join("q.json"), "{}").expect("vault qa, empty");
    let grants = Grants::default();
    let (id, proposal, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    std::fs::write(dir.path().join("q.json"), r#"{"username":"u"}"#).expect("one key added");
    let audited = Audited::default();
    let done = answered(
        dir.path(),
        &grants,
        &audited,
        &press(id, Press::Once, &proposal.digest()),
    );
    assert!(matches!(done, Ok(Pressed::Done { .. })), "{done:?}");
    assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
}

#[test]
fn a_vault_the_project_doesnt_have_is_never_asked_about() {
    let dir = project("");
    let unknown = Suggested {
        vault: "nope".to_owned(),
        ..suggested("https://shop.example.test/")
    };
    assert!(judged(dir.path(), "qa", "shop-login", &unknown, &[]).is_err());
}

/// The person's ruling of 2026-11-07: Keep blocked counts for the persona until purlis restarts,
/// so a new chat of the same persona starts no fresh tries.
#[test]
fn two_keep_blocked_end_a_personas_login_notices_for_this_run() {
    let grants = Grants::default();
    grants.keep("qa", "one", "https://a.example.test");
    assert!(!grants.kept("qa", "two", "https://b.example.test"));
    grants.keep("qa", "two", "https://b.example.test");
    assert!(grants.kept("qa", "three", "https://c.example.test"));
    assert!(!grants.kept("ops", "three", "https://c.example.test"));
    // A chat closing takes none of it with it.
    grants.forget(7);
    assert!(grants.kept("qa", "three", "https://c.example.test"));
}

#[test]
fn an_empty_field_never_falls_back_to_the_chats_site() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    let audited = Audited::default();
    let digest = proposal.digest();
    let asked = Asked {
        typed: Some("  "),
        ..press(id, Press::Once, &digest)
    };
    assert!(answered(dir.path(), &grants, &audited, &asked).is_err());
    assert!(tool_of(&grants, "shop-login").is_none());
}

#[test]
fn always_on_a_typed_site_shows_its_line_first() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://evil.example.test/login");
    let audited = Audited::default();
    let before = file_of(dir.path());
    let digest = proposal.digest();
    let asked = Asked {
        typed: Some("shop.example.test"),
        ..press(id, Press::Always, &digest)
    };
    let drawn = answered(dir.path(), &grants, &audited, &asked);
    assert!(matches!(drawn, Ok(Pressed::DrawnAgain { .. })), "{drawn:?}");
    assert_eq!(file_of(dir.path()), before);
    assert_eq!(
        grants.waiting()[0].proposal.origin,
        "https://shop.example.test"
    );
}

#[test]
fn keep_blocked_is_not_asked_again_of_that_persona_in_any_chat() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    let audited = Audited::default();
    let digest = proposal.digest();
    let asked = Asked {
        reason: Some("not that site"),
        ..press(id, Press::Keep, &digest)
    };
    answered(dir.path(), &grants, &audited, &asked).expect("kept");
    assert_eq!(
        heard.try_recv(),
        Ok(Heard::Kept(Some("not that site".to_owned())))
    );
    grants.forget(7);
    assert!(grants.kept("qa", "shop-login", "https://shop.example.test"));
    assert!(!grants.kept("ops", "shop-login", "https://shop.example.test"));
    assert!(audited.borrow().is_empty());
}

#[test]
fn always_writes_the_one_line_and_allows_it_here() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let grants = Grants::default();
    let (id, proposal, heard) = raised(dir.path(), &grants, "https://shop.example.test/login");
    let audited = Audited::default();
    answered(
        dir.path(),
        &grants,
        &audited,
        &press(id, Press::Always, &proposal.digest()),
    )
    .expect("allowed");
    assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
    assert!(file_of(dir.path()).contains("https://shop.example.test/login"));
    let (declared, allowed) = sandbox::tools::declared_and_allowed_here(dir.path(), "qa");
    assert_eq!(declared.len(), 1);
    assert_eq!(allowed, declared, "allowed here in the same press");
    assert!(
        tool_of(&grants, "shop-login").is_some(),
        "the call that waited is filled too"
    );
}

#[test]
fn always_is_not_offered_while_the_personas_list_waits_for_an_allow() {
    let dir = project(
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.qa]\nhosts = [\"unseen.example.test\"]\n",
    );
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    assert!(grants.waiting()[0].no_always.is_some());
    let audited = Audited::default();
    let before = file_of(dir.path());
    assert!(
        answered(
            dir.path(),
            &grants,
            &audited,
            &press(id, Press::Always, &proposal.digest())
        )
        .is_err()
    );
    assert_eq!(file_of(dir.path()), before);
    assert!(sandbox::local::allowed_persona_hosts(dir.path()).is_empty());
    assert!(audited.borrow().is_empty());
}

#[test]
fn always_is_held_by_a_project_with_proposals_off_and_the_rest_are_not() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    sandbox::policy::set_for_this_test(Locks::parse(
        r#"{"sandbox": {"proposed-changes": "off"}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    assert!(grants.waiting()[0].no_always.is_some());
    let audited = Audited::default();
    assert!(
        answered(
            dir.path(),
            &grants,
            &audited,
            &press(id, Press::Always, &proposal.digest())
        )
        .is_err()
    );
    answered(
        dir.path(),
        &grants,
        &audited,
        &press(id, Press::Once, &proposal.digest()),
    )
    .expect("once stays");
}

#[test]
fn a_mint_mark_added_while_the_notice_is_up_refuses_the_press() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    std::fs::write(
        purlis_core::names::manifest(dir.path()),
        "[sandbox.personas.dev.tools.gh]\nvault = \"qa\"\nkey = \"password\"\nmint = \
         \"github-app\"\napp = \"Iv1.abc\"\n",
    )
    .expect("written");
    let audited = Audited::default();
    assert!(
        answered(
            dir.path(),
            &grants,
            &audited,
            &press(id, Press::Once, &proposal.digest())
        )
        .is_err()
    );
    assert!(tool_of(&grants, "shop-login").is_none());
}

#[test]
fn a_press_that_lands_after_its_chat_closed_holds_nothing() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    let tool = proposal.tool().expect("a tool");
    grants.forget(7);
    assert!(!grants.allow(id, "qa", tool, false));
    assert!(tool_of(&grants, "shop-login").is_none());
}

#[test]
fn a_closed_chat_stops_its_call_waiting() {
    let dir = project("");
    let grants = Grants::default();
    let (_, _, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    grants.forget(7);
    assert_eq!(heard.try_recv(), Ok(Heard::Gone));
    assert!(grants.waiting().is_empty());
}

// ---- A form on another host (#4995) ----

const AUTH: &str = "https://auth.example.test/realms/x/protocol/openid-connect/auth?state=s";

fn on_auth(_: &str) -> Result<String, String> {
    Ok(AUTH.to_owned())
}

#[test]
fn a_form_on_another_host_is_drawn_and_once_fills_there_alone() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, heard) =
        raised_finding(dir.path(), &grants, "https://app.example.test/", &on_auth);
    assert_eq!(proposal.form, "https://auth.example.test");
    assert!(
        proposal
            .warnings
            .iter()
            .any(|warning| matches!(warning, loginask::Warning::FormElsewhere { .. })),
        "{proposal:?}"
    );
    let audited = Audited::default();
    answered(
        dir.path(),
        &grants,
        &audited,
        &press(id, Press::Once, &proposal.digest()),
    )
    .expect("allowed");
    assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
    let login = tool_of(&grants, "shop-login")
        .and_then(|tool| tool.login)
        .expect("a login");
    assert_eq!(login.url, "https://app.example.test/");
    assert_eq!(login.fills_on(), "https://auth.example.test");
}

#[test]
fn a_page_that_cant_be_followed_to_its_form_is_never_asked_about() {
    let dir = project("");
    let refused = super::judged(
        dir.path(),
        Whose {
            grants: "qa",
            reads_as: "qa",
        },
        "shop-login",
        &suggested("https://app.example.test/"),
        &|_| Ok("http://auth.example.test/login".to_owned()),
        None,
        &[],
    )
    .expect_err("refused");
    assert!(refused.contains("not https"), "{refused}");
    assert!(refused.contains("records nothing"), "{refused}");
}

/// A site the person typed is followed to its own form: the form found for the chat's page
/// never stands for it, and a form elsewhere is drawn before a press counts.
#[test]
fn a_typed_site_is_followed_to_its_own_form_before_anything_is_allowed() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, heard) = raised(dir.path(), &grants, "https://shop.example.test/login");
    let audited = Audited::default();
    let digest = proposal.digest();
    let asked = Asked {
        typed: Some("app.example.test"),
        ..press(id, Press::Once, &digest)
    };
    let drawn = answered_finding(dir.path(), &grants, &audited, &asked, &on_auth);
    assert!(matches!(drawn, Ok(Pressed::DrawnAgain { .. })), "{drawn:?}");
    assert!(heard.try_recv().is_err(), "the chat still waits");
    assert!(tool_of(&grants, "shop-login").is_none());
    assert_eq!(
        grants.waiting()[0].proposal.form,
        "https://auth.example.test"
    );
}

#[test]
fn a_typed_site_that_cant_be_followed_to_its_form_is_refused_and_the_ask_stays() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/login");
    let audited = Audited::default();
    let digest = proposal.digest();
    let asked = Asked {
        typed: Some("app.example.test"),
        ..press(id, Press::Once, &digest)
    };
    let refused = answered_finding(dir.path(), &grants, &audited, &asked, &|_| {
        Err("no login form appeared".to_owned())
    });
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(
        grants.waiting()[0].proposal,
        proposal,
        "the ask stays as drawn"
    );
    assert!(tool_of(&grants, "shop-login").is_none());
}

#[test]
fn always_writes_the_form_the_notice_showed() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let grants = Grants::default();
    let (id, proposal, _heard) =
        raised_finding(dir.path(), &grants, "https://app.example.test/", &on_auth);
    let audited = Audited::default();
    answered(
        dir.path(),
        &grants,
        &audited,
        &press(id, Press::Always, &proposal.digest()),
    )
    .expect("allowed");
    let file = file_of(dir.path());
    assert!(
        file.contains("form = \"https://auth.example.test\""),
        "{file}"
    );
    assert!(!file.contains("state=s"), "{file}");
}

// ---- A call naming another site for a declared login (#4996) ----

/// The project of #4996: `shop-login` recorded on the app's address, allowed on this machine.
const RECORDED: &str = "[sandbox]\nmode = \"on\"\n[sandbox.personas.qa]\ntools = { shop-login = { \
                        vault = \"qa\", browser = \"https://app.example.test/\", user = \
                        \"username\", key = \"password\" } }\n";

/// A project that records `shop-login` on the app's address, with qa's list allowed here.
fn recorded() -> tempfile::TempDir {
    let dir = project(RECORDED);
    let locks = Locks::of(dir.path());
    let digest = sandbox::persona::digest_in(dir.path(), RECORDED, "qa", &locks).expect("a digest");
    sandbox::local::allow_persona_hosts(dir.path(), "qa", &digest).expect("allowed");
    dir
}

fn declared_shop(root: &Path) -> Tool {
    declared_now(root, "qa", "shop-login").expect("declared")
}

/// The replace notice of #4996, raised for chat 7.
fn raised_replacing(root: &Path, grants: &Grants) -> (u32, Proposal, mpsc::Receiver<Heard>) {
    let judged = super::judged(
        root,
        Whose {
            grants: "qa",
            reads_as: "qa",
        },
        "shop-login",
        &suggested("https://app.example.test/"),
        &on_auth,
        Some(&declared_shop(root)),
        &[],
    )
    .expect("asked")
    .expect("a replacement");
    let proposal = judged.proposal.clone();
    let (id, heard) = grants
        .raise(LoginWait {
            id: 0,
            session: 7,
            chat: CHAT_7.map(str::to_owned),
            proposal: judged.proposal,
            since: 1,
            vault_too: judged.vault_too,
            no_always: judged.no_always,
            pool: None,
            reads_as: "qa".to_owned(),
        })
        .expect("raised");
    (id, proposal, heard)
}

#[test]
fn a_call_naming_the_site_a_login_has_is_no_ask() {
    let dir = recorded();
    let judged = super::judged(
        dir.path(),
        Whose {
            grants: "qa",
            reads_as: "qa",
        },
        "shop-login",
        &suggested("https://app.example.test/"),
        &on_its_page,
        Some(&declared_shop(dir.path())),
        &[],
    )
    .expect("judged");
    assert!(judged.is_none());
}

#[test]
fn a_call_naming_another_site_asks_to_replace_the_recorded_one() {
    let dir = recorded();
    let grants = Grants::default();
    let (_, proposal, _heard) = raised_replacing(dir.path(), &grants);
    let old = proposal.replaces.expect("it replaces");
    assert_eq!(old.page, "https://app.example.test/");
    assert_eq!(old.form, "https://app.example.test");
    assert_eq!(proposal.form, "https://auth.example.test");
    assert_eq!(
        file_of(dir.path()),
        RECORDED,
        "nothing changes before a press"
    );
}

#[test]
fn once_and_this_chat_write_nothing_and_the_next_login_fills_on_the_new_host() {
    for press_it in [Press::Once, Press::Chat] {
        let dir = recorded();
        let grants = Grants::default();
        let (id, proposal, heard) = raised_replacing(dir.path(), &grants);
        let audited = Audited::default();
        answered(
            dir.path(),
            &grants,
            &audited,
            &press(id, press_it, &proposal.digest()),
        )
        .expect("allowed");
        assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
        assert_eq!(file_of(dir.path()), RECORDED);
        let login = tool_of(&grants, "shop-login")
            .and_then(|tool| tool.login)
            .expect("held for the chat");
        assert_eq!(login.fills_on(), "https://auth.example.test");
    }
}

#[test]
fn always_replaces_the_recorded_line_and_allows_it_here() {
    let dir = recorded();
    let grants = Grants::default();
    let (id, proposal, _heard) = raised_replacing(dir.path(), &grants);
    let audited = Audited::default();
    let done = answered(
        dir.path(),
        &grants,
        &audited,
        &press(id, Press::Always, &proposal.digest()),
    )
    .expect("replaced");
    assert!(
        matches!(&done, Pressed::Done { said } if said.starts_with("Replaced")),
        "{done:?}"
    );
    let file = file_of(dir.path());
    assert!(
        file.contains("form = \"https://auth.example.test\""),
        "{file}"
    );
    let (declared, allowed) = sandbox::tools::declared_and_allowed_here(dir.path(), "qa");
    let tool = declared
        .iter()
        .find(|tool| tool.name == "shop-login")
        .expect("declared");
    assert_eq!(
        tool.login.as_ref().map(|login| login.fills_on()),
        Some("https://auth.example.test")
    );
    assert!(
        allowed.iter().any(|one| one.name == "shop-login"),
        "{allowed:?}"
    );
}

#[test]
fn keep_leaves_the_recorded_site_and_tells_the_chat_plainly() {
    let dir = recorded();
    let grants = Grants::default();
    let (id, proposal, heard) = raised_replacing(dir.path(), &grants);
    let audited = Audited::default();
    let done = answered(
        dir.path(),
        &grants,
        &audited,
        &press(id, Press::Keep, &proposal.digest()),
    )
    .expect("kept");
    assert!(
        matches!(&done, Pressed::Done { said } if said.contains("recorded site")),
        "{done:?}"
    );
    assert_eq!(heard.try_recv(), Ok(Heard::Kept(None)));
    assert_eq!(file_of(dir.path()), RECORDED);
    assert!(tool_of(&grants, "shop-login").is_none());
    let told = kept_where_it_is_said("shop-login", "https://app.example.test/", None);
    assert!(
        told.contains("kept login shop-login on https://app.example.test/"),
        "{told}"
    );
}

#[test]
fn always_is_held_back_by_proposals_off_for_a_replacement_too() {
    let dir = recorded();
    sandbox::policy::set_for_this_test(Locks::parse(
        r#"{"sandbox": {"proposed-changes": "off"}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let grants = Grants::default();
    let (id, proposal, _heard) = raised_replacing(dir.path(), &grants);
    assert!(grants.waiting()[0].no_always.is_some());
    let audited = Audited::default();
    assert!(
        answered(
            dir.path(),
            &grants,
            &audited,
            &press(id, Press::Always, &proposal.digest()),
        )
        .is_err()
    );
    assert_eq!(file_of(dir.path()), RECORDED);
}

/// #4995: a chat's calls side by side never open a browser apiece to follow their pages.
#[test]
fn a_chat_follows_one_page_to_its_form_at_a_time() {
    let grants = Grants::default();
    let one = grants.follow(7).expect("the first");
    assert!(
        grants.follow(7).is_none(),
        "a second, side by side, is refused"
    );
    assert!(grants.follow(8).is_some(), "another chat's is its own");
    drop(one);
    assert!(
        grants.follow(7).is_some(),
        "once it's done, the next may follow"
    );
}

/// Nothing is opened for what needs no browser to judge: a declared login called with other keys,
/// or on its own page where its line records its form.
#[test]
fn a_declared_login_is_judged_before_any_page_is_followed() {
    let text = RECORDED.replace(
        "browser = \"https://app.example.test/\",",
        "browser = \"https://app.example.test/\", form = \"https://auth.example.test\",",
    );
    let dir = project(&text);
    let opened = std::cell::Cell::new(false);
    let finds = |page: &str| {
        opened.set(true);
        Ok(page.to_owned())
    };
    let declared = declared_shop(dir.path());
    let same = super::judged(
        dir.path(),
        Whose {
            grants: "qa",
            reads_as: "qa",
        },
        "shop-login",
        &suggested("https://app.example.test/"),
        &finds,
        Some(&declared),
        &[],
    )
    .expect("judged");
    assert!(same.is_none(), "the declared login fills");
    let other_key = Suggested {
        key: "other".into(),
        ..suggested("https://elsewhere.example.test/")
    };
    let refused = super::judged(
        dir.path(),
        Whose {
            grants: "qa",
            reads_as: "qa",
        },
        "shop-login",
        &other_key,
        &finds,
        Some(&declared),
        &[],
    )
    .expect_err("refused");
    assert!(refused.contains("change its site alone"), "{refused}");
    assert!(!opened.get(), "no page was followed");
}

/// A declared login called on its own page fills as declared where the page can't be followed:
/// its fill keeps the password on its origin all the same.
#[test]
fn a_declared_login_on_its_own_page_fills_as_declared_where_the_page_cant_be_followed() {
    let dir = recorded();
    let judged = super::judged(
        dir.path(),
        Whose {
            grants: "qa",
            reads_as: "qa",
        },
        "shop-login",
        &suggested("https://app.example.test/"),
        &|_| Err("no login form appeared".to_owned()),
        Some(&declared_shop(dir.path())),
        &[],
    )
    .expect("judged");
    assert!(judged.is_none());
}

/// #5121: a login allowed from a notice is known by its name and its page, Always too.
#[test]
fn a_login_allowed_from_a_notice_is_known_by_its_name_and_its_page() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    let tool = proposal.tool().expect("a tool");
    let page = tool.login.as_ref().expect("a login").url.clone();
    assert!(!grants.allowed_from_a_notice(Some("qa"), "shop-login", &page));

    assert!(grants.allow(id, "qa", tool, false));

    assert!(grants.allowed_from_a_notice(Some("qa"), "shop-login", &page));
    assert!(!grants.allowed_from_a_notice(Some("qa"), "shop-login", "https://other.example.test/"));
    assert!(!grants.allowed_from_a_notice(Some("qa"), "other", &page));
    assert!(!grants.allowed_from_a_notice(Some("devops"), "shop-login", &page));
    assert!(!grants.allowed_from_a_notice(None, "shop-login", &page));
}

/// #5251: an Allow once a read took is the chat's again where its login was refused before it
/// ran, and spent where it ran, whatever came of it.
#[test]
fn allow_once_is_given_back_only_to_a_login_that_never_ran() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/login");
    let audited = Audited::default();
    answered(
        dir.path(),
        &grants,
        &audited,
        &press(id, Press::Once, &proposal.digest()),
    )
    .expect("allowed");

    assert!(grants.take(7, CHAT_7, Some("qa"), "shop-login").is_some());
    assert!(
        tool_of(&grants, "shop-login").is_none(),
        "taken by its read"
    );
    grants.answered(7, CHAT_7, Some("qa"), "shop-login", false);
    assert!(tool_of(&grants, "shop-login").is_some(), "the chat's again");
    // Another chat's or persona's answer gives nothing back to this one.
    grants.answered(8, None, Some("qa"), "shop-login", false);

    assert!(grants.take(7, CHAT_7, Some("qa"), "shop-login").is_some());
    grants.answered(7, CHAT_7, Some("qa"), "shop-login", true);
    assert!(tool_of(&grants, "shop-login").is_none(), "spent");
    grants.answered(7, CHAT_7, Some("qa"), "shop-login", false);
    assert!(tool_of(&grants, "shop-login").is_none(), "answered once");
}

/// A call cancelled while its notice waits is noticed: the notice goes, and nothing is held for
/// the chat's next call.
#[test]
fn a_call_whose_asker_hung_up_ends_its_wait_and_its_notice() {
    let dir = project("");
    let grants = Grants::default();
    let (id, _, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    let looked = std::cell::Cell::new(0);
    let gone = || {
        looked.set(looked.get() + 1);
        looked.get() >= 2
    };
    let started = Instant::now();

    let waited = waited_for_the_person(&grants, id, &heard, Duration::from_secs(600), &gone);

    assert_eq!(waited, Ended::AskerGone);
    assert!(
        started.elapsed() < Duration::from_secs(60),
        "it looked again, and stopped"
    );
    assert!(grants.waiting().is_empty(), "the notice went");
    assert!(tool_of(&grants, "shop-login").is_none());
}

#[test]
fn a_wait_nobody_answers_ends_its_notice_and_a_press_in_time_is_heard() {
    let dir = project("");
    let grants = Grants::default();
    let (id, _, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    let here = || false;
    assert_eq!(
        waited_for_the_person(&grants, id, &heard, Duration::from_millis(20), &here),
        Ended::NotAnswered
    );
    assert!(grants.waiting().is_empty(), "the notice went");

    let (id, _, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    grants.end(id, Heard::Kept(None));
    assert_eq!(
        waited_for_the_person(&grants, id, &heard, Duration::from_secs(600), &here),
        Ended::Heard(Heard::Kept(None))
    );
}

/// The person's ruling of 2026-11-07: a runner chat's notice, which no call waits on, goes after
/// as long as a waiting call waits, unless it was answered or its chat closed by then.
#[test]
fn a_notice_nobody_waits_on_goes_unanswered_after_its_time_unless_answered() {
    let dir = project("");
    let grants = Grants::default();
    let (id, _, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    let went = went_unanswered(&heard, Duration::from_millis(20), || {
        Some(&grants).filter(|grants| grants.end(id, Heard::Gone))
    });
    assert!(went.is_some(), "nobody answered, so it went");
    assert!(grants.waiting().is_empty());

    let (id, _, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    grants.end(id, Heard::Allowed);
    let went = went_unanswered(&heard, Duration::from_millis(20), || Some(()));
    assert!(went.is_none(), "an answer ends the watch");

    let (_, _, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    grants.forget(7);
    let went = went_unanswered(&heard, Duration::from_millis(20), || Some(()));
    assert!(went.is_none(), "a close ends the watch");
}

// ---- A login's keys are judged before its notice (#5831) ----

/// `qa/password` linked to `prod/password`: an entry of a vault `qa` may not use. Vault qa's own
/// value under `password` goes first, as a link never replaces one.
fn linked_out_of_reach(root: &Path) {
    std::fs::write(root.join("q.json"), r#"{"username":"u"}"#).expect("vault qa");
    std::fs::write(root.join("p.json"), r#"{"password":"n0t-yours"}"#).expect("vault prod");
    let ctx = Ctx::new(root, Env::from_process());
    let qa = purlis_core::secrets::cmd::provider(&ctx, "qa").expect("vault qa");
    purlis_core::secrets::links::link(&ctx, &qa, "password", "prod", "password").expect("linked");
}

/// What the chat hears names the key and its own vault, never the other vault or its value.
fn says_out_of_reach(why: &str) {
    assert!(
        why.contains("password in vault qa reads an entry of a vault qa may not use"),
        "{why}"
    );
    assert!(!why.contains("prod"), "names no other vault: {why}");
    assert!(!why.contains("n0t-yours"), "never the value: {why}");
}

/// A login that reads through a link into a vault its persona may not use is refused at the
/// judge, so no notice asks the person to allow it, or to grant its vault, for a read that is
/// then refused.
#[test]
fn a_login_with_a_key_out_of_reach_is_refused_before_its_notice() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    linked_out_of_reach(dir.path());

    let refused = super::judged(
        dir.path(),
        Whose {
            grants: "qa",
            reads_as: "qa",
        },
        "shop-login",
        &suggested("https://shop.example.test/login"),
        &|_| panic!("no page is followed for a login that is refused"),
        None,
        &[],
    )
    .map(|_| ())
    .expect_err("refused");

    says_out_of_reach(&refused);
}

/// A link made while the notice waits is judged at the press: Always is allowed nothing, writes
/// no line, and grants qa none of the vault it would have, and the chat hears the read's words.
#[test]
fn a_login_linked_out_of_reach_while_its_notice_waits_is_refused_at_the_press() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let root = dir.path();
    std::fs::write(root.join("p.json"), r#"{"username":"u"}"#).expect("vault prod");
    let grants = Grants::default();
    let from_prod = Suggested {
        vault: "prod".to_owned(),
        ..suggested("https://shop.example.test/login")
    };
    let judged = super::judged(
        root,
        Whose {
            grants: "qa",
            reads_as: "qa",
        },
        "shop-login",
        &from_prod,
        &on_its_page,
        None,
        &[],
    )
    .expect("asked")
    .expect("an ask");
    assert!(judged.vault_too, "Always would grant qa vault prod");
    let proposal = judged.proposal.clone();
    let (id, heard) = grants
        .raise(LoginWait {
            id: 0,
            session: 7,
            chat: CHAT_7.map(str::to_owned),
            proposal: judged.proposal,
            since: 1,
            vault_too: judged.vault_too,
            no_always: judged.no_always,
            pool: None,
            reads_as: "qa".to_owned(),
        })
        .expect("raised");
    // While it waits, prod's password is linked into vault `ops-only`, which qa may not use.
    std::fs::write(
        root.join("vaults.json"),
        r#"{"vaults":{
            "qa":{"provider":"plain-file","config":{"file":"q.json"},"persona":"qa"},
            "prod":{"provider":"plain-file","config":{"file":"p.json"},"persona":"ops"},
            "ops-only":{"provider":"plain-file","config":{"file":"x.json"},"persona":"ops"}
        }}"#,
    )
    .expect("a registry");
    std::fs::write(root.join("x.json"), r#"{"password":"n0t-yours"}"#).expect("ops-only");
    let ctx = Ctx::new(root, Env::from_process());
    let prod = purlis_core::secrets::cmd::provider(&ctx, "prod").expect("vault prod");
    purlis_core::secrets::links::link(&ctx, &prod, "password", "ops-only", "password")
        .expect("linked");
    let before = file_of(root);
    let audited = Audited::default();

    let said = answered(
        root,
        &grants,
        &audited,
        &press(id, Press::Always, &proposal.digest()),
    )
    .map(|_| ())
    .expect_err("refused");

    assert!(said.starts_with("Nothing was allowed: "), "{said}");
    assert!(
        said.contains("password in vault prod reads an entry of a vault qa may not use"),
        "{said}"
    );
    assert!(!said.contains("ops-only"), "names no other vault: {said}");
    assert!(!said.contains("n0t-yours"), "never the value: {said}");
    assert!(!said.contains("The person"), "the person reads you: {said}");
    assert!(said.contains("You can link it"), "{said}");
    assert_eq!(file_of(root), before, "no line written");
    assert!(audited.borrow().is_empty(), "nothing allowed is audited");
    assert!(tool_of(&grants, "shop-login").is_none(), "nothing allowed");
    assert!(
        purlis_core::sandbox::local::granted_vaults(root).is_empty(),
        "no vault granted"
    );
    // The notice stays, showing why, as for every press refused; Keep blocked still ends it.
    assert_eq!(grants.waiting().len(), 1, "the notice stays");
    assert!(heard.try_recv().is_err(), "the chat still waits");
}

/// A press on a narrowed chat's login is judged as the persona it was started as (#5831): a chat
/// started as ops, narrowed to qa's grants, is allowed a login whose key links into a vault ops
/// may use.
#[test]
fn a_narrowed_chats_login_press_is_judged_as_the_persona_it_was_started_as() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let root = dir.path();
    let grants = Grants::default();
    let judged = super::judged(
        root,
        Whose {
            grants: "qa",
            reads_as: "ops",
        },
        "shop-login",
        &suggested("https://shop.example.test/login"),
        &on_its_page,
        None,
        &[],
    )
    .expect("asked")
    .expect("an ask");
    let proposal = judged.proposal.clone();
    let (id, heard) = grants
        .raise(LoginWait {
            id: 0,
            session: 7,
            chat: CHAT_7.map(str::to_owned),
            proposal: judged.proposal,
            since: 1,
            vault_too: judged.vault_too,
            no_always: judged.no_always,
            pool: None,
            reads_as: "ops".to_owned(),
        })
        .expect("raised");
    // While it waits, qa's password is linked into vault prod, which ops may use and qa may not.
    linked_out_of_reach(root);

    answered(
        root,
        &grants,
        &Audited::default(),
        &press(id, Press::Once, &proposal.digest()),
    )
    .expect("allowed");

    assert_eq!(heard.try_recv(), Ok(Heard::Allowed));
}

/// A chat started as ops and narrowed to qa's grants reads ops's vaults, as its read judges a link
/// (#5831): a link into a vault ops may use is no reason to refuse its login's notice.
#[test]
fn a_narrowed_chats_login_is_judged_as_the_persona_it_was_started_as() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    linked_out_of_reach(dir.path());

    let judged = super::judged(
        dir.path(),
        Whose {
            grants: "qa",
            reads_as: "ops",
        },
        "shop-login",
        &suggested("https://shop.example.test/login"),
        &on_its_page,
        None,
        &[],
    );

    assert!(matches!(judged, Ok(Some(_))), "asked about: {judged:?}");
}

/// And the other way: a chat started as qa and narrowed to ops's grants is refused a link into a
/// vault qa may not use, though ops may use it.
#[test]
fn a_narrowed_chats_login_is_refused_a_link_its_started_persona_may_not_read() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    linked_out_of_reach(dir.path());

    let refused = super::judged(
        dir.path(),
        Whose {
            grants: "ops",
            reads_as: "qa",
        },
        "shop-login",
        &suggested("https://shop.example.test/login"),
        &|_| panic!("no page is followed for a login that is refused"),
        None,
        &[],
    )
    .map(|_| ())
    .expect_err("refused");

    says_out_of_reach(&refused);
}

/// A site the person typed is never followed for a login refused at the press (#5831): purlis
/// opens no browser to find its form, and the person reads why.
#[test]
fn a_typed_site_is_not_followed_for_a_login_linked_out_of_reach_while_its_notice_waits() {
    let dir = project("[sandbox]\nmode = \"on\"\n");
    let root = dir.path();
    let grants = Grants::default();
    let (id, proposal, heard) = raised(root, &grants, "https://shop.example.test/login");
    linked_out_of_reach(root);
    let digest = proposal.digest();
    let asked = Asked {
        typed: Some("app.example.test"),
        ..press(id, Press::Once, &digest)
    };

    let said = answered_finding(root, &grants, &Audited::default(), &asked, &|_| {
        panic!("no site is followed for a login refused")
    })
    .map(|_| ())
    .expect_err("refused");

    assert!(said.starts_with("Nothing was allowed: "), "{said}");
    says_out_of_reach(&said);
    assert_eq!(grants.waiting().len(), 1, "the notice stays");
    assert!(heard.try_recv().is_err(), "the chat still waits");
}
