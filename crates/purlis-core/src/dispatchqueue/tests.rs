// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::hookwire::ChatToken;

/// A project's hook socket, and the token chat `chat` was issued there, its key remembered as
/// the app remembers it.
fn project(chat: u32, token: &str) -> (tempfile::TempDir, PathBuf, ChatToken) {
    let root = tempfile::tempdir().expect("a folder");
    let app = root.path().join(".purlis/app");
    std::fs::create_dir_all(&app).expect("the app folder");
    let socket = app.join("hooks.sock");
    let token = ChatToken::from(token);
    crate::hookwire::spool::remember(&spool_dir(&socket), chat, &token).expect("the key");
    (root, socket, token)
}

fn tell(to: u32, text: &str) -> Queued {
    Queued::Tell {
        to,
        text: text.to_owned(),
    }
}

fn messages(found: Vec<Found>) -> Vec<Waiting> {
    found
        .into_iter()
        .map(|found| match found {
            Found::Message(message) => message,
            Found::Unchecked(path) | Found::Delivered(path) => {
                panic!("{} is not a fresh message", path.display())
            }
        })
        .collect()
}

#[test]
fn queued_messages_wait_in_the_order_they_were_queued_until_each_is_delivered() {
    let (_root, socket, token) = project(3, "tok-three");
    let first = queue(&socket, 3, &token, tell(4, "one"), 10).expect("queued");
    let second = queue(
        &socket,
        3,
        &token,
        Queued::TellAll { text: "two".into() },
        11,
    )
    .expect("queued");

    assert!(holds(&socket, 3));
    assert_eq!(chats(&socket), vec![3]);
    let waiting = messages(waiting(&socket, 3));
    assert_eq!(
        waiting
            .iter()
            .map(|m| (m.id.clone(), m.what.clone()))
            .collect::<Vec<_>>(),
        vec![
            (first, tell(4, "one")),
            (second, Queued::TellAll { text: "two".into() })
        ]
    );
    assert_eq!(waiting[0].at, 10);

    // Delivered once: gone, and the next look finds only the other.
    delivered(&socket, 3, &waiting[0]);
    let left = messages(super::waiting(&socket, 3));
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].what, Queued::TellAll { text: "two".into() });
    delivered(&socket, 3, &left[0]);
    assert!(!holds(&socket, 3));
}

#[test]
fn a_message_signed_with_another_token_than_the_chat_s_is_never_delivered() {
    let (_root, socket, _token) = project(3, "tok-three");
    // Another chat's token, written into chat 3's queue.
    queue(
        &socket,
        3,
        &ChatToken::from("not-three"),
        tell(4, "forged"),
        1,
    )
    .expect("written");

    assert!(matches!(&waiting(&socket, 3)[..], [Found::Unchecked(_)]));
}

#[test]
fn a_message_whose_body_was_changed_after_it_was_queued_does_not_check() {
    let (_root, socket, token) = project(3, "tok-three");
    queue(&socket, 3, &token, tell(4, "as sent"), 1).expect("queued");
    let path = dir_for(&socket, 3).join(name(1, "json"));
    let text = std::fs::read_to_string(&path).expect("the file");
    std::fs::write(&path, text.replace("as sent", "as changed")).expect("rewritten");

    let found = waiting(&socket, 3);
    let [Found::Unchecked(unchecked)] = &found[..] else {
        panic!("{found:?}");
    };
    drop_unchecked(unchecked);
    assert!(!holds(&socket, 3));
}

#[test]
fn nothing_is_queued_beside_a_socket_outside_a_project_s_state_folder() {
    let root = tempfile::tempdir().expect("a folder");
    let socket = root.path().join("hooks.sock");

    let refused = queue(&socket, 3, &ChatToken::from("t"), tell(4, "x"), 1).expect_err("refused");

    assert!(matches!(refused, Unqueued::Unreachable(_)), "{refused:?}");
    assert!(
        refused.say().contains("nothing was queued"),
        "{}",
        refused.say()
    );
}

