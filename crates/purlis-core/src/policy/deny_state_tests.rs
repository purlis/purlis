// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::policy::decide::{self, Asker, Sealed};
use crate::policy::levels::Chat;
use crate::sandbox::local::OwnConfigHome;
use crate::secrets::answers::{self, Lost, Press};
use crate::secrets::seal;

/// A machine of the test's own: its config home, a keyring that knows nothing, a key.
fn machine() -> (OwnConfigHome, tempfile::TempDir) {
    let home = OwnConfigHome::new();
    seal::forget_kept_for_this_test();
    seal::set_for_this_test(Some(vec![7; 32]));
    seal::set_foreign_for_this_test(false);
    seal::set_locked_for_this_test(false);
    seal::set_no_keyring_for_this_test(false);
    (home, tempfile::tempdir().expect("a folder for planes"))
}

#[allow(clippy::disallowed_methods, reason = "a test plays the person's press")]
fn press() -> Press {
    Press::of_the_persons_press()
}

fn asker() -> Asker<'static> {
    Asker {
        chat: 1,
        of: Chat {
            workspace: None,
            persona: None,
        },
        sandbox: "floor",
        attended: true,
        harness_mode: "default",
        depth: 0,
        via: "hook",
        start: None,
    }
}

fn denied(causes: Vec<Deny>) -> Standing {
    Standing {
        state: State::Deny(causes),
        entries: Vec::new(),
        kept: Vec::new(),
        store_verified: false,
        everywhere: Vec::new(),
        no_answer: Vec::new(),
        notices: Vec::new(),
    }
}

/// The real store: a plane that sealed a never and then had its `.git` swapped, as the app reads
/// it, is refused for every action but `start`, by the standing the store gives.
#[test]
fn in_the_deny_state_every_action_but_start_is_refused_and_a_re_press_is_raised() {
    let (_home, planes) = machine();
    let root = std::fs::canonicalize(planes.path()).unwrap().join("p");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    answers::register(&root).expect("registered");
    answers::commit(&root, |draft| draft.add(&press(), answers::NEVER, "rm -rf")).expect("sealed");
    let sealed = Sealed::of(&root);
    assert!(!sealed.standing.is_deny(), "{:?}", sealed.standing);
    for action in ["exec", "merge", "connect", "write", "read"] {
        assert!(
            refused(action, &sealed.standing).is_none(),
            "{action} is not refused while the store holds"
        );
    }

    // `.git` swapped at an anchored path: the deny state.
    // Made beside the old one before it goes, so its inode is another (a freed inode is reused).
    std::fs::rename(root.join(".git"), root.join(".git-old")).unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::remove_dir_all(root.join(".git-old")).unwrap();
    let sealed = Sealed::of(&root);
    assert!(sealed.standing.is_deny(), "{:?}", sealed.standing);
    for (action, resource) in [
        ("exec", ("Command", "ls")),
        ("merge", ("Branch", "acme/web#main")),
        ("connect", ("Host", "example.org")),
        ("read", ("Path", "/tmp/x")),
    ] {
        // Through the decision function itself, the root never read: the refusal comes first.
        let said = decide::in_plane(&root, &asker(), action, resource, &sealed);
        assert_eq!(said.outcome, Outcome::Deny, "{action}: {said:?}");
        assert_eq!(said.after_mode, Outcome::Deny);
        assert_eq!(said.ids(), [RULE_ID], "{action}: {said:?}");
        let state = said.deny_state.as_ref().expect("it names the deny state");
        assert!(
            state.causes.iter().any(|c| c.contains("swapped")),
            "{state:?}"
        );
        // A re-press is raised, and says what it does.
        assert!(
            state.re_press.contains("press to start again"),
            "{}",
            state.re_press
        );
        assert!(said.deny_state.is_some() && !said.rules_said().is_empty());
    }
}

/// Where the standing is not the deny state nothing is refused, whatever the state: fresh, holds,
/// no keyring, not the app.
#[test]
fn only_the_deny_state_refuses() {
    for state in [
        State::Fresh,
        State::NoKeyring,
        State::Holds,
        State::Unverifiable,
    ] {
        let standing = Standing {
            state,
            ..Standing::fresh()
        };
        assert!(refused("exec", &standing).is_none());
        assert!(DenyState::of(&standing).is_none());
        assert!(RePress::of(&standing).is_none());
    }
}

/// A `start` is judged by the rules in the deny state, never refused by the deny state itself, so
/// the chat starts and only a rule's forbid on it refuses it.
#[test]
fn the_deny_state_never_refuses_a_start_itself() {
    let standing = denied(vec![Deny::CloneKeySwapped]);
    assert!(refused("start", &standing).is_none());
    assert!(refused("exec", &standing).is_some());
}

