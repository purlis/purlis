// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A runner chat's declared tool, sent its values from this desktop one run at a time (#2047):
//! the window's gate, the person's answer for each run, and what reaches the runner.

use std::sync::Mutex;

use purlis_core::sandbox::tools::Handed as As;

use super::*;

fn builds(image: &str) -> Bound {
    Bound {
        pool: "builds".into(),
        definition: "d".repeat(64),
        image: image.repeat(64),
        runner: "01JB0RUNNER0000000000000000".into(),
        runner_key: "ab".repeat(32),
        runner_image: "a".repeat(64),
        engine: "this machine's Docker".into(),
    }
}

fn kubectl() -> Tool {
    Tool::one("kubectl", "team", "KUBECONFIG", As::File, "kubeconfig")
}

/// Chat 6's run `r1` of kubectl, with its own vault and hand-over, as its runner handed it on.
fn kubectl_asked() -> brokered::Ask {
    brokered::Ask {
        chat: 6,
        secret_exec: brokered::Wanted {
            vault: "team".into(),
            file: vec!["KUBECONFIG=kubeconfig".into()],
            command: vec!["/opt/kube/kubectl".into(), "get".into(), "pods".into()],
            ..Default::default()
        },
        runs_as: Some("kubectl".into()),
        waited: false,
        run: Some("r1".into()),
    }
}

/// kubectl declared for `devops` and allowed on this machine.
fn kubectl_declared_and_allowed(_root: &Path, _persona: &str) -> (Vec<Tool>, Vec<Tool>) {
    (vec![kubectl()], vec![kubectl()])
}

fn every_vault_allowed(_persona: &str, _vault: &str) -> Result<(), String> {
    Ok(())
}

fn owned_builds(_config: &Path, _pool: &str) -> Result<Bound, String> {
    Ok(builds("a"))
}

/// The gate for `ask` with every check passing: chat 6 open in its sandbox as `devops`, kubectl
/// declared and allowed, the vault allowed, and pool `builds` this machine's Docker.
fn a_gate(ask: &brokered::Ask) -> Gate<'_> {
    Gate {
        pool: "builds",
        ask,
        chat: Some((Some("devops".into()), false)),
        root: Path::new("/project"),
        pools_in: Some(PathBuf::from("/config")),
        tools: &kubectl_declared_and_allowed,
        authorised: &every_vault_allowed,
        owned: &owned_builds,
    }
}

fn refused(gate: &Gate<'_>) -> String {
    checked(gate).expect_err("refused")
}

#[test]
fn a_run_that_passes_every_check_is_the_personas_tool_on_the_pool_as_it_is_now() {
    let ask = kubectl_asked();
    assert_eq!(
        checked(&a_gate(&ask)),
        Ok(Checked {
            persona: "devops".into(),
            tool: kubectl(),
            bound: builds("a"),
            run: "r1".into(),
        })
    );
}

#[test]
fn a_chat_this_window_does_not_have_open_is_sent_nothing() {
    let ask = kubectl_asked();
    let gate = Gate {
        chat: None,
        ..a_gate(&ask)
    };
    assert!(refused(&gate).contains("not one this app has open"));
}

#[test]
fn a_runner_chat_started_without_the_sandbox_is_sent_nothing() {
    let ask = kubectl_asked();
    let gate = Gate {
        chat: Some((Some("devops".into()), true)),
        ..a_gate(&ask)
    };
    assert!(refused(&gate).contains("without the sandbox"));
}

#[test]
fn a_chat_on_no_persona_is_sent_nothing() {
    let ask = kubectl_asked();
    for persona in [None, Some(String::new())] {
        let gate = Gate {
            chat: Some((persona, false)),
            ..a_gate(&ask)
        };
        assert!(refused(&gate).contains("no persona"));
    }
}

#[test]
fn a_runner_that_does_not_name_the_tool_and_its_run_is_sent_nothing() {
    for (runs_as, run) in [(None, Some("r1")), (Some("kubectl"), None)] {
        let ask = brokered::Ask {
            runs_as: runs_as.map(str::to_owned),
            run: run.map(str::to_owned),
            ..kubectl_asked()
        };
        assert!(refused(&a_gate(&ask)).contains("update purlis on the runner"));
    }
}

