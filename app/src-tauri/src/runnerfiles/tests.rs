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
            home: None,
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

    let read = reads(&runner).read("big.txt", None);

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
        reads(&runner).read("empty", None),
        Ok(ReadThere::Text(String::new()))
    );
}

#[test]
fn a_file_that_changed_between_pieces_is_read_again_from_its_start() {
    let text = "b".repeat(MOST_FILE_PIECE as usize + 3);
    let runner = fake(text.as_bytes());
    *runner.versions.lock().unwrap() = vec!["v1", "v2", "v2"];

    assert_eq!(
        reads(&runner).read("moving.txt", None),
        Ok(ReadThere::Text(text))
    );
    assert_eq!(runner.asked.lock().unwrap().len(), 4);
}

#[test]
fn a_file_that_keeps_changing_is_given_up_saying_so() {
    let text = "c".repeat(MOST_FILE_PIECE as usize + 3);
    let runner = fake(text.as_bytes());
    *runner.versions.lock().unwrap() = vec!["v1", "v2", "v3", "v4", "v5"];

    let said = reads(&runner).read("moving.txt", None).unwrap_err();
    assert!(said.starts_with("The file kept changing"), "{said}");
    assert!(said.ends_with("Open it again in a moment."), "{said}");
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
        reads(&runner).read("a.png", None),
        Ok(ReadThere::Binary(40))
    );
    runner.answer = Some(FileRead::TooLarge {
        path: "/w/a.log".into(),
        size: 5 << 20,
    });
    assert_eq!(
        reads(&runner).read("a.log", None),
        Ok(ReadThere::TooLarge(5 << 20))
    );
}

#[test]
fn a_file_now_outside_the_workspace_is_not_shown_and_says_to_open_it_again() {
    let mut runner = fake(b"");
    runner.answer = Some(FileRead::Outside {
        path: "/etc/hosts".into(),
    });

    let said = reads(&runner).read("hosts", None).unwrap_err();
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
        home: None,
        size: 2,
        version: "v1".into(),
        piece: Some(FilePiece {
            at: 0,
            bytes: b"abc".to_vec(),
        }),
    });

    assert!(reads(&runner).read("a", None).is_err());
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

#[test]
fn a_file_opened_on_an_answer_is_read_anywhere_while_it_is_still_that_file() {
    let mut runner = fake(b"");
    runner.answer = Some(FileRead::Plain {
        path: "/etc/hosts".into(),
        from_workspace: None,
        home: None,
        size: 3,
        version: "8:42:3:1.0".into(),
        piece: Some(FilePiece {
            at: 0,
            bytes: b"ok\n".to_vec(),
        }),
    });

    assert_eq!(
        reads(&runner).read("/etc/hosts", Some("8:42")),
        Ok(ReadThere::Text("ok\n".into()))
    );
    assert_eq!(runner.asked.lock().unwrap()[0].1, FileReach::Anywhere);
}

#[test]
fn a_file_opened_on_an_answer_and_since_replaced_is_not_read_and_says_to_open_it_again() {
    let replaced = |path: &str, version: &str| FileRead::Plain {
        path: path.into(),
        from_workspace: None,
        home: None,
        size: 3,
        version: version.into(),
        piece: Some(FilePiece {
            at: 0,
            bytes: b"no\n".to_vec(),
        }),
    };
    for (why, answer) in [
        (
            "another file in its place",
            replaced("/etc/hosts", "8:43:3:1.0"),
        ),
        (
            "a link to another file",
            replaced("/etc/shadow", "8:42:3:1.0"),
        ),
    ] {
        let mut runner = fake(b"");
        runner.answer = Some(answer);

        let said = reads(&runner)
            .read("/etc/hosts", Some("8:42"))
            .expect_err(why);
        assert!(
            said.starts_with("This file changed on runner pool/runner-1"),
            "{why}: {said}"
        );
        assert!(
            said.contains("Open it from its link in the chat again"),
            "{why}: {said}"
        );
    }
}

/// The runner's own read, over files on this machine (`onrunner::read_for_window`).
#[cfg(unix)]
mod over_real_files {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use purlis_core::panelinks::RunnerFiles as _;
    use purlis_core::panelinks::onrunner::{Asked, identity_of, read_for_window};
    use purlis_session_protocol::session::{FileReach, FileRead};

    use super::super::{AsksRunner, ReadThere, ReadsOnRunner};

    struct Here {
        project: PathBuf,
    }

    impl AsksRunner for Here {
        fn ask(
            &self,
            path: &str,
            reach: FileReach,
            at: Option<u64>,
            _within: Duration,
        ) -> Result<FileRead, String> {
            Ok(read_for_window(&Asked {
                project: &self.project,
                cwd: Some(&self.project),
                path,
                reach,
                at,
            }))
        }
    }

    fn identity(reads: &ReadsOnRunner, path: &Path) -> String {
        match reads.look(path.to_str().expect("text")) {
            Some(FileRead::Plain { version, .. }) => {
                identity_of(&version).expect("an identity").to_owned()
            }
            other => panic!("expected a plain file, got {other:?}"),
        }
    }

    #[test]
    fn a_file_opened_on_an_answer_then_swapped_for_a_link_to_another_outside_file_is_not_read() {
        let here = tempfile::tempdir().expect("a folder");
        let root = std::fs::canonicalize(here.path()).expect("resolved");
        let project = root.join("project");
        std::fs::create_dir(&project).expect("a project");
        let approved = root.join("notes.txt");
        let other = root.join("other.txt");
        std::fs::write(&approved, "approved\n").expect("written");
        std::fs::write(&other, "other\n").expect("written");
        let reads = ReadsOnRunner::new(
            "pool/runner-1".into(),
            Box::new(Here {
                project: project.clone(),
            }),
        );
        let opened = identity(&reads, &approved);
        let path = approved.to_str().expect("text");
        assert_eq!(
            reads.read(path, Some(&opened)),
            Ok(ReadThere::Text("approved\n".into()))
        );

        std::fs::remove_file(&approved).expect("removed");
        std::os::unix::fs::symlink(&other, &approved).expect("a link");

        let said = reads
            .read(path, Some(&opened))
            .expect_err("a link put in its place is not followed on the answer given for it");
        assert!(said.contains("changed"), "{said}");

        // The file it was is kept under another name, so the new one is never given its number.
        std::fs::remove_file(&approved).expect("removed");
        std::fs::write(&other, "approved\n").expect("written");
        std::fs::rename(&other, &approved).expect("put in its place");
        assert!(
            reads.read(path, Some(&opened)).is_err(),
            "a new file with the same name is another file"
        );
    }
}

mod in_a_pane {
    use std::path::PathBuf;

    use purlis_core::panelinks::{
        Asks, Candidate, Kind, LocalFile, Pane, Resolver, RunnerFile, Runs,
    };
    use purlis_session_protocol::session::{FileHome, FileRead};

    use super::{Fake, fake};
    use crate::pane_links::{LinkAsk, LinkFileAt, LinkHome, LinkOpened, answer};
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
            home: Some(FileHome {
                workspace: Some("app".into()),
                folder: "/w".into(),
            }),
            size: 1,
            version: "8:42:1:0.0".into(),
            piece: None,
        });
        let asks = Asks::default();

        let LinkOpened::Ask { about, token, .. } = opened(&runner, &asks, "/etc/hosts", None)
        else {
            panic!("expected an ask");
        };
        assert_eq!(
            about,
            LinkAsk::FileOnRunner {
                runner: "pool/runner-1".into(),
                home: Some(LinkHome::Workspace {
                    name: "app".into(),
                    folder: "/w".into(),
                }),
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
