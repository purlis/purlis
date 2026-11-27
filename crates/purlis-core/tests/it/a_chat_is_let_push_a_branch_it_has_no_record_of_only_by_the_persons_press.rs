// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A branch pushed before the route existed has no owner, and only the person's press gives it
//! one** (ADR 0128, amended 2026-11-23, #7363). The route refuses a chat's update of an existing
//! branch that no chat owns in the app's record, or another does; it raises one ask, **Let this
//! chat push it**, and tells git so. The person's Allow records the chat as the owner at the tip
//! the forge holds when they press; Keep blocked leaves it refused.
//!
//! Driven as the app drives it: a chat's push through the listener ([`World::asking_in`], whose
//! ask is [`windowonly::PushAsks`]), then the person's press ([`windowonly::let_chat_push`],
//! [`windowonly::keep_push_blocked`]) with the forge's answer a stand-in for the person's read.

use crate::support::forge_listener::{ACME_APP_ID, Answer, PROJECT_ID, SESSION, World, basic, pkt};

use std::path::{Path, PathBuf};

use purlis_core::forge::createdbranches;
use purlis_core::forge::route::HostScope;
use purlis_core::windowonly::{self, Control};

const CHAT: &str = "chat-listener";
const HOST: &str = "github.com";
const REPO: &str = "acme/app";
const OTHER_PROJECT_ID: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S2Q";

/// A project folder whose `charter.toml` keeps the stable id `id`.
fn project(id: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(
        root.join("charter.toml"),
        format!("schema = 1\n\n[project]\nid = \"{id}\"\n"),
    )
    .unwrap();
    (dir, root)
}

fn reply(w: &World, branch: &str, old: &str, new: &str) -> crate::support::forge_listener::Reply {
    reply_to(w, &[(old, new, &format!("refs/heads/{branch}"))])
}

/// A push of `commands`, `(old, new, ref name)` each, through the listener.
fn reply_to(w: &World, commands: &[(&str, &str, &str)]) -> crate::support::forge_listener::Reply {
    let mut body = String::new();
    for (i, (old, new, name)) in commands.iter().enumerate() {
        let caps = if i == 0 {
            "\0 report-status side-band-64k agent=git/2"
        } else {
            ""
        };
        body.push_str(&pkt(&format!("{old} {new} {name}{caps}")));
    }
    body.push_str("0000PACK....");
    let branch = commands[0].2.trim_start_matches("refs/heads/");
    let request = format!(
        "POST /acme/app.git/git-receive-pack HTTP/1.1\r\nHost: github.com\r\nAuthorization: {}\r\nContent-Type: application/x-git-receive-pack-request\r\nContent-Length: {}\r\n\r\n{body}",
        basic("x-access-token", w.sentinel.expose()),
        body.len()
    );
    let ca = purlis_core::forge::proxy::listener::ChatCa::new(&w.proxy.terminable_hosts(&w.chat))
        .unwrap();
    w.fake.answer(Answer::ok(&format!(
        "{}{}0000",
        pkt("unpack ok\n"),
        pkt(&format!("ok refs/heads/{branch}\n"))
    )));
    crate::support::forge_listener::tls(w, &ca, HOST, 443, HOST, request.as_bytes())
        .expect("a handshake")
}

/// Whether the route refused the push itself, in git's words, and the text it said.
fn refused(w: &World, branch: &str, old: &str, new: &str) -> Option<String> {
    let reply = reply(w, branch, old, new);
    let text = reply.text();
    (reply.status == 200 && text.contains(&format!("ng refs/heads/{branch} purlis: ")))
        .then_some(text)
}

fn tip_of(config: &Path, chat: &str, branch: &str) -> Option<String> {
    createdbranches::tip_of(config, PROJECT_ID, chat, HOST, ACME_APP_ID, branch)
}

fn control(branch: &str) -> Control {
    Control::LetChatPush {
        chat: CHAT.to_owned(),
        host: HOST.to_owned(),
        repo: REPO.to_owned(),
        repo_id: ACME_APP_ID,
        branch: branch.to_owned(),
    }
}

/// What the person's read of the forge says the branch is at.
fn forge_at(tip: &str) -> impl Fn(&str, &str, &str) -> Result<String, String> + '_ {
    move |_, _, _| Ok(tip.to_owned())
}

