// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill grade --run` against `purlis serve`, with a test standing in for the window
//! (SK-61, #3241): a no starts nothing, a yes starts the plan the window answered by its digest,
//! and a plan other than the one printed is never asked about. An outcome eval runs only where
//! named, on a yes typed at a real terminal (#4312).

#![cfg(unix)]

use crate::support;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::Duration;

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use purlis_core::skills::grades;
use purlis_core::skills::verbs::Level;
use purlis_session_protocol::auth::{Credential, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Client, Command, Events, GradeAnswer, GradeAsk, GradeEndTold, GradePlanTold,
    GradeStepTold, Pushed, Told,
};

const PATIENCE: Duration = Duration::from_secs(30);

struct Ends(Child);

impl Drop for Ends {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Place {
    _dir: tempfile::TempDir,
    at: PathBuf,
    root: PathBuf,
    _host: Ends,
}

fn homes(command: &mut std::process::Command, dir: &Path) {
    command
        .env("HOME", dir.join("home"))
        .env("PURLIS_CONFIG_HOME", dir.join("config"))
        .env("PURLIS_DATA_HOME", dir.join("data"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("XDG_RUNTIME_DIR", dir.join("run"))
        .env("TMPDIR", dir);
}

/// A project with a skill to grade, and `purlis serve` once it says it is ready.
fn a_place() -> Place {
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path().canonicalize().unwrap();
    for folder in ["run", "config", "data", "home"] {
        std::fs::create_dir_all(at.join(folder)).unwrap();
    }
    std::fs::set_permissions(at.join("run"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let root = at.join("plane");
    std::fs::create_dir_all(root.join("skills/deploy/evals")).unwrap();
    std::fs::write(root.join("charter.toml"), "[plane]\n").unwrap();
    std::fs::write(
        root.join("skills/deploy/SKILL.md"),
        "---\nname: deploy\ndescription: Ships the build.\n---\n\nShip it.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("skills/deploy/evals/triggers.json"),
        r#"[{"query": "ship it", "should_trigger": true}]"#,
    )
    .unwrap();
    std::fs::write(
        root.join("skills/deploy/evals/evals.json"),
        r#"{"skill_name": "deploy", "evals": [
            {"id": 1, "prompt": "ship v2", "expectations": ["It ships"]},
            {"id": 2, "prompt": "ship v3", "expectations": ["It ships"]}
        ]}"#,
    )
    .unwrap();
    let mut serve = support::purlis();
    serve
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    homes(&mut serve, &at);
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
    Place {
        _dir: dir,
        at,
        root,
        _host: host,
    }
}

/// The window's link, holding the project.
async fn a_window(place: &Place) -> (Client, Events) {
    let socket = purlis_core::hostsocket::here_for(
        Some(place.at.join("run").as_os_str()),
        Some(&place.at.join("config")),
    );
    let credential = Credential::read(
        &purlis_core::machine::charterd(&place.at.join("config")),
        Scope::LocalUi,
    )
    .unwrap();
    let stream = tokio::net::UnixStream::connect(&socket).await.unwrap();
    let link = link::connect(stream, session::speaks(), Scope::LocalUi, &credential)
        .await
        .unwrap();
    let (client, _views, events) = Client::new(link);
    client.subscribe(session::Since::new()).await.unwrap();
    let project = purlis_core::scaffold::planefile::id_to_hold_by(&place.root).unwrap();
    client
        .hold_project(&project, place.root.to_str().unwrap())
        .await
        .unwrap();
    (client, events)
}

/// `purlis skill grade deploy --run` with `args`, as a person's terminal would start it.
fn the_command(place: &Place, args: &[&str]) -> std::process::Command {
    let mut command = support::purlis();
    command
        .args(["skill", "grade", "deploy", "--run"])
        .args(args)
        .current_dir(&place.root)
        .env("CHARTER_ROOT", &place.root)
        .env("PURLIS_SKILL_STUDIO", "1")
        .env("NO_COLOR", "1")
        .env_remove(purlis_core::hookwire::CHAT_ENV)
        .env_remove(purlis_core::hookwire::SOCKET_ENV)
        .env_remove(purlis_core::hookwire::SANDBOXED_ENV)
        .env_remove(purlis_core::active::SESSION_ID_ENV)
        .env_remove("CHARTER_CHAT")
        .env_remove("CHARTER_HOOK_SOCKET")
        .env_remove("CHARTER_SANDBOXED")
        .env_remove("CHARTER_SESSION_ID");
    homes(&mut command, &place.at);
    command
}

/// [`the_command`], piped, answering its question with `stdin`.
fn the_terminal(place: &Place, args: &[&str], stdin: &str) -> tokio::task::JoinHandle<String> {
    let mut command = the_command(place, args);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let stdin = stdin.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
        let mut said = String::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut said)
            .unwrap();
        let mut err = String::new();
        child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut err)
            .unwrap();
        let code = child.wait().unwrap();
        format!("{said}{err}exit {}", code.code().unwrap_or(-1))
    })
}

