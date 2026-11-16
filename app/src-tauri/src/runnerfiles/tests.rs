// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_core::panelinks::RunnerFiles as _;
use purlis_session_protocol::session::{FilePiece, FileReach, FileRead, MOST_FILE_PIECE};

use super::{AsksRunner, ReadThere, ReadsOnRunner};

type Asked = (String, FileReach, Option<u64>);

/// A runner whose file is `bytes`, at `version` (which may move after each piece), and who
/// records what it was asked.
#[derive(Clone)]
pub(super) struct Fake {
    pub(super) bytes: Vec<u8>,
    versions: Arc<Mutex<Vec<&'static str>>>,
    asked: Arc<Mutex<Vec<Asked>>>,
    pub(super) answer: Option<FileRead>,
}

pub(super) fn fake(bytes: &[u8]) -> Fake {
    Fake {
        bytes: bytes.to_vec(),
        versions: Arc::new(Mutex::new(vec!["v1"])),
        asked: Arc::default(),
        answer: None,
    }
}

impl AsksRunner for Fake {
    fn ask(
        &self,
        path: &str,
        reach: FileReach,
        at: Option<u64>,
        _within: Duration,
    ) -> Result<FileRead, String> {
        self.asked
            .lock()
            .unwrap()
            .push((path.to_owned(), reach, at));
        if let Some(answer) = &self.answer {
            return Ok(answer.clone());
        }
        let version = {
            let mut versions = self.versions.lock().unwrap();
            if versions.len() > 1 {
                versions.remove(0)
            } else {
                versions[0]
            }
        };
        let size = self.bytes.len() as u64;
        let piece = at.map(|at| {
            let from = at as usize;
            let to = (at + MOST_FILE_PIECE).min(size) as usize;
            FilePiece {
                at,
                bytes: self.bytes[from..to].to_vec(),
            }
        });
        Ok(FileRead::Plain {
            path: format!("/w/{path}"),
            from_workspace: Some(path.to_owned()),
            size,
            version: version.to_owned(),
            piece,
        })
    }
}

fn reads(runner: &Fake) -> ReadsOnRunner {
    ReadsOnRunner::new("pool/runner-1".into(), Box::new(runner.clone()))
}

#[test]
fn a_file_longer_than_a_piece_is_read_whole_in_pieces() {
    let text = "a".repeat(MOST_FILE_PIECE as usize) + "tail\n";
    let runner = fake(text.as_bytes());

    let read = reads(&runner).read("big.txt", FileReach::Workspace);

    assert_eq!(read, Ok(ReadThere::Text(text)));
    let asked = runner.asked.lock().unwrap();
    assert_eq!(
        *asked,
        vec![
            ("big.txt".to_owned(), FileReach::Workspace, Some(0)),
            (
                "big.txt".to_owned(),
                FileReach::Workspace,
                Some(MOST_FILE_PIECE)
            ),
        ]
    );
}

#[test]
fn an_empty_file_is_read_as_no_text() {
    let runner = fake(b"");

    assert_eq!(
        reads(&runner).read("empty", FileReach::Workspace),
        Ok(ReadThere::Text(String::new()))
    );
}

#[test]
fn a_file_that_changed_between_pieces_is_read_again_from_its_start() {
    let text = "b".repeat(MOST_FILE_PIECE as usize + 3);
    let runner = fake(text.as_bytes());
    *runner.versions.lock().unwrap() = vec!["v1", "v2", "v2"];

    assert_eq!(
        reads(&runner).read("moving.txt", FileReach::Workspace),
        Ok(ReadThere::Text(text))
    );
    assert_eq!(runner.asked.lock().unwrap().len(), 4);
}

#[test]
fn a_file_that_keeps_changing_is_given_up_saying_so() {
    let text = "c".repeat(MOST_FILE_PIECE as usize + 3);
    let runner = fake(text.as_bytes());
    *runner.versions.lock().unwrap() = vec!["v1", "v2", "v3", "v4", "v5"];

    let said = reads(&runner)
        .read("moving.txt", FileReach::Workspace)
        .unwrap_err();
    assert!(said.contains("kept changing"), "{said}");
    assert!(said.contains("pool/runner-1"), "{said}");
}

#[test]
fn a_binary_or_too_large_file_is_said_by_its_size() {
    let mut runner = fake(b"");
    runner.answer = Some(FileRead::Binary {
        path: "/w/a.png".into(),
        size: 40,
    });
    assert_eq!(
        reads(&runner).read("a.png", FileReach::Workspace),
        Ok(ReadThere::Binary(40))
    );
    runner.answer = Some(FileRead::TooLarge {
        path: "/w/a.log".into(),
        size: 5 << 20,
    });
    assert_eq!(
        reads(&runner).read("a.log", FileReach::Workspace),
        Ok(ReadThere::TooLarge(5 << 20))
    );
}

