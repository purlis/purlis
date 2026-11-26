// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Running chats pick up a change** (M75-23, #7032; spec §9.5). A chat is started on a plane, and
//! the plane is then changed under it, in each direction: a tightening reaches it (at its next
//! decision, and as a mark that its compiled sandbox is out of date), a loosening does not.
//!
//! The decisions go through [`decide::for_chat`], the function the app's session host answers every
//! hook's ask with; the marks through [`AtStart`], the one the app keeps for each chat from its
//! start. Nothing here names the person's press except where a test plays it with an accepted copy
//! built by hand (as `force_tests` does).

use std::path::{Path, PathBuf};

use super::*;
use crate::enforcement::Resolved;
use crate::policy::decide::{self, ChatFacts, Outcome, Sealed};
use crate::policy::force::tests::{
    HEADER, Machine, STRICT, accepted, commit, holding, machine, plane, version,
};
use crate::policy::grants::{Grant, Never, Scope};
use crate::secrets::answers::{self, Entry};

const CHAT: &str = "01JCHAT";

/// A plane with one workspace, `web`, on a fresh machine.
fn a_plane(m: &Machine) -> PathBuf {
    plane(
        m,
        &[
            ("purlis.toml", STRICT),
            ("workspaces/web/README.md", "web\n"),
            ("workspaces/web/ledger/2026.csv", "1\n"),
        ],
    )
}

fn facts() -> Facts {
    Facts {
        chat: Some(CHAT.to_owned()),
        workspace: Some("web".to_owned()),
        persona: Some("builder".to_owned()),
        sandbox: "floor".to_owned(),
    }
}

fn started(root: &Path) -> AtStart {
    AtStart::take(root, facts(), false)
}

/// Why `at` is out of date over the plane at `root` now, read as the app reads it (once).
fn stale(at: &AtStart, root: &Path) -> Vec<Why> {
    at.out_of_date(&Shared::read(root))
}

/// The rule that forbids reading the ledger folder of `web`.
fn ban_on_the_ledger(root: &Path) -> String {
    format!(
        "{HEADER}@id(\"sandbox-no-ledger\") @level(\"project\")\n\
         forbid (principal, action == Action::\"read\", resource) when {{ \
         resource in Path::\"{}/workspaces/web/ledger\" }};\n",
        root.display()
    )
}

/// What the app's session host answers when the chat reads `file`: `decide::for_chat` over the
/// store as it stands now, as `decisions::decision` calls it for every ask.
fn reads(root: &Path, file: &str) -> Outcome {
    reads_with(root, file, &Sealed::of(root))
}

fn reads_with(root: &Path, file: &str, sealed: &Sealed) -> Outcome {
    let facts = ChatFacts {
        workspace: Some("web".to_owned()),
        persona: Some("builder".to_owned()),
        sandboxed: true,
        level: None,
        attended: true,
        harness_mode: None,
    };
    let path = format!("{}/{file}", root.display());
    decide::for_chat(
        root,
        7,
        Some(&facts),
        &Resolved::strict(),
        sealed,
        "read",
        ("Path", &path),
    )
    .expect("the chat is open")
    .after_mode
}

const LEDGER: &str = "workspaces/web/ledger/2026.csv";

fn a_never_of(scope: Scope, workspace: Option<&str>) -> Never {
    Never {
        action: "read".to_owned(),
        resource_kind: "Path".to_owned(),
        resource: "/somewhere/secret".to_owned(),
        scope,
        workspace: workspace.map(str::to_owned),
    }
}

fn a_grant(scope: Scope) -> Grant {
    Grant {
        rule: "sandbox-no-ledger".to_owned(),
        action: "read".to_owned(),
        resource_kind: "Path".to_owned(),
        resource: "/somewhere/ledger".to_owned(),
        narrowing: serde_json::Map::new(),
        scope,
        chat: Some(CHAT.to_owned()),
        workspace: Some("web".to_owned()),
        timed: None,
    }
}

