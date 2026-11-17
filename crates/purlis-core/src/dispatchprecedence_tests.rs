// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Dispatch's full precedence, and the sealed grants it rests on** (ticket T1.1-73, #6968;
//! rulings Q306, Q308b, Q320): lock > the person's never > policy deny > same persona > policy
//! allow > sealed grant > policy ask, one test per rank, and each guarantee of the seal. Every
//! test runs under `[policy] template = "strict"`, set explicitly (Q308), except where it says
//! it runs under every template.

use std::path::Path;

use super::*;
use crate::dispatchunattended::{self as unattended, Answer, Reach, Refusal};
use crate::enforcement::{Family, Mode, Resolved, Settings, Template};

/// The project's `[policy]`, written out.
const STRICT: &str = "[policy]\ntemplate = \"strict\"\n";

/// How `template` runs the `dispatch` family, read from the text a project commits.
fn dispatch_mode(template: Template) -> Mode {
    let text = format!("[policy]\ntemplate = \"{}\"\n", template.word());
    Resolved::of(
        &Settings::from_text(Some(&text)),
        &crate::sandbox::policy::Locks::none(),
    )
    .mode(Family::Dispatch)
}

fn strict() -> Mode {
    let mode = Resolved::of(
        &Settings::from_text(Some(STRICT)),
        &crate::sandbox::policy::Locks::none(),
    )
    .mode(Family::Dispatch);
    assert_eq!(mode, Mode::Enforce, "strict enforces dispatch");
    mode
}

fn pair(asking: &str, target: &str) -> Pair {
    Pair::new(asking, target).expect("a pair")
}

fn locks(json: &str) -> Locks {
    Locks::parse(json, Path::new("/etc/purlis/policy.json"))
}

/// Every way a pair can be let through at once: the project's policy allow, a sealed grant of
/// yours, and one made for the chat.
fn every_allow() -> InForce {
    InForce {
        project: vec![pair("steward", "devops")],
        you: vec![pair("steward", "devops")],
        chat: vec![ChatPair {
            asking: Some("steward".to_owned()),
            target: "devops".to_owned(),
        }],
        ..InForce::default()
    }
}

/// `covers` for steward to devops, as `mode` runs the family.
fn judged(grants: &InForce, policy: &Locks, mode: Mode) -> Covers {
    as_the_policy_runs(covers(Some("steward"), "devops", grants, policy), mode)
}

/// A project with its committed `[policy]` and settings, and this test's own config home.
struct Project {
    _home: crate::sandbox::local::OwnConfigHome,
    dir: tempfile::TempDir,
}

impl Project {
    fn new(settings: &str) -> Self {
        let home = crate::sandbox::local::OwnConfigHome::new();
        let dir = tempfile::tempdir().expect("a project");
        std::fs::write(crate::names::manifest(dir.path()), settings).expect("written");
        Self { _home: home, dir }
    }

