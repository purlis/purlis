// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A bundle of code crosses a link and a relay, both ways, into and out of a project the runner
//! keeps (1.34, #2046). What the runner does with the bundle is a recording's here; the core's
//! own tests move real git (`purlis_core::codelink`).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::hostlink::{self, Handshake, Identity, LinkKey, Liveness, Pinned};
use purlis_session_protocol::link::{self, Acceptor, Link};
use purlis_session_protocol::session::{CallError, Client, CodeRef};
use purlis_session_protocol::view::{Opened, Viewer};
use purlis_terminal::session::Spec;
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

use super::*;
use crate::standin::StandIns;
use crate::{Compose, Dial, Relay, ToCompose};

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);
const REPO: &str = "workspaces/app/web";

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// One take the runner's binary was handed: the project's folder, the repository's in it, the
/// bundle's bytes where one came, and the refs named.
type Taken = (PathBuf, String, Option<Vec<u8>>, Vec<CodeRef>);

/// One conversation the runner's binary kept: what it was carried as, and its bytes.
type KeptConversation = (Carried, Vec<u8>);

/// What the runner's binary was handed: each take.
#[derive(Default, Clone)]
struct Recorded {
    taken: Arc<Mutex<Vec<Taken>>>,
    /// Each work tree taken over: the branch it went onto (1.44).
    taken_over: Arc<Mutex<Vec<String>>>,
    /// Each conversation kept: what it was carried as, and its bytes (1.44).
    conversations: Arc<Mutex<Vec<KeptConversation>>>,
}

const CONVERSATION: &[u8] = b"{\"said\":\"the runner's conversation\"}\n";

const GIVEN: &[u8] = b"a bundle of the runner's branches";

fn fix() -> CodeRef {
    CodeRef {
        name: "refs/heads/fix".into(),
        oid: "4e90af629364f93a93f7ad38f3c4d02189bc5dcc".into(),
    }
}

impl Code for Recorded {
    fn take(
        &self,
        project: &Path,
        repo: &str,
        bundle: Option<&Path>,
        refs: &[CodeRef],
        _checkout: Option<&str>,
    ) -> Result<Vec<CodeRef>, Refusal> {
        let bytes = bundle.map(|path| std::fs::read(path).unwrap());
        self.taken.lock().unwrap().push((
            project.to_path_buf(),
            repo.to_owned(),
            bytes,
            refs.to_vec(),
        ));
        Ok(refs.to_vec())
    }

    fn give(
        &self,
        _project: &Path,
        _repo: &str,
        have: &[String],
        into: &Path,
    ) -> Result<(Vec<CodeRef>, bool), Refusal> {
        if have.contains(&fix().oid) {
            return Ok((vec![fix()], false));
        }
        std::fs::write(into, GIVEN).unwrap();
        Ok((vec![fix()], true))
    }

    fn take_work(
        &self,
        _project: &Path,
        _repo: &str,
        _bundle: Option<&Path>,
        refs: &[CodeRef],
        checkout: &str,
    ) -> Result<Vec<CodeRef>, Refusal> {
        self.taken_over.lock().unwrap().push(checkout.to_owned());
        Ok(refs.to_vec())
    }

    fn take_conversation(
        &self,
        _project: &Path,
        carried: &Carried,
        from: &Path,
    ) -> Result<(), Refusal> {
        let bytes = std::fs::read(from).unwrap();
        self.conversations
            .lock()
            .unwrap()
            .push((carried.clone(), bytes));
        Ok(())
    }

    fn give_conversation(
        &self,
        _project: &Path,
        carried: &Carried,
        into: &Path,
    ) -> Result<String, Refusal> {
        std::fs::write(into, CONVERSATION).unwrap();
        Ok(format!(
            "{{config}}/projects/{{cwd_key}}/{}.jsonl",
            carried.conversation
        ))
    }
}

struct Runner {
    _home: tempfile::TempDir,
    projects: PathBuf,
    host: Hosting,
    recorded: Recorded,
}

fn a_runner() -> Runner {
    let home = tempfile::tempdir().unwrap();
    let projects = home.path().canonicalize().unwrap().join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    let recorded = Recorded::default();
    let host = Hosting::running_with(Echo, StandIns::default())
        .keeping_projects_in(projects.clone())
        .moving_code_with(recorded.clone());
    Runner {
        _home: home,
        projects,
        host,
        recorded,
    }
}

/// A link to `host` admitted as `scope`.
async fn as_scope(scope: Scope, host: impl session::Host) -> Link {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), host, None));
    client.unwrap()
}

