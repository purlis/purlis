// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The sandbox scenario suite** (M74.1): every scenario runs in the scenario host
//! ([`host`], T1.1-06), production's path with the shipped kit, scaled timings and the resolver
//! seam, in a child of this test binary with homes of its own.
//!
//! - [`host`]: the fixture: `Scenario::new(homes, protection, persona, dialect)`.
//! - [`switches`]: every test-only switch on a scenario's path, forced or listed.
//!
//! They bind sockets and apply purlis's OS wrap, so their red and green are CI's
//! (`sandbox-scenarios-linux`, and the `macos-scenarios`-labelled legs): a scenario that cannot
//! apply its wrap fails there and, anywhere else, says why it did not run.

#[cfg(unix)]
#[path = "scenarios/host.rs"]
pub(crate) mod host;
#[cfg(unix)]
#[path = "scenarios/report.rs"]
mod report;
#[path = "scenarios/switches.rs"]
mod switches;

#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use host::{Dialect, Homes, Protection, Scenario};

/// What a binary says it was built with (`purlis_core::fence::BUILT_WITH_ARG`).
#[cfg(unix)]
fn built_with(binary: &Path) -> String {
    let out = purlis_core::forklock::output(
        std::process::Command::new(binary).arg(purlis_core::fence::BUILT_WITH_ARG),
    )
    .unwrap_or_else(|why| panic!("{} does not run: {why}", binary.display()));
    assert!(out.status.success(), "{}: {out:?}", binary.display());
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// What a release build says it was built with.
#[cfg(unix)]
const AS_RELEASED: &str = "fenced=false test-support=false";

/// The files in `folder`, by name.
#[cfg(unix)]
fn files_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .map(|read| {
            read.flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// **The scenario kit is what the app ships** (T1.1-06): the `purlis` CI built apart from the
/// test build, with neither the fence nor the test seams, never the tests' stand-in; the repo's
/// plugin, not an empty folder; git hooks and shims that binary wrote; the skill sets' and chat
/// settings' folders; and the same kit on the window's side and as the wrap. The host process
/// `in_a_host_process_at` runs is a release build's too.
#[cfg(unix)]
#[test]
fn the_scenario_kit_is_what_the_app_ships() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let kit = &scenario.kit;

    let head = std::fs::read(&kit.binary).expect("the binary reads");
    assert!(
        !head.starts_with(b"#!"),
        "{} is a script, the tests' stand-in, not a built purlis",
        kit.binary.display()
    );
    assert_ne!(Some(&kit.binary), crate::Shipped::default().binary.as_ref());
    assert_eq!(
        built_with(&kit.binary),
        AS_RELEASED,
        "the purlis a chat runs"
    );
    match std::env::var_os(host::HOST_APP_VAR) {
        Some(app) => assert_eq!(
            built_with(Path::new(&app)),
            AS_RELEASED,
            "the host process `in_a_host_process_at` runs"
        ),
        None => assert!(
            !(cfg!(target_os = "linux") && host::on_ci()),
            "{} names no host process on Linux CI",
            host::HOST_APP_VAR
        ),
    }

    assert_eq!(kit.plugin, host::repo().join("app/src-tauri/plugin"));
    assert!(
        kit.plugin
            .join(".claude-plugin")
            .join("plugin.json")
            .is_file(),
        "the repo's plugin, with its manifest: {:?}",
        files_in(&kit.plugin)
    );
    for (what, folder) in [
        ("git hooks", kit.git_hooks.dir().to_path_buf()),
        ("shims", kit.shims.bin()),
    ] {
        let written = files_in(&folder);
        assert!(
            !written.is_empty(),
            "no {what} were written in {}",
            folder.display()
        );
        let names_the_binary = written.iter().any(|name| {
            std::fs::read_to_string(folder.join(name))
                .is_ok_and(|text| text.contains(&kit.binary.display().to_string()))
        });
        assert!(
            names_the_binary,
            "the {what} run another purlis: {written:?}"
        );
    }
    let for_the_host = kit.for_the_host();
    assert!(for_the_host.skill_sets.is_some(), "skill_sets is set");
    assert!(for_the_host.chat_settings.is_some(), "chat_settings is set");
    assert!(for_the_host.git_hooks.is_some(), "the git hooks are armed");

    let shipped = scenario.planes().shipped().clone();
    assert_eq!(
        shipped.binary.as_ref(),
        Some(&kit.binary),
        "the window's binary"
    );
    assert_eq!(
        shipped.plugin.as_ref(),
        Some(&kit.plugin),
        "the window's plugin"
    );
    assert!(shipped.shims.is_some(), "the window's shims");
    assert!(shipped.git_hooks.is_some(), "the window's git hooks");
    assert_eq!(
        crate::sandboxing::scenario_wrap(),
        Some(kit.binary.as_path()),
        "the wrap"
    );
}

/// **The scenario host runs chats in the session host** (T1.1-06): chats run in the host by
/// default, the window's chats are the host's, the host's own `Confine` started the chat's
/// confinement, and the host, not the window, writes the event log in the scenario's data home.
#[cfg(unix)]
#[test]
fn the_scenario_host_runs_chats_in_the_session_host() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    assert!(
        crate::hostedsessions::chats_run_in_the_host(),
        "production's default: chats run in the session host"
    );
    let Some(scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let chat = scenario.starts(&serde_json::json!({
        "turns": [{"prompt": "go", "calls": [{"step": "here", "tool": "bash", "run": "echo here"}]}]
    }));
    scenario.until_played(chat, &["here"]);

    let started = scenario.confines.started();
    assert!(
        !started.is_empty(),
        "the host's own Confine started the chat's confinement"
    );
    assert!(
        started
            .iter()
            .all(|one| one.planned != purlis_host::Planned::Elsewhere),
        "{started:?}"
    );
    let events = purlis_core::eventlog::read(&homes.data).expect("the host's event log reads");
    assert!(
        !events.is_empty(),
        "the host wrote the event log in the scenario's data home"
    );
}

/// **One `Timings` value reaches the board, the throttle and the hold's end, in host mode**
/// (T1.1-06; T1.1-01's in-process test is `planes::tests::…_in_process`): the scaled value the
/// scenario was given is what the host's board holds an ask for, what the window's hooks throttle
/// blocks by and end a hold at, and what its chats hand their hearing.
#[cfg(unix)]
#[test]
fn one_timings_value_reaches_the_board_the_throttle_and_hold_ends_in_host_mode() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let timings = scenario.timings;
    assert_ne!(
        timings,
        purlis_core::sandbox::timings::Timings::production()
    );
    let chat = scenario.starts(&serde_json::json!({
        "turns": [{"prompt": "go", "calls": [{"step": "here", "tool": "bash", "run": "echo here"}]}]
    }));
    scenario.until_played(chat, &["here"]);

    let started = scenario.confines.started();
    assert!(!started.is_empty(), "a confinement was started in the host");
    for one in &started {
        assert_eq!(
            one.holds_asks_for,
            Some(timings.hold()),
            "the host's board: {one:?}"
        );
    }
    let held = scenario.held();
    assert_eq!(held.hooks().timings(), timings, "the window's hooks");
    assert_eq!(
        held.hooks().throttle_window(),
        timings.throttle_window(),
        "the block throttle"
    );
    assert_eq!(
        crate::asking::hold_ends(None, 1_000, &held.hooks().timings()),
        1_000 + timings.hold().as_secs(),
        "the window's end of a hold"
    );
    assert_eq!(held.chats().timings(), timings, "the window's chats");
}

/// **A Linux Claude Code chat in the scenario host is sandboxed** (T1.1-06): the production
/// branch of `Chats::start_as_opted`, never the test build's that can leave one unsandboxed, at
/// the floor and at Full. Its proxy is the host's, and the vault no chat reads is refused to it,
/// which only a wrap does. Elsewhere Claude Code holds itself (ticket 10), so nothing is asserted.
#[cfg(unix)]
#[test]
fn a_linux_claude_chat_in_the_scenario_host_is_sandboxed() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    if !cfg!(target_os = "linux") {
        return;
    }
    for protection in [Protection::Basic, Protection::Full] {
        let Some(scenario) = Scenario::new(&homes, protection, None, Dialect::Claude) else {
            return;
        };
        let vaults = scenario.root.join(".purlis").join("vaults");
        std::fs::create_dir_all(&vaults).expect("the vaults");
        std::fs::write(
            vaults.join("covering.json"),
            "{\"covering\": \"VAULT-HELD\"}\n",
        )
        .expect("a vault");
        let reads = format!("cat {}", vaults.join("covering.json").display());
        let chat = scenario.starts(&serde_json::json!({
            "turns": [{"prompt": "go", "calls": [
                {"step": "reads-the-vault", "tool": "bash", "run": reads}
            ]}]
        }));
        scenario.until_played(chat, &["reads-the-vault"]);
        assert!(
            !scenario.confines.started().is_empty(),
            "{protection:?}: the chat runs confined, its proxy started by the host"
        );
        let read = scenario.result_of(chat, "reads-the-vault").expect("played");
        assert!(
            !read.to_string().contains("VAULT-HELD"),
            "{protection:?}: the Claude Code chat read the vault, so it ran unsandboxed: {read}\n{}",
            scenario.said()
        );
    }
}

