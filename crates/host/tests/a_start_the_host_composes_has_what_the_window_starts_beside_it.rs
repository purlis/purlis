// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A start with no launch is composed in the host (#2171): its binary composes it from the
//! project's own files, and what the window starts beside a start of its own the host starts
//! here. The chat is issued its token by its project's channel, its confinement is the program's
//! own from the first and asks nobody, and what the start decided reaches the host's event log.
//! The binary's composition is a stand-in here; the core's is the app's behaviour suite.

#![cfg(unix)]

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_host::standin::{StandInConfines, StandIns};
use purlis_host::{Channel, Compose, Ended, HandOn, Hear, Hosting, Record, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Beside, Client, ConfinePlan, Layer, Listed, ProgramKind, Reporting, StartProgram,
};
use purlis_terminal::session::Spec;
use serde_json::json;
use tokio::io::duplex;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// Composes a start with no launch as its binary would: it asks the host for the confinement
/// it plans and writes what it decided, then runs `/bin/echo` naming its confinement and its
/// token. A start whose first message is `refuse` is refused once its confinement is started.
struct ComposesItself;

impl Compose for ComposesItself {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        assert!(starting.start.launch.is_none(), "a start the host composes");
        let confined = starting.beside.confine(&plan())?;
        starting.beside.record(json!({ "decided": "sandboxed" }))?;
        if starting.start.first_message.as_deref() == Some("refuse") {
            return Err("this one is refused".into());
        }
        let mut spec = Spec::new("/bin/echo", starting.size);
        spec.env
            .push(("CONFINED".into(), confined.confinement.into()));
        spec.env.push((
            "TOKEN".into(),
            starting
                .reporting
                .map(|reporting| reporting.token.clone())
                .unwrap_or_default()
                .into(),
        ));
        let ids = |id: &Option<String>| id.clone().unwrap_or_default().into();
        spec.env.push(("CHAT".into(), ids(&starting.start.program)));
        spec.env.push(("RUN".into(), ids(&starting.start.run)));
        spec.env
            .push(("STARTED_BY".into(), starting.started_by.word().into()));
        Ok(spec)
    }
}

fn plan() -> ConfinePlan {
    ConfinePlan {
        listed: vec![Listed {
            host: "api.anthropic.com".into(),
            layer: Layer::Open,
        }],
        never: Vec::new(),
        withdrawn: Vec::new(),
        anything: false,
        tunnel_ports: vec![443],
        plain_ports: vec![80],
        any_port: false,
        beside: Beside::Wrapped,
        given: Vec::new(),
    }
}

struct Hears;

impl Hear for Hears {
    fn open(
        &self,
        _project: &str,
        _path: &Path,
        _hand_on: HandOn,
    ) -> Result<Option<Arc<dyn Channel>>, String> {
        Ok(Some(Arc::new(TokenPerChat)))
    }
}

struct TokenPerChat;

impl Channel for TokenPerChat {
    fn report_to(&self, number: u32) -> Result<Reporting, String> {
        Ok(Reporting {
            socket: "/the/project/hooks.sock".into(),
            token: format!("token-{number}"),
        })
    }
    fn bind(&self, _number: u32, _pid: u32) {}
    fn ended(&self, _number: u32, _pid: u32) {}
    fn forget(&self, _number: u32) {}
    fn replied(&self, _id: u64, _reply: serde_json::Value) {}
}

/// The host's event log: what was written in it.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<serde_json::Value>>>);

impl Record for Log {
    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        self.0.lock().unwrap().push(entry);
        Ok(())
    }

    fn ended(&self, _ended: &Ended<'_>) {}
}

struct Linked {
    client: Client,
    runs: StandIns,
    confines: StandInConfines,
    log: Log,
    _project: tempfile::TempDir,
}

async fn linked() -> Linked {
    linked_as(Scope::LocalUi).await
}