#[test]
fn refused_then_allow_then_the_retry_from_the_recorded_tip_is_allowed() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    // The branch was pushed by the person before the route existed. The chat read an older tip
    // of it (its claimed old id), and the forge has moved on since.
    let (claimed, on_forge, next) = ("3".repeat(40), "4".repeat(40), "5".repeat(40));

    let text = refused(&w, "theirs", &claimed, &next).expect("an unowned branch is refused");
    assert!(
        text.contains("Let this chat push it is waiting in the window"),
        "git says an ask is waiting: {text}"
    );
    assert!(w.fake.requests().is_empty(), "never sent");
    // One ask, however often the chat tries again.
    refused(&w, "theirs", &claimed, &next).expect("still refused");
    let waiting = windowonly::every(&root);
    assert_eq!(waiting.len(), 1, "{waiting:?}");
    assert_eq!(waiting[0].session, SESSION);
    assert_eq!(waiting[0].control, control("theirs"));
    assert_eq!(waiting[0].control.label(), "Allow");
    // Nothing is recorded before the person answers.
    assert_eq!(tip_of(config.path(), CHAT, "theirs"), None);

    // Allow: the tip recorded is the forge's, read at the press, never the chat's claimed id.
    let recorded = windowonly::let_chat_push(
        &HostScope::for_a_test(),
        config.path(),
        &root,
        SESSION,
        &control("theirs"),
        &forge_at(&on_forge),
    )
    .unwrap();
    assert_eq!(recorded, on_forge);
    assert_eq!(
        tip_of(config.path(), CHAT, "theirs"),
        Some(on_forge.clone())
    );
    assert!(windowonly::every(&root).is_empty(), "the ask is answered");

    // The tip rule applies as usual: the chat's claimed id is stale and is refused, with no new ask;
    // the recorded tip is allowed, and the confirmed push moves it.
    let stale = refused(&w, "theirs", &claimed, &next).expect("a stale tip is refused");
    assert!(!stale.contains("is waiting in the window"), "{stale}");
    assert!(
        windowonly::every(&root).is_empty(),
        "a stale tip asks nothing"
    );
    assert!(
        refused(&w, "theirs", &on_forge, &next).is_none(),
        "the retry from the recorded tip is let through"
    );
    assert_eq!(w.fake.requests().len(), 1, "sent once");
    assert_eq!(tip_of(config.path(), CHAT, "theirs"), Some(next));
    windowonly::project_ended(&root);
}

#[test]
fn keep_blocked_leaves_it_refused_and_the_chat_is_not_asked_again() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    let (old, new) = ("3".repeat(40), "5".repeat(40));
    refused(&w, "theirs", &old, &new).expect("refused");
    assert_eq!(windowonly::every(&root).len(), 1);

    windowonly::keep_push_blocked(&root, SESSION, &control("theirs")).unwrap();
    assert!(windowonly::every(&root).is_empty(), "the ask is answered");
    // Allow can no longer be pressed on it, and records nothing.
    assert!(
        windowonly::let_chat_push(
            &HostScope::for_a_test(),
            config.path(),
            &root,
            SESSION,
            &control("theirs"),
            &forge_at(&old)
        )
        .is_err()
    );

    let text = refused(&w, "theirs", &old, &new).expect("still refused");
    assert!(
        text.contains("kept"),
        "git says it was kept blocked: {text}"
    );
    assert!(
        windowonly::every(&root).is_empty(),
        "and no second ask is raised"
    );
    assert_eq!(tip_of(config.path(), CHAT, "theirs"), None);
    assert!(!createdbranches::file_in(config.path()).exists());
    assert!(w.fake.requests().is_empty(), "never sent");
    windowonly::project_ended(&root);
}

/// How every family runs where the project's `[policy]` sets `consents` to `consents`, as the
/// app would answer it, for this test's thread alone (#7153): put away when dropped.
struct ConsentsHere;