/// **A scenario leaves no process and no temp folder behind** (T1.1-06): once its child ends,
/// nothing it started runs, a process its chat left in the background included, and every
/// folder of its own is gone (the parent's check, `Homes::of_this_scenario`).
#[cfg(unix)]
#[test]
fn a_scenario_leaves_no_process_and_no_temp_dir_behind() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let chat = scenario.starts(&serde_json::json!({
        "turns": [{"prompt": "go", "calls": [
            {"step": "sleeps", "tool": "bash",
             "run": "sleep 600 >/dev/null 2>&1 & echo $! > .scenario/sleeper.pid; echo started"}
        ]}]
    }));
    scenario.until_played(chat, &["sleeps"]);
    let sleeper: u32 = std::fs::read_to_string(scenario.root.join(".scenario/sleeper.pid"))
        .unwrap_or_else(|why| {
            panic!(
                "the chat said nothing of what it left running ({why})\n{}",
                scenario.said()
            )
        })
        .trim()
        .parse()
        .expect("a process id");
    assert!(
        purlis_core::process::alive(sleeper),
        "it runs while the scenario does"
    );
    scenario.also_started(sleeper);
    assert!(!scenario.pids().is_empty(), "the chat runs");
    drop(scenario);
}

/// **A held connection in a scenario raises an ask and blocks at the hold** (T1.1-06): the first
/// end-to-end smoke, in every dialect at Full. A chat asks for a host nothing lists; its proxy in
/// the session host holds the connection and the window is asked (`ask.raised`); nobody answers,
/// so at the scaled hold the call is refused, never carried: the host is never looked up,
/// dialled or reached. It shows today's behaviour: the block itself is written at the next
/// prompt (#6673), and `ask.raised` may be written twice (#6674), so neither is pinned here.
#[cfg(unix)]
#[test]
fn a_held_connection_in_a_scenario_raises_an_ask_and_blocks_at_the_hold() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    for dialect in Dialect::ALL {
        let Some(scenario) = Scenario::new(&homes, Protection::Full, Some("steward"), dialect)
        else {
            return;
        };
        let held = format!("held-{}.example.test", dialect.word());
        let began = std::time::Instant::now();
        let chat = scenario.starts(&serde_json::json!({
            "turns": [{"prompt": "go", "calls": [{
                "step": "held", "tool": "bash",
                "run": reaches(&held)
            }]}]
        }));
        until(&scenario, "the window is asked", || {
            !scenario
                .events_naming(purlis_core::eventlog::stops::ASK_RAISED, &held)
                .is_empty()
        });
        scenario.until_played(chat, &["held"]);
        let took = began.elapsed();
        assert!(
            took >= scenario.timings.hold(),
            "{dialect:?}: the call ended after {took:?}, before the hold of {:?} ended",
            scenario.timings.hold()
        );
        let result = scenario.result_of(chat, "held").expect("played");
        assert!(
            !result.to_string().contains("status 200"),
            "{dialect:?}: the held connection was carried: {result}"
        );
        assert!(
            scenario.names.lookups().iter().all(|one| one.name != held),
            "{dialect:?}: a host nobody allowed was looked up: {:?}",
            scenario.names.lookups()
        );
        assert!(scenario.names.origin_hits().is_empty());
    }
}

