// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! When an ask raises a system notification (#1694, I-7; #1989; #6003, ADR 0121 decision 12):
//! one per chat, updated in place and taken away when it is answered; one for a burst of three
//! chats or more, across projects; none while the person is looking or in their quiet hours;
//! and what a click lands on.

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
        expires: None,
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

fn permission(session: u32, key: &str, says: &str) -> Shown {
    ask(session, key, says, AskSource::Permission)
}

const NOBODY_LOOKS: &dyn Fn(u32) -> bool = &|_| false;
const NOBODY_LOOKS_AT_ANY: &dyn Fn(&Chat) -> bool = &|_| false;
const NOT_QUIET: &dyn Fn() -> bool = &|| false;

/// An ask of chat `session`, whose session is named `name`.
fn named(session: u32, key: &str, name: &str) -> Shown {
    let mut one = permission(session, key, "Run ls");
    one.chain = vec![name.to_owned()];
    one
}

/// What `rules` does for `asks` of `plane`, heard at `now`, with nobody looking.
fn heard(rules: &mut Rules, plane: &PlaneId, asks: &[Shown], now: Instant) -> Vec<Act> {
    rules.heard(&Heard {
        plane,
        asks,
        looking: NOBODY_LOOKS,
        quiet: NOT_QUIET,
        now,
    })
}

/// The notifications `acts` show.
fn shown(acts: &[Act]) -> Vec<Notice> {
    acts.iter()
        .filter_map(|act| match act {
            Act::Show(notice) => Some(notice.clone()),
            Act::Remove(_) => None,
        })
        .collect()
}

/// The one notification `acts` show, and nothing else.
fn only_shown(acts: &[Act]) -> Notice {
    match acts {
        [Act::Show(notice)] => notice.clone(),
        other => panic!("expected one notification shown, got {other:?}"),
    }
}

#[test]
fn a_new_ask_is_one_notification_naming_who_asks_its_project_and_what_kind() {
    let mut rules = Rules::default();
    let ide = plane("/work/smart-ide");
    let acts = heard(
        &mut rules,
        &ide,
        &[permission(3, "p1", "Run cargo test")],
        Instant::now(),
    );
    assert_eq!(
        acts,
        vec![Act::Show(Notice {
            id: 1,
            replaces: false,
            news: true,
            chats: vec![(ide, 3)],
            title: "steward 12 › chat 3".to_owned(),
            body: "smart-ide: Asks your permission".to_owned(),
            update: None,
            sound: false,
        })]
    );
}

#[test]
fn a_notification_says_a_question_s_kind_and_never_what_the_chat_said() {
    let mut question = ask(
        3,
        "question:3",
        "Waiting on your reply",
        AskSource::Question,
    );
    question.last_message = Some("Which branch should I use?".to_owned());
    let acts = heard(
        &mut Rules::default(),
        &plane("/a"),
        &[question],
        Instant::now(),
    );
    assert_eq!(only_shown(&acts).body, "a: Waiting on your reply");
}

#[test]
fn an_ask_already_told_is_not_told_again_however_often_the_list_is_read() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let asks = [permission(3, "p1", "Run cargo test")];
    assert_eq!(heard(&mut rules, &a, &asks, start).len(), 1);
    assert!(heard(&mut rules, &a, &asks, start + TOGETHER * 3).is_empty());
    assert!(
        rules
            .woke(start + TOGETHER * 4, NOBODY_LOOKS_AT_ANY)
            .is_empty()
    );
}

#[test]
fn a_chat_s_next_ask_updates_its_one_notification_in_place() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let one = [permission(3, "p1", "Run cargo test")];
    let first = only_shown(&heard(&mut rules, &a, &one, start));
    // A second ask, at once or an hour on: the same notification, said again.
    let two = [
        one[0].clone(),
        ask(3, "dispatch:9", "Hand on", AskSource::Dispatch),
    ];
    let second = only_shown(&heard(&mut rules, &a, &two, start + Duration::from_secs(1)));
    assert_eq!(second.id, first.id);
    assert!(second.replaces);
    assert!(!second.news, "it tells of no chat not told already");
    assert_eq!(second.body, "a: Asks to hand a task on, and 1 more");
    let three = [two.to_vec(), vec![permission(3, "p3", "Run ls")]].concat();
    let third = only_shown(&heard(&mut rules, &a, &three, start + TOGETHER * 720));
    assert_eq!(third.id, first.id);
    assert_eq!(third.body, "a: Asks your permission, and 2 more");
}

#[test]
fn a_chat_s_notification_counts_down_as_it_is_answered_and_goes_when_all_are() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let two = [
        permission(3, "p1", "Run cargo test"),
        permission(3, "p2", "Run ls"),
    ];
    let first = only_shown(&heard(&mut rules, &a, &two, start));
    assert_eq!(first.body, "a: Asks your permission, and 1 more");
    let fewer = only_shown(&heard(
        &mut rules,
        &a,
        &two[1..],
        start + Duration::from_secs(1),
    ));
    assert_eq!(
        (fewer.id, fewer.replaces, fewer.news),
        (first.id, true, false)
    );
    assert_eq!(fewer.body, "a: Asks your permission");
    assert_eq!(
        heard(&mut rules, &a, &[], start + Duration::from_secs(2)),
        vec![Act::Remove(first.id)]
    );
}

