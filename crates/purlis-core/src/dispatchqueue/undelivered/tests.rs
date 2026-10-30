// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::dispatchqueue::{dir_for, holds, queue, spool_dir};
use crate::hookwire::ChatToken;
use crate::hookwire::spool::forget_all_but;

/// A chat's token, made at random as the app makes one: no test token is written out.
fn a_token() -> ChatToken {
    ChatToken::from(uuid::Uuid::new_v4().simple().to_string().as_str())
}

/// A project's hook socket, and the token chat `chat` was issued there, its key remembered as
/// the app remembers it.
fn project(chat: u32) -> (tempfile::TempDir, PathBuf, ChatToken) {
    let root = tempfile::tempdir().expect("a folder");
    let app = root.path().join(".purlis/app");
    std::fs::create_dir_all(&app).expect("the app folder");
    let socket = app.join("hooks.sock");
    let token = a_token();
    crate::hookwire::spool::remember(&spool_dir(&socket), chat, &token).expect("the key");
    (root, socket, token)
}

fn tell(to: u32, text: &str) -> Queued {
    Queued::Tell {
        to,
        text: text.to_owned(),
    }
}

fn whats(report: &Report) -> Vec<Queued> {
    report.messages.iter().map(|m| m.what.clone()).collect()
}

/// A forge token's shape, written in two halves so no whole one is in this file.
fn a_secret() -> String {
    format!("{}{}", "ghp_", "0123456789abcdefghijABCDEFGHIJ012345")
}

/// A sender that does not come back after a restart has its queue cleared as the project opens
/// ([`forget_all_but`]), and what it queued is set aside first, each message with whom it was
/// for, read from its own file while its key still checks it. A chat that comes back keeps its
/// queue, and nothing of it is set aside.
#[test]
fn what_a_sender_that_never_came_back_queued_is_set_aside_before_its_queue_is_cleared() {
    let (_root, socket, token) = project(3);
    let back = a_token();
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
    std::fs::write(dir_for(&socket, 3).join(format!("{:020}.json", 99)), b"{}").expect("a stray");

    forget_all_but(&spool_dir(&socket), &[5]).expect("forgotten");

    assert!(
        !dir_for(&socket, 3).exists(),
        "the sender's queue is cleared"
    );
    assert!(holds(&socket, 5), "a chat that came back keeps its queue");
    let reports = to_place(&socket);
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

/// A report goes once it is placed, a sender with nothing that checks leaves none, and a notice
/// goes once it is put away, by its own name only.
#[test]
fn a_report_goes_once_placed_and_a_notice_once_put_away() {
    let (_root, socket, token) = project(3);
    queue(&socket, 3, &token, tell(4, "one"), 10).expect("queued");
    let (_other, quiet, _) = project(7);
    forget_all_but(&spool_dir(&quiet), &[]).expect("forgotten");
    assert!(to_place(&quiet).is_empty());

    forget_all_but(&spool_dir(&socket), &[]).expect("forgotten");
    let reports = to_place(&socket);
    assert_eq!(reports.len(), 1);
    let (path, report) = &reports[0];
    keep_as_notice(
        path,
        report,
        report.messages.clone(),
        Some("steward 1".into()),
    );
    assert!(to_place(&socket).is_empty(), "the report became the notice");
    let notices = notices(&socket);
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].1.name.as_deref(), Some("steward 1"));
    assert!(put_away(&socket, "../records.json").is_err());
    put_away(&socket, &notices[0].0).expect("put away");
    assert!(super::notices(&socket).is_empty());
}

/// **A message whose text holds a secret keeps its line, not its words** (#1520, #1872): its
/// report holds none of it, and the notice says it was left out.
#[test]
fn a_secret_in_a_message_is_never_kept_in_a_report_or_shown() {
    let (_root, socket, token) = project(3);
    let secret = a_secret();
    queue(
        &socket,
        3,
        &token,
        tell(4, &format!("use {secret} to sign in")),
        10,
    )
    .expect("queued");
    queue(&socket, 3, &token, tell(4, "plain words"), 11).expect("queued");

    forget_all_but(&spool_dir(&socket), &[]).expect("forgotten");

    let reports = to_place(&socket);
    let (path, report) = &reports[0];
    let bytes = std::fs::read_to_string(path).expect("the report");
    assert!(!bytes.contains(&secret), "{bytes}");
    assert!(!bytes.contains("sign in"), "{bytes}");
    assert!(report.messages[0].secret);
    assert!(!report.messages[1].secret);
    let notice = report.notice();
    assert!(!notice.contains(&secret), "{notice}");
    assert!(
        notice.contains("a follow-up to chat 4: (its text held a secret, so it is not kept)"),
        "{notice}"
    );
    assert!(
        notice.contains("a follow-up to chat 4: \"plain words\""),
        "{notice}"
    );
}

