//! The host a paired phone talks to (iOS design §4.3, §5): what it lists, what it refuses, and
//! what reaches the app's own seam. Over `Planes::telling` and the pretend session host, the
//! way `lib.rs`'s tests build a plane, with the protocol's own client on a duplex link.

use std::sync::Arc;

use purlis_core::engine::Size;
use purlis_core::harness::asks::test_support;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, NO_SUCH_CHAT, NOT_AN_AGENT};
use tokio::io::duplex;

use super::board::{Board, DeviceOf, Opened, silent};
use crate::host::pretend::Pretend;
use crate::planes::Planes;

pub(super) const KEPT: &str = "01J9ZQ3V7K8M2N4P6R8T0V2X4Z";
const SHELL: &str = "01J9SHELL00000000000000000";

fn size() -> Size {
    Size {
        columns: 80,
        rows: 24,
    }
}

/// A plane on the pretend host, with nothing open yet.
fn a_plane(pretend: &Pretend) -> Arc<Planes> {
    a_plane_recording(pretend, None)
}

/// [`a_plane`], its hooks recording into `events` where one is given.
pub(super) fn a_plane_recording(
    pretend: &Pretend,
    events: Option<crate::hooks::Events>,
) -> Arc<Planes> {
    a_plane_telling(pretend, events, Arc::new(|_: crate::hooks::Moved| {}))
}

/// [`a_plane_recording`], the window told each move through `tell`.
pub(super) fn a_plane_telling(
    pretend: &Pretend,
    events: Option<crate::hooks::Events>,
    tell: Arc<dyn Fn(crate::hooks::Moved) + Send + Sync>,
) -> Arc<Planes> {
    let dir = tempfile::tempdir().unwrap().keep();
    let root = std::fs::canonicalize(&dir).unwrap();
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").unwrap();
    std::fs::create_dir_all(root.join("workspaces/login/repo")).unwrap();
    let host = pretend.clone();
    let planes = Planes::telling(tell, crate::Shipped::default(), None)
        .running_sessions_on(Arc::new(move |_| Box::new(host.clone())))
        .recording_events(events);
    planes.open(&root);
    Arc::new(planes)
}

/// Waits up to five seconds for `holds`.
async fn until(holds: impl Fn() -> bool) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !holds() {
        if std::time::Instant::now() > deadline {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    true
}

#[tokio::test]
async fn a_link_that_drops_closes_every_view_and_timeline_it_held_open_in_the_app() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let (client, views) = linked(board_over(planes.clone())).await;
    client.attach(KEPT).await.unwrap();
    client.attach_timeline(KEPT).await.unwrap();
    assert_eq!(pretend.watching(session).len(), 1);
    // The phone went away: its link closes, and nothing it held stays open on the desktop.
    drop(client);
    drop(views);
    assert!(
        until(|| pretend.watching(session).is_empty()).await,
        "the app's view is closed when the link drops, timeline or no timeline"
    );
}

#[tokio::test]
async fn the_screen_a_view_opens_on_reaches_the_phone_once() {
    use purlis_session_protocol::view::{Chunk, Viewer};
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    pretend.shows(session, "screen so far");
    let (client, mut views) = linked(board_over(planes)).await;
    client.attach(KEPT).await.unwrap();
    let mut reader = Viewer::default()
        .accept(views.accept().await.unwrap())
        .await
        .unwrap();
    pretend.emit(session, "later");
    let mut seen = Vec::new();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !String::from_utf8_lossy(&seen).contains("later") {
            match reader.next().await.unwrap().unwrap() {
                Chunk::Snapshot(b) | Chunk::Live(b) => {
                    reader.ack(b.len()).await.unwrap();
                    seen.extend_from_slice(&b);
                }
            }
        }
    })
    .await
    .expect("the live bytes follow the screen");
    let text = String::from_utf8_lossy(&seen);
    assert_eq!(text.matches("screen so far").count(), 1, "{text}");
}

#[tokio::test]
async fn stop_from_the_phone_is_the_planes_own_close_which_tells_the_window() {
    let told = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counting = Arc::clone(&told);
    let pretend = Pretend::default();
    let planes = a_plane_telling(
        &pretend,
        None,
        Arc::new(move |_| {
            counting.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }),
    );
    let session = start(&planes, "/nowhere/claude", "one", Some(KEPT), true);
    let (client, _views) = linked(board_over(planes.clone())).await;
    let before = told.load(std::sync::atomic::Ordering::SeqCst);
    client.stop(KEPT).await.unwrap();
    assert!(pretend.closed().contains(&session));
    assert!(
        told.load(std::sync::atomic::Ordering::SeqCst) > before,
        "the window hears the chat close, as it does for its own ×"
    );
}

fn root_of(planes: &Planes) -> std::path::PathBuf {
    let plane = planes.open_now()[0].clone();
    planes.held(&plane).unwrap().root().to_path_buf()
}