#[test]
fn the_re_press_after_a_lost_store_diffs_against_the_shipped_default() {
    let standing = denied(vec![Deny::ManifestOlder(Lost {
        writes: 4,
        nevers: 2,
        floors: 1,
    })]);
    let said = RePress::of(&standing).expect("raised").said();
    assert!(
        said.contains("shipped default holds no nevers and no accepted floor"),
        "{said}"
    );
    assert!(said.contains("press to start again"), "{said}");
}

#[test]
fn the_re_press_names_how_many_nevers_and_accepted_floor_versions_were_lost() {
    let standing = denied(vec![Deny::ManifestMissing(Lost {
        writes: 9,
        nevers: 3,
        floors: 2,
    })]);
    let raised = RePress::of(&standing).expect("raised");
    assert_eq!((raised.nevers, raised.floors), (3, 2));
    let said = raised.said();
    assert!(said.contains("3 nevers and 2 accepted versions"), "{said}");
    // One of each reads in the singular.
    let one = denied(vec![Deny::ManifestOlder(Lost {
        writes: 1,
        nevers: 1,
        floors: 1,
    })]);
    assert!(
        RePress::of(&one)
            .unwrap()
            .said()
            .contains("1 never and 1 accepted version in"),
        "{}",
        RePress::of(&one).unwrap().said()
    );
}

#[test]
fn the_re_press_names_how_many_everywhere_nevers_were_lost() {
    let standing = denied(vec![
        Deny::ManifestOlder(Lost {
            writes: 1,
            nevers: 1,
            floors: 0,
        }),
        Deny::EverywhereOlder(Lost {
            writes: 2,
            nevers: 5,
            floors: 0,
        }),
    ]);
    let raised = RePress::of(&standing).expect("raised");
    assert_eq!(raised.everywhere_nevers, 5);
    let said = raised.said();
    assert!(said.contains("5 nevers that apply everywhere"), "{said}");
    // Only the Everywhere entry older: this project lost none, and the sentence still says so.
    let only = denied(vec![Deny::EverywhereOlder(Lost {
        writes: 1,
        nevers: 2,
        floors: 0,
    })]);
    let said = RePress::of(&only).unwrap().said();
    assert!(said.contains("0 nevers"), "{said}");
    assert!(said.contains("2 nevers that apply everywhere"), "{said}");
}

/// A cause that does not say what was lost never invents a number.
#[test]
fn the_re_press_invents_no_count_where_the_cause_does_not_say() {
    let standing = denied(vec![Deny::CloneKeySwapped]);
    let raised = RePress::of(&standing).expect("raised");
    assert!(!raised.counted);
    assert!(!raised.said().contains("Lost:"), "{}", raised.said());
}

/// No Ask the host answers over the socket seals, accepts or confirms: the press is a value only
/// the window makes, and the wire has no word for it. A chat that sends one is not understood.
#[test]
fn a_chat_cannot_obtain_a_seal_over_the_host_socket() {
    for line in [
        r#"{"seal":{"kind":"never","text":"x"}}"#,
        r#"{"accept":{}}"#,
        r#"{"press":{}}"#,
        r#"{"recommit":{"entries":[]}}"#,
        r#"{"answers":{"kind":"grant","text":"once"}}"#,
    ] {
        assert!(
            serde_json::from_str::<crate::hookwire::Ask>(line).is_err(),
            "the wire understood {line}"
        );
    }
}

#[test]
fn a_chat_cannot_confirm_the_device_id() {
    for line in [
        r#"{"confirm_device":{"id":"01JABCDEFGHJKMNPQRSTVWXYZ0"}}"#,
        r#"{"confirm":{"id":"01JABCDEFGHJKMNPQRSTVWXYZ0"}}"#,
        r#"{"device":{"confirm":true}}"#,
    ] {
        assert!(
            serde_json::from_str::<crate::hookwire::Ask>(line).is_err(),
            "the wire understood {line}"
        );
    }
    // And the id is unconfirmed until the window's press, whatever a chat said: no `context.machine`.
    let (_home, planes) = machine();
    let root = std::fs::canonicalize(planes.path()).unwrap().join("p");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    answers::register(&root).expect("registered");
    assert_eq!(answers::context_machine(), None);
    assert_eq!(Sealed::of(&root).machine, None);
    let answers::Device::Unconfirmed(id) = answers::device() else {
        panic!("an unconfirmed id");
    };
    answers::confirm_device(&press(), &id).expect("the person's press");
    assert_eq!(Sealed::of(&root).machine.as_deref(), Some(id.as_str()));
}

