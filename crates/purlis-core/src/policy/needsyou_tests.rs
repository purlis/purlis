// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::*;
use crate::askwait::Ended;
use crate::attendance::{Begun, Fact};
use crate::harness::hooked::HOOK_TIMEOUT;
use crate::harness::model::HOOK_MARGIN;
use crate::secrets::answers::Press;

const NOW: u64 = 1_800_000_000;

fn key(request: &str) -> Key {
    Key::new(
        "guards-merge-hold",
        "acme/web#main",
        "web",
        &digest_of("merge", "acme/web#main", request),
    )
}

fn raising<'a>(request: &str, chat: &'a str, why: Why) -> Raise<'a> {
    Raise {
        key: key(request),
        action: "merge",
        said: "merge acme/web#main",
        why,
        chat,
    }
}

#[allow(clippy::disallowed_methods, reason = "a test plays the person's press")]
fn press() -> Press {
    Press::of_the_persons_press()
}

#[test]
fn an_ask_for_an_unattended_chat_is_refused_and_raised_in_needs_you() {
    // A task has nobody at it from its start (R-106), and its window is open.
    let task = Fact::at_start(Begun::Task, None, &[]);
    let reach = Reach::of(crate::attendance::of_held(Some(task)), true);
    let route = route(reach);
    assert_eq!(route, Route::Refuse(Why::Unattended));

    let mut record = Record::default();
    record.raise(raising("a", "c1", Why::Unattended), NOW, Caps::shipped());
    assert_eq!(record.items().len(), 1);
    assert_eq!(record.items()[0].why, Why::Unattended);
}

#[test]
fn an_ask_with_no_window_reachable_is_refused_and_raised_in_needs_you() {
    // The app closed, a pool, headless `purlis serve`: an attended chat, and no window.
    let person = Fact::at_start(Begun::Person, None, &[]);
    let reach = Reach::of(crate::attendance::of_held(Some(person)), false);
    assert_eq!(route(reach), Route::Refuse(Why::NoWindow));

    let mut record = Record::default();
    record.raise(raising("a", "c1", Why::NoWindow), NOW, Caps::shipped());
    assert_eq!(record.items()[0].why, Why::NoWindow);
}

#[test]
fn an_attended_chat_with_a_window_is_asked() {
    let person = Fact::at_start(Begun::Person, None, &[]);
    let reach = Reach::of(crate::attendance::of_held(Some(person)), true);
    assert_eq!(route(reach), Route::Ask);
}

#[test]
fn a_chat_the_app_holds_no_fact_for_is_refused_never_asked() {
    // Fail closed, as attendance does: no fact is no one at it.
    let reach = Reach::of(crate::attendance::of_held(None), true);
    assert_eq!(route(reach), Route::Refuse(Why::Unattended));
}

#[test]
fn a_runners_browser_login_hold_is_refused_and_raised_in_needs_you() {
    // Q314.2: the runner's login hold is this rule. A runner has nobody at it.
    let runner = Fact::at_start(Begun::Runner, None, &[]);
    let reach = Reach::of(crate::attendance::of_held(Some(runner)), true);
    let Route::Refuse(why) = route(reach) else {
        panic!("a runner's login is asked of nobody");
    };
    let mut record = Record::default();
    let mut login = raising("login to forge.example", "runner-1", why);
    login.action = "browser_login";
    record.raise(login, NOW, Caps::shipped());
    assert_eq!(record.items().len(), 1);
    assert_eq!(record.items()[0].action, "browser_login");
}

#[test]
fn an_unanswered_ask_is_refused_before_the_hook_timeout() {
    // The cap sits below the harness's hook timeout by at least the margin.
    assert!(
        cap() + HOOK_MARGIN <= HOOK_TIMEOUT,
        "the cap is {:?}",
        cap()
    );
    assert!(cap() > Duration::ZERO);

    // Nobody answers: the wait ends refused, never decided by the harness.
    let (_keep, heard) = mpsc::channel::<u8>();
    let began = Instant::now();
    let settled = wait(&heard, Duration::from_millis(30), &|| false, &|| None);
    assert_eq!(settled, Settled::Refused(Why::NoAnswer));
    assert!(began.elapsed() < Duration::from_secs(5));
}

#[test]
fn an_answer_that_comes_in_time_is_heard() {
    let (tell, heard) = mpsc::channel::<u8>();
    tell.send(7).expect("sent");
    let settled = wait(&heard, Duration::from_secs(5), &|| false, &|| None);
    assert_eq!(settled, Settled::Answered(7));
}