#[test]
fn three_chats_asking_together_are_one_notification_naming_them_that_lands_on_the_first() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let acts = heard(
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
        acts,
        vec![Act::Show(Notice {
            id: 1,
            replaces: false,
            news: true,
            chats: vec![(a.clone(), 3), (a.clone(), 4), (a, 5)],
            title: "3 chats need you".to_owned(),
            body: "a: steward 3, devops 4 and builder 5".to_owned(),
            update: None,
            sound: false,
        })]
    );
}

#[test]
fn two_chats_asking_together_are_each_told_with_their_own() {
    let acts = heard(
        &mut Rules::default(),
        &plane("/a"),
        &[named(3, "p1", "steward 3"), named(4, "p2", "devops 4")],
        Instant::now(),
    );
    let told = shown(&acts);
    assert_eq!(told.len(), 2);
    assert_eq!(told[0].title, "steward 3");
    assert_eq!(told[1].title, "devops 4");
    assert_ne!(told[0].id, told[1].id);
}

#[test]
fn a_notification_names_at_most_three_chats_and_counts_the_rest() {
    let asks: Vec<Shown> = (1..=5)
        .map(|n| named(n, &format!("p{n}"), &format!("chat {n}")))
        .collect();
    let acts = heard(&mut Rules::default(), &plane("/a"), &asks, Instant::now());
    let told = only_shown(&acts);
    assert_eq!(told.title, "5 chats need you");
    assert_eq!(told.body, "a: chat 1, chat 2, chat 3 and 2 more");
}

#[test]
fn a_burst_across_projects_tells_its_first_chat_at_once_and_all_of_it_as_one_when_it_closes() {
    let mut rules = Rules::default();
    let (ide, web) = (plane("/work/smart-ide"), plane("/work/site"));
    let start = Instant::now();
    let first = only_shown(&heard(
        &mut rules,
        &ide,
        &[named(3, "p1", "steward 3")],
        start,
    ));
    // Two more chats, each within the window of the one before, in another project.
    let soon = start + Duration::from_secs(2);
    assert!(heard(&mut rules, &web, &[named(4, "p1", "devops 4")], soon).is_empty());
    let later = soon + Duration::from_secs(4);
    let both = [named(4, "p1", "devops 4"), named(5, "p2", "builder 5")];
    assert!(heard(&mut rules, &web, &both, later).is_empty());
    // Nothing until the burst closes, and then one for all, which takes over from the first's.
    assert_eq!(rules.next_due(), Some(later + TOGETHER));
    assert!(
        rules
            .woke(
                later + TOGETHER - Duration::from_millis(1),
                NOBODY_LOOKS_AT_ANY,
            )
            .is_empty()
    );
    let acts = rules.woke(later + TOGETHER, NOBODY_LOOKS_AT_ANY);
    assert_eq!(acts[0], Act::Remove(first.id));
    let all = only_shown(&acts[1..]);
    assert_eq!(all.title, "3 chats in 2 projects need you");
    assert_eq!(
        all.body,
        "steward 3 (smart-ide), devops 4 (site) and builder 5 (site)"
    );
    assert_eq!(all.chats, vec![(ide, 3), (web.clone(), 4), (web, 5)]);
    assert!(all.news);
    assert_eq!(rules.next_due(), None);
}

#[test]
fn a_second_chat_in_a_burst_is_told_with_its_own_when_it_closes() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let first = [named(3, "p1", "steward 3")];
    assert_eq!(heard(&mut rules, &a, &first, start).len(), 1);
    let both = [first[0].clone(), named(4, "p2", "devops 4")];
    let soon = start + Duration::from_secs(2);
    assert!(heard(&mut rules, &a, &both, soon).is_empty());
    let told = only_shown(&rules.woke(soon + TOGETHER, NOBODY_LOOKS_AT_ANY));
    assert_eq!(told.title, "devops 4");
    assert_eq!(told.chats, vec![(a, 4)]);
}

#[test]
fn a_chat_answered_or_looked_at_before_its_burst_closed_is_not_told() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let first = [named(3, "p1", "steward 3")];
    assert_eq!(heard(&mut rules, &a, &first, start).len(), 1);
    let soon = start + Duration::from_secs(1);
    let three = [
        first[0].clone(),
        named(4, "p2", "devops 4"),
        named(5, "p3", "builder 5"),
    ];
    assert!(heard(&mut rules, &a, &three, soon).is_empty());
    // Chat 4 answered, chat 5 on screen: nothing more is told.
    let left = [first[0].clone(), named(5, "p3", "builder 5")];
    assert!(heard(&mut rules, &a, &left, soon + Duration::from_secs(1)).is_empty());
    let five_on_screen: &dyn Fn(&Chat) -> bool = &|chat| chat.1 == 5;
    assert!(rules.woke(soon + TOGETHER * 2, five_on_screen).is_empty());
}

#[test]
fn a_burst_s_notification_counts_down_as_its_chats_are_answered_and_goes_when_all_are() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let three = [
        named(3, "p1", "steward 3"),
        named(4, "p2", "devops 4"),
        named(5, "p3", "builder 5"),
    ];
    let all = only_shown(&heard(&mut rules, &a, &three, start));
    let fewer = only_shown(&heard(&mut rules, &a, &three[1..], start + TOGETHER * 2));
    assert_eq!(
        (fewer.id, fewer.replaces, fewer.news),
        (all.id, true, false)
    );
    assert_eq!(fewer.title, "2 chats need you");
    assert_eq!(
        heard(&mut rules, &a, &[], start + TOGETHER * 3),
        vec![Act::Remove(all.id)]
    );
}