/// Start a chat named `name` running `program` with `id`, in the plane's `login` workspace
/// when `in_workspace`.
pub(super) fn start(
    planes: &Planes,
    program: &str,
    name: &str,
    id: Option<&str>,
    in_workspace: bool,
) -> u32 {
    let plane = planes.open_now()[0].clone();
    let held = planes.held(&plane).unwrap();
    let root = root_of(planes);
    let chat = purlis_core::reopen::Chat {
        program: program.to_owned(),
        name: name.to_owned(),
        cwd: in_workspace.then(|| root.join("workspaces/login/repo")),
        identity: purlis_core::reopen::Identity {
            id: id.map(str::to_owned),
            ..Default::default()
        },
        ..Default::default()
    };
    held.chats()
        .start(&chat, size())
        .expect("the host starts it")
}

fn a_plane_with_one_chat(pretend: &Pretend) -> (Arc<Planes>, u32) {
    let planes = a_plane(pretend);
    let session = start(&planes, "/nowhere/claude", "one", Some(KEPT), true);
    (planes, session)
}

pub(super) fn board_over(planes: Arc<Planes>) -> Board {
    Board::new(
        Arc::new(planes) as Arc<dyn Opened>,
        DeviceOf {
            key: [7u8; 32],
            device: "01J9PHONE00000000000000000".into(),
        },
        silent(),
    )
}

pub(super) async fn linked(board: Board) -> (Client, purlis_session_protocol::link::Acceptor) {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect_as_a_device(a, session::speaks(), Scope::RemoteDevice),
        link::serve_as_a_device(
            b,
            session::speaks(),
            link::ProvenDevice::stand_in_as(Scope::RemoteDevice)
        )
    );
    tokio::spawn(session::serve(served.unwrap(), board, None));
    let (client, views, _events) = Client::new(client.unwrap());
    (client, views)
}

pub(super) fn held_of(planes: &Planes) -> Arc<crate::planes::Held> {
    let plane = planes.open_now()[0].clone();
    planes.held(&plane).unwrap()
}

#[tokio::test]
async fn list_names_each_open_chat_by_its_id_project_workspace_and_level() {
    let pretend = Pretend::default();
    let (planes, _session) = a_plane_with_one_chat(&pretend);
    let (client, _views) = linked(board_over(planes.clone())).await;
    let chats = client.list().await.unwrap();
    assert_eq!(chats.len(), 1);
    let chat = &chats[0];
    assert_eq!(chat.chat, KEPT);
    assert_eq!(chat.title.as_deref(), Some("one"));
    assert_eq!(chat.workspace.as_deref(), Some("login"));
    assert_eq!(chat.level, Some(2), "a harness chat is level 2");
    assert_eq!(chat.project.len(), 26, "the project's stable id, a ULID");
    assert!(chat.project_path.is_some());
    assert!(["working", "needs_you", "idle", "ended"].contains(&chat.state.as_str()));
}

#[tokio::test]
async fn a_chat_is_named_only_by_the_id_the_app_minted_for_it() {
    let pretend = Pretend::default();
    let planes = a_plane(&pretend);
    let session = start(&planes, "/nowhere/claude", "nameless", None, false);
    let (client, _views) = linked(board_over(planes.clone())).await;
    let listed = client.list().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].chat.len(), 26, "the app minted it an id at start");
    assert_eq!(listed[0].workspace, None, "started at the project root");
    // Neither its session number nor its name names it on the wire.
    for not_it in [
        session.to_string(),
        "nameless".to_owned(),
        "not-a-chat".to_owned(),
    ] {
        let refused = client.attach(&not_it).await.unwrap_err();
        assert_eq!(
            refused.refusal().map(|r| r.code.as_str()),
            Some(NO_SUCH_CHAT),
            "{not_it}"
        );
    }
    let refused = client.attach("not-a-chat").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(NO_SUCH_CHAT)
    );
}

#[tokio::test]
async fn write_reaches_the_session_and_prompt_is_one_bracketed_paste_then_enter() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let (client, _views) = linked(board_over(planes)).await;
    client.write(KEPT, b"y").await.unwrap();
    client
        .prompt(KEPT, "also add a test for expiry")
        .await
        .unwrap();
    let typed = pretend.typed(session);
    assert_eq!(typed[0], b"y");
    assert_eq!(
        typed[1],
        format!(
            "{}\r",
            purlis_core::curation::bracketed("also add a test for expiry")
        )
        .into_bytes(),
        "one write: the paste and Enter together"
    );
}

