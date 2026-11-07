// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #5427: **A chat on a pool's runner records its turns there, and they come back with its
//! code** (ADR 0078 §7; session protocol 1.78). The runner's host records
//! `refs/purlis/turns/<chat>/<n>/…` in its own folder; the desktop's fetch takes them as
//! `refs/purlis/runners/<pool>/turns/…` beside the runner's branches, where the turn diff reads
//! them; and they go with the chat. Through `codelink::give_with_turns`,
//! `codelink::fetch_back_with_turns`, `files::turn_changed` and `files::turns`, with real git.

use crate::support;

use std::path::Path;

use purlis_core::codelink::{self, Bundled};
use purlis_core::files::{self, Branch, FileDiff, InTurn, TurnChanged, turns};

fn reader() -> files::Reader {
    files::Reader::new(
        std::env::current_exe().expect("the test binary"),
        [
            "a_runners_turns_come_back_with_its_code::reader_child",
            "--exact",
            "--nocapture",
            "--test-threads=1",
            files::READ_ARG,
        ]
        .map(std::ffi::OsString::from),
    )
}

/// The reader's child: answers the one question it was asked and exits.
#[test]
fn reader_child() {
    purlis_core::unsteered!();
    if let Some(code) = files::serve_if_asked() {
        std::process::exit(code);
    }
}

const CHAT: &str = "01J9CHAT0000000000000000RN";
const POOL: &str = "builds";