#[test]
fn a_full_queue_refuses_one_more() {
    let (_root, socket, token) = project(3, "tok-three");
    for n in 0..MOST_QUEUED {
        queue(&socket, 3, &token, tell(4, &n.to_string()), 1).expect("queued");
    }

    let refused = queue(&socket, 3, &token, tell(4, "one more"), 1).expect_err("full");

    assert!(matches!(refused, Unqueued::Full));
}

#[test]
fn a_copy_of_a_delivered_message_is_refused_and_numbers_never_go_back() {
    let (_root, socket, token) = project(3, "tok-three");
    queue(&socket, 3, &token, tell(4, "once"), 1).expect("queued");
    let path = dir_for(&socket, 3).join(name(1, "json"));
    let copy = std::fs::read(&path).expect("the file");
    let first = messages(waiting(&socket, 3));
    delivered(&socket, 3, &first[0]);

    // Put back after the folder drained: it is said to be delivered already, not delivered
    // again.
    std::fs::write(&path, &copy).expect("put back");
    assert!(matches!(&waiting(&socket, 3)[..], [Found::Delivered(_)]));
    drop_unchecked(&path);

    // And the next message numbers past it, though the folder is empty.
    queue(&socket, 3, &token, tell(4, "next"), 2).expect("queued");
    let next = messages(waiting(&socket, 3));
    assert_eq!(next[0].seq, 2);
}

#[test]
fn only_the_names_a_sender_writes_are_read_and_a_dead_part_is_cleared() {
    let (_root, socket, token) = project(3, "tok-three");
    queue(&socket, 3, &token, tell(4, "real"), 1).expect("queued");
    let dir = dir_for(&socket, 3);
    // A name of another shape is not a message: not read, not counted.
    std::fs::write(dir.join("2.json"), b"{}").expect("written");
    std::fs::write(dir.join("../3/0002.json"), b"{}").expect("written");
    assert_eq!(messages(waiting(&socket, 3)).len(), 1);

    // A part of a sender long dead is removed.
    let part = dir.join(name(7, "part"));
    let old = std::fs::File::create(&part).expect("a part");
    old.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 60 * 60))
        .expect("aged");
    drop(old);
    let _ = waiting(&socket, 3);
    assert!(!part.exists());
}

#[test]
fn a_file_past_what_a_message_can_be_is_not_read() {
    let (_root, socket, token) = project(3, "tok-three");
    queue(&socket, 3, &token, tell(4, "x"), 1).expect("queued");
    let path = dir_for(&socket, 3).join(name(1, "json"));
    let mut text = std::fs::read_to_string(&path).expect("the file");
    text.push_str(&" ".repeat(70 * 1024));
    std::fs::write(&path, text).expect("padded");

    assert!(matches!(&waiting(&socket, 3)[..], [Found::Unchecked(_)]));
}

#[test]
fn a_message_still_being_written_is_never_overtaken_by_a_later_one() {
    let (_root, socket, token) = project(3, "tok-three");
    // One sender has taken number 1 and is still writing it; another queues number 2.
    let writing = dir_for(&socket, 3);
    std::fs::create_dir_all(&writing).expect("the folder");
    std::fs::File::create(writing.join(name(1, "part"))).expect("a part");
    queue(&socket, 3, &token, tell(4, "second"), 1).expect("queued");

    // Nothing is handed on past number 1 while it is written.
    assert!(waiting(&socket, 3).is_empty());

    // The first sender finishes: both are handed on, in their order.
    std::fs::remove_file(writing.join(name(1, "part"))).expect("taken back");
    let first = queued_as(&socket, 3, &token, 1, tell(4, "first"));
    assert_eq!(first, 1);
    let found = messages(waiting(&socket, 3));
    assert_eq!(
        found
            .iter()
            .map(|m| (m.seq, m.what.clone()))
            .collect::<Vec<_>>(),
        vec![(1, tell(4, "first")), (2, tell(4, "second"))]
    );
}