impl ConsentsHere {
    fn at(consents: &str) -> Self {
        use purlis_core::enforcement::{self, Family, Mode, Resolved, Settings};
        let resolved = Resolved::of(
            &Settings::from_text(Some(&format!("[policy]\nconsents = \"{consents}\"\n"))),
            &purlis_core::sandbox::policy::Locks::none(),
        );
        assert_eq!(
            resolved.mode(Family::Consents).word(),
            Mode::parse(consents).unwrap().word()
        );
        enforcement::answered_on_this_thread(Some(resolved));
        Self
    }
}

impl Drop for ConsentsHere {
    fn drop(&mut self) {
        purlis_core::enforcement::answered_on_this_thread(None);
    }
}

/// A push of `branch` from `old` to `new`, handed to the route on this thread, so the policy
/// this thread was answered is the one it reads.
fn handled(w: &World, branch: &str, old: &str, new: &str) -> purlis_core::forge::proxy::Outcome {
    use purlis_core::forge::proxy::{Inbound, RequestHead};
    let mut body = pkt(&format!(
        "{old} {new} refs/heads/{branch}\0 report-status side-band-64k agent=git/2"
    ));
    body.push_str("0000PACK....");
    let head = RequestHead::new("POST", HOST, 443, "/acme/app.git/git-receive-pack")
        .with_content_type(Some("application/x-git-receive-pack-request"));
    let headers = vec![
        (
            "Authorization".to_owned(),
            basic("x-access-token", w.sentinel.expose()),
        ),
        (
            "Content-Type".to_owned(),
            "application/x-git-receive-pack-request".to_owned(),
        ),
    ];
    w.proxy.handle(
        &w.chat,
        &Inbound {
            head: &head,
            headers: &headers,
            body: body.as_bytes(),
        },
    )
}

#[test]
fn a_not_your_branch_push_stays_refused_without_the_press_under_consents_off_and_log() {
    purlis_core::unsteered!();
    // ADR 0125, amended 2026-11-27, §10 (#7749): turned down, `consents` still raises the ask,
    // and nothing loosens. The push is refused until the person presses Allow.
    use purlis_core::forge::proxy::{Outcome, Refusal};
    for consents in ["off", "log"] {
        let _here = ConsentsHere::at(consents);
        let config = tempfile::tempdir().unwrap();
        let (_p, root) = project(PROJECT_ID);
        let w = World::asking_in(config.path().to_owned(), root.clone());
        let (old, new) = ("3".repeat(40), "5".repeat(40));

        for attempt in 0..2 {
            match handled(&w, "theirs", &old, &new) {
                Outcome::Held(held) => {
                    assert_eq!(held.refusal, Refusal::NotYourBranch, "{consents}");
                    assert!(
                        held.said
                            .contains("Let this chat push it is waiting in the window"),
                        "{consents}, attempt {attempt}: {}",
                        held.said
                    );
                }
                other => panic!("{consents}, attempt {attempt}: no ask was raised: {other:?}"),
            }
        }
        let waiting = windowonly::every(&root);
        assert_eq!(waiting.len(), 1, "{consents}: {waiting:?}");
        assert_eq!(waiting[0].control, control("theirs"), "{consents}");
        // Until the press nothing is recorded or sent.
        assert_eq!(tip_of(config.path(), CHAT, "theirs"), None, "{consents}");
        assert!(w.fake.requests().is_empty(), "{consents}: never sent");

        // The press is still the one way through.
        windowonly::let_chat_push(
            &HostScope::for_a_test(),
            config.path(),
            &root,
            SESSION,
            &control("theirs"),
            &forge_at(&old),
        )
        .unwrap();
        assert!(
            matches!(handled(&w, "theirs", &old, &new), Outcome::Forward(_)),
            "{consents}: let through after the press"
        );
        windowonly::project_ended(&root);
    }
}

#[test]
fn a_protected_default_deploy_or_pages_branch_raises_no_ask() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    let (old, new) = ("3".repeat(40), "5".repeat(40));
    for branch in [
        "main",
        "MAIN",
        "gh-pages",
        "deploy",
        "release/1.0",
        "production/x",
    ] {
        let text = refused(&w, branch, &old, &new).unwrap_or_else(|| panic!("{branch} is refused"));
        assert!(
            !text.contains("is waiting in the window"),
            "{branch}: {text}"
        );
    }
    assert!(windowonly::every(&root).is_empty());
    assert!(w.fake.requests().is_empty(), "never sent");
    windowonly::project_ended(&root);
}