#[test]
fn a_chat_the_person_is_looking_at_is_not_told() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let acts = rules.heard(&Heard {
        plane: &a,
        asks: &[named(3, "p1", "steward 3"), named(4, "p2", "devops 4")],
        looking: &|session| session == 3,
        quiet: NOT_QUIET,
        now: Instant::now(),
    });
    let told = only_shown(&acts);
    assert_eq!(told.chats, vec![(a, 4)]);
    assert_eq!(told.title, "devops 4");
}

/// What `rules` does for `asks` of `plane` at `now`, nobody looking, in quiet hours or not.
fn heard_quiet(
    rules: &mut Rules,
    plane: &PlaneId,
    asks: &[Shown],
    now: Instant,
    quiet: bool,
) -> Vec<Act> {
    rules.heard(&Heard {
        plane,
        asks,
        looking: NOBODY_LOOKS,
        quiet: &move || quiet,
        now,
    })
}

/// Quiet moving to `quiet` at `at` seconds since 1970, nobody looking, the person `away` or at
/// the window: what it does to the notifications, and the summary when it ends.
fn quiet_at(rules: &mut Rules, quiet: bool, at: u32, away: bool) -> (Vec<Act>, Option<QuietEnded>) {
    rules.quiet(quiet, at, away, NOBODY_LOOKS_AT_ANY)
}

#[test]
fn nothing_is_told_while_quiet_and_one_notification_names_what_still_waits_if_the_person_is_away() {
    let mut rules = Rules::default();
    let (a, b) = (plane("/a"), plane("/b"));
    let start = Instant::now();
    assert!(quiet_at(&mut rules, true, 1_000, true).0.is_empty());
    let two = [named(3, "p1", "steward 3"), named(4, "p2", "devops 4")];
    assert!(heard_quiet(&mut rules, &a, &two[..1], start, true).is_empty());
    assert!(heard_quiet(&mut rules, &a, &two, start + TOGETHER * 2, true).is_empty());
    let other = [named(1, "p1", "builder 1")];
    assert!(heard_quiet(&mut rules, &b, &other, start + TOGETHER * 3, true).is_empty());
    assert!(rules.holds_in_quiet_hours());
    // Still quiet: still nothing, however often it is looked at.
    assert!(quiet_at(&mut rules, true, 1_060, true).0.is_empty());
    assert!(
        rules
            .woke(start + TOGETHER * 4, NOBODY_LOOKS_AT_ANY)
            .is_empty()
    );
    // Quiet over, the person away: one notification for what still waits, and only one.
    let (acts, _) = quiet_at(&mut rules, false, 1_120, true);
    let all = only_shown(&acts);
    assert_eq!(all.title, "While you were quiet: 3 chats need you");
    assert_eq!(all.body, "steward 3 (a), devops 4 (a) and builder 1 (b)");
    assert!(!rules.holds_in_quiet_hours());
    assert!(quiet_at(&mut rules, false, 1_180, true).0.is_empty());
    assert!(heard_quiet(&mut rules, &a, &two, start + TOGETHER * 7, false).is_empty());
}

#[test]
fn quiet_ending_with_one_chat_waiting_is_still_one_notification_saying_it_was_quiet() {
    let mut rules = Rules::default();
    let a = plane("/smart-ide");
    quiet_at(&mut rules, true, 1_000, true);
    heard_quiet(
        &mut rules,
        &a,
        &[named(3, "p1", "steward 3")],
        Instant::now(),
        true,
    );
    let (acts, _) = quiet_at(&mut rules, false, 1_100, true);
    let one = only_shown(&acts);
    assert_eq!(one.title, "While you were quiet: 1 chat needs you");
    assert_eq!(one.body, "smart-ide: steward 3");
    assert_eq!(one.chats, vec![(a, 3)]);
}

#[test]
fn quiet_ending_with_the_person_at_the_window_notifies_nothing_and_leaves_the_summary_to_asks() {
    let mut rules = Rules::default();
    let a = plane("/a");
    quiet_at(&mut rules, true, 1_000, false);
    heard_quiet(
        &mut rules,
        &a,
        &[named(3, "p1", "steward 3")],
        Instant::now(),
        true,
    );
    let (acts, ended) = quiet_at(&mut rules, false, 1_100, false);
    assert!(acts.is_empty());
    assert!(!rules.holds_in_quiet_hours());
    let ended = ended.expect("a summary");
    assert_eq!(ended.asks.len(), 1);
}

#[test]
fn what_was_answered_while_quiet_is_neither_told_nor_summed_up_when_it_ends() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    quiet_at(&mut rules, true, 1_000, true);
    let asks = [named(3, "p1", "steward 3")];
    assert!(heard_quiet(&mut rules, &a, &asks, start, true).is_empty());
    assert!(heard_quiet(&mut rules, &a, &[], start + TOGETHER * 2, true).is_empty());
    let (acts, ended) = quiet_at(&mut rules, false, 1_100, true);
    assert!(acts.is_empty());
    assert!(!rules.holds_in_quiet_hours());
    assert_eq!(ended.expect("a summary").asks, Vec::<EndedAsk>::new());
}

