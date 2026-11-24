// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's own branch is one the app recorded it creating through the forge route**, and
//! nothing a chat can write: not the piece log (a hand-written `claimed` line, or taking a free
//! folder, is a claim on a folder and no more) and not a clone's refs (a remote ref can be deleted
//! or never fetched). The cut also refuses a name a remote has, as a courtesy.

use crate::support;
use crate::support::forge_listener::{ACME_APP_ID, Answer, PROJECT_ID, World, basic, pkt};

use std::path::Path;

use purlis_core::forge::createdbranches::{self, CreatedBranch};
use purlis_core::piececlaim::{self, Act, Live, Writer};
use purlis_core::pieces::Who;
use purlis_core::sandbox::{self, Compiled, Level, Machine, Os, grant};
use purlis_core::worktree::{self, Located, Refusal};

const CHAT: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S2A";
const OTHER: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S2B";
const PROJECT: &str = PROJECT_ID;
const REPO_ID: u64 = ACME_APP_ID;
const HOST: &str = "github.com";
const REPO: &str = "acme/app";

fn who(id: &str) -> Who {
    Who {
        session: Some("7".into()),
        persona: Some("builder".into()),
        chat: Some(id.to_owned()),
        host: "box".into(),
        log: "box".into(),
    }
}

/// The clone has seen `branch` on `origin`, as a fetch leaves it.
fn remote_has(f: &support::Fixture, branch: &str) {
    support::git(
        &f.clone,
        &["remote", "add", "origin", "https://example.invalid/x.git"],
    );
    support::git(
        &f.clone,
        &[
            "update-ref",
            &format!("refs/remotes/origin/{branch}"),
            "HEAD",
        ],
    );
}

#[test]
fn a_cut_may_not_take_the_name_of_a_branch_a_remote_has() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    remote_has(&f, "phase-1-proven");

    let refused = worktree::add(&f.plane, &f.ws, &f.repo, "mine", Some("phase-1-proven"));
    assert!(
        matches!(refused, Err(Refusal::BranchTaken { ref branch, .. }) if branch == "phase-1-proven"),
        "{refused:?}"
    );

    // The piece's own name is read the same way, and another name is cut.
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "phase-1-proven", None);
    assert!(
        matches!(piece, Err(Refusal::BranchTaken { .. })),
        "{piece:?}"
    );
    worktree::add(&f.plane, &f.ws, &f.repo, "mine", Some("task/mine")).unwrap();
}

fn own(config: &Path, chat: &str) -> Vec<String> {
    createdbranches::created_by(config, PROJECT, chat, HOST, REPO_ID)
}

fn made(branch: &str) -> CreatedBranch {
    CreatedBranch {
        host: HOST.to_owned(),
        repo: REPO.to_owned(),
        repo_id: Some(REPO_ID),
        branch: branch.to_owned(),
        tip: "1".repeat(40),
    }
}

fn located(f: &support::Fixture, piece: &str) -> Located {
    Located {
        workspace: f.ws.clone(),
        repo: f.repo.clone(),
        piece: piece.to_owned(),
    }
}

#[test]
fn a_hand_written_claim_line_makes_a_chat_the_folders_holder_and_no_branch_its_own() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let config = tempfile::tempdir().unwrap();
    worktree::add(&f.plane, &f.ws, &f.repo, "task-one", Some("phase-1-proven")).unwrap();
    // A chat writes a `claimed` line for itself with a ts far ahead, into the log it can reach.
    let log = purlis_core::pieces::log_path(&f.plane, &f.ws, "box");
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    let line = serde_json::json!({
        "event": "claimed", "piece": "task-one", "repo": f.repo, "chat": CHAT,
        "session": "7", "persona": "builder", "ts": "2999-01-01T00:00:00Z",
    });
    std::fs::write(&log, format!("{line}\n")).unwrap();

    // The claim does what a claim does: the chat holds the folder...
    let writer = piececlaim::writer(
        &f.plane,
        &located(&f, "task-one"),
        Some(CHAT),
        None,
        Act::Commit,
    );
    assert_eq!(writer, Writer::Yours);
    // ...and that makes no branch its own: the app's record is not written from the log, and a
    // line a chat forged for itself into the one file it can reach is not read as a creation.
    assert!(!createdbranches::file_in(config.path()).exists());
    assert!(own(config.path(), CHAT).is_empty());
}

