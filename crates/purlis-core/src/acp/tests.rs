// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use serde_json::json;

use super::*;

fn update(value: serde_json::Value) -> SessionUpdate {
    serde_json::from_value(value).expect("a session update")
}

#[test]
fn a_reply_longer_than_a_piece_comes_in_pieces_cut_on_character_boundaries() {
    assert_eq!(pieces("").count(), 0);
    assert_eq!(pieces("short").collect::<Vec<_>>(), ["short"]);
    // One byte, then two a character: the cap falls inside a character, so the first piece
    // ends one byte short of it.
    let text = format!("a{}", "é".repeat(MOST_TEXT_BYTES / 2));
    let cut: Vec<String> = pieces(&text).collect();
    assert_eq!(
        cut.iter().map(String::len).collect::<Vec<_>>(),
        [MOST_TEXT_BYTES - 1, 2]
    );
    assert_eq!(cut.concat(), text);
}

#[test]
fn a_long_step_is_cut_short_with_an_ellipsis() {
    assert_eq!(cut("step".to_owned(), 8), "step");
    assert_eq!(cut("abcdefgh".to_owned(), 8), "abcdefgh");
    assert_eq!(cut("abcdefghi".to_owned(), 8), "abcde…");
    let cut = cut("é".repeat(10), 8);
    assert_eq!(cut, "éé…");
    assert!(cut.len() <= 8);
}

#[test]
fn an_event_weighs_the_text_it_carries_and_a_little_for_itself() {
    assert_eq!(weight(&Event::Text("abc".to_owned())), 64 + 3);
    assert_eq!(weight(&Event::Ended), 64);
    assert_eq!(weight(&Event::Said(Said::Turn(Turn::Began))), 64);
    let plan = Plan {
        steps: vec![
            Step {
                text: "1234".to_owned(),
                done: false,
            },
            Step {
                text: "56".to_owned(),
                done: true,
            },
        ],
    };
    assert_eq!(weight(&Event::Said(Said::Plan(plan))), 64 + 6);
    assert_eq!(
        weight(&Event::ToolCallStatus {
            id: "t1".to_owned(),
            status: ToolStatus::Completed,
        }),
        // The status is charter's own word, and carries no text of the agent's.
        64 + 2
    );
    assert_eq!(
        weight(&Event::Refused {
            method: "fs/read".to_owned()
        }),
        64 + 7
    );
}

fn lines_of(bytes: &[u8]) -> Vec<std::io::Result<Option<String>>> {
    let mut reader = std::io::BufReader::new(bytes);
    let mut read = Vec::new();
    loop {
        let line = capped_line(&mut reader);
        let done = !matches!(line, Ok(Some(_)));
        read.push(line);
        if done {
            break;
        }
    }
    read
}

/// Whether `thread` ends within `within`.
fn ends_within(thread: &std::thread::JoinHandle<()>, within: std::time::Duration) -> bool {
    let until = Instant::now() + within;
    while !thread.is_finished() && Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    thread.is_finished()
}

/// Asks `incoming` for its next line once, and finds none there yet.
fn asked_and_waiting(incoming: &mut Incoming) {
    let mut cx = std::task::Context::from_waker(futures::task::noop_waker_ref());
    assert!(incoming.poll_next_unpin(&mut cx).is_pending());
}

/// #1117: each chat reads its agent's stdout on a thread of its own, not on a pool every chat
/// shares, so a chat whose stdout nothing writes to holds up no other chat's lines.
#[test]
fn a_chat_whose_stdout_is_held_open_holds_up_no_other_chat_s_lines() {
    let (stuck, _held) = std::io::pipe().expect("a pipe");
    let (mut stuck, _) = read_on_a_thread_of_its_own("stuck", stuck).expect("a reader");
    asked_and_waiting(&mut stuck);
    let (other, mut written) = std::io::pipe().expect("a pipe");
    std::io::Write::write_all(&mut written, b"one\r\ntwo\n").expect("written");
    drop(written);
    let (other, _) = read_on_a_thread_of_its_own("other", other).expect("a reader");
    let read: Vec<String> =
        futures::executor::block_on(other.map(|line| line.expect("a line")).collect());
    assert_eq!(read, ["one", "two"]);
}

