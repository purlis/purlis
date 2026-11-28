// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`guards-git-claimed-folder`, and the one-writer mutex beneath it** (M75-25, #7034). The owner
//! records are sealed for real, in a machine of the test's own, and every refusal that proves the
//! rule runs under `strict`, written out (ADR 0125). The branch folders are real: a clone in
//! workspace `alpha`, its folders cut with git.

use std::path::{Path, PathBuf};

use super::*;
use crate::gitbroker::{self, Asker};
use crate::hookwire::{Answer, ClaimAct, GitAsk, GitWork};
use crate::policy::force::tests::{HEADER, Machine, STRICT, git, machine, plane as plane_with};
use crate::policy::owner::{Records, record_dispatch, record_owner};
use crate::repocmd::Say;
use crate::secrets::answers;

/// The person's first chat, its task, and the task's own task.
const ROOT: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S00";
const TASK: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S0A";
const GRANDCHILD: &str = "01J9ZQ3V5N8X4T2K7M6P0R1SAA";
/// A chat of another line.
const STRANGER: &str = "01J9ZQ3V5N8X4T2K7M6P0R1SZZ";

/// `widget`'s branch folder `p1`, in workspace `alpha`.
fn p1() -> Located {
    Located {
        workspace: "alpha".to_owned(),
        repo: "widget".to_owned(),
        piece: "p1".to_owned(),
    }
}

fn facts() -> ChatFacts {
    ChatFacts {
        workspace: Some("alpha".to_owned()),
        persona: None,
        sandboxed: false,
        level: None,
        attended: true,
        harness_mode: None,
    }
}

/// What a brokered act tells the policy: the broker's own value, which runs no hooks.
const BROKERED: GitFacts = gitbroker::BROKERED;

/// A plane committing `toml` as its `purlis.toml`, registered as the app opens it, with the line
/// `ROOT` → `TASK` → `GRANDCHILD` dispatched, and `widget` cloned in workspace `alpha` with its
/// branch folder `p1` cut.
fn plane(m: &Machine, toml: &str) -> PathBuf {
    let root = plane_with(m, &[("purlis.toml", toml)]);
    answers::register(&root).expect("registered");
    assert!(record_dispatch(&root, TASK, ROOT).expect("recorded"));
    assert!(record_dispatch(&root, GRANDCHILD, TASK).expect("recorded"));
    clone_in(&root, "alpha");
    cut(&root, "alpha", "p1");
    root
}

/// `widget` cloned in workspace `ws`: a repository of its own, with one commit.
fn clone_in(root: &Path, ws: &str) {
    let clone = root.join("workspaces").join(ws).join("widget");
    std::fs::create_dir_all(&clone).expect("a clone");
    git(&clone, &["init", "-q"]);
    std::fs::write(clone.join("README.md"), "widget\n").expect("a file");
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-q", "-m", "first"]);
}

/// `widget`'s branch folder `piece` cut in workspace `ws`, on a branch of its name.
fn cut(root: &Path, ws: &str, piece: &str) -> PathBuf {
    let folder = root
        .join("workspaces")
        .join(ws)
        .join(".worktrees/widget")
        .join(piece);
    let clone = root.join("workspaces").join(ws).join("widget");
    let path = folder.display().to_string();
    git(&clone, &["worktree", "add", "-q", "-b", piece, &path]);
    folder
}

/// The chat a record names as the owner of the folder that stands at `at` now.
fn holder_of(root: &Path, at: &Located) -> Option<String> {
    let path = folder_of(root, at);
    let made = made_of(&path)?;
    Records::read(root)
        .holder(&Owned::Folder {
            path: &path,
            made: &made,
        })
        .map(str::to_owned)
}

/// How many folder owner records the plane's store holds, whatever they name.
fn folder_records(root: &Path) -> usize {
    answers::standing(root)
        .entries
        .iter()
        .filter(|entry| entry.kind == answers::OWNER && entry.text.contains("\"owns\""))
        .count()
}