#[test]
fn a_tool_not_declared_or_not_allowed_here_is_sent_nothing() {
    let ask = kubectl_asked();
    let undeclared = |_: &Path, _: &str| (Vec::new(), Vec::new());
    let gate = Gate {
        tools: &undeclared,
        ..a_gate(&ask)
    };
    assert!(refused(&gate).contains("not one"), "{}", refused(&gate));
    let not_allowed = |_: &Path, _: &str| (vec![kubectl()], Vec::new());
    let gate = Gate {
        tools: &not_allowed,
        ..a_gate(&ask)
    };
    assert!(refused(&gate).contains("not armed"), "{}", refused(&gate));
}

#[test]
fn a_run_asking_for_another_vault_or_hand_over_is_sent_nothing() {
    let mut other_vault = kubectl_asked();
    other_vault.secret_exec.vault = "prod".into();
    let mut other_key = kubectl_asked();
    other_key.secret_exec.file = vec!["KUBECONFIG=admin".into()];
    let mut as_env = kubectl_asked();
    as_env.secret_exec.env = std::mem::take(&mut as_env.secret_exec.file);
    for ask in [other_vault, other_key, as_env] {
        assert!(
            refused(&a_gate(&ask)).contains("runs it with exactly that"),
            "{}",
            refused(&a_gate(&ask))
        );
    }
}

#[test]
fn a_vault_the_persona_may_not_use_here_is_sent_nothing() {
    let ask = kubectl_asked();
    let not_tagged = |_: &str, vault: &str| Err(format!("vault {vault} is not this persona's"));
    let gate = Gate {
        authorised: &not_tagged,
        ..a_gate(&ask)
    };
    assert_eq!(refused(&gate), "vault team is not this persona's");
}

#[test]
fn a_runner_not_shown_to_be_the_persons_own_is_sent_nothing() {
    let ask = kubectl_asked();
    let remote =
        |_: &Path, _: &str| Err("this machine's DOCKER_HOST is tcp://build-box:2376".into());
    let gate = Gate {
        owned: &remote,
        ..a_gate(&ask)
    };
    assert!(refused(&gate).contains("tcp://build-box:2376"));
    let gate = Gate {
        pools_in: None,
        ..a_gate(&ask)
    };
    assert!(refused(&gate).contains("keeps no pools"));
}

fn nothing_moved() {}

/// A run waiting with `tools`, its asker `gone`, read from `vault`, the person waited for
/// `within`; what was read is noted in `reads`.
fn waiting<'a>(
    tools: &'a PoolTools,
    gone: &'a AtomicBool,
    read: Reads<'a>,
    within: Duration,
) -> Waiting<'a> {
    Waiting {
        tools,
        gone,
        moved: &nothing_moved,
        read,
        within,
        now: 1_700_000_000,
    }
}

/// Decides the run of `gate` on a thread, waiting on `tools`: what came of it, once it does.
fn deciding(
    tools: Arc<PoolTools>,
    gone: Arc<AtomicBool>,
    reads: Arc<Mutex<u32>>,
    owned: Arc<Mutex<Bound>>,
    within: Duration,
) -> mpsc::Receiver<Result<Approved, String>> {
    let (done, decided) = mpsc::channel();
    std::thread::spawn(move || {
        let ask = kubectl_asked();
        let now = move |_: &Path, _: &str| Ok(owned.lock().unwrap().clone());
        let gate = Gate {
            owned: &now,
            ..a_gate(&ask)
        };
        let read = move |tool: &Tool| {
            *reads.lock().unwrap() += 1;
            Ok(vec![(
                tool.values[0].key.clone(),
                Zeroizing::new("v-kubeconfig".to_owned()),
            )])
        };
        let checked = checked(&gate).expect("passes the gate");
        let mut said = Vec::new();
        let came = answered(
            &gate,
            checked,
            &waiting(&tools, &gone, &read, within),
            &mut said,
        );
        let _ = done.send(came);
    });
    decided
}