/// A chat's reader reads only when the connection asks for a line, so a chat that ends while
/// nothing is asked leaves no reader behind, even while a process outside the agent's group
/// still holds its stdout.
#[test]
fn a_reader_nothing_asked_ends_with_its_chat_though_its_stdout_is_held() {
    let (stdout, held) = std::io::pipe().expect("a pipe");
    let (incoming, reading) = read_on_a_thread_of_its_own("ended", stdout).expect("a reader");
    drop(incoming);
    assert!(ends_within(&reading, std::time::Duration::from_secs(5)));
    drop(held);
}

/// A reader the chat left in a read waits there only as long as whatever holds the agent's
/// stdout lives, and then ends: the cost of an escaped holder is its own thread, for its own
/// life, and nobody else's.
#[test]
fn a_reader_left_in_a_read_ends_once_whatever_held_the_stdout_lets_go() {
    let (stdout, held) = std::io::pipe().expect("a pipe");
    let (mut incoming, reading) = read_on_a_thread_of_its_own("left", stdout).expect("a reader");
    asked_and_waiting(&mut incoming);
    drop(incoming);
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(
        !reading.is_finished(),
        "still in its read while the holder lives"
    );
    drop(held);
    assert!(ends_within(&reading, std::time::Duration::from_secs(5)));
}

/// Train 77 review: many chats started and ended leave no reader behind, whether a chat
/// ended with nothing asked, mid-read with its agent gone, or after reading to the end.
#[test]
fn many_chats_started_and_ended_leave_no_reader_behind() {
    let mut readers = Vec::new();
    for n in 0..60 {
        let (stdout, mut written) = std::io::pipe().expect("a pipe");
        let (mut incoming, reading) =
            read_on_a_thread_of_its_own(&format!("chat-{n}"), stdout).expect("a reader");
        match n % 3 {
            // Ended with nothing asked.
            0 => drop(incoming),
            // Ended mid-read, then its agent went.
            1 => {
                asked_and_waiting(&mut incoming);
                drop(incoming);
            }
            // Read to the end.
            _ => {
                std::io::Write::write_all(&mut written, b"{\"jsonrpc\":\"2.0\"}\n")
                    .expect("written");
                drop(written);
                let read: Vec<String> = futures::executor::block_on(
                    incoming.map(|line| line.expect("a line")).collect(),
                );
                assert_eq!(read.len(), 1);
                readers.push(reading);
                continue;
            }
        }
        drop(written);
        readers.push(reading);
    }
    for reading in &readers {
        assert!(ends_within(reading, std::time::Duration::from_secs(5)));
    }
}

/// A line as long as the cap allows comes through whole on the reader's thread.
#[test]
fn a_long_line_comes_through_whole_on_the_reader_s_thread() {
    let long = "x".repeat(MOST_LINE_BYTES);
    let (stdout, mut written) = std::io::pipe().expect("a pipe");
    let (incoming, _) = read_on_a_thread_of_its_own("long", stdout).expect("a reader");
    let writing = {
        let long = long.clone();
        std::thread::spawn(move || {
            std::io::Write::write_all(&mut written, long.as_bytes()).expect("written");
            std::io::Write::write_all(&mut written, b"\nshort\n").expect("written");
        })
    };
    let read: Vec<String> =
        futures::executor::block_on(incoming.map(|line| line.expect("a line")).collect());
    writing.join().expect("the writer");
    assert_eq!(read, [long, "short".to_owned()]);
}

#[test]
fn a_line_is_read_without_its_ending_and_the_last_one_needs_none() {
    let read: Vec<Option<String>> = lines_of(b"one\r\ntwo\nthree")
        .into_iter()
        .map(|line| line.expect("read"))
        .collect();
    assert_eq!(
        read,
        [
            Some("one".to_owned()),
            Some("two".to_owned()),
            Some("three".to_owned()),
            None
        ]
    );
}

#[test]
fn a_line_past_the_cap_is_an_error_and_one_at_it_is_read() {
    let mut at_cap = vec![b'x'; MOST_LINE_BYTES];
    at_cap.push(b'\n');
    let read = lines_of(&at_cap);
    assert_eq!(
        read[0].as_ref().expect("read").as_ref().map(String::len),
        Some(MOST_LINE_BYTES)
    );
    let mut past = vec![b'x'; MOST_LINE_BYTES + 1];
    past.push(b'\n');
    assert!(lines_of(&past)[0].is_err());
}