/// [`the_command`], on a real terminal a person types `typed` at.
fn at_a_real_terminal(
    place: &Place,
    args: &[&str],
    typed: &str,
) -> tokio::task::JoinHandle<String> {
    let command = the_command(place, args);
    let mut starts = CommandBuilder::new(command.get_program());
    starts.args(command.get_args());
    starts.cwd(command.get_current_dir().unwrap());
    for (name, value) in command.get_envs() {
        match value {
            Some(value) => starts.env(name, value),
            None => starts.env_remove(name),
        }
    }
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 50,
            cols: 200,
            ..PtySize::default()
        })
        .unwrap();
    let mut child = pair.slave.spawn_command(starts).unwrap();
    drop(pair.slave);
    let master = pair.master;
    let mut reader = master.try_clone_reader().unwrap();
    let mut writer = master.take_writer().unwrap();
    // It waits on the terminal's line until the program asks.
    writer.write_all(typed.as_bytes()).unwrap();
    tokio::task::spawn_blocking(move || {
        let mut said = Vec::new();
        // A terminal whose program ended is an end of file on macOS and an error on Linux.
        let _ = reader.read_to_end(&mut said);
        let code = child.wait().unwrap();
        drop((writer, master));
        format!(
            "{}exit {}",
            String::from_utf8_lossy(&said),
            code.exit_code()
        )
    })
}

/// What the window is asked next: `None` where the terminal asked nothing more.
async fn next_ask(events: &mut Events, within: Duration) -> Option<(u64, GradeAsk)> {
    loop {
        match tokio::time::timeout(within, events.recv()).await {
            Ok(Some(Told::Pushed(Pushed::GradeAsked { question, ask, .. }))) => {
                return Some((question, ask));
            }
            Ok(Some(_)) => {}
            _ => return None,
        }
    }
}

fn plan_of(place: &Place, graded: Option<String>) -> GradePlanTold {
    let planned = grades::planned(&place.root, &Level::project(), "deploy", None).unwrap();
    GradePlanTold {
        profile: Some("work".into()),
        model: None,
        sandbox: Some("Full".into()),
        refused: None,
        chats: 6,
        token_cap: 200_000,
        limit_micro_usd: 15_000_000,
        running: None,
        graded: graded.or(planned.consent),
        consent: Some("the-windows-digest".into()),
    }
}

/// [`plan_of`] with the outcome evals `named` in it, as the window answers it.
fn plan_naming(place: &Place, named: &[&str]) -> GradePlanTold {
    let named: Vec<String> = named.iter().map(|id| (*id).to_owned()).collect();
    let planned =
        grades::planned_named(&place.root, &Level::project(), "deploy", None, &named).unwrap();
    GradePlanTold {
        chats: planned.plan.unwrap().chats,
        ..plan_of(place, planned.consent)
    }
}

async fn answers(window: &Client, question: u64, answer: GradeAnswer) {
    window
        .call(Command::AnswerGrade { question, answer })
        .await
        .unwrap();
}