fn give(root: &Path, entry: &Entry) {
    // A never or a grant is the person's answer, given by their press.
    #[allow(clippy::disallowed_methods, reason = "a test plays the person's press")]
    let press = answers::Press::of_the_persons_press();
    answers::commit(root, |draft| draft.add(&press, &entry.kind, &entry.text))
        .expect("the answer is sealed");
}

fn take_back(root: &Path, entry: &Entry) {
    answers::commit(root, |draft| {
        draft.remove_narrowing(&entry.kind, &entry.text);
    })
    .expect("the answer is taken out");
}

#[test]
fn a_running_chat_takes_a_tightening_at_its_next_decision() {
    let m = machine();
    let root = a_plane(&m);
    assert_ne!(reads(&root, LEDGER), Outcome::Deny, "not forbidden yet");
    // The chat goes on running: nothing restarts it, and its next read is judged by what is
    // committed now.
    commit(&root, &[("policy.cedar", &ban_on_the_ledger(&root))]);
    assert_eq!(reads(&root, LEDGER), Outcome::Deny);
    // A path the rule does not name is as it was.
    assert_ne!(reads(&root, "workspaces/web/README.md"), Outcome::Deny);
}

#[test]
fn a_loosening_reaches_the_running_chats_decisions_at_once_after_the_press() {
    let m = machine();
    let ban = ban_on_the_ledger(&plane_root_of(&m));
    let root = plane(
        &m,
        &[
            ("purlis.toml", STRICT),
            ("policy.cedar", &ban),
            ("workspaces/web/README.md", "web\n"),
            ("workspaces/web/ledger/2026.csv", "1\n"),
        ],
    );
    // What the person accepted holds the ban.
    let pressed = |rules: Option<&str>| Sealed {
        standing: holding(vec![accepted(&version(STRICT, rules))]),
        machine: None,
        chat: None,
        now: None,
        retired: Default::default(),
    };
    let running = started(&root);
    assert_eq!(
        reads_with(&root, LEDGER, &pressed(Some(&ban))),
        Outcome::Deny
    );
    // The commit drops the ban. The chat that runs is held to the accepted copy still, and is
    // not told it is out of date, since nothing that tightens moved.
    commit(&root, &[("policy.cedar", "-")]);
    assert_eq!(
        reads_with(&root, LEDGER, &pressed(Some(&ban))),
        Outcome::Deny,
        "the ban stands until the press"
    );
    assert_eq!(stale(&running, &root), Vec::<Why>::new());
    // The press accepts the loosened copy: a loosening reaches the running chat's decisions at
    // once (they read the sealed store fresh at each ask), and the chat is not marked, since a
    // loosening leaves its sandbox as strict as it was.
    assert_ne!(reads_with(&root, LEDGER, &pressed(None)), Outcome::Deny);
    assert_eq!(stale(&running, &root), Vec::<Why>::new());
}

/// The plane's root, as `a_plane` will make it, for a rule written before the plane exists.
fn plane_root_of(m: &Machine) -> PathBuf {
    std::fs::create_dir_all(m.dir.path().join("plane")).expect("a folder");
    std::fs::canonicalize(m.dir.path().join("plane")).expect("a real path")
}

#[test]
fn a_never_marks_a_running_chat_out_of_date() {
    let m = machine();
    let root = a_plane(&m);
    let running = started(&root);
    assert_eq!(stale(&running, &root), Vec::<Why>::new());
    // A Never for another workspace is not this chat's.
    give(&root, &a_never_of(Scope::Workspace, Some("api")).entry());
    assert_eq!(stale(&running, &root), Vec::<Why>::new());
    // Everywhere's is.
    give(&root, &a_never_of(Scope::Everywhere, None).entry());
    assert_eq!(stale(&running, &root), [Why::Never]);
}

