// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Refusals name an unaccepted policy as their cause** (ADR 0130 §7, #7717): under a Broken or
//! Waiting acceptance, a refusal that the unaccepted policy causes says the project's policy
//! isn't accepted on this machine and that purlis's window accepts it, and never tells the
//! person to set the family to `log`. A refusal on its own merits keeps its words.

use super::*;
use crate::dispatchgrant::{Covers, Deny};
use crate::forge::{Forge, Kind};
use crate::handoffguard::Caller;
use crate::sandbox::local::OwnConfigHome;
use crate::sandbox::policy::Locks;
use crate::toolgate::{self, Call, Launched, OVERRIDE_NOTE, Plane};

/// What [`unaccepted_said`] says for this machine.
fn the_cause() -> String {
    unaccepted_said(AcceptedOn::ThisMachine)
}

/// A plane whose committed file says `committed`, with a vault the project registers for
/// `devops` alone.
fn plane(committed: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::write(dir.path().join("charter.toml"), committed).expect("the committed file");
    std::fs::write(
        dir.path().join("vaults.json"),
        serde_json::json!({ "vaults": {
            "prod": {"provider": "plain-file", "config": {"file": "prod.json"}, "persona": "devops"},
        }})
        .to_string(),
    )
    .expect("the registry");
    dir
}

/// `committed`, pressed, and then the sealed record taken away: **Broken**.
fn broken_plane(committed: &str) -> tempfile::TempDir {
    let root = plane(committed);
    press(root.path(), &digest_of(root.path())).expect("pressed");
    let file = crate::sandbox::local::path(root.path());
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("kept")).expect("JSON");
    let object = doc.as_object_mut().expect("an object");
    object.remove("policy_accepted");
    object.remove("policy_seal");
    std::fs::write(&file, doc.to_string()).expect("written");
    assert!(broken(root.path()), "the acceptance is broken");
    root
}

/// `accepted` kept by a first start, and then the committed file loosened to `now`: **Waiting**.
fn waiting_plane(accepted: &str, now: &str) -> tempfile::TempDir {
    let root = plane(accepted);
    at_start(root.path());
    std::fs::write(root.path().join("charter.toml"), now).expect("the committed file");
    assert!(!waiting(root.path()).is_empty(), "a loosening waits");
    root
}

/// What the Bash guard says of `cmd` in the plane at `root` under `policy`, or none.
fn refusal(root: &Path, cmd: &str, policy: &Resolved) -> Option<String> {
    let at = root.display().to_string();
    let state = root.join(".charter");
    let forges = vec![Forge::default_of(Kind::GitHub)];
    let plane = Plane {
        root: &at,
        forges: &forges,
        session_dir: &at,
        launched: Launched {
            sandboxed: false,
            chat_dir: Some(&at),
            attendance: None,
        },
    };
    let call = Call {
        command: cmd,
        cwd: &at,
        state_dir: &state,
        caller: Caller {
            agent_id: None,
            harness: Some("claude-code"),
            permission_mode: Some("default"),
        },
    };
    toolgate::judged_under(&call, Some(&plane), policy, AcceptedOn::ThisMachine)
        .refused
        .map(|verdict| verdict.said())
}

/// A guard's line `open` lets through: one credential (`guards.credential`).
const AN_SSH_CLONE: &str = "git clone git@github.com:o/r.git";
/// A walk into a vault (`secrets`).
const A_VAULT_WALK: &str = "cat .charter/vaults/db.json";

fn assert_names_the_unaccepted_policy(said: &str) {
    assert!(said.contains(&the_cause()), "{said}");
    assert!(said.contains("isn't accepted on this machine"), "{said}");
    assert!(said.contains("purlis's window"), "{said}");
    assert!(!said.contains(OVERRIDE_NOTE), "{said}");
    assert!(!said.contains("to log or off"), "{said}");
}

#[test]
fn under_a_broken_acceptance_a_bash_guard_refusal_names_the_unaccepted_policy() {
    let _home = OwnConfigHome::new();
    let root = broken_plane("[policy]\ntemplate = \"open\"\n");
    let policy = in_force(root.path());
    assert!(policy.unaccepted(Family::GuardsCredential));
    let said = refusal(root.path(), AN_SSH_CLONE, &policy).expect("refused under strict mode");
    assert_names_the_unaccepted_policy(&said);
}

