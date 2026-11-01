// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! When an ask raises a system notification (#1694, I-7; #1989): only asks, one for several
//! chats asking together, none while the person is looking or in their quiet hours, and what a
//! click lands on.

use std::path::Path;
use std::time::{Duration, Instant};

use super::*;
use crate::asking::{AnswerPath, AskSource, Shown, reasons_besides_failures};

fn plane(root: &str) -> PlaneId {
    PlaneId::for_tests(Path::new(root))
}

fn ask(session: u32, key: &str, says: &str, source: AskSource) -> Shown {
    Shown {
        session,
        ask: key.to_owned(),
        says: says.to_owned(),
        options: Vec::new(),
        source,
        chain: vec!["steward 12".to_owned(), format!("chat {session}")],
        answer: AnswerPath::InItsPane,
        since: None,
        held_until: None,
        writes: None,
        lines: None,
        login: None,
    }
}

fn permission(session: u32, key: &str, says: &str) -> Shown {
    ask(session, key, says, AskSource::Permission)
}

const NOBODY_LOOKS: &dyn Fn(u32) -> bool = &|_| false;
const NOT_QUIET: &dyn Fn() -> bool = &|| false;

/** An ask of chat `session`, whose session is named `name`. */
fn named(session: u32, key: &str, name: &str) -> Shown {
    let mut one = permission(session, key, "Run ls");
    one.chain = vec![name.to_owned()];
    one
}

/// What `rules` sends for `asks` of `plane`, heard at `now`, with nobody looking.
fn heard(rules: &mut Rules, plane: &PlaneId, asks: &[Shown], now: Instant) -> Vec<Notice> {
    rules.heard(&Heard {
        plane,
        asks,
        looking: NOBODY_LOOKS,
        quiet: NOT_QUIET,
        now,
    })
}

#[test]
fn a_new_ask_is_one_notification_titled_by_who_asks_and_saying_what_kind() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let sent = heard(
        &mut rules,
        &a,
        &[permission(3, "p1", "Run cargo test")],
        Instant::now(),
    );
    assert_eq!(
        sent,
        vec![Notice {
            plane: a,
            session: 3,
            sessions: vec![3],
            title: "steward 12 › chat 3".to_owned(),
            body: "Asks your permission".to_owned(),
        }]
    );
}

#[test]
fn an_ask_already_told_is_not_told_again_however_often_the_list_is_read() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let asks = [permission(3, "p1", "Run cargo test")];
    assert_eq!(heard(&mut rules, &a, &asks, start).len(), 1);
    let later = start + TOGETHER * 3;
    assert!(heard(&mut rules, &a, &asks, later).is_empty());
}

#[test]
fn asks_of_one_chat_a_few_seconds_apart_share_one_notification() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    // Two in one read: one notification, which says there is more than one.
    let two = [
        permission(3, "p1", "Run cargo test"),
        permission(3, "p2", "Change src/lib.rs"),
    ];
    let sent = heard(&mut rules, &a, &two, start);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].body, "Asks your permission, and 1 more");
    // A third two seconds on, and a fourth two seconds after that: each rides the first.
    let three = [two.to_vec(), vec![permission(3, "p3", "Run ls")]].concat();
    let soon = start + Duration::from_secs(2);
    assert!(heard(&mut rules, &a, &three, soon).is_empty());
    let four = [three.clone(), vec![permission(3, "p4", "Run pwd")]].concat();
    let sooner = soon + Duration::from_secs(2);
    assert!(heard(&mut rules, &a, &four, sooner).is_empty());
    // Once the chat has been quiet past the window, its next ask is a notification again.
    let five = [four.clone(), vec![permission(3, "p5", "Run make")]].concat();
    let sent = heard(&mut rules, &a, &five, sooner + TOGETHER);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].body, "Asks your permission");
}

#[test]
fn asks_of_several_chats_together_are_one_notification_naming_them_that_lands_on_the_first() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let sent = heard(
        &mut rules,
        &a,
        &[
            named(3, "p1", "steward 3"),
            named(4, "p2", "devops 4"),
            named(4, "p3", "devops 4"),
            named(5, "p4", "builder 5"),
        ],
        Instant::now(),
    );
    assert_eq!(
        sent,
        vec![Notice {
            plane: a,
            session: 3,
            sessions: vec![3, 4, 5],
            title: "3 chats need you".to_owned(),
            body: "steward 3, devops 4 and builder 5".to_owned(),
        }]
    );
}