#[test]
fn a_branch_the_chat_owns_at_another_tip_raises_no_ask() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    // The chat makes the branch (a create is no update), then pushes from a tip it did not leave.
    assert!(refused(&w, "mine", &"0".repeat(40), &"1".repeat(40)).is_none());
    let text = refused(&w, "mine", &"9".repeat(40), &"2".repeat(40)).expect("refused");
    assert!(!text.contains("is waiting in the window"), "{text}");
    assert!(windowonly::every(&root).is_empty());
    windowonly::project_ended(&root);
}

#[test]
fn an_ask_of_another_projects_chat_cannot_grant_this_one() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_a, root_a) = project(PROJECT_ID);
    let (_b, root_b) = project(OTHER_PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root_a.clone());
    let (old, new, on_forge) = ("3".repeat(40), "5".repeat(40), "4".repeat(40));
    refused(&w, "theirs", &old, &new).expect("refused");
    assert_eq!(windowonly::every(&root_a).len(), 1);

    // The other project's window presses the same control, for the same session: nothing waits
    // there, so nothing is recorded, for either project, and the first ask still waits.
    let wrong = windowonly::let_chat_push(
        &HostScope::for_a_test(),
        config.path(),
        &root_b,
        SESSION,
        &control("theirs"),
        &forge_at(&on_forge),
    );
    assert!(wrong.is_err(), "{wrong:?}");
    assert!(windowonly::keep_push_blocked(&root_b, SESSION, &control("theirs")).is_err());
    assert!(!createdbranches::file_in(config.path()).exists());
    assert_eq!(windowonly::every(&root_a).len(), 1, "the ask still waits");

    // A control no chat was refused for, and another chat's session, grant nothing either.
    assert!(
        windowonly::let_chat_push(
            &HostScope::for_a_test(),
            config.path(),
            &root_a,
            SESSION,
            &control("never-asked"),
            &forge_at(&on_forge)
        )
        .is_err()
    );
    assert!(
        windowonly::let_chat_push(
            &HostScope::for_a_test(),
            config.path(),
            &root_a,
            SESSION + 1,
            &control("theirs"),
            &forge_at(&on_forge)
        )
        .is_err()
    );
    assert!(!createdbranches::file_in(config.path()).exists());

    // And an allow records under the pressing project's own id: the other project's chat of the
    // same id owns nothing.
    windowonly::let_chat_push(
        &HostScope::for_a_test(),
        config.path(),
        &root_a,
        SESSION,
        &control("theirs"),
        &forge_at(&on_forge),
    )
    .unwrap();
    assert_eq!(tip_of(config.path(), CHAT, "theirs"), Some(on_forge));
    assert_eq!(
        createdbranches::tip_of(
            config.path(),
            OTHER_PROJECT_ID,
            CHAT,
            HOST,
            ACME_APP_ID,
            "theirs"
        ),
        None
    );
    windowonly::project_ended(&root_a);
    windowonly::project_ended(&root_b);
}

#[test]
fn a_forge_that_does_not_answer_a_commit_records_nothing_and_the_ask_still_waits() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    refused(&w, "theirs", &"3".repeat(40), &"5".repeat(40)).expect("refused");

    let down = |_: &str, _: &str, _: &str| Err::<String, String>("the forge is down".to_owned());
    assert!(
        windowonly::let_chat_push(
            &HostScope::for_a_test(),
            config.path(),
            &root,
            SESSION,
            &control("theirs"),
            &down
        )
        .is_err()
    );
    assert!(
        windowonly::let_chat_push(
            &HostScope::for_a_test(),
            config.path(),
            &root,
            SESSION,
            &control("theirs"),
            &forge_at("not-a-commit")
        )
        .is_err()
    );
    assert!(!createdbranches::file_in(config.path()).exists());
    assert_eq!(
        windowonly::every(&root).len(),
        1,
        "the person may press again"
    );
    windowonly::project_ended(&root);
}

fn ask_count(root: &Path) -> usize {
    windowonly::every(root).len()
}