#[test]
fn a_wait_whose_asker_hung_up_raises_nothing() {
    assert_eq!(settle(Ended::<u8>::AskerGone), Settled::Gone);
    assert_eq!(settle(Ended::<u8>::Closed), Settled::Gone);
}

#[test]
fn a_late_answer_counts_once_within_thirty_minutes() {
    let mut record = Record::default();
    record.raise(raising("a", "c1", Why::NoAnswer), NOW, Caps::shipped());
    let allowed = record
        .allow(&press(), &key("a"), Scope::Once("c1".into()), NOW + 60)
        .expect("the item is there");
    let mut late = Late::default();
    late.hold(allowed.deferred.expect("a Once grant is deferred"));

    let digest = key("a").digest;
    // The chat asks again within the half hour: it counts, once.
    assert!(late.spend("c1", &digest, NOW + 60 + LATE_FOR_SECS));
    assert!(!late.spend("c1", &digest, NOW + 61), "spent once");

    // One that waited past the half hour counts for nothing.
    let mut record = Record::default();
    record.raise(raising("b", "c1", Why::NoAnswer), NOW, Caps::shipped());
    let allowed = record
        .allow(&press(), &key("b"), Scope::Once("c1".into()), NOW)
        .expect("the item is there");
    let mut late = Late::default();
    late.hold(allowed.deferred.expect("deferred"));
    assert!(!late.spend("c1", &key("b").digest, NOW + LATE_FOR_SECS + 1));
}

#[test]
fn a_deferred_once_grant_serves_only_its_chat_and_request() {
    let mut record = Record::default();
    record.raise(raising("a", "c1", Why::NoAnswer), NOW, Caps::shipped());
    let allowed = record
        .allow(&press(), &key("a"), Scope::Once("c1".into()), NOW)
        .expect("there");
    let mut late = Late::default();
    late.hold(allowed.deferred.expect("deferred"));

    // Another chat's same request, and the same chat's other request, are not served.
    assert!(!late.spend("c2", &key("a").digest, NOW + 1));
    assert!(!late.spend("c1", &key("other").digest, NOW + 1));
    // And neither spent it: its own chat and request still can.
    assert!(late.spend("c1", &key("a").digest, NOW + 2));
}

#[test]
fn needs_you_holds_one_item_per_rule_resource_and_workspace() {
    let mut record = Record::default();
    let caps = Caps::shipped();
    record.raise(raising("a", "c1", Why::Unattended), NOW, caps);
    record.raise(raising("a", "c2", Why::Unattended), NOW + 10, caps);
    record.raise(raising("a", "c2", Why::Unattended), NOW + 20, caps);
    assert_eq!(record.items().len(), 1, "the same key is one item");
    let item = &record.items()[0];
    assert_eq!(item.chat_count(), 2, "two chats, counted once each");
    assert_eq!(item.last, NOW + 20);
    assert_eq!(item.first, NOW);

    // Another resource, another workspace and another rule are each an item of their own.
    let mut other = raising("a", "c1", Why::Unattended);
    other.key.resource = "acme/api#main".into();
    record.raise(other, NOW, caps);
    let mut other = raising("a", "c1", Why::Unattended);
    other.key.workspace = "api".into();
    record.raise(other, NOW, caps);
    let mut other = raising("a", "c1", Why::Unattended);
    other.key.rule = "guards-git-protected-push".into();
    record.raise(other, NOW, caps);
    assert_eq!(record.items().len(), 4);
}

#[test]
fn the_same_rule_and_resource_with_another_request_is_another_item() {
    let mut record = Record::default();
    record.raise(
        raising("git push origin a", "c1", Why::Unattended),
        NOW,
        Caps::shipped(),
    );
    record.raise(
        raising("git push --force origin a", "c1", Why::Unattended),
        NOW,
        Caps::shipped(),
    );
    assert_eq!(record.items().len(), 2);
}

#[test]
fn needs_you_lets_go_of_the_oldest_past_its_caps() {
    let caps = Caps {
        max_items: 2,
        max_days: 30,
        quiet_days: 7,
    };
    let mut record = Record::default();
    record.raise(raising("a", "c1", Why::Unattended), NOW, caps);
    record.raise(raising("b", "c1", Why::Unattended), NOW + 1, caps);
    record.raise(raising("c", "c1", Why::Unattended), NOW + 2, caps);
    let kept: Vec<_> = record.items().iter().map(|one| one.key.clone()).collect();
    assert_eq!(
        kept,
        vec![key("b"), key("c")],
        "the least recent goes first"
    );

    // An item nobody raised again for the quiet days, and one older than the most days, go.
    let later = NOW + 8 * 86_400;
    record.raise(raising("d", "c1", Why::Unattended), later, caps);
    let kept: Vec<_> = record.items().iter().map(|one| one.key.clone()).collect();
    assert_eq!(kept, vec![key("d")], "b and c were quiet for eight days");
}

