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
        pod: String::new(),
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

/// Vaults that let every persona use every vault, with no key a link out of reach.
struct EveryVaultNothingLinked;

impl Vaults for EveryVaultNothingLinked {
    fn authorised(&self, _persona: &str, _vault: &str) -> Result<(), String> {
        Ok(())
    }

    fn out_of_reach(&self, _persona: &str, _tool: &Tool) -> Option<String> {
        None
    }
}

/// Vaults no persona may use.
struct NoVaultTagged;

impl Vaults for NoVaultTagged {
    fn authorised(&self, _persona: &str, vault: &str) -> Result<(), String> {
        Err(format!("vault {vault} is not this persona's"))
    }

    fn out_of_reach(&self, _persona: &str, _tool: &Tool) -> Option<String> {
        None
    }
}

/// Vaults every persona may use, where every tool's first key is a link out of reach.
struct FirstKeyOutOfReach;

impl Vaults for FirstKeyOutOfReach {
    fn authorised(&self, _persona: &str, _vault: &str) -> Result<(), String> {
        Ok(())
    }

    fn out_of_reach(&self, persona: &str, tool: &Tool) -> Option<String> {
        Some(format!(
            "{} in vault {} reads an entry of a vault {persona} may not use",
            tool.values[0].key, tool.vault
        ))
    }
}