#[test]
fn quiet_ending_says_when_it_was_and_every_ask_that_came_in_it_still_waiting() {
    let mut rules = Rules::default();
    let (a, b) = (plane("/a"), plane("/b"));
    let start = Instant::now();
    // Asked before Quiet: not the summary's.
    heard(&mut rules, &a, &[permission(2, "before", "Run ls")], start);
    assert_eq!(quiet_at(&mut rules, true, 1_000, false).1, None);
    let in_quiet = [
        permission(2, "before", "Run ls"),
        permission(3, "p1", "Run ls"),
        permission(3, "p2", "Run cat"),
    ];
    heard_quiet(&mut rules, &a, &in_quiet, start + TOGETHER, true);
    heard_quiet(
        &mut rules,
        &b,
        &[permission(1, "p9", "Run ls")],
        start + TOGETHER,
        true,
    );
    let (_, ended) = quiet_at(&mut rules, false, 1_200, false);
    assert_eq!(
        ended,
        Some(QuietEnded {
            from: 1_000,
            to: 1_200,
            asks: vec![
                EndedAsk {
                    plane: a.clone(),
                    key: "p1".to_owned()
                },
                EndedAsk {
                    plane: a.clone(),
                    key: "p2".to_owned()
                },
                EndedAsk {
                    plane: b,
                    key: "p9".to_owned()
                },
            ],
        })
    );
    // The next Quiet starts with nothing of this one.
    quiet_at(&mut rules, true, 2_000, false);
    let (_, next) = quiet_at(&mut rules, false, 2_100, false);
    assert_eq!(next.expect("a summary").asks, Vec::<EndedAsk>::new());
}

#[test]
fn an_ask_the_person_was_looking_at_while_quiet_is_not_in_its_summary() {
    let mut rules = Rules::default();
    let a = plane("/a");
    quiet_at(&mut rules, true, 1_000, false);
    rules.heard(&Heard {
        plane: &a,
        asks: &[named(3, "p1", "steward 3"), named(4, "p2", "devops 4")],
        looking: &|session| session == 3,
        quiet: &|| true,
        now: Instant::now(),
    });
    let (_, ended) = quiet_at(&mut rules, false, 1_100, false);
    let keys: Vec<String> = ended
        .expect("a summary")
        .asks
        .into_iter()
        .map(|one| one.key)
        .collect();
    assert_eq!(keys, vec!["p2".to_owned()]);
}

#[test]
fn quiet_holds_on_its_schedule_by_quiet_now_and_until_a_while_after_the_last_key() {
    let start = Instant::now();
    let mut sources = QuietSources::default();
    assert!(!sources.is_quiet(false, start));
    assert!(sources.is_quiet(true, start), "the schedule alone");
    sources.set_now(true);
    assert!(sources.is_quiet(false, start), "Quiet now alone");
    sources.set_now(false);
    assert!(sources.typed(start), "the first key starts it");
    assert!(
        !sources.typed(start + TYPING_QUIET / 2),
        "a key while typing does not"
    );
    assert!(sources.is_quiet(false, start + TYPING_QUIET));
    assert_eq!(
        sources.typing_ends(start),
        Some(start + TYPING_QUIET * 3 / 2)
    );
    assert!(!sources.is_quiet(false, start + TYPING_QUIET * 3 / 2));
    assert_eq!(sources.typing_ends(start + TYPING_QUIET * 2), None);
    assert!(
        sources.typed(start + TYPING_QUIET * 2),
        "a key after it ended starts it again"
    );
}

#[test]
fn typing_quiet_lasts_about_thirty_seconds() {
    assert_eq!(TYPING_QUIET, Duration::from_secs(30));
}

#[test]
fn the_terminal_answering_by_itself_is_not_the_person_typing() {
    assert!(is_typing(b"l"));
    assert!(is_typing(b"\r"));
    // A cursor position report and a focus report are the terminal's own.
    assert!(!is_typing(b"\x1b[12;5R"));
    assert!(!is_typing(b"\x1b[I"));
}

#[test]
fn the_same_chat_number_in_two_projects_is_two_chats() {
    let mut rules = Rules::default();
    let now = Instant::now();
    let asks = [permission(3, "p1", "Run ls")];
    let a = only_shown(&heard(&mut rules, &plane("/a"), &asks, now));
    // The second is a burst's, told when it closes.
    assert!(heard(&mut rules, &plane("/b"), &asks, now).is_empty());
    let b = only_shown(&rules.woke(now + TOGETHER, NOBODY_LOOKS_AT_ANY));
    assert_ne!(a.id, b.id);
    assert_eq!(b.chats, vec![(plane("/b"), 3)]);
}

#[test]
fn nothing_is_told_while_the_person_is_looking_and_looking_away_does_not_hold_the_next_one() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let first = [permission(3, "p1", "Run ls")];
    let acts = rules.heard(&Heard {
        plane: &a,
        asks: &first,
        looking: &|_| true,
        quiet: NOT_QUIET,
        now: start,
    });
    assert!(acts.is_empty());
    // Seen where it was asked: it is not told later either.
    assert!(heard(&mut rules, &a, &first, start + Duration::from_secs(1)).is_empty());
    // The person looked away: the chat's next ask, a second on, is told.
    let second = [first.to_vec(), vec![permission(3, "p2", "Run pwd")]].concat();
    let told = only_shown(&heard(
        &mut rules,
        &a,
        &second,
        start + Duration::from_secs(2),
    ));
    assert_eq!(told.body, "a: Asks your permission, and 1 more");
}