    /// Another project in this test's config home.
    fn beside(&self, settings: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a project");
        std::fs::write(crate::names::manifest(dir.path()), settings).expect("written");
        dir
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn grants(&self) -> InForce {
        InForce::read(self.root(), Vec::new())
    }
}

// ---- one test per rank --------------------------------------------------------------------------

#[test]
fn a_lock_outranks_a_never() {
    let mut grants = every_allow();
    grants.never = vec![("steward".to_owned(), "devops".to_owned())];
    grants.denied = vec![pair("steward", "devops")];
    let policy =
        locks(r#"{"owner": "IT", "dispatch": {"locked": [{"from": "steward", "to": "devops"}]}}"#);
    assert!(matches!(
        judged(&grants, &policy, strict()),
        Covers::Locked(_)
    ));
}

#[test]
fn a_never_holds_over_a_policy_allow_and_a_sealed_grant_in_every_mode() {
    let mut grants = every_allow();
    grants.never = vec![("steward".to_owned(), "devops".to_owned())];
    for template in Template::ALL {
        let mode = dispatch_mode(template);
        assert_eq!(
            judged(&grants, &Locks::none(), mode),
            Covers::Never,
            "{}",
            template.word()
        );
        // And for a chat nobody is at, with no sandbox, on a system where the seal holds.
        let answer = unattended::as_the_policy_runs(
            unattended::answer_on(
                covers(Some("steward"), "devops", &grants, &Locks::none()),
                Some("steward"),
                "devops",
                false,
                Reach {
                    sandboxed: false,
                    sealed_here: true,
                },
            ),
            mode,
        );
        assert_eq!(
            answer,
            Answer::Refused(Refusal::Never("steward".to_owned(), "devops".to_owned())),
            "{}",
            template.word()
        );
    }
    // `open` included, written out.
    assert_eq!(dispatch_mode(Template::Open), Mode::Off);
}

#[test]
fn a_grant_never_lifts_a_policy_deny() {
    let mut grants = every_allow();
    grants.denied = vec![pair("steward", "devops")];
    assert_eq!(
        judged(&grants, &Locks::none(), strict()),
        Covers::Denied(DENY_RULE.to_owned())
    );
    // "Every other persona" denies the same, and no other asker.
    let mut any = every_allow();
    any.denied_any = vec!["steward".to_owned()];
    assert_eq!(
        judged(&any, &Locks::none(), strict()),
        Covers::Denied(DENY_RULE.to_owned())
    );
    assert_eq!(
        covers(Some("qa"), "devops", &any, &Locks::none()),
        Covers::NeedsGrant
    );
    // Said to the chat, and not kept for the needs-you list: no press would lift it.
    let refused = unattended::answer_on(
        covers(Some("steward"), "devops", &grants, &Locks::none()),
        Some("steward"),
        "devops",
        false,
        Reach {
            sandboxed: true,
            sealed_here: true,
        },
    );
    let Answer::Refused(refusal) = refused else {
        panic!("refused")
    };
    assert_eq!(refusal, Refusal::Denied("devops".to_owned()));
    assert_eq!(crate::dispatchaway::pair_kept(&refusal), None);
    assert!(
        refusal.say().contains("No grant lifts it"),
        "{}",
        refusal.say()
    );
}

#[test]
fn a_policy_deny_is_read_from_the_project_s_committed_file() {
    let project = Project::new(&format!(
        "{STRICT}[dispatch.grants]\nsteward = [\"devops\"]\n\n[dispatch.denies]\nsteward = [\"devops\"]\nqa = [\"*\"]\n"
    ));
    let grants = project.grants();
    assert_eq!(grants.denied, vec![pair("steward", "devops")]);
    assert_eq!(grants.denied_any, vec!["qa".to_owned()]);
    assert_eq!(
        committed_denies(Some("[dispatch.denies]\nsteward = [\"Not A Name\", 3]\n"))
            .refused
            .len(),
        2
    );
    // And the limits reader does not refuse the table as a limit it does not know.
    let read = crate::dispatchlimits::read(
        Some(&std::fs::read_to_string(crate::names::manifest(project.root())).expect("read")),
        "purlis.toml",
    );
    assert!(read.refused.is_empty(), "{:?}", read.refused);
}

#[test]
fn same_persona_dispatch_needs_no_grant() {
    // Nothing granted, a deny of every other persona, and the policy asking for everything else.
    let grants = InForce {
        denied_any: vec!["steward".to_owned()],
        ..InForce::default()
    };
    assert_eq!(
        as_the_policy_runs(
            covers(Some("steward"), "steward", &grants, &Locks::none()),
            strict()
        ),
        Covers::Covered
    );
    // With no sandbox and nobody at the chat, on a system where no seal holds.
    assert_eq!(
        unattended::answer_on(
            covers(Some("steward"), "steward", &grants, &Locks::none()),
            Some("steward"),
            "steward",
            false,
            Reach {
                sandboxed: false,
                sealed_here: false,
            },
        ),
        Answer::Covered
    );
}

#[test]
fn a_policy_allow_needs_no_grant() {
    // The project's committed `[dispatch.grants]` is the policy allow; this machine's sealed
    // acceptance is the loosening press, and nothing of the person's own is granted.
    let project = Project::new(&format!(
        "{STRICT}[dispatch.grants]\nsteward = [\"devops\"]\n"
    ));
    acknowledge_pair(project.root(), &pair("steward", "devops")).expect("accepted");
    let grants = project.grants();
    assert!(grants.you.is_empty() && grants.chat.is_empty());
    assert_eq!(
        grants.level_of(Some("steward"), "devops"),
        Some(Level::Project)
    );
    assert_eq!(judged(&grants, &Locks::none(), strict()), Covers::Covered);
}

#[test]
fn an_unsandboxed_unattended_chat_dispatches_to_a_sealed_granted_pair() {
    let project = Project::new(STRICT);
    crate::sandbox::local::grant_dispatch(project.root(), "steward", "devops").expect("granted");
    let grants = project.grants();
    assert_eq!(grants.level_of(Some("steward"), "devops"), Some(Level::You));
    // Where the seal keeps the grant out of the chat's reach (macOS, in the app).
    let answer = unattended::as_the_policy_runs(
        unattended::answer_on(
            covers(Some("steward"), "devops", &grants, &Locks::none()),
            Some("steward"),
            "devops",
            false,
            Reach {
                sandboxed: false,
                sealed_here: true,
            },
        ),
        strict(),
    );
    assert_eq!(answer, Answer::Covered);
    // On this system, as the app asks it.
    #[cfg(target_os = "macos")]
    assert_eq!(
        unattended::covers(
            Some("steward"),
            "devops",
            &grants,
            &Locks::none(),
            false,
            false
        ),
        Answer::Covered
    );
}

#[test]
fn an_ungranted_pair_at_a_policy_ask_reaches_needs_you_with_its_words() {
    let project = Project::new(STRICT);
    let answer = unattended::as_the_policy_runs(
        unattended::answer_on(
            covers(Some("steward"), "devops", &project.grants(), &Locks::none()),
            Some("steward"),
            "devops",
            false,
            Reach {
                sandboxed: false,
                sealed_here: true,
            },
        ),
        strict(),
    );
    let Answer::Refused(refusal) = answer else {
        panic!("refused at once: {answer:?}")
    };
    assert!(matches!(refusal, Refusal::Unsandboxed(_)), "{refusal:?}");
    assert_eq!(
        crate::dispatchaway::pair_kept(&refusal),
        Some(("steward", "devops"))
    );
    let said = refusal.say();
    for words in [
        "nobody is here to answer",
        "no grant the person made covers",
        "needs-you list",
    ] {
        assert!(said.contains(words), "{words}: {said}");
    }
}

// ---- the seal's guarantees ----------------------------------------------------------------------

/// This machine's record for the project, as JSON, to change as a chat with no sandbox would.
fn record(root: &Path) -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(crate::sandbox::local::path(root)).expect("the record"),
    )
    .expect("json")
}

