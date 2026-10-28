// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::cell::Cell;
use std::path::Path;

use super::*;
use crate::planes::PlaneId;

fn plane() -> PlaneId {
    PlaneId::for_tests(Path::new("/plane"))
}

fn runner(pool: &str, runner: &str, draining: bool) -> KeptRunner {
    KeptRunner {
        pool: pool.to_owned(),
        runner: runner.to_owned(),
        draining,
    }
}

fn chat(session: u32, name: &str, on: Option<&str>) -> Placed {
    Placed {
        plane: plane(),
        session,
        name: name.to_owned(),
        on: on.map(str::to_owned),
    }
}

fn browser(session: u32, name: &str, chat: &str, on: Option<&str>) -> BrowserPlaced {
    BrowserPlaced {
        plane: plane(),
        session,
        program: format!("{session}-{name}"),
        name: name.to_owned(),
        chat: chat.to_owned(),
        on: on.map(str::to_owned),
    }
}

fn sessions(on: &OnADevice) -> Vec<u32> {
    on.chats.iter().map(|chat| chat.session).collect()
}

#[test]
fn a_chat_with_no_pool_is_on_this_machine_and_one_on_a_pool_is_on_its_runner() {
    let shown = placed(
        &[runner("builds", "builds", false)],
        vec![chat(1, "lint", None), chat(2, "verify", Some("builds"))],
        Vec::new(),
    );
    assert_eq!(sessions(&shown.here), [1]);
    assert_eq!(shown.runners.len(), 1);
    assert_eq!(shown.runners[0].pool, "builds");
    assert_eq!(sessions(&shown.runners[0].on), [2]);
    assert_eq!(shown.runners[0].on.chats[0].name, "verify");
    assert!(shown.unkept.is_empty());
}

#[test]
fn a_chat_on_a_pool_is_on_its_runner_that_takes_new_chats_and_one_named_whole_on_the_draining_one()
{
    let shown = placed(
        &[
            runner("builds", "builds", true),
            runner("builds", "builds-2b1f", false),
        ],
        vec![
            chat(3, "new", Some("builds")),
            chat(4, "old", Some("builds/builds")),
        ],
        Vec::new(),
    );
    let by_name = |name: &str| {
        shown
            .runners
            .iter()
            .find(|one| one.runner == name)
            .expect("the runner is listed")
    };
    assert_eq!(sessions(&by_name("builds-2b1f").on), [3]);
    assert_eq!(sessions(&by_name("builds").on), [4]);
    assert!(by_name("builds").draining);
}

#[test]
fn every_kept_runner_is_listed_whether_or_not_a_chat_runs_on_it() {
    let shown = placed(
        &[runner("builds", "builds", false), runner("qa", "qa", false)],
        Vec::new(),
        Vec::new(),
    );
    let names: Vec<&str> = shown
        .runners
        .iter()
        .map(|one| one.runner.as_str())
        .collect();
    assert_eq!(names, ["builds", "qa"]);
    assert!(shown.runners.iter().all(|one| one.on.chats.is_empty()));
}

#[test]
fn a_chat_on_a_pool_this_machine_no_longer_keeps_is_said_apart_never_put_on_another_device() {
    let shown = placed(
        &[runner("builds", "builds", false)],
        vec![
            chat(5, "gone", Some("removed")),
            chat(6, "gone too", Some("builds/old")),
        ],
        Vec::new(),
    );
    assert!(shown.here.chats.is_empty());
    assert!(shown.runners[0].on.chats.is_empty());
    let unkept: Vec<(u32, &str)> = shown
        .unkept
        .iter()
        .map(|one| (one.session, one.pool.as_str()))
        .collect();
    assert_eq!(unkept, [(5, "removed"), (6, "builds/old")]);
}