#[test]
fn one_push_raises_one_ask_and_a_chat_has_three_waiting_at_most() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    let (old, new) = ("3".repeat(40), "5".repeat(40));
    let update = |b: &'static str| (old.as_str(), new.as_str(), format!("refs/heads/{b}"));

    // Three branches in one push: one ask, for the first, and git says which.
    let cmds: Vec<_> = ["b1", "b2", "b3"].map(update).into();
    let refs: Vec<_> = cmds.iter().map(|(o, n, r)| (*o, *n, r.as_str())).collect();
    let text = reply_to(&w, &refs).text();
    assert!(text.contains("b1 in acme/app"), "{text}");
    assert_eq!(ask_count(&root), 1);

    // Each push after it raises one more, until three wait; the fourth raises none.
    reply_to(&w, &refs);
    assert_eq!(ask_count(&root), 2);
    reply_to(&w, &refs);
    assert_eq!(ask_count(&root), 3);
    let four = [update("b4")];
    let refs: Vec<_> = four.iter().map(|(o, n, r)| (*o, *n, r.as_str())).collect();
    let text = reply_to(&w, &refs).text();
    assert_eq!(ask_count(&root), 3, "past the cap no ask is raised");
    assert!(text.contains("already waiting"), "git says why: {text}");
    assert!(w.fake.requests().is_empty(), "never sent");

    // Answering one makes room for the next.
    windowonly::keep_push_blocked(&root, SESSION, &control("b1")).unwrap();
    reply_to(&w, &refs);
    assert_eq!(ask_count(&root), 3);
    assert!(
        windowonly::every(&root)
            .iter()
            .any(|r| r.control == control("b4"))
    );
    windowonly::project_ended(&root);
}

#[test]
fn an_invented_name_raises_no_ask() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    let (old, new) = ("3".repeat(40), "5".repeat(40));
    for branch in ["invented", "-dash"] {
        let text = refused(&w, branch, &old, &new).unwrap_or_else(|| panic!("{branch} refused"));
        assert!(
            !text.contains("is waiting in the window"),
            "{branch}: {text}"
        );
    }
    assert_eq!(ask_count(&root), 0);
    windowonly::project_ended(&root);
}

#[test]
fn a_delete_a_tag_or_a_protected_ref_beside_an_unowned_branch_raises_no_ask() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let w = World::asking_in(config.path().to_owned(), root.clone());
    let (old, new, zero) = ("3".repeat(40), "5".repeat(40), "0".repeat(40));
    let theirs = (old.as_str(), new.as_str(), "refs/heads/theirs");
    for beside in [
        (old.as_str(), zero.as_str(), "refs/heads/other"),
        (old.as_str(), new.as_str(), "refs/tags/v1"),
        (old.as_str(), new.as_str(), "refs/heads/main"),
    ] {
        let alone = reply_to(&w, &[beside]).text();
        assert!(
            !alone.contains("is waiting in the window"),
            "{beside:?}: {alone}"
        );
        let mixed = reply_to(&w, &[theirs, beside]).text();
        assert!(
            !mixed.contains("is waiting in the window"),
            "{beside:?}: {mixed}"
        );
    }
    assert_eq!(ask_count(&root), 0);
    assert!(w.fake.requests().is_empty(), "never sent");
    windowonly::project_ended(&root);
}

// ---- after Allow, the branch moves only forward (#7841) ----

/// A push gate that sends on only what the core does not mark as fast-forward only, and keeps
/// every update it was handed. The app's gate reads the push's objects for the mark
/// (`forgewiring`'s tests); this one shows what the core hands it.
#[derive(Default)]
struct RefusesFastForwardOnly(
    std::sync::Mutex<Vec<purlis_core::forge::proxy::listener::PushUpdate>>,
);

impl purlis_core::forge::proxy::listener::PushGate for RefusesFastForwardOnly {
    fn check(
        &self,
        _: &purlis_core::forge::proxy::ChatId,
        push: &purlis_core::forge::proxy::listener::PushRead,
    ) -> purlis_core::forge::proxy::listener::PushVerdict {
        use purlis_core::forge::proxy::listener::PushVerdict;
        self.0.lock().unwrap().extend(push.updates.iter().cloned());
        if push.updates.iter().any(|u| u.fast_forward_only) {
            PushVerdict::Refuse("this push replaces the branch's history".to_owned())
        } else {
            PushVerdict::Forward
        }
    }
}

