// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A project whose vault `devops` is tagged for persona `devops`, and `loose` for nobody.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a project");
    std::fs::write(
        dir.path().join("vaults.json"),
        serde_json::json!({ "vaults": {
            "devops": {"provider": "plain-file", "config": {"file": "d.json"}, "persona": "devops"},
            "loose": {"provider": "plain-file", "config": {"file": "l.json"}},
        }})
        .to_string(),
    )
    .expect("a registry");
    dir
}

fn plane(root: &std::path::Path) -> PlaneId {
    PlaneId::for_tests(root)
}

/// Chat 3, as the app recorded it: started sandboxed, as `persona`.
fn asker(root: &std::path::Path, persona: Option<&str>) -> brokered::Asker {
    brokered::Asker {
        root: root.to_path_buf(),
        env: Env::of(&[]),
        chat: 3,
        persona: persona.map(str::to_owned),
        folder: None,
        confines: Some(sandbox::Confines::default()),
        harness: None,
    }
}

fn steward() -> Chat<'static> {
    Chat {
        session: 3,
        persona: Some("steward"),
        name: Some("steward 1".to_owned()),
    }
}

fn told() -> (
    Arc<Mutex<Vec<VaultRefused>>>,
    impl Fn(VaultRefused) + Send + Sync,
) {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let into = Arc::clone(&heard);
    (heard, move |one| into.lock().unwrap().push(one))
}