/// The command a chat runs to reach `host` on HTTPS, saying the status it was answered.
#[cfg(unix)]
fn reaches(host: &str) -> String {
    format!("curl -sS -o /dev/null -w 'status %{{http_code}}' --max-time 300 https://{host}/")
}

/// The block chat `session` is held on now for `target`, as its Notice shows it.
#[cfg(unix)]
fn held_on(
    scenario: &Scenario,
    session: u32,
    target: &str,
) -> Option<crate::taskblocks::BlockShown> {
    scenario
        .held()
        .chats()
        .blocks()
        .every()
        .into_iter()
        .find(|(held, block)| *held == session && block.target.contains(target))
        .map(|(_, block)| crate::taskblocks::BlockShown {
            operation: block.operation,
            kind: block.kind,
            what: block.what,
            target: block.target,
        })
}

/// **Two chats in one host keep their grants apart** (T1.1-06, §3.7): the person allows a held
/// host for one chat alone, through the window's own command (`allow_sandbox_block`); that chat's
/// held connection is carried (its proxy looks the host up), and the other chat, asking for the
/// same host in the same host process, is held and refused at the hold, never carried.
#[cfg(unix)]
#[test]
fn two_chats_in_one_host_keep_their_grants_apart() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, Some("steward"), Dialect::Codex)
    else {
        return;
    };
    let host = "granted-to-one.example.test";
    let script = serde_json::json!({
        "turns": [{"prompt": "go", "calls": [{"step": "reaches", "tool": "bash", "run": reaches(host)}]}]
    });
    let lookups = || {
        scenario
            .names
            .lookups()
            .iter()
            .filter(|one| one.name == host)
            .count()
    };

    let one = scenario.starts(&script);
    until(&scenario, "the first chat is held on the host", || {
        held_on(&scenario, one, host).is_some()
    });
    let shown = held_on(&scenario, one, host).expect("held");
    tauri::async_runtime::block_on(crate::sandboxing::allow_sandbox_block(
        tauri::Manager::state(scenario.app()),
        scenario.plane.clone(),
        one,
        shown,
        crate::sandboxing::GrantLevel::Chat,
    ))
    .expect("the person allows it for this chat");
    scenario.until_played(one, &["reaches"]);
    until(&scenario, "the first chat's proxy carries it", || {
        lookups() > 0
    });
    let carried = lookups();

    let other = scenario.starts(&script);
    let began = std::time::Instant::now();
    until(&scenario, "the other chat is held on the host", || {
        held_on(&scenario, other, host).is_some()
    });
    scenario.until_played(other, &["reaches"]);
    assert!(
        began.elapsed() >= scenario.timings.hold(),
        "the other chat's call ended before the hold did: {:?}\n{}",
        scenario.result_of(other, "reaches"),
        scenario.said()
    );
    assert_eq!(
        lookups(),
        carried,
        "the other chat's proxy looked the host up: the first chat's grant reached it"
    );
}