#[test]
fn an_ask_answered_and_asked_again_later_is_told_again_in_a_notification_of_its_own() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let waiting = [ask(
        3,
        "question:3",
        "Waiting on your reply",
        AskSource::Question,
    )];
    let first = only_shown(&heard(&mut rules, &a, &waiting, start));
    // Replied to: it leaves the list, and its notification goes.
    assert_eq!(
        heard(&mut rules, &a, &[], start + Duration::from_secs(1)),
        vec![Act::Remove(first.id)]
    );
    // Its next turn ends on the person, past the window.
    let again = only_shown(&heard(&mut rules, &a, &waiting, start + TOGETHER * 2));
    assert_ne!(again.id, first.id);
    assert!(!again.replaces);
}

#[test]
fn a_project_let_go_of_is_forgotten_and_its_notifications_go() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let asks = [permission(3, "p1", "Run ls")];
    let now = Instant::now();
    let told = only_shown(&heard(&mut rules, &a, &asks, now));
    assert_eq!(rules.forget(&a), vec![Act::Remove(told.id)]);
    assert_eq!(heard(&mut rules, &a, &asks, now + TOGETHER).len(), 1);
}

#[test]
fn a_chat_without_names_is_titled_by_its_number() {
    let mut nameless = permission(7, "p1", "Run ls");
    nameless.chain.clear();
    let acts = heard(
        &mut Rules::default(),
        &plane("/a"),
        &[nameless],
        Instant::now(),
    );
    assert_eq!(only_shown(&acts).title, "chat 7");
}

#[test]
fn a_chat_that_asks_anew_is_told_again_though_its_ask_reads_the_same() {
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
    let first = only_shown(&heard(&mut rules, &a, &both, start));
    // Replied to and ended on the person again between two reads: the board says so.
    rules.renew(&a, 3);
    let later = start + TOGETHER * 2;
    let told = only_shown(&heard(&mut rules, &a, &both, later));
    assert_eq!(told.id, first.id, "in place");
    assert_eq!(
        told.body, "a: Waiting on your reply, and 1 more",
        "the reply it waits on anew first; the held permission prompt is the same ask"
    );
    // Another chat's renewal says nothing about this one.
    rules.renew(&a, 4);
    assert!(heard(&mut rules, &a, &both, later + TOGETHER * 2).is_empty());
}

#[test]
fn a_notification_says_what_kind_of_ask_and_never_what_it_asks() {
    let a = plane("/a");
    let now = Instant::now();
    let said = |ask: Shown| only_shown(&heard(&mut Rules::default(), &a, &[ask], now)).body;
    let host = ask(
        4,
        "block:4:connect:host:internal.example",
        "The sandbox refused internal.example",
        AskSource::SandboxHost,
    );
    assert_eq!(said(host), "a: Asks to reach a host");
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
    assert_eq!(said(folder), "a: Asks to write in a folder");
    let dispatch = ask(
        4,
        "dispatch:7",
        "Wants to hand a task to devops",
        AskSource::Dispatch,
    );
    assert_eq!(said(dispatch), "a: Asks to hand a task on");
    let terminal = ask(
        4,
        "terminal:4",
        "Waiting in its terminal",
        AskSource::Terminal,
    );
    assert_eq!(said(terminal), "a: Waiting in its terminal");
    // A command line, a host, a path stay in the window, never on a lock screen.
    let told = said(permission(3, "p1", "Run deploy --to prod.internal"));
    assert!(!told.contains("prod.internal"));
}

#[test]
fn where_a_notification_cannot_be_replaced_an_update_telling_of_no_new_chat_is_not_shown() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    let one = [permission(3, "p1", "Run ls")];
    let first = only_shown(&heard(&mut rules, &a, &one, start));
    let two = [one[0].clone(), permission(3, "p2", "Run pwd")];
    let update = only_shown(&heard(&mut rules, &a, &two, start + Duration::from_secs(1)));
    // Linux replaces it; macOS and Windows keep the one shown, which still lands on the chat.
    assert!(shown_here(&first, false) && shown_here(&first, true));
    assert!(shown_here(&update, true));
    assert!(!shown_here(&update, false));
}

#[test]
fn a_burst_s_notification_growing_to_new_chats_is_shown_where_it_cannot_be_replaced() {
    let mut rules = Rules::default();
    let (a, b) = (plane("/a"), plane("/b"));
    let start = Instant::now();
    let three = [
        named(3, "p1", "steward 3"),
        named(4, "p2", "devops 4"),
        named(5, "p3", "builder 5"),
    ];
    let all = only_shown(&heard(&mut rules, &a, &three, start));
    let more = [
        named(1, "p1", "chat 1"),
        named(2, "p2", "chat 2"),
        named(6, "p3", "chat 6"),
    ];
    let soon = start + Duration::from_secs(1);
    assert!(heard(&mut rules, &b, &more, soon).is_empty());
    let grown = only_shown(&rules.woke(soon + TOGETHER, NOBODY_LOOKS_AT_ANY));
    assert_eq!((grown.id, grown.replaces, grown.news), (all.id, true, true));
    assert_eq!(grown.title, "6 chats in 2 projects need you");
    assert!(shown_here(&grown, false));
}

