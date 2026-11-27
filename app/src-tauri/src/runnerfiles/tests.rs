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
    /// How long the runner takes to answer each ask.
    pub(super) slow: Duration,
}

pub(super) fn fake(bytes: &[u8]) -> Fake {
    Fake {
        bytes: bytes.to_vec(),
        versions: Arc::new(Mutex::new(vec!["v1"])),
        asked: Arc::default(),
        answer: None,
        slow: Duration::ZERO,
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
        std::thread::sleep(self.slow);
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
fn pieces_that_join_into_a_file_that_is_not_text_are_said_to_be_binary() {
    // Each piece passed as text on the runner, cut inside a character; whole, it is not.
    let runner = fake(b"caf\xe9\n");

    assert_eq!(
        reads(&runner).read("latin1.txt", None),
        Ok(ReadThere::Binary(5))
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
fn a_file_the_runner_keeps_is_not_shown_even_when_it_was_opened_on_an_ask() {
    let mut runner = fake(b"");
    runner.answer = Some(FileRead::Kept {
        path: "/home/purlis/.codex/auth.json".into(),
    });

    for opened in [None, Some("1:2")] {
        let said = reads(&runner)
            .read("/home/purlis/.codex/auth.json", opened)
            .unwrap_err();
        assert_eq!(said, format!("{}.", purlis_core::panelinks::RUNNER_KEPT));
    }
}

#[test]
fn a_file_the_runner_could_not_tell_of_is_not_shown_and_the_tab_says_so() {
    let mut runner = fake(b"");
    runner.answer = Some(FileRead::Unsure {
        path: "/w/second.md".into(),
    });

    let said = reads(&runner).read("/w/second.md", None).unwrap_err();

    assert_eq!(said, format!("{}.", purlis_core::panelinks::RUNNER_UNSURE));
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
                machine: &purlis_core::sandbox::Machine::this(),
                guarded: &[],
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
            now_in: None,
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
            target: None,
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

    /// The places among `MOST_RUNNER_LOOKS` are the app's, shared by every line: one test of a
    /// line's looks at a time.
    static ONE_LINE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn each_path_on_a_runner_chats_line_is_looked_at_within_the_panes_own_wait() {
        let _alone = ONE_LINE_AT_A_TIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut runner = fake(b"fn main() {}\n");
        runner.slow = std::time::Duration::from_millis(1200);
        let candidates: Vec<Candidate> = ["src/a.rs", "src/b.rs", "src/c.rs"]
            .into_iter()
            .map(file)
            .collect();
        let started = std::time::Instant::now();

        let seen = crate::pane_links::seen_on_line(
            &links(&runner),
            &on_runner(),
            &candidates,
            &crate::pane_links::ChatsHere::none(),
        );

        assert!(
            started.elapsed() < crate::pane_links::LOOK_FOR,
            "{:?}: one slow path holds up the others",
            started.elapsed()
        );
        assert!(seen.iter().all(Option::is_some), "{seen:?}");
    }

    #[test]
    fn a_runner_chats_line_of_more_slow_paths_than_are_looked_at_together_keeps_those_found() {
        let _alone = ONE_LINE_AT_A_TIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut runner = fake(b"fn main() {}\n");
        runner.slow = std::time::Duration::from_millis(1200);
        let candidates: Vec<Candidate> = (0..9).map(|n| file(&format!("src/{n}.rs"))).collect();
        let started = std::time::Instant::now();

        let seen = crate::pane_links::seen_on_line(
            &links(&runner),
            &on_runner(),
            &candidates,
            &crate::pane_links::ChatsHere::none(),
        );

        assert!(
            started.elapsed() < crate::pane_links::LOOK_FOR,
            "{:?}: past the pane's wait, the whole line is unlinked",
            started.elapsed()
        );
        assert_eq!(seen.len(), 9);
        assert_eq!(
            seen.iter().filter(|one| one.is_some()).count(),
            crate::pane_links::MOST_RUNNER_LOOKS,
            "{seen:?}"
        );
    }

    #[test]
    fn a_runner_chats_line_asked_while_every_runner_look_is_taken_answers_at_once_unlinked() {
        let _alone = ONE_LINE_AT_A_TIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut runner = fake(b"fn main() {}\n");
        runner.slow = std::time::Duration::from_millis(1200);
        let taken = crate::pane_links::RunnerLooks::take(crate::pane_links::MOST_RUNNER_LOOKS);
        let started = std::time::Instant::now();

        let seen = crate::pane_links::seen_on_line(
            &links(&runner),
            &on_runner(),
            &[file("src/a.rs"), file("src/b.rs")],
            &crate::pane_links::ChatsHere::none(),
        );

        assert!(started.elapsed() < std::time::Duration::from_millis(500));
        assert_eq!(seen, vec![None, None]);
        assert!(runner.asked.lock().unwrap().is_empty());
        drop(taken);
    }
}

/// `runners.json` under `config`: the Docker pool `builds` with its runner `builds-01234567` and
/// the runner it replaced, `builds`, draining; the Docker pool `empty` with no runner; and the
/// cluster pool `team` with its own runner and one chat's fresh runner. The fresh runner's link
/// name.
#[cfg(unix)]
fn every_kind_of_runner_kept(config: &std::path::Path) -> String {
    use std::os::unix::fs::PermissionsExt as _;
    const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let cluster_row = |device: &str, key: &str, claim: &str| {
        serde_json::json!({
            "name": "team", "pool": "cluster", "namespace": "ada", "claim": claim,
            "claimUid": "6f1c2b8e-0f43-4c2e-9d7a-1b2c3d4e5f60", "requester": "ada@example.com",
            "pod": claim, "device": device, "key": key.repeat(32), "paired": 1_800_000_000_u64,
        })
    };
    let mut fresh = cluster_row("01JB0RUNNER0000000000000002", "5e", "team-2");
    fresh["chat"] = "01M7HNDAQDBEJFC36Z3NYPRGR4".into();
    fresh["plane"] = "p-1".into();
    fresh["workspace"] = "app".into();
    let file = serde_json::json!({
        "version": 1,
        "pools": [
            { "name": "builds", "kind": "docker", "image": IMAGE },
            { "name": "empty", "kind": "docker", "image": IMAGE },
            {
                "name": "team", "kind": "cluster", "kubeconfig": "/home/ada/.kube/config",
                "context": "team", "pool": "default", "namespace": "ada",
                "operatorKey": format!("ed25519:{}", "ab".repeat(32)),
                "cluster": "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b",
                "holds": ["/opt/homebrew/bin/kubectl"],
            },
        ],
        "runners": [
            {
                "name": "builds", "pool": "docker", "container": "a".repeat(64), "image": IMAGE,
                "user": "10001:10001", "home": "/home/purlis",
                "device": "01JB0RUNNER0000000000000000", "key": "12".repeat(32), "paired": 1,
                "draining": true,
            },
            {
                "name": "builds-01234567", "pool": "docker", "container": "b".repeat(64),
                "image": IMAGE, "user": "10001:10001", "home": "/home/purlis",
                "device": "01JB0RUNNER0000000000000003", "key": "13".repeat(32), "paired": 2,
                "of": "builds",
            },
            cluster_row("01JB0RUNNER0000000000000001", "4d", "team-1"),
            fresh,
        ],
    });
    let folder = purlis_core::machine::dir(config);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = folder.join(purlis_core::machine::RUNNERS_FILE);
    std::fs::write(&path, file.to_string()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let fresh = purlis_core::runners::fresh_runners(config).expect("the runners are read");
    assert_eq!(fresh.len(), 1, "the fresh runner is kept");
    fresh[0].link_name()
}

/// **The window reads a runner chat's files on every kind of runner purlis pairs, and only on
/// one a chat reaches** (#6949): a Docker pool's runner, and its draining runner by its exact
/// name; a cluster pool's own runner; and a chat's fresh runner on a cluster pool. A Docker pool
/// with no runner kept, a runner of a pool that is not draining, a runner name this machine does
/// not keep, an unkept fresh runner and an unknown name read none: on every kind, the reach is the
/// one a chat's link has (`runners::reachable`, `runners::cluster_runner_for`).
#[cfg(unix)]
#[test]
fn a_docker_pools_runner_a_cluster_pools_runner_and_a_fresh_runner_are_read_and_no_other() {
    use purlis_session_protocol::session::KNOWS_KEPT_FILES;
    let config = tempfile::tempdir().unwrap();
    let fresh = every_kind_of_runner_kept(config.path());

    assert!(super::reading_files_needs(config.path(), "builds") == Some(KNOWS_KEPT_FILES));
    assert!(
        super::reading_files_needs(config.path(), "builds/builds") == Some(KNOWS_KEPT_FILES),
        "a Docker pool's runner draining beside its new one"
    );
    assert!(super::reading_files_needs(config.path(), "team") == Some(KNOWS_KEPT_FILES));
    assert!(super::reading_files_needs(config.path(), &fresh) == Some(KNOWS_KEPT_FILES));
    for none in [
        "empty",
        "builds/builds-01234567",
        "builds/gone",
        "team/fresh-0000",
        "nowhere",
    ] {
        assert!(
            super::reading_files_needs(config.path(), none).is_none(),
            "{none} reads no file"
        );
    }
}

/// **A runner before 1.97 is not asked for a file, and one from 1.97 is** (#6387, #7064): what
/// reading files needs, on every kind, is the version that keeps a runner's own folders and
/// sign-ins from the window.
#[cfg(unix)]
#[test]
fn a_runner_before_1_97_is_not_asked_for_a_file_and_one_from_1_97_is() {
    use purlis_session_protocol::version::Version;
    let at = |minor| Version { major: 1, minor };
    let config = tempfile::tempdir().unwrap();
    let fresh = every_kind_of_runner_kept(config.path());

    for named in ["builds", "team", fresh.as_str()] {
        let needs = super::reading_files_needs(config.path(), named).unwrap();
        assert!(at(96) < needs, "{named}");
        assert!(at(97) >= needs, "{named}");
        assert!(
            purlis_session_protocol::session::VERSION >= needs,
            "{named}"
        );
    }
}