#[test]
fn a_browser_is_on_the_device_its_chat_runs_it_on() {
    let shown = placed(
        &[runner("builds", "builds", false)],
        Vec::new(),
        vec![
            browser(1, "b1", "lint", None),
            browser(2, "b1", "verify", Some("builds")),
        ],
    );
    assert_eq!(shown.here.browsers.len(), 1);
    assert_eq!(shown.here.browsers[0].chat, "lint");
    assert_eq!(shown.runners[0].on.browsers.len(), 1);
    assert_eq!(shown.runners[0].on.browsers[0].session, 2);
    assert_eq!(shown.runners[0].on.browsers[0].name, "b1");
}

fn listed(
    id: &str,
    project: &str,
    number: Option<u32>,
    state: &str,
    browser: bool,
) -> ListedProgram {
    ListedProgram {
        id: id.to_owned(),
        project: project.to_owned(),
        number,
        ended: state == "ended",
        browser,
    }
}

#[test]
fn a_suspended_runner_is_never_linked_to_and_keeps_when_it_last_answered() {
    let linked = Cell::new(false);
    let asked = answered(
        Ok(false),
        || {
            linked.set(true);
            Ok(Vec::new())
        },
        || None,
        Some(1_000),
        2_000,
    );
    assert!(!linked.get(), "a suspended runner is not woken to be asked");
    assert_eq!(asked.standing, RunnerStanding::Suspended);
    assert_eq!(asked.link, None);
    assert_eq!(asked.contacted, Some(1_000));
}

#[test]
fn a_running_runner_that_answers_is_contacted_now_and_counts_what_it_holds_that_has_not_ended() {
    let asked = answered(
        Ok(true),
        || {
            Ok(vec![
                listed("c1", "p1", Some(1), "running", false),
                listed("c2", "p1", Some(2), "ended", false),
                listed("c3", "p2", Some(1), "running", false),
                listed("b1", "p1", None, "running", true),
            ])
        },
        || Some("0.9.0".to_owned()),
        Some(1_000),
        2_000,
    );
    assert_eq!(asked.standing, RunnerStanding::Running);
    assert_eq!(asked.link, Some(PoolLink::Linked));
    assert_eq!(asked.contacted, Some(2_000));
    assert_eq!(asked.version.as_deref(), Some("0.9.0"));
    let holds: Vec<(&str, Option<u32>, bool)> = asked
        .holds
        .iter()
        .map(|one| (one.project.as_str(), one.number, one.browser))
        .collect();
    assert_eq!(
        holds,
        [
            ("p1", Some(1), false),
            ("p2", Some(1), false),
            ("p1", None, true)
        ]
    );
}

#[test]
fn a_running_runner_that_does_not_answer_says_why_and_keeps_its_last_contact() {
    let asked = answered(
        Ok(true),
        || {
            Err(PoolLink::Down {
                why: "the runner did not answer in time".to_owned(),
            })
        },
        || None,
        Some(1_000),
        2_000,
    );
    assert_eq!(
        asked.link,
        Some(PoolLink::Down {
            why: "the runner did not answer in time".to_owned()
        })
    );
    assert_eq!(asked.contacted, Some(1_000));
    assert!(asked.holds.is_empty());
}

#[test]
fn a_runner_whose_container_cannot_be_read_is_not_linked_to_and_says_why() {
    let linked = Cell::new(false);
    let asked = answered(
        Err("docker is not running".to_owned()),
        || {
            linked.set(true);
            Ok(Vec::new())
        },
        || None,
        None,
        2_000,
    );
    assert!(!linked.get());
    assert_eq!(
        asked.standing,
        RunnerStanding::Unknown {
            why: "docker is not running".to_owned()
        }
    );
    assert_eq!(asked.contacted, None);
}

#[test]
fn the_last_contact_is_kept_for_each_runner_apart() {
    let contacts = Contacts::default();
    contacts.answered("builds/builds", 10);
    contacts.answered("qa/qa", 20);
    contacts.answered("builds/builds", 30);
    assert_eq!(contacts.last("builds/builds"), Some(30));
    assert_eq!(contacts.last("qa/qa"), Some(20));
    assert_eq!(contacts.last("other/other"), None);
}