fn owned_builds(_config: &Path, _pool: &str) -> Result<Bound, NotSent> {
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
        vaults: &EveryVaultNothingLinked,
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
fn a_cluster_pool_not_marked_is_refused_saying_how_and_nobody_is_asked() {
    let ask = kubectl_asked();
    let not_marked = |_: &Path, pool: &str| {
        Err(NotSent::NotYours(purlis_core::poollogins::not_marked_said(
            pool,
        )))
    };
    let gate = Gate {
        owned: &not_marked,
        ..a_gate(&ask)
    };
    let refused = refused(&gate);
    assert!(refused.contains("kubectl was sent no value"), "{refused}");
    assert!(refused.contains("This is my own"), "{refused}");
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

/// The tool's `args` and the forge's built-in refusals (#3421) are judged here too, before
/// anything is read or minted, and not only by the runner's host.
#[test]
fn a_run_with_arguments_the_tool_does_not_allow_is_sent_nothing() {
    let only_get = |_: &Path, _: &str| {
        let mut tool = kubectl();
        tool.args = Some(vec!["get".into()]);
        (vec![tool.clone()], vec![tool])
    };
    let ask = kubectl_asked();
    let gate = Gate {
        tools: &only_get,
        ..a_gate(&ask)
    };
    assert!(refused(&gate).contains("pods"), "{}", refused(&gate));
    let gh = || Tool::one("gh", "team", "GH_TOKEN", As::Env, "token");
    let gh_tools = |_: &Path, _: &str| (vec![gh()], vec![gh()]);
    let mut auth = kubectl_asked();
    auth.runs_as = Some("gh".into());
    auth.secret_exec.file = Vec::new();
    auth.secret_exec.env = vec!["GH_TOKEN=token".into()];
    auth.secret_exec.command = vec!["/opt/gh/gh".into(), "auth".into(), "token".into()];
    let gate = Gate {
        tools: &gh_tools,
        ..a_gate(&auth)
    };
    assert!(refused(&gate).contains("auth"), "{}", refused(&gate));
}

#[test]
fn a_vault_the_persona_may_not_use_here_is_sent_nothing() {
    let ask = kubectl_asked();
    let gate = Gate {
        vaults: &NoVaultTagged,
        ..a_gate(&ask)
    };
    assert_eq!(refused(&gate), "vault team is not this persona's");
}

#[test]
fn a_runner_not_shown_to_be_the_persons_own_is_sent_nothing() {
    let ask = kubectl_asked();
    let remote = |_: &Path, _: &str| {
        Err(NotSent::NotYours(
            "this machine's DOCKER_HOST is tcp://build-box:2376".into(),
        ))
    };
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
        let read = move |_persona: &str, tool: &Tool| {
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
fn a_pool_that_changed_while_its_vault_was_read_is_sent_nothing() {
    let tools = Arc::new(PoolTools::default());
    let gone = Arc::new(AtomicBool::new(false));
    let owned = Arc::new(Mutex::new(builds("a")));
    let (done, decided) = mpsc::channel();
    {
        let (tools, gone, owned) = (Arc::clone(&tools), gone, owned);
        std::thread::spawn(move || {
            let ask = kubectl_asked();
            let shown = Arc::clone(&owned);
            let now = move |_: &Path, _: &str| Ok(shown.lock().unwrap().clone());
            let gate = Gate {
                owned: &now,
                ..a_gate(&ask)
            };
            let read = move |_persona: &str, tool: &Tool| {
                *owned.lock().unwrap() = builds("b");
                Ok(vec![(
                    tool.values[0].key.clone(),
                    Zeroizing::new("v-kubeconfig".to_owned()),
                )])
            };
            let checked = checked(&gate).expect("passes the gate");
            let mut said = Vec::new();
            let _ = done.send(answered(
                &gate,
                checked,
                &waiting(&tools, &gone, &read, Duration::from_secs(60)),
                &mut said,
            ));
        });
    }
    let asked = the_run_asked(&tools);
    tools.answer(6, asked.run, true);
    let refused = decided.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(
        refused
            .expect_err("changed")
            .contains("changed while the person was asked")
    );
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

/// #3408: values the person allowed for a command that has gone since are never handed on, and
/// not recorded as sent.
#[test]
fn values_for_a_command_that_went_after_the_answer_are_never_handed_on() {
    let desktop = tempfile::tempdir().unwrap();
    let gone = AtomicBool::new(true);

    let sent = sent(
        desktop.path(),
        allowed(),
        while_its_asker_waits(&gone, |_| panic!("handed on")),
    );

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

/// A mint that answers a token only for chat 6's `gh` on a runner.
fn minted_for_chat_6(
    _: &Ctx,
    chat: &purlis_core::forge::apptoken::For<'_>,
    tool: &Tool,
) -> Result<Zeroizing<String>, String> {
    if chat.chat == 6
        && chat.on_a_runner
        && tool.name == "gh"
        && chat.started.is_some_and(|started| started.len() == 1)
    {
        Ok(Zeroizing::new("minted-for-the-runner-7e1f".to_owned()))
    } else {
        Err("asked for another chat or tool".to_owned())
    }
}

fn refused_mint(
    _: &Ctx,
    _: &purlis_core::forge::apptoken::For<'_>,
    _: &Tool,
) -> Result<Zeroizing<String>, String> {
    Err("installed on none of this chat's repos".to_owned())
}

/// A GitHub App's tool on a runner (#2053): its key is sent the token minted for the chat, as
/// that key's one value, and the token is remembered as a value so this desktop masks it.
#[test]
fn a_github_app_tool_on_a_runner_is_sent_the_token_minted_for_it_as_its_key_s_value() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Ctx::new(dir.path(), Env::of(&[]));
    let started = [purlis_core::forge::apptoken::Started {
        clone: dir.path().join("workspaces/ws/api"),
        git: Some((1, 2)),
    }];
    let gh = Tool {
        mint: Some(purlis_core::sandbox::tools::mint::Mint {
            app: "Iv23liExample".into(),
        }),
        ..Tool::one("gh", "forge", "GH_TOKEN", As::Env, "app-key")
    };
    let chat = purlis_core::forge::apptoken::For {
        root: dir.path(),
        chat: 6,
        folder: None,
        here: None,
        writes: &[],
        started: Some(&started),
        on_a_runner: true,
        config_root: None,
        ends_seen: None,
    };
    let sent: Vec<(String, String)> = minted(&ctx, &gh, &chat, &minted_for_chat_6)
        .unwrap()
        .into_iter()
        .map(|(key, value)| (key, value.to_string()))
        .collect();
    assert_eq!(
        sent,
        [(
            "app-key".to_owned(),
            "minted-for-the-runner-7e1f".to_owned()
        )]
    );
    assert!(purlis_core::secrets::seen::holds(
        "it said minted-for-the-runner-7e1f"
    ));
    // A refused mint sends nothing.
    assert_eq!(
        minted(&ctx, &gh, &chat, &refused_mint).map(|sent| sent.len()),
        Err("installed on none of this chat's repos".to_owned())
    );
}

/// A runner chat whose session start this window did not see (#2053): no record of its
/// workspace clones then, so nothing is minted, and the mint is never asked.
#[test]
fn a_runner_chat_with_no_recorded_start_is_minted_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Ctx::new(dir.path(), Env::of(&[]));
    let gh = Tool {
        mint: Some(purlis_core::sandbox::tools::mint::Mint {
            app: "Iv23liExample".into(),
        }),
        ..Tool::one("gh", "forge", "GH_TOKEN", As::Env, "app-key")
    };
    let chat = purlis_core::forge::apptoken::For {
        root: dir.path(),
        chat: 6,
        folder: None,
        here: None,
        writes: &[],
        started: None,
        on_a_runner: true,
        config_root: None,
        ends_seen: None,
    };
    assert_eq!(
        minted(&ctx, &gh, &chat, &never_minted).map(|sent| sent.len()),
        Err(NO_START_RECORDED.to_owned())
    );
}

fn never_minted(
    _: &Ctx,
    _: &purlis_core::forge::apptoken::For<'_>,
    _: &Tool,
) -> Result<Zeroizing<String>, String> {
    panic!("a runner chat with no recorded start is never minted for")
}

// A runner chat's tool whose key this desktop's vault does not hold (#5077, ADR 0109): the
// Secret ask is raised here, the fill stored here, and the value reaches the runner only by the
// run's own Allow, recorded as forwarded.

use purlis_core::secrets::ask::{Answer as SecretAnswer, Asks as SecretAsks};

const FILLED: &str = "f1lled-kubeconfig";

/// A desktop project whose vault `team`, `devops`'s, is a plain file that holds no key.
fn a_desktop_missing_the_key() -> tempfile::TempDir {
    let desktop = tempfile::tempdir().unwrap();
    std::fs::write(
        desktop.path().join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}, "persona": "devops"},
        }})
        .to_string(),
    )
    .unwrap();
    std::fs::write(desktop.path().join("team.json"), "{}").unwrap();
    desktop
}