fn write_record(root: &Path, value: &serde_json::Value) {
    std::fs::write(
        crate::sandbox::local::path(root),
        serde_json::to_string_pretty(value).expect("json"),
    )
    .expect("written");
}

#[test]
fn a_grant_edited_by_a_chat_or_copied_from_another_clone_is_dropped_and_shown_read_only() {
    use crate::sandbox::local::{DispatchKind, dispatch_unsealed, grant_dispatch};
    let project = Project::new(STRICT);
    let root = project.root();
    grant_dispatch(root, "steward", "devops").expect("granted");
    grant_dispatch(root, "steward", "qa").expect("granted");
    // Edited: the pair changed, its seal not.
    let mut held = record(root);
    held["dispatch_mine"][0]["target"] = "prod".into();
    held["dispatch_sealed"]["entries"][0]["text"] = "steward -> prod".into();
    write_record(root, &held);
    assert_eq!(yours(root), [pair("steward", "qa")]);
    assert_eq!(
        covers(Some("steward"), "prod", &project.grants(), &Locks::none()),
        Covers::NeedsGrant
    );
    assert!(dispatch_unsealed(root).contains(&(DispatchKind::Mine, "steward -> prod".to_owned())));
    // Copied from another clone: its seal is bound to that clone.
    let other = project.beside(STRICT);
    grant_dispatch(other.path(), "steward", "ops").expect("granted there");
    let theirs = record(other.path());
    let mut held = record(root);
    held["dispatch_mine"]
        .as_array_mut()
        .expect("a list")
        .push(theirs["dispatch_mine"][0].clone());
    held["dispatch_sealed"]["entries"]
        .as_array_mut()
        .expect("a list")
        .push(theirs["dispatch_sealed"]["entries"][0].clone());
    write_record(root, &held);
    assert!(!yours(root).contains(&pair("steward", "ops")));
    assert!(dispatch_unsealed(root).contains(&(DispatchKind::Mine, "steward -> ops".to_owned())));
}

