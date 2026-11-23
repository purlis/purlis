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
    createdbranches::record(
        config.path(),
        PROJECT,
        CHAT,
        &[made("task/one")],
    )
    .unwrap();

    assert_eq!(own(config.path(), CHAT), ["task/one"]);
    // A resumed chat has another id: it owns none, and may fast-forward the branch only.
    assert!(own(config.path(), OTHER).is_empty());
}

#[test]
fn another_projects_chat_with_the_same_id_and_an_old_line_with_no_project_id_own_nothing() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    createdbranches::record(
        config.path(),
        PROJECT,
        CHAT,
        &[made("task/one")],
    )
    .unwrap();
    let file = createdbranches::file_in(config.path());
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str(&format!(
        "{}\n",
        serde_json::json!({"project": "/work/project", "chat": CHAT, "host": HOST, "repo": REPO, "branch": "task/old"})
    ));
    std::fs::write(&file, text).unwrap();

    assert_eq!(own(config.path(), CHAT), ["task/one"]);
    let elsewhere =
        createdbranches::created_by(config.path(), "01J9ZQ3V5N8X4T2K7M6P0R1S2Q", CHAT, HOST, REPO_ID);
    assert!(elsewhere.is_empty());
}

fn record_line(chat: &str, branch: &str) -> String {
    format!(
        "{}\n",
        serde_json::json!({"project_id": PROJECT, "chat": chat, "host": HOST, "repo": REPO, "repo_id": REPO_ID, "branch": branch})
    )
}

#[test]
fn a_line_forged_for_another_chats_id_is_not_own_for_this_chat() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    createdbranches::record(
        config.path(),
        PROJECT,
        OTHER,
        &[made("task/theirs")],
    )
    .unwrap();
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
    let new = "1".repeat(40);
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
        .status
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
    let again = createdbranches::created_by(
        config.path(),
        PROJECT,
        "chat-listener",
        HOST,
        REPO_ID + 1,
    );
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
    w.proxy.note_advertised(&w.chat, HOST, REPO, vec!["main".into()]);
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