fn ctx_of(root: &Path) -> Ctx {
    Ctx::new(root, purlis_core::secrets::Env::of(&[]))
}

/// The keys `tool`'s vault holds in the project at `root`, by name.
fn keys_in(root: &Path) -> impl Fn(&Tool) -> Result<Vec<String>, String> + '_ {
    move |tool: &Tool| {
        let ctx = ctx_of(root);
        let vault = purlis_core::secrets::cmd::provider(&ctx, &tool.vault)
            .map_err(|e| e.message.to_string())?;
        purlis_core::secrets::cmd::keys(&ctx, &vault).map_err(|e| e.message.to_string())
    }
}

/// Waits until `asks` holds a Secret ask, and answers what it is.
fn the_secret_asked(asks: &SecretAsks) -> purlis_core::secrets::ask::Open {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(one) = asks.open().into_iter().next() {
            return one;
        }
        assert!(
            Instant::now() < deadline,
            "the person was never asked for the key"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// What chat 6's run of kubectl on pool builds comes to, on this desktop at `root`, with the
/// runner as `owned` reads it; and the value each vault read gave the run, kept in `read`.
fn a_runner_run(
    root: &Path,
    asks: &SecretAsks,
    tools: &PoolTools,
    owned: &(dyn Fn(&Path, &str) -> Result<Bound, NotSent> + Sync),
    read: &Mutex<Vec<String>>,
) -> Result<Approved, Refused> {
    let ask = kubectl_asked();
    let gate = Gate {
        root,
        owned,
        ..a_gate(&ask)
    };
    let ctx = ctx_of(root);
    let reads = |persona: &str, tool: &Tool| {
        let values = super::read(&ctx, persona, tool)?;
        read.lock()
            .unwrap()
            .extend(values.iter().map(|(_, value)| value.to_string()));
        Ok(values)
    };
    let keys = keys_in(root);
    let gone = AtomicBool::new(false);
    decided(
        &gate,
        &Missing {
            asks,
            keys: &keys,
            within: Duration::from_secs(60),
        },
        &waiting(tools, &gone, &reads, Duration::from_secs(60)),
        &mut Vec::new(),
    )
}

#[test]
fn a_runner_chats_missing_key_is_asked_on_the_desktop_and_reaches_it_only_by_the_runs_allow() {
    let desktop = a_desktop_missing_the_key();
    let root = desktop.path();
    let (asks, tools) = (SecretAsks::new(root), PoolTools::default());
    let read = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        let run = scope.spawn(|| a_runner_run(root, &asks, &tools, &owned_builds, &read));
        let asked = the_secret_asked(&asks);
        assert_eq!(asked.chats, vec![6]);
        assert_eq!(
            (
                asked.vault.as_str(),
                asked.key.as_str(),
                asked.tool.as_deref()
            ),
            ("team", "kubeconfig", Some("kubectl"))
        );
        assert!(
            tools.waiting().is_empty(),
            "the run's Allow comes after the fill"
        );
        asks.answer(
            &ctx_of(root),
            asked.id,
            SecretAnswer::Fill(FILLED.to_owned().into()),
            crate::asking::the_window(),
        )
        .unwrap();
        // Filled, the run asks its Allow as every run of a tool on a runner does, and nothing
        // is read for it until then.
        let allow = the_run_asked(&tools);
        assert!(read.lock().unwrap().is_empty(), "read before the Allow");
        tools.answer(allow.session, allow.run, true).unwrap();
        let approved = run.join().unwrap().expect("allowed");
        assert_eq!(*read.lock().unwrap(), vec![FILLED.to_owned()]);
        // Sent by the one route there is, and recorded as forwarded.
        let mut went = None;
        sent(root, approved, |handed| {
            went = Some(handed);
            Ok(())
        })
        .unwrap();
        assert!(went.is_some_and(|handed| handed.is_for(6, "kubectl", "r1")));
    });
    let forwarded: Vec<_> = traced(root)
        .into_iter()
        .filter(|line| line["event"] == "secret.forwarded")
        .collect();
    assert_eq!(forwarded.len(), 1, "{forwarded:?}");
    assert_eq!(forwarded[0]["key_names"], serde_json::json!(["kubeconfig"]));
    assert!(!forwarded[0].to_string().contains(FILLED));
}