#[test]
fn a_revoked_grant_put_back_from_a_copy_is_not_honoured() {
    use crate::sandbox::local::{grant_dispatch, revoke_dispatch};
    let project = Project::new(STRICT);
    let root = project.root();
    grant_dispatch(root, "steward", "devops").expect("granted");
    grant_dispatch(root, "steward", "qa").expect("granted");
    let copy = record(root);
    assert!(revoke_dispatch(root, "steward", "devops").expect("revoked"));
    // The whole record put back: its counter is behind the keyring's anchor, so none counts.
    write_record(root, &copy);
    assert_eq!(yours(root), []);
    // The entry alone put back beside the live record: it is in no live list.
    assert!(revoke_dispatch(root, "steward", "qa").is_ok());
    let project = Project::new(STRICT);
    let root = project.root();
    grant_dispatch(root, "steward", "devops").expect("granted");
    grant_dispatch(root, "steward", "qa").expect("granted");
    let copy = record(root);
    revoke_dispatch(root, "steward", "devops").expect("revoked");
    let mut held = record(root);
    held["dispatch_mine"]
        .as_array_mut()
        .expect("a list")
        .push(copy["dispatch_mine"][0].clone());
    held["dispatch_sealed"]["entries"]
        .as_array_mut()
        .expect("a list")
        .push(copy["dispatch_sealed"]["entries"][0].clone());
    write_record(root, &held);
    assert_eq!(yours(root), [pair("steward", "qa")]);
}

#[test]
fn there_is_no_bulk_reseal_of_entries_on_disk() {
    use crate::sandbox::local::grant_dispatch;
    let project = Project::new(STRICT);
    let root = project.root();
    grant_dispatch(root, "steward", "qa").expect("granted");
    // A pair written into the list by something other than the app.
    let mut held = record(root);
    held["dispatch_mine"]
        .as_array_mut()
        .expect("a list")
        .push(serde_json::json!({"asking": "steward", "target": "devops"}));
    write_record(root, &held);
    assert_eq!(yours(root), [pair("steward", "qa")]);
    // A later write of the app's, for another answer and for none, seals nothing of it.
    grant_dispatch(root, "steward", "ops").expect("granted");
    crate::sandbox::local::revoke_dispatch(root, "steward", "ops").expect("revoked");
    assert_eq!(yours(root), [pair("steward", "qa")]);
    // Only the per-pair ask grants it.
    grant_dispatch(root, "steward", "devops").expect("granted");
    assert!(yours(root).contains(&pair("steward", "devops")));
}

#[test]
fn an_acceptance_of_the_project_s_grant_is_sealed() {
    let project = Project::new(&format!(
        "{STRICT}[dispatch.grants]\nsteward = [\"devops\"]\n"
    ));
    let root = project.root();
    // Written by hand, as a chat with no sandbox could: no acceptance.
    let mut held = serde_json::json!({"dispatch_seen": ["steward -> devops"]});
    std::fs::create_dir_all(
        crate::sandbox::local::path(root)
            .parent()
            .expect("a folder"),
    )
    .expect("made");
    write_record(root, &held);
    assert_eq!(crate::sandbox::local::dispatch_seen(root), Some(Vec::new()));
    // Accepted in the window: sealed, and in force.
    acknowledge_pair(root, &pair("steward", "devops")).expect("accepted");
    assert_eq!(
        crate::sandbox::local::dispatch_seen(root),
        Some(vec!["steward -> devops".to_owned()])
    );
    assert_eq!(project.grants().project, [pair("steward", "devops")]);
    // And an "any persona" acceptance, written by hand, is none.
    held = record(root);
    held["dispatch_any_seen"] = serde_json::json!(["qa"]);
    write_record(root, &held);
    assert_eq!(
        crate::sandbox::local::dispatch_any_seen(root),
        Vec::<String>::new()
    );
}

#[test]
fn the_hold_arg_writer_refuses_the_seal_service() {
    // Any program can start the app's binary as the one-item writer: were it to write the seal's
    // service, a chat could put a key or an anchor of its choosing there.
    for account in [
        "answers",
        "dispatch-answers-live:0123",
        "dispatch-nevers-live:0123",
        "policy-accepted:0123",
    ] {
        let item = serde_json::json!({
            "service": crate::secrets::seal::SERVICE, "account": account, "value": "00",
        });
        assert!(
            crate::secrets::keyhold::asked(item.to_string().as_bytes()).is_err(),
            "{account}"
        );
    }
}

#[test]
fn the_seal_key_is_unreadable_from_a_sandboxed_chat_and_its_env() {
    // A chat's process is not the app: it seals nothing, verifies nothing, and a grant counts
    // there as none. (The cell that the Keychain refuses the item to a sandboxed chat's process
    // is the matrix's, E-SEC-04.)
    let project = Project::new(STRICT);
    crate::sandbox::local::grant_dispatch(project.root(), "steward", "devops").expect("granted");
    crate::secrets::seal::set_for_this_test(None);
    assert!(!crate::secrets::seal::can_seal());
    assert_eq!(crate::secrets::seal::sealed("k", "c", "t"), None);
    assert_eq!(yours(project.root()), []);
    // Nothing of the seal is in what a chat starts with.
    for (name, value) in std::env::vars() {
        assert!(!name.contains("SEAL"), "{name}");
        assert!(!value.contains(crate::secrets::seal::SERVICE), "{name}");
    }
    crate::secrets::seal::set_for_this_test(Some(vec![7; 32]));
}