/// Each stop reason is its own, a cancel among them (#1117). One the protocol adds later reads
/// as [`Stop::Unknown`], never as a cancel; the SDK has none to build here.
#[test]
fn each_stop_reason_is_its_own() {
    assert_eq!(stop(StopReason::EndTurn), Stop::EndTurn);
    assert_eq!(stop(StopReason::MaxTokens), Stop::MaxTokens);
    assert_eq!(stop(StopReason::MaxTurnRequests), Stop::MaxTurnRequests);
    assert_eq!(stop(StopReason::Refusal), Stop::Refusal);
    assert_eq!(stop(StopReason::Cancelled), Stop::Cancelled);
}

/// #1117: a tool call's kind and status are charter's own enums, so a consumer matches them
/// rather than compare the protocol's words; a kind the protocol does not know is `Other`.
#[test]
fn a_tool_call_s_kind_and_status_are_charter_s_own() {
    let call = |kind: &str, status: &str| {
        reported_as(update(
            json!({"sessionUpdate": "tool_call", "toolCallId": "t2",
                                  "title": "Switch", "kind": kind, "status": status}),
        ))
    };
    assert_eq!(
        call("switch_mode", "in_progress"),
        [Event::ToolCall {
            id: "t2".to_owned(),
            title: "Switch".to_owned(),
            kind: ToolKind::SwitchMode,
            status: ToolStatus::InProgress,
        }]
    );
    let unknown = call("teleport", "pending");
    let [Event::ToolCall { kind, .. }] = unknown.as_slice() else {
        panic!("one tool call");
    };
    assert_eq!(*kind, ToolKind::Other);
    for (kind, word) in [
        (ToolKind::Read, "read"),
        (ToolKind::Edit, "edit"),
        (ToolKind::Delete, "delete"),
        (ToolKind::Move, "move"),
        (ToolKind::Search, "search"),
        (ToolKind::Execute, "execute"),
        (ToolKind::Think, "think"),
        (ToolKind::Fetch, "fetch"),
        (ToolKind::SwitchMode, "switch_mode"),
        (ToolKind::Other, "other"),
    ] {
        assert_eq!(
            call(word, "pending")[0],
            Event::ToolCall {
                id: "t2".to_owned(),
                title: "Switch".to_owned(),
                kind,
                status: ToolStatus::Pending,
            }
        );
        assert_eq!(kind.word(), word);
    }
    for (status, word) in [
        (ToolStatus::Pending, "pending"),
        (ToolStatus::InProgress, "in_progress"),
        (ToolStatus::Completed, "completed"),
        (ToolStatus::Failed, "failed"),
    ] {
        assert_eq!(status.word(), word);
    }
}

#[test]
fn a_plan_keeps_its_first_steps_each_cut_to_size() {
    let mut entries = vec![
        json!({"content": "p".repeat(MOST_STEP_BYTES * 2), "priority": "high", "status": "completed"}),
    ];
    entries.extend((0..MOST_PLAN_STEPS * 2).map(
        |at| json!({"content": format!("step {at}"), "priority": "low", "status": "pending"}),
    ));
    let reported = reported_as(update(json!({"sessionUpdate": "plan", "entries": entries})));
    let [Event::Said(Said::Plan(plan))] = reported.as_slice() else {
        panic!("{reported:?}");
    };
    assert_eq!(plan.steps.len(), MOST_PLAN_STEPS);
    assert!(plan.steps[0].done);
    assert!(plan.steps[0].text.len() <= MOST_STEP_BYTES);
    assert!(plan.steps[0].text.ends_with('…'));
    assert_eq!(plan.steps[1].text, "step 0");
}