#[test]
fn a_runner_chats_fill_kept_off_at_its_allow_is_stored_here_and_sent_nowhere() {
    let desktop = a_desktop_missing_the_key();
    let root = desktop.path();
    let (asks, tools) = (SecretAsks::new(root), PoolTools::default());
    let read = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        let run = scope.spawn(|| a_runner_run(root, &asks, &tools, &owned_builds, &read));
        let asked = the_secret_asked(&asks);
        asks.answer(
            &ctx_of(root),
            asked.id,
            SecretAnswer::Fill(FILLED.to_owned().into()),
            crate::asking::the_window(),
        )
        .unwrap();
        let allow = the_run_asked(&tools);
        tools.answer(allow.session, allow.run, false).unwrap();
        let refused = run.join().unwrap().expect_err("kept off");
        assert!(
            refused.why.contains("kept this run of kubectl off"),
            "{}",
            refused.why
        );
    });
    assert!(read.lock().unwrap().is_empty());
    assert!(
        traced(root)
            .iter()
            .all(|line| line["event"] != "secret.forwarded")
    );
    let ctx = ctx_of(root);
    let vault = purlis_core::secrets::cmd::provider(&ctx, "team").unwrap();
    assert_eq!(
        purlis_core::secrets::cmd::get_value(&ctx, &vault, "kubeconfig").unwrap(),
        FILLED
    );
}