#[test]
fn a_never_for_the_chats_own_workspace_marks_it_out_of_date() {
    let m = machine();
    let root = a_plane(&m);
    let running = started(&root);
    give(&root, &a_never_of(Scope::Workspace, Some("web")).entry());
    assert_eq!(stale(&running, &root), [Why::Never]);
}

#[test]
fn a_removed_grant_marks_a_running_chat_out_of_date() {
    let m = machine();
    let root = a_plane(&m);
    let grant = a_grant(Scope::Workspace).entry();
    give(&root, &grant);
    let running = started(&root);
    assert_eq!(stale(&running, &root), Vec::<Why>::new());
    take_back(&root, &grant);
    assert_eq!(stale(&running, &root), [Why::GrantRemoved]);
}

#[test]
fn a_grant_given_while_a_chat_runs_or_a_once_grant_spent_marks_nothing() {
    let m = machine();
    let root = a_plane(&m);
    let once = a_grant(Scope::Once).entry();
    give(&root, &once);
    let running = started(&root);
    // A grant is a loosening: it reaches the chat at its next start only.
    give(&root, &a_grant(Scope::Everywhere).entry());
    // A Once grant is spent by its use, which is not a removal of a grant that served.
    take_back(&root, &once);
    assert_eq!(stale(&running, &root), Vec::<Why>::new());
}

#[test]
fn a_path_tightening_marks_a_running_chat_out_of_date_and_offers_a_restart() {
    let m = machine();
    let root = a_plane(&m);
    let running = started(&root);
    commit(&root, &[("policy.cedar", &ban_on_the_ledger(&root))]);
    assert_eq!(stale(&running, &root), [Why::PathTightened]);
    // The offer is the app's: the mark carries a phrase the chat's tab and its Start fresh say.
    assert_eq!(Why::PathTightened.said(), "a path rule tightened");
}

#[test]
fn a_path_rule_that_is_dropped_marks_nothing() {
    let m = machine();
    let ban = ban_on_the_ledger(&plane_root_of(&m));
    let root = plane(
        &m,
        &[
            ("purlis.toml", STRICT),
            ("policy.cedar", &ban),
            ("workspaces/web/README.md", "web\n"),
        ],
    );
    let running = started(&root);
    commit(&root, &[("policy.cedar", "-")]);
    assert_eq!(stale(&running, &root), Vec::<Why>::new());
}

#[test]
fn a_workspace_created_after_a_chat_starts_marks_it_out_of_date() {
    let m = machine();
    let root = a_plane(&m);
    let running = started(&root);
    commit(&root, &[("workspaces/api/README.md", "api\n")]);
    // The shipped `sandbox-other-workspace-read` forbids reading another workspace's folder,
    // which the new workspace is to a chat of `web`: the compiled sandbox lacks it.
    assert_eq!(
        stale(&running, &root),
        [Why::PathTightened, Why::WorkspaceAdded]
    );
}

#[test]
fn a_workspace_that_goes_marks_nothing() {
    let m = machine();
    let root = plane(
        &m,
        &[
            ("purlis.toml", STRICT),
            ("workspaces/web/README.md", "web\n"),
            ("workspaces/api/README.md", "api\n"),
        ],
    );
    let running = started(&root);
    commit(&root, &[("workspaces/api/README.md", "-")]);
    assert_eq!(stale(&running, &root), Vec::<Why>::new());
}

#[test]
fn an_app_restart_marks_the_chat_out_of_date() {
    let m = machine();
    let root = a_plane(&m);
    // A chat the app found already running (reattached) was started by an app that is gone.
    let found = AtStart::take(&root, facts(), true);
    assert_eq!(stale(&found, &root), [Why::AppRestarted]);
    assert_eq!(stale(&started(&root), &root), Vec::<Why>::new());
}