#[test]
fn a_click_lands_in_each_project_a_notification_counts_on_its_first_chat_there() {
    let (a, b) = (plane("/a"), plane("/b"));
    let notice = Notice {
        id: 1,
        replaces: false,
        news: true,
        chats: vec![(b.clone(), 4), (a.clone(), 3), (b.clone(), 2)],
        title: String::new(),
        body: String::new(),
        update: None,
        sound: false,
    };
    let landed: Vec<(PlaneId, u32, Vec<u32>)> = landings(&notice)
        .into_iter()
        .map(|landing| (landing.plane, landing.session, landing.sessions))
        .collect();
    assert_eq!(landed, vec![(b, 4, vec![4, 2]), (a, 3, vec![3])]);
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
    let title = &only_shown(&sent).title;
    assert!(title.starts_with("evilname x"), "{title}");
    assert!(!title.chars().any(char::is_control));
    assert!(!title.contains('\u{202e}'));
    assert!(
        title.chars().count() <= MOST_NAME,
        "{}",
        title.chars().count()
    );
    assert!(title.ends_with('…'));
    let three = [one, named(4, "p2", "devops 4"), named(5, "p3", "builder 5")];
    let sent = heard(&mut Rules::default(), &plane("/a"), &three, Instant::now());
    let body = only_shown(&sent).body;
    assert!(body.chars().count() <= MOST_NAME * 3 + 20, "{body}");
    assert!(!body.contains('\n'));
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

// **Should know** (#6005, ADR 0121 decision 3): a task of the person's own that fails notifies,
// and so does one that finishes after a long while with the person away; nothing else does.

/// A task that ended: dispatch `id`, asked for from chat `session`, which worked `minutes`.
fn finish(id: &str, session: u32, failed: bool, minutes: u64) -> Finish {
    Finish {
        id: id.to_owned(),
        session,
        name: format!("task {id}"),
        failed,
        by_person: true,
        worked: Duration::from_secs(minutes * 60),
        report: "cargo test failed: 3 tests\nthread 'main' panicked at src/lib.rs".to_owned(),
    }
}

/// What `rules` does for the tasks of `plane` that stand ended now, nobody looking, with the
/// person `away` or not and five minutes as a long finish.
fn ended(rules: &mut Rules, plane: &PlaneId, finishes: &[Finish], away: bool) -> Vec<Act> {
    rules.ended(&Ended {
        plane,
        finishes,
        long: Duration::from_secs(5 * 60),
        away,
        looking: NOBODY_LOOKS,
        quiet: false,
        sources: Sources::default(),
    })
}

#[test]
fn a_task_of_the_person_s_that_fails_notifies_naming_it_its_project_and_its_first_line() {
    let mut rules = Rules::default();
    let ide = plane("/work/smart-ide");
    assert!(ended(&mut rules, &ide, &[], false).is_empty());
    let failed = finish("01F", 3, true, 1);
    let notice = only_shown(&ended(&mut rules, &ide, &[failed], false));
    assert_eq!(notice.title, "task 01F");
    assert_eq!(notice.body, "smart-ide: Failed: cargo test failed: 3 tests");
    assert_eq!(notice.chats, vec![(ide, 3)]);
    assert_eq!(notice.update.as_deref(), Some("task:01F"));
}

#[test]
fn a_task_that_finishes_after_more_than_five_minutes_while_the_person_is_away_notifies() {
    let mut rules = Rules::default();
    let ide = plane("/work/smart-ide");
    assert!(ended(&mut rules, &ide, &[], true).is_empty());
    let long = finish("01L", 4, false, 12);
    let notice = only_shown(&ended(&mut rules, &ide, std::slice::from_ref(&long), true));
    assert_eq!(notice.title, "task 01L");
    assert_eq!(notice.body, "smart-ide: Finished after 12 minutes");
    assert_eq!(notice.chats, vec![(ide.clone(), 4)]);
    assert_eq!(notice.update.as_deref(), Some("task:01L"));
    // Read again, it is not told again.
    assert!(ended(&mut rules, &ide, &[long], true).is_empty());
}

#[test]
fn a_finish_within_five_minutes_or_with_the_window_in_front_is_silent() {
    let mut rules = Rules::default();
    let ide = plane("/a");
    assert!(ended(&mut rules, &ide, &[], true).is_empty());
    assert!(ended(&mut rules, &ide, &[finish("short", 1, false, 5)], true).is_empty());
    assert!(ended(&mut rules, &ide, &[finish("seen", 1, false, 30)], false).is_empty());
}

#[test]
fn a_task_a_chat_dispatched_reports_to_that_chat_and_notifies_nobody_even_when_it_fails() {
    let mut rules = Rules::default();
    let ide = plane("/a");
    assert!(ended(&mut rules, &ide, &[], true).is_empty());
    let mut failed = finish("chat-asked", 2, true, 30);
    failed.by_person = false;
    let mut long = finish("chat-asked-long", 2, false, 30);
    long.by_person = false;
    assert!(ended(&mut rules, &ide, &[failed, long], true).is_empty());
}

#[test]
fn what_had_ended_before_a_project_was_first_read_is_never_told() {
    let mut rules = Rules::default();
    let ide = plane("/a");
    let before = [
        finish("old-fail", 1, true, 1),
        finish("old-long", 1, false, 40),
    ];
    assert!(ended(&mut rules, &ide, &before, true).is_empty());
    assert!(ended(&mut rules, &ide, &before, true).is_empty());
}

#[test]
fn a_failure_in_the_chat_the_person_is_looking_at_is_not_told() {
    let mut rules = Rules::default();
    let ide = plane("/a");
    let looking: &dyn Fn(u32) -> bool = &|session| session == 3;
    let mut read = |finishes: &[Finish]| {
        rules.ended(&Ended {
            plane: &ide,
            finishes,
            long: FINISHED_AFTER,
            away: false,
            looking,
            quiet: false,
            sources: Sources::default(),
        })
    };
    assert!(read(&[]).is_empty());
    assert!(read(&[finish("here", 3, true, 1)]).is_empty());
    assert_eq!(shown(&read(&[finish("there", 4, true, 1)])).len(), 1);
}

/// [`ended`], while Quiet.
fn ended_quiet(rules: &mut Rules, plane: &PlaneId, finishes: &[Finish]) -> Vec<Act> {
    rules.ended(&Ended {
        plane,
        finishes,
        long: FINISHED_AFTER,
        away: true,
        looking: NOBODY_LOOKS,
        quiet: true,
        sources: Sources::default(),
    })
}

#[test]
fn while_quiet_finishes_and_failures_are_held_and_counted_in_the_one_notification_after() {
    let mut rules = Rules::default();
    let a = plane("/a");
    assert!(ended(&mut rules, &a, &[], true).is_empty());
    assert!(quiet_at(&mut rules, true, 1_000, true).0.is_empty());
    let three = [
        finish("f1", 3, false, 20),
        finish("f2", 4, false, 9),
        finish("x1", 5, true, 1),
    ];
    assert!(ended_quiet(&mut rules, &a, &three).is_empty());
    let (acts, _) = quiet_at(&mut rules, false, 1_120, true);
    let after = only_shown(&acts);
    assert_eq!(after.title, "While you were quiet: 2 finished, 1 failed");
    assert_eq!(after.body, "a: task f1, task f2 and task x1");
    assert_eq!(after.update.as_deref(), Some("task:f1"));
    // Told once.
    assert!(quiet_at(&mut rules, false, 1_180, true).0.is_empty());
    assert!(ended(&mut rules, &a, &three, true).is_empty());
}

#[test]
fn quiet_ending_with_asks_and_finishes_held_is_still_one_notification_counting_both() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let start = Instant::now();
    assert!(ended(&mut rules, &a, &[], true).is_empty());
    assert!(quiet_at(&mut rules, true, 1_000, true).0.is_empty());
    assert!(heard_quiet(&mut rules, &a, &[named(3, "p1", "steward 3")], start, true).is_empty());
    assert!(ended_quiet(&mut rules, &a, &[finish("x1", 5, true, 1)]).is_empty());
    let (acts, _) = quiet_at(&mut rules, false, 1_120, true);
    let after = only_shown(&acts);
    assert_eq!(
        after.title,
        "While you were quiet: 1 chat needs you, 1 failed"
    );
}