#[test]
fn taking_a_free_folder_on_an_existing_branch_makes_it_no_ones_own() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let config = tempfile::tempdir().unwrap();
    worktree::add(&f.plane, &f.ws, &f.repo, "task-one", Some("task/one")).unwrap();
    let live = [Live {
        id: CHAT.to_owned(),
        chat: Some(7),
        back: true,
    }];
    piececlaim::take(
        &f.plane,
        &located(&f, "task-one"),
        &who(CHAT),
        Some(&live),
        Act::Commit,
        jiff::Timestamp::now(),
    )
    .unwrap();

    assert!(!createdbranches::file_in(config.path()).exists());
    assert!(own(config.path(), CHAT).is_empty());
}

#[test]
fn a_remote_ref_that_is_deleted_or_never_fetched_lets_the_name_be_cut_and_still_not_be_owned() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let config = tempfile::tempdir().unwrap();
    remote_has(&f, "m75-policy");
    assert!(worktree::add(&f.plane, &f.ws, &f.repo, "spoof", Some("m75-policy")).is_err());

    // The chat deletes the ref the cut check reads (or it was never fetched), and cuts the name.
    support::git(
        &f.clone,
        &["update-ref", "-d", "refs/remotes/origin/m75-policy"],
    );
    worktree::add(&f.plane, &f.ws, &f.repo, "spoof", Some("m75-policy")).unwrap();

    assert!(!createdbranches::file_in(config.path()).exists());
    assert!(own(config.path(), CHAT).is_empty());
}

#[test]
fn a_branch_recorded_as_created_is_its_creating_chats_own_and_no_other_chats() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    createdbranches::record(config.path(), PROJECT, CHAT, &[made("task/one")]).unwrap();

    assert_eq!(own(config.path(), CHAT), ["task/one"]);
    // A resumed chat has another id: it owns none, and may fast-forward the branch only.
    assert!(own(config.path(), OTHER).is_empty());
}

#[test]
fn another_projects_chat_with_the_same_id_and_an_old_line_with_no_project_id_own_nothing() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    createdbranches::record(config.path(), PROJECT, CHAT, &[made("task/one")]).unwrap();
    let file = createdbranches::file_in(config.path());
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str(&format!(
        "{}\n",
        serde_json::json!({"project": "/work/project", "chat": CHAT, "host": HOST, "repo": REPO, "branch": "task/old"})
    ));
    std::fs::write(&file, text).unwrap();

    assert_eq!(own(config.path(), CHAT), ["task/one"]);
    let elsewhere = createdbranches::created_by(
        config.path(),
        "01J9ZQ3V5N8X4T2K7M6P0R1S2Q",
        CHAT,
        HOST,
        REPO_ID,
    );
    assert!(elsewhere.is_empty());
}

fn record_line(chat: &str, branch: &str) -> String {
    format!(
        "{}\n",
        serde_json::json!({"project_id": PROJECT, "chat": chat, "host": HOST, "repo": REPO, "repo_id": REPO_ID, "branch": branch, "tip": "1".repeat(40)})
    )
}

#[test]
fn a_line_forged_for_another_chats_id_is_not_own_for_this_chat() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    createdbranches::record(config.path(), PROJECT, OTHER, &[made("task/theirs")]).unwrap();
    // A line a chat appended by hand, naming the other chat, in the file the route reads.
    let file = createdbranches::file_in(config.path());
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str(&record_line(OTHER, "task/forged"));
    std::fs::write(&file, text).unwrap();

    assert!(own(config.path(), CHAT).is_empty());
    assert_eq!(own(config.path(), OTHER), ["task/theirs", "task/forged"]);
}

#[test]
fn the_record_file_is_in_a_folder_the_floor_denies_a_chat_to_write() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[(
            "XDG_CONFIG_HOME",
            &config.path().display().to_string(),
        )]),
        home: Some(std::path::PathBuf::from("/nonexistent-home/op")),
        os: Os::MacOs,
    };
    let compiled = Compiled::floor(project.path(), &machine, &grant::Grants::default());
    assert_eq!(compiled.level, Level::Floor);
    let denied =
        |path: std::path::PathBuf| sandbox::covering(&compiled.denied.paths, &[path]).is_some();

    // Where the record is kept, and where it was kept in the first version of this change
    // (`<config>/forge-created-branches.jsonl`, beside purlis's folder and not in it).
    assert!(denied(createdbranches::file_in(config.path())));
    assert!(!denied(config.path().join("forge-created-branches.jsonl")));
}