#[test]
fn under_a_broken_acceptance_a_secrets_refusal_names_the_unaccepted_policy() {
    let _home = OwnConfigHome::new();
    let root = broken_plane("[policy]\ntemplate = \"open\"\n");
    // A vault the leak guard refuses a walk into. A sandbox that runs these tests may deny
    // making one, as `toolgate::policy_tests` says: CI judges this test there.
    let vaults = root.path().join(".charter").join("vaults");
    std::fs::create_dir_all(&vaults).expect("the vault directory");
    std::fs::write(vaults.join("db.json"), "{}\n").expect("a vault");
    let policy = in_force(root.path());
    assert!(policy.unaccepted(Family::Secrets));
    let said = refusal(root.path(), A_VAULT_WALK, &policy).expect("the walk is refused");
    assert_names_the_unaccepted_policy(&said);
}

#[test]
fn under_a_broken_acceptance_a_vaults_refusal_names_the_unaccepted_policy() {
    use crate::secrets::{Ctx, Env, brokered};
    let _home = OwnConfigHome::new();
    let root = broken_plane("[policy]\ntemplate = \"open\"\n");
    let ctx = Ctx::new(root.path(), Env::of(&[]));
    let why = brokered::authorise(&ctx, Some("steward"), "prod").expect_err("refused");
    assert!(why.contains(&the_cause()), "{why}");
    // A vault the project does not register is refused for its own reason.
    let unregistered = brokered::authorise(&ctx, Some("devops"), "nope").expect_err("refused");
    assert!(!unregistered.contains(&the_cause()), "{unregistered}");
}

#[test]
fn under_a_broken_acceptance_a_dispatch_to_another_persona_names_the_unaccepted_policy() {
    let _home = OwnConfigHome::new();
    let root = broken_plane("[policy]\ntemplate = \"open\"\n");
    let grants = crate::dispatchgrant::InForce::read(root.path(), Vec::new());
    let covers = crate::dispatchgrant::covers(Some("steward"), "devops", &grants, &Locks::none());
    assert_eq!(covers, Covers::Denied(Deny::Unaccepted));
    let said = crate::dispatchgrant::denied_said("devops", Deny::Unaccepted);
    assert!(said.contains(&the_cause()), "{said}");
    assert!(!said.contains("could not read"), "{said}");
    // The press mends it.
    press(root.path(), &digest_of(root.path())).expect("pressed again");
    let grants = crate::dispatchgrant::InForce::read(root.path(), Vec::new());
    assert_ne!(
        crate::dispatchgrant::covers(Some("steward"), "devops", &grants, &Locks::none()),
        Covers::Denied(Deny::Unaccepted)
    );
}

#[test]
fn under_a_broken_acceptance_no_refusal_blames_the_list_of_pairs_said_never_to() {
    // ADR 0125 §3: the nevers are sealed with the acceptance's anchor, so a seal key another
    // program made unreads both. The list is as purlis wrote it; the policy is the cause.
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n");
    press(root.path(), &digest_of(root.path())).expect("pressed");
    crate::dispatchnever::add(root.path(), "steward", "web").expect("a never");
    crate::secrets::seal::set_foreign_for_this_test(true);
    let unread = crate::dispatchnever::read(root.path());
    let said = crate::dispatchnever::unread_said(root.path());
    let grants = crate::dispatchgrant::InForce::read(root.path(), Vec::new());
    let covers = crate::dispatchgrant::covers(Some("steward"), "devops", &grants, &Locks::none());
    let added = crate::dispatchnever::add(root.path(), "steward", "devops");
    crate::secrets::seal::set_foreign_for_this_test(false);
    assert_eq!(unread, crate::dispatchnever::Nevers::Unread);
    assert!(
        !said.contains("the list of pairs you said never to"),
        "{said}"
    );
    assert!(said.contains(&the_cause()), "{said}");
    assert_eq!(covers, Covers::Denied(Deny::Unaccepted));
    let refused = added.expect_err("nothing written").to_string();
    assert!(
        !refused.contains("the list of pairs you said never to"),
        "{refused}"
    );
}