#[test]
fn an_item_raised_again_each_week_goes_past_the_most_days() {
    let caps = Caps::shipped();
    let mut record = Record::default();
    let mut at = NOW;
    for _ in 0..6 {
        record.raise(raising("a", "c1", Why::Unattended), at, caps);
        at += 6 * 86_400;
    }
    // 30 days after its first raise it is let go, though it was never quiet.
    record.raise(raising("z", "c1", Why::Unattended), at, caps);
    assert!(record.items().iter().all(|one| one.key != key("a")));
}

#[test]
fn an_allow_from_needs_you_grants_only_the_request_it_showed() {
    let mut record = Record::default();
    record.raise(
        raising("git push origin a", "c1", Why::Unattended),
        NOW,
        Caps::shipped(),
    );
    let shown = key("git push origin a");
    let allowed = record
        .allow(&press(), &shown, Scope::Everywhere, NOW + 5)
        .expect("there");
    let text = allowed.grant.text();
    // The digest is in the grant, so a different request is no part of it.
    assert!(text.contains(&shown.digest), "{text}");
    assert!(!text.contains(&key("git push --force origin a").digest));
    assert!(
        text.contains("guards-merge-hold")
            && text.contains("merge")
            && text.contains("acme/web#main")
    );
    assert_eq!(allowed.grant.key, shown);
}

#[test]
fn a_chat_scoped_allow_names_one_of_the_chats_that_asked() {
    let mut record = Record::default();
    record.raise(raising("a", "c1", Why::Unattended), NOW, Caps::shipped());
    // A chat that never asked is not one this item can be granted to.
    assert!(
        record
            .allow(&press(), &key("a"), Scope::Chat("c9".into()), NOW)
            .is_none()
    );
    assert_eq!(record.items().len(), 1, "a refused allow changes nothing");
    let allowed = record
        .allow(&press(), &key("a"), Scope::Chat("c1".into()), NOW)
        .expect("c1 asked");
    assert!(allowed.grant.text().contains("chat=c1"));
}

#[test]
fn a_needs_you_answer_becomes_a_grant_and_the_chat_is_told_next_call() {
    let mut record = Record::default();
    record.raise(raising("a", "c1", Why::NoAnswer), NOW, Caps::shipped());
    record.raise(raising("a", "c2", Why::NoAnswer), NOW, Caps::shipped());
    let allowed = record
        .allow(&press(), &key("a"), Scope::Workspace, NOW + 9)
        .expect("there");
    assert!(allowed.deferred.is_none(), "only a Once grant is deferred");
    assert!(
        record.items().is_empty(),
        "an answered item leaves needs-you"
    );

    // Each chat that waited is told at its next call, once, and nothing was run again.
    let told = record.take_told("c1");
    assert_eq!(told.len(), 1);
    assert_eq!(told[0].answer, Answer::Allowed(Scope::Workspace));
    assert!(told[0].said.contains("merge acme/web#main"));
    assert!(told[0].words().contains("ask again"), "{}", told[0].words());
    assert!(record.take_told("c1").is_empty(), "told once");
    assert_eq!(record.take_told("c2").len(), 1);
    assert!(record.take_told("c3").is_empty());
}

#[test]
fn a_deny_from_needs_you_tells_the_chat_and_grants_nothing() {
    let mut record = Record::default();
    record.raise(raising("a", "c1", Why::Unattended), NOW, Caps::shipped());
    let gone = record.deny(&key("a"), NOW + 1).expect("there");
    assert_eq!(gone.key, key("a"));
    assert!(record.items().is_empty());
    let told = record.take_told("c1");
    assert_eq!(told[0].answer, Answer::Denied);
}

#[test]
fn a_dismiss_removes_the_item_and_tells_nobody() {
    let mut record = Record::default();
    record.raise(raising("a", "c1", Why::Unattended), NOW, Caps::shipped());
    assert!(record.dismiss(&key("a")).is_some());
    assert!(record.items().is_empty());
    assert!(record.take_told("c1").is_empty());
    // And the same ask later is a new item.
    record.raise(
        raising("a", "c1", Why::Unattended),
        NOW + 5,
        Caps::shipped(),
    );
    assert_eq!(record.items().len(), 1);
}