/// [`linked`], the start's link admitted as `scope`; the project is held on a window's link.
async fn linked_as(scope: Scope) -> Linked {
    let (runs, confines, log) = (
        StandIns::default(),
        StandInConfines::default(),
        Log::default(),
    );
    let hosting = Hosting::with(ComposesItself, runs.clone(), Hears, confines.clone())
        .recording_with(log.clone());
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    let (window, _views, _events) = Client::new(client.unwrap());
    let project = tempfile::tempdir().unwrap();
    window
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .expect("held");
    let client = if scope == Scope::LocalUi {
        window
    } else if scope == Scope::RemoteLink {
        std::mem::forget(window);
        Client::new(over_the_link_between_hosts(hosting).await).0
    } else {
        let (a, b) = duplex(256 * 1024);
        let (client, served) = tokio::join!(
            link::connect(a, session::speaks(), scope, HELD.of(scope)),
            link::serve_any(b, session::speaks(), &HELD)
        );
        tokio::spawn(session::serve(served.unwrap(), hosting, None));
        std::mem::forget(window);
        Client::new(client.unwrap()).0
    };
    Linked {
        client,
        runs,
        confines,
        log,
        _project: project,
    }
}

/// A link to `hosting` as the link between hosts carries it: served as `remote-link`, from the
/// desktop it pinned.
async fn over_the_link_between_hosts(hosting: Hosting) -> link::Link {
    use purlis_session_protocol::hostlink::{self, Handshake, Identity, LinkKey, Liveness, Pinned};
    let identity = |device: &str| Identity {
        device: device.into(),
        key: LinkKey::generate().unwrap(),
    };
    let (desktop, runner) = (
        identity("01JA0DESKT0P00000000000000"),
        identity("01JB0RUNNER0000000000000000"),
    );
    let pinned = |identity: &Identity| Pinned {
        device: identity.device.clone(),
        key: identity.key.public(),
    };
    let (peer, runner_pinned) = (pinned(&desktop), pinned(&runner));
    let (a, b) = duplex(256 * 1024);
    tokio::spawn(async move {
        let answered = hostlink::answer(
            b,
            Handshake::THIS_BUILD,
            &runner,
            &peer,
            Liveness::default(),
        )
        .await
        .unwrap();
        session::serve(answered.link, hosting, None).await
    });
    hostlink::connect(
        a,
        Handshake::THIS_BUILD,
        &desktop,
        &runner_pinned,
        Liveness::default(),
    )
    .await
    .unwrap()
}