#[test]
fn a_tightening_is_sealed_at_the_decision_that_sees_it() {
    let m = machine();
    let root = a_plane(&m);
    commit(&root, &[("policy.cedar", &ban_on_the_ledger(&root))]);
    let floor_before = answers::standing(&root).entries;
    assert!(
        force::floor_in(&floor_before).is_empty(),
        "nothing is sealed until a decision sees the tightening"
    );
    assert_eq!(reads(&root, LEDGER), Outcome::Deny);
    let sealed = force::floor_in(&answers::standing(&root).entries);
    assert_eq!(sealed.len(), 1, "the decision sealed what it saw");
    let version = sealed[0].as_ref().expect("a version that reads");
    assert!(
        version
            .rules
            .as_deref()
            .is_some_and(|rules| rules.contains("sandbox-no-ledger")),
        "{version:?}"
    );
    // So the ban holds with HEAD moved back, and a compile made after it forbids it too.
    let again = AtStart::take(&root, facts(), false);
    commit(&root, &[("policy.cedar", "-")]);
    assert_eq!(reads(&root, LEDGER), Outcome::Deny, "a sealed forbid stays");
    assert_eq!(stale(&again, &root), Vec::<Why>::new());
}

// ---- review round 2: what the rules resolve to, permits, the deny state, one shared reading ----

/// One side's places for one action, as a compile made them.
fn side(permits: &[Reach], forbids: &[Reach]) -> force::ActionPlaces {
    force::ActionPlaces {
        permits: permits.to_vec(),
        forbids: forbids.to_vec(),
    }
}

#[test]
fn a_permit_narrowed_or_taken_away_marks_a_running_chat_out_of_date() {
    // A read is allowed only where a permit allows it (spec §11.2, §11.5), so a place a permit let
    // through that none does now is a path tightening, like a new forbid. No project rule can say
    // it (the catch-all permit's id is the shipped default's alone, and a project cannot replace
    // it), so the Organisation's rules are where a side's permits narrow: the sides' places are
    // handed in as a compile made them.
    let web = || Reach::Folder("/p/workspaces/web".to_owned());
    let ledger = || Reach::Folder("/p/workspaces/web/ledger".to_owned());
    let everything = [Reach::Everything];
    let then = Places::of([side(&[web()], &[]), side(&everything, &[])].iter());
    assert_eq!(
        then.permits,
        Some(vec![web()]),
        "intersected across the sides"
    );
    // Narrowed: the ledger alone.
    let narrowed = Places::of([side(&[ledger()], &[]), side(&everything, &[])].iter());
    assert!(narrowed.tightens(&then));
    // Taken away: no permit lets anything through.
    let none = Places::of([side(&[], &[]), side(&everything, &[])].iter());
    assert_eq!(none.permits, Some(Vec::new()));
    assert!(none.tightens(&then));
    // Another side that now permits less narrows the intersection, though this side did not move.
    let other = Places::of([side(&[web()], &[]), side(&[ledger()], &[])].iter());
    assert!(other.tightens(&then));
    // Widened, or the same: nothing.
    let widened = Places::of([side(&everything, &[]), side(&everything, &[])].iter());
    assert!(!widened.tightens(&then));
    assert!(!then.tightens(&then));
    // The permit of a side that does not read or compile is none, and its forbid is everything.
    let closed = force::ActionPlaces {
        permits: Vec::new(),
        forbids: vec![Reach::Everything],
    };
    assert!(Places::of([side(&everything, &[]), closed].iter()).tightens(&then));
}

#[test]
fn a_set_whose_members_change_under_the_same_rule_marks_a_running_chat() {
    // The rule's text never changes: its `PathSet` keeps its name while what it holds grows, so a
    // comparison of the text sees nothing and the sandbox compiled at the start lacks the file.
    let m = machine();
    let rule = format!(
        "{HEADER}@id(\"sandbox-no-private\") @level(\"project\")\n\
         forbid (principal, action == Action::\"read\", resource) when {{ \
         resource in PathSet::\"private\" }};\n"
    );
    let with = |members: &str| format!("{STRICT}\n[policy.sets]\nprivate = [{members}]\n");
    let root = plane(
        &m,
        &[
            ("purlis.toml", &with("\"workspaces/web/ledger\"")),
            ("policy.cedar", &rule),
            ("workspaces/web/README.md", "web\n"),
            ("workspaces/web/ledger/2026.csv", "1\n"),
        ],
    );
    let running = started(&root);
    commit(
        &root,
        &[(
            "purlis.toml",
            &with("\"workspaces/web/ledger\", \"workspaces/web/README.md\""),
        )],
    );
    assert_eq!(stale(&running, &root), [Why::PathTightened]);
    // A member that goes is a loosening: a chat that started after the first commit is not marked.
    let later = started(&root);
    commit(
        &root,
        &[("purlis.toml", &with("\"workspaces/web/ledger\""))],
    );
    assert_eq!(stale(&later, &root), Vec::<Why>::new());
}

