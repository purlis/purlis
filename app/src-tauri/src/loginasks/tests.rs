// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One notice for a login a chat suggests (#4595): a press answers exactly what it was shown,
//! Once and this chat write nothing, and Always blesses nothing nobody saw.

use std::cell::RefCell;
use std::path::Path;

use purlis_core::loginask::{Proposal, Suggested};
use purlis_core::sandbox::tools::Tool;

use super::*;

const VAULTS: &str = r#"{"vaults":{
    "qa":{"provider":"plain-file","config":{"file":"q.json"},"persona":"qa"},
    "prod":{"provider":"plain-file","config":{"file":"p.json"},"persona":"ops"}
}}"#;

fn project(file: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::write(dir.path().join("vaults.json"), VAULTS).expect("a registry");
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

fn raised(root: &Path, grants: &Grants, site: &str) -> (u32, Proposal, mpsc::Receiver<Heard>) {
    let judged = judged(root, "qa", "shop-login", &suggested(site), &[]).expect("asked");
    let proposal = judged.proposal.clone();
    let (id, heard) = grants
        .raise(LoginWait {
            id: 0,
            session: 7,
            proposal: judged.proposal,
            since: 1,
            vault_too: judged.vault_too,
            no_always: judged.no_always,
            pool: None,
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
    answer(root, grants, asked, &|_, one| {
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
    grants.granted(7, Some("qa"), name)
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
    assert!(grants.take(7, Some("qa"), "shop-login").is_some());
    assert!(tool_of(&grants, "shop-login").is_none(), "used once");
    assert!(grants.take(7, Some("qa"), "shop-login").is_none());
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
    assert!(grants.take(7, Some("qa"), "shop-login").is_some());
    assert!(tool_of(&grants, "shop-login").is_some());
    assert!(grants.granted(8, Some("qa"), "shop-login").is_none());
    assert!(grants.granted(7, Some("ops"), "shop-login").is_none());
    grants.forget(7);
    assert!(
        tool_of(&grants, "shop-login").is_none(),
        "a later chat dealt 7 gets nothing"
    );
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
        proposal: other.proposal,
        since: 2,
        vault_too: false,
        no_always: None,
        pool: None,
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
    let asked = Asked {
        typed: Some("shop.example.test"),
        ..press(id, Press::Once, &proposal.digest())
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
    let asked = Asked {
        typed: Some("pаypal.example.test"),
        ..press(id, Press::Once, &proposal.digest())
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
    let asked = Asked {
        typed: Some("http://shop.example.test"),
        ..press(id, Press::Once, &proposal.digest())
    };
    assert!(answered(dir.path(), &grants, &audited, &asked).is_err());
    assert_eq!(grants.waiting().len(), 1);
}

#[test]
fn a_warning_that_came_up_while_the_notice_was_drawn_is_drawn_before_a_press_counts() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    grants.grant(9, "qa", other_site(dir.path()), true);
    grants.take(9, Some("qa"), "other");
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
fn a_vault_the_project_doesnt_have_is_never_asked_about() {
    let dir = project("");
    let unknown = Suggested {
        vault: "nope".to_owned(),
        ..suggested("https://shop.example.test/")
    };
    assert!(judged(dir.path(), "qa", "shop-login", &unknown, &[]).is_err());
}

#[test]
fn two_keep_blocked_end_a_chats_login_notices() {
    let grants = Grants::default();
    grants.keep(7, "one", "https://a.example.test");
    assert!(!grants.kept(7, "two", "https://b.example.test"));
    grants.keep(7, "two", "https://b.example.test");
    assert!(grants.kept(7, "three", "https://c.example.test"));
    assert!(!grants.kept(8, "three", "https://c.example.test"));
}

#[test]
fn an_empty_field_never_falls_back_to_the_chats_site() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, _heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    let audited = Audited::default();
    let asked = Asked {
        typed: Some("  "),
        ..press(id, Press::Once, &proposal.digest())
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
    let asked = Asked {
        typed: Some("shop.example.test"),
        ..press(id, Press::Always, &proposal.digest())
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
fn keep_blocked_is_not_asked_again_in_that_chat() {
    let dir = project("");
    let grants = Grants::default();
    let (id, proposal, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    let audited = Audited::default();
    let asked = Asked {
        reason: Some("not that site"),
        ..press(id, Press::Keep, &proposal.digest())
    };
    answered(dir.path(), &grants, &audited, &asked).expect("kept");
    assert_eq!(
        heard.try_recv(),
        Ok(Heard::Kept(Some("not that site".to_owned())))
    );
    assert!(grants.kept(7, "shop-login", "https://shop.example.test"));
    assert!(!grants.kept(8, "shop-login", "https://shop.example.test"));
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
    let dir = project("[sandbox]\nmode = \"on\"\nproposed-changes = \"off\"\n");
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
fn a_closed_chat_stops_its_call_waiting() {
    let dir = project("");
    let grants = Grants::default();
    let (_, _, heard) = raised(dir.path(), &grants, "https://shop.example.test/");
    grants.forget(7);
    assert_eq!(heard.try_recv(), Ok(Heard::Gone));
    assert!(grants.waiting().is_empty());
}