#[test]
fn what_an_update_reports_in_charter_s_words() {
    assert_eq!(
        reported_as(update(
            json!({"sessionUpdate": "usage_update", "used": 50, "size": 200})
        )),
        [Event::Said(Said::Usage(Usage {
            input_tokens: None,
            output_tokens: None,
            context_percent: Some(25)
        }))]
    );
    // A window of no size reports no percentage, rather than divide by it.
    assert_eq!(
        reported_as(update(
            json!({"sessionUpdate": "usage_update", "used": 50, "size": 0})
        )),
        [Event::Said(Said::Usage(Usage::default()))]
    );
    assert_eq!(
        reported_as(update(
            json!({"sessionUpdate": "tool_call_update", "toolCallId": "t1"})
        )),
        []
    );
    assert_eq!(
        reported_as(update(
            json!({"sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "failed"})
        )),
        [Event::ToolCallStatus {
            id: "t1".to_owned(),
            status: ToolStatus::Failed,
        }]
    );
    let long = "a".repeat(MOST_TEXT_BYTES + 1);
    assert_eq!(
        reported_as(update(json!({"sessionUpdate": "agent_message_chunk",
                                   "content": {"type": "text", "text": long}})))
        .len(),
        2
    );
    assert_eq!(
        reported_as(update(json!({"sessionUpdate": "agent_thought_chunk",
                                   "content": {"type": "text", "text": "hmm"}}))),
        []
    );
}

#[test]
fn what_waits_to_be_written_is_bounded_in_bytes_and_one_line_past_the_bound_closes_it() {
    let unwritten = Unwritten::default();
    let half = "h".repeat(MOST_UNWRITTEN_BYTES / 2 - 1);
    unwritten.hold(half.clone()).expect("half fits");
    unwritten
        .hold(half.clone())
        .expect("the other half fits, line endings included");
    assert!(
        unwritten.hold("x".to_owned()).is_err(),
        "one byte past the bound"
    );
    // Closed: nothing more is held, and what was held is let go.
    assert!(unwritten.hold(String::new()).is_err());
    assert_eq!(lock(&unwritten.queue).lines.len(), 0);
}

#[test]
fn a_line_longer_than_the_bound_is_refused_on_its_own() {
    let unwritten = Unwritten::default();
    assert!(unwritten.hold("y".repeat(MOST_UNWRITTEN_BYTES)).is_err());
}

#[test]
fn a_prompt_too_long_to_write_is_refused_before_it_is_sent() {
    let session = SessionId::new("s-1");
    assert_eq!(prompt_request(&session, "hello").map(|_| ()), Ok(()));
    // The text alone fills the budget, so the line it is written in cannot fit.
    assert_eq!(
        prompt_request(&session, &"p".repeat(MOST_UNWRITTEN_BYTES)).map(|_| ()),
        Err(TurnFailed::TooLong)
    );
    // Measured as written: each quote is escaped, so half the budget in quotes is over it.
    assert_eq!(
        prompt_request(&session, &"\"".repeat(MOST_UNWRITTEN_BYTES / 2)).map(|_| ()),
        Err(TurnFailed::TooLong)
    );
    // Under the bound with room for the envelope: sent.
    assert!(prompt_request(&session, &"p".repeat(MOST_UNWRITTEN_BYTES - 4096)).is_ok());
    assert_eq!(
        TurnFailed::TooLong.to_string(),
        "the prompt is longer than purlis sends to an agent, 8 MiB"
    );
}

#[test]
fn the_writer_writes_each_line_in_order_and_gives_its_bytes_back() {
    let unwritten = Arc::new(Unwritten::default());
    unwritten.hold("one".to_owned()).expect("held");
    unwritten.hold("two".to_owned()).expect("held");
    let (read, written) = std::io::pipe().expect("a pipe");
    let writer = {
        let unwritten = Arc::clone(&unwritten);
        std::thread::spawn(move || unwritten.write_to(written))
    };
    let mut lines = std::io::BufRead::lines(std::io::BufReader::new(read));
    assert_eq!(lines.next().expect("a line").expect("read"), "one");
    assert_eq!(lines.next().expect("a line").expect("read"), "two");
    unwritten.close();
    writer.join().expect("the writer ends once closed");
    assert_eq!(lock(&unwritten.queue).bytes, 0);
}

#[test]
fn the_board_hears_a_chat_s_turns_and_asks_and_nothing_of_what_it_said() {
    // ADR 0073: the board reads the neutral model, so a level-3 chat moves exactly as a level-2
    // chat whose hooks said the same. Its text, its tool calls and a refused call move nothing;
    // its end is the program's exit, which the host reads from the operating system.
    let asks = Asks::new();
    let ask = crate::harness::model::Ask::default();
    let raised = asks.raise("chat-1", ask.clone(), Instant::now()).raised;
    let began = Said::Turn(Turn::Began);

    assert_eq!(Event::Said(began.clone()).said(), Some(began));
    assert_eq!(Event::Raised(raised).said(), Some(Said::Ask(ask)));
    for quiet in [
        Event::Text("hello".to_owned()),
        Event::ToolCall {
            id: "t".to_owned(),
            title: "ls".to_owned(),
            kind: ToolKind::Execute,
            status: ToolStatus::Pending,
        },
        Event::ToolCallStatus {
            id: "t".to_owned(),
            status: ToolStatus::Completed,
        },
        Event::Refused {
            method: "fs/read_text_file".to_owned(),
        },
        Event::Ended,
    ] {
        assert_eq!(quiet.said(), None, "{quiet:?}");
    }
}