#[test]
fn a_file_now_outside_the_workspace_is_not_shown_and_says_to_open_it_again() {
    let mut runner = fake(b"");
    runner.answer = Some(FileRead::Outside {
        path: "/etc/hosts".into(),
    });

    let said = reads(&runner)
        .read("hosts", FileReach::Workspace)
        .unwrap_err();
    assert!(
        said.contains("outside the chat's workspace on runner pool/runner-1"),
        "{said}"
    );
}

#[test]
fn pieces_that_run_past_the_files_size_are_refused() {
    let mut runner = fake(b"");
    runner.answer = Some(FileRead::Plain {
        path: "/w/a".into(),
        from_workspace: Some("a".into()),
        size: 2,
        version: "v1".into(),
        piece: Some(FilePiece {
            at: 0,
            bytes: b"abc".to_vec(),
        }),
    });

    assert!(reads(&runner).read("a", FileReach::Workspace).is_err());
}

#[test]
fn a_hover_only_looks_and_may_learn_of_a_file_outside_the_workspace() {
    let runner = fake(b"x\n");

    let looked = reads(&runner).look("src/main.rs");

    assert!(
        matches!(looked, Some(FileRead::Plain { piece: None, .. })),
        "{looked:?}"
    );
    assert_eq!(
        *runner.asked.lock().unwrap(),
        vec![("src/main.rs".to_owned(), FileReach::Anywhere, None)]
    );
}

mod in_a_pane {
    use std::path::PathBuf;

    use purlis_core::panelinks::{
        Asks, Candidate, Kind, LocalFile, Pane, Resolver, RunnerFile, Runs,
    };
    use purlis_session_protocol::session::FileRead;

    use super::{Fake, fake};
    use crate::pane_links::{LinkAsk, LinkFileAt, LinkOpened, answer};
    use crate::runnerfiles::ReadsOnRunner;

    fn on_runner() -> Pane {
        Pane {
            project: Some(PathBuf::from("/planes/one")),
            session: Some(7),
            folder: None,
            runs: Runs::OnRunner,
        }
    }

    fn links(runner: &Fake) -> Resolver {
        Resolver::with(vec![
            Box::new(LocalFile::with(None, Vec::new())),
            Box::new(RunnerFile::new(Box::new(ReadsOnRunner::new(
                "pool/runner-1".into(),
                Box::new(runner.clone()),
            )))),
        ])
    }

    fn file(text: &str) -> Candidate {
        Candidate {
            kind: Kind::File,
            text: text.to_owned(),
        }
    }

    fn opened(runner: &Fake, asks: &Asks, text: &str, token: Option<&str>) -> LinkOpened {
        answer(
            &links(runner),
            &on_runner(),
            asks,
            &file(text),
            token,
            |_| Err("never a browser".into()),
        )
    }

    #[test]
    fn a_runner_chats_file_in_its_workspace_opens_in_the_runner_tab_at_its_line() {
        let runner = fake(b"fn main() {}\n");

        assert_eq!(
            opened(&runner, &Asks::default(), "src/main.rs:3:2", None),
            LinkOpened::File {
                target: "/w/src/main.rs:3:2".into(),
                at: LinkFileAt::Runner {
                    runner: "pool/runner-1".into(),
                    path: "/w/src/main.rs".into(),
                },
                line: Some(3),
                column: Some(2),
            }
        );
    }

    #[test]
    fn a_runner_chats_file_outside_its_workspace_asks_naming_the_runner_then_opens_once() {
        let mut runner = fake(b"");
        runner.answer = Some(FileRead::Plain {
            path: "/etc/hosts".into(),
            from_workspace: None,
            size: 1,
            version: "v".into(),
            piece: None,
        });
        let asks = Asks::default();

        let LinkOpened::Ask { about, token, .. } = opened(&runner, &asks, "/etc/hosts", None)
        else {
            panic!("expected an ask");
        };
        assert_eq!(
            about,
            LinkAsk::FileOutside {
                home: None,
                runner: Some("pool/runner-1".into())
            }
        );
        assert!(matches!(
            opened(&runner, &asks, "/etc/hosts", Some(&token)),
            LinkOpened::File {
                at: LinkFileAt::Runner { .. },
                ..
            }
        ));
        assert!(
            asks.opened_on_runner(&on_runner(), "pool/runner-1", "/etc/hosts")
                .is_some(),
            "its tab may read it on the runner now"
        );
    }
}