fn open_chat(session: u32, pool: Option<&str>) -> OpenChat {
    OpenChat {
        plane: plane(),
        session,
        pool: pool.map(str::to_owned),
    }
}

fn open_browser(program: &str, session: u32, pool: Option<&str>) -> (String, OpenChat) {
    (program.to_owned(), open_chat(session, pool))
}

#[test]
fn what_a_runner_holds_for_chats_and_browsers_the_window_has_not_open_is_counted_apart() {
    let holds = vec![
        listed("c1", "p1", Some(1), "running", false),
        listed("c3", "p2", Some(1), "running", false),
        listed("c4", "p1", Some(7), "running", false),
        listed("b1", "p1", None, "running", true),
        listed("b2", "p2", None, "running", true),
    ];
    let open = OpenHere {
        chats: [(("p1".to_owned(), 1), open_chat(1, Some("builds")))]
            .into_iter()
            .collect(),
        browsers: [open_browser("b1", 1, Some("builds"))]
            .into_iter()
            .collect(),
    };
    assert_eq!(
        not_open_here(&holds, &open, "builds"),
        NotOpenHere {
            open: vec![ChatHeld {
                plane: plane(),
                session: 1
            }],
            browsers_open: vec!["b1".to_owned()],
            chats: 2,
            browsers: 1
        }
    );
}

#[test]
fn a_runner_is_believed_only_about_chats_and_browsers_the_window_placed_on_its_own_pool() {
    // A runner of pool `qa` says it holds a chat and a browser this machine runs, and a chat
    // and a browser the window placed on pool `builds`.
    let holds = vec![
        listed("c1", "p1", Some(1), "running", false),
        listed("c2", "p1", Some(2), "running", false),
        listed("b1", "p1", None, "running", true),
        listed("b2", "p1", None, "running", true),
    ];
    let open = OpenHere {
        chats: [
            (("p1".to_owned(), 1), open_chat(1, None)),
            (("p1".to_owned(), 2), open_chat(2, Some("builds"))),
        ]
        .into_iter()
        .collect(),
        browsers: [
            open_browser("b1", 1, None),
            open_browser("b2", 2, Some("builds")),
        ]
        .into_iter()
        .collect(),
    };
    assert_eq!(not_open_here(&holds, &open, "qa"), NotOpenHere::default());
}

#[test]
fn a_browser_on_a_pool_this_machine_no_longer_keeps_is_said_apart_with_its_chat() {
    let shown = placed(
        &[runner("builds", "builds", false)],
        vec![chat(5, "gone", Some("removed"))],
        vec![browser(5, "b1", "gone", Some("removed"))],
    );
    assert!(shown.here.browsers.is_empty());
    assert!(shown.runners[0].on.browsers.is_empty());
    let unkept: Vec<(u32, &str, &str)> = shown
        .unkept_browsers
        .iter()
        .map(|one| (one.session, one.name.as_str(), one.pool.as_str()))
        .collect();
    assert_eq!(unkept, [(5, "b1", "removed")]);
}

#[test]
fn text_shown_from_elsewhere_has_no_hidden_or_reordering_characters_and_one_line() {
    let said = shown_safely("bad\u{202E}gnirts\nnext\tline\u{200B}");
    assert_eq!(said, "badgnirts next line");
}

#[test]
fn text_shown_from_elsewhere_is_capped() {
    let said = shown_safely(&"x".repeat(1 << 20));
    assert_eq!(said.chars().count(), MOST_SHOWN + 1);
    assert!(said.ends_with('…'));
}

#[test]
fn text_shown_from_elsewhere_names_no_address_s_user_or_password() {
    assert_eq!(
        shown_safely("Cannot connect to ssh://me:secret@build.example:22 or tcp://ops@10.0.0.2"),
        "Cannot connect to ssh://…@build.example:22 or tcp://…@10.0.0.2"
    );
}