#[test]
fn a_deleted_nevers_file_is_not_no_nevers() {
    let project = Project::new(STRICT);
    let root = project.root();
    never(root, &pair("steward", "devops")).expect("said");
    std::fs::remove_file(crate::dispatchnever::path(root)).expect("deleted");
    assert_eq!(
        covers(Some("steward"), "qa", &project.grants(), &Locks::none()),
        Covers::Unread
    );
    // Emptied, every never taken out by hand.
    let project = Project::new(STRICT);
    let root = project.root();
    never(root, &pair("steward", "devops")).expect("said");
    let mut file: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::dispatchnever::path(root)).expect("read"),
    )
    .expect("json");
    file["never"] = serde_json::json!([]);
    std::fs::write(crate::dispatchnever::path(root), file.to_string()).expect("written");
    assert_eq!(
        crate::dispatchnever::read(root),
        crate::dispatchnever::Nevers::Unread
    );
    // And with every never lifted in the window, deleting the file is still not "none".
    let project = Project::new(STRICT);
    let root = project.root();
    never(root, &pair("steward", "devops")).expect("said");
    assert!(lift_never(root, "steward", "devops").expect("lifted"));
    std::fs::remove_file(crate::dispatchnever::path(root)).expect("deleted");
    assert_eq!(
        crate::dispatchnever::read(root),
        crate::dispatchnever::Nevers::Unread
    );
}

#[test]
fn a_never_written_by_something_else_still_refuses() {
    let project = Project::new(STRICT);
    let root = project.root();
    never(root, &pair("steward", "devops")).expect("said");
    let mut file: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::dispatchnever::path(root)).expect("read"),
    )
    .expect("json");
    file["never"]
        .as_array_mut()
        .expect("a list")
        .push(serde_json::json!({"asking": "steward", "target": "qa"}));
    std::fs::write(crate::dispatchnever::path(root), file.to_string()).expect("written");
    assert_eq!(
        covers(Some("steward"), "qa", &project.grants(), &Locks::none()),
        Covers::Never
    );
}

#[test]
fn the_person_s_nevers_from_before_are_imported_once_and_sealed() {
    let project = Project::new(STRICT);
    let root = project.root();
    // A file as a build before the seal wrote it.
    let file = crate::dispatchnever::path(root);
    std::fs::create_dir_all(file.parent().expect("a folder")).expect("made");
    std::fs::write(
        &file,
        r#"{"never": [{"asking": "steward", "target": "devops"}]}"#,
    )
    .expect("written");
    assert_eq!(
        crate::dispatchnever::read(root),
        crate::dispatchnever::Nevers::Read(vec![("steward".to_owned(), "devops".to_owned())])
    );
    let sealed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("read")).expect("json");
    assert!(sealed["never"][0]["mac"].is_string(), "{sealed}");
    // Once: taking the imported never out by hand now reads as unread.
    std::fs::write(&file, r#"{"never": []}"#).expect("written");
    assert_eq!(
        crate::dispatchnever::read(root),
        crate::dispatchnever::Nevers::Unread
    );
}

#[test]
fn an_unreadable_key_with_nevers_present_refuses_all_cross_persona_dispatch() {
    let project = Project::new(STRICT);
    let root = project.root();
    never(root, &pair("steward", "devops")).expect("said");
    crate::sandbox::local::grant_dispatch(root, "steward", "qa").expect("granted");
    crate::secrets::seal::set_for_this_test(None);
    let grants = project.grants();
    assert_eq!(
        covers(Some("steward"), "qa", &grants, &Locks::none()),
        Covers::Unread
    );
    assert_eq!(
        covers(Some("steward"), "steward", &grants, &Locks::none()),
        Covers::Covered
    );
    // The file gone too: the keyring's marker still says nevers were kept.
    std::fs::remove_file(crate::dispatchnever::path(root)).expect("deleted");
    assert_eq!(
        crate::dispatchnever::read(root),
        crate::dispatchnever::Nevers::Unread
    );
    crate::secrets::seal::set_for_this_test(Some(vec![7; 32]));
}