#[test]
fn headless_inbox_can_deny_or_dismiss_but_never_allow() {
    // The store a headless machine opens has a deny and a dismiss and no allow: an allow is a
    // method of the record that takes the person's press, which only the window makes.
    let dir = tempfile::tempdir().expect("a folder");
    let store = Store::in_data(dir.path());
    store
        .raise(raising("a", "c1", Why::NoWindow), NOW, Caps::shipped())
        .expect("raised");
    store
        .raise(raising("b", "c1", Why::NoWindow), NOW, Caps::shipped())
        .expect("raised");

    let listed = store.read();
    assert_eq!(listed.items().len(), 2);
    let a = listed.items()[0].key.id();
    let b = listed.items()[1].key.id();

    assert!(store.deny(&a, NOW + 1).expect("written").is_some());
    assert!(store.dismiss(&b).expect("written").is_some());
    assert!(store.read().items().is_empty());
    // The refusal the chat is told survives the restart a store is.
    assert_eq!(store.read().told().len(), 1);
    assert_eq!(
        store.take_told("c1").expect("written")[0].answer,
        Answer::Denied
    );
    assert!(store.take_told("c1").expect("written").is_empty());
}

#[test]
fn an_item_is_found_by_the_start_of_its_id_only_when_one_item_has_it() {
    let mut record = Record::default();
    record.raise(raising("a", "c1", Why::Unattended), NOW, Caps::shipped());
    let id = key("a").id();
    assert_eq!(
        record.find(&id[..6]).map(|one| one.key.clone()),
        Some(key("a"))
    );
    assert_eq!(
        record.find("").map(|one| one.key.clone()),
        Some(key("a")),
        "one item: any prefix"
    );
    record.raise(raising("b", "c1", Why::Unattended), NOW, Caps::shipped());
    assert!(
        record.find("").is_none(),
        "two items share the empty prefix"
    );
    assert!(record.find("zz").is_none());
}

#[test]
fn a_file_this_build_cannot_read_reads_as_nothing_and_is_written_over() {
    let dir = tempfile::tempdir().expect("a folder");
    let store = Store::in_data(dir.path());
    std::fs::create_dir_all(dir.path().join(DIR)).expect("folder");
    std::fs::write(store.file(), b"{ not json").expect("written");
    assert!(store.read().items().is_empty());
    store
        .raise(raising("a", "c1", Why::NoWindow), NOW, Caps::shipped())
        .expect("raised");
    assert_eq!(store.read().items().len(), 1);
}

#[test]
fn the_caps_default_to_the_shipped_limits() {
    let caps = Caps::shipped();
    assert_eq!(
        (caps.max_items, caps.max_days, caps.quiet_days),
        (50, 30, 7)
    );
    let caps = Caps::from_limits(&|name| match name {
        "asks.needs-you-max-items" => Some(3),
        _ => None,
    });
    assert_eq!((caps.max_items, caps.max_days, caps.quiet_days), (3, 30, 7));
}

#[test]
fn the_refusal_says_what_happened_and_what_to_do() {
    for why in [Why::Unattended, Why::NoWindow, Why::NoAnswer] {
        let said = refusal(why, "merge acme/web#main");
        assert!(said.contains("merge acme/web#main"), "{said}");
        assert!(said.contains("needs-you"), "{said}");
        assert!(!said.contains("TODO"));
    }
    assert_ne!(
        refusal(Why::Unattended, "x"),
        refusal(Why::NoWindow, "x"),
        "each cause is said in its own words"
    );
}

#[test]
fn no_rule_offer_after_two_allows() {
    // Q314.15: the rule offer is gone, in the core and in the window. The core's record of
    // answers no longer has an `offer`, and no source file names the constant that counted to it.
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = here.join("../..");
    for file in [
        here.join("src/askrules.rs"),
        workspace.join("app/src-tauri/src/asking.rs"),
        workspace.join("app/src/AskRules.tsx"),
        workspace.join("app/src/InboxAsks.tsx"),
    ] {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        assert!(
            !text.contains("OFFER_AFTER"),
            "{} still counts to an offer",
            file.display()
        );
        assert!(!text.contains("OFFER_WINDOW_SECS"), "{}", file.display());
        assert!(!text.contains("pub fn offer("), "{}", file.display());
        assert!(
            !text.contains("rule?.offer") && !text.contains("rule.offer"),
            "{}",
            file.display()
        );
        assert!(!text.contains("RuleOffered"), "{}", file.display());
    }
}