#[test]
fn a_runner_chats_secret_ask_declined_ends_its_run_declined_and_asks_no_allow() {
    let desktop = a_desktop_missing_the_key();
    let root = desktop.path();
    let (asks, tools) = (SecretAsks::new(root), PoolTools::default());
    let read = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        let run = scope.spawn(|| a_runner_run(root, &asks, &tools, &owned_builds, &read));
        let asked = the_secret_asked(&asks);
        asks.answer(
            &ctx_of(root),
            asked.id,
            SecretAnswer::Decline,
            crate::asking::the_window(),
        )
        .unwrap();
        let refused = run.join().unwrap().expect_err("declined");
        assert_eq!(refused.why, "declined team/kubeconfig");
        assert_eq!(refused.code, 1);
    });
    assert!(tools.waiting().is_empty());
}

#[test]
fn a_runner_that_cannot_receive_is_told_declined_and_the_person_may_still_store_the_key_here() {
    let desktop = a_desktop_missing_the_key();
    let root = desktop.path();
    let (asks, tools) = (SecretAsks::new(root), PoolTools::default());
    let read = Mutex::new(Vec::new());
    let hosted = |_: &Path, _: &str| {
        Err::<Bound, _>(NotSent::NotYours(
            "its runner is not shown to be yours".into(),
        ))
    };

    let refused = a_runner_run(root, &asks, &tools, &hosted, &read).expect_err("refused");

    assert_eq!(
        refused.why,
        "declined team/kubeconfig: runner can't receive secrets"
    );
    assert_eq!(refused.code, 1);
    assert!(tools.waiting().is_empty(), "no Allow is asked");
    let asked = asks.open();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(asked[0].cannot_receive, vec![6]);
    let answered = asks
        .answer(
            &ctx_of(root),
            asked[0].id,
            SecretAnswer::Fill(FILLED.to_owned().into()),
            crate::asking::the_window(),
        )
        .unwrap();
    assert!(answered.nudge.is_empty(), "the runner chat gets nothing");
    assert!(read.lock().unwrap().is_empty());
    assert!(
        traced(root)
            .iter()
            .all(|line| line["event"] != "secret.forwarded")
    );
}

#[test]
fn a_runner_that_cannot_receive_a_key_this_desktop_holds_is_refused_as_before_and_nothing_is_asked()
{
    let desktop = a_desktop_missing_the_key();
    let root = desktop.path();
    std::fs::write(root.join("team.json"), r#"{"kubeconfig": "held"}"#).unwrap();
    let (asks, tools) = (SecretAsks::new(root), PoolTools::default());
    let hosted = |_: &Path, _: &str| {
        Err::<Bound, _>(NotSent::NotYours(
            "its runner is not shown to be yours".into(),
        ))
    };

    let refused =
        a_runner_run(root, &asks, &tools, &hosted, &Mutex::default()).expect_err("refused");

    assert!(
        refused.why.contains("was sent no value on pool builds"),
        "{}",
        refused.why
    );
    assert!(asks.open().is_empty());
}

#[test]
fn a_runner_chat_whose_key_this_desktop_holds_asks_its_allow_and_no_secret() {
    let desktop = a_desktop_missing_the_key();
    let root = desktop.path();
    std::fs::write(root.join("team.json"), r#"{"kubeconfig": "held"}"#).unwrap();
    let (asks, tools) = (SecretAsks::new(root), PoolTools::default());
    let read = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        let run = scope.spawn(|| a_runner_run(root, &asks, &tools, &owned_builds, &read));
        let allow = the_run_asked(&tools);
        assert!(asks.open().is_empty());
        tools.answer(allow.session, allow.run, true).unwrap();
        run.join().unwrap().expect("allowed");
    });
    assert_eq!(*read.lock().unwrap(), vec!["held".to_owned()]);
}

