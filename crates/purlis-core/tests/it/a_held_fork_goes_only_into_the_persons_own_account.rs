// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A fork is a held act on the person's press, into their own account and nowhere else**
//! (ADR 0128 D7, D8.6). Its source must be a recorded repo.

use purlis_core::forge::Kind;
use purlis_core::heldact::{
    Asked, Call, HeldActs, NotDone, Pressed, Recognised, Refused, Requested, Why, recognise,
};
use serde_json::json;

use crate::support::held_acts::{Fake, NoTrees, Rec, Second, press, repo, world};

fn fork() -> Requested {
    Requested::Fork {
        source: repo("acme/app"),
    }
}

#[test]
fn the_ask_names_the_source_and_the_persons_own_account_as_the_only_place_it_goes() {
    purlis_core::unsteered!();
    let (forge, held) = (Fake::github(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &fork(), 10) else {
        panic!()
    };
    let (shown, digest) = held.shown(id).unwrap();
    assert_eq!(shown.rows[0].value, "acme/app");
    assert_eq!(shown.rows[1].value, "your own account, aaron");
    assert!(forge.log().is_empty(), "an ask forks nothing");

    press(&held, &world, id, &digest).unwrap();
    assert!(forge.log().is_empty(), "one press forks nothing");
    let Ok(Pressed::Done(done)) = press(&held, &world, id, &digest) else {
        panic!()
    };
    assert_eq!(forge.log(), vec!["make_fork acme/app id-1".to_owned()]);
    // What it made is said, so the app records it as a second repo of the chat (D4).
    assert_eq!(done.forked.unwrap().path, "aaron/app");
}

#[test]
fn a_source_that_is_not_a_recorded_repo_is_refused_not_held() {
    purlis_core::unsteered!();
    let (forge, held) = (Fake::github(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let other = Requested::Fork {
        source: repo("someone/else"),
    };
    assert_eq!(
        held.ask(&world, 1, true, &other, 10),
        Asked::Refused(Refused::NotRecorded(repo("someone/else")))
    );
    assert!(held.every().is_empty());
}

#[test]
fn a_host_that_forbids_forking_leaves_the_person_read_only_and_says_so() {
    purlis_core::unsteered!();
    let (forge, held) = (Fake::github(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    forge.with(|s| s.may_fork = false);
    let world = world(&forge, &NoTrees, &second, &rec);
    assert_eq!(
        held.ask(&world, 1, true, &fork(), 10),
        Asked::Refused(Refused::ForkNotAllowed(repo("acme/app")))
    );
}

#[test]
fn a_fork_that_lands_outside_the_persons_account_is_not_recorded() {
    purlis_core::unsteered!();
    let (forge, held) = (Fake::github(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    forge.with(|s| s.fork_lands_in = Some("an-org".into()));
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &fork(), 10) else {
        panic!()
    };
    let (_, digest) = held.shown(id).unwrap();
    press(&held, &world, id, &digest).unwrap();
    let refused = press(&held, &world, id, &digest).unwrap_err();
    assert!(
        matches!(refused, NotDone::ForkElsewhere { .. }),
        "{refused:?}"
    );
}

#[test]
fn a_repo_deleted_and_made_again_under_the_name_is_not_the_one_asked_about() {
    purlis_core::unsteered!();
    let (forge, held) = (Fake::github(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &fork(), 10) else {
        panic!()
    };
    let (_, digest) = held.shown(id).unwrap();
    forge.with(|s| s.source_id = "id-2".into());
    let refused = press(&held, &world, id, &digest).unwrap_err();
    assert!(matches!(refused, NotDone::Changed { .. }), "{refused:?}");
    assert!(forge.log().is_empty());
}

#[test]
fn a_chats_fork_into_another_organization_or_namespace_is_refused_whoever_asks() {
    purlis_core::unsteered!();
    let rec = Rec::acme();
    let github_body = json!({"organization": "someone-else"});
    let segments: Vec<String> = ["repos", "acme", "app", "forks"]
        .map(str::to_owned)
        .to_vec();
    let call = Call {
        method: "POST",
        segments: &segments,
        query: &[],
        body: &github_body,
    };
    assert_eq!(
        recognise(Kind::GitHub, &call, &rec),
        Recognised::Refuse(Why::ForkElsewhere)
    );

    let gitlab_body = json!({"namespace_path": "other-group"});
    let segments: Vec<String> = ["api", "v4", "projects", "acme/app", "fork"]
        .map(str::to_owned)
        .to_vec();
    let call = Call {
        method: "POST",
        segments: &segments,
        query: &[],
        body: &gitlab_body,
    };
    assert_eq!(
        recognise(Kind::GitLab, &call, &rec),
        Recognised::Refuse(Why::ForkElsewhere)
    );
    // With none named, it is a fork the person is asked about.
    let none = json!({});
    let call = Call {
        method: "POST",
        segments: &segments,
        query: &[],
        body: &none,
    };
    assert_eq!(
        recognise(Kind::GitLab, &call, &rec),
        Recognised::Hold(Requested::Fork {
            source: repo("acme/app")
        })
    );
}