#[test]
fn quiet_ending_with_the_person_at_the_window_tells_no_finish_and_leaves_them_to_the_summary() {
    let mut rules = Rules::default();
    let a = plane("/a");
    assert!(ended(&mut rules, &a, &[], true).is_empty());
    assert!(quiet_at(&mut rules, true, 1_000, true).0.is_empty());
    assert!(ended_quiet(&mut rules, &a, &[finish("x1", 5, true, 1)]).is_empty());
    assert!(quiet_at(&mut rules, false, 1_120, false).0.is_empty());
}

#[test]
fn a_long_finish_is_five_minutes_unless_the_layout_says_another_whole_number_of_them() {
    let of = |document: serde_json::Value| finished_after_of_layout(&document);
    assert_eq!(of(serde_json::json!({})), FINISHED_AFTER);
    assert_eq!(FINISHED_AFTER, Duration::from_secs(300));
    assert_eq!(
        of(serde_json::json!({"notifications": {"finishedAfter": 12}})),
        Duration::from_secs(12 * 60)
    );
    for not_one in [
        serde_json::json!({"notifications": {"finishedAfter": 0}}),
        serde_json::json!({"notifications": {"finishedAfter": -3}}),
        serde_json::json!({"notifications": {"finishedAfter": 2.5}}),
        serde_json::json!({"notifications": {"finishedAfter": "10"}}),
    ] {
        assert_eq!(of(not_one), FINISHED_AFTER);
    }
}

#[test]
fn a_click_on_a_finish_s_notification_lands_on_its_line_in_updates() {
    let mut rules = Rules::default();
    let a = plane("/a");
    assert!(ended(&mut rules, &a, &[], false).is_empty());
    let notice = only_shown(&ended(&mut rules, &a, &[finish("01F", 3, true, 1)], false));
    let landed: Vec<(PlaneId, u32, Option<String>)> = landings(&notice)
        .into_iter()
        .map(|landing| (landing.plane, landing.session, landing.update))
        .collect();
    assert_eq!(landed, vec![(a, 3, Some("task:01F".to_owned()))]);
}

#[test]
fn a_project_let_go_of_and_held_again_tells_nothing_of_what_ended_meanwhile() {
    let mut rules = Rules::default();
    let a = plane("/a");
    assert!(ended(&mut rules, &a, &[], true).is_empty());
    let _ = rules.forget(&a);
    let meanwhile = [finish("while-closed", 1, true, 1)];
    assert!(ended(&mut rules, &a, &meanwhile, true).is_empty());
}