#[test]
fn a_runner_s_refusal_of_its_list_is_said_in_purlis_s_words_with_its_code_alone() {
    use purlis_session_protocol::session::{CallError, Refusal};
    let refused = |code: &str, why: String| {
        list_refused(&CallError::Refused(Refusal {
            code: code.to_owned(),
            why,
        }))
    };
    assert_eq!(
        refused("not_allowed", "x".repeat(1 << 20)),
        "the runner refused to list what it holds (not_allowed)"
    );
    assert_eq!(
        refused(
            "Open \u{202E}Settings",
            "key changed, open Settings".to_owned()
        ),
        "the runner refused to list what it holds"
    );
    assert_eq!(
        list_refused(&CallError::Unexpected("{\"lots\": 1}".to_owned())),
        "the runner answered something purlis does not read"
    );
}

#[test]
fn what_docker_says_of_a_container_is_shown_safely() {
    let asked = answered(
        Err("cannot connect to ssh://me:pw@host\u{202E}x".to_owned()),
        || Ok(Vec::new()),
        || None,
        None,
        2_000,
    );
    assert_eq!(
        asked.standing,
        RunnerStanding::Unknown {
            why: "cannot connect to ssh://…@hostx".to_owned()
        }
    );
}

#[test]
fn a_runner_asked_again_while_it_is_being_asked_is_asked_once() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    let joined = Arc::new(Joined::<u32>::default());
    let asked = Arc::new(AtomicU32::new(0));
    let (started, go) = (
        Arc::new(std::sync::Barrier::new(2)),
        Arc::new(std::sync::Barrier::new(2)),
    );
    let first = {
        let (joined, asked, started, go) =
            (joined.clone(), asked.clone(), started.clone(), go.clone());
        std::thread::spawn(move || {
            joined.once(
                "builds/builds",
                std::time::Duration::from_secs(60),
                || {
                    started.wait();
                    go.wait();
                    asked.fetch_add(1, Ordering::SeqCst) + 1
                },
                || 0,
            )
        })
    };
    started.wait();
    let second = {
        let (joined, asked) = (joined.clone(), asked.clone());
        std::thread::spawn(move || {
            joined.once(
                "builds/builds",
                std::time::Duration::from_secs(60),
                || asked.fetch_add(1, Ordering::SeqCst) + 1,
                || 0,
            )
        })
    };
    // The second waits on the first's answer rather than asking again.
    while !joined.is_waited_on("builds/builds") {
        std::thread::yield_now();
    }
    go.wait();
    assert_eq!(first.join().unwrap(), 1);
    assert_eq!(second.join().unwrap(), 1);
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    // Once answered, the next ask asks again.
    assert_eq!(
        joined.once(
            "builds/builds",
            std::time::Duration::from_secs(60),
            || 7,
            || 0
        ),
        7
    );
}

#[test]
fn an_ask_joined_to_one_that_never_answers_is_answered_late_once_its_time_is_up() {
    use std::sync::Arc;
    let joined = Arc::new(Joined::<u32>::default());
    let (started, release) = (
        Arc::new(std::sync::Barrier::new(2)),
        Arc::new(std::sync::Barrier::new(2)),
    );
    let first = {
        let (joined, started, release) = (joined.clone(), started.clone(), release.clone());
        std::thread::spawn(move || {
            joined.once(
                "builds/builds",
                std::time::Duration::from_secs(60),
                || {
                    started.wait();
                    // An ask that does not answer until the test lets it.
                    release.wait();
                    1
                },
                || 0,
            )
        })
    };
    started.wait();
    let waited = std::time::Instant::now();
    let late = joined.once(
        "builds/builds",
        std::time::Duration::from_millis(200),
        || 2,
        || 9,
    );
    assert_eq!(late, 9, "it is answered late, not left waiting");
    assert!(waited.elapsed() < std::time::Duration::from_secs(10));
    release.wait();
    assert_eq!(first.join().unwrap(), 1);
    assert!(!joined.is_asking("builds/builds"));
}