#[tokio::test]
async fn a_no_starts_nothing() {
    let place = a_place();
    let (window, mut told) = a_window(&place).await;
    let terminal = the_terminal(&place, &[], "n\n");
    let (question, ask) = next_ask(&mut told, PATIENCE).await.expect("the plan's ask");
    assert!(matches!(ask, GradeAsk::Plan { .. }));
    answers(
        &window,
        question,
        GradeAnswer::Plan(Box::new(plan_of(&place, None))),
    )
    .await;
    let said = terminal.await.unwrap();
    assert!(said.contains("Nothing was run."), "{said}");
    assert!(said.contains("$15.00"), "{said}");
    assert!(
        next_ask(&mut told, Duration::from_millis(500))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn a_plan_other_than_the_one_printed_is_never_asked_about() {
    let place = a_place();
    let (window, mut told) = a_window(&place).await;
    let terminal = the_terminal(&place, &["--yes"], "");
    let (question, _) = next_ask(&mut told, PATIENCE).await.expect("the plan's ask");
    answers(
        &window,
        question,
        GradeAnswer::Plan(Box::new(plan_of(&place, Some("another read".into())))),
    )
    .await;
    let said = terminal.await.unwrap();
    assert!(said.contains("nothing was run"), "{said}");
    assert!(
        next_ask(&mut told, Duration::from_millis(500))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn a_yes_starts_the_plan_by_its_digest_and_follows_it_to_its_end() {
    let place = a_place();
    let (window, mut told) = a_window(&place).await;
    let terminal = the_terminal(&place, &[], "y\n");
    let (question, _) = next_ask(&mut told, PATIENCE).await.expect("the plan's ask");
    answers(
        &window,
        question,
        GradeAnswer::Plan(Box::new(plan_of(&place, None))),
    )
    .await;
    let (question, ask) = next_ask(&mut told, PATIENCE).await.expect("the start");
    let GradeAsk::Start { consent, .. } = ask else {
        panic!("not a start: {ask:?}");
    };
    assert_eq!(consent, "the-windows-digest");
    let step = |ran, ended: Option<GradeEndTold>| GradeStepTold {
        grade: "g".into(),
        ran,
        of: 6,
        tokens: None,
        cost_micro_usd: Some(100_000),
        asked: 0,
        ended,
        said: None,
    };
    answers(
        &window,
        question,
        GradeAnswer::Started { grade: "g".into() },
    )
    .await;
    window
        .call(Command::GradeStep {
            question,
            step: step(3, None),
        })
        .await
        .unwrap();
    window
        .call(Command::GradeStep {
            question,
            step: step(6, Some(GradeEndTold::Whole)),
        })
        .await
        .unwrap();
    let said = terminal.await.unwrap();
    assert!(said.contains("3 of 6 chats, $0.10 so far"), "{said}");
    assert!(said.ends_with("exit 0"), "a whole grade succeeds: {said}");
}

#[tokio::test]
async fn a_named_eval_is_in_the_plan_the_person_says_yes_to_and_in_the_start() {
    let place = a_place();
    let (window, mut told) = a_window(&place).await;
    let terminal = at_a_real_terminal(&place, &["--eval", "1"], "y\n");
    let (question, ask) = next_ask(&mut told, PATIENCE).await.expect("the plan's ask");
    let GradeAsk::Plan { of } = ask else {
        panic!("not a plan: {ask:?}");
    };
    assert_eq!(of.evals, ["1"]);
    answers(
        &window,
        question,
        GradeAnswer::Plan(Box::new(plan_naming(&place, &["1"]))),
    )
    .await;
    let (question, ask) = next_ask(&mut told, PATIENCE).await.expect("the start");
    let GradeAsk::Start { of, consent } = ask else {
        panic!("not a start: {ask:?}");
    };
    assert_eq!(of.evals, ["1"], "the start is of what was named");
    assert_eq!(consent, "the-windows-digest");
    answers(
        &window,
        question,
        GradeAnswer::Started { grade: "g".into() },
    )
    .await;
    window
        .call(Command::GradeStep {
            question,
            step: GradeStepTold {
                grade: "g".into(),
                ran: 18,
                of: 18,
                tokens: None,
                cost_micro_usd: None,
                asked: 0,
                ended: Some(GradeEndTold::Whole),
                said: None,
            },
        })
        .await
        .unwrap();
    let said = terminal.await.unwrap();
    assert!(said.contains("Outcome eval 1. ship v2  (on: "), "{said}");
    assert!(said.contains("Outcome eval 2. ship v3  (off:"), "{said}");
    assert!(said.contains("1 outcome eval"), "{said}");
    assert!(said.ends_with("exit 0"), "a whole grade succeeds: {said}");
}

#[tokio::test]
async fn with_no_eval_named_no_eval_runs() {
    let place = a_place();
    let (window, mut told) = a_window(&place).await;
    let terminal = the_terminal(&place, &[], "y\n");
    let (question, ask) = next_ask(&mut told, PATIENCE).await.expect("the plan's ask");
    let GradeAsk::Plan { of } = ask else {
        panic!("not a plan: {ask:?}");
    };
    assert!(of.evals.is_empty(), "{of:?}");
    answers(
        &window,
        question,
        GradeAnswer::Plan(Box::new(plan_of(&place, None))),
    )
    .await;
    let (question, ask) = next_ask(&mut told, PATIENCE).await.expect("the start");
    let GradeAsk::Start { of, .. } = ask else {
        panic!("not a start: {ask:?}");
    };
    assert!(of.evals.is_empty(), "{of:?}");
    answers(
        &window,
        question,
        GradeAnswer::Refused {
            why: "Enough seen.".into(),
        },
    )
    .await;
    let said = terminal.await.unwrap();
    assert!(said.contains("Outcome eval 1. ship v2  (off:"), "{said}");
    assert!(!said.contains("(on:"), "{said}");
    assert!(
        !said.contains("outcome eval"),
        "only the offers say it: {said}"
    );
}

#[tokio::test]
async fn a_named_eval_takes_no_yes_from_a_pipe_or_from_its_flag_and_the_window_is_never_asked() {
    let place = a_place();
    let (_window, mut told) = a_window(&place).await;
    let piped = the_terminal(&place, &["--eval", "1"], "y\n").await.unwrap();
    assert!(piped.contains("typed at a terminal"), "{piped}");
    assert!(piped.ends_with("exit 1"), "{piped}");
    let skipped = at_a_real_terminal(&place, &["--eval", "1", "--yes"], "")
        .await
        .unwrap();
    assert!(skipped.contains("typed at a terminal"), "{skipped}");
    assert!(
        next_ask(&mut told, Duration::from_millis(500))
            .await
            .is_none()
    );
}