/// A chat's two turns: the first played before the restart, the second typed after it.
#[cfg(unix)]
fn two_turns(first: &str, second: &str) -> serde_json::Value {
    serde_json::json!({
        "turns": [
            {"prompt": "first", "calls": [{"step": first, "tool": "bash", "run": "echo one"}]},
            {"prompt": "second", "calls": [{"step": second, "tool": "bash", "run": "echo two"}]}
        ]
    })
}

/// **A crashed window reattaches to the surviving host and chat** (T1.1-06, `restart: window`):
/// the window's links are cut as its crash cuts them and the window goes; the chat lives on in
/// the host, its process the same; a new window on the same homes takes it back, and what the
/// person types next reaches the same chat. What else survives is asserted by the cells
/// (ticket 43's expectations).
#[cfg(unix)]
#[test]
fn a_crashed_window_reattaches_to_the_surviving_host_and_chat() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(mut scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let chat = scenario.starts(&two_turns("before-the-crash", "after-the-crash"));
    scenario.until_played(chat, &["before-the-crash"]);
    let before = scenario.open_chats();
    let pid = before
        .iter()
        .find(|(id, _)| *id == chat)
        .and_then(|(_, pid)| *pid)
        .expect("the chat runs");

    scenario.the_window_crashes_and_comes_back();
    assert!(
        purlis_core::process::alive(pid),
        "the chat outlived the window"
    );
    until(&scenario, "the new window has the chat back", || {
        scenario
            .open_chats()
            .iter()
            .any(|(_, back)| *back == Some(pid))
    });
    let (back, _) = scenario
        .open_chats()
        .into_iter()
        .find(|(_, back)| *back == Some(pid))
        .expect("the same chat");
    scenario.types(back, "second");
    scenario.until_played(back, &["after-the-crash"]);
    assert_eq!(
        scenario.open_chats().len(),
        before.len(),
        "the chat was taken back, not started again beside itself"
    );
}