#[test]
fn senders_queueing_at_once_each_take_a_number_of_their_own() {
    let (_root, socket, token) = project(3, "tok-three");
    let threads: Vec<_> = (0..8)
        .map(|sender| {
            let (socket, token) = (socket.clone(), token.clone());
            std::thread::spawn(move || {
                for n in 0..10 {
                    queue(&socket, 3, &token, tell(4, &format!("{sender}-{n}")), 1)
                        .expect("queued");
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("a sender");
    }

    let found = messages(waiting(&socket, 3));
    assert_eq!(found.len(), 80);
    let numbers: Vec<u64> = found.iter().map(|m| m.seq).collect();
    assert_eq!(numbers, (1..=80).collect::<Vec<u64>>());
    // Each sender's own messages keep their order.
    for sender in 0..8 {
        let own: Vec<String> = found
            .iter()
            .filter_map(|m| match &m.what {
                Queued::Tell { text, .. } if text.starts_with(&format!("{sender}-")) => {
                    Some(text.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            own,
            (0..10).map(|n| format!("{sender}-{n}")).collect::<Vec<_>>()
        );
    }
}

/// Writes `what` as message `seq` of chat `chat`, signed as [`queue`] signs it: a sender that
/// took that number earlier and finishes now.
fn queued_as(socket: &Path, chat: u32, token: &ChatToken, seq: u64, what: Queued) -> u64 {
    let body = serde_json::to_string(&Body {
        id: "late".to_owned(),
        at: 0,
        what,
    })
    .expect("a body");
    let (key, mac) = sign(token, chat, seq, &body);
    let file = File {
        v: VERSION,
        seq,
        key,
        body,
        mac,
    };
    std::fs::write(
        dir_for(socket, chat).join(name(seq, "json")),
        serde_json::to_vec(&file).expect("a file"),
    )
    .expect("written");
    seq
}

#[test]
fn a_number_a_drain_delivered_while_a_sender_claimed_it_is_passed_over() {
    // A drain delivers another sender's number 2 and raises the floor to it while this sender,
    // which read the floor as 0, claims 2: it lets 2 go and takes 3.
    let folder = tempfile::tempdir().expect("a folder");
    let floor = std::cell::Cell::new(0);
    let looks = std::cell::Cell::new(0);
    let (seq, _part) = claim(folder.path(), 1, &|| {
        looks.set(looks.get() + 1);
        // The first look is before the claim; by the second, the drain has delivered 2.
        if looks.get() > 1 {
            floor.set(2);
        }
        floor.get()
    })
    .expect("claimed");

    assert_eq!(seq, 3);
    assert!(!folder.path().join(name(2, "part")).exists(), "2 let go of");
}

#[test]
fn a_part_that_could_not_be_written_is_removed() {
    let folder = tempfile::tempdir().expect("a folder");
    let (seq, part) = claim(folder.path(), 0, &|| 0).expect("claimed");
    // The whole name is there already: the link fails, and the part does not stay behind.
    std::fs::write(folder.path().join(name(seq, "json")), b"x").expect("in the way");
    let file = File {
        v: VERSION,
        seq,
        key: String::new(),
        body: String::new(),
        mac: String::new(),
    };

    assert!(written(folder.path(), seq, part, &file).is_err());
    assert!(!folder.path().join(name(seq, "part")).exists());
}

#[test]
fn a_chat_started_again_in_its_place_keeps_what_it_queued_in_order() {
    let (_root, socket, token) = project(3, "tok-three");
    crate::hookwire::spool::remember(&spool_dir(&socket), 7, &ChatToken::from("tok-seven"))
        .expect("the new run's key");
    queue(&socket, 3, &token, tell(4, "one"), 1).expect("queued");
    queue(&socket, 3, &token, tell(4, "two"), 2).expect("queued");

    assert_eq!(moved(&socket, 3, 7), 2);

    assert!(!holds(&socket, 3));
    let found = messages(waiting(&socket, 7));
    assert_eq!(
        found.iter().map(|m| m.what.clone()).collect::<Vec<_>>(),
        vec![tell(4, "one"), tell(4, "two")]
    );
}

/// #3914: Claude Code runs purlis's MCP server outside the sandbox it confines a command in, so
/// its `dispatch_*` tools queue where the command cannot; Codex and opencode run the server
/// inside purlis's wrap, so neither does there, and nothing points a chat to them for it.
#[test]
fn only_a_harness_that_runs_purlis_s_tools_outside_its_sandbox_has_tools_that_queue() {
    assert_eq!(tool_path(Some("claude-code")), ToolPath::Queues);
    assert_eq!(
        tool_path(None),
        ToolPath::Queues,
        "a chat that names no harness is Claude Code's"
    );
    assert_eq!(tool_path(Some("codex")), ToolPath::KeptOut);
    assert_eq!(tool_path(Some("opencode")), ToolPath::KeptOut);
    assert_eq!(
        tool_path(Some("a-harness-purlis-does-not-know")),
        ToolPath::KeptOut
    );
}

/// A chat's token, made at random as the app makes one: no test token is written out.
fn a_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// #3958: a sender that does not come back after a restart has its queue cleared as the project
/// opens ([`crate::hookwire::spool::forget_all_but`]), and what it queued is reported first, each
/// message with whom it was for, read from its own file while its key still checks it. A chat
/// that comes back keeps its queue, and nothing of it is reported.
#[test]
fn what_a_sender_that_never_came_back_queued_is_reported_before_its_queue_is_cleared() {
    let (_root, socket, token) = project(3, &a_token());
    let back = ChatToken::from(a_token().as_str());
    crate::hookwire::spool::remember(&spool_dir(&socket), 5, &back).expect("the key");
    let told = queue(&socket, 3, &token, tell(4, "one"), 10).expect("queued");
    let asked = queue(
        &socket,
        3,
        &token,
        Queued::Question {
            text: "two?".into(),
        },
        11,
    )
    .expect("queued");
    queue(&socket, 5, &back, tell(6, "kept"), 12).expect("queued");
    // A file that does not check is no message of the sender's: removed, never reported.
    std::fs::write(dir_for(&socket, 3).join(name(99, "json")), b"{}").expect("a stray");

    crate::hookwire::spool::forget_all_but(&spool_dir(&socket), &[5]).expect("forgotten");

    assert!(
        !dir_for(&socket, 3).exists(),
        "the sender's queue is cleared"
    );
    assert!(holds(&socket, 5), "a chat that came back keeps its queue");
    let reports = left(&socket);
    assert_eq!(
        reports.len(),
        1,
        "one report, of the sender that did not come back"
    );
    let (_, report) = &reports[0];
    assert_eq!(report.sender, 3);
    assert_eq!(
        report
            .messages
            .iter()
            .map(|m| (m.id.clone(), m.what.clone()))
            .collect::<Vec<_>>(),
        vec![
            (told, tell(4, "one")),
            (
                asked,
                Queued::Question {
                    text: "two?".into()
                }
            ),
        ]
    );
}

/// A report is removed once it is recorded where the person reads it ([`reported`]), and a
/// sender with nothing that checks left behind leaves no report.
#[test]
fn a_report_goes_once_recorded_and_an_empty_queue_leaves_none() {
    let (_root, socket, token) = project(3, &a_token());
    queue(&socket, 3, &token, tell(4, "one"), 10).expect("queued");
    let (_other, quiet, _) = project(7, &a_token());
    crate::hookwire::spool::forget_all_but(&spool_dir(&quiet), &[]).expect("forgotten");
    assert!(left(&quiet).is_empty());

    crate::hookwire::spool::forget_all_but(&spool_dir(&socket), &[]).expect("forgotten");
    let reports = left(&socket);
    assert_eq!(reports.len(), 1);
    reported(&reports[0].0);
    assert!(left(&socket).is_empty());
}