/// A route whose confirmed branches are written to `config`, and a push of `branch` through it.
fn push_through(w: &World, branch: &str, old: &str, answer: Answer) -> u16 {
    push_moving(w, branch, old, &"1".repeat(40), answer)
}

/// What [`push_moving`] says of a push the route refused: git is answered `200` with a
/// report-status that rejects each ref ([`refused_in_gits_words`]), which this stands for.
const REFUSED: u16 = 403;

/// The same push, from `old` to `new`: the forge's status where the route let it through, and
/// [`REFUSED`] where the route answered it itself.
fn push_moving(w: &World, branch: &str, old: &str, new: &str, answer: Answer) -> u16 {
    let reply = push_reply(w, branch, old, new, answer);
    if refused_in_gits_words(&reply, branch) {
        REFUSED
    } else {
        reply.status
    }
}

/// Whether the route answered the push itself, with a report-status that rejects `branch`.
fn refused_in_gits_words(reply: &support::forge_listener::Reply, branch: &str) -> bool {
    reply.status == 200
        && reply
            .text()
            .contains(&format!("ng refs/heads/{branch} purlis: "))
}

fn push_reply(
    w: &World,
    branch: &str,
    old: &str,
    new: &str,
    answer: Answer,
) -> support::forge_listener::Reply {
    let line = format!("{old} {new} refs/heads/{branch}\0 report-status side-band-64k agent=git/2");
    let body = format!("{}0000PACK....", pkt(&line));
    let request = format!(
        "POST /acme/app.git/git-receive-pack HTTP/1.1\r\nHost: github.com\r\nAuthorization: {}\r\nContent-Type: application/x-git-receive-pack-request\r\nContent-Length: {}\r\n\r\n{body}",
        basic("x-access-token", w.sentinel.expose()),
        body.len()
    );
    let ca = purlis_core::forge::proxy::listener::ChatCa::new(&w.proxy.terminable_hosts(&w.chat))
        .unwrap();
    w.fake.answer(answer);
    support::forge_listener::tls(w, &ca, "github.com", 443, "github.com", request.as_bytes())
        .expect("a handshake")
}

#[test]
fn a_refused_or_failed_push_leaves_no_record_on_disk_and_an_accepted_one_leaves_it() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    let zero = "0".repeat(40);
    let file = createdbranches::file_in(config.path());

    // The forge says `ng`.
    let ng = Answer::ok(&format!(
        "{}{}0000",
        pkt("unpack ok\n"),
        pkt("ng refs/heads/mine rejected\n")
    ));
    assert_eq!(push_through(&w, "mine", &zero, ng), 200);
    assert!(!file.exists(), "an ng is no creation");

    // The forge answers with an error status, even with an `ok` body.
    let ok = || {
        Answer::ok(&format!(
            "{}{}0000",
            pkt("unpack ok\n"),
            pkt("ok refs/heads/mine\n")
        ))
    };
    assert_eq!(push_through(&w, "mine", &zero, ok().status(502)), 502);
    assert!(!file.exists(), "a non-200 is no creation");
    assert!(own(config.path(), "chat-listener").is_empty());

    // The forge takes it: the record is on disk, under this chat's id only.
    assert_eq!(push_through(&w, "mine", &zero, ok()), 200);
    assert_eq!(own(config.path(), "chat-listener"), ["mine"]);
    assert!(own(config.path(), CHAT).is_empty());
}

#[test]
fn a_create_that_differs_from_a_branch_the_forge_has_only_in_case_is_refused() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    w.proxy.note_advertised(
        &w.chat,
        "github.com",
        "acme/app",
        vec!["main".into(), "fix".into()],
    );
    let ok = Answer::ok(&format!(
        "{}{}0000",
        pkt("unpack ok\n"),
        pkt("ok refs/heads/Fix\n")
    ));

    assert_eq!(push_through(&w, "Fix", &"0".repeat(40), ok), 403);
    assert!(w.fake.requests().is_empty(), "never sent");
    assert!(!createdbranches::file_in(config.path()).exists());
}