#[test]
fn a_repo_term_that_comes_to_name_a_folder_marks_a_running_chat() {
    // `Repo::"api/svc"` names a workspace that does not exist yet, so it holds nothing; once the
    // workspace is made it holds the repo's branch folders. Same text, new reach.
    let m = machine();
    let rule = format!(
        "{HEADER}@id(\"sandbox-no-svc\") @level(\"project\")\n\
         forbid (principal, action == Action::\"read\", resource) when {{ \
         resource in Repo::\"api/svc\" }};\n"
    );
    let root = plane(
        &m,
        &[
            ("purlis.toml", STRICT),
            ("policy.cedar", &rule),
            ("workspaces/web/README.md", "web\n"),
        ],
    );
    let running = started(&root);
    commit(&root, &[("workspaces/api/README.md", "api\n")]);
    assert_eq!(
        stale(&running, &root),
        [Why::PathTightened, Why::WorkspaceAdded]
    );
}

#[test]
fn entering_the_deny_state_or_losing_the_everywhere_answers_marks_a_running_chat() {
    let m = machine();
    let root = a_plane(&m);
    let running = started(&root);
    let mut shared = Shared::read(&root);
    assert_eq!(running.out_of_date(&shared), Vec::<Why>::new());
    shared.standing.everywhere_unreadable = true;
    shared.memo.borrow_mut().clear();
    assert_eq!(running.out_of_date(&shared), [Why::AnswersUnreadable]);
    shared.standing.state = answers::State::Deny(Vec::new());
    shared.memo.borrow_mut().clear();
    assert_eq!(
        running.out_of_date(&shared),
        [Why::DenyState, Why::AnswersUnreadable]
    );
    // A chat that started in the deny state is not marked by it again, and leaving it is a
    // loosening.
    let started_denied = AtStart {
        facts: facts(),
        reading: shared.reading(&facts()),
        reattached: false,
    };
    assert_eq!(started_denied.out_of_date(&shared), Vec::<Why>::new());
    assert_eq!(
        started_denied.out_of_date(&Shared::read(&root)),
        Vec::<Why>::new()
    );
}

#[test]
fn chats_of_the_same_facts_share_one_compile_and_the_shared_parts_are_read_once() {
    let m = machine();
    let root = a_plane(&m);
    let shared = Shared::read(&root);
    let one = shared.reading(&facts());
    assert_eq!(shared.reading(&facts()), one);
    assert_eq!(shared.memo.borrow().len(), 1, "the second was the first's");
    let other = Facts {
        chat: Some("01JOTHER".to_owned()),
        ..facts()
    };
    shared.reading(&other);
    assert_eq!(shared.memo.borrow().len(), 2);
}

mod places {
    use super::*;
    use crate::policy::sandbox_rules::Reach::{Entry, Everything, Folder, Inside, Unowned};

    fn folder(path: &str) -> Reach {
        Folder(path.to_owned())
    }