#[test]
fn a_notification_names_at_most_three_chats_and_counts_the_rest() {
    let asks: Vec<Shown> = (1..=5)
        .map(|n| named(n, &format!("p{n}"), &format!("chat {n}")))
        .collect();
    let sent = heard(&mut Rules::default(), &plane("/a"), &asks, Instant::now());
    assert_eq!(sent[0].title, "5 chats need you");
    assert_eq!(sent[0].body, "chat 1, chat 2, chat 3 and 2 more");
}

#[test]
fn another_chat_asking_a_few_seconds_later_rides_the_notification_sent() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let first = [named(3, "p1", "steward 3")];
    assert_eq!(heard(&mut rules, &a, &first, start).len(), 1);
    let both = [first[0].clone(), named(4, "p2", "devops 4")];
    assert!(heard(&mut rules, &a, &both, start + Duration::from_secs(2)).is_empty());
}

#[test]
fn a_chat_asking_later_while_another_still_waits_counts_both_and_lands_on_the_longest_waiting() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let first = [named(3, "p1", "steward 3")];
    assert_eq!(heard(&mut rules, &a, &first, start).len(), 1);
    let both = [first[0].clone(), named(4, "p2", "devops 4")];
    let sent = heard(&mut rules, &a, &both, start + TOGETHER * 2);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].title, "2 chats need you");
    assert_eq!(sent[0].sessions, vec![3, 4]);
    assert_eq!(sent[0].session, 3);
}

#[test]
fn a_chat_the_person_is_looking_at_is_not_counted() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let sent = rules.heard(&Heard {
        plane: &a,
        asks: &[named(3, "p1", "steward 3"), named(4, "p2", "devops 4")],
        looking: &|session| session == 3,
        quiet: NOT_QUIET,
        now: Instant::now(),
    });
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].sessions, vec![4]);
    assert_eq!(sent[0].title, "devops 4");
}

/// What `rules` sends for `asks` of `plane` at `now`, nobody looking, in quiet hours or not.
fn heard_quiet(
    rules: &mut Rules,
    plane: &PlaneId,
    asks: &[Shown],
    now: Instant,
    quiet: bool,
) -> Vec<Notice> {
    rules.heard(&Heard {
        plane,
        asks,
        looking: NOBODY_LOOKS,
        quiet: &move || quiet,
        now,
    })
}

#[test]
fn nothing_is_sent_in_quiet_hours_and_one_names_what_still_waits_when_they_end() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let two = [named(3, "p1", "steward 3"), named(4, "p2", "devops 4")];
    assert!(heard_quiet(&mut rules, &a, &two[..1], start, true).is_empty());
    assert!(heard_quiet(&mut rules, &a, &two, start + TOGETHER * 2, true).is_empty());
    assert_eq!(rules.held_in_quiet_hours(), vec![a.clone()]);
    // Still quiet, nothing new: still nothing.
    assert!(heard_quiet(&mut rules, &a, &two, start + TOGETHER * 3, true).is_empty());
    // Quiet hours over: one notification for what still waits, and only one.
    let sent = heard_quiet(&mut rules, &a, &two, start + TOGETHER * 4, false);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].title, "2 chats need you");
    assert_eq!(sent[0].body, "steward 3 and devops 4");
    assert!(rules.held_in_quiet_hours().is_empty());
    assert!(heard_quiet(&mut rules, &a, &two, start + TOGETHER * 6, false).is_empty());
}

#[test]
fn what_was_answered_in_quiet_hours_is_not_sent_when_they_end() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let asks = [named(3, "p1", "steward 3")];
    assert!(heard_quiet(&mut rules, &a, &asks, start, true).is_empty());
    assert!(heard_quiet(&mut rules, &a, &[], start + TOGETHER * 2, false).is_empty());
    assert!(rules.held_in_quiet_hours().is_empty());
}

#[test]
fn a_chat_that_rode_a_notification_and_still_waits_is_named_when_the_window_closes() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let first = [named(3, "p1", "steward 3")];
    assert_eq!(heard(&mut rules, &a, &first, start).len(), 1);
    let both = [first[0].clone(), named(4, "p2", "devops 4")];
    let soon = start + Duration::from_secs(2);
    assert!(heard(&mut rules, &a, &both, soon).is_empty());
    // The notifier comes back when the window closes.
    assert_eq!(rules.next_due(), Some(soon + TOGETHER));
    let sent = heard(&mut rules, &a, &both, soon + TOGETHER);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].title, "2 chats need you");
    assert_eq!(sent[0].body, "steward 3 and devops 4");
    assert_eq!(sent[0].sessions, vec![3, 4]);
}