fn refs(dir: &Path, under: &str) -> Vec<String> {
    let out = support::git(dir, &["for-each-ref", "--format=%(refname)", under]);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The runner's turn 1 of [`CHAT`] on `main`, which adds a line to the README: the two planes
/// start from the same commit, as the fixture pins its author and dates.
fn a_turn_on_the_runner() -> (support::Fixture, support::Fixture) {
    let runner = support::plane_with_clone("web");
    let desk = support::plane_with_clone("web");
    let branch = Branch::repo(&runner.ws, &runner.repo);
    assert_eq!(turns::began(&runner.plane, branch, CHAT).unwrap(), Some(1));
    std::fs::write(runner.clone.join("README.md"), "one\ntwo\n").unwrap();
    assert_eq!(turns::ended(&runner.plane, branch, CHAT).unwrap(), Some(1));
    (runner, desk)
}

/// The desktop fetches the runner's branches and turns, as a link of 1.78 gives them.
fn fetched(runner: &support::Fixture, desk: &support::Fixture) -> Vec<codelink::Ref> {
    let scratch = tempfile::tempdir().unwrap();
    codelink::fetch_back_with_turns(&desk.clone, POOL, scratch.path(), |have, into| {
        let (refs, bundled) =
            codelink::give_with_turns(&runner.clone, &have, into).map_err(|e| e.to_string())?;
        Ok((refs, bundled == Bundled::Wrote))
    })
    .expect("the runner's code and turns come back")
}

#[test]
fn a_turn_the_runner_recorded_comes_back_with_its_code_and_the_turn_diff_reads_it_as_it_ended() {
    let (runner, desk) = a_turn_on_the_runner();

    let came = fetched(&runner, &desk);

    // The runner's branches are where they always were, and no turn is listed among them.
    let names: Vec<&str> = came.iter().map(|one| one.name.as_str()).collect();
    assert_eq!(names, ["refs/remotes/builds/main"]);
    assert_eq!(
        refs(&desk.clone, "refs/purlis/runners/"),
        [
            format!("refs/purlis/runners/{POOL}/turns/{CHAT}/1/end"),
            format!("refs/purlis/runners/{POOL}/turns/{CHAT}/1/start"),
        ]
    );
    assert!(refs(&desk.clone, "refs/purlis/turns/").is_empty());

    // The turn diff reads it, as the turn ended there: the desktop's own folder, which never
    // had the runner's edit, is not what the turn is compared with.
    let read = files::turn_changed(
        &reader(),
        &desk.plane,
        Branch::repo(&desk.ws, &desk.repo),
        "README.md",
        None,
    )
    .expect("it reads");
    let TurnChanged::Of {
        chat,
        number: 1,
        turns: listed,
        file: InTurn::Changed(shown),
        ..
    } = read
    else {
        panic!("README.md is what the runner's turn changed: {read:?}");
    };
    assert_eq!((chat.as_str(), listed), (CHAT, vec![1]));
    assert!(!shown.since_turn_ended, "{shown:?}");
    let FileDiff::Text { base, head, .. } = shown.diff else {
        panic!("README.md is text");
    };
    assert_eq!((base.as_str(), head.as_str()), ("one\n", "one\ntwo\n"));
    assert_eq!(
        turns::runner_of(&desk.plane, Branch::repo(&desk.ws, &desk.repo), CHAT, 1),
        Some(POOL.to_owned())
    );
}

#[test]
fn a_turn_the_runner_undid_is_marked_undone_here_and_no_turn_is_left_to_undo() {
    let (runner, desk) = a_turn_on_the_runner();
    fetched(&runner, &desk);
    let here = Branch::repo(&desk.ws, &desk.repo);

    // As the runner answers an undo through the link, the desktop marks its copy.
    turns::undone(&desk.plane, here, CHAT, 1).expect("the copy is marked undone");

    assert!(
        refs(&desk.clone, "refs/purlis/runners/")
            .contains(&format!("refs/purlis/runners/{POOL}/turns/{CHAT}/1/undone"))
    );
    let read = files::turn_changed(&reader(), &desk.plane, here, "README.md", None).unwrap();
    let TurnChanged::Of { undo, .. } = read else {
        panic!("the turn still reads: {read:?}");
    };
    assert_eq!(undo, None, "an undone turn is not offered again");
}

#[test]
fn a_removed_chats_turns_go_from_the_runners_folder_and_from_the_copy_here() {
    let (runner, desk) = a_turn_on_the_runner();
    fetched(&runner, &desk);
    assert!(!refs(&desk.clone, "refs/purlis/runners/").is_empty());

    turns::forget(&runner.plane, &runner.ws, &runner.repo, CHAT).unwrap();
    turns::forget(&desk.plane, &desk.ws, &desk.repo, CHAT).unwrap();

    assert!(refs(&runner.clone, "refs/purlis/turns/").is_empty());
    assert!(refs(&desk.clone, "refs/purlis/runners/").is_empty());
    // The runner's branch it fetched is not the chat's, and stays.
    assert!(!refs(&desk.clone, "refs/remotes/builds/").is_empty());
}

#[test]
fn a_fetch_that_asks_for_no_turns_brings_none_as_before() {
    let (runner, desk) = a_turn_on_the_runner();
    let scratch = tempfile::tempdir().unwrap();

    codelink::fetch_back(&desk.clone, POOL, scratch.path(), |have, into| {
        let (refs, bundled) =
            codelink::give(&runner.clone, &have, into).map_err(|e| e.to_string())?;
        Ok((refs, bundled == Bundled::Wrote))
    })
    .unwrap();

    assert!(refs(&desk.clone, "refs/purlis/runners/").is_empty());
}

#[test]
fn a_turn_ref_a_runner_names_that_purlis_never_writes_is_refused_and_nothing_is_fetched() {
    let (runner, desk) = a_turn_on_the_runner();
    let scratch = tempfile::tempdir().unwrap();

    let refused =
        codelink::fetch_back_with_turns(&desk.clone, POOL, scratch.path(), |have, into| {
            let (mut refs, bundled) =
                codelink::give_with_turns(&runner.clone, &have, into).map_err(|e| e.to_string())?;
            let oid = refs[0].oid.clone();
            refs.push(codelink::Ref {
                name: format!("refs/purlis/turns/{CHAT}/1/elsewhere"),
                oid,
            });
            Ok((refs, bundled == Bundled::Wrote))
        })
        .unwrap_err();

    assert!(refused.to_string().contains("elsewhere"), "{refused}");
    assert!(refs(&desk.clone, "refs/purlis/runners/").is_empty());
    assert!(refs(&desk.clone, "refs/remotes/").is_empty());
}

// ---------------------------------------------------------------------------------------------
// The runner's host: what its hook channel does with its chats' turns
// (`purlis_core::hookchannel::HostedTurns`), off the socket the channel reads them from.
// ---------------------------------------------------------------------------------------------

mod on_the_runner {
    use super::*;

    use purlis_core::hookchannel::HostedTurns;
    use purlis_core::hookwire::relay::Heard;
    use purlis_core::hookwire::{CHAT_ENV, HARNESS_ENV, Report};
    use purlis_core::state::{Board, Event};
    use purlis_host::TurnToUndo;
    use purlis_host::protocol::session::{Answer, NOT_ALLOWED, UndoWaitsOn};

    const NUMBER: u32 = 7;

    fn heard(event: Event) -> Heard {
        Heard::Report(
            Report::read(event, "{}", &|name| match name {
                CHAT_ENV => Some(NUMBER.to_string()),
                HARNESS_ENV => Some("claude-code".to_owned()),
                _ => None,
            })
            .expect("a report of the chat"),
        )
    }

    fn until(what: &str, done: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !done() {
            assert!(
                std::time::Instant::now() < deadline,
                "{what} never happened"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// A runner whose host composed chat [`NUMBER`], [`CHAT`], in its clone, and the board it
    /// keeps of it.
    struct Runner {
        f: support::Fixture,
        hosted: HostedTurns,
        board: Board,
    }

    impl Runner {
        fn new() -> Runner {
            let f = support::plane_with_clone("web");
            purlis_core::hostedchats::record_place(&f.plane, NUMBER, CHAT, &f.clone);
            let mut board = Board::new();
            board.opened(NUMBER, None, None);
            let hosted = HostedTurns::new(f.plane.clone(), reader());
            Runner { f, hosted, board }
        }

        /// The chat's line `event`, as the channel hears it.
        fn hears(&mut self, event: Event) {
            let line = heard(event);
            self.board.hear(&line);
            self.hosted.heard(NUMBER, &line);
            until("the boundary to be recorded", || {
                !self.hosted.recording(CHAT)
            });
        }

        fn undo(&self, number: u32) -> Result<Answer, purlis_host::protocol::session::Refusal> {
            self.hosted.undo(
                &self.board,
                &TurnToUndo {
                    workspace: self.f.ws.clone(),
                    repo: self.f.repo.clone(),
                    piece: None,
                    chat: CHAT.to_owned(),
                    number,
                },
            )
        }

        fn readme(&self) -> String {
            std::fs::read_to_string(self.f.clone.join("README.md")).unwrap()
        }
    }

    impl Drop for Runner {
        fn drop(&mut self) {
            purlis_core::hostedchats::forget_place(&self.f.plane, NUMBER);
        }
    }

    #[test]
    fn the_runners_host_records_its_chats_turn_from_its_prompt_and_stop_and_the_desktop_fetches_it()
    {
        let mut runner = Runner::new();
        runner.hears(Event::UserPromptSubmit);
        std::fs::write(runner.f.clone.join("README.md"), "one\nthree\n").unwrap();
        runner.hears(Event::Stop);

        assert_eq!(
            refs(&runner.f.clone, "refs/purlis/turns/"),
            [
                format!("refs/purlis/turns/{CHAT}/1/end"),
                format!("refs/purlis/turns/{CHAT}/1/start"),
            ]
        );
        let desk = support::plane_with_clone("web");
        fetched(&runner.f, &desk);
        let read = files::turn_changed(
            &reader(),
            &desk.plane,
            Branch::repo(&desk.ws, &desk.repo),
            "README.md",
            None,
        )
        .unwrap();
        assert!(
            matches!(read, TurnChanged::Of { number: 1, ref chat, .. } if chat == CHAT),
            "{read:?}"
        );
    }

    #[test]
    fn an_undo_through_the_link_waits_while_the_chat_is_running_and_writes_nothing() {
        let mut runner = Runner::new();
        runner.hears(Event::UserPromptSubmit);
        std::fs::write(runner.f.clone.join("README.md"), "one\nthree\n").unwrap();
        runner.hears(Event::Stop);
        // A prompt begins its next turn.
        runner.hears(Event::UserPromptSubmit);

        let answer = runner.undo(1).expect("answered");

        let Answer::UndoWaits { chats } = answer else {
            panic!("it waits: {answer:?}");
        };
        assert_eq!(
            chats,
            [UndoWaitsOn {
                number: NUMBER,
                state: "is running".into()
            }]
        );
        assert_eq!(runner.readme(), "one\nthree\n");
    }

    #[test]
    fn an_undo_through_the_link_of_a_turn_whose_file_changed_since_it_ended_is_refused_naming_it() {
        let mut runner = Runner::new();
        runner.hears(Event::UserPromptSubmit);
        std::fs::write(runner.f.clone.join("README.md"), "one\nthree\n").unwrap();
        runner.hears(Event::Stop);
        std::fs::write(runner.f.clone.join("README.md"), "one\nthree\nfour\n").unwrap();

        let refused = runner.undo(1).unwrap_err();

        assert_eq!(refused.code, NOT_ALLOWED, "{refused:?}");
        assert!(refused.why.contains("README.md"), "{}", refused.why);
        assert_eq!(runner.readme(), "one\nthree\nfour\n");
    }

    #[test]
    fn an_undo_through_the_link_puts_the_turns_files_back_on_the_runner_and_marks_it_undone() {
        let mut runner = Runner::new();
        runner.hears(Event::UserPromptSubmit);
        std::fs::write(runner.f.clone.join("README.md"), "one\nthree\n").unwrap();
        runner.hears(Event::Stop);

        let answer = runner.undo(1).expect("undone");

        let Answer::TurnUndone { edit, unmarked } = answer else {
            panic!("undone: {answer:?}");
        };
        assert_eq!(unmarked, None);
        let edit: purlis_core::humanedit::HumanEdit =
            serde_json::from_value(edit).expect("the record of the edit");
        assert_eq!(edit.files.len(), 1, "{edit:?}");
        assert_eq!(runner.readme(), "one\n");
        assert!(
            refs(&runner.f.clone, "refs/purlis/turns/")
                .contains(&format!("refs/purlis/turns/{CHAT}/1/undone"))
        );
        // Only the latest turn not yet undone is: there is none left.
        assert_eq!(runner.undo(1).unwrap_err().code, NOT_ALLOWED);
    }

    #[test]
    fn a_chat_its_host_forgets_takes_its_turns_with_it() {
        let mut runner = Runner::new();
        runner.hears(Event::UserPromptSubmit);
        runner.hears(Event::Stop);
        assert!(!refs(&runner.f.clone, "refs/purlis/turns/").is_empty());

        runner.hosted.removed(NUMBER);

        until("the chat's turns to go", || {
            refs(&runner.f.clone, "refs/purlis/turns/").is_empty()
        });
    }
}