#[test]
fn an_ask_that_panics_leaves_its_place_and_whoever_waits_on_it_is_answered_late() {
    use std::sync::Arc;
    let joined = Arc::new(Joined::<u32>::default());
    let (started, go) = (
        Arc::new(std::sync::Barrier::new(2)),
        Arc::new(std::sync::Barrier::new(2)),
    );
    let first = {
        let (joined, started, go) = (joined.clone(), started.clone(), go.clone());
        std::thread::spawn(move || {
            joined.once(
                "builds/builds",
                std::time::Duration::from_secs(60),
                || {
                    started.wait();
                    go.wait();
                    panic!("the ask failed");
                },
                || 0,
            )
        })
    };
    started.wait();
    let second = {
        let joined = joined.clone();
        std::thread::spawn(move || {
            joined.once(
                "builds/builds",
                std::time::Duration::from_secs(60),
                || 2,
                || 9,
            )
        })
    };
    while !joined.is_waited_on("builds/builds") {
        std::thread::yield_now();
    }
    go.wait();
    assert!(first.join().is_err(), "the first ask panicked");
    assert_eq!(
        second.join().unwrap(),
        9,
        "the one waiting on it is answered late"
    );
    assert!(!joined.is_asking("builds/builds"), "its place is cleared");
    assert_eq!(
        joined.once(
            "builds/builds",
            std::time::Duration::from_secs(60),
            || 3,
            || 0
        ),
        3,
        "the next ask asks again"
    );
}

#[test]
fn a_runner_not_answered_in_time_is_not_known_and_keeps_its_last_contact() {
    let asked = unanswered(Some(1_000));
    assert_eq!(asked.standing, RunnerStanding::Unanswered);
    assert_eq!(asked.link, None);
    assert_eq!(asked.contacted, Some(1_000));
}

#[test]
fn a_password_that_holds_a_slash_is_hidden_too() {
    assert_eq!(
        shown_safely("cannot reach tcp://someone:pass/word@engine.example:2376"),
        "cannot reach tcp://…@engine.example:2376"
    );
}

#[test]
fn a_user_named_without_a_scheme_is_hidden_and_an_image_digest_is_kept() {
    assert_eq!(
        shown_safely("someone@build.example: Permission denied (publickey)."),
        "…@build.example: Permission denied (publickey)."
    );
    let pulling = format!(
        "pulling ghcr.io/example/runner@sha256:{} and runner@sha256:{}",
        "0".repeat(64),
        "a".repeat(64)
    );
    assert_eq!(shown_safely(&pulling), pulling);
    assert_eq!(shown_safely("as 'someone@host'"), "as '…@host'");
}

#[test]
fn a_user_and_password_without_a_scheme_are_hidden_whatever_the_password_holds() {
    assert_eq!(
        shown_safely("cannot log in as someone:pass/word@build.example"),
        "cannot log in as …@build.example"
    );
    // A path or a registry before an `@` is no one's name.
    assert_eq!(shown_safely("see docs/guide@main"), "see docs/guide@main");
}

#[test]
fn a_long_text_cut_part_way_through_an_address_shows_none_of_that_address() {
    let said = shown_safely(&format!(
        "{}tcp://someone:hidden@engine.example",
        " ".repeat(1585)
    ));
    assert!(!said.contains("someone"), "{said}");
    assert!(!said.contains("tcp:"), "{said}");
}

#[test]
fn words_are_parted_by_every_kind_of_blank_before_an_address_is_read() {
    for blank in ['\u{00A0}', '\u{000C}', '\u{2028}', '\u{0085}', '\u{000B}'] {
        assert_eq!(
            shown_safely(&format!("see path/x{blank}someone:hidden@build.example")),
            "see path/x …@build.example",
            "{blank:?}"
        );
    }
}