#[test]
fn an_unreadable_seal_key_honours_no_grant_and_raises_a_notice() {
    let project = Project::new(&format!("{STRICT}[dispatch.grants]\nsteward = [\"ops\"]\n"));
    let root = project.root();
    crate::sandbox::local::grant_dispatch(root, "steward", "devops").expect("granted");
    acknowledge_pair(root, &pair("steward", "ops")).expect("accepted");
    assert_eq!(project.grants().project, [pair("steward", "ops")]);
    crate::secrets::seal::set_for_this_test(None);
    let grants = project.grants();
    // Neither grant counts. The `[policy]` this machine accepted on the first read cannot be
    // verified either, so what the person refused is unknown too (the review of #7113): every
    // dispatch across personas waits, none is covered.
    for target in ["devops", "ops"] {
        assert_eq!(
            as_the_policy_runs(
                covers(Some("steward"), target, &grants, &Locks::none()),
                strict()
            ),
            Covers::Unread,
            "{target}"
        );
    }
    // What the window's Notice reads.
    assert!(crate::sandbox::local::dispatch_seals_unread(root));
    // A new grant is refused, with what to do, rather than kept where it would not hold.
    let refused = crate::sandbox::local::grant_dispatch(root, "steward", "qa")
        .expect_err("not kept")
        .to_string();
    assert!(refused.contains("allow it to use its key"), "{refused}");
    crate::secrets::seal::set_for_this_test(Some(vec![7; 32]));
    assert!(!crate::sandbox::local::dispatch_seals_unread(root));
}

#[test]
fn off_macos_an_unsandboxed_unattended_chat_is_still_refused_with_why() {
    let grants = InForce {
        you: vec![pair("steward", "devops")],
        ..InForce::default()
    };
    let answer = unattended::as_the_policy_runs(
        unattended::answer_on(
            covers(Some("steward"), "devops", &grants, &Locks::none()),
            Some("steward"),
            "devops",
            false,
            Reach {
                sandboxed: false,
                sealed_here: false,
            },
        ),
        strict(),
    );
    let Answer::Refused(refusal) = answer else {
        panic!("refused: {answer:?}")
    };
    assert_eq!(refusal, Refusal::NotSealedHere("devops".to_owned()));
    assert!(
        refusal.say().contains("only the macOS Keychain"),
        "{}",
        refusal.say()
    );
    // Not kept for the needs-you list: a grant stands already.
    assert_eq!(crate::dispatchaway::pair_kept(&refusal), None);
    // The nevers still hold there.
    let mut held = grants.clone();
    held.never = vec![("steward".to_owned(), "devops".to_owned())];
    assert_eq!(
        covers(Some("steward"), "devops", &held, &Locks::none()),
        Covers::Never
    );
    // On this system, as the app asks it.
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        unattended::covers(
            Some("steward"),
            "devops",
            &grants,
            &Locks::none(),
            false,
            false
        ),
        Answer::Refused(Refusal::NotSealedHere("devops".to_owned()))
    );
}

// ---- the family's log, and a dispatched chat's policy ---------------------------------------------

#[test]
fn under_log_a_deny_and_an_ask_go_ahead_and_the_asking_chat_s_log_names_the_rule() {
    const CHAT: &str = "01JABCDEFGHJKMNPQRSTVWXYZ0";
    let text = "[policy]\ntemplate = \"strict\"\ndispatch = \"log\"\n";
    let project = Project::new(text);
    let root = project.root();
    let denied = InForce {
        denied: vec![pair("steward", "devops")],
        ..InForce::default()
    };
    let mode = Resolved::of(
        &Settings::from_text(Some(text)),
        &crate::sandbox::policy::Locks::none(),
    )
    .mode(Family::Dispatch);
    assert_eq!(mode, Mode::Log);
    for (grants, rule) in [(&denied, DENY_RULE), (&InForce::default(), ASK_RULE)] {
        assert_eq!(
            as_the_policy_runs_for(
                root,
                Some(CHAT),
                covers(Some("steward"), "devops", grants, &Locks::none()),
                mode
            ),
            Covers::Covered
        );
        let logged = crate::enforcement::read_log(root, CHAT);
        assert_eq!(logged.last().map(|line| line.rule.as_str()), Some(rule));
        assert_eq!(
            logged.last().map(|line| line.family.as_str()),
            Some("dispatch")
        );
    }
    // Under enforce, nothing is logged and the deny stands.
    assert_eq!(
        as_the_policy_runs_for(
            root,
            Some(CHAT),
            covers(Some("steward"), "devops", &denied, &Locks::none()),
            strict()
        ),
        Covers::Denied(DENY_RULE.to_owned())
    );
    assert_eq!(crate::enforcement::read_log(root, CHAT).len(), 2);
}