#[test]
fn an_unreadable_nevers_file_under_a_held_acceptance_keeps_its_own_sentence() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n");
    press(root.path(), &digest_of(root.path())).expect("pressed");
    let path = crate::dispatchnever::path(root.path());
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    std::fs::write(&path, "not json").expect("broken");
    assert!(!broken(root.path()));
    let said = crate::dispatchnever::unread_said(root.path());
    assert_eq!(said, crate::dispatchnever::file_unread_said(root.path()));
    assert!(
        said.contains("the list of pairs you said never to"),
        "{said}"
    );
    let grants = crate::dispatchgrant::InForce::read(root.path(), Vec::new());
    assert_eq!(
        crate::dispatchgrant::covers(Some("steward"), "devops", &grants, &Locks::none()),
        Covers::Unread
    );
}

#[test]
fn under_a_broken_acceptance_an_unreadable_nevers_file_still_blames_the_file() {
    let _home = OwnConfigHome::new();
    let root = broken_plane("[policy]\ntemplate = \"open\"\n");
    let path = crate::dispatchnever::path(root.path());
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    std::fs::write(&path, "not json").expect("broken");
    assert_eq!(
        crate::dispatchnever::unread_said(root.path()),
        crate::dispatchnever::file_unread_said(root.path())
    );
}

#[test]
fn under_waiting_a_family_the_committed_file_loosens_names_the_unaccepted_policy() {
    let _home = OwnConfigHome::new();
    let root = waiting_plane(
        "[policy]\ntemplate = \"strict\"\n",
        "[policy]\ntemplate = \"open\"\n",
    );
    let policy = in_force(root.path());
    assert_eq!(policy.mode(Family::GuardsCredential), Mode::Enforce);
    assert!(policy.unaccepted(Family::GuardsCredential));
    let said = refusal(root.path(), AN_SSH_CLONE, &policy).expect("still enforced");
    assert_names_the_unaccepted_policy(&said);
}

#[test]
fn under_waiting_a_deny_the_committed_file_dropped_names_the_unaccepted_policy() {
    let _home = OwnConfigHome::new();
    let root = waiting_plane(
        "[policy]\ntemplate = \"open\"\n\n[dispatch.denies]\nsteward = [\"devops\"]\n",
        "[policy]\ntemplate = \"open\"\n",
    );
    let grants = crate::dispatchgrant::InForce::read(root.path(), Vec::new());
    assert_eq!(
        grants.denied_by(Some("steward"), "devops"),
        Some(Deny::Unaccepted)
    );
}

#[test]
fn a_family_enforced_on_its_own_merits_keeps_its_words_and_the_override_note() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"open\"\n");
    // A lock, and a template that enforces the family, each under a Broken acceptance.
    for committed in [
        Resolved::of(
            &Settings::from_text(Some("[policy]\ntemplate = \"open\"\n")),
            &Locks::locking_enforcement(Family::GuardsCredential),
        ),
        Resolved::of(
            &Settings::from_text(Some("[policy]\ntemplate = \"watch\"\n")),
            &Locks::none(),
        ),
    ] {
        let policy = Resolved::accepting(&committed, &broken_accepted());
        assert!(!policy.unaccepted(Family::GuardsCredential));
        let said = refusal(root.path(), AN_SSH_CLONE, &policy).expect("refused");
        assert!(said.ends_with(OVERRIDE_NOTE), "{said}");
        assert!(!said.contains(&the_cause()), "{said}");
    }
}

#[test]
fn a_held_acceptance_with_nothing_waiting_names_no_cause() {
    let _home = OwnConfigHome::new();
    let root = plane("[policy]\ntemplate = \"strict\"\n");
    press(root.path(), &digest_of(root.path())).expect("pressed");
    let policy = in_force(root.path());
    assert!(Family::ALL.iter().all(|family| !policy.unaccepted(*family)));
    let said = refusal(root.path(), AN_SSH_CLONE, &policy).expect("refused");
    assert!(said.ends_with(OVERRIDE_NOTE), "{said}");
}