/// The forge's answer to a push that makes `branch`.
fn accepted(branch: &str) -> Answer {
    Answer::ok(&format!(
        "{}{}0000",
        pkt("unpack ok\n"),
        pkt(&format!("ok refs/heads/{branch}\n"))
    ))
}

#[test]
fn the_normal_case_a_chat_creates_a_branch_through_the_route_and_owns_it_next_run() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    assert_eq!(
        push_through(&w, "mine", &"0".repeat(40), accepted("mine")),
        200
    );

    assert_eq!(own(config.path(), "chat-listener"), ["mine"]);
    // The line names the forge's repo id and the tip the push made the branch at.
    let text = std::fs::read_to_string(createdbranches::file_in(config.path())).unwrap();
    let line: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(line["repo_id"], REPO_ID);
    assert_eq!(line["project_id"], PROJECT);
    assert_eq!(line["tip"], "1".repeat(40));
}

#[test]
fn a_repo_made_again_at_the_same_path_owns_nothing_the_old_repo_recorded() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    push_through(&w, "mine", &"0".repeat(40), accepted("mine"));
    assert_eq!(own(config.path(), "chat-listener"), ["mine"]);

    // Same host and path, another forge id.
    let again =
        createdbranches::created_by(config.path(), PROJECT, "chat-listener", HOST, REPO_ID + 1);
    assert!(again.is_empty());
}

#[test]
fn a_different_project_at_the_same_path_owns_nothing_the_first_recorded() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    push_through(&w, "mine", &"0".repeat(40), accepted("mine"));

    // Another project (another stable id), whatever path it sits at, with the same chat id.
    let other = createdbranches::created_by(
        config.path(),
        "01J9ZQ3V5N8X4T2K7M6P0R1S2Q",
        "chat-listener",
        HOST,
        REPO_ID,
    );
    assert!(other.is_empty());
}

#[test]
fn a_branch_deleted_and_made_again_by_someone_else_is_not_the_chats() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    push_through(&w, "mine", &"0".repeat(40), accepted("mine"));
    assert_eq!(own(config.path(), "chat-listener"), ["mine"]);

    // The forge's next advertisement for a push does not list `mine`: it was deleted. A
    // colleague makes the name again outside the route; the route never sees that.
    w.proxy
        .note_advertised(&w.chat, HOST, REPO, vec!["main".into()]);
    assert!(own(config.path(), "chat-listener").is_empty());

    // The chat's own later update of the name is refused, though the forge has it again.
    w.proxy
        .note_advertised(&w.chat, HOST, REPO, vec!["main".into(), "mine".into()]);
    assert_eq!(
        push_through(&w, "mine", &"2".repeat(40), accepted("mine")),
        403
    );
    assert!(own(config.path(), "chat-listener").is_empty());
}

fn tip_of(config: &Path, chat: &str) -> Option<String> {
    createdbranches::tip_of(config, PROJECT, chat, HOST, REPO_ID, "mine")
}

#[test]
fn consecutive_pushes_of_a_branch_the_chat_made_are_allowed_each_from_the_tip_before() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    let (one, two, three) = ("1".repeat(40), "2".repeat(40), "3".repeat(40));
    assert_eq!(
        push_through(&w, "mine", &"0".repeat(40), accepted("mine")),
        200
    );
    assert_eq!(tip_of(config.path(), "chat-listener"), Some(one.clone()));

    // Each confirmed push moves the tip the next update must name.
    assert_eq!(push_moving(&w, "mine", &one, &two, accepted("mine")), 200);
    assert_eq!(tip_of(config.path(), "chat-listener"), Some(two.clone()));
    assert_eq!(push_moving(&w, "mine", &two, &three, accepted("mine")), 200);
    assert_eq!(tip_of(config.path(), "chat-listener"), Some(three));

    // A push the forge refused moves nothing: the old tip stays the one to name.
    let ng = Answer::ok(&format!(
        "{}{}0000",
        pkt("unpack ok\n"),
        pkt("ng refs/heads/mine rejected\n")
    ));
    assert_eq!(push_moving(&w, "mine", &"3".repeat(40), &one, ng), 200);
    assert_eq!(tip_of(config.path(), "chat-listener"), Some("3".repeat(40)));
}

