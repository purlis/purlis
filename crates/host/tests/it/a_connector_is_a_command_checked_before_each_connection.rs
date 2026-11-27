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

/// **A runner removed outside purlis is not linked to again** (#4149): its check finds Docker
/// lists no container of its id, so the kept link says it is down as gone, once, and ends,
/// never trying the check again as it would for a runner that is only away.
#[tokio::test]
async fn a_check_finding_the_runner_gone_is_told_once_and_never_tried_again() {
    use purlis_host::NotChecked;
    use purlis_session_protocol::hostlink::{
        self, Handshake, Identity, LinkKey, Linked, Liveness, Pinned,
    };
    let checks = Arc::new(AtomicUsize::new(0));
    let connector = CommandConnector::checked("/bin/cat", Vec::new(), {
        let checks = Arc::clone(&checks);
        Arc::new(move || {
            checks.fetch_add(1, Ordering::SeqCst);
            Err(NotChecked::Gone(
                "runner first was removed outside purlis".to_owned(),
            ))
        })
    });
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
            assert!(down.gone, "{down:?}");
            assert!(!down.key_changed, "{down:?}");
            assert!(down.why.contains("removed outside purlis"), "{down:?}");
        }
        _ => panic!("the link came up"),
    }
    // Seen red: before #4149 the link was tried again after each pause, forever.
    let next = tokio::time::timeout(std::time::Duration::from_secs(2), links.next()).await;
    assert!(matches!(next, Ok(None)), "the kept link went on");
    assert_eq!(checks.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_check_that_outlasts_the_connectors_wait_is_refused_in_time_and_says_so() {
    // A Docker runner's check runs `docker` over SSH several times: after its machine restarts,
    // one can hang. The link says why it is down rather than the desktop's own wait running out
    // in the middle of the handshake that follows.
    let marker = tempfile::tempdir().unwrap();
    let ran = marker.path().join("ran");
    let connector = CommandConnector::new(
        "/usr/bin/touch",
        vec![ran.to_str().unwrap().to_owned()],
        Arc::new(|| {
            std::thread::sleep(std::time::Duration::from_secs(3));
            Ok(())
        }),
    )
    .within(std::time::Duration::from_millis(200));
    let started = std::time::Instant::now();
    let refused = connector.connect().await.err().expect("refused in time");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert!(
        refused.to_string().contains("did not finish within"),
        "{refused}"
    );
    assert!(!ran.exists(), "nothing ran");
}

/// **A link made inside a step names it, and so does what its check runs** (#7182): the rows of
/// the start's table with no parent are the ones to sum, so a link's `link_check` inside the
/// caller's `first_link`, and the container's check inside the link's, say so.
#[tokio::test]
async fn a_links_check_and_what_it_runs_name_the_steps_they_are_inside() {
    use purlis_host::NotChecked;
    use purlis_session_protocol::hostlink::{
        self, Handshake, Identity, LinkKey, Linked, Liveness, Pinned,
    };
    use purlis_session_protocol::steps;

    #[derive(Clone, Default)]
    struct Said(Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for Said {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Said {
        type Writer = Said;
        fn make_writer(&'a self) -> Said {
            self.clone()
        }
    }
    // The binary's one subscriber, set once: a scoped one is not seen by a blocking thread, and
    // `tracing` caches whether a step is wanted, which other tests running beside this one with
    // none would starve. Other tests' lines land in it too, so each check below is "some line".
    static SAID: std::sync::OnceLock<Said> = std::sync::OnceLock::new();
    let said = SAID
        .get_or_init(|| {
            let said = Said::default();
            let _ = tracing::subscriber::set_global_default(
                tracing_subscriber::fmt()
                    .with_writer(said.clone())
                    .with_ansi(false)
                    .with_max_level(tracing::Level::INFO)
                    .finish(),
            );
            said
        })
        .clone();
    let connector = CommandConnector::checked(
        "/bin/cat",
        Vec::new(),
        Arc::new(|| {
            steps::begin(steps::Name::ContainerCheck).done();
            Err(NotChecked::Refused("no".to_owned()))
        }),
    );
    let first = steps::begin(steps::Name::FirstLink);
    let mut links = first.inside(|| {
        hostlink::keep_linked(
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
        )
    });
    assert!(matches!(links.next().await, Some(Linked::Down(_))));
    first.done();
    let out = String::from_utf8(said.0.lock().unwrap().clone()).unwrap();
    let some = |name: &str, inside: &str| {
        out.lines().any(|line| {
            line.contains(&format!("purlis: step {name} "))
                && line.contains(&format!("(parent: {inside})"))
        })
    };
    assert!(some("container_check", "link_check"), "{out}");
    assert!(some("link_check", "first_link"), "{out}");
    assert!(
        out.lines().any(|line| line.contains("purlis: step first_link done")
            && !line.contains("(parent:")),
        "{out}"
    );
}