/// A runner whose cluster did not confirm it as the person's own just now.
fn unconfirmed(_: &Path, pool: &str) -> Result<Bound, NotSent> {
    Err(NotSent::Unconfirmed(format!(
        "purlis could not confirm pool {pool}'s runner as yours just now"
    )))
}

#[test]
fn a_runner_unreachable_just_now_waits_on_the_ask_is_declined_nothing_and_is_nudged_on_its_fill() {
    let desktop = a_desktop_missing_the_key();
    let root = desktop.path();
    let (asks, tools) = (SecretAsks::new(root), PoolTools::default());
    let read = Mutex::new(Vec::new());

    let refused = a_runner_run(root, &asks, &tools, &unconfirmed, &read).expect_err("waits");

    assert_eq!(
        refused.why,
        "waiting on you for team/kubeconfig, and the runner was unreachable just now; retry \
         once answered"
    );
    assert_eq!(refused.code, brokered::WAITING_ON_THE_PERSON);
    assert!(tools.waiting().is_empty(), "no Allow is asked");
    let asked = asks.open();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(asked[0].chats, vec![6], "the chat waits on it");
    assert!(asked[0].cannot_receive.is_empty(), "declined nothing");
    let answered = asks
        .answer(
            &ctx_of(root),
            asked[0].id,
            SecretAnswer::Fill(FILLED.to_owned().into()),
            crate::asking::the_window(),
        )
        .unwrap();
    assert_eq!(answered.nudge, vec![6], "the runner chat hears its answer");
    assert!(read.lock().unwrap().is_empty(), "no value was read for it");
    assert!(
        traced(root)
            .iter()
            .all(|line| line["event"] != "secret.forwarded")
    );
}

#[test]
fn a_runner_not_shown_to_be_yours_is_declined_but_one_unreachable_just_now_is_not() {
    let desktop = a_desktop_missing_the_key();
    let root = desktop.path();
    let (asks, tools) = (SecretAsks::new(root), PoolTools::default());
    let not_yours = |_: &Path, _: &str| {
        Err::<Bound, _>(NotSent::NotYours(
            "its runner is not shown to be yours".into(),
        ))
    };

    let declined =
        a_runner_run(root, &asks, &tools, &not_yours, &Mutex::default()).expect_err("declined");
    let waits =
        a_runner_run(root, &asks, &tools, &unconfirmed, &Mutex::default()).expect_err("waits");

    assert!(
        declined.why.ends_with("runner can't receive secrets"),
        "{}",
        declined.why
    );
    assert!(!waits.why.contains("declined"), "{}", waits.why);
    assert_eq!(
        (declined.code, waits.code),
        (1, brokered::WAITING_ON_THE_PERSON)
    );
}

/// A runner chat's tool whose key is a link into a vault its persona may not use is read for no
/// run on this desktop (#5795): the link is judged as it is read, whenever it was made.
#[test]
fn a_runner_chats_key_linked_into_a_vault_its_persona_may_not_use_is_never_read() {
    let desktop = a_desktop_missing_the_key();
    let root = desktop.path();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}, "persona": "devops"},
            "other": {"provider": "plain-file", "config": {"file": "other.json"}},
        }})
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("other.json"),
        serde_json::json!({"kubeconfig": "n0t-yours"}).to_string(),
    )
    .unwrap();
    let ctx = ctx_of(root);
    let team = purlis_core::secrets::cmd::provider(&ctx, "team").unwrap();
    purlis_core::secrets::links::link(&ctx, &team, "kubeconfig", "other", "kubeconfig").unwrap();

    let why = super::read(&ctx, "devops", &kubectl())
        .map(|values| values.len())
        .unwrap_err();

    assert!(
        why.contains("kubeconfig in vault team reads an entry of a vault devops may not use"),
        "{why}"
    );
    assert!(!why.contains("other"), "names no other vault: {why}");
    assert!(!why.contains("n0t-yours"), "never the value: {why}");
}