/// `TASK` claimed `p1` first.
fn claimed_by_task(root: &Path) {
    let path = folder_of(root, &p1());
    let made = made_of(&path).expect("p1 stands");
    let owned = Owned::Folder {
        path: &path,
        made: &made,
    };
    assert!(record_owner(root, &owned, TASK).expect("recorded"));
}

fn judged(root: &Path, chat: &str, act: Act) -> Result<Cleared, String> {
    let facts = facts();
    let asking = Asking {
        chat: 4,
        id: Some(chat),
        facts: &facts,
    };
    judge(root, &asking, &p1(), act, BROKERED)
}

#[test]
fn a_commit_into_a_folder_another_chat_claims_names_guards_git_claimed_folder() {
    let m = machine();
    let root = plane(&m, STRICT);
    claimed_by_task(&root);
    for act in [Act::Commit, Act::Update, Act::Remove] {
        // The owner itself writes.
        assert!(judged(&root, TASK, act).is_ok(), "{act:?}");
        // Its asker ("task" to it), its own task ("asker") and a stranger ("other") do not.
        for chat in [ROOT, GRANDCHILD, STRANGER] {
            let refused = judged(&root, chat, act).expect_err("another chat's folder");
            assert!(refused.contains("`guards-git-claimed-folder`"), "{refused}");
            assert!(refused.contains("widget's branch folder 'p1'"), "{refused}");
        }
    }
    // A push is not this rule's: it reads its forge branch's owner (deferred, #7034).
    assert!(judged(&root, STRANGER, Act::Push).is_ok());
}

#[test]
fn a_guards_git_refusal_names_its_id_level_and_owner() {
    let m = machine();
    let root = plane(&m, STRICT);
    claimed_by_task(&root);
    let refused = judged(&root, STRANGER, Act::Commit).expect_err("another chat's folder");
    let version = env!("CARGO_PKG_VERSION");
    assert!(
        refused.contains(&format!(
            "`guards-git-claimed-folder` (project, purlis default {version}) refuses it"
        )),
        "{refused}"
    );
    assert!(refused.contains("Nothing was changed"), "{refused}");
    assert!(refused.contains("purlis worktree add"), "{refused}");
}

#[test]
fn a_folder_no_record_names_is_decided_as_before_and_its_first_writer_is_recorded() {
    let m = machine();
    let root = plane(&m, STRICT);
    let holder = |root: &Path| holder_of(root, &p1());
    // "none": the rule does not fire, and nothing is recorded until the mutex is taken.
    let cleared = judged(&root, STRANGER, Act::Commit).expect("no one's folder");
    assert_eq!(holder(&root), None);
    cleared.record(&root);
    assert_eq!(holder(&root).as_deref(), Some(STRANGER));
    // The first record stands: a second chat's act is refused and records nothing.
    assert!(judged(&root, TASK, Act::Commit).is_err());
    judged(&root, STRANGER, Act::Commit)
        .expect("its own")
        .record(&root);
    assert_eq!(holder(&root).as_deref(), Some(STRANGER));
}

#[test]
fn a_project_rule_that_asks_about_a_commit_refuses_it_and_says_so() {
    let m = machine();
    let rule = format!(
        "{HEADER}@id(\"guards-git-ask-commits\") @ask(\"you\")\n\
         forbid (principal, action == Action::\"commit\", resource);\n"
    );
    let root = plane_with(&m, &[("purlis.toml", STRICT), ("policy.cedar", &rule)]);
    answers::register(&root).expect("registered");
    let refused = judged(&root, STRANGER, Act::Commit).expect_err("an ask is no allow");
    assert!(refused.contains("`guards-git-ask-commits`"), "{refused}");
    assert!(refused.contains("asks the person first"), "{refused}");
    assert!(refused.contains("Ask the person"), "{refused}");
}