/// A start where the standing is the deny state is compiled at `full` with the minimum permits,
/// and where it is not, the person's own level decides: the start says nothing of its own.
#[cfg(unix)]
#[test]
fn a_start_in_the_deny_state_gets_the_deny_sandbox_and_a_start_outside_it_gets_none_of_its_own() {
    let (_home, planes) = machine();
    let root = std::fs::canonicalize(planes.path()).unwrap().join("p");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("purlis.toml"), "schema = 1\n").unwrap();
    let machine = crate::sandbox::Machine::this();
    let has = |_: &str| true;
    let harness = crate::harness::Harness::ClaudeCode;
    let fresh = Standing::fresh();
    assert!(
        crate::start::deny_state_sandbox(harness, &root, &machine, &has, &fresh).is_none(),
        "no deny state, no word of its own"
    );
    let held = crate::start::deny_state_sandbox(
        harness,
        &root,
        &machine,
        &has,
        &denied(vec![Deny::CloneKeySwapped]),
    );
    // Where this machine has a backend the chat is sandboxed; where it has none the start is
    // refused, never started bare.
    match held.expect("the deny state decides the start") {
        Ok(applied) => assert_eq!(applied.level(), crate::sandbox::Level::Full, "{applied:?}"),
        Err(refused) => assert!(!refused.to_string().is_empty()),
    }
}

/// F5: where a start is composed outside the app the standing is `Unverifiable`, and that is
/// decided, not left to `is_deny()`: an anchored plane is started in the deny sandbox (fail
/// closed), and one nothing was ever sealed for is the person's own level, as a fresh install.
#[test]
fn a_start_composed_outside_the_app_is_in_the_deny_sandbox_only_on_an_anchored_plane() {
    let (_home, planes) = machine();
    let base = std::fs::canonicalize(planes.path()).unwrap();
    let anchored = base.join("anchored");
    let unsealed = base.join("unsealed");
    for root in [&anchored, &unsealed] {
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("purlis.toml"), "schema = 1\n").unwrap();
    }
    answers::register(&anchored).expect("registered by the app");
    // A process that is not the app has no seal key.
    seal::set_for_this_test(None);
    let standing = answers::standing(&anchored);
    assert!(standing.is_unverifiable(), "{standing:?}");
    assert!(!standing.is_deny());
    let machine = crate::sandbox::Machine::this();
    let has = |_: &str| true;
    let harness = crate::harness::Harness::ClaudeCode;
    let held = crate::start::deny_state_sandbox(harness, &anchored, &machine, &has, &standing);
    match held.expect("an anchored plane that cannot be verified is started as denied") {
        Ok(applied) => assert_eq!(applied.level(), crate::sandbox::Level::Full, "{applied:?}"),
        Err(refused) => assert!(!refused.to_string().is_empty()),
    }
    assert!(
        crate::start::deny_state_sandbox(
            harness,
            &unsealed,
            &machine,
            &has,
            &answers::standing(&unsealed)
        )
        .is_none(),
        "nothing was sealed for a plane the app never anchored"
    );
}

/// F4: a harness the project declares has no adapter and so no deny sandbox: in the deny state its
/// start is refused, never started bare, whatever the person's opt-out says.
#[test]
fn a_declared_harness_is_refused_in_the_deny_state() {
    crate::unsteered!();
    let (_home, planes) = machine();
    let root = std::fs::canonicalize(planes.path()).unwrap().join("p");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("purlis.toml"), "schema = 1\n").unwrap();
    let dir = root.join(crate::harness_declaration::DIR);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("shelly.toml"),
        "name = \"shelly\"\ntitle = \"Shelly\"\nprogram = \"true\"\n\
         [session]\nchosen_by = \"purlis\"\nnew = [\"--session={id}\"]\n\
         resume = [\"--resume={id}\"]\nnamed_by = [\"--resume\"]\n\
         [capabilities]\nresumes_by_id = \"yes\"\n",
    )
    .unwrap();
    let set = crate::profiles::current(&root);
    let profile = set.get("shelly").expect("the declaration gave a profile");
    let shown = crate::profiletrust::shown(&root, profile);
    crate::profiletrust::approve(&root, profile, &shown).expect("approved");
    let start = crate::start::Start {
        profile: Some("shelly".to_owned()),
        persona: None,
        name: "ide.7".to_owned(),
        cwd: Some(root.clone()),
        resume: None,
        show_footer: false,
        resuming: None,
        without_sandbox: Some(crate::sandbox::OptOut::default()),
        held: None,
        grants: Default::default(),
        made: crate::sandbox::Made::Window,
    };
    // An anchored plane that cannot be verified here is the deny state for a start.
    answers::register(&root).expect("registered by the app");
    seal::set_for_this_test(None);
    let refused = crate::start::ready(&start, &root).expect_err("refused in the deny state");
    assert_eq!(refused, crate::start::DECLARED_HARNESS_IN_THE_DENY_STATE);
}
