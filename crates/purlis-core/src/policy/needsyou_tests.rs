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

fn caps() -> Caps {
    Caps::shipped()
}

/// `raise` with its digest made again, after a test changed the action or the resource.
fn rekeyed(mut raise: Raise<'_>) -> Raise<'_> {
    raise.key.digest = digest_of(raise.action, &raise.key.resource, raise.request);
    raise
}

fn raising<'a>(request: &'a str, chat: &'a str, why: Why) -> Raise<'a> {
    Raise {
        key: key(request),
        action: "merge",
        said: "merge acme/web#main",
        request,
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
    assert!(record.raise(rekeyed(login), NOW, Caps::shipped()));
    assert_eq!(record.items().len(), 1);
    assert_eq!(record.items()[0].action, "browser_login");
}

#[test]
fn an_unanswered_ask_is_refused_before_the_hook_timeout() {
    // The cap sits below the harness's hook timeout by at least the margin.
    let at_once = cap(Duration::ZERO).expect("a fresh hook has room");
    assert!(
        at_once + HOOK_MARGIN <= HOOK_TIMEOUT,
        "the cap is {at_once:?}"
    );
    assert!(at_once > Duration::ZERO);

    // Nobody answers: the wait ends refused, never decided by the harness.
    let (_keep, heard) = mpsc::channel::<u8>();
    let began = Instant::now();
    let settled = wait(&heard, Duration::from_millis(30), &|| false, &|| None);
    assert_eq!(settled, Settled::Refused(Why::NoAnswer));
    assert!(began.elapsed() < Duration::from_secs(5));
}

#[test]
fn the_cap_is_what_the_hook_has_left_less_the_margin() {
    // A hook that ran 10 s has 50 s of its 60 left, and the refusal must land 2 s before that.
    let ran = Duration::from_secs(10);
    let left = cap(ran).expect("room");
    assert_eq!(left, HOOK_TIMEOUT - ran - HOOK_MARGIN);
    assert!(cap(Duration::from_secs(5)).expect("room") > left);
}

#[test]
fn an_ask_with_no_time_left_is_refused_at_once_and_never_left_to_the_pane() {
    // Within the margin of the timeout, and past it, there is no cap: not a wait of zero that
    // falls to the harness's own prompt, but a refusal now.
    for ran_for in [
        HOOK_TIMEOUT - HOOK_MARGIN,
        HOOK_TIMEOUT - Duration::from_millis(1),
        HOOK_TIMEOUT,
        HOOK_TIMEOUT * 3,
    ] {
        assert_eq!(cap(ran_for), None, "{ran_for:?}");
        let (_keep, heard) = mpsc::channel::<u8>();
        let began = Instant::now();
        let settled = wait_for_the_person(&heard, ran_for, &|| false, &|| None);
        assert_eq!(settled, Settled::Refused(Why::NoAnswer), "{ran_for:?}");
        assert!(began.elapsed() < Duration::from_secs(1), "{ran_for:?}");
    }
}

#[test]
fn a_wait_with_a_little_time_left_ends_at_its_deadline_refused() {
    // A real deadline: 150 ms above the margin is left, so the wait is that long, no longer.
    let ran_for = HOOK_TIMEOUT - HOOK_MARGIN - Duration::from_millis(150);
    assert_eq!(cap(ran_for), Some(Duration::from_millis(150)));
    let (_keep, heard) = mpsc::channel::<u8>();
    let began = Instant::now();
    let settled = wait_for_the_person(&heard, ran_for, &|| false, &|| None);
    assert_eq!(settled, Settled::Refused(Why::NoAnswer));
    let took = began.elapsed();
    assert!(took >= Duration::from_millis(100), "{took:?}");
    assert!(took < Duration::from_secs(5), "{took:?}");
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
    record.raise(rekeyed(other), NOW, caps);
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
    // The grant is the sealed store's own, for the rule, action and resource that were shown; the
    // digest was checked at the Allow, so what was shown is what it covers.
    let fields: serde_json::Value = serde_json::from_str(&text).expect("one JSON object");
    assert_eq!(fields["rule"], "guards-merge-hold");
    assert_eq!(fields["action"], "merge");
    assert_eq!(fields["resource"], "acme/web#main");
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
    let fields: serde_json::Value =
        serde_json::from_str(&allowed.grant.text()).expect("one JSON object");
    assert_eq!(fields["chat"], "c1");
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

    let listed = store.read(NOW, caps()).expect("read");
    assert_eq!(listed.items().len(), 2);
    let a = listed.items()[0].key.id();
    let b = listed.items()[1].key.id();

    assert!(store.deny(&a, NOW + 1, caps()).expect("written").is_some());
    assert!(
        store
            .dismiss(&b, NOW + 1, caps())
            .expect("written")
            .is_some()
    );
    assert!(store.read(NOW, caps()).expect("read").items().is_empty());
    // The refusal the chat is told survives the restart a store is.
    assert_eq!(store.read(NOW, caps()).expect("read").told().len(), 1);
    assert_eq!(
        store.take_told("c1", NOW + 1, caps()).expect("written")[0].answer,
        Answer::Denied
    );
    assert!(
        store
            .take_told("c1", NOW + 1, caps())
            .expect("written")
            .is_empty()
    );
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
    assert!(
        record.find("").is_none(),
        "the empty prefix is not an id, even of the only item"
    );
    assert!(record.find(&id[..MIN_PREFIX - 1]).is_none(), "too short");
    assert_eq!(
        record.find(&id[..MIN_PREFIX]).map(|one| one.key.clone()),
        Some(key("a"))
    );
    record.raise(raising("b", "c1", Why::Unattended), NOW, Caps::shipped());
    assert!(record.find("zzzz").is_none());
}

#[test]
fn a_file_this_build_cannot_read_reads_as_nothing_and_is_written_over() {
    let dir = tempfile::tempdir().expect("a folder");
    let store = Store::in_data(dir.path());
    std::fs::create_dir_all(dir.path().join(DIR)).expect("folder");
    std::fs::write(store.file(), b"{ not json").expect("written");
    assert!(store.read(NOW, caps()).expect("read").items().is_empty());
    store
        .raise(raising("a", "c1", Why::NoWindow), NOW, Caps::shipped())
        .expect("raised");
    assert_eq!(store.read(NOW, caps()).expect("read").items().len(), 1);
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
        let said = refusal(why, "merge acme/web#main", true);
        assert!(said.contains("merge acme/web#main"), "{said}");
        assert!(said.contains("needs-you"), "{said}");
        assert!(!said.contains("TODO"));
    }
    assert_ne!(
        refusal(Why::Unattended, "x", true),
        refusal(Why::NoWindow, "x", true),
        "each cause is said in its own words"
    );
}

#[test]
fn a_file_that_cannot_be_opened_is_not_written_over() {
    // Only a missing file is an empty record. A name that is not a file at all (here a
    // folder, as a permission or device error would be) refuses the change and leaves it.
    let dir = tempfile::tempdir().expect("a folder");
    let store = Store::in_data(dir.path());
    std::fs::create_dir_all(store.file()).expect("a folder where the file goes");
    assert!(store.read(NOW, caps()).is_err());
    let raised = store.raise(raising("a", "c1", Why::NoWindow), NOW, caps());
    assert!(raised.is_err(), "{raised:?}");
    assert!(store.file().is_dir(), "left as it was");
}

#[test]
fn an_allow_refuses_an_item_whose_digest_is_not_its_requests() {
    // A forged item: the digest is that of a harmless request and the request is another.
    let mut record = Record::default();
    record.raise(raising("a", "c1", Why::NoWindow), NOW, caps());
    let forged = record.items[0].clone();
    record.items[0].request = "git push --force origin main".into();
    assert!(
        record
            .allow(&press(), &forged.key, Scope::Everywhere, NOW)
            .is_none(),
        "what is shown is what is granted"
    );
    assert_eq!(record.items().len(), 1, "a refused allow changes nothing");
    assert!(record.take_told("c1").is_empty());
    // A file without the request at all is as forged.
    record.items[0].request = String::new();
    assert!(
        record
            .allow(&press(), &forged.key, Scope::Workspace, NOW)
            .is_none()
    );
}

#[test]
fn a_raise_whose_digest_is_not_its_requests_is_not_kept() {
    let mut record = Record::default();
    let mut forged = raising("a", "c1", Why::NoWindow);
    forged.request = "something else";
    assert!(!record.raise(forged, NOW, caps()));
    assert!(record.items().is_empty());
}

#[test]
fn one_value_cannot_run_into_the_next_in_a_digest_or_an_id() {
    assert_ne!(digest_of("a", "bc", "d"), digest_of("ab", "c", "d"));
    assert_ne!(digest_of("a", "b", "cd"), digest_of("a", "bc", "d"));
    let one = Key::new("r", "a\0b", "w", "d");
    let other = Key::new("r", "a", "b\0w", "d");
    assert_ne!(one.id(), other.id());
}

#[test]
fn a_grant_with_a_space_a_quote_or_an_equals_in_a_value_stays_one_field() {
    let mut record = Record::default();
    let resource = "acme/web branch=main\" chat=c9";
    let request = "git push";
    let digest = digest_of("merge", resource, request);
    record.raise(
        Raise {
            key: Key::new("guards-merge-hold", resource, "web", &digest),
            action: "merge",
            said: "merge",
            request,
            why: Why::NoWindow,
            chat: "c1",
        },
        NOW,
        caps(),
    );
    let key = record.items()[0].key.clone();
    let allowed = record
        .allow(&press(), &key, Scope::Chat("c1".into()), NOW)
        .expect("there");
    let fields: serde_json::Value =
        serde_json::from_str(&allowed.grant.text()).expect("one JSON object");
    assert_eq!(fields["resource"], resource);
    assert_eq!(fields["chat"], "c1", "the resource did not name a chat");
}

#[test]
fn a_chat_scoped_allow_tells_only_that_chat() {
    for scope in [Scope::Once("c1".into()), Scope::Chat("c1".into())] {
        let mut record = Record::default();
        record.raise(raising("a", "c1", Why::NoAnswer), NOW, caps());
        record.raise(raising("a", "c2", Why::NoAnswer), NOW, caps());
        record
            .allow(&press(), &key("a"), scope, NOW + 1)
            .expect("there");
        assert_eq!(record.take_told("c1").len(), 1);
        assert!(record.take_told("c2").is_empty(), "c2 was not allowed");
    }
}

#[test]
fn a_raise_past_a_bound_is_not_kept() {
    let mut record = Record::default();
    let long = "x".repeat(MOST_FIELD + 1);
    for field in 0..4 {
        let mut raise = raising("a", "c1", Why::NoWindow);
        match field {
            0 => raise.key.rule = long.clone(),
            1 => raise.key.workspace = long.clone(),
            2 => raise.chat = &long,
            _ => raise.action = &long,
        }
        assert!(!record.raise(raise, NOW, caps()), "field {field}");
    }
    let mut raise = raising("a", "c1", Why::NoWindow);
    raise.key.resource = "r".repeat(MOST_RESOURCE + 1);
    assert!(!record.raise(raise, NOW, caps()));
    let big = "y".repeat(MOST_REQUEST + 1);
    let digest = digest_of("merge", "acme/web#main", &big);
    let raise = Raise {
        key: Key::new("guards-merge-hold", "acme/web#main", "web", &digest),
        action: "merge",
        said: "merge",
        request: &big,
        why: Why::NoWindow,
        chat: "c1",
    };
    assert!(!record.raise(raise, NOW, caps()));
    assert!(record.items().is_empty());
}

#[test]
fn one_item_names_at_most_the_most_chats() {
    let mut record = Record::default();
    for n in 0..MOST_CHATS + 20 {
        record.raise(raising("a", &format!("c{n}"), Why::NoWindow), NOW, caps());
    }
    assert_eq!(record.items().len(), 1);
    assert_eq!(record.items()[0].chat_count(), MOST_CHATS);
    // A chat past the most is not named, so it is not told the item is there for it.
    assert!(!record.raise(raising("a", "late", Why::NoWindow), NOW, caps()));
    assert!(record.raise(raising("a", "c1", Why::NoWindow), NOW, caps()));
}

#[test]
fn what_an_item_says_is_marked_so_a_line_end_cannot_forge_a_line() {
    let mut record = Record::default();
    let mut raise = raising("a", "c1", Why::NoWindow);
    raise.said = "merge x\n  Allowed everything";
    record.raise(raise, NOW, caps());
    let said = &record.items()[0].said;
    assert!(!said.contains('\n'), "{said:?}");
    assert!(said.contains("U+000A"), "{said:?}");
}

#[test]
fn one_chat_cannot_push_the_others_items_out_of_needs_you() {
    let caps = Caps {
        max_items: 4,
        max_days: 30,
        quiet_days: 7,
    };
    let mut record = Record::default();
    record.raise(raising("quiet-1", "calm", Why::NoWindow), NOW, caps);
    record.raise(raising("quiet-2", "calm", Why::NoWindow), NOW + 1, caps);
    for n in 0..20 {
        record.raise(
            raising(&format!("flood-{n}"), "noisy", Why::NoWindow),
            NOW + 2 + n,
            caps,
        );
    }
    assert_eq!(record.items().len(), 4);
    let calm = record
        .items()
        .iter()
        .filter(|one| one.chats == ["calm"])
        .count();
    assert_eq!(calm, 2, "the calm chat's items stay: {:?}", record.items());
    // And the noisy chat lost its own least recent ones, not the newest.
    assert!(record.items().iter().any(|one| one.key == key("flood-19")));
}

#[test]
fn the_caps_are_held_when_the_record_is_read_and_listed() {
    let dir = tempfile::tempdir().expect("a folder");
    let store = Store::in_data(dir.path());
    store
        .raise(raising("a", "c1", Why::NoWindow), NOW, caps())
        .expect("raised");
    assert_eq!(
        store
            .read(NOW + 86_400, caps())
            .expect("read")
            .items()
            .len(),
        1
    );
    // Eight days later nobody raised it for the quiet days: a read does not list it.
    let later = NOW + 8 * 86_400;
    assert!(store.read(later, caps()).expect("read").items().is_empty());
}

#[test]
fn a_re_raise_past_the_most_days_is_not_said_to_be_in_needs_you() {
    let caps = Caps::shipped();
    let mut record = Record::default();
    assert!(record.raise(raising("a", "c1", Why::NoWindow), NOW, caps));
    // Raised again every few days it never goes quiet, and 31 days after its first raise it is
    // past the most days: the same call lets it go, so it must not claim it is there.
    let mut at = NOW;
    for _ in 0..5 {
        at += 6 * 86_400;
        assert!(record.raise(raising("a", "c1", Why::NoWindow), at, caps));
    }
    at += 86_400;
    let kept = record.raise(raising("a", "c1", Why::NoWindow), at, caps);
    assert!(!kept);
    assert!(record.items().is_empty());
    let said = refusal(Why::NoWindow, "merge", kept);
    assert!(!said.contains("It is in needs-you"), "{said}");
    assert!(refusal(Why::NoWindow, "merge", true).contains("It is in needs-you"));
}

/// Raises `n` items of a chat, each with a request of about `size` bytes, in one change.
fn flood(record: &mut Record, chat: &str, from: u64, n: u64, size: usize, caps: Caps) {
    for k in from..from + n {
        let request = format!("{k:08}{}", "z".repeat(size - 8));
        let digest = digest_of("merge", "acme/web#main", &request);
        record.raise(
            Raise {
                key: Key::new("guards-merge-hold", "acme/web#main", "web", &digest),
                action: "merge",
                said: "merge",
                request: &request,
                why: Why::NoWindow,
                chat,
            },
            NOW + k,
            caps,
        );
    }
}

#[test]
fn one_chats_flood_never_evicts_another_chats_item_from_the_byte_cap() {
    let dir = tempfile::tempdir().expect("a folder");
    let store = Store::in_data(dir.path());
    let caps = Caps {
        max_items: usize::MAX,
        max_days: 30,
        quiet_days: 7,
    };
    // A quiet chat raises one item first, so it is the globally oldest. Then a flooding chat
    // raises far more than the 4 MiB the file is read to (300 requests of 16 KiB), in one change.
    store
        .change(NOW, caps, |record| {
            flood(record, "quiet", 0, 1, 1000, caps);
            flood(record, "flood", 1, 300, MOST_REQUEST, caps);
        })
        .expect("written");
    let size = std::fs::metadata(store.file()).expect("written").len();
    assert!(size <= MOST_BYTES, "{size} bytes");
    let record = store.read(NOW + 300, caps).expect("read");
    assert!(
        record.items().iter().any(|one| owner(one) == "quiet"),
        "the quiet chat's item, the oldest of all, stayed"
    );
    let flooded = record
        .items()
        .iter()
        .filter(|one| owner(one) == "flood")
        .count();
    assert!(
        flooded > 0 && flooded < 300,
        "{flooded} of the flood stayed"
    );
    assert!(
        record.items().iter().any(|one| one.last == NOW + 300),
        "the flood's newest stayed"
    );
}

#[test]
fn an_item_past_the_most_item_bytes_is_not_kept() {
    // A control character is six bytes in JSON, so a request of them is six times its length.
    let request = "\u{1}".repeat(MOST_REQUEST);
    let digest = digest_of("merge", "acme/web#main", &request);
    let mut record = Record::default();
    let kept = record.raise(
        Raise {
            key: Key::new("guards-merge-hold", "acme/web#main", "web", &digest),
            action: "merge",
            said: "merge",
            request: &request,
            why: Why::NoWindow,
            chat: "c1",
        },
        NOW,
        caps(),
    );
    assert!(!kept);
    assert!(record.items().is_empty());
}

#[test]
fn a_limit_of_zero_or_less_still_holds_an_item() {
    let caps = Caps::from_limits(&|_| Some(0));
    assert_eq!((caps.max_items, caps.max_days, caps.quiet_days), (1, 1, 1));
    let caps = Caps::from_limits(&|_| Some(-5));
    assert_eq!((caps.max_items, caps.max_days, caps.quiet_days), (1, 1, 1));
    let mut record = Record::default();
    assert!(record.raise(raising("a", "c1", Why::NoWindow), NOW, caps));
    assert_eq!(record.items().len(), 1);
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
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|why| panic!("{} cannot be read: {why}", file.display()));
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

/// An item raised with its narrowing context, as the host raises one for a force push.
fn raised_narrowed(record: &mut Record, force: bool) {
    record.raise_narrowed(
        raising("git push origin a", "c1", Why::Unattended),
        Narrowed {
            resource_kind: "Branch".to_owned(),
            narrowing: serde_json::json!({"force": force, "via": "hook"})
                .as_object()
                .cloned()
                .expect("an object"),
        },
        NOW,
        caps(),
    );
}

/// The decision of the one ask `guards-merge-hold`, as the engine makes it.
fn asked_by_the_merge_hold() -> crate::policy::decide::Decision {
    use crate::policy::decide::{Deciding, Decision, Outcome};
    Decision {
        outcome: Outcome::Ask,
        deciding: vec![Deciding {
            id: "guards-merge-hold".to_owned(),
            level: "project".to_owned(),
            owner: "purlis default".to_owned(),
            file: "default.cedar".to_owned(),
            line: 1,
            off: false,
        }],
        family: "guards.merge".to_owned(),
        mode_family: "guards.merge".to_owned(),
        mode: crate::enforcement::Mode::Enforce,
        after_mode: Outcome::Ask,
        lifted: Vec::new(),
        asked: Vec::new(),
        lock: None,
        grant: None,
        unasked: None,
        keep: None,
        timed: None,
        deny_state: None,
    }
}

#[test]
fn a_grant_given_in_the_inbox_serves_the_same_ask_through_the_sealed_store_s_apply() {
    use crate::policy::decide::{Outcome, Request};
    use crate::policy::grants;
    use crate::policy::levels::Chat;
    use crate::secrets::answers::{self, Entry, Standing, State};

    let mut record = Record::default();
    raised_narrowed(&mut record, false);
    let allowed = record
        .allow(&press(), &key("git push origin a"), Scope::Workspace, NOW + 1)
        .expect("there");
    // The text is what the store would seal, and it reads as the store's own grant.
    let sealed = grants::Grant::parse(&allowed.grant.text()).expect("the store's own format");
    assert_eq!(sealed, allowed.grant.sealed());
    let standing = Standing {
        state: State::Holds,
        entries: vec![Entry::new(answers::GRANT, &allowed.grant.text())],
        ..Standing::fresh()
    };
    let request = |force: bool| Request {
        chat: "1",
        of: Chat {
            workspace: Some("web"),
            persona: None,
        },
        action: "merge",
        resource: ("Branch", "acme/web#main"),
        context: serde_json::json!({
            "sandbox": "floor", "attended": true, "harness_mode": "default", "depth": 0,
            "force": force, "via": "hook"
        }),
    };
    let apply = |force: bool| {
        grants::apply(
            asked_by_the_merge_hold(),
            &request(force),
            Some("01M85WM43DT07ANNAQ6NQEJTXD"),
            &standing,
            &mut |_| true,
        )
    };
    // The ask it was given for is served; the same ask with another context is not.
    assert_eq!(apply(false).after_mode, Outcome::Allow);
    assert_eq!(apply(true).after_mode, Outcome::Ask);
    // An item raised with no narrowing and no kind makes a grant that matches nothing.
    let mut bare = Record::default();
    bare.raise(raising("a", "c1", Why::Unattended), NOW, caps());
    let made = bare
        .allow(&press(), &key("a"), Scope::Everywhere, NOW)
        .expect("there");
    let none = Standing {
        state: State::Holds,
        entries: vec![Entry::new(answers::GRANT, &made.grant.text())],
        ..Standing::fresh()
    };
    let said = grants::apply(
        asked_by_the_merge_hold(),
        &request(false),
        None,
        &none,
        &mut |_| true,
    );
    assert_eq!(said.after_mode, Outcome::Ask, "{said:?}");
}