/// A chat the app has open, as its git broker asks for it, standing in workspace `alpha`.
fn broker_asker(root: &Path, number: u32, id: &str, live: &[(&str, u32)]) -> Asker {
    Asker {
        chat: number,
        cwd: Some(root.join("workspaces/alpha")),
        persona: None,
        harnessed: true,
        unsandboxed: true,
        roots_git_held: false,
        unreadable: Vec::new(),
        config: None,
        id: Some(id.to_owned()),
        live: Some(
            live.iter()
                .map(|(id, chat)| crate::piececlaim::Live {
                    id: (*id).to_owned(),
                    chat: Some(*chat),
                    back: true,
                })
                .collect(),
        ),
        policy: None,
        decided_as: facts(),
    }
}

/// The guard's claim of `p1` for plain git's `act`.
fn claimed(root: &Path, asker: &Asker, act: ClaimAct) -> Answer {
    asked(
        root,
        asker,
        "alpha",
        GitWork::WorktreeClaim {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            act,
            amend: false,
            skip_hooks: false,
        },
    )
}

/// What the broker answers `asker` for `work` in workspace `ws`.
fn asked(root: &Path, asker: &Asker, ws: &str, work: GitWork) -> Answer {
    gitbroker::answer(
        root,
        asker,
        &GitAsk {
            chat: 0,
            workspace: ws.to_owned(),
            alive: false,
            work,
        },
        &crate::worktree::git::Isolated::default(),
        jiff::Timestamp::now(),
    )
}

/// The chat's own `purlis worktree done` in `p1`.
fn done_in_p1(root: &Path, asker: &Asker) -> Answer {
    asked(
        root,
        asker,
        "alpha",
        GitWork::WorktreeDone {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
        },
    )
}

fn held_with(answer: &Answer) -> Option<String> {
    match answer {
        Answer::Said { lines, code } if *code == crate::hookwire::CLAIM_HELD => {
            let [Say::Fail(why)] = lines.as_slice() else {
                panic!("{lines:?}");
            };
            Some(why.to_string())
        }
        _ => None,
    }
}

#[test]
fn the_folder_mutex_holds_under_guards_git_off() {
    let m = machine();
    let off = format!("{STRICT}\"guards.git\" = \"off\"\n");
    let root = plane(&m, &off);
    let both = [(TASK, 4), (STRANGER, 9)];
    // TASK writes first, takes the folder, and is recorded as its owner.
    let may = claimed(
        &root,
        &broker_asker(&root, 4, TASK, &both),
        ClaimAct::Commit,
    );
    assert_eq!(held_with(&may), None, "{may:?}");
    assert_eq!(holder_of(&root, &p1()).as_deref(), Some(TASK));
    // With `guards.git` off, the rule is lifted, but a second live chat is still refused by the
    // mutex, which names the holder and no rule.
    let held = claimed(
        &root,
        &broker_asker(&root, 9, STRANGER, &both),
        ClaimAct::Commit,
    );
    let why = held_with(&held).expect("the mutex holds in every mode");
    assert!(why.contains("claimed by chat 4"), "{why}");
    assert!(!why.contains("guards-git-claimed-folder"), "{why}");
    // TASK ended: the mutex lets the next chat take the folder, and `off` lifts the rule.
    let alone = [(STRANGER, 9)];
    let goes = claimed(
        &root,
        &broker_asker(&root, 9, STRANGER, &alone),
        ClaimAct::Commit,
    );
    assert_eq!(held_with(&goes), None, "{goes:?}");
}

