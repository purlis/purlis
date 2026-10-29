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
            Found::Unchecked(path) => panic!("{} did not check", path.display()),
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
    delivered(&waiting[0]);
    let left = messages(super::waiting(&socket, 3));
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].what, Queued::TellAll { text: "two".into() });
    delivered(&left[0]);
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
    let path = dir_for(&socket, 3).join(format!("{:020}.json", 1));
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