impl RefusesFastForwardOnly {
    /// Whether the last update it was handed for `branch` was marked fast-forward only.
    fn marked(&self, branch: &str) -> Option<bool> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|u| u.name == format!("refs/heads/{branch}"))
            .map(|u| u.fast_forward_only)
    }
}

#[test]
fn after_allow_a_push_that_is_not_a_fast_forward_is_refused_and_nothing_is_sent() {
    purlis_core::unsteered!();
    // Floor item 6 of ADR 0125's 2026-11-27 amendment: no force push onto another chat's
    // branch. The core cannot tell a force push from a fast-forward (old and new ids only), so
    // every update of a branch the chat holds by Allow reaches the gate marked, and the gate's
    // refusal sends nothing.
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let mut w = World::asking_in(config.path().to_owned(), root.clone());
    let gate = std::sync::Arc::new(RefusesFastForwardOnly::default());
    w.wiring.push = gate.clone();
    let (on_forge, forced) = ("4".repeat(40), "6".repeat(40));
    refused(&w, "theirs", &on_forge, &forced).expect("an unowned branch is refused");
    windowonly::let_chat_push(
        &HostScope::for_a_test(),
        config.path(),
        &root,
        SESSION,
        &control("theirs"),
        &forge_at(&on_forge),
    )
    .unwrap();

    let reply = reply(&w, "theirs", &on_forge, &forced);

    assert_eq!(
        gate.marked("theirs"),
        Some(true),
        "handed to the gate marked"
    );
    assert!(
        reply.text().contains("replaces the branch's history"),
        "{}",
        reply.text()
    );
    assert!(w.fake.requests().is_empty(), "nothing is sent upstream");
    assert_eq!(
        tip_of(config.path(), CHAT, "theirs"),
        Some(on_forge),
        "the record does not move"
    );
    windowonly::project_ended(&root);
}

#[test]
fn after_allow_every_later_update_stays_marked_and_a_branch_the_chat_made_is_not() {
    purlis_core::unsteered!();
    // A fast-forward the gate lets through is sent and recorded, and the record keeps the Allow:
    // the next update is marked again, so a later force push is no more allowed than the first.
    let config = tempfile::tempdir().unwrap();
    let (_p, root) = project(PROJECT_ID);
    let mut w = World::asking_in(config.path().to_owned(), root.clone());
    let (on_forge, next, after) = ("4".repeat(40), "5".repeat(40), "6".repeat(40));
    refused(&w, "theirs", &on_forge, &next).expect("refused");
    windowonly::let_chat_push(
        &HostScope::for_a_test(),
        config.path(),
        &root,
        SESSION,
        &control("theirs"),
        &forge_at(&on_forge),
    )
    .unwrap();
    // The app's gate would read this one as a fast-forward and send it on.
    assert!(
        refused(&w, "theirs", &on_forge, &next).is_none(),
        "a fast-forward is sent"
    );
    assert_eq!(w.fake.requests().len(), 1);
    assert_eq!(tip_of(config.path(), CHAT, "theirs"), Some(next.clone()));

    let gate = std::sync::Arc::new(RefusesFastForwardOnly::default());
    w.wiring.push = gate.clone();
    reply(&w, "theirs", &next, &after);
    assert_eq!(gate.marked("theirs"), Some(true), "still the Allow's");
    assert_eq!(w.fake.requests().len(), 1, "not sent");

    // A branch the chat makes through the route is its own, and is never marked.
    let zero = "0".repeat(40);
    reply(&w, "fresh", &zero, &next);
    assert_eq!(gate.marked("fresh"), Some(false));
    reply(&w, "fresh", &next, &after);
    assert_eq!(
        gate.marked("fresh"),
        Some(false),
        "its own branch moves freely"
    );
    assert_eq!(w.fake.requests().len(), 3);
    windowonly::project_ended(&root);
}

// ---- the caps and Keep blocked hold whatever `consents` runs as (#7841, review of #7840) ----