fn identity(device: &str) -> Identity {
    Identity {
        device: device.into(),
        key: LinkKey::generate().unwrap(),
    }
}

/// The link between hosts: `runner` serves it as `remote-link`.
async fn between_hosts(runner: impl session::Host) -> Link {
    let (a_end, b_end) = duplex(256 * 1024);
    let desktop = identity("01JA0DESKT0P00000000000000");
    let host_b = identity("01JB0RUNNER0000000000000000");
    let pinned = |identity: &Identity| Pinned {
        device: identity.device.clone(),
        key: identity.key.public(),
    };
    let (peers, b_pinned) = (pinned(&desktop), pinned(&host_b));
    tokio::spawn(async move {
        let answered = hostlink::answer(
            b_end,
            Handshake::THIS_BUILD,
            &host_b,
            &peers,
            Liveness::default(),
        )
        .await
        .unwrap();
        session::serve(answered.link, runner, None).await
    });
    hostlink::connect(
        a_end,
        Handshake::THIS_BUILD,
        &desktop,
        &b_pinned,
        Liveness::default(),
    )
    .await
    .unwrap()
}

fn over_the_link(runner: Hosting) -> Dial {
    Arc::new(move || {
        let runner = runner.clone();
        Box::pin(async move { Ok(between_hosts(runner).await) })
    })
}

/// A client on the desktop's host as `scope`, which relays to `runner` over the link.
async fn through_the_relay(scope: Scope, runner: &Runner) -> (Client, Acceptor) {
    let relay = Relay::new(over_the_link(runner.host.clone()));
    let (client, streams, _events) = Client::new(as_scope(scope, relay).await);
    (client, streams)
}

/// The window's side of one transfer: the code stream the host opens for `transfer`.
async fn code_stream(streams: &mut Acceptor, transfer: u64) -> link::Stream {
    let stream = tokio::time::timeout(PATIENCE, streams.accept())
        .await
        .expect("the host opened a stream")
        .unwrap();
    match Viewer::default().accept_any(stream).await.unwrap() {
        Opened::Code(header, stream) => {
            assert_eq!(header.transfer, transfer);
            stream
        }
        Opened::View(_) => panic!("a view, where a code stream was meant"),
    }
}

/// Take `bytes` into the runner's repository through `client`, as the window does.
async fn take(
    client: &Client,
    streams: &mut Acceptor,
    bytes: &[u8],
) -> Result<Vec<CodeRef>, CallError> {
    let asked = client.take_code(PROJECT, REPO, 7, vec![fix()], None);
    let writing = async {
        let mut stream = code_stream(streams, 7).await;
        let _ = stream.write_all(bytes).await;
        let _ = stream.shutdown().await;
    };
    let (answered, ()) = tokio::time::timeout(PATIENCE, async { tokio::join!(asked, writing) })
        .await
        .expect("the take was answered");
    answered
}

#[tokio::test]
async fn a_bundle_the_window_writes_reaches_a_project_the_runner_keeps_through_the_relay() {
    let runner = a_runner();
    let (client, mut streams) = through_the_relay(Scope::LocalUi, &runner).await;
    client.hold_project(PROJECT, "").await.unwrap();
    let kept = runner.projects.join(PROJECT);
    assert!(kept.is_dir(), "the runner made the project's folder");

    let heads = take(&client, &mut streams, b"the desktop's bundle")
        .await
        .unwrap();

    assert_eq!(heads, vec![fix()]);
    let taken = runner.recorded.taken.lock().unwrap().clone();
    assert_eq!(
        taken,
        vec![(
            kept,
            REPO.to_owned(),
            Some(b"the desktop's bundle".to_vec()),
            vec![fix()]
        )]
    );
    // Nothing of the transfer is left beside the projects.
    let left: Vec<_> = std::fs::read_dir(&runner.projects)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(left, vec![std::ffi::OsString::from(PROJECT)]);
}

#[tokio::test]
async fn an_empty_code_stream_takes_the_refs_from_what_the_repository_holds() {
    let runner = a_runner();
    let (client, mut streams) = through_the_relay(Scope::LocalUi, &runner).await;
    client.hold_project(PROJECT, "").await.unwrap();
    take(&client, &mut streams, b"").await.unwrap();
    let taken = runner.recorded.taken.lock().unwrap().clone();
    assert_eq!(taken[0].2, None);
}