#[tokio::test]
async fn a_prompt_to_a_shell_tab_is_not_an_agent() {
    let pretend = Pretend::default();
    let planes = a_plane(&pretend);
    start(&planes, "/bin/zsh", "shell", Some(SHELL), false);
    let (client, _views) = linked(board_over(planes)).await;
    let listed = client.list().await.unwrap();
    assert_eq!(listed[0].level, Some(1));
    let refused = client.prompt(SHELL, "ls").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(NOT_AN_AGENT)
    );
    client.write(SHELL, b"ls\r").await.unwrap();
}

#[tokio::test]
async fn a_prompt_to_a_chat_that_ended_is_refused() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let (client, _views) = linked(board_over(planes.clone())).await;
    assert_eq!(client.list().await.unwrap().len(), 1);
    // The chat left the board between list and prompt.
    held_of(&planes).chats().close(session).unwrap();
    let refused = client.prompt(KEPT, "hello?").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(NO_SUCH_CHAT)
    );
}

#[tokio::test]
async fn attach_opens_a_view_fed_by_the_sessions_output_and_detach_closes_it() {
    use purlis_session_protocol::view::{Chunk, Viewer};
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let (client, mut views) = linked(board_over(planes)).await;
    let attached = client.attach(KEPT).await.unwrap();
    assert_eq!((attached.cols, attached.rows), (80, 24));
    assert_eq!(pretend.watching(session).len(), 1, "the app's view is open");
    pretend.emit(session, "hello from the pane");
    let mut reader = Viewer::default()
        .accept(views.accept().await.unwrap())
        .await
        .unwrap();
    let mut seen = Vec::new();
    let deadline = std::time::Duration::from_secs(5);
    tokio::time::timeout(deadline, async {
        while !String::from_utf8_lossy(&seen).contains("hello from the pane") {
            match reader.next().await.unwrap().unwrap() {
                Chunk::Snapshot(b) | Chunk::Live(b) => {
                    reader.ack(b.len()).await.unwrap();
                    seen.extend_from_slice(&b);
                }
            }
        }
    })
    .await
    .expect("the pane's bytes reach the phone");
    client.detach(KEPT, attached.view).await.unwrap();
    assert!(
        pretend.watching(session).is_empty(),
        "the app's view was closed"
    );
}

#[tokio::test]
async fn an_ask_the_window_answered_first_is_refused_to_the_device() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let held = held_of(&planes);
    let raised = held.hooks().asks().raise_ask(
        &session.to_string(),
        test_support::an_allow_ask(),
        std::time::Instant::now(),
    );
    let ask = raised.raised.id.to_string();
    held.hooks().answer(session, &ask, "allow").unwrap();
    let (client, _views) = linked(board_over(planes.clone())).await;
    let refused = client.answer(KEPT, &ask, "allow").await.unwrap_err();
    let why = refused.refusal().unwrap();
    assert_ne!(
        why.code,
        session::NOT_ALLOWED,
        "not a scope refusal: an answered ask"
    );
    assert!(why.why.contains("desktop"), "{}", why.why);
}

#[tokio::test]
async fn an_open_ask_answered_from_the_device_closes() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let held = held_of(&planes);
    let raised = held.hooks().asks().raise_ask(
        &session.to_string(),
        test_support::an_allow_ask(),
        std::time::Instant::now(),
    );
    let ask = raised.raised.id.to_string();
    let (client, _views) = linked(board_over(planes.clone())).await;
    client.answer(KEPT, &ask, "allow").await.unwrap();
    assert!(
        held.hooks()
            .asks()
            .pending(std::time::Instant::now())
            .is_empty()
    );
}

#[tokio::test]
async fn a_lasting_choice_and_a_secrets_ask_are_refused_before_the_board_hears_them() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let held = held_of(&planes);
    let now = std::time::Instant::now();
    // One pane ask at a time: a chat's next ask supersedes the one before it.
    let lasting = held.hooks().asks().raise_ask(
        &session.to_string(),
        test_support::an_ask_with_an_always_choice(),
        now,
    );
    let (client, _views) = linked(board_over(planes.clone())).await;
    let lasting_id = lasting.raised.id.to_string();
    let refused = client
        .answer(KEPT, &lasting_id, "always")
        .await
        .unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_ALLOWED)
    );
    client.answer(KEPT, &lasting_id, "once").await.unwrap();
    let secret = held.hooks().asks().raise_ask(
        &session.to_string(),
        test_support::an_ask_that_elicits_a_secret(),
        now,
    );
    let secret_id = secret.raised.id.to_string();
    let refused = client.answer(KEPT, &secret_id, "yes").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_ALLOWED)
    );
    assert_eq!(
        held.hooks().asks().pending(std::time::Instant::now()).len(),
        1,
        "the secret's ask stays open"
    );
}

#[tokio::test]
async fn stop_ends_the_chat() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let (client, _views) = linked(board_over(planes.clone())).await;
    client.stop(KEPT).await.unwrap();
    assert!(pretend.closed().contains(&session));
    assert!(client.list().await.unwrap().is_empty());
}
