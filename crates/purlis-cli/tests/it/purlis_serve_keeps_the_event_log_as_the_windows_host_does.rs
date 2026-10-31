// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis serve`, the runner image's session host, keeps the event log as the window's own host
//! does (#2494): a start it composes writes its run and its trust event, a chat's sub-agents'
//! runs end with its own, and a dispatch it worked that ended with no window there ends as one
//! that went without its report.
//!
//! It binds sockets and runs a program in a terminal, which a sandboxed agent cannot: it first
//! runs on CI. Linux only, as `purlis_serve_is_this_users_one_session_host` is.

#![cfg(target_os = "linux")]

use crate::support;

use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use purlis_core::dispatchrecord;
use purlis_core::eventlog::{Attributes, Began, Entry};
use purlis_session_protocol::auth::{Credential, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Answer, Client, Command, Launch, ProgramKind, StartProgram,
};

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(30);
const CHAT: &str = "01K6W0RKER000000000000000B";
const RUN: &str = "01K6W0RKRUN00000000000000B";

/// Ends the host this test started, by its pid, however the test ends.
struct Ends(Child);

impl Drop for Ends {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `purlis serve` with its config, data and runtime homes in `dir`, once it says it is ready.
fn serving(dir: &Path) -> Ends {
    for folder in ["run", "config", "data"] {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
    }
    std::fs::set_permissions(dir.join("run"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut serve = support::purlis();
    serve
        .arg("serve")
        .env("PURLIS_CONFIG_HOME", dir.join("config"))
        .env("PURLIS_DATA_HOME", dir.join("data"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("XDG_RUNTIME_DIR", dir.join("run"))
        .env("TMPDIR", dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut host = Ends(serve.spawn().expect("purlis serve starts"));
    let mut line = String::new();
    BufReader::new(host.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    if line.trim_end() != "ready" {
        let _ = host.0.kill();
        let mut said = String::new();
        let _ = host.0.stderr.take().unwrap().read_to_string(&mut said);
        panic!("the host did not say it was ready: {line:?}\n{said}");
    }
    host
}

/// A chat that waits until `go` is there, then exits with code 3 by itself.
fn a_chat_that_ends_once(go: &Path) -> StartProgram {
    StartProgram {
        launch: Some(Launch {
            program: "/bin/sh".into(),
            args: vec![
                "-c".into(),
                "while [ ! -e \"$1\" ]; do sleep 0.1; done; exit 3".into(),
                "sh".into(),
                go.display().to_string(),
            ],
            cwd: None,
            set: Vec::new(),
            strip: Vec::new(),
            pass: Vec::new(),
            declared: Vec::new(),
            harness: None,
            git_hooks: None,
            reporting: None,
            confinement: None,
            prompt_key: None,
        }),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

/// A dispatch chat `number` works, owing its report to a chat that asked.
fn a_dispatch_worked_by(root: &Path, number: u32) -> String {
    dispatchrecord::open(
        root,
        dispatchrecord::Opening {
            mode: dispatchrecord::Mode::Task,
            asker: dispatchrecord::Asker {
                chat: dispatchrecord::ChatRef {
                    chat: 99,
                    id: Some("01K6ASKER0000000000000000A".into()),
                    name: "steward 99".into(),
                    persona: Some("steward".into()),
                },
                workspace: None,
                by_person: false,
                session_record: None,
            },
            persona: Some("devops".into()),
            worker: dispatchrecord::Worker {
                chat: dispatchrecord::ChatRef {
                    chat: number,
                    id: Some(CHAT.into()),
                    name: "look at the queue".into(),
                    persona: Some("devops".into()),
                },
                harness: None,
                profile: None,
                session_record: None,
            },
            task: Some("look at the queue".into()),
            place: dispatchrecord::Place {
                workspace: None,
                folder: None,
                worktree: None,
            },
            brief: "Look at the queue.".into(),
            report_owed: true,
        },
        chrono::Utc::now(),
    )
    .expect("a running dispatch")
    .id
}

/// Every event the host's log holds, in the one device folder it writes under `data`.
fn the_hosts_events(data: &Path) -> Vec<purlis_core::eventlog::Event> {
    let Ok(devices) = std::fs::read_dir(data.join(purlis_core::eventlog::EVENTS)) else {
        return Vec::new();
    };
    devices
        .flatten()
        .filter(|device| device.path().is_dir())
        .flat_map(|device| purlis_core::eventlog::read(&device.path()).unwrap_or_default())
        .collect()
}

/// A link to the host serving with its homes in `dir`, admitted as the window, with what keeps
/// it up.
async fn the_window_of(dir: &Path) -> (Client, impl Sized) {
    let socket = purlis_core::hostsocket::here_for(
        Some(dir.join("run").as_os_str()),
        Some(&dir.join("config")),
    );
    let credential = Credential::read(
        &purlis_core::machine::charterd(&dir.join("config")),
        Scope::LocalUi,
    )
    .expect("the window's credential");
    let stream = tokio::net::UnixStream::connect(&socket)
        .await
        .expect("the host's socket");
    let link = link::connect(stream, session::speaks(), Scope::LocalUi, &credential)
        .await
        .expect("admitted as the window");
    let (client, views, events) = Client::new(link);
    (client, (views, events))
}

fn a_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn until(what: &str, done: impl Fn() -> bool) {
    let by = Instant::now() + PATIENCE;
    while !done() {
        assert!(Instant::now() < by, "{what}, within {PATIENCE:?}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_chat_ending_with_no_window_there_ends_its_sub_agents_and_its_dispatch_without_a_report() {
    let dir = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("charter.toml"), "").unwrap();
    let root = project.path().to_path_buf();
    let go = dir.path().join("go");
    let _host = serving(dir.path());

    let dispatched = a_runtime().block_on(async {
        let (client, _kept) = the_window_of(dir.path()).await;
        client
            .hold_project(PROJECT, root.to_str().unwrap())
            .await
            .expect("held");
        let number = client
            .start_program_told(a_chat_that_ends_once(&go))
            .await
            .expect("it starts")
            .number
            .expect("its number");

        // The chat's run, and a sub-agent of it, as the window tells the host's log.
        for entry in [
            Entry::Begin {
                plane: root.clone(),
                number,
                chat: CHAT.into(),
                run: RUN.into(),
                cause: Began::Start,
                attributes: Attributes::default(),
                held: false,
                trust: None,
            },
            Entry::Tool {
                plane: root.clone(),
                call: purlis_core::hookwire::ToolCall {
                    chat: number,
                    tool_hook: "pretooluse".into(),
                    tool: Some("Bash".into()),
                    call: Some("toolu_01".into()),
                    args: None,
                    decision: purlis_core::hookwire::Decision::None,
                    rule: None,
                    hook_ms: 1,
                    agent: Some("agent-1".into()),
                    at_ms: 0,
                    skill: None,
                },
            },
        ] {
            let entry = serde_json::to_value(&entry).unwrap();
            match client.call(Command::Record { entry }).await {
                Ok(Answer::Done) => {}
                other => panic!("the host did not record it: {other:?}"),
            }
        }
        let dispatched = a_dispatch_worked_by(&root, number);

        // The window lets go of the project, and the chat ends by itself.
        client.let_go_of_project(PROJECT).await.expect("let go of");
        std::fs::write(&go, "").unwrap();
        dispatched
    });

    let data = dir.path().join("data");
    until("the sub-agent's run ends with its own", || {
        the_hosts_events(&data).iter().any(|event| {
            event.kind == "run.ended"
                && event.parent_run.as_deref() == Some(RUN)
                && event.body["state"] == "failed"
                && event.body["cause"] == "exited"
        })
    });
    until("its dispatch ends without its report", || {
        dispatchrecord::read(&root, &dispatched).is_some_and(|record| !record.running())
    });
    let ended = dispatchrecord::read(&root, &dispatched).expect("the record");
    assert_eq!(ended.ended_by, Some(dispatchrecord::EndedBy::Unreported));
    assert!(ended.undelivered.is_some(), "owed to the chat that asked");
}

/// A project with a persona, a workspace and a profile `work` this machine trusts, whose harness
/// is a stand-in Claude Code that ends at once.
fn a_project_with_a_profile(bin: &Path) -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    let root = project.path();
    std::fs::write(root.join("charter.toml"), "").unwrap();
    std::fs::create_dir_all(root.join("personas/steward")).unwrap();
    std::fs::write(root.join("personas/steward/persona.md"), "# steward\n").unwrap();
    std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
    let program = stand_in::claude_code(bin, "claude", "#!/bin/sh\n");
    std::fs::write(
        root.join(purlis_core::profiles::LOCAL_FILE),
        format!(
            "[harness.work]\nkind = \"claude\"\ncommand = [{:?}]\n",
            program.display().to_string()
        ),
    )
    .unwrap();
    let declared = purlis_core::profiles::current(root);
    purlis_core::profiletrust::record_launched(
        root,
        "work",
        &purlis_core::profiletrust::fingerprint(declared.get("work").unwrap()),
    )
    .unwrap();
    project
}

#[test]
fn a_start_the_host_composes_writes_its_run_and_its_trust_event() {
    let dir = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let project = a_project_with_a_profile(bin.path());
    let root = project.path().to_path_buf();
    let _host = serving(dir.path());

    a_runtime().block_on(async {
        let (client, _kept) = the_window_of(dir.path()).await;
        client
            .hold_project(PROJECT, root.to_str().unwrap())
            .await
            .expect("held");
        client
            .start_program_told(StartProgram {
                program: Some(CHAT.into()),
                run: Some(RUN.into()),
                persona: Some("steward".into()),
                profile: Some("work".into()),
                workspace: Some("alpha".into()),
                // Out of the sandbox, so its trust event is the person's opt-out.
                without_sandbox: true,
                ..StartProgram::of(PROJECT, ProgramKind::Harness)
            })
            .await
            .expect("it starts");
    });

    let data = dir.path().join("data");
    until("its run is started in the host's log", || {
        the_hosts_events(&data).iter().any(|event| {
            event.kind == "run.started"
                && event.chat.as_deref() == Some(CHAT)
                && event.run.as_deref() == Some(RUN)
        })
    });
    assert!(
        the_hosts_events(&data).iter().any(|event| {
            event.kind == "trust.sandbox.off" && event.run.as_deref() == Some(RUN)
        }),
        "its opt-out is written under its run"
    );
}
