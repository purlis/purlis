// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **No held act happens without the person's press** (ADR 0128 D8, D8.4, D8.5): nobody present
//! means nothing is performed; no rule, grant or mode presses in the person's place; a press is
//! made only from a connection the host admitted as the person's.

use purlis_core::askrules::{Rule, Scope, Store};
use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::heldact::{self, Asked, HeldActs, NOBODY_HERE, NotDone, Refused, Requested, said};

use crate::support::held_acts::{Fake, NoTrees, Rec, Second, repo, sha, tag, the_person, world};

fn every_kind() -> Vec<Requested> {
    vec![
        Requested::Tag {
            repo: repo("acme/app"),
            name: tag("v1"),
            rev: "main".into(),
        },
        Requested::Release {
            repo: repo("acme/app"),
            tag: tag("v1"),
            rev: None,
            title: String::new(),
            notes: String::new(),
            draft: false,
            prerelease: false,
        },
        Requested::Fork {
            source: repo("acme/app"),
        },
        Requested::CiPush {
            repo: repo("acme/app"),
            branch: crate::support::held_acts::base("feature"),
            tip: sha('2'),
        },
    ]
}

#[test]
fn unattended_every_kind_is_refused_with_the_one_sentence_and_nothing_is_performed() {
    purlis_core::unsteered!();
    let forge = Fake::github();
    forge.with(|s| {
        s.revs.insert("main".into(), sha('1'));
    });
    let (second, rec, held) = (Second::there(), Rec::acme(), HeldActs::default());
    let world = world(&forge, &NoTrees, &second, &rec);
    for requested in every_kind() {
        let asked = held.ask(&world, 1, false, &requested, 10);
        assert_eq!(
            asked,
            Asked::Refused(Refused::Unattended { queued: true }),
            "{requested:?}"
        );
        let said = said::not_asked(&requested, &Refused::Unattended { queued: true });
        assert!(said.ends_with(NOBODY_HERE), "{said}");
    }
    assert_eq!(NOBODY_HERE, "Held: nobody is here to approve it.");
    assert!(forge.log().is_empty());
    assert!(held.every().is_empty());
    assert_eq!(second.reads(), 0);
    // They wait for a desktop that connects, which asks again and reads again.
    assert_eq!(held.take_queued().len(), 4);
    assert!(held.take_queued().is_empty());
}

#[test]
fn the_same_act_is_queued_once_however_often_the_unattended_chat_tries() {
    purlis_core::unsteered!();
    let forge = Fake::github();
    let (second, rec, held) = (Second::there(), Rec::acme(), HeldActs::default());
    let world = world(&forge, &NoTrees, &second, &rec);
    let one = &every_kind()[0];
    held.ask(&world, 1, false, one, 10);
    let again = held.ask(&world, 1, false, one, 20);
    assert_eq!(again, Asked::Refused(Refused::Unattended { queued: false }));
    assert_eq!(held.take_queued().len(), 1);
}

fn a_rule_for(source: &str) -> Rule {
    Rule {
        id: "rule-1".to_owned(),
        at: 1_000,
        project: "alpha".to_owned(),
        root: "/plane".into(),
        session: 4,
        request: "acme/app v1".to_owned(),
        source: source.to_owned(),
        says: "Tag v1 of acme/app".to_owned(),
        label: "Tag".to_owned(),
        option: "tag".to_owned(),
        scope: Scope::Chat,
        grant: Some("tag acme/app v1".to_owned()),
        declared: None,
    }
}

#[test]
fn no_rule_can_be_made_from_the_answer_to_a_held_act() {
    purlis_core::unsteered!();
    let data = tempfile::tempdir().unwrap();
    let store = Store::in_data(data.path());
    for scope in [Scope::Chat, Scope::Project, Scope::Machine] {
        let refused = store
            .make(Rule {
                scope,
                ..a_rule_for(heldact::SOURCE)
            })
            .unwrap_err();
        assert_eq!(
            refused.kind(),
            std::io::ErrorKind::InvalidInput,
            "{scope:?}"
        );
        assert!(
            refused.to_string().contains("never remembered"),
            "{refused}"
        );
    }
    assert!(store.read().rules.is_empty());
}

#[test]
fn only_the_windows_own_connection_can_press() {
    purlis_core::unsteered!();
    // A chat, an editor, a terminal and the fleet's MCP server are never an answerer, and
    // `charter inbox` (approval) is one for asks in general but not for a held forge act (D5).
    for scope in [
        Admitted::Chat,
        Admitted::Editor,
        Admitted::Terminal,
        Admitted::FleetMcp,
    ] {
        assert!(Answerer::admitted(scope).is_none(), "{scope:?}");
    }
    assert!(Answerer::admitted(Admitted::LocalUi).is_some());

    let forge = Fake::github();
    forge.with(|s| {
        s.revs.insert("main".into(), sha('1'));
    });
    let (second, rec, held) = (Second::there(), Rec::acme(), HeldActs::default());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &every_kind()[0], 10) else {
        panic!()
    };
    let (_, digest) = held.shown(id).unwrap();
    let approval = Answerer::admitted(Admitted::Approval).unwrap();
    // From `charter inbox`: neither press, nor Don't, is the window's.
    assert_eq!(
        held.press(&world, id, &digest, None, approval).unwrap_err(),
        NotDone::NotTheWindow
    );
    assert_eq!(held.dont(id, approval).unwrap_err(), NotDone::NotTheWindow);
    assert_eq!(held.every().len(), 1, "the ask waits, untouched");
    assert!(forge.log().is_empty());
    // The window's Don't ends it.
    assert_eq!(held.dont(id, the_person()), Ok(true));
}

#[test]
fn a_press_takes_two_presses_for_every_kind() {
    purlis_core::unsteered!();
    assert_eq!(heldact::PRESSES, 2);
}
