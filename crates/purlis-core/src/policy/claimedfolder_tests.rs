// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`guards-git-claimed-folder`, and the one-writer mutex beneath it** (M75-25, #7034). The owner
//! records are sealed for real, in a machine of the test's own, and every refusal that proves the
//! rule runs under `strict`, written out (ADR 0125).

use std::path::{Path, PathBuf};

use super::*;
use crate::gitbroker::{self, Asker};
use crate::hookwire::{Answer, ClaimAct, GitAsk, GitWork};
use crate::policy::force::tests::{HEADER, Machine, STRICT, machine, plane as plane_with};
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

/// What a brokered act tells the policy.
const BROKERED: GitFacts = GitFacts {
    force: false,
    skip_hooks: false,
    brokered: true,
    amend: false,
};

/// A plane committing `toml` as its `purlis.toml`, registered as the app opens it, with the line
/// `ROOT` → `TASK` → `GRANDCHILD` dispatched and the workspace `alpha` made.
fn plane(m: &Machine, toml: &str) -> PathBuf {
    let root = plane_with(m, &[("purlis.toml", toml)]);
    answers::register(&root).expect("registered");
    assert!(record_dispatch(&root, TASK, ROOT).expect("recorded"));
    assert!(record_dispatch(&root, GRANDCHILD, TASK).expect("recorded"));
    std::fs::create_dir_all(root.join("workspaces/alpha/widget")).expect("a workspace");
    root
}

/// `TASK` claimed `p1` first.
fn claimed_by_task(root: &Path) {
    assert!(record_owner(root, &Owned::Folder(&folder_of(root, &p1())), TASK).expect("recorded"));
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
    let owned = folder_of(&root, &p1());
    let holder = |root: &Path| {
        Records::read(root)
            .holder(&Owned::Folder(&owned))
            .map(str::to_owned)
    };
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

/// A chat the app has open, as its git broker asks for it.
fn broker_asker(number: u32, id: &str, live: &[(&str, u32)]) -> Asker {
    Asker {
        chat: number,
        cwd: None,
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
        decided_as: Some(facts()),
    }
}

/// The guard's claim of `p1` for plain git's `act`.
fn claimed(root: &Path, asker: &Asker, act: ClaimAct) -> Answer {
    gitbroker::answer(
        root,
        asker,
        &GitAsk {
            chat: 0,
            workspace: "alpha".to_owned(),
            alive: false,
            work: GitWork::WorktreeClaim {
                repo: "widget".to_owned(),
                piece: "p1".to_owned(),
                act,
                amend: false,
                skip_hooks: false,
            },
        },
        &crate::worktree::git::Isolated::default(),
        jiff::Timestamp::now(),
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
    let may = claimed(&root, &broker_asker(4, TASK, &both), ClaimAct::Commit);
    assert_eq!(held_with(&may), None, "{may:?}");
    assert_eq!(
        Records::read(&root).holder(&Owned::Folder(&folder_of(&root, &p1()))),
        Some(TASK)
    );
    // With `guards.git` off, the rule is lifted, but a second live chat is still refused by the
    // mutex, which names the holder and no rule.
    let held = claimed(&root, &broker_asker(9, STRANGER, &both), ClaimAct::Commit);
    let why = held_with(&held).expect("the mutex holds in every mode");
    assert!(why.contains("claimed by chat 4"), "{why}");
    assert!(!why.contains("guards-git-claimed-folder"), "{why}");
    // TASK ended: the mutex lets the next chat take the folder, and `off` lifts the rule.
    let alone = [(STRANGER, 9)];
    let goes = claimed(&root, &broker_asker(9, STRANGER, &alone), ClaimAct::Commit);
    assert_eq!(held_with(&goes), None, "{goes:?}");
}

#[test]
fn under_strict_an_ended_owners_folder_is_refused_by_the_rule_and_never_taken() {
    let m = machine();
    let root = plane(&m, STRICT);
    let both = [(TASK, 4), (STRANGER, 9)];
    let may = claimed(&root, &broker_asker(4, TASK, &both), ClaimAct::Update);
    assert_eq!(held_with(&may), None, "{may:?}");
    // TASK ended: no live chat holds the folder, so the mutex would let STRANGER take it, and
    // the rule refuses it, answered as the one code a guard of any version refuses on.
    let alone = [(STRANGER, 9)];
    let refused = claimed(&root, &broker_asker(9, STRANGER, &alone), ClaimAct::Commit);
    let why = held_with(&refused).expect("the rule refuses");
    assert!(why.contains("`guards-git-claimed-folder`"), "{why}");
    // Refused before the mutex: STRANGER took nothing, so TASK, back, still writes.
    let back = claimed(&root, &broker_asker(4, TASK, &both), ClaimAct::Commit);
    assert_eq!(held_with(&back), None, "{back:?}");
}