    #[test]
    fn a_reach_is_covered_only_where_it_clearly_lies_inside() {
        assert!(covered(&folder("/p/a/b"), &[folder("/p/a")]));
        assert!(!covered(&folder("/p/a"), &[folder("/p/a/b")]));
        assert!(covered(&folder("/p/a"), &[Everything]));
        assert!(covered(&Entry("/p/a/f".into()), &[folder("/p/a")]));
        assert!(!covered(&Entry("/p/b/f".into()), &[folder("/p/a")]));
        assert!(covered(&Inside("/p/a/b".into()), &[folder("/p/a")]));
        assert!(!covered(&folder("/p/a"), &[Unowned(vec!["/p/a".into()])]));
        // What this cannot place stands as not covered: it marks more, never less.
        assert!(!covered(
            &Unowned(vec!["/p/a".into()]),
            &[folder("/p"), Inside("/q".into())]
        ));
    }

    #[test]
    fn two_places_meet_in_the_deeper_one_or_in_nothing() {
        assert_eq!(
            meet(&folder("/p/a"), &folder("/p/a/b")),
            Some(Some(folder("/p/a/b")))
        );
        assert_eq!(meet(&folder("/p/a"), &folder("/p/c")), Some(None));
        assert_eq!(
            meet(&Everything, &folder("/p/a")),
            Some(Some(folder("/p/a")))
        );
        assert_eq!(
            meet(&Entry("/p/a/f".into()), &folder("/p/a")),
            Some(Some(Entry("/p/a/f".into())))
        );
    }

    #[test]
    fn what_cannot_be_written_as_places_fails_closed() {
        // A folder above a workspace's folder, met with what no workspace owns: part of it is
        // owned and part is not, which no place says. The reading then stands as "unknown", and
        // a start that knew becomes tightened by it.
        let above = [folder("/p")];
        let unowned = [Unowned(vec!["/p/web".into()])];
        assert_eq!(meet_all(&above, &unowned), None);
        let known = Places {
            forbids: Vec::new(),
            permits: Some(vec![folder("/p")]),
        };
        let unknown = Places {
            forbids: Vec::new(),
            permits: None,
        };
        assert!(unknown.tightens(&known));
        assert!(!known.tightens(&unknown));
        assert!(!unknown.tightens(&unknown));
    }

    #[test]
    fn a_forbid_is_a_tightening_only_where_the_start_did_not_hold_it() {
        let holding = |places: &[Reach]| Places {
            forbids: places.to_vec(),
            permits: Some(vec![Everything]),
        };
        let then = holding(&[folder("/p/a")]);
        assert!(holding(&[folder("/p/a"), folder("/p/b")]).tightens(&then));
        assert!(
            !holding(&[folder("/p/a/x")]).tightens(&then),
            "a smaller forbid"
        );
        assert!(holding(&[folder("/p")]).tightens(&then), "a larger one");
        assert!(!holding(&[]).tightens(&then));
    }
}

#[test]
fn a_write_of_the_sealed_store_tells_who_named_the_plane_and_no_other() {
    // The window asks again which chats are out of date when this tells it: a Never added, a grant
    // taken out and a Once grant spent write the store and change no file of the plane.
    use std::sync::atomic::{AtomicUsize, Ordering};
    let m = machine();
    let root = a_plane(&m);
    let told = std::sync::Arc::new(AtomicUsize::new(0));
    answers::tell_when_written(&root, {
        let told = std::sync::Arc::clone(&told);
        Some(std::sync::Arc::new(move || {
            told.fetch_add(1, Ordering::SeqCst);
        }))
    });
    let grant = a_grant(Scope::Workspace).entry();
    give(&root, &grant);
    assert_eq!(told.load(Ordering::SeqCst), 1, "a grant was given");
    take_back(&root, &grant);
    assert_eq!(told.load(Ordering::SeqCst), 2, "and taken out");
    give(&root, &a_never_of(Scope::Everywhere, None).entry());
    assert_eq!(told.load(Ordering::SeqCst), 3, "a Never was added");
    // Nobody named is told nothing.
    answers::tell_when_written(&root, None);
    give(&root, &a_never_of(Scope::Workspace, Some("web")).entry());
    assert_eq!(told.load(Ordering::SeqCst), 3);
}