/// **A quit ends every chat and a relaunch restarts the live ones** (T1.1-06, `restart: quit`,
/// §3.17, M-BH-11): the window tells the host to quit, which ends every chat and stops; a new
/// host and window start on the same homes and relaunch the chat that was live, on its
/// conversation, so it plays on from its next turn.
#[cfg(unix)]
#[test]
fn a_quit_ends_every_chat_and_a_relaunch_restarts_the_live_ones() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(mut scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let chat = scenario.starts(&two_turns("before-the-quit", "after-the-relaunch"));
    scenario.until_played(chat, &["before-the-quit"]);
    let before = scenario.pids();
    assert!(!before.is_empty(), "the chat runs");

    scenario.quits_and_relaunches();
    for pid in &before {
        assert!(
            !purlis_core::process::alive(*pid),
            "the quit ended every chat: {pid} still runs"
        );
    }
    until(&scenario, "the relaunch restarts the live chat", || {
        !scenario.pids().is_empty()
    });
    let (again, _) = scenario
        .open_chats()
        .into_iter()
        .next()
        .expect("the chat again");
    scenario.types(again, "second");
    scenario.until_played(again, &["after-the-relaunch"]);
    let replayed = scenario
        .transcript()
        .iter()
        .filter(|line| line["kind"] == "tool_result" && line["step"] == "before-the-quit")
        .count();
    assert_eq!(
        replayed, 1,
        "it played on from its next turn, not from the start"
    );
}

/// **The host kill is observed** (T1.1-06, `restart: host_killed`): on Linux, in a session host
/// of its own process, the `purlis-app` CI built apart from the test build (`PURLIS_HOST_APP`),
/// at production timings, the host is killed outright, and the window is told its chat ended
/// with its host. What survives and what is reported beyond that are ticket 48's cells.
#[cfg(target_os = "linux")]
#[test]
fn the_host_kill_is_observed() {
    let Some(app) = std::env::var_os(host::HOST_APP_VAR) else {
        return host::cannot_run::<()>(&format!(
            "{} names no purlis-app built apart from the test build",
            host::HOST_APP_VAR
        ))
        .unwrap_or(());
    };
    assert_eq!(built_with(Path::new(&app)), AS_RELEASED, "the host process");
    let home = tempfile::tempdir().expect("the host's home");
    let target = crate::behaviour::in_a_host_process_running(home.path(), app.into());
    let ends = crate::behaviour::told_of_ends(&target);
    let chat = target
        .sessions
        .open(
            None,
            &crate::behaviour::a_shell("printf alive.; read line"),
            &|_| {},
        )
        .expect("the chat starts in the host process");
    crate::behaviour::until("the chat runs", || {
        target.sessions.process_id(chat).is_some()
    });

    (target.kill_the_host.as_ref().expect("a host process"))();

    crate::behaviour::until("the window is told the chat ended with its host", || {
        ends.lock()
            .expect("the ends")
            .iter()
            .any(|(id, _)| *id == chat)
    });
    assert!(
        target.sessions.lost_with_its_host(chat),
        "the chat's end is the host's, as the window reports it"
    );
}