#[test]
fn the_sentence_names_where_the_acceptance_lives() {
    let here = unaccepted_said(AcceptedOn::ThisMachine);
    assert!(here.contains("isn't accepted on this machine"), "{here}");
    assert!(here.contains("in purlis's window;"), "{here}");
    let there = unaccepted_said(AcceptedOn::Machine("build-mac"));
    assert!(there.contains("isn't accepted on build-mac"), "{there}");
    assert!(there.contains("in purlis's window on build-mac"), "{there}");
    assert!(!there.contains("this machine"), "{there}");
    let dispatch = crate::dispatchgrant::denied_said_on(
        "devops",
        Deny::Unaccepted,
        AcceptedOn::Machine("build-mac"),
    );
    assert!(dispatch.contains(&there), "{dispatch}");
    assert!(
        crate::dispatchnever::unaccepted_said(AcceptedOn::Machine("build-mac")).contains(&there)
    );
}

#[test]
fn a_hook_takes_the_cause_from_the_apps_answer_and_an_older_answer_keeps_todays_words() {
    use crate::hookwire::Answer;
    let strict = Resolved::all(Mode::Enforce);
    // An older app's answer carries no cause.
    let older: Answer =
        serde_json::from_value(serde_json::json!({"policy": {"spelled": strict.spelled()}}))
            .expect("an older answer reads");
    let Answer::Policy {
        spelled,
        unaccepted,
    } = older
    else {
        panic!("a policy answer");
    };
    let heard = Resolved::parse(&spelled).with_unaccepted(&unaccepted);
    assert!(!heard.unaccepted(Family::GuardsCredential));
    // This app's carries it, and a cause marks only a family the answer enforces.
    let answer = Answer::Policy {
        spelled: Resolved::all(Mode::Log).spelled(),
        unaccepted: vec!["guards.credential".to_owned(), "nonsense".to_owned()],
    };
    let wire = serde_json::to_value(&answer).expect("written");
    let read: Answer = serde_json::from_value(wire).expect("read back");
    assert_eq!(read, answer);
    let Answer::Policy {
        spelled,
        unaccepted,
    } = read
    else {
        panic!("a policy answer");
    };
    assert!(
        !Resolved::parse(&spelled)
            .with_unaccepted(&unaccepted)
            .unaccepted(Family::GuardsCredential),
        "a logged family is refused by nothing"
    );
    assert!(
        Resolved::parse(&strict.spelled())
            .with_unaccepted(&unaccepted)
            .unaccepted(Family::GuardsCredential)
    );
    // No cause, no new key on the wire: an older hook reads the same bytes.
    let none = serde_json::to_value(&Answer::Policy {
        spelled: strict.spelled(),
        unaccepted: Vec::new(),
    })
    .expect("written");
    assert!(!none.to_string().contains("unaccepted"), "{none}");
}

#[test]
fn a_floor_joined_to_the_answer_keeps_the_answers_cause() {
    // The start's `PURLIS_POLICY` and a chain's policy carry no cause, and only tighten.
    let answer = Resolved::accepting(
        &Resolved::of(
            &Settings::from_text(Some("[policy]\ntemplate = \"open\"\n")),
            &Locks::none(),
        ),
        &broken_accepted(),
    );
    let floored = answer.stricter(&Resolved::all(Mode::Enforce));
    assert!(floored.unaccepted(Family::GuardsCredential));
    assert!(
        answer
            .clone()
            .for_chat(Some(&answer.spelled()))
            .unaccepted(Family::GuardsCredential)
    );
}

#[test]
fn a_refusals_own_words_end_where_its_note_or_its_cause_begins() {
    let mut verdict = toolgate::too_deep();
    let own = verdict.denial.clone();
    assert_eq!(
        toolgate::own_words(&verdict.said()),
        format!("purlis guard: {own}")
    );
    verdict.unaccepted = Some(the_cause());
    let said = verdict.said();
    assert_eq!(toolgate::own_words(&said), format!("purlis guard: {own}"));
    assert!(said.ends_with(&the_cause()), "{said}");
}

#[test]
fn a_refusal_that_names_the_shape_to_retry_names_an_unaccepted_policy_too() {
    let mut verdict = toolgate::too_deep();
    verdict.reason = toolgate::RETRY_SHAPED[0].to_owned();
    assert!(!verdict.said().contains(OVERRIDE_NOTE));
    verdict.unaccepted = Some(the_cause());
    assert!(verdict.said().ends_with(&the_cause()));
}
