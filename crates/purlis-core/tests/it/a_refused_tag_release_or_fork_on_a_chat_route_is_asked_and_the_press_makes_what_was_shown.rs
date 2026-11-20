// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The proxy's hand-off** (ADR 0128 D8.1): a tag, a release or a fork a chat sends to its forge
//! route is not forwarded. The proxy answers with the one sentence that names the ask, the person
//! is asked, and the press makes exactly the act the ask showed, as the person.

use std::sync::Arc;

use purlis_core::forge::proxy::{
    AllowedRepo, ChatId, ForgeProxy, Inbound, Kind, NoCredentials, NoObjects, Outcome, Refusal,
    RequestHead, Sentinel,
};
use purlis_core::heldact::{Handoff, HeldActs, HeldContext, OwnedWorld, Pressed};
use purlis_core::mergehold::{Repo, Request};

use crate::support::held_acts::{Fake, NoTrees, Rec, Second, press, sha};

struct App {
    forge: Arc<Fake>,
    attended: bool,
}

impl HeldContext for App {
    fn session(&self, chat: &ChatId) -> Option<u32> {
        (chat == &ChatId::new("chat-a")).then_some(7)
    }
    fn attended(&self) -> bool {
        self.attended
    }
    fn now(&self) -> u64 {
        100
    }
    fn world(&self, _: &ChatId, _: &Repo) -> Result<OwnedWorld, String> {
        Ok(OwnedWorld {
            forge: self.forge.clone(),
            trees: Arc::new(NoTrees),
            workflow: Arc::new(Second::there()),
            recorded: Arc::new(Rec::acme()),
        })
    }
    fn merge(&self, _: &ChatId, _: &Request) -> String {
        "merge held".to_owned()
    }
}

struct Scene {
    proxy: ForgeProxy,
    chat: ChatId,
    sentinel: Sentinel,
    forge: Arc<Fake>,
    held: Arc<HeldActs>,
}

fn scene(attended: bool) -> Scene {
    let forge = Arc::new(Fake::github());
    forge.with(|s| {
        s.revs.insert("main".into(), sha('1'));
        s.revs.insert(sha('2').as_str().into(), sha('2'));
        s.merged.insert(sha('1'));
    });
    let held = Arc::new(HeldActs::default());
    let handoff = Handoff::new(
        held.clone(),
        Arc::new(App {
            forge: forge.clone(),
            attended,
        }),
    );
    let proxy =
        ForgeProxy::new(Box::new(NoCredentials), Box::new(NoObjects)).with_held(Box::new(handoff));
    let chat = ChatId::new("chat-a");
    let sentinel = proxy.enrol(
        &chat,
        vec![AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/app", "main").unwrap()],
    );
    Scene {
        proxy,
        chat,
        sentinel,
        forge,
        held,
    }
}

impl Scene {
    fn send(&self, method: &str, target: &str, body: &str, bearer: &str) -> Outcome {
        let head = RequestHead::new(method, "api.github.com", 443, target);
        let headers = vec![
            ("Authorization".to_owned(), format!("Bearer {bearer}")),
            ("Content-Type".to_owned(), "application/json".to_owned()),
        ];
        self.proxy.handle(
            &self.chat,
            &Inbound {
                head: &head,
                headers: &headers,
                body: body.as_bytes(),
            },
        )
    }

    fn own(&self, method: &str, target: &str, body: &str) -> Outcome {
        self.send(method, target, body, self.sentinel.expose())
    }

    /// The two presses of ask `id`, as the window makes them.
    fn press_twice(&self, world: &purlis_core::heldact::World<'_>, id: u64) {
        let (_, digest) = self.held.shown(id).unwrap();
        assert!(matches!(
            press(&self.held, world, id, &digest),
            Ok(Pressed::AsksOnceMore { .. })
        ));
        assert!(matches!(
            press(&self.held, world, id, &digest),
            Ok(Pressed::Done(_))
        ));
    }
}

fn said(outcome: Outcome) -> String {
    match outcome {
        Outcome::Held(held) => held.said,
        other => panic!("expected a held act, got {other:?}"),
    }
}

#[test]
fn a_tag_the_chat_asks_for_is_refused_with_the_asks_name_and_made_only_by_the_press() {
    purlis_core::unsteered!();
    let s = scene(true);
    let said = said(s.own(
        "POST",
        "/repos/acme/app/git/refs",
        &format!(r#"{{"ref":"refs/tags/v1","sha":"{}"}}"#, sha('2').as_str()),
    ));
    assert!(
        said.starts_with("purlis holds tags. It has asked the person"),
        "{said}"
    );
    assert!(said.contains("Ask 1: tag v1 in acme/app."), "{said}");
    // Asking made nothing.
    assert!(s.forge.log().is_empty());
    let waiting = s.held.every();
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].session, 7);

    let (second, rec) = (Second::there(), Rec::acme());
    let world = crate::support::held_acts::world(&s.forge, &NoTrees, &second, &rec);
    s.press_twice(&world, 1);
    assert_eq!(
        s.forge.log(),
        [format!("create_tag acme/app v1 {}", sha('2').as_str())]
    );
}

