// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A connector held as an argument vector (ADR 0078 §2, #2044): each connection is a new run of
//! the command, its standard input and output the byte stream; and nothing runs when the check
//! made before each connection refuses.

#![cfg(unix)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use purlis_host::CommandConnector;
use purlis_session_protocol::hostlink::Connector;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn each_connection_is_a_new_run_of_the_command_on_its_standard_input_and_output() {
    let connector = CommandConnector::new("/bin/cat", Vec::new(), Arc::new(|| Ok(())));
    for word in ["first", "second"] {
        let mut io = connector.connect().await.expect("connected");
        io.write_all(word.as_bytes()).await.unwrap();
        io.shutdown().await.unwrap();
        let mut echoed = String::new();
        io.read_to_string(&mut echoed).await.unwrap();
        assert_eq!(echoed, word);
    }
}

#[tokio::test]
async fn nothing_runs_when_the_check_before_a_connection_refuses() {
    let runs = Arc::new(AtomicUsize::new(0));
    let marker = tempfile::tempdir().unwrap();
    let ran = marker.path().join("ran");
    let connector =
        CommandConnector::new("/usr/bin/touch", vec![ran.to_str().unwrap().to_owned()], {
            let runs = Arc::clone(&runs);
            Arc::new(move || {
                runs.fetch_add(1, Ordering::SeqCst);
                Err("runner first's container is privileged".to_owned())
            })
        });
    let refused = connector.connect().await.err().expect("refused");
    assert!(refused.to_string().contains("privileged"), "{refused}");
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    assert!(!ran.exists());
}

#[tokio::test]
async fn the_commands_arguments_are_its_own_words_never_a_shell_string() {
    let connector = CommandConnector::new(
        "/bin/echo",
        vec!["$HOME; echo nope".to_owned()],
        Arc::new(|| Ok(())),
    );
    let mut io = connector.connect().await.unwrap();
    let mut said = String::new();
    io.read_to_string(&mut said).await.unwrap();
    assert_eq!(said, "$HOME; echo nope\n");
}

#[tokio::test]
async fn a_check_refusing_the_far_ends_key_is_told_as_a_changed_key_and_runs_nothing() {
    use purlis_host::NotChecked;
    use purlis_session_protocol::hostlink::{
        self, Handshake, Identity, LinkKey, Linked, Liveness, Pinned,
    };
    let marker = tempfile::tempdir().unwrap();
    let ran = marker.path().join("ran");
    let connector = CommandConnector::checked(
        "/usr/bin/touch",
        vec![ran.to_str().unwrap().to_owned()],
        Arc::new(|| {
            Err(NotChecked::KeyChanged(
                "the cluster operator vouches for another key".to_owned(),
            ))
        }),
    );
    let mut links = hostlink::keep_linked(
        connector,
        Handshake::THIS_BUILD,
        Identity {
            device: "01JA0DESKT0P00000000000000".into(),
            key: LinkKey::generate().unwrap(),
        },
        Pinned {
            device: "01JB0RUNNER000000000000000".into(),
            key: LinkKey::generate().unwrap().public(),
        },
        Liveness::default(),
    );
    match links.next().await {
        Some(Linked::Down(down)) => {
            assert!(down.key_changed, "{down:?}");
            assert!(down.why.contains("another key"), "{down:?}");
        }
        _ => panic!("the link came up"),
    }
    assert!(!ran.exists());
}