#[test]
fn under_strict_an_ended_owners_folder_is_refused_by_the_rule_until_its_done() {
    let m = machine();
    let root = plane(&m, STRICT);
    let both = [(TASK, 4), (STRANGER, 9)];
    let may = claimed(
        &root,
        &broker_asker(&root, 4, TASK, &both),
        ClaimAct::Update,
    );
    assert_eq!(held_with(&may), None, "{may:?}");
    // TASK ended without its `done`: no live chat holds the folder, so the mutex would let
    // STRANGER take it, and the rule refuses it, answered as the one code a guard of any version
    // refuses on. The record stands until its owner's `done` or the folder's removal (Q339.9).
    let alone = [(STRANGER, 9)];
    let refused = claimed(
        &root,
        &broker_asker(&root, 9, STRANGER, &alone),
        ClaimAct::Commit,
    );
    let why = held_with(&refused).expect("the rule refuses");
    assert!(why.contains("`guards-git-claimed-folder`"), "{why}");
    assert!(why.contains("runs `purlis worktree done` there"), "{why}");
    assert!(!why.contains("hand this one over"), "{why}");
    // Refused before the mutex: STRANGER took nothing, so TASK, back, still writes.
    let back = claimed(
        &root,
        &broker_asker(&root, 4, TASK, &both),
        ClaimAct::Commit,
    );
    assert_eq!(held_with(&back), None, "{back:?}");
}

/// The review of #8010, F1: a claim names its folder in the chat's own words, so only a branch
/// folder of the asking chat's own workspace, standing on disk, gets an owner record. The mutex
/// answers each of these asks as it did.
#[test]
fn a_claim_for_another_workspace_records_nothing_and_the_mutex_answers_as_before() {
    let m = machine();
    let root = plane(&m, STRICT);
    clone_in(&root, "beta");
    cut(&root, "beta", "p1");
    let both = [(TASK, 4), (STRANGER, 9)];
    // TASK stands in `alpha` and claims `beta`'s p1: the mutex takes it as before...
    let in_beta = |asker: &Asker| {
        asked(
            &root,
            asker,
            "beta",
            GitWork::WorktreeClaim {
                repo: "widget".to_owned(),
                piece: "p1".to_owned(),
                act: ClaimAct::Commit,
                amend: false,
                skip_hooks: false,
            },
        )
    };
    let may = in_beta(&broker_asker(&root, 4, TASK, &both));
    assert_eq!(held_with(&may), None, "{may:?}");
    // ...and no owner is recorded for it.
    let beta = Located {
        workspace: "beta".to_owned(),
        ..p1()
    };
    assert_eq!(holder_of(&root, &beta), None);
    assert_eq!(folder_records(&root), 0);
    // The mutex still holds STRANGER out while TASK lives, naming no rule.
    let held = in_beta(&broker_asker(&root, 9, STRANGER, &both));
    let why = held_with(&held).expect("the mutex holds");
    assert!(why.contains("claimed by chat 4"), "{why}");
    assert!(!why.contains("guards-git-claimed-folder"), "{why}");
}

#[test]
fn a_claim_for_a_folder_that_is_not_there_records_nothing() {
    let m = machine();
    let root = plane(&m, STRICT);
    let asker = broker_asker(&root, 4, TASK, &[(TASK, 4)]);
    let p9 = asked(
        &root,
        &asker,
        "alpha",
        GitWork::WorktreeClaim {
            repo: "widget".to_owned(),
            piece: "p9".to_owned(),
            act: ClaimAct::Commit,
            amend: false,
            skip_hooks: false,
        },
    );
    assert_eq!(held_with(&p9), None, "{p9:?}");
    assert_eq!(folder_records(&root), 0);
    // Cut later under that name, it is no one's.
    cut(&root, "alpha", "p9");
    let p9 = Located {
        piece: "p9".to_owned(),
        ..p1()
    };
    assert_eq!(holder_of(&root, &p9), None);
}

#[test]
fn a_claim_for_a_folder_that_is_no_branch_folder_records_nothing() {
    let m = machine();
    let root = plane(&m, STRICT);
    let pieces = root.join("workspaces/alpha/.worktrees/widget");
    // A plain directory, a link to a real branch folder, and a folder of another clone.
    std::fs::create_dir_all(pieces.join("plain")).expect("a directory");
    std::os::unix::fs::symlink(pieces.join("p1"), pieces.join("linked")).expect("a link");
    clone_in(&root, "beta");
    let elsewhere = cut(&root, "beta", "p2");
    std::fs::rename(&elsewhere, pieces.join("moved")).expect("moved in");
    let asker = broker_asker(&root, 4, TASK, &[(TASK, 4)]);
    for piece in ["plain", "linked", "moved"] {
        let may = asked(
            &root,
            &asker,
            "alpha",
            GitWork::WorktreeClaim {
                repo: "widget".to_owned(),
                piece: piece.to_owned(),
                act: ClaimAct::Commit,
                amend: false,
                skip_hooks: false,
            },
        );
        assert_eq!(held_with(&may), None, "{piece}: {may:?}");
        assert_eq!(folder_records(&root), 0, "{piece}");
    }
}