#[test]
fn a_dispatched_chat_is_never_looser_than_its_asker() {
    let project = Resolved::of(
        &Settings::from_text(Some("[policy]\ntemplate = \"open\"\n")),
        &crate::sandbox::policy::Locks::none(),
    );
    let asker = Resolved::of(
        &Settings::from_text(Some(STRICT)),
        &crate::sandbox::policy::Locks::none(),
    );
    let child = project.clone().for_chat(Some(&asker.spelled()));
    for family in Family::ALL {
        assert!(child.mode(family) >= asker.mode(family), "{}", family.key());
        assert!(
            child.mode(family) >= project.mode(family),
            "{}",
            family.key()
        );
    }
    assert_eq!(child.mode(Family::Dispatch), Mode::Enforce);
    // A chat nobody dispatched runs as its project does.
    assert_eq!(project.clone().for_chat(None), project);
}

// ---- the review of #7113: denies read as `[policy]` is, and anchors that follow the plane ----------

/// Runs git in `root`, with no configuration of the person's, and asserts it did what it said.
fn git(root: &Path, args: &[&str]) {
    let mut git = std::process::Command::new("git");
    git.current_dir(root)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    let ran = crate::forklock::output(&mut git).expect("git runs");
    assert!(ran.status.success(), "{args:?}: {ran:?}");
}

/// A project whose settings are committed at `HEAD`.
fn committed(settings: &str) -> Project {
    let project = Project::new(settings);
    git(project.root(), &["init", "-q"]);
    git(project.root(), &["add", "-A"]);
    git(project.root(), &["commit", "-q", "-m", "settings"]);
    project
}

const DENIES: &str = "[dispatch.denies]\nsteward = [\"devops\"]\n";

#[test]
fn a_deny_taken_out_of_the_working_tree_still_denies() {
    let project = committed(&format!("{STRICT}{DENIES}"));
    let root = project.root();
    assert!(project.grants().denies(Some("steward"), "devops"));
    // A chat deletes the table from the file as it stands, and commits nothing.
    std::fs::write(crate::names::manifest(root), STRICT).expect("the working tree");
    assert!(project.grants().denies(Some("steward"), "devops"));
    assert!(matches!(
        covers(Some("steward"), "devops", &project.grants(), &Locks::none()),
        Covers::Denied(_)
    ));
}

#[test]
fn a_deny_the_project_takes_away_holds_until_the_person_presses() {
    let project = committed(&format!("{STRICT}{DENIES}"));
    let root = project.root();
    // Read once: the deny is accepted as it is seen, a tightening needing no press.
    assert!(project.grants().denies(Some("steward"), "devops"));
    // A commit takes it out: a loosening, which waits for the person.
    std::fs::write(crate::names::manifest(root), STRICT).expect("written");
    git(root, &["commit", "-q", "-a", "-m", "no deny"]);
    assert!(project.grants().denies(Some("steward"), "devops"));
    let standing = crate::enforcement::standing(root);
    assert_eq!(standing.lifted, ["steward -> devops"]);
    crate::enforcement::press(root, &standing.digest).expect("pressed");
    assert!(!project.grants().denies(Some("steward"), "devops"));
    assert!(crate::enforcement::standing(root).lifted.is_empty());
}

#[test]
fn a_project_file_that_does_not_read_denies_every_dispatch_across_personas() {
    // A folder where the file should be.
    let project = Project::new(STRICT);
    let root = project.root();
    std::fs::remove_file(crate::names::manifest(root)).expect("removed");
    std::fs::create_dir(crate::names::manifest(root)).expect("a folder");
    let grants = project.grants();
    assert!(grants.denies(Some("steward"), "devops"));
    assert!(!grants.denies(Some("steward"), "steward"));
    // Not TOML.
    let project = Project::new("[dispatch.denies\nsteward = ");
    assert!(project.grants().denies(Some("qa"), "ops"));
    // A table of denies that is no table, and a list that holds what is no name.
    let project = Project::new(&format!("{STRICT}[dispatch]\ndenies = 3\n"));
    assert!(project.grants().denies(Some("qa"), "ops"));
    let project = Project::new(&format!("{STRICT}[dispatch.denies]\nsteward = [3]\n"));
    let grants = project.grants();
    assert!(grants.denies(Some("steward"), "ops"));
    assert!(!grants.denies(Some("qa"), "ops"));
}