/// A runner chat's key out of reach is refused before the person is asked anything (#5795): no
/// Secret ask for a key it lacks, and no Allow for a run that would be refused once allowed.
#[test]
fn a_runner_chats_key_out_of_reach_is_refused_before_anything_is_asked() {
    let desktop = a_desktop_missing_the_key();
    let root = desktop.path();
    let (asks, tools) = (SecretAsks::new(root), PoolTools::default());
    let ask = kubectl_asked();
    let gate = Gate {
        root,
        vaults: &FirstKeyOutOfReach,
        ..a_gate(&ask)
    };
    let keys = keys_in(root);
    let gone = AtomicBool::new(false);
    let reads = |_: &str, _: &Tool| -> Result<Vec<(String, Zeroizing<String>)>, String> {
        panic!("nothing is read")
    };

    let refused = decided(
        &gate,
        &Missing {
            asks: &asks,
            keys: &keys,
            within: Duration::from_secs(60),
        },
        &waiting(&tools, &gone, &reads, Duration::from_secs(60)),
        &mut Vec::new(),
    )
    .map(|_| ())
    .unwrap_err();

    assert!(refused.why.contains("may not use"), "{}", refused.why);
    assert!(asks.open().is_empty(), "no Secret ask");
    assert!(tools.waiting().is_empty(), "no Allow");
}

/// What a runner chat's gate judges up front is the core's judge on the tool's keys (#5795).
#[test]
fn a_runner_chats_gate_judges_the_tools_keys_by_the_cores_judge() {
    let desktop = a_desktop_missing_the_key();
    let root = desktop.path();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}, "persona": "devops"},
            "other": {"provider": "plain-file", "config": {"file": "other.json"}},
        }})
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("other.json"),
        serde_json::json!({"kubeconfig": "n0t-yours"}).to_string(),
    )
    .unwrap();
    let ctx = ctx_of(root);
    assert_eq!(super::out_of_reach(&ctx, "devops", &kubectl()), None);
    let team = purlis_core::secrets::cmd::provider(&ctx, "team").unwrap();
    purlis_core::secrets::links::link(&ctx, &team, "kubeconfig", "other", "kubeconfig").unwrap();

    let why = super::out_of_reach(&ctx, "devops", &kubectl()).expect("refused");

    assert!(why.contains("may not use"), "{why}");
}

/// A runner chat's gate holding this desktop's vaults, as `run_on_its_runner` builds it, refuses a
/// key out of reach before the person is asked anything (#5857): the core's judge of a link
/// through the gate's own [`Vaults`] for a project, not a fake.
#[test]
fn a_runner_chats_gate_as_wired_refuses_a_key_out_of_reach_before_anything_is_asked() {
    let desktop = a_desktop_missing_the_key();
    let root = desktop.path();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}, "persona": "devops"},
            "other": {"provider": "plain-file", "config": {"file": "other.json"}},
        }})
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("other.json"),
        serde_json::json!({"kubeconfig": "n0t-yours"}).to_string(),
    )
    .unwrap();
    let ctx = ctx_of(root);
    let team = purlis_core::secrets::cmd::provider(&ctx, "team").unwrap();
    purlis_core::secrets::links::link(&ctx, &team, "kubeconfig", "other", "kubeconfig").unwrap();
    let (asks, tools) = (SecretAsks::new(root), PoolTools::default());
    let ask = kubectl_asked();
    // The gate holds the project's vaults as a run's does: `vaults: &ctx`.
    let gate = Gate {
        root,
        vaults: &ctx,
        ..a_gate(&ask)
    };
    let keys = keys_in(root);
    let gone = AtomicBool::new(false);
    let reads = |_: &str, _: &Tool| -> Result<Vec<(String, Zeroizing<String>)>, String> {
        panic!("nothing is read")
    };

    let refused = decided(
        &gate,
        &Missing {
            asks: &asks,
            keys: &keys,
            within: Duration::from_secs(60),
        },
        &waiting(&tools, &gone, &reads, Duration::from_secs(60)),
        &mut Vec::new(),
    )
    .map(|_| ())
    .unwrap_err();

    assert!(
        refused
            .why
            .contains("kubeconfig in vault team reads an entry of a vault devops may not use"),
        "{}",
        refused.why
    );
    assert!(!refused.why.contains("other"), "names no other vault");
    assert!(asks.open().is_empty(), "no Secret ask");
    assert!(tools.waiting().is_empty(), "no Allow");
}

