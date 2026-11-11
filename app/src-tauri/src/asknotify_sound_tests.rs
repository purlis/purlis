// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The Expiring sound (#6110, ADR 0121 decision 12): with the person's setting on, an Expiring
//! ask in its last [`SOUND_BEFORE`] seconds shows its chat's notification again with the
//! system's own sound, once per ask; never with the setting off, while Quiet, or while the person
//! looks at its chat.

use std::path::Path;
use std::time::{Duration, Instant};

use super::*;
use crate::asking::{AnswerPath, AskSource, Shown};

/// Now, in seconds since 1970, as the tests read an ask's end against it.
const WALL: u32 = 1_000_000;

const NOBODY_LOOKS: &dyn Fn(u32) -> bool = &|_| false;

fn plane() -> PlaneId {
    PlaneId::for_tests(Path::new("/work/smart-ide"))
}

/// A permission ask of chat `session` that ends `left` seconds after [`WALL`].
fn expiring(session: u32, key: &str, left: u32) -> Shown {
    Shown {
        session,
        ask: key.to_owned(),
        says: "Run cargo test".to_owned(),
        options: Vec::new(),
        source: AskSource::Permission,
        chain: vec!["steward 12".to_owned(), format!("chat {session}")],
        answer: AnswerPath::InItsPane,
        since: None,
        held_until: None,
        expires: Some(WALL + left),
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    }
}

/// What `rules` hears of `asks` at `now`, then what sounds of them at `wall`.
fn read(
    rules: &mut Rules,
    asks: &[Shown],
    now: Instant,
    wall: u32,
    on: bool,
    quiet: bool,
) -> Vec<Act> {
    let plane = plane();
    let mut acts = rules.heard(&Heard {
        plane: &plane,
        asks,
        looking: NOBODY_LOOKS,
        quiet: &|| quiet,
        now,
    });
    acts.extend(rules.expiring(&Expiring {
        plane: &plane,
        asks,
        wall,
        now,
        on,
        quiet,
        looking: NOBODY_LOOKS,
    }));
    acts
}