/// Waits until `tools` holds an Ask, and answers its run.
fn the_run_asked(tools: &PoolTools) -> PoolToolWaits {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(one) = tools.waiting().into_iter().next() {
            return one;
        }
        assert!(Instant::now() < deadline, "the person was never asked");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A run deciding on a thread: the Asks, its asker's going, its vault reads, the pool as it is
/// read now, and what came of it.
type Deciding = (
    Arc<PoolTools>,
    Arc<AtomicBool>,
    Arc<Mutex<u32>>,
    Arc<Mutex<Bound>>,
    mpsc::Receiver<Result<Approved, String>>,
);

fn a_run_deciding(within: Duration) -> Deciding {
    let tools = Arc::new(PoolTools::default());
    let gone = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(Mutex::new(0));
    let owned = Arc::new(Mutex::new(builds("a")));
    let decided = deciding(
        Arc::clone(&tools),
        Arc::clone(&gone),
        Arc::clone(&reads),
        Arc::clone(&owned),
        within,
    );
    (tools, gone, reads, owned, decided)
}

#[test]
fn a_run_waits_on_the_person_and_once_allowed_is_sent_its_values_for_that_run() {
    let (tools, _gone, reads, _owned, decided) = a_run_deciding(Duration::from_secs(60));

    let asked = the_run_asked(&tools);
    assert_eq!(asked.tool, "kubectl");
    assert_eq!(asked.bound, builds("a"));
    assert!(decided.try_recv().is_err(), "it waits on the answer");
    assert_eq!(*reads.lock().unwrap(), 0, "no vault read before the answer");
    assert!(tools.answer(6, asked.run, true).is_some());

    let handed = decided
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .expect("sent")
        .handed;
    assert!(
        handed.is_for(6, "kubectl", "r1"),
        "bound to the run: {handed:?}"
    );
    assert_eq!(handed.keys().collect::<Vec<_>>(), ["kubeconfig"]);
    assert_eq!(*reads.lock().unwrap(), 1);
    assert!(tools.waiting().is_empty(), "the Ask is gone");
}

#[test]
fn every_run_asks_however_the_one_before_was_answered() {
    let (tools, _gone, _reads, _owned, first) = a_run_deciding(Duration::from_secs(60));
    let asked = the_run_asked(&tools);
    tools.answer(6, asked.run, true);
    assert!(first.recv_timeout(Duration::from_secs(10)).unwrap().is_ok());

    let reads = Arc::new(Mutex::new(0));
    let second = deciding(
        Arc::clone(&tools),
        Arc::new(AtomicBool::new(false)),
        Arc::clone(&reads),
        Arc::new(Mutex::new(builds("a"))),
        Duration::from_secs(60),
    );

    let again = the_run_asked(&tools);
    assert_ne!(again.run, asked.run, "an Ask of its own");
    assert!(second.try_recv().is_err(), "it waits on the person again");
    tools.answer(6, again.run, false);
    let refused = second.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(
        refused
            .expect_err("kept off")
            .contains("kept this run of kubectl off")
    );
    assert_eq!(
        *reads.lock().unwrap(),
        0,
        "no vault read for a run kept off"
    );
}

#[test]
fn a_run_whose_asker_went_takes_its_ask_with_it_and_reads_no_vault() {
    let (tools, gone, reads, _owned, decided) = a_run_deciding(Duration::from_secs(60));
    let _ = the_run_asked(&tools);

    gone.store(true, Ordering::SeqCst);

    assert!(
        decided
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .is_err()
    );
    assert!(tools.waiting().is_empty(), "its Ask went");
    assert_eq!(*reads.lock().unwrap(), 0);
}

#[test]
fn a_run_the_person_did_not_answer_in_time_is_refused_and_its_ask_goes() {
    let (tools, _gone, reads, _owned, decided) = a_run_deciding(Duration::from_millis(300));

    let refused = decided.recv_timeout(Duration::from_secs(10)).unwrap();

    assert!(refused.expect_err("unanswered").contains("did not answer"));
    assert!(tools.waiting().is_empty());
    assert_eq!(*reads.lock().unwrap(), 0);
}

#[test]
fn a_run_whose_chat_closes_is_told_so() {
    let (tools, _gone, reads, _owned, decided) = a_run_deciding(Duration::from_secs(60));
    let _ = the_run_asked(&tools);

    tools.forget(6);

    let refused = decided.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(refused.expect_err("closed").contains("was closed"));
    assert_eq!(*reads.lock().unwrap(), 0);
}

#[test]
fn a_pool_that_changed_while_the_person_was_asked_is_sent_nothing() {
    let (tools, _gone, reads, owned, decided) = a_run_deciding(Duration::from_secs(60));
    let asked = the_run_asked(&tools);

    *owned.lock().unwrap() = builds("b");
    tools.answer(6, asked.run, true);

    let refused = decided.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(
        refused
            .expect_err("changed")
            .contains("changed while the person was asked")
    );
    assert_eq!(*reads.lock().unwrap(), 0);
}

#[test]
fn a_run_answered_already_or_of_another_chat_is_not_answered_again() {
    let tools = PoolTools::default();
    let (_answered, run) = tools.ask(PoolToolWaits {
        session: 6,
        run: 0,
        persona: "devops".into(),
        bound: builds("a"),
        tool: "kubectl".into(),
        vault: "team".into(),
        since: 1,
        waited: false,
    });

    assert!(tools.answer(7, run, true).is_none(), "another chat's");
    assert!(tools.answer(6, run, true).is_some());
    assert!(tools.answer(6, run, true).is_none(), "once");
}

/// What a run the person allowed sends: chat 6's run `r1` of kubectl on pool builds.
fn allowed() -> Approved {
    let ask = kubectl_asked();
    let checked = checked(&a_gate(&ask)).expect("passes the gate");
    Approved {
        handed: Handed::of(
            6,
            "kubectl",
            "r1",
            [("kubeconfig".to_owned(), "v-kubeconfig".to_owned())],
        ),
        checked,
    }
}

/// Every line of chat 6's trace in `project`.
fn traced(project: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(purlis_core::trace::file(project, "6"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("a trace line"))
        .collect()
}

#[test]
fn values_sent_to_a_runner_are_recorded_on_this_desktop_as_forwarded() {
    let desktop = tempfile::tempdir().unwrap();
    let mut went = None;

    let sent = sent(desktop.path(), allowed(), |handed| {
        went = Some(handed);
        Ok(())
    });

    assert!(sent.is_ok());
    assert!(went.is_some_and(|handed| handed.is_for(6, "kubectl", "r1")));
    let lines = traced(desktop.path());
    assert_eq!(lines.len(), 1, "{lines:?}");
    let forwarded = &lines[0];
    assert_eq!(forwarded["event"], "secret.forwarded");
    assert_eq!(forwarded["vault"], "team");
    assert_eq!(forwarded["key_names"], serde_json::json!(["kubeconfig"]));
    assert_eq!(forwarded["runner"], "01JB0RUNNER0000000000000000");
    assert_eq!(forwarded["pool"], "builds");
    assert_eq!(forwarded["chat"], 6);
    assert_eq!(forwarded["persona"], "devops");
    assert_eq!(forwarded["tool"], "kubectl");
    assert!(
        !forwarded.to_string().contains("v-kubeconfig"),
        "{forwarded}"
    );
}

#[test]
fn values_that_could_not_be_sent_are_not_recorded_as_sent() {
    let desktop = tempfile::tempdir().unwrap();

    let sent = sent(desktop.path(), allowed(), |_| Err(()));

    assert!(sent.is_err());
    assert_eq!(traced(desktop.path()), Vec::<serde_json::Value>::new());
}

#[test]
fn values_too_large_for_the_link_are_never_handed_on() {
    let ask = kubectl_asked();
    let huge = "v".repeat(purlis_session_protocol::link::MOST_CONTROL_FRAME_BYTES);
    let handed = Handed::of(6, "kubectl", "r1", [("kubeconfig".to_owned(), huge)]);
    let approved = Approved {
        handed,
        checked: checked(&a_gate(&ask)).expect("passes the gate"),
    };
    let desktop = tempfile::tempdir().unwrap();

    assert!(sent(desktop.path(), approved, hand_on_the_link).is_err());
    assert_eq!(traced(desktop.path()), Vec::<serde_json::Value>::new());
}