#[test]
fn a_plane_moved_away_with_a_link_at_its_old_path_keeps_its_nevers() {
    let _home = crate::sandbox::local::OwnConfigHome::new();
    let parent = tempfile::tempdir().expect("a folder");
    let old = parent.path().join("plane");
    std::fs::create_dir(&old).expect("made");
    std::fs::write(crate::names::manifest(&old), STRICT).expect("written");
    never(&old, &pair("steward", "devops")).expect("said");
    // Moved, its nevers file taken away, and a link left where it was.
    let moved = parent.path().join("elsewhere");
    std::fs::rename(&old, &moved).expect("moved");
    std::fs::remove_file(crate::dispatchnever::path(&moved)).expect("deleted");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&moved, &old).expect("linked");
    #[cfg(unix)]
    assert_eq!(
        crate::dispatchnever::read(&old),
        crate::dispatchnever::Nevers::Unread
    );
}

#[test]
fn a_nevers_file_the_app_wrote_with_no_anchor_now_is_not_no_nevers() {
    let project = Project::new(STRICT);
    let root = project.root();
    never(root, &pair("steward", "devops")).expect("said");
    assert!(lift_never(root, "steward", "devops").expect("lifted"));
    // The anchor taken out of the keyring: the file says the app wrote it.
    crate::secrets::seal::forget_kept_for_this_test();
    assert_eq!(
        crate::dispatchnever::read(root),
        crate::dispatchnever::Nevers::Unread
    );
}

#[test]
fn a_grant_revoked_while_the_key_does_not_read_is_not_honoured_once_it_does() {
    use crate::sandbox::local::{grant_dispatch, revoke_dispatch};
    let project = Project::new(STRICT);
    let root = project.root();
    grant_dispatch(root, "steward", "devops").expect("granted");
    let copy = record(root);
    crate::secrets::seal::set_for_this_test(None);
    assert!(revoke_dispatch(root, "steward", "devops").expect("revoked"));
    crate::secrets::seal::set_for_this_test(Some(vec![7; 32]));
    // The row put back as it was.
    let mut held = record(root);
    held["dispatch_mine"] = copy["dispatch_mine"].clone();
    write_record(root, &held);
    assert_eq!(yours(root), []);
}

#[test]
fn a_seal_never_follows_a_grant_to_a_wider_workspace() {
    use crate::sandbox::local::{DispatchIn, Limited, give_back_dispatch, keep_dispatch_in};
    let project = Project::new(STRICT);
    let root = project.root();
    keep_dispatch_in(
        root,
        Limited::Mine,
        &DispatchIn {
            asking: "steward".to_owned(),
            target: "devops".to_owned(),
            any: false,
            workspace: "runners".to_owned(),
            seen: 0,
        },
    )
    .expect("kept");
    // A row planted among the grants set aside, for every workspace, then given back.
    let mut held = record(root);
    held["dispatch_dormant"] =
        serde_json::json!([{"asking": "steward", "target": "devops", "was": "devops"}]);
    write_record(root, &held);
    give_back_dispatch(root, "devops", "", &|_| true, &|_| true).expect("given back");
    assert_eq!(yours(root), []);
}

#[test]
fn a_workspace_count_raised_by_hand_counts_for_nothing() {
    use crate::sandbox::local::{DispatchIn, Limited, dispatch_mine_in, keep_dispatch_in};
    let project = Project::new(STRICT);
    let root = project.root();
    keep_dispatch_in(
        root,
        Limited::Mine,
        &DispatchIn {
            asking: "steward".to_owned(),
            target: "devops".to_owned(),
            any: false,
            workspace: "runners".to_owned(),
            seen: 0,
        },
    )
    .expect("kept");
    assert_eq!(dispatch_mine_in(root).len(), 1);
    let mut held = record(root);
    held["dispatch_mine_in"][0]["seen"] = 1.into();
    write_record(root, &held);
    assert_eq!(dispatch_mine_in(root), []);
}

#[test]
fn a_machine_with_no_keyring_has_no_nevers_rather_than_unknown_ones() {
    let project = Project::new(STRICT);
    let root = project.root();
    crate::secrets::seal::set_no_keyring_for_this_test(true);
    assert_eq!(
        crate::dispatchnever::read(root),
        crate::dispatchnever::Nevers::Read(Vec::new())
    );
    assert_eq!(
        covers(Some("steward"), "devops", &project.grants(), &Locks::none()),
        Covers::NeedsGrant
    );
    // And nothing is kept that would not hold.
    assert!(crate::sandbox::local::grant_dispatch(root, "steward", "devops").is_err());
    crate::secrets::seal::set_no_keyring_for_this_test(false);
}