/// **The maximal chat holds every grant kind** (T1.1-06): one chat holds a grant of every
/// `grant::What` variant (host, write, socket, service, port, listen), as the person allowed
/// them for it, and runs confined with an ssh route beside its proxy; a `purlis secret exec`
/// tunnel to a host it may reach is dialled through the scenario's seam. A variant added to
/// `grant::What` fails to compile in `host::kind_of`, and once it is named there, fails here
/// until the maximal chat holds one. The chat's open browser is not held yet (see the PR).
#[cfg(unix)]
#[test]
fn the_maximal_chat_holds_every_grant_kind() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, Some("steward"), Dialect::Codex)
    else {
        return;
    };
    let every = host::one_of_every_kind(&scenario.root);
    let kinds: std::collections::BTreeSet<usize> = every.iter().map(host::kind_of).collect();
    assert_eq!(
        kinds,
        (0..host::GRANT_KINDS).collect(),
        "the maximal chat holds no grant of a `grant::What` variant"
    );
    let mut grants = purlis_core::sandbox::grant::Grants::default();
    for what in &every {
        grants.add(what);
    }
    let chat = scenario.starts_holding(
        &serde_json::json!({
            "turns": [{"prompt": "go", "calls": [{"step": "maximal", "tool": "bash", "run": "echo held"}]}]
        }),
        grants,
    );
    scenario.until_played(chat, &["maximal"]);
    let started = scenario.confines.started();
    assert!(
        started.iter().any(|one| one.ssh.is_some()),
        "an ssh route beside its proxy: {started:?}"
    );

    let origin = purlis_scenarios::net::Origin::http().expect("an origin");
    let name = "maximal-tunnel.test";
    scenario.names.origin(name, 5432, origin.addr());
    let _held = scenario.names.carry_tunnels();
    let tunnel = purlis_core::sandbox::tunnel::Tunnel::open(
        &purlis_core::sandbox::tunnel::pointed(&format!("{name}:5432")).expect("a target"),
        &purlis_core::sandbox::reach::Decision::Allowed(purlis_core::sandbox::reach::By::Chat),
        purlis_core::sandbox::reach::Reach::open(vec![format!("{name}:5432")]),
        None,
    )
    .expect("the tunnel opens");
    let _ = std::net::TcpStream::connect(("127.0.0.1", tunnel.port())).map(|mut stream| {
        use std::io::{Read as _, Write as _};
        let _ = write!(
            stream,
            "GET / HTTP/1.1\r\nHost: {name}\r\nConnection: close\r\n\r\n"
        );
        let _ = stream.read_to_end(&mut Vec::new());
    });
    assert_eq!(scenario.names.origin_hits(), vec![(name.to_owned(), 5432)]);
}

/// Until `done`, within [`PLAYS_WITHIN`]: a chat's harness starts and plays first.
#[cfg(unix)]
fn until(scenario: &Scenario, what: &str, done: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + PLAYS_WITHIN;
    while !done() {
        assert!(
            std::time::Instant::now() < deadline,
            "{what} never happened\n{}",
            scenario.said()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// **The scenario host never uses `testgit`'s config** (T1.1-06): it sets
/// `safe.bareRepository=all` (`purlis_core::testgit`), which would mask ticket 21's
/// embedded-bare-repo cell. Neither the scenario host's sources nor the suite's name it.
#[test]
fn the_scenario_host_never_uses_testgits_config() {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for file in [
        "scenarios.rs",
        "scenarios/host.rs",
        "scenarios/report.rs",
        "scenarios/switches.rs",
    ] {
        let text = std::fs::read_to_string(here.join(file)).expect("a source");
        let named = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .any(|line| line.contains(concat!("test", "git::")));
        assert!(!named, "{file} uses testgit's config");
    }
}

/// How long a chat's script has to play.
#[cfg(unix)]
pub(crate) const PLAYS_WITHIN: Duration = Duration::from_secs(120);

/// The entries run at `Timings::production()` (ticket 54), picked by the `production-timings`
/// label and run in the merge queue.
mod production_timings {
    #[test]
    fn the_production_timing_entries_run_when_asked() {}
}