/// The notifications `acts` show with the sound.
fn sounding(acts: &[Act]) -> Vec<Notice> {
    acts.iter()
        .filter_map(|act| match act {
            Act::Show(notice) if notice.sound => Some(notice.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_expiring_ask_sounds_only_in_its_last_seconds_with_the_setting_on_and_once() {
    let ask = expiring(3, "p1", 10);
    assert!(should_sound(&ask, WALL, true, false, false, false));
    assert!(
        should_sound(&ask, WALL + 10 - SOUND_BEFORE, true, false, false, false),
        "its last {SOUND_BEFORE} seconds include the first of them"
    );
    assert!(!should_sound(&ask, WALL, false, false, false, false), "off");
    assert!(
        !should_sound(&ask, WALL, true, false, false, true),
        "it sounded"
    );
    assert!(
        !should_sound(
            &expiring(3, "p1", SOUND_BEFORE + 1),
            WALL,
            true,
            false,
            false,
            false
        ),
        "not nearly out yet"
    );
    assert!(
        !should_sound(&ask, WALL + 10, true, false, false, false),
        "out already"
    );
    let mut lasting = ask.clone();
    lasting.expires = None;
    assert!(
        !should_sound(&lasting, WALL, true, false, false, false),
        "no countdown: not Expiring, as an ask Quiet holds open"
    );
}

#[test]
fn nothing_sounds_while_quiet_or_while_the_person_looks_at_its_chat() {
    let ask = expiring(3, "p1", 10);
    assert!(!should_sound(&ask, WALL, true, true, false, false));
    assert!(!should_sound(&ask, WALL, true, false, true, false));
}

#[test]
fn an_ask_nearly_out_shows_its_chat_s_notification_again_with_the_sound_once() {
    let mut rules = Rules::default();
    let start = Instant::now();
    let asks = [expiring(3, "p1", 10)];
    let acts = read(&mut rules, &asks, start, WALL, true, false);
    let told = sounding(&acts);
    assert_eq!(told.len(), 1, "{acts:?}");
    assert_eq!(told[0].id, 1, "the chat's own notification, shown again");
    assert!(told[0].replaces && told[0].news);
    assert_eq!(told[0].body, "smart-ide: Asks your permission");
    let again = read(&mut rules, &asks, start + TOGETHER, WALL + 2, true, false);
    assert!(sounding(&again).is_empty(), "{again:?}");
}

#[test]
fn with_the_setting_off_nothing_sounds_and_nothing_is_read_again_for_it() {
    let mut rules = Rules::default();
    let start = Instant::now();
    let soon = read(
        &mut rules,
        &[expiring(3, "p1", 10)],
        start,
        WALL,
        false,
        false,
    );
    assert!(sounding(&soon).is_empty());
    let later = read(
        &mut rules,
        &[expiring(4, "p2", 50)],
        start,
        WALL,
        false,
        false,
    );
    assert!(sounding(&later).is_empty());
    assert!(rules.next_due().is_none_or(|at| at <= start + TOGETHER));
}

#[test]
fn an_ask_heard_while_quiet_never_sounds_however_near_its_end() {
    let mut rules = Rules::default();
    let acts = read(
        &mut rules,
        &[expiring(3, "p1", 5)],
        Instant::now(),
        WALL,
        true,
        true,
    );
    assert!(acts.is_empty(), "{acts:?}");
}

#[test]
fn an_ask_with_time_left_is_read_again_when_it_comes_nearly_out_and_sounds_then() {
    let mut rules = Rules::default();
    let start = Instant::now();
    let asks = [expiring(3, "p1", 50)];
    let first = read(&mut rules, &asks, start, WALL, true, false);
    assert!(sounding(&first).is_empty());
    let at = start + Duration::from_secs(u64::from(50 - SOUND_BEFORE));
    assert_eq!(rules.next_due(), Some(at));
    assert!(rules.due(at - Duration::from_secs(1)).is_empty());
    assert_eq!(rules.due(at), vec![plane()]);
    let then = read(&mut rules, &asks, at, WALL + 50 - SOUND_BEFORE, true, false);
    assert_eq!(sounding(&then).len(), 1, "{then:?}");
    assert!(rules.due(at).is_empty(), "nothing more to sound");
}

#[test]
fn a_chat_with_two_asks_nearly_out_sounds_once() {
    let mut rules = Rules::default();
    let asks = [expiring(3, "p1", 10), expiring(3, "p2", 12)];
    let acts = read(&mut rules, &asks, Instant::now(), WALL, true, false);
    assert_eq!(sounding(&acts).len(), 1, "{acts:?}");
}

#[test]
fn a_chat_no_notification_stands_for_is_told_with_the_sound() {
    let mut rules = Rules::default();
    let start = Instant::now();
    // The first chat opens a burst; the second joins it and waits for it to close.
    read(
        &mut rules,
        &[expiring(3, "p1", 40)],
        start,
        WALL,
        true,
        false,
    );
    let asks = [expiring(3, "p1", 40), expiring(4, "p2", 8)];
    let acts = read(
        &mut rules,
        &asks,
        start + Duration::from_secs(1),
        WALL + 1,
        true,
        false,
    );
    let told = sounding(&acts);
    assert_eq!(told.len(), 1, "{acts:?}");
    assert_eq!(told[0].chats, vec![(plane(), 4)]);
    assert!(!told[0].replaces, "a new notification, its own");
}

#[test]
fn a_new_ask_after_an_answered_one_with_the_same_key_sounds_again() {
    let mut rules = Rules::default();
    let start = Instant::now();
    let asks = [expiring(3, "p1", 10)];
    assert_eq!(
        sounding(&read(&mut rules, &asks, start, WALL, true, false)).len(),
        1
    );
    read(&mut rules, &[], start + TOGETHER, WALL + 5, true, false);
    let back = read(&mut rules, &asks, start + TOGETHER * 2, WALL, true, false);
    assert_eq!(sounding(&back).len(), 1, "{back:?}");
}

#[test]
fn the_sound_is_on_only_where_the_layout_says_true() {
    let of = |document: serde_json::Value| expiring_sound_of_layout(&document);
    assert!(!of(serde_json::json!({})));
    assert!(!of(serde_json::json!({"notifications": {}})));
    assert!(!of(
        serde_json::json!({"notifications": {"expiringSound": "yes"}})
    ));
    assert!(!of(
        serde_json::json!({"notifications": {"expiringSound": false}})
    ));
    assert!(of(
        serde_json::json!({"notifications": {"expiringSound": true}})
    ));
}