#[test]
fn a_chat_deletes_no_branch_not_even_one_it_made_at_the_tip_it_last_pushed() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    let (one, two, zero) = ("1".repeat(40), "2".repeat(40), "0".repeat(40));
    push_through(&w, "mine", &zero, accepted("mine"));
    assert_eq!(push_moving(&w, "mine", &one, &two, accepted("mine")), 200);

    // From the tip it last pushed, even its own branch is not deleted; a stale or foreign tip is
    // refused alike, and none of them is sent.
    let requests = w.fake.requests().len();
    for old in [&two, &one, &"3".repeat(40)] {
        assert_eq!(push_moving(&w, "mine", old, &zero, accepted("mine")), 403);
    }

    // A branch another chat made, or one with no record, is not the chat's to delete either.
    createdbranches::record(config.path(), PROJECT, OTHER, &[made("theirs")]).unwrap();
    for branch in ["theirs", "colleague"] {
        let old = "1".repeat(40);
        assert_eq!(
            push_moving(&w, branch, &old, &zero, accepted(branch)),
            403,
            "{branch}"
        );
    }
    assert_eq!(w.fake.requests().len(), requests, "never sent");
}

#[test]
fn an_update_of_a_branch_someone_else_made_again_is_refused_though_the_advertisement_lists_it() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    push_through(&w, "mine", &"0".repeat(40), accepted("mine"));

    // A colleague deleted `mine` and made it again, at another commit, outside the route. The
    // chat's next advertisement lists the name, so no `gone` line is ever written for it.
    w.proxy
        .note_advertised(&w.chat, HOST, REPO, vec!["main".into(), "mine".into()]);
    let theirs = "3".repeat(40);
    let requests = w.fake.requests().len();
    assert_eq!(
        push_moving(&w, "mine", &theirs, &"4".repeat(40), accepted("mine")),
        403
    );
    assert_eq!(w.fake.requests().len(), requests, "never sent");
    assert_eq!(tip_of(config.path(), "chat-listener"), Some("1".repeat(40)));
}

#[test]
fn an_update_of_a_branch_someone_else_made_again_is_refused_when_no_advertisement_was_read() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    push_through(&w, "mine", &"0".repeat(40), accepted("mine"));

    // The chat sends receive-pack without reading the advertisement again; nothing ends the
    // record, and the forge's `mine` is another commit.
    let requests = w.fake.requests().len();
    assert_eq!(
        push_moving(
            &w,
            "mine",
            &"3".repeat(40),
            &"4".repeat(40),
            accepted("mine")
        ),
        403
    );
    assert_eq!(w.fake.requests().len(), requests, "never sent");
}

#[test]
fn a_push_another_chat_made_through_the_route_moves_the_tip_under_this_live_run() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    push_through(&w, "mine", &"0".repeat(40), accepted("mine"));

    // Another chat's confirmed push of the name is the last line for it.
    createdbranches::record(
        config.path(),
        PROJECT,
        OTHER,
        &[CreatedBranch {
            tip: "3".repeat(40),
            ..made("mine")
        }],
    )
    .unwrap();

    // This run read the record at its start, and still names the tip it left: refused.
    assert_eq!(
        push_moving(
            &w,
            "mine",
            &"1".repeat(40),
            &"4".repeat(40),
            accepted("mine")
        ),
        403
    );
    assert_eq!(tip_of(config.path(), "chat-listener"), None);
}