#[test]
fn the_asker_of_a_run_is_told_where_the_person_answers_and_for_how_long() {
    let said = asked_said(
        "kubectl",
        "builds",
        Path::new("/home/one/smart-ide"),
        Duration::from_secs(15 * 60),
    );
    assert!(
        said.contains(
            "waiting for the person: Asks (\u{270b}) in the purlis window, project smart-ide, for \
             up to 15 minutes"
        ),
        "{said}"
    );
}

const HOST_KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n";

/// This machine's pools under a folder of their own: pool `builds` on another machine, reached
/// over SSH and pinned, its one runner paired, and marked for its pin where `owned`.
fn a_pool_on_another_machine(owned: bool) -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    let config = tempfile::tempdir().unwrap();
    let path = purlis_core::machine::dir(config.path()).join(purlis_core::machine::RUNNERS_FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let image = format!("ghcr.io/purlis/purlis-runner@sha256:{}", "a".repeat(64));
    let pin = serde_json::json!({ "hostKey": HOST_KEY, "engine": "ABCD" });
    let mut pool = serde_json::json!({
        "name": "builds", "kind": "remote-docker", "image": image,
        "route": "ssh://me@build-box/var/run/docker.sock", "pin": pin,
    });
    if owned {
        pool["owned"] = pin;
    }
    let text = serde_json::json!({
        "version": 1,
        "pools": [pool],
        "runners": [{
            "name": "builds", "pool": "docker", "container": "e".repeat(64), "image": image,
            "user": "10001:10001", "home": "/home/purlis",
            "device": "01JB0RUNNER0000000000000000", "key": "12".repeat(32), "paired": 1,
        }],
    });
    std::fs::write(&path, text.to_string()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    config
}

/// **A tool's value for one run reaches a pool on another machine only once the person said only
/// they sign in there** (#6271, ADR 0123 §5), by the one check a login's goes by: refused, saying
/// how, until then, and once marked bound to the machine as pinned, whatever this machine's own
/// Docker is.
#[test]
fn a_pool_on_another_machine_is_sent_a_tools_value_only_once_marked_and_bound_to_its_pin() {
    let ask = kubectl_asked();
    let gate_on = |config: &Path| {
        let config = config.to_path_buf();
        move |_: &Path, pool: &str| {
            Bound::over_the_link_with_kinded(
                Err("purlis finds no Docker engine on this machine".to_owned()),
                &config,
                pool,
                Some("builds/builds"),
            )
        }
    };
    let unmarked = a_pool_on_another_machine(false);
    let owned = gate_on(unmarked.path());
    let gate = Gate {
        owned: &owned,
        ..a_gate(&ask)
    };
    let said = refused(&gate);
    assert!(said.contains("Who can sign in to build-box?"), "{said}");

    let marked = a_pool_on_another_machine(true);
    let owned = gate_on(marked.path());
    let gate = Gate {
        owned: &owned,
        ..a_gate(&ask)
    };
    let passed = checked(&gate).expect("a marked pool passes");
    assert_eq!(passed.bound.runner, "01JB0RUNNER0000000000000000");
    assert!(
        passed.bound.engine.contains("build-box"),
        "{:?}",
        passed.bound
    );
    assert!(passed.bound.engine.contains("ABCD"), "{:?}", passed.bound);
}