#[test]
fn a_window_closing_with_no_other_chat_on_it_sends_nothing() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let two = [
        permission(3, "p1", "Run ls"),
        permission(3, "p2", "Run pwd"),
    ];
    assert_eq!(heard(&mut rules, &a, &two[..1], start).len(), 1);
    // The same chat asking again rides, and is no reason to come back.
    assert!(heard(&mut rules, &a, &two, start + Duration::from_secs(1)).is_empty());
    assert_eq!(rules.next_due(), None);
    assert!(heard(&mut rules, &a, &two, start + TOGETHER * 2).is_empty());
}

#[test]
fn a_chat_that_rode_and_was_answered_before_the_window_closed_is_not_sent() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let first = [named(3, "p1", "steward 3")];
    assert_eq!(heard(&mut rules, &a, &first, start).len(), 1);
    let both = [first[0].clone(), named(4, "p2", "devops 4")];
    assert!(heard(&mut rules, &a, &both, start + Duration::from_secs(1)).is_empty());
    assert!(heard(&mut rules, &a, &first, start + TOGETHER * 2).is_empty());
}

/// SK-49: every bidi control is dropped from a name, as the generated table names them.
#[test]
fn every_bidi_control_is_dropped_from_a_name() {
    for c in [
        '\u{200E}', '\u{200F}', '\u{061C}', '\u{202A}', '\u{202E}', '\u{2066}', '\u{2069}',
    ] {
        assert_eq!(plain(&format!("ab{c}cd")), "abcd", "U+{:04X}", u32::from(c));
    }
    assert_eq!(
        plain("\u{1F468}\u{200D}\u{1F469}"),
        "\u{1F468}\u{200D}\u{1F469}"
    );
}

#[test]
fn a_chat_s_name_is_plain_text_and_capped() {
    let long = format!("evil\u{202e}name\n{}", "x".repeat(200));
    let one = named(3, "p1", &long);
    let sent = heard(
        &mut Rules::default(),
        &plane("/a"),
        std::slice::from_ref(&one),
        Instant::now(),
    );
    let title = &sent[0].title;
    assert!(title.starts_with("evilname x"), "{title}");
    assert!(!title.chars().any(char::is_control));
    assert!(!title.contains('\u{202e}'));
    assert!(
        title.chars().count() <= MOST_NAME,
        "{}",
        title.chars().count()
    );
    assert!(title.ends_with('…'));
    let two = [one, named(4, "p2", "devops 4")];
    let sent = heard(&mut Rules::default(), &plane("/a"), &two, Instant::now());
    assert!(sent[0].body.chars().count() <= MOST_NAME * 3 + 20);
    assert!(!sent[0].body.contains('\n'));
}

#[test]
fn quiet_hours_cover_their_span_and_one_across_midnight() {
    let at = |text: &str| minute_of(text).unwrap();
    let night = QuietHours {
        from: at("22:00"),
        to: at("07:00"),
    };
    let day = QuietHours {
        from: at("09:00"),
        to: at("17:30"),
    };
    let none = QuietHours {
        from: at("08:00"),
        to: at("08:00"),
    };
    for (hours, now, quiet) in [
        (night, "22:00", true),
        (night, "23:59", true),
        (night, "00:00", true),
        (night, "06:59", true),
        (night, "07:00", false),
        (night, "12:00", false),
        (night, "21:59", false),
        (day, "09:00", true),
        (day, "17:29", true),
        (day, "17:30", false),
        (day, "08:59", false),
        (day, "23:00", false),
        (none, "08:00", false),
        (none, "20:00", false),
    ] {
        assert_eq!(hours.covers(at(now)), quiet, "{hours:?} at {now}");
    }
}

#[test]
fn quiet_hours_are_read_from_the_layout_only_when_they_are_on_and_well_formed() {
    let read = |document: serde_json::Value| QuietHours::of_layout(&document);
    let quiet =
        |quiet: serde_json::Value| read(serde_json::json!({"notifications": {"quiet": quiet}}));
    assert_eq!(read(serde_json::json!({"version": 2, "regions": []})), None);
    assert_eq!(
        quiet(serde_json::json!({"on": true, "from": "22:00", "to": "07:00"})),
        Some(QuietHours {
            from: 22 * 60,
            to: 7 * 60
        })
    );
    assert_eq!(
        quiet(serde_json::json!({"on": false, "from": "22:00", "to": "07:00"})),
        None
    );
    assert_eq!(
        quiet(serde_json::json!({"on": true, "from": "25:00", "to": "07:00"})),
        None
    );
    assert_eq!(
        quiet(serde_json::json!({"on": true, "from": "22:00"})),
        None
    );
    assert_eq!(
        quiet(serde_json::json!({"on": "yes", "from": "22:00", "to": "07:00"})),
        None
    );
    assert_eq!(quiet(serde_json::json!("22-07")), None);
    for wrong in ["7:00", "07:60", "07:0", " 07:00", "0700", "-1:00"] {
        assert_eq!(minute_of(wrong), None, "{wrong}");
    }
}

