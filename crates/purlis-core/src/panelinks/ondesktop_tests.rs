// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use purlis_session_protocol::session::{FileHome, FileRead};

use super::super::{
    Asks, Candidate, Home, HomeIs, Hover, Kind, LocalFile, Opening, Pane, ReadFrom, Resolved,
    Resolver, Runs, Target,
};
use super::{RunnerFile, RunnerFiles};

/// A runner that answers each path from its table, and records every path it was asked.
#[derive(Clone, Default)]
struct Fake {
    files: Vec<(&'static str, FileRead)>,
    asked: Arc<Mutex<Vec<String>>>,
    silent: bool,
}

impl RunnerFiles for Fake {
    fn runner(&self) -> &str {
        "docker-1"
    }

    fn look(&self, path: &str) -> Option<FileRead> {
        self.asked.lock().unwrap().push(path.to_owned());
        if self.silent {
            return None;
        }
        Some(
            self.files
                .iter()
                .find(|(named, _)| *named == path)
                .map_or(FileRead::NotThere, |(_, found)| found.clone()),
        )
    }
}

const MAIN: &str = "/home/purlis/projects/p/workspaces/app/repos/web/src/main.rs";

const APP: &str = "/home/purlis/projects/p/workspaces/app";

/// The chat's workspace on the runner, `app`.
fn in_app() -> Home {
    Home {
        is: HomeIs::Workspace("app".into()),
        folder: PathBuf::from(APP),
    }
}

fn plain(path: &str, from_workspace: Option<&str>) -> FileRead {
    FileRead::Plain {
        path: path.to_owned(),
        from_workspace: from_workspace.map(str::to_owned),
        home: Some(FileHome {
            workspace: Some("app".into()),
            folder: APP.into(),
        }),
        size: 13,
        version: "1:2:13:0.0".into(),
        piece: None,
    }
}

fn fake() -> Fake {
    Fake {
        files: vec![
            ("src/main.rs", plain(MAIN, Some("repos/web/src/main.rs"))),
            ("/etc/hosts", plain("/etc/hosts", None)),
            (
                "logo.png",
                FileRead::Binary {
                    path: "/w/logo.png".into(),
                    size: 9,
                },
            ),
            (
                "huge.log",
                FileRead::TooLarge {
                    path: "/w/huge.log".into(),
                    size: 3 << 20,
                },
            ),
            (
                "src",
                FileRead::NotPlain {
                    path: "/w/src".into(),
                },
            ),
            (
                "/home/purlis/.codex/auth.json",
                FileRead::Kept {
                    path: "/home/purlis/.codex/auth.json".into(),
                },
            ),
            (
                "/w/second.md",
                FileRead::Unsure {
                    path: "/w/second.md".into(),
                },
            ),
        ],
        ..Fake::default()
    }
}

fn links(runner: &Fake) -> Resolver {
    Resolver::with(vec![
        Box::new(LocalFile::with(None, Vec::new())),
        Box::new(RunnerFile::new(Box::new(runner.clone()))),
    ])
}

fn on_runner() -> Pane {
    Pane {
        project: Some(PathBuf::from("/planes/one")),
        session: Some(7),
        folder: None,
        runs: Runs::OnRunner,
    }
}

fn file(text: &str) -> Candidate {
    Candidate {
        kind: Kind::File,
        text: text.to_owned(),
        target: None,
    }
}

fn file_at(target: &Target) -> &super::super::FileAt {
    match target {
        Target::File(file) => file,
        other => panic!("expected a file, got {other:?}"),
    }
}

#[test]
fn a_file_in_the_runner_chats_workspace_links_and_opens_at_its_line_without_asking() {
    let runner = fake();
    let links = links(&runner);

    let Hover::Link { target, opening } = links.hover(&file("src/main.rs:42:7"), &on_runner())
    else {
        panic!("expected a link");
    };
    let at = file_at(&target);
    assert_eq!(at.path, PathBuf::from(MAIN));
    assert_eq!(
        at.read_from,
        ReadFrom::Runner {
            runner: "docker-1".into(),
            home: Some(in_app()),
            identity: Some("1:2".into()),
        }
    );
    assert_eq!((at.line, at.column), (Some(42), Some(7)));
    assert_eq!(at.short(), "repos/web/src/main.rs:42:7");
    assert_eq!(at.shown(), format!("{MAIN}:42:7"));
    assert!(matches!(*opening, Opening::Open(_)));

    let opened = links.open(
        &file("src/main.rs:42:7"),
        &on_runner(),
        &Asks::default(),
        None,
    );
    assert!(
        matches!(opened, Opening::Open(Target::File(_))),
        "{opened:?}"
    );
}

#[test]
fn the_runner_is_asked_the_path_alone_as_the_chat_printed_it() {
    let runner = fake();

    links(&runner).resolve(&file("src/main.rs:42:7"), &on_runner());

    assert_eq!(*runner.asked.lock().unwrap(), ["src/main.rs"]);
}

#[test]
fn a_file_outside_the_runner_chats_workspace_asks_first_naming_the_runner() {
    let runner = fake();
    let links = links(&runner);
    let asks = Asks::default();

    let Opening::Ask {
        reason,
        target,
        token: Some(token),
        ..
    } = links.open(&file("/etc/hosts"), &on_runner(), &asks, None)
    else {
        panic!("expected an ask");
    };
    assert_eq!(reason, "outside app on runner docker-1");
    assert_eq!(file_at(&target).short(), "/etc/hosts");
    assert_eq!(
        file_at(&target).read_from,
        ReadFrom::Runner {
            runner: "docker-1".into(),
            home: Some(in_app()),
            identity: Some("1:2".into()),
        },
        "the ask names the chat's workspace on the runner"
    );
    assert!(
        asks.opened_on_runner(&on_runner(), "docker-1", "/etc/hosts")
            .is_none()
    );

    runner.asked.lock().unwrap().clear();
    let opened = links.open(&file("/etc/hosts"), &on_runner(), &asks, Some(&token));
    assert_eq!(
        *runner.asked.lock().unwrap(),
        ["/etc/hosts"],
        "the answer is one look at the runner: the one that finds the file again"
    );
    assert!(
        matches!(opened, Opening::Open(Target::File(_))),
        "{opened:?}"
    );
    assert_eq!(
        asks.opened_on_runner(&on_runner(), "docker-1", "/etc/hosts")
            .as_deref(),
        Some("1:2"),
        "kept as the file it was when the ask was answered"
    );
    let other_chat = Pane {
        session: Some(8),
        ..on_runner()
    };
    assert!(
        asks.opened_on_runner(&other_chat, "docker-1", "/etc/hosts")
            .is_none()
    );
    assert!(
        asks.opened_on_runner(&on_runner(), "docker-2", "/etc/hosts")
            .is_none()
    );
}

#[test]
fn what_the_runner_does_not_find_as_a_plain_file_says_nothing() {
    let runner = fake();

    for text in ["src/gone.rs", "src"] {
        assert_eq!(
            links(&runner).resolve(&file(text), &on_runner()),
            Resolved::Quiet,
            "{text}"
        );
    }
}

#[test]
fn a_binary_or_too_large_file_is_no_link_and_says_why() {
    let runner = fake();

    for (text, why) in [("logo.png", "not text"), ("huge.log", "2 MiB")] {
        match links(&runner).hover(&file(text), &on_runner()) {
            Hover::NotALink(said) => assert!(said.contains(why), "{text}: {said}"),
            other => panic!("{text}: expected no link, got {other:?}"),
        }
    }
}

#[test]
fn a_file_the_runner_could_not_tell_of_says_so_and_not_that_it_is_kept() {
    let runner = fake();

    match links(&runner).hover(&file("/w/second.md"), &on_runner()) {
        Hover::NotALink(said) => {
            assert_eq!(said, super::RUNNER_UNSURE);
            assert_ne!(said, super::RUNNER_KEPT);
        }
        other => panic!("expected no link, got {other:?}"),
    }
}

#[test]
fn a_file_the_runner_keeps_is_no_link_says_why_and_never_asks() {
    let runner = fake();
    let links = links(&runner);
    let text = "/home/purlis/.codex/auth.json";

    match links.hover(&file(text), &on_runner()) {
        Hover::NotALink(said) => assert_eq!(said, super::RUNNER_KEPT),
        other => panic!("expected no link, got {other:?}"),
    }
    assert!(
        !matches!(
            links.open(&file(text), &on_runner(), &Asks::default(), None),
            Opening::Ask { .. } | Opening::Open(_)
        ),
        "a kept file is never asked for or opened"
    );
}

#[test]
fn a_runner_that_does_not_answer_leaves_the_text_quiet() {
    let runner = Fake {
        silent: true,
        ..fake()
    };

    assert_eq!(
        links(&runner).resolve(&file("src/main.rs"), &on_runner()),
        Resolved::Quiet
    );
}

#[test]
fn a_home_path_or_a_spaced_word_is_never_asked_of_the_runner() {
    let runner = fake();

    for text in [
        "~/.claude/.credentials.json",
        "~purlis/x",
        "",
        "a b",
        "src/\u{7}x",
    ] {
        assert_eq!(
            links(&runner).resolve(&file(text), &on_runner()),
            Resolved::Quiet,
            "{text:?}"
        );
    }
    assert!(runner.asked.lock().unwrap().is_empty());
}

#[test]
fn a_file_address_names_the_runners_file() {
    let runner = fake();

    let resolved = links(&runner).resolve(&file("file:///etc/hosts"), &on_runner());

    assert!(
        matches!(resolved, Resolved::Link(Target::File(_))),
        "{resolved:?}"
    );
    assert_eq!(*runner.asked.lock().unwrap(), ["/etc/hosts"]);
}

/// **A runner chat's path is read as a path on this machine is** (#7081): one reading, the host
/// file link's, so a `FILE:` address is the same scheme as `file:` on both, and one naming
/// another host says why on both, before the runner is asked anything.
#[test]
fn a_runner_chats_path_is_read_as_a_path_here_is() {
    let runner = fake();

    let capitals = links(&runner).resolve(&file("FILE:///etc/hosts:3"), &on_runner());
    let elsewhere = links(&runner).resolve(&file("file://build-7/etc/hosts"), &on_runner());

    let Resolved::Link(Target::File(found)) = capitals else {
        panic!("a FILE: address names the runner's file: {capitals:?}");
    };
    assert_eq!(found.line, Some(3));
    let here = Pane {
        runs: Runs::Here,
        ..on_runner()
    };
    assert_eq!(
        elsewhere,
        super::super::LinkKind::resolve(
            &LocalFile::with(None, Vec::new()),
            "file://build-7/etc/hosts",
            &here
        )
    );
    assert!(matches!(elsewhere, Resolved::NotALink(_)), "{elsewhere:?}");
    assert_eq!(*runner.asked.lock().unwrap(), ["/etc/hosts"]);
}

#[test]
fn a_runner_chats_path_is_never_read_on_this_machine() {
    let here = tempfile::tempdir().expect("a folder");
    let local = std::fs::canonicalize(here.path())
        .expect("resolved")
        .join("notes.md");
    std::fs::write(&local, "notes\n").expect("written");
    let runner = fake();
    let text = local.to_str().expect("text");

    assert_eq!(
        links(&runner).resolve(&file(text), &on_runner()),
        Resolved::Quiet
    );
    assert_eq!(*runner.asked.lock().unwrap(), [text]);
}

#[test]
fn a_chat_on_this_machine_never_asks_a_runner() {
    let runner = fake();
    let here = Pane {
        runs: Runs::Here,
        ..on_runner()
    };

    links(&runner).resolve(&file("src/main.rs"), &here);

    assert!(runner.asked.lock().unwrap().is_empty());
}

#[test]
fn a_runner_chat_with_no_way_to_read_its_runner_links_no_file() {
    assert_eq!(
        Resolver::default().resolve(&file("src/main.rs"), &on_runner()),
        Resolved::Quiet
    );
}

#[test]
fn a_file_the_runner_names_no_identity_for_is_not_kept_as_opened() {
    let runner = Fake {
        files: vec![(
            "/etc/hosts",
            FileRead::Plain {
                path: "/etc/hosts".into(),
                from_workspace: None,
                home: None,
                size: 13,
                version: "13:1700000000".into(),
                piece: None,
            },
        )],
        ..Fake::default()
    };
    let links = links(&runner);
    let asks = Asks::default();
    let Opening::Ask {
        token: Some(token), ..
    } = links.open(&file("/etc/hosts"), &on_runner(), &asks, None)
    else {
        panic!("expected an ask");
    };

    links.open(&file("/etc/hosts"), &on_runner(), &asks, Some(&token));

    assert!(
        asks.opened_on_runner(&on_runner(), "docker-1", "/etc/hosts")
            .is_none()
    );
}

#[test]
fn an_answer_given_for_a_file_since_swapped_for_another_opens_nothing() {
    let runner = fake();
    let asks = Asks::default();
    let Opening::Ask {
        token: Some(token), ..
    } = links(&runner).open(&file("/etc/hosts"), &on_runner(), &asks, None)
    else {
        panic!("expected an ask");
    };
    // The same file in every way but which one it is.
    let mut another = plain("/etc/hosts", None);
    if let FileRead::Plain { version, .. } = &mut another {
        *version = "1:3:13:0.0".into();
    }
    let swapped = Fake {
        files: vec![("/etc/hosts", another)],
        ..Fake::default()
    };

    let opened = links(&swapped).open(&file("/etc/hosts"), &on_runner(), &asks, Some(&token));

    assert!(!matches!(opened, Opening::Open(_)), "{opened:?}");
    assert!(
        asks.opened_on_runner(&on_runner(), "docker-1", "/etc/hosts")
            .is_none()
    );
}