type Audits = Mutex<Vec<(Option<u32>, &'static str, String, String)>>;

fn auditing(
    heard: &Audits,
) -> impl Fn(Option<u32>, &sandbox::grant::Audited<'_>) -> Result<(), String> + '_ {
    |number, audited| {
        heard.lock().unwrap().push((
            number,
            audited.kind(),
            audited.what.to_owned(),
            audited.target.to_owned(),
        ));
        Ok(())
    }
}

#[test]
fn a_refused_vault_raises_one_notice_however_often_the_chat_tries() {
    let project = project();
    let root = project.path();
    let refusals = Refusals::default();
    let (heard, tell) = told();
    for _ in 0..3 {
        refused(
            &plane(root),
            &refusals,
            &asker(root, Some("steward")),
            false,
            "devops",
            &tell,
        );
    }
    assert_eq!(
        *heard.lock().unwrap(),
        [VaultRefused {
            plane: plane(root),
            session: 3,
            vault: "devops".to_owned(),
            persona: "steward".to_owned(),
            tagged_for: Some("devops".to_owned()),
            // The fixture defines no persona, so there is nobody to dispatch to.
            dispatch_to: None,
            locked: None,
            login: None,
        }]
    );
    assert_eq!(refusals.of(3).len(), 1);
}

/// #2692: the Notice goes by the record of how the chat's run started, never by what the window
/// holds for it now. A chat a sandbox holds whose compile the window keeps no record of (a
/// runner's, or one reattached under a sandbox that changed) is still told.
#[test]
fn a_chat_whose_run_started_sandboxed_is_told_though_no_compile_of_its_sandbox_is_held() {
    let project = project();
    let root = project.path();
    let refusals = Refusals::default();
    let (heard, tell) = told();
    let mut held = asker(root, Some("steward"));
    held.confines = None;
    refused(&plane(root), &refusals, &held, false, "devops", &tell);
    assert_eq!(heard.lock().unwrap().len(), 1, "told once");
    assert_eq!(refusals.of(3).len(), 1);
}

#[test]
fn nothing_is_raised_where_there_is_nothing_to_allow() {
    let project = project();
    let root = project.path();
    let refusals = Refusals::default();
    let (heard, tell) = told();
    // Its own vault, a vault nobody registered, a chat on no persona, and a chat the app
    // did not start sandboxed, which reads its vault itself.
    refused(
        &plane(root),
        &refusals,
        &asker(root, Some("devops")),
        false,
        "devops",
        &tell,
    );
    refused(
        &plane(root),
        &refusals,
        &asker(root, Some("steward")),
        false,
        "made-up",
        &tell,
    );
    refused(
        &plane(root),
        &refusals,
        &asker(root, None),
        false,
        "devops",
        &tell,
    );
    // Started without the sandbox, by the record of how its run started (#2692).
    refused(
        &plane(root),
        &refusals,
        &asker(root, Some("steward")),
        true,
        "devops",
        &tell,
    );
    assert_eq!(*heard.lock().unwrap(), []);
    assert_eq!(refusals.of(3), []);
}

#[test]
fn allow_is_audited_kept_on_this_machine_and_never_written_to_the_registry() {
    let project = project();
    let root = project.path();
    let registry_before = std::fs::read_to_string(root.join("vaults.json")).unwrap();
    let refusals = Refusals::default();
    let (_, tell) = told();
    refused(
        &plane(root),
        &refusals,
        &asker(root, Some("steward")),
        false,
        "devops",
        &tell,
    );
    let heard = Audits::default();
    let Ok(Allowed::Made { grant, .. }) =
        allow(root, &refusals, &steward(), "devops", &auditing(&heard), 42)
    else {
        panic!("allowed");
    };
    assert_eq!(grant.target(), "devops for steward");
    assert_eq!(
        *heard.lock().unwrap(),
        [(
            Some(3),
            "trust.sandbox.grant",
            "vault".to_owned(),
            "devops for steward".to_owned()
        )]
    );
    assert_eq!(
        sandbox::local::granted_vaults(root),
        std::slice::from_ref(&grant)
    );
    assert_eq!(
        sandbox::local::made(root),
        [sandbox::local::Made {
            what: "vault".to_owned(),
            target: "devops for steward".to_owned(),
            level: "you".to_owned(),
            at: 42,
            chat: Some("steward 1".to_owned()),
        }]
    );
    assert_eq!(
        std::fs::read_to_string(root.join("vaults.json")).unwrap(),
        registry_before,
        "the committed registry is not edited from a Notice"
    );
    // The same chat's next run is authorised, with nothing restarted.
    let ctx = Ctx::new(root, Env::of(&[]));
    assert_eq!(brokered::authorise(&ctx, Some("steward"), "devops"), Ok(()));
    assert_eq!(refusals.of(3), [], "the Notice is answered");
    assert_eq!(told_allowed(&grant, false), TOLD_DEVOPS);
}

/// What chat 3 is told once steward is allowed `devops`, where steward declares no tool
/// with it: what now runs, and nothing else (qw385).
const TOLD_DEVOPS: &str = "⬢ purlis: using vault `devops` — Allowed for chats opened as \
                           `steward` in this project on this machine — retry. `purlis \
                           secret list devops` now lists its key names here. Reading a value \
                           directly (`secret get`, `secret cp`) stays refused in a sandboxed \
                           chat.";

/// The same, where steward declares a tool with `devops`.
const TOLD_DEVOPS_TOOLED: &str = if purlis_core::secrets::brokered::runs_a_chats_command_here() {
    "⬢ purlis: using vault `devops` — Allowed for chats opened as `steward` in this project \
     on this machine — retry. `purlis secret list devops` now lists its key names here, \
     and the tools steward declares with it now run (`purlis secret exec devops … -- \
     <tool>`). Reading a value directly (`secret get`, `secret cp`) stays refused in a \
     sandboxed chat."
} else {
    "⬢ purlis: using vault `devops` — Allowed for chats opened as `steward` in this project \
     on this machine — retry. `purlis secret list devops` now lists its key names here. \
     Reading a value directly (`secret get`, `secret cp`) stays refused in a sandboxed \
     chat, and a command that uses one runs in a terminal outside the chat on this system."
};

#[test]
fn the_allowed_chat_is_told_what_now_runs_and_a_tool_only_where_one_is_declared() {
    let grant = sandbox::local::VaultGrant {
        vault: "devops".to_owned(),
        persona: "steward".to_owned(),
    };
    for (tooled, said) in [(false, TOLD_DEVOPS), (true, TOLD_DEVOPS_TOOLED)] {
        let told = told_allowed(&grant, tooled);
        assert_eq!(told, said);
        assert!(!told.contains("again"), "{told}");
        assert!(told.contains("purlis secret list devops"), "{told}");
        assert!(told.contains("stays refused"), "{told}");
    }
    assert!(!told_allowed(&grant, false).contains("secret exec"));
}

#[test]
fn whether_a_persona_declares_a_tool_with_the_vault_is_read_from_the_project() {
    let project = project();
    let root = project.path();
    assert!(!brokered::declares_a_tool_with(root, "steward", "devops"));
    std::fs::write(
        purlis_core::names::manifest(root),
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.steward]\ntools = { kubectl = { vault = \"devops\", as = \
         \"KUBECONFIG file\", key = \"kubeconfig\" } }\n",
    )
    .unwrap();
    assert!(brokered::declares_a_tool_with(root, "steward", "devops"));
    assert!(!brokered::declares_a_tool_with(root, "steward", "loose"));
    assert!(!brokered::declares_a_tool_with(root, "devops", "devops"));
}

/// Chat `session` as the app recorded it: started sandboxed, as steward.
fn steward_chat(root: &std::path::Path, session: u32) -> brokered::Asker {
    brokered::Asker {
        chat: session,
        ..asker(root, Some("steward"))
    }
}

#[test]
fn an_allow_answers_every_chat_it_cured_and_a_second_press_says_already_allowed() {
    // Screenshot 43: two steward chats were refused the same vault. The Allow on one made
    // the grant; the other's Notice stayed, and each press there said "not refused".
    let project = project();
    let root = project.path();
    let refusals = Refusals::default();
    let (_, tell) = told();
    for session in [3, 4] {
        refused(
            &plane(root),
            &refusals,
            &steward_chat(root, session),
            false,
            "devops",
            &tell,
        );
    }
    let heard = Audits::default();
    let made = allow(root, &refusals, &steward(), "devops", &auditing(&heard), 1);
    let Ok(Allowed::Made { cured, .. }) = &made else {
        panic!("made: {made:?}");
    };
    // Chat 4 is told what now runs too, as chat 3 is (review of #1967).
    assert_eq!(cured, &[4]);
    assert_eq!(
        refusals.of(4),
        [],
        "chat 4's refusal is answered by the same grant"
    );

    // A press on chat 4's Notice, drawn before the window re-read it: already allowed,
    // nothing audited again, nothing kept twice, and no Notice left.
    let four = Chat {
        session: 4,
        ..steward()
    };
    let again = allow(root, &refusals, &four, "devops", &auditing(&heard), 2);
    let Ok(Allowed::Already(grant)) = again else {
        panic!("already allowed: {again:?}");
    };
    assert_eq!(
        already_said(&grant),
        "Already allowed. Chats opened as steward can use vault devops in this project on \
         this machine. Nothing was changed."
    );
    assert_eq!(
        heard.lock().unwrap().len(),
        1,
        "one audit, for the one grant"
    );
    assert_eq!(sandbox::local::granted_vaults(root).len(), 1);
    assert_eq!(refusals.of(3), []);
    assert_eq!(refusals.of(4), []);
}

#[test]
fn every_chat_one_allow_answered_is_told_the_same_line_as_its_own_board_allows() {
    let refusals = Refusals::default();
    let sent = Mutex::new(Vec::new());
    // Chat 3 (pressed) waits, chat 4 waits, chat 5 is mid-turn, chat 6 is asking.
    let glance = |chat: u32| match chat {
        5 => (State::Running, false),
        6 => (State::Waiting, true),
        _ => (State::Waiting, false),
    };
    let told = tell_every(
        &refusals,
        3,
        &[4, 5, 6],
        &glance,
        "the line",
        &|chat, text| {
            sent.lock().unwrap().push((chat, text.to_owned()));
            Ok(())
        },
    );
    assert_eq!(told, Told::Sent);
    assert_eq!(
        *sent.lock().unwrap(),
        [(3, "the line".to_owned()), (4, "the line".to_owned())]
    );
    assert_eq!(refusals.queued_for(5), 1, "told when its turn ends");
    assert_eq!(refusals.queued_for(6), 0, "never onto a prompt");
}

#[test]
fn a_press_where_the_grant_already_stands_puts_the_notice_away_and_changes_nothing() {
    let project = project();
    let root = project.path();
    let refusals = Refusals::default();
    let (_, tell) = told();
    refused(
        &plane(root),
        &refusals,
        &asker(root, Some("steward")),
        false,
        "devops",
        &tell,
    );
    // Granted since, from somewhere else (Settings, another project window's press).
    sandbox::local::grant_vault(root, "devops", "steward").unwrap();
    let heard = Audits::default();
    let said = allow(root, &refusals, &steward(), "devops", &auditing(&heard), 1);
    assert!(matches!(said, Ok(Allowed::Already(_))), "{said:?}");
    assert_eq!(
        refusals.of(3),
        [],
        "the Notice is answered, not left to loop"
    );
    assert_eq!(*heard.lock().unwrap(), []);
}

#[test]
fn allow_follows_a_refusal_an_audit_and_the_apps_own_record_of_the_chat() {
    let project = project();
    let root = project.path();
    let refusals = Refusals::default();
    let heard = Audits::default();
    // No refusal was raised for this chat and vault: nothing to press, nothing allowed.
    let unasked =
        allow(root, &refusals, &steward(), "devops", &auditing(&heard), 1).expect_err("refused");
    assert!(unasked.contains("was not refused that vault"), "{unasked}");

    let (_, tell) = told();
    refused(
        &plane(root),
        &refusals,
        &asker(root, Some("steward")),
        false,
        "devops",
        &tell,
    );
    // Not audited, not allowed.
    let unaudited = allow(
        root,
        &refusals,
        &steward(),
        "devops",
        &|_, _| Err("the event log is not open".to_owned()),
        1,
    )
    .expect_err("refused");
    assert!(unaudited.contains("event log"), "{unaudited}");
    // A chat on no persona is allowed nothing, whatever the window sends.
    let nobody = Chat {
        session: 3,
        persona: None,
        name: None,
    };
    assert!(allow(root, &refusals, &nobody, "devops", &auditing(&heard), 1).is_err());
    assert_eq!(sandbox::local::granted_vaults(root), []);
    assert_eq!(*heard.lock().unwrap(), []);
}

#[test]
fn policy_forbids_the_allow_and_the_notice_says_who_locked_it() {
    use sandbox::policy::{Locks, set_for_this_test};
    let project = project();
    let root = project.path();
    let refusals = Refusals::default();
    let (heard, tell) = told();
    set_for_this_test(Locks::parse(
        r#"{"owner": "IT", "sandbox": {"vault-grants": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    ));
    refused(
        &plane(root),
        &refusals,
        &asker(root, Some("steward")),
        false,
        "devops",
        &tell,
    );
    let audits = Audits::default();
    let locked = allow(root, &refusals, &steward(), "devops", &auditing(&audits), 1);
    set_for_this_test(Locks::none());
    let said = "Policy forbids allowing a persona a vault it is not tagged for. Locked by \
                policy, set by IT in /etc/purlis/policy.json.";
    assert_eq!(heard.lock().unwrap()[0].locked.as_deref(), Some(said));
    assert_eq!(locked.expect_err("locked"), said);
    assert_eq!(*audits.lock().unwrap(), []);
    assert_eq!(sandbox::local::granted_vaults(root), []);
}

#[test]
fn a_chat_holds_a_handful_of_refusals_and_keep_blocked_puts_one_away() {
    let refusals = Refusals::default();
    let not = |vault: &str| NotTagged {
        vault: vault.to_owned(),
        persona: "steward".to_owned(),
        tagged_for: None,
    };
    for n in 0..AT_MOST_PER_CHAT + 2 {
        assert!(refusals.note(3, not(&format!("v{n}"))));
    }
    assert!(refusals.note(4, not("v0")), "another chat's is its own");
    let mine: Vec<String> = refusals.of(3).into_iter().map(|one| one.vault).collect();
    assert_eq!(mine, ["v2", "v3", "v4", "v5", "v6"]);
    refusals.put_away(3, "v4");
    assert!(!refusals.holds(3, "v4"));
    assert!(refusals.holds(4, "v0"));
    assert!(
        refusals.note(3, not("v4")),
        "its next refusal is news again"
    );
}

#[test]
fn a_closed_chat_takes_its_refusals_and_its_queued_lines_with_it() {
    let refusals = Refusals::default();
    let not = |vault: &str| NotTagged {
        vault: vault.to_owned(),
        persona: "steward".to_owned(),
        tagged_for: None,
    };
    refusals.note(3, not("a"));
    refusals.note(4, not("a"));
    tell(
        &refusals,
        3,
        || (State::Running, false),
        "retry".to_owned(),
        |_, _| Ok(()),
    );
    tell(
        &refusals,
        4,
        || (State::Running, false),
        "retry".to_owned(),
        |_, _| Ok(()),
    );
    refusals.forget(3);
    assert_eq!(refusals.of(3), []);
    assert_eq!(refusals.queued_for(3), 0);
    assert!(refusals.holds(4, "a"), "another chat's stay");
    assert_eq!(refusals.queued_for(4), 1);
    assert!(
        refusals.note(3, not("a")),
        "a later chat dealt the number starts clean"
    );
}

// ---- telling the chat, gated by the board (D-1430-11) ----

/// What `tell` did for a chat in `state`, and every write it made.
fn told_in(state: State, asking: bool) -> (Told, Vec<String>, usize) {
    let refusals = Refusals::default();
    let mut sent = Vec::new();
    let told = tell(
        &refusals,
        3,
        || (state, asking),
        "retry".to_owned(),
        |_, text| {
            sent.push(text.to_owned());
            Ok(())
        },
    );
    (told, sent, refusals.queued_for(3))
}

#[test]
fn a_waiting_chat_is_told_now() {
    assert_eq!(when(State::Waiting, false), Sending::Now);
    assert_eq!(
        told_in(State::Waiting, false),
        (Told::Sent, vec!["retry".to_owned()], 0)
    );
}

#[test]
fn a_chat_mid_turn_is_told_when_its_turn_ends_and_never_before() {
    assert_eq!(when(State::Running, false), Sending::AtTurnsEnd);
    assert_eq!(told_in(State::Running, false), (Told::Queued, vec![], 1));

    let refusals = Refusals::default();
    for text in ["one", "two"] {
        let told = tell(
            &refusals,
            3,
            || (State::Running, false),
            text.to_owned(),
            |_, _| panic!("nothing is written into a turn"),
        );
        assert_eq!(told, Told::Queued);
    }
    let sent = std::cell::RefCell::new(Vec::new());
    let send = |chat: u32, text: &str| {
        sent.borrow_mut().push((chat, text.to_owned()));
        Ok(())
    };
    // Still running: a tool's report mid-turn sends nothing.
    reported_sending(&refusals, 3, || (State::Running, false), send);
    // Its turn ended on a question for the person: still nothing.
    reported_sending(&refusals, 3, || (State::Waiting, true), send);
    // Another chat's turn ending is not this one's.
    reported_sending(&refusals, 4, || (State::Waiting, false), send);
    assert_eq!(*sent.borrow(), []);
    assert_eq!(refusals.queued_for(3), 2);
    // Waiting, and asking nothing: one write for both lines, and then nothing is left.
    reported_sending(&refusals, 3, || (State::Waiting, false), send);
    reported_sending(&refusals, 3, || (State::Waiting, false), send);
    assert_eq!(*sent.borrow(), [(3, "one\ntwo".to_owned())]);
    assert_eq!(refusals.queued_for(3), 0);
}

#[test]
fn a_turn_that_ends_while_allow_is_queueing_still_sends_the_line_on_that_turns_end() {
    // The interleaving: Allow reads the board as running, and the turn ends before the
    // line is queued. The board is read under the queue's lock, which the turn's end
    // takes too, so the end waits, finds the line queued, and sends it. Read outside the
    // lock, the end would find nothing queued and the line would wait for a turn that
    // may never come.
    let refusals = Arc::new(Refusals::default());
    let sent = Arc::new(Mutex::new(Vec::new()));
    let turns_end = Mutex::new(None);
    let told = tell(
        &refusals,
        3,
        || {
            // Allow has read "running". Now the turn ends, on the hooks' own thread.
            let (refusals, heard) = (Arc::clone(&refusals), Arc::clone(&sent));
            let (began, has_begun) = std::sync::mpsc::channel();
            *turns_end.lock().unwrap() = Some(std::thread::spawn(move || {
                began.send(()).unwrap();
                reported_sending(
                    &refusals,
                    3,
                    || (State::Waiting, false),
                    |chat, text| {
                        heard.lock().unwrap().push((chat, text.to_owned()));
                        Ok(())
                    },
                );
            }));
            has_begun.recv().unwrap();
            // Time for the turn's end to reach the queue, were nothing holding it back.
            std::thread::sleep(std::time::Duration::from_millis(100));
            assert_eq!(*sent.lock().unwrap(), [], "the end waits for the line");
            (State::Running, false)
        },
        "retry".to_owned(),
        |_, _| panic!("nothing is written into a turn"),
    );
    assert_eq!(told, Told::Queued);
    turns_end
        .lock()
        .unwrap()
        .take()
        .expect("the turn ended")
        .join()
        .expect("the turn's end ran");
    assert_eq!(*sent.lock().unwrap(), [(3, "retry".to_owned())]);
    assert_eq!(refusals.queued_for(3), 0);
}

#[test]
fn a_chat_that_is_asking_is_sent_nothing_so_no_enter_lands_on_its_prompt() {
    // A permission prompt or a question is open: an Enter would confirm its highlighted
    // choice. Whatever the state beside it, nothing is written and nothing is queued.
    for state in [
        State::Waiting,
        State::Running,
        State::Unknown,
        State::Done,
        State::Failed,
    ] {
        assert_eq!(when(state, true), Sending::Never, "{state:?}");
        assert_eq!(
            told_in(state, true),
            (Told::NotSent, vec![], 0),
            "{state:?}"
        );
    }
}

#[test]
fn a_chat_whose_state_is_unknown_or_that_has_ended_is_sent_nothing() {
    for state in [State::Unknown, State::Done, State::Failed] {
        assert_eq!(when(state, false), Sending::Never, "{state:?}");
        assert_eq!(
            told_in(state, false),
            (Told::NotSent, vec![], 0),
            "{state:?}"
        );
    }
    // A chat that ends with a line queued has it dropped, never sent to what comes next.
    let refusals = Refusals::default();
    tell(
        &refusals,
        3,
        || (State::Running, false),
        "retry".to_owned(),
        |_, _| Ok(()),
    );
    reported_sending(
        &refusals,
        3,
        || (State::Done, false),
        |_, _| panic!("an ended chat is sent nothing"),
    );
    assert_eq!(refusals.queued_for(3), 0);
}

#[test]
fn a_write_the_chat_did_not_take_is_said_as_not_sent() {
    let refusals = Refusals::default();
    let told = tell(
        &refusals,
        3,
        || (State::Waiting, false),
        "retry".to_owned(),
        |_, _| Err("the pty took nothing".to_owned()),
    );
    assert_eq!(told, Told::NotSent);
    assert_eq!(refusals.queued_for(3), 0);
}

#[test]
fn the_notice_says_what_became_of_the_line_and_the_line_is_one_paste_and_one_enter() {
    let grant = sandbox::local::VaultGrant {
        vault: "devops".to_owned(),
        persona: "steward".to_owned(),
    };
    let head = "Allowed. Chats opened as steward can use vault devops in this project on \
                this machine.";
    assert_eq!(
        allowed_said(&grant, Told::Sent),
        format!("{head} The chat is told what now runs. It does not restart.")
    );
    assert_eq!(
        allowed_said(&grant, Told::Queued),
        format!("{head} The chat is told what now runs when its turn ends. It does not restart.")
    );
    assert_eq!(
        allowed_said(&grant, Told::NotSent),
        format!("{head} The chat is told when it next starts a turn. It does not restart.")
    );
    let sent = sent_as(&told_allowed(&grant, false));
    assert!(
        sent.starts_with("\x1b[200~⬢ purlis: using vault `devops` — Allowed"),
        "{sent:?}"
    );
    assert!(
        sent.ends_with("chat.\x1b[201~\r") || sent.ends_with("system.\x1b[201~\r"),
        "{sent:?}"
    );
    assert_eq!(sent.matches('\r').count(), 1, "one Enter: {sent:?}");
}