/// A push of `commands`, `(old, new, branch)` each, handed to the route on this thread.
fn handled_all(w: &World, commands: &[(&str, &str, &str)]) -> purlis_core::forge::proxy::Outcome {
    use purlis_core::forge::proxy::{Inbound, RequestHead};
    let mut body = String::new();
    for (i, (old, new, branch)) in commands.iter().enumerate() {
        let caps = if i == 0 {
            "\0 report-status side-band-64k agent=git/2"
        } else {
            ""
        };
        body.push_str(&pkt(&format!("{old} {new} refs/heads/{branch}{caps}")));
    }
    body.push_str("0000PACK....");
    let head = RequestHead::new("POST", HOST, 443, "/acme/app.git/git-receive-pack")
        .with_content_type(Some("application/x-git-receive-pack-request"));
    let headers = vec![
        (
            "Authorization".to_owned(),
            basic("x-access-token", w.sentinel.expose()),
        ),
        (
            "Content-Type".to_owned(),
            "application/x-git-receive-pack-request".to_owned(),
        ),
    ];
    w.proxy.handle(
        &w.chat,
        &Inbound {
            head: &head,
            headers: &headers,
            body: body.as_bytes(),
        },
    )
}

/// What the chat is told for a push the route held with an ask's sentence.
fn said_of(outcome: purlis_core::forge::proxy::Outcome, consents: &str) -> String {
    match outcome {
        purlis_core::forge::proxy::Outcome::Held(held) => held.said,
        other => panic!("{consents}: not refused with a sentence: {other:?}"),
    }
}

#[test]
fn one_ask_a_push_and_three_waiting_hold_under_consents_off_and_log() {
    purlis_core::unsteered!();
    for consents in ["off", "log"] {
        let _here = ConsentsHere::at(consents);
        let config = tempfile::tempdir().unwrap();
        let (_p, root) = project(PROJECT_ID);
        let w = World::asking_in(config.path().to_owned(), root.clone());
        let (old, new) = ("3".repeat(40), "5".repeat(40));
        let three = [
            (old.as_str(), new.as_str(), "b1"),
            (old.as_str(), new.as_str(), "b2"),
            (old.as_str(), new.as_str(), "b3"),
        ];

        // One push of three branches raises one ask; each push after it one more, to three.
        let said = said_of(handled_all(&w, &three), consents);
        assert!(said.contains("b1 in acme/app"), "{consents}: {said}");
        assert_eq!(ask_count(&root), 1, "{consents}");
        handled_all(&w, &three);
        handled_all(&w, &three);
        assert_eq!(ask_count(&root), 3, "{consents}");
        let said = said_of(
            handled_all(&w, &[(old.as_str(), new.as_str(), "b4")]),
            consents,
        );
        assert_eq!(ask_count(&root), 3, "{consents}: past the cap no ask");
        assert!(said.contains("already waiting"), "{consents}: {said}");
        assert!(w.fake.requests().is_empty(), "{consents}: never sent");
        windowonly::project_ended(&root);
    }
}

#[test]
fn keep_blocked_holds_under_consents_off_and_log() {
    purlis_core::unsteered!();
    for consents in ["off", "log"] {
        let _here = ConsentsHere::at(consents);
        let config = tempfile::tempdir().unwrap();
        let (_p, root) = project(PROJECT_ID);
        let w = World::asking_in(config.path().to_owned(), root.clone());
        let (old, new) = ("3".repeat(40), "5".repeat(40));
        said_of(handled(&w, "theirs", &old, &new), consents);
        assert_eq!(ask_count(&root), 1, "{consents}");

        windowonly::keep_push_blocked(&root, SESSION, &control("theirs")).unwrap();

        let said = said_of(handled(&w, "theirs", &old, &new), consents);
        assert!(said.contains("kept"), "{consents}: {said}");
        assert_eq!(ask_count(&root), 0, "{consents}: no second ask");
        assert!(
            windowonly::let_chat_push(
                &HostScope::for_a_test(),
                config.path(),
                &root,
                SESSION,
                &control("theirs"),
                &forge_at(&old),
            )
            .is_err(),
            "{consents}: Allow can no longer be pressed"
        );
        assert_eq!(tip_of(config.path(), CHAT, "theirs"), None, "{consents}");
        assert!(w.fake.requests().is_empty(), "{consents}: never sent");
        windowonly::project_ended(&root);
    }
}