#[test]
fn a_tip_line_that_is_garbled_or_missing_fails_closed() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    push_through(&w, "mine", &"0".repeat(40), accepted("mine"));
    let file = createdbranches::file_in(config.path());

    for tip in ["not-a-commit", "", &"1".repeat(39)] {
        // The chat's own latest line for the branch, with a tip that does not read.
        let line = serde_json::json!({
            "project_id": PROJECT, "chat": "chat-listener", "host": HOST, "repo": REPO,
            "repo_id": REPO_ID, "branch": "mine", "tip": tip,
        });
        let mut text = std::fs::read_to_string(&file).unwrap();
        text.push_str(&format!("{line}\n"));
        std::fs::write(&file, text).unwrap();

        // Even naming the tip the chat's last good push left: no tip reads, no update.
        assert_eq!(
            push_moving(
                &w,
                "mine",
                &"1".repeat(40),
                &"2".repeat(40),
                accepted("mine")
            ),
            403,
            "tip {tip:?}"
        );
    }

    // A line with no tip key at all is the same.
    let line = serde_json::json!({
        "project_id": PROJECT, "chat": "chat-listener", "host": HOST, "repo": REPO,
        "repo_id": REPO_ID, "branch": "mine",
    });
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str(&format!("{line}\n"));
    std::fs::write(&file, text).unwrap();
    assert_eq!(
        push_moving(
            &w,
            "mine",
            &"1".repeat(40),
            &"2".repeat(40),
            accepted("mine")
        ),
        403
    );
}

#[test]
fn a_refused_push_is_answered_in_gits_own_words_and_never_reaches_the_forge() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let w = World::recording_to(config.path().to_owned());
    // A branch no record says the chat made: an update of it is refused.
    let reply = push_reply(
        &w,
        "theirs",
        &"3".repeat(40),
        &"4".repeat(40),
        accepted("theirs"),
    );
    assert_eq!(reply.status, 200, "git reads a report-status, not an error");
    assert_eq!(
        reply.header("content-type"),
        Some("application/x-git-receive-pack-result")
    );
    // Side-band channel 1 carries the report: `unpack ok` and one `ng` with purlis's reason.
    assert_eq!(reply.body[4], 1, "the side-band the client asked for");
    let text = reply.text();
    assert!(text.contains("unpack ok\n"), "{text}");
    assert!(
        text.contains(
            "ng refs/heads/theirs purlis: this branch is not one this chat made, so the push is refused; ask the person to push it\n"
        ),
        "{text}"
    );
    assert!(reply.body.ends_with(b"0000"));
    // Nothing went to the forge, and what git is told carries no credential, sentinel or path.
    assert!(w.fake.requests().is_empty(), "never sent");
    assert!(!text.contains(w.sentinel.expose()), "{text}");
    assert!(!text.contains(config.path().to_str().unwrap()), "{text}");
    assert!(!createdbranches::file_in(config.path()).exists());
}

#[test]
fn a_refused_advertisement_is_an_err_line_git_prints_and_the_forge_never_sees_it() {
    purlis_core::unsteered!();
    let w = World::new();
    let request = format!(
        "GET /acme/other.git/info/refs?service=git-receive-pack HTTP/1.1\r\nHost: github.com\r\nAuthorization: {}\r\n\r\n",
        basic("x-access-token", w.sentinel.expose()),
    );
    let ca = purlis_core::forge::proxy::listener::ChatCa::new(&w.proxy.terminable_hosts(&w.chat))
        .unwrap();
    let reply =
        support::forge_listener::tls(&w, &ca, "github.com", 443, "github.com", request.as_bytes())
            .expect("a handshake");
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.header("content-type"),
        Some("application/x-git-receive-pack-advertisement")
    );
    let text = reply.text();
    assert!(
        text.starts_with("001f# service=git-receive-pack\n0000"),
        "{text}"
    );
    assert!(
        text.ends_with("ERR purlis: this repo is not one of this chat's\n"),
        "{text}"
    );
    assert!(w.fake.requests().is_empty(), "never sent");
}