/// **A queue that could not be set aside is kept, with its keys**, for the next open: nothing it
/// held is cleared unseen.
#[test]
fn a_queue_that_could_not_be_set_aside_is_kept_for_the_next_open() {
    let (_root, socket, token) = project(3);
    queue(&socket, 3, &token, tell(4, "one"), 10).expect("queued");
    // Where the reports go is a file, so none can be written there.
    let blocked = spool_dir(&socket).join(super::super::DIR).join(DIR);
    std::fs::write(&blocked, b"").expect("in the way");

    forget_all_but(&spool_dir(&socket), &[]).expect("forgotten");

    assert!(holds(&socket, 3), "its queue is kept");
    assert!(
        matches!(
            crate::dispatchqueue::waiting(&socket, 3).first(),
            Some(Found::Message(_))
        ),
        "and its keys, so it still checks"
    );

    std::fs::remove_file(&blocked).expect("out of the way");
    forget_all_but(&spool_dir(&socket), &[]).expect("forgotten");
    assert!(!holds(&socket, 3));
    assert_eq!(to_place(&socket).len(), 1, "set aside at the next open");
}

/// **A report beside a socket outside a project's state folder is none**: the chat sandbox's
/// integrity denial does not cover it, so anything could have written it.
#[test]
fn a_report_outside_the_integrity_denial_is_never_read() {
    let root = tempfile::tempdir().expect("a folder");
    let socket = root.path().join("hooks.sock");
    let folder = spool_dir(&socket).join(super::super::DIR).join(DIR);
    std::fs::create_dir_all(&folder).expect("a folder");
    let forged = serde_json::json!({
        "v": 1, "sender": 3, "at": 0,
        "messages": [{ "id": "a", "at": 0, "what": "tell", "to": 4, "text": "forged" }],
    })
    .to_string();
    std::fs::write(folder.join("3.a.json"), &forged).expect("written");
    std::fs::write(folder.join("3.b.notice.json"), &forged).expect("written");

    assert!(to_place(&socket).is_empty());
    assert!(notices(&socket).is_empty());
    assert!(put_away(&socket, "3.b.notice.json").is_err());
    assert!(folder.join("3.b.notice.json").exists());
}

/// **An app that stopped after setting a queue aside, and before clearing it**, sets it aside
/// again at the next open: no message is in two reports.
#[test]
fn a_queue_set_aside_twice_is_reported_once() {
    let (_root, socket, token) = project(3);
    queue(&socket, 3, &token, tell(4, "one"), 10).expect("queued");

    assert_eq!(set_aside(&spool_dir(&socket), 3, 20).expect("set aside"), 1);
    assert_eq!(set_aside(&spool_dir(&socket), 3, 21).expect("set aside"), 0);
    forget_all_but(&spool_dir(&socket), &[]).expect("forgotten");

    let reports = to_place(&socket);
    assert_eq!(reports.len(), 1);
    assert_eq!(whats(&reports[0].1), vec![tell(4, "one")]);
}

/// **An app that stopped after keeping a notice, and before removing its report**, keeps no
/// second notice of the same messages.
#[test]
fn a_report_kept_as_a_notice_twice_is_one_notice() {
    let (_root, socket, token) = project(3);
    queue(&socket, 3, &token, tell(4, "one"), 10).expect("queued");
    forget_all_but(&spool_dir(&socket), &[]).expect("forgotten");
    let (path, report) = to_place(&socket).remove(0);
    let bytes = std::fs::read(&path).expect("the report");

    keep_as_notice(&path, &report, report.messages.clone(), None);
    // The report as the app that stopped left it.
    std::fs::write(&path, &bytes).expect("left behind");
    keep_as_notice(&path, &report, report.messages.clone(), None);

    assert_eq!(notices(&socket).len(), 1);
    assert!(to_place(&socket).is_empty());
}

/// What the notice says: the sender by name and number, how many, and each message's line,
/// an acknowledgement alone among them.
#[test]
fn a_notice_names_the_sender_how_many_and_what_each_said() {
    let report: Report = serde_json::from_value(serde_json::json!({
        "v": 1,
        "sender": 3,
        "name": "steward 1",
        "at": 0,
        "messages": [
            { "id": "a", "at": 0, "what": "tell", "to": 4, "text": "carry on" },
            { "id": "b", "at": 0, "what": "question", "text": "which\nqueue?" },
            { "id": "c", "at": 0, "what": "note", "ack": 2 },
        ],
    }))
    .expect("a report");
    assert_eq!(
        report.notice(),
        "steward 1 (chat 3) did not come back after purlis restarted, and 3 messages it queued \
         while purlis did not answer were never delivered, with no running dispatch left to \
         keep them on: a follow-up to chat 4: \"carry on\"; a question to its asking chat: \
         \"which queue?\"; a progress note to its asking chat acknowledging instruction 2"
    );
}