#[test]
fn the_same_chat_number_in_two_projects_is_two_chats() {
    let mut rules = Rules::default();
    let now = Instant::now();
    assert_eq!(
        heard(
            &mut rules,
            &plane("/a"),
            &[permission(3, "p1", "Run ls")],
            now
        )
        .len(),
        1
    );
    assert_eq!(
        heard(
            &mut rules,
            &plane("/b"),
            &[permission(3, "p1", "Run ls")],
            now
        )
        .len(),
        1
    );
}

#[test]
fn nothing_is_sent_while_the_person_is_looking_and_looking_away_does_not_hold_the_next_one() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let first = [permission(3, "p1", "Run ls")];
    let sent = rules.heard(&Heard {
        plane: &a,
        asks: &first,
        looking: &|_| true,
        quiet: NOT_QUIET,
        now: start,
    });
    assert!(sent.is_empty());
    // Seen where it was asked: it is not sent later either.
    assert!(heard(&mut rules, &a, &first, start + Duration::from_secs(1)).is_empty());
    // The person looked away: the chat's next ask, a second on, is sent.
    let second = [first.to_vec(), vec![permission(3, "p2", "Run pwd")]].concat();
    let sent = heard(&mut rules, &a, &second, start + Duration::from_secs(2));
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].body, "Asks your permission");
}

#[test]
fn an_ask_answered_and_asked_again_later_is_sent_again() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let waiting = [ask(
        3,
        "question:3",
        "Waiting on your reply",
        AskSource::Question,
    )];
    assert_eq!(heard(&mut rules, &a, &waiting, start).len(), 1);
    // Replied to: it leaves the list.
    assert!(heard(&mut rules, &a, &[], start + Duration::from_secs(1)).is_empty());
    // Its next turn ends on the person, past the window.
    let again = start + Duration::from_secs(1) + TOGETHER;
    assert_eq!(heard(&mut rules, &a, &waiting, again).len(), 1);
}

#[test]
fn a_project_let_go_of_is_forgotten() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let asks = [permission(3, "p1", "Run ls")];
    let now = Instant::now();
    assert_eq!(heard(&mut rules, &a, &asks, now).len(), 1);
    rules.forget(&a);
    assert_eq!(heard(&mut rules, &a, &asks, now + TOGETHER).len(), 1);
}

#[test]
fn a_chat_without_names_is_titled_by_its_number() {
    let mut rules = Rules::default();
    let mut nameless = permission(7, "p1", "Run ls");
    nameless.chain.clear();
    let sent = heard(&mut rules, &plane("/a"), &[nameless], Instant::now());
    assert_eq!(sent[0].title, "chat 7");
}

#[test]
fn the_person_is_looking_only_from_the_window_in_front_at_the_inbox_or_the_chat() {
    let in_front = Looking {
        window_in_front: true,
        inbox_open: true,
        chat_on_screen: false,
    };
    assert!(
        in_front.is_looking(),
        "the Inbox open in the focused window"
    );
    assert!(
        Looking {
            inbox_open: false,
            chat_on_screen: true,
            ..in_front
        }
        .is_looking(),
        "the chat itself on screen"
    );
    assert!(
        !Looking {
            inbox_open: false,
            ..in_front
        }
        .is_looking(),
        "focused, but on something else"
    );
    assert!(
        !Looking {
            window_in_front: false,
            ..in_front
        }
        .is_looking(),
        "the Inbox open in a window behind another app"
    );
}

#[test]
fn the_inbox_is_open_where_its_window_said_and_until_it_says_otherwise() {
    let open = InboxOpen::default();
    let a = plane("/a");
    assert!(!open.is_open("main", &a), "a window that never said");
    open.shown("main", &a, true);
    assert!(open.is_open("main", &a));
    assert!(
        !open.is_open("main", &plane("/b")),
        "another project's Inbox"
    );
    assert!(!open.is_open("split-1", &a), "another window");
    open.shown("main", &a, false);
    assert!(!open.is_open("main", &a));
    open.shown("main", &a, true);
    open.forget_window("main");
    assert!(!open.is_open("main", &a), "a window gone");
}