#[tokio::test]
async fn the_runners_bundle_reaches_the_window_through_the_relay_and_nothing_when_it_lacks_nothing()
{
    let runner = a_runner();
    let (client, mut streams) = through_the_relay(Scope::LocalUi, &runner).await;
    client.hold_project(PROJECT, "").await.unwrap();

    let asked = client.give_code(PROJECT, REPO, 8, Vec::new());
    let reading = async {
        let mut stream = code_stream(&mut streams, 8).await;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        bytes
    };
    let (heads, bytes) = tokio::time::timeout(PATIENCE, async { tokio::join!(asked, reading) })
        .await
        .unwrap();
    assert_eq!((heads.unwrap(), bytes), (vec![fix()], GIVEN.to_vec()));

    let asked = client.give_code(PROJECT, REPO, 9, vec![fix().oid]);
    let reading = async {
        let mut stream = code_stream(&mut streams, 9).await;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        bytes
    };
    let (heads, bytes) = tokio::time::timeout(PATIENCE, async { tokio::join!(asked, reading) })
        .await
        .unwrap();
    assert_eq!((heads.unwrap(), bytes), (vec![fix()], Vec::new()));
}

#[tokio::test]
async fn a_bundle_past_the_hosts_bound_is_refused_too_large_and_nothing_is_taken() {
    let mut runner = a_runner();
    runner.host = runner.host.clone().holding_bundles_to(8);
    let (client, mut streams) = through_the_relay(Scope::LocalUi, &runner).await;
    client.hold_project(PROJECT, "").await.unwrap();

    let refused = take(&client, &mut streams, b"nine byte").await.unwrap_err();

    let refusal = refused.refusal().expect("the host refused it");
    assert_eq!(refusal.code, session::TOO_LARGE);
    assert!(
        refusal.why.contains("larger than 8 bytes"),
        "{}",
        refusal.why
    );
    assert!(runner.recorded.taken.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_project_held_at_a_place_the_client_named_moves_no_code() {
    let runner = a_runner();
    let elsewhere = tempfile::tempdir().unwrap();
    let (client, mut streams) = through_the_relay(Scope::LocalUi, &runner).await;
    client
        .hold_project(PROJECT, elsewhere.path().to_str().unwrap())
        .await
        .unwrap();
    let refused = tokio::time::timeout(
        PATIENCE,
        client.take_code(PROJECT, REPO, 7, vec![fix()], None),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert_eq!(refused.refusal().unwrap().code, session::NOT_ALLOWED);
    assert!(runner.recorded.taken.lock().unwrap().is_empty());
    // And no stream was opened for it.
    assert!(
        tokio::time::timeout(Duration::from_millis(200), streams.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_hold_whose_id_is_not_a_projects_makes_no_folder() {
    let runner = a_runner();
    let (client, _streams) = through_the_relay(Scope::LocalUi, &runner).await;
    for id in [
        "../escape",
        "01J9PRJCT0000000000000000/",
        "a",
        "01J9prjct00000000000000000",
    ] {
        let refused = client.hold_project(id, "").await.unwrap_err();
        assert_eq!(
            refused.refusal().unwrap().code,
            session::NOT_ALLOWED,
            "{id}"
        );
    }
    assert_eq!(std::fs::read_dir(&runner.projects).unwrap().count(), 0);
    assert!(!runner.projects.parent().unwrap().join("escape").exists());
}

#[tokio::test]
async fn a_host_that_keeps_no_projects_holds_none_without_a_place() {
    let host = Hosting::running_with(Echo, StandIns::default());
    let (client, _streams, _events) = Client::new(as_scope(Scope::LocalUi, host).await);
    let refused = client.hold_project(PROJECT, "").await.unwrap_err();
    assert_eq!(refused.refusal().unwrap().code, session::NO_SUCH_PROJECT);
}

#[tokio::test]
async fn only_the_window_moves_code_through_a_relay() {
    let runner = a_runner();
    let mut clients = Vec::new();
    for scope in [Scope::Terminal, Scope::FleetMcp] {
        clients.push((scope, through_the_relay(scope, &runner).await.0));
    }
    // Another host's link to the relay: granted code as the desktop's host relaying its own
    // window would be, and still refused, since the relay sends a transfer on for its window alone.
    let relay = Relay::new(over_the_link(runner.host.clone()));
    let (another_host, _streams, _events) = Client::new(between_hosts(relay).await);
    clients.push((Scope::RemoteLink, another_host));
    for (scope, client) in clients {
        for asked in [
            client.take_code(PROJECT, REPO, 1, vec![fix()], None).await,
            client.give_code(PROJECT, REPO, 2, Vec::new()).await,
        ] {
            let refused = asked.unwrap_err();
            assert_eq!(
                refused.refusal().unwrap().code,
                session::NOT_ALLOWED,
                "{scope}"
            );
        }
    }
    assert!(runner.recorded.taken.lock().unwrap().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn a_projects_folder_that_is_a_link_is_not_held_and_nothing_is_taken_through_it() {
    let runner = a_runner();
    let elsewhere = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), runner.projects.join(PROJECT)).unwrap();
    let (client, _streams) = through_the_relay(Scope::LocalUi, &runner).await;

    let refused = client.hold_project(PROJECT, "").await.unwrap_err();

    assert_eq!(refused.refusal().unwrap().code, session::NOT_ALLOWED);
    let refused = tokio::time::timeout(
        PATIENCE,
        client.take_code(PROJECT, REPO, 7, vec![fix()], None),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(refused.refusal().is_some(), "{refused:?}");
    assert!(runner.recorded.taken.lock().unwrap().is_empty());
    assert_eq!(std::fs::read_dir(elsewhere.path()).unwrap().count(), 0);
}

const ID: &str = "0d6f3c1e-8a4b-4c7d-9e21-5b3f2a1c0e99";

#[tokio::test]
async fn a_moving_chats_work_tree_is_taken_over_and_never_taken_as_a_plain_send() {
    let runner = a_runner();
    let (client, mut streams) = through_the_relay(Scope::LocalUi, &runner).await;
    client.hold_project(PROJECT, "").await.unwrap();

    let asked = client.take_work(PROJECT, REPO, 21, vec![fix()], fix().name);
    let writing = async {
        let mut stream = code_stream(&mut streams, 21).await;
        let _ = stream.write_all(b"the work's bundle").await;
        let _ = stream.shutdown().await;
    };
    let (answered, ()) = tokio::time::timeout(PATIENCE, async { tokio::join!(asked, writing) })
        .await
        .unwrap();
    answered.unwrap();
    assert_eq!(
        *runner.recorded.taken_over.lock().unwrap(),
        vec![fix().name]
    );
    assert!(runner.recorded.taken.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_conversation_crosses_the_relay_both_ways_with_the_names_it_is_kept_by() {
    let runner = a_runner();
    let (client, mut streams) = through_the_relay(Scope::LocalUi, &runner).await;
    client.hold_project(PROJECT, "").await.unwrap();
    let file = format!("{{config}}/projects/{{cwd_key}}/{ID}.jsonl");

    let asked = client.take_conversation(PROJECT, 22, "app", "claude", ID, &file);
    let writing = async {
        let mut stream = code_stream(&mut streams, 22).await;
        let _ = stream.write_all(b"the desktop's conversation").await;
        let _ = stream.shutdown().await;
    };
    let (answered, ()) = tokio::time::timeout(PATIENCE, async { tokio::join!(asked, writing) })
        .await
        .unwrap();
    answered.unwrap();
    assert_eq!(
        *runner.recorded.conversations.lock().unwrap(),
        vec![(
            Carried {
                workspace: "app".into(),
                profile: "claude".into(),
                conversation: ID.into(),
                file: file.clone(),
            },
            b"the desktop's conversation".to_vec()
        )]
    );

    let asked = client.give_conversation(PROJECT, 23, "app", "claude", ID);
    let reading = async {
        let mut stream = code_stream(&mut streams, 23).await;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        bytes
    };
    let (kept, bytes) = tokio::time::timeout(PATIENCE, async { tokio::join!(asked, reading) })
        .await
        .unwrap();
    assert_eq!((kept.unwrap(), bytes), (file, CONVERSATION.to_vec()));
}

#[tokio::test]
async fn only_the_window_carries_a_conversation_through_the_relay() {
    let runner = a_runner();
    for scope in [Scope::Terminal, Scope::FleetMcp] {
        let (client, _streams) = through_the_relay(scope, &runner).await;
        let refused = tokio::time::timeout(
            PATIENCE,
            client.give_conversation(PROJECT, 24, "app", "claude", ID),
        )
        .await
        .unwrap()
        .unwrap_err();
        let refusal = refused.refusal().expect("refused");
        assert_eq!(refusal.code, session::NOT_ALLOWED, "{scope:?}");
    }
    assert!(runner.recorded.conversations.lock().unwrap().is_empty());
}