#[test]
fn a_project_whose_snooze_runs_out_is_read_then_and_its_chat_s_asks_are_sent_again() {
    // ADR 0121 decision 14 (#6018): a snoozed chat's asks are left out of what the notifier
    // hears, so they neither count nor notify; when its time is up they are heard anew.
    let mut rules = Rules::default();
    let a = plane("/a");
    let now = Instant::now();
    only_shown(&heard(
        &mut rules,
        &a,
        &[permission(3, "p1", "Run ls")],
        now,
    ));

    // Snoozed for an hour: the list the notifier hears no longer holds it, so its
    // notification is taken away and nothing is shown.
    let back = now + Duration::from_secs(3_600);
    assert!(shown(&heard(&mut rules, &a, &[], now + TOGETHER)).is_empty());
    rules.snoozed_until(&a, Some(back));
    assert_eq!(
        rules.next_due(),
        Some(back),
        "it wakes by itself when the snooze runs out"
    );
    assert!(rules.due(back - Duration::from_secs(1)).is_empty());
    assert_eq!(rules.due(back), vec![a.clone()]);

    let sent = only_shown(&heard(
        &mut rules,
        &a,
        &[permission(3, "p1", "Run ls")],
        back,
    ));
    assert_eq!(
        sent.chats,
        vec![(a.clone(), 3)],
        "its ask is sent again: {sent:?}"
    );
    rules.snoozed_until(&a, None);
    assert_eq!(rules.next_due(), None);
}

/// [`ended`], with the person away, nobody looking, and `sources` on.
fn ended_from(
    rules: &mut Rules,
    plane: &PlaneId,
    finishes: &[Finish],
    sources: Sources,
) -> Vec<Act> {
    rules.ended(&Ended {
        plane,
        finishes,
        long: FINISHED_AFTER,
        away: true,
        looking: NOBODY_LOOKS,
        quiet: false,
        sources,
    })
}

#[test]
fn a_failure_turned_off_in_attention_notifies_nothing_while_a_long_finish_still_does() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let off = Sources {
        failed: false,
        finished: true,
    };
    assert!(ended_from(&mut rules, &a, &[], off).is_empty());
    let told = ended_from(
        &mut rules,
        &a,
        &[finish("x1", 3, true, 1), finish("f1", 4, false, 20)],
        off,
    );
    assert_eq!(only_shown(&told).update.as_deref(), Some("task:f1"));
}

#[test]
fn a_long_finish_turned_off_in_attention_notifies_nothing_while_a_failure_still_does() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let off = Sources {
        failed: true,
        finished: false,
    };
    assert!(ended_from(&mut rules, &a, &[], off).is_empty());
    let told = ended_from(
        &mut rules,
        &a,
        &[finish("x1", 3, true, 1), finish("f1", 4, false, 20)],
        off,
    );
    assert_eq!(only_shown(&told).update.as_deref(), Some("task:x1"));
}

#[test]
fn a_source_turned_off_is_not_held_for_after_quiet_either() {
    let mut rules = Rules::default();
    let a = plane("/a");
    let off = Sources {
        failed: false,
        finished: false,
    };
    assert!(ended_from(&mut rules, &a, &[], off).is_empty());
    assert!(quiet_at(&mut rules, true, 1_000, true).0.is_empty());
    let held = rules.ended(&Ended {
        plane: &a,
        finishes: &[finish("x1", 3, true, 1), finish("f1", 4, false, 20)],
        long: FINISHED_AFTER,
        away: true,
        looking: NOBODY_LOOKS,
        quiet: true,
        sources: off,
    });
    assert!(held.is_empty());
    let (after, _) = quiet_at(&mut rules, false, 2_000, true);
    assert!(shown(&after).is_empty());
}

#[test]
fn both_sources_are_on_unless_the_layout_turns_one_off_by_its_kind() {
    let of = |document: serde_json::Value| Sources::of_layout(&document);
    let on = Sources {
        failed: true,
        finished: true,
    };
    assert_eq!(of(serde_json::json!({})), on);
    assert_eq!(Sources::default(), on);
    assert_eq!(
        of(serde_json::json!({"notifications": {"off": ["task-failed"]}})),
        Sources {
            failed: false,
            finished: true
        }
    );
    assert_eq!(
        of(serde_json::json!({"notifications": {"off": ["task-done", "something-else"]}})),
        Sources {
            failed: true,
            finished: false
        }
    );
    // Not a list of kinds: every source stays on, since a Should-know failure is worth hearing.
    assert_eq!(
        of(serde_json::json!({"notifications": {"off": "task-failed"}})),
        on
    );
}

/// **A vault ask's notification says Blocked, as the Inbox does** (R-170, Q259): the stop word
/// is Blocked, and the notification mirrors `InboxAsks.tsx`'s words for the kind.
#[test]
fn a_vault_asks_notification_says_blocked_as_the_inbox_does() {
    let a = plane("/a");
    let vault = ask(4, "vault:4:qa", "qa", AskSource::Vault);
    assert_eq!(
        only_shown(&heard(&mut Rules::default(), &a, &[vault], Instant::now())).body,
        "a: Blocked from a vault its persona isn't tagged for"
    );
}