#[test]
fn a_chat_that_asks_anew_is_sent_again_though_its_ask_reads_the_same() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let held = permission(3, "p1", "Run ls");
    let waiting = ask(
        3,
        "question:3",
        "Waiting on your reply",
        AskSource::Question,
    );
    let both = [held.clone(), waiting.clone()];
    assert_eq!(heard(&mut rules, &a, &both, start).len(), 1);
    // Replied to and ended on the person again between two reads: the board says so.
    rules.renew(&a, 3);
    let later = start + TOGETHER * 2;
    let sent = heard(&mut rules, &a, &both, later);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].body, "Waiting on your reply",
        "the held permission prompt is the same ask, and is not sent again"
    );
    // Another chat's renewal sends nothing about this one.
    rules.renew(&a, 4);
    assert!(heard(&mut rules, &a, &both, later + TOGETHER * 2).is_empty());
}

#[test]
fn a_notification_says_what_kind_of_ask_and_never_what_it_asks() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let now = Instant::now();
    let said = |ask: Shown| {
        heard(&mut Rules::default(), &a, &[ask], now)[0]
            .body
            .clone()
    };
    let host = ask(
        4,
        "block:4:connect:host:internal.example",
        "The sandbox refused internal.example",
        AskSource::SandboxHost,
    );
    assert_eq!(said(host), "Asks to reach a host");
    // A folder's write is a sandbox ask of its own (#1700), and says so.
    let mut folder = ask(
        4,
        "block:4:write:home:/Users/dev/secret-project",
        "The sandbox refused a write in /Users/dev/secret-project",
        AskSource::SandboxWrite,
    );
    folder.answer = AnswerPath::SandboxBlock {
        shown: crate::taskblocks::BlockShown {
            operation: "write".to_owned(),
            kind: "home".to_owned(),
            what: crate::sandboxing::GrantWhat::Write,
            target: "/Users/dev/secret-project".to_owned(),
        },
    };
    assert_eq!(said(folder), "Asks to write in a folder");
    let dispatch = ask(
        4,
        "dispatch:7",
        "Wants to hand a task to devops",
        AskSource::Dispatch,
    );
    assert_eq!(said(dispatch), "Asks to hand a task on");
    let terminal = ask(
        4,
        "terminal:4",
        "Waiting in its terminal",
        AskSource::Terminal,
    );
    assert_eq!(said(terminal), "Waiting in its terminal");
    // A command line, a host, a path stay in the window, never on a lock screen.
    let sent = heard(
        &mut rules,
        &a,
        &[permission(3, "p1", "Run deploy --to prod.internal")],
        now,
    );
    assert!(!sent[0].body.contains("prod.internal"));
}

#[test]
fn a_task_that_failed_never_makes_a_chat_s_own_wait_an_update() {
    use crate::hooks::{HowFailed, Need};
    let failed = Need::TaskFailed {
        id: "d1".to_owned(),
        chat: None,
        task: "drill".to_owned(),
        how: HowFailed::Failed,
        why: "it said it was blocked".to_owned(),
    };
    let undelivered = Need::ReportUndelivered {
        asker: "steward 12".to_owned(),
    };
    // A failure left unlooked-at beside a turn that ended on the person: still an ask.
    assert_eq!(
        reasons_besides_failures(Some(std::slice::from_ref(&failed)), 0),
        0
    );
    assert_eq!(reasons_besides_failures(None, 0), 0);
    // A report with nowhere to go, or a refused commit: the app's reasons, an update.
    assert_eq!(
        reasons_besides_failures(Some(&[failed.clone(), undelivered]), 0),
        1
    );
    assert_eq!(reasons_besides_failures(Some(&[failed]), 2), 2);
}

// A click on a notification reaches the app only as the app being brought forward (the desktop
// notification API reports no click). With every window hidden, macOS says so as a reopen,
// and the window brought back is the one holding the project the latest notification was
// about, where its landing waits for the focus.

#[test]
fn a_reopen_with_nothing_notified_brings_back_the_main_window() {
    let latest = LatestSent::default();
    assert_eq!(
        brought_back(latest.plane().as_ref(), |_| Some("split-2".to_owned())),
        crate::windows::MAIN
    );
}

#[test]
fn a_reopen_brings_back_the_window_holding_the_latest_notified_project() {
    let latest = LatestSent::default();
    latest.sent(&plane("/a"));
    latest.sent(&plane("/b"));
    let holder = |held: &PlaneId| (*held == plane("/b")).then(|| "split-2".to_owned());
    assert_eq!(brought_back(latest.plane().as_ref(), holder), "split-2");
}

#[test]
fn a_reopen_whose_notified_project_no_window_holds_brings_back_the_main_window() {
    let latest = LatestSent::default();
    latest.sent(&plane("/gone"));
    assert_eq!(
        brought_back(latest.plane().as_ref(), |_| None),
        crate::windows::MAIN
    );
}