/// The person's ruling Q339.9: the owner's `purlis worktree done` ends its record, and the next
/// chat to write there owns the folder.
#[test]
fn done_by_the_owner_ends_its_record_and_the_next_chat_owns_the_folder() {
    let m = machine();
    let root = plane(&m, STRICT);
    let both = [(TASK, 4), (STRANGER, 9)];
    let task = broker_asker(&root, 4, TASK, &both);
    assert_eq!(held_with(&claimed(&root, &task, ClaimAct::Commit)), None);
    assert_eq!(holder_of(&root, &p1()).as_deref(), Some(TASK));
    let said = done_in_p1(&root, &task);
    assert!(
        matches!(&said, Answer::Said { code: 0, lines } if lines.is_empty()),
        "{said:?}"
    );
    assert_eq!(holder_of(&root, &p1()), None);
    assert_eq!(folder_records(&root), 0);
    // TASK ended: STRANGER writes, and is the owner from then on.
    let stranger = broker_asker(&root, 9, STRANGER, &[(STRANGER, 9)]);
    let goes = claimed(&root, &stranger, ClaimAct::Commit);
    assert_eq!(held_with(&goes), None, "{goes:?}");
    assert_eq!(holder_of(&root, &p1()).as_deref(), Some(STRANGER));
}

/// Q339.9: nobody else can end the record, by a `done` of its own or by removing the folder.
#[test]
fn done_or_removal_by_another_chat_ends_nothing() {
    let m = machine();
    let root = plane(&m, STRICT);
    claimed_by_task(&root);
    // TASK has ended. Its asker, its own task and a stranger each say `done` there...
    for (chat, id) in [(1, ROOT), (5, GRANDCHILD), (9, STRANGER)] {
        let other = broker_asker(&root, chat, id, &[(id, chat)]);
        let said = done_in_p1(&root, &other);
        assert!(matches!(&said, Answer::Said { code: 0, .. }), "{said:?}");
        assert_eq!(holder_of(&root, &p1()).as_deref(), Some(TASK), "{id}");
        // ...and asks for it removed, which the rule refuses before anything is run.
        let removal = asked(
            &root,
            &other,
            "alpha",
            GitWork::WorktreeRemove {
                repo: "widget".to_owned(),
                piece: "p1".to_owned(),
                delete_branch: false,
            },
        );
        let Answer::No { why } = &removal else {
            panic!("{removal:?}");
        };
        assert!(why.contains("`guards-git-claimed-folder`"), "{why}");
        assert!(folder_of(&root, &p1()).is_dir(), "{id}");
        assert_eq!(holder_of(&root, &p1()).as_deref(), Some(TASK), "{id}");
    }
    // No word that a folder was removed ends its record while the folder stands.
    let path = folder_of(&root, &p1());
    assert_eq!(
        owner::end_of_a_removed_folder(&root, &path).expect("read"),
        0
    );
    assert_eq!(holder_of(&root, &p1()).as_deref(), Some(TASK));
}