#[test]
fn a_release_names_the_commit_main_was_at_and_the_press_makes_the_tag_then_the_release() {
    purlis_core::unsteered!();
    let s = scene(true);
    let said = said(s.own(
        "POST",
        "/repos/acme/app/releases",
        r#"{"tag_name":"v2","target_commitish":"main","name":"Two","body":"Notes here"}"#,
    ));
    assert!(said.contains("purlis holds releases"), "{said}");
    assert!(said.contains("Ask 1: release v2 in acme/app."), "{said}");
    // The branch moves after the ask: what is released stays what was shown.
    s.forge.with(|f| {
        f.revs.insert("main".into(), sha('3'));
    });
    let (second, rec) = (Second::there(), Rec::acme());
    let world = crate::support::held_acts::world(&s.forge, &NoTrees, &second, &rec);
    let (_, digest) = s.held.shown(1).unwrap();
    // The first press reads the forge again: what was shown still stands because the pinned
    // commit, not the branch's name, is what is read.
    let first = press(&s.held, &world, 1, &digest);
    assert!(first.is_ok(), "{first:?}");
    let done = press(&s.held, &world, 1, &digest);
    assert!(matches!(done, Ok(Pressed::Done(_))), "{done:?}");
    let log = s.forge.log();
    assert_eq!(log.len(), 2, "{log:?}");
    assert_eq!(
        log[0],
        format!("create_tag acme/app v2 {}", sha('1').as_str())
    );
    assert!(
        log[1].starts_with("create_release acme/app v2 at "),
        "{log:?}"
    );
    assert!(log[1].contains("notes=\"Notes here\""), "{log:?}");
}

#[test]
fn a_fork_goes_into_the_persons_own_account_and_a_fork_into_an_organization_is_refused_whole() {
    purlis_core::unsteered!();
    let s = scene(true);
    let said = said(s.own("POST", "/repos/acme/app/forks", "{}"));
    assert!(said.contains("purlis holds forks"), "{said}");
    let elsewhere = said_refused_whole(s.own(
        "POST",
        "/repos/acme/app/forks",
        r#"{"organization":"somewhere-else"}"#,
    ));
    assert!(
        elsewhere.contains("only into the person's own account"),
        "{elsewhere}"
    );
    assert_eq!(s.held.every().len(), 1);
    let (second, rec) = (Second::there(), Rec::acme());
    let world = crate::support::held_acts::world(&s.forge, &NoTrees, &second, &rec);
    s.press_twice(&world, 1);
    assert_eq!(s.forge.log(), ["make_fork acme/app id-1"]);
}

fn said_refused_whole(outcome: Outcome) -> String {
    said(outcome)
}

#[test]
fn nobody_present_means_nothing_is_asked_and_the_chat_is_told_so() {
    purlis_core::unsteered!();
    let s = scene(false);
    let said = said(s.own(
        "POST",
        "/repos/acme/app/git/refs",
        &format!(r#"{{"ref":"refs/tags/v1","sha":"{}"}}"#, sha('2').as_str()),
    ));
    assert!(
        said.contains("Held: nobody is here to approve it."),
        "{said}"
    );
    assert!(s.held.every().is_empty());
}

#[test]
fn a_request_without_the_chats_sentinel_asks_nobody() {
    purlis_core::unsteered!();
    let s = scene(true);
    let out = s.send(
        "POST",
        "/repos/acme/app/git/refs",
        &format!(r#"{{"ref":"refs/tags/v1","sha":"{}"}}"#, sha('2').as_str()),
        "not-the-sentinel",
    );
    // Refused as any request of a chat that proved nothing is: no sentence, no ask.
    assert!(matches!(out, Outcome::Refused(_)), "{out:?}");
    assert!(s.held.every().is_empty());
}

#[test]
fn a_repo_the_chat_does_not_hold_asks_nobody() {
    purlis_core::unsteered!();
    let s = scene(true);
    let out = s.own(
        "POST",
        "/repos/acme/other/git/refs",
        &format!(r#"{{"ref":"refs/tags/v1","sha":"{}"}}"#, sha('2').as_str()),
    );
    assert!(
        matches!(out, Outcome::Refused(Refusal::OtherRepo)),
        "{out:?}"
    );
    assert!(s.held.every().is_empty());
}

#[test]
fn moving_or_deleting_a_tag_is_refused_whole_and_asks_nobody() {
    purlis_core::unsteered!();
    let s = scene(true);
    let moved = said(s.own(
        "PATCH",
        "/repos/acme/app/git/refs/tags/v1",
        r#"{"sha":"2222222222222222222222222222222222222222","force":true}"#,
    ));
    assert!(moved.contains("never moves a tag"), "{moved}");
    assert!(s.held.every().is_empty());
}

#[test]
fn without_a_hand_off_the_same_requests_are_refused_as_before() {
    purlis_core::unsteered!();
    let proxy = ForgeProxy::new(Box::new(NoCredentials), Box::new(NoObjects));
    let chat = ChatId::new("chat-a");
    let sentinel = proxy.enrol(
        &chat,
        vec![AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/app", "main").unwrap()],
    );
    let head = RequestHead::new("POST", "api.github.com", 443, "/repos/acme/app/releases");
    let headers = vec![
        (
            "Authorization".to_owned(),
            format!("Bearer {}", sentinel.expose()),
        ),
        ("Content-Type".to_owned(), "application/json".to_owned()),
    ];
    let out = proxy.handle(
        &chat,
        &Inbound {
            head: &head,
            headers: &headers,
            body: br#"{"tag_name":"v1"}"#,
        },
    );
    assert!(
        matches!(out, Outcome::Refused(Refusal::ReleaseOrTag)),
        "{out:?}"
    );
}
