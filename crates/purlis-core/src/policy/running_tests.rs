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
    }
}

fn give(root: &Path, entry: &Entry) {
    // A never or a grant is the person's answer, given by their press.
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
fn a_running_chat_takes_a_loosening_only_at_its_next_start() {
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
    assert_eq!(running.out_of_date(&root), Vec::<Why>::new());
    // The press accepts the loosened copy: a chat that starts after it is held to that, and the
    // chat that ran through it is not marked either, since it is on what it started on.
    assert_ne!(reads_with(&root, LEDGER, &pressed(None)), Outcome::Deny);
    assert_eq!(running.out_of_date(&root), Vec::<Why>::new());
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
    assert_eq!(running.out_of_date(&root), Vec::<Why>::new());
    // A Never for another workspace is not this chat's.
    give(&root, &a_never_of(Scope::Workspace, Some("api")).entry());
    assert_eq!(running.out_of_date(&root), Vec::<Why>::new());
    // Everywhere's is.
    give(&root, &a_never_of(Scope::Everywhere, None).entry());
    assert_eq!(running.out_of_date(&root), [Why::Never]);
}

#[test]
fn a_never_for_the_chats_own_workspace_marks_it_out_of_date() {
    let m = machine();
    let root = a_plane(&m);
    let running = started(&root);
    give(&root, &a_never_of(Scope::Workspace, Some("web")).entry());
    assert_eq!(running.out_of_date(&root), [Why::Never]);
}

#[test]
fn a_removed_grant_marks_a_running_chat_out_of_date() {
    let m = machine();
    let root = a_plane(&m);
    let grant = a_grant(Scope::Workspace).entry();
    give(&root, &grant);
    let running = started(&root);
    assert_eq!(running.out_of_date(&root), Vec::<Why>::new());
    take_back(&root, &grant);
    assert_eq!(running.out_of_date(&root), [Why::GrantRemoved]);
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
    assert_eq!(running.out_of_date(&root), Vec::<Why>::new());
}

#[test]
fn a_path_tightening_marks_a_running_chat_out_of_date_and_offers_a_restart() {
    let m = machine();
    let root = a_plane(&m);
    let running = started(&root);
    commit(&root, &[("policy.cedar", &ban_on_the_ledger(&root))]);
    assert_eq!(running.out_of_date(&root), [Why::PathTightened]);
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
    assert_eq!(running.out_of_date(&root), Vec::<Why>::new());
}

#[test]
fn a_workspace_created_after_a_chat_starts_marks_it_out_of_date() {
    let m = machine();
    let root = a_plane(&m);
    let running = started(&root);
    commit(&root, &[("workspaces/api/README.md", "api\n")]);
    assert_eq!(running.out_of_date(&root), [Why::WorkspaceAdded]);
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
    assert_eq!(running.out_of_date(&root), Vec::<Why>::new());
}

#[test]
fn an_app_restart_marks_the_chat_out_of_date() {
    let m = machine();
    let root = a_plane(&m);
    // A chat the app found already running (reattached) was started by an app that is gone.
    let found = AtStart::take(&root, facts(), true);
    assert_eq!(found.out_of_date(&root), [Why::AppRestarted]);
    assert_eq!(started(&root).out_of_date(&root), Vec::<Why>::new());
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
    assert_eq!(again.out_of_date(&root), Vec::<Why>::new());
}