/// Q339.9: a removed folder's records end, and a folder cut again under its name starts with no
/// owner, even where its record outlived it (removed where no app could end it).
#[test]
fn a_removed_folders_name_cut_again_starts_with_no_owner() {
    let m = machine();
    let root = plane(&m, STRICT);
    claimed_by_task(&root);
    // Removed by purlis: its record ends with it.
    crate::worktree::remove(&root, "alpha", "widget", "p1", false, false).expect("removed");
    assert_eq!(folder_records(&root), 0);
    let clone = root.join("workspaces/alpha/widget");
    git(&clone, &["branch", "-q", "-D", "p1"]);
    cut(&root, "alpha", "p1");
    assert_eq!(holder_of(&root, &p1()), None);
    // Removed by hand, so its record outlives it: the folder cut again is still no one's, and
    // STRANGER writes there under `strict`.
    claimed_by_task(&root);
    std::fs::remove_dir_all(folder_of(&root, &p1())).expect("removed by hand");
    git(&clone, &["worktree", "prune"]);
    git(&clone, &["branch", "-q", "-D", "p1"]);
    cut(&root, "alpha", "p1");
    assert_eq!(folder_records(&root), 1);
    assert_eq!(holder_of(&root, &p1()), None);
    assert!(judged(&root, STRANGER, Act::Commit).is_ok());
}

/// The review of #8010, F6: the brokered commit is decided with the app's record of the chat.
#[test]
fn a_brokered_commit_into_another_chats_folder_is_refused_by_the_rule() {
    let m = machine();
    let root = plane(&m, STRICT);
    claimed_by_task(&root);
    let folder = folder_of(&root, &p1());
    std::fs::write(folder.join("README.md"), "changed\n").expect("a change");
    let ask = crate::hookwire::CommitAsk {
        chat: 9,
        message: "a change".to_owned(),
        stage: crate::hookwire::Stage::Tracked,
        folder: None,
    };
    let stranger = Asker {
        cwd: Some(folder.clone()),
        ..broker_asker(&root, 9, STRANGER, &[(STRANGER, 9)])
    };
    let refused = gitbroker::commit::answer(
        &root,
        &stranger,
        &ask,
        &crate::worktree::git::Isolated::default(),
    );
    let Answer::No { why } = &refused else {
        panic!("{refused:?}");
    };
    assert!(why.contains("`guards-git-claimed-folder`"), "{why}");
    assert!(why.contains("will not commit in"), "{why}");
    assert_eq!(git(&folder, &["rev-list", "--count", "HEAD"]).trim(), "1");
}

/// F6: the brokered update is decided with the app's record of the chat, before any git runs.
#[test]
fn a_brokered_update_of_another_chats_folder_is_refused_by_the_rule() {
    let m = machine();
    let root = plane(&m, STRICT);
    claimed_by_task(&root);
    let stranger = broker_asker(&root, 9, STRANGER, &[(STRANGER, 9)]);
    let refused = asked(
        &root,
        &stranger,
        "alpha",
        GitWork::WorktreeUpdate {
            repo: "widget".to_owned(),
            piece: "p1".to_owned(),
            step: crate::hookwire::UpdateStep::Start { base: None },
        },
    );
    let Answer::No { why } = &refused else {
        panic!("{refused:?}");
    };
    assert!(why.contains("`guards-git-claimed-folder`"), "{why}");
    assert!(why.contains("will not update"), "{why}");
}

#[test]
fn on_a_file_system_that_ignores_case_another_spelling_of_the_folder_reads_its_owner() {
    let m = machine();
    let root = plane(&m, STRICT);
    claimed_by_task(&root);
    let shouted = Located {
        piece: "P1".to_owned(),
        ..p1()
    };
    let place = folder_of(&root, &p1());
    if !place.with_file_name("P1").exists() {
        return; // A file system that keeps case apart has no second spelling of `p1`.
    }
    assert_eq!(folder_of(&root, &shouted), place);
    let stranger = broker_asker(&root, 9, STRANGER, &[(STRANGER, 9)]);
    let refused = asked(
        &root,
        &stranger,
        "alpha",
        GitWork::WorktreeUpdate {
            repo: "widget".to_owned(),
            piece: "P1".to_owned(),
            step: crate::hookwire::UpdateStep::Start { base: None },
        },
    );
    let Answer::No { why } = &refused else {
        panic!("{refused:?}");
    };
    assert!(why.contains("`guards-git-claimed-folder`"), "{why}");
}