#[test]
fn a_user_and_password_after_a_path_are_hidden() {
    assert_eq!(
        shown_safely("org/someone:hidden@build.example"),
        "org/…@build.example"
    );
    assert_eq!(
        shown_safely("/someone:hidden@build.example"),
        "/…@build.example"
    );
    assert_eq!(
        shown_safely("./someone:hidden@build.example"),
        "./…@build.example"
    );
}

#[test]
fn a_user_and_password_are_hidden_whatever_host_follows_them() {
    assert_eq!(shown_safely("someone:hidden@sha256:0123"), "…@sha256:0123");
    assert_eq!(shown_safely("someone:hidden@SHA256:0123"), "…@SHA256:0123");
}

#[test]
fn harmless_words_digests_and_image_names_still_show() {
    let (sha256, sha384, sha512) = ("0".repeat(64), "B".repeat(96), "c".repeat(128));
    for kept in [
        // Each under the cap on what is shown.
        format!("pulling ghcr.io/example/runner@sha256:{sha256}"),
        format!("pulling runner@SHA384:{sha384}"),
        format!("localhost:5000/example/runner@sha512:{sha512}"),
        "see docs/guide@main, tcp://127.0.0.1:2375 and https://example.com/a/b".to_owned(),
        "listening on 0.0.0.0:8080".to_owned(),
    ] {
        assert_eq!(shown_safely(&kept), kept);
    }
}

#[test]
fn an_image_named_by_a_tag_and_a_digest_is_kept_whole() {
    let digest = "0123456789abcdef".repeat(4);
    for kept in [
        format!("example/runner:v1@sha256:{digest}"),
        format!("runner:v1@sha256:{digest}"),
        format!("registry.example:5000/team/runner:v1@sha256:{digest}"),
        format!("pulled \"runner:v1@sha256:{digest}\"."),
    ] {
        assert_eq!(shown_safely(&kept), kept);
    }
}

#[test]
fn a_password_that_starts_with_digits_is_hidden() {
    assert_eq!(
        shown_safely("someone:5000/secret@build.example"),
        "…@build.example"
    );
}

#[test]
fn a_user_and_password_joined_to_a_path_by_a_hidden_character_are_hidden() {
    assert_eq!(
        shown_safely("see path/x\u{200B}someone:hidden@build.example"),
        "see path/…@build.example"
    );
}

/// A pool's hours this week include its runners no longer kept (#3492): a draining runner
/// removed once its chats ended still ran for its pool. A kept runner's hours are in its own
/// answer, so they are not counted here twice.
#[test]
fn a_pools_removed_runners_hours_this_week_are_counted_and_its_kept_ones_are_not() {
    use purlis_core::runnerhours::{Event, Line};
    const HOUR: u64 = 3600;
    let week = 1_792_972_800;
    let line = |at, pool: &str, runner: &str, event| Line::new(at, pool, runner, event);
    let hours = vec![
        line(week - 3 * HOUR, "builds", "builds", Event::Start),
        line(week + HOUR, "builds", "builds", Event::Stop),
        line(week + 2 * HOUR, "builds", "builds-1a2b3c4d", Event::Start),
        line(week + 5 * HOUR, "builds", "builds-1a2b3c4d", Event::Stop),
        line(week + 2 * HOUR, "tests", "tests", Event::Start),
        line(week + 9 * HOUR, "tests", "tests", Event::Stop),
    ];
    // `builds` was removed; its successor is kept.
    let kept = [runner("builds", "builds-1a2b3c4d", false)];
    assert_eq!(
        removed_ran(&hours, "builds", &kept, week),
        u32::try_from(HOUR).unwrap()
    );
    assert_eq!(removed_ran(&hours, "other", &kept, week), 0);
}