fn composed(first_message: Option<&str>) -> StartProgram {
    StartProgram {
        persona: Some("steward".into()),
        profile: Some("work".into()),
        first_message: first_message.map(str::to_owned),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

fn env_of(spec: &Spec, name: &str) -> Option<String> {
    spec.env
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.to_string_lossy().into_owned())
}

async fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + PATIENCE;
    while !done() {
        assert!(
            std::time::Instant::now() < deadline,
            "{what} never happened"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn a_start_with_no_launch_runs_with_its_token_in_a_confinement_of_its_own() {
    let linked = linked().await;

    let program = linked
        .client
        .start_program(composed(None))
        .await
        .expect("started");

    let (spec, _) = linked.runs.ran().pop().expect("it ran");
    assert_eq!(env_of(&spec, "TOKEN").as_deref(), Some("token-1"));
    let started = linked.confines.seen()[0].confine.clone();
    assert_eq!(env_of(&spec, "CONFINED"), Some(started.confinement.clone()));
    assert_eq!(started.project, PROJECT);
    assert_eq!(started.plan, plan());
    assert!(!started.asks, "no window hears what it would ask about");
    assert_eq!(
        *linked.log.0.lock().unwrap(),
        [json!({ "decided": "sandboxed" })]
    );

    linked.client.close(&program).await.expect("closed");
    until("its confinement ends with it", || {
        linked.confines.ended(&started.confinement)
    })
    .await;
}

#[tokio::test]
async fn a_start_the_host_could_not_compose_ends_the_confinement_it_asked_for() {
    let linked = linked().await;

    let refused = linked
        .client
        .start_program(composed(Some("refuse")))
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|refusal| refusal.code.as_str()),
        Some(session::NOT_STARTED)
    );
    assert!(linked.runs.ran().is_empty(), "nothing ran");
    let started = linked.confines.seen()[0].confine.confinement.clone();
    until("its confinement ends with the start", || {
        linked.confines.ended(&started)
    })
    .await;
}

/// Its ids as the window names them: the chat's own and its run's.
fn named_ids(mut start: StartProgram) -> StartProgram {
    start.program = Some("01J9CHAT000000000000000000".into());
    start.run = Some("01J9RUN0000000000000000000".into());
    start
}

#[tokio::test]
async fn a_start_from_the_window_runs_under_the_ids_the_window_named() {
    let linked = linked().await;

    let program = linked
        .client
        .start_program(named_ids(composed(None)))
        .await
        .expect("started");

    let (spec, _) = linked.runs.ran().pop().expect("it ran");
    assert_eq!(program, "01J9CHAT000000000000000000");
    assert_eq!(
        env_of(&spec, "CHAT").as_deref(),
        Some("01J9CHAT000000000000000000")
    );
    assert_eq!(
        env_of(&spec, "RUN").as_deref(),
        Some("01J9RUN0000000000000000000")
    );
    assert_eq!(env_of(&spec, "STARTED_BY").as_deref(), Some("local-ui"));
}

/// **A runner keeps the ids its desktop relays for the window** (#2446): what it writes of the
/// chat joins the window's own chat and run (ADR 0066). A chat the runner already holds is not
/// started again under its id.
#[tokio::test]
async fn a_start_relayed_over_the_link_runs_under_the_ids_the_window_named() {
    let linked = linked_as(Scope::RemoteLink).await;

    let program = linked
        .client
        .start_program(named_ids(composed(None)))
        .await
        .expect("started");

    let (spec, _) = linked.runs.ran().pop().expect("it ran");
    assert_eq!(program, "01J9CHAT000000000000000000");
    assert_eq!(
        env_of(&spec, "RUN").as_deref(),
        Some("01J9RUN0000000000000000000")
    );
    assert_eq!(env_of(&spec, "STARTED_BY").as_deref(), Some("remote-link"));
    let again = linked
        .client
        .start_program(named_ids(composed(None)))
        .await
        .unwrap_err();
    assert_eq!(
        again.refusal().map(|refusal| refusal.code.as_str()),
        Some(session::MALFORMED)
    );
    assert_eq!(linked.runs.ran().len(), 1, "nothing ran again");
}

#[tokio::test]
async fn a_start_from_another_scope_runs_under_ids_the_host_minted() {
    let linked = linked_as(Scope::Terminal).await;

    let program = linked
        .client
        .start_program(named_ids(composed(None)))
        .await
        .expect("started");

    let (spec, _) = linked.runs.ran().pop().expect("it ran");
    let (chat, run) = (
        env_of(&spec, "CHAT").unwrap(),
        env_of(&spec, "RUN").unwrap(),
    );
    assert_eq!(chat, program, "it runs under the id it is answered by");
    assert_ne!(chat, "01J9CHAT000000000000000000");
    assert!(!run.is_empty());
    assert_ne!(run, "01J9RUN0000000000000000000");
    assert_eq!(env_of(&spec, "STARTED_BY").as_deref(), Some("terminal"));
}

#[tokio::test]
async fn a_start_whose_confinement_could_not_be_started_runs_nothing() {
    let linked = linked().await;
    linked.confines.refuse("no loopback port was free");

    let refused = linked
        .client
        .start_program(composed(None))
        .await
        .unwrap_err();

    assert_eq!(
        refused.refusal().map(|refusal| refusal.code.as_str()),
        Some(session::NOT_STARTED)
    );
    assert!(
        linked.runs.ran().is_empty(),
        "nothing runs without its proxy"
    );
}