/// The chat in [`World`] (`chat-listener`) asked for a task, which made `work` and ended.
fn a_world_whose_chat_dispatched_the_task_that_made(
    config: &Path,
    project: &Path,
    branch: &str,
) -> World {
    use purlis_core::dispatchrecord::{self, Asker, ChatRef, Mode, Opening, Place, Worker};
    // As the app's own project path: a store is read through no link.
    let project = &std::fs::canonicalize(project).unwrap();
    let chat = |id: &str| ChatRef {
        chat: 1,
        id: Some(id.to_owned()),
        name: "a chat".to_owned(),
        persona: None,
    };
    let opening = |asker: &str, worker: &str| Opening {
        mode: Mode::Task,
        asker: Asker {
            chat: chat(asker),
            ..Asker::default()
        },
        persona: None,
        worker: Worker {
            chat: chat(worker),
            ..Worker::default()
        },
        task: None,
        place: Place::default(),
        brief: "do it".to_owned(),
        report_owed: true,
    };
    let now = "2026-11-23T10:00:00Z".parse().unwrap();
    dispatchrecord::open(project, opening("chat-listener", "task-chat"), now).unwrap();
    // The task made the branch through the route, then ended.
    createdbranches::record(
        config,
        PROJECT,
        "task-chat",
        &[CreatedBranch {
            tip: "1".repeat(40),
            ..made(branch)
        }],
    )
    .unwrap();
    World::recording_with_dispatches(config.to_owned(), project.to_owned())
}

#[test]
fn a_chat_may_push_the_branch_a_task_it_dispatched_made_from_the_recorded_tip_and_then_owns_it() {
    purlis_core::unsteered!();
    let (config, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let w = a_world_whose_chat_dispatched_the_task_that_made(
        config.path(),
        project.path(),
        "task/work",
    );
    let (one, two) = ("1".repeat(40), "2".repeat(40));
    assert_eq!(
        push_moving(&w, "task/work", &one, &two, accepted("task/work")),
        200
    );
    // The confirmed push is recorded under the dispatcher: the task owns nothing of it any more.
    assert_eq!(
        createdbranches::owner_of(config.path(), HOST, REPO_ID, "task/work"),
        Some((PROJECT.to_owned(), "chat-listener".to_owned(), two.clone()))
    );
    let task_may = createdbranches::tip_for(
        config.path(),
        PROJECT,
        "task-chat",
        HOST,
        REPO_ID,
        "task/work",
        &|_| false,
    );
    assert_eq!(
        task_may, None,
        "a task does not move what its dispatcher took up"
    );
    // And the dispatcher goes on from the new tip.
    assert_eq!(
        push_moving(
            &w,
            "task/work",
            &two,
            &"3".repeat(40),
            accepted("task/work")
        ),
        200
    );
}

#[test]
fn a_chat_may_not_push_a_tasks_branch_from_a_stale_tip_or_a_branch_of_an_undispatched_chat() {
    purlis_core::unsteered!();
    let (config, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let w = a_world_whose_chat_dispatched_the_task_that_made(
        config.path(),
        project.path(),
        "task/work",
    );
    // A stale tip: the task's recorded tip is `1…`, the chat names another.
    let requests = w.fake.requests().len();
    assert_eq!(
        push_moving(
            &w,
            "task/work",
            &"9".repeat(40),
            &"2".repeat(40),
            accepted("task/work")
        ),
        REFUSED
    );
    // A branch whose owner no dispatch record ties to the chat.
    createdbranches::record(
        config.path(),
        PROJECT,
        "stranger",
        &[CreatedBranch {
            tip: "5".repeat(40),
            ..made("task/theirs")
        }],
    )
    .unwrap();
    assert_eq!(
        push_moving(
            &w,
            "task/theirs",
            &"5".repeat(40),
            &"6".repeat(40),
            accepted("task/theirs")
        ),
        REFUSED
    );
    assert_eq!(w.fake.requests().len(), requests, "never sent");
}

#[test]
fn a_task_does_not_move_the_branch_of_the_chat_that_dispatched_it() {
    purlis_core::unsteered!();
    let (config, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let _w = a_world_whose_chat_dispatched_the_task_that_made(
        config.path(),
        project.path(),
        "task/work",
    );
    // The dispatcher's own branch, as the task would see it: the link runs one way.
    createdbranches::record(
        config.path(),
        PROJECT,
        "chat-listener",
        &[CreatedBranch {
            tip: "7".repeat(40),
            ..made("asker/own")
        }],
    )
    .unwrap();
    let records =
        purlis_core::dispatchrecord::list(&std::fs::canonicalize(project.path()).unwrap());
    let tip = createdbranches::tip_for(
        config.path(),
        PROJECT,
        "task-chat",
        HOST,
        REPO_ID,
        "asker/own",
        &|owner| purlis_core::dispatchrecord::dispatched_by(&records, "task-chat", owner),
    );
    assert_eq!(tip, None);
}
