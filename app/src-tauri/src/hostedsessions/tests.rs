// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's side of a chat in the session host, over an in-memory link to a real host whose
//! programs are stand-ins, so none of this needs a terminal: what the window hands the host for
//! a start, and how it draws, drives, hears the end of and closes what the host runs. The same
//! over real terminals is the behaviour suite (`crate::behaviour`), on CI.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::engine::Size;
use purlis_host::Hosting;
use purlis_host::standin::{Savers, StandIns};
use purlis_session_protocol::session::{self, Exit};

use super::*;

/// The window's connection to the host `hosting` serves as, over in-memory links, run on
/// `runtime`: its own link, ready, and one more opened for each project root, as the window's
/// are over the host's socket.
pub(crate) fn in_memory<H: session::Host + Clone>(
    hosting: H,
    runtime: &tokio::runtime::Runtime,
) -> (Arc<Connection>, Arc<HostLink>) {
    let link = Arc::new(
        runtime
            .block_on(a_link_to(hosting.clone(), || {}))
            .expect("the window speaks to the host"),
    );
    let handle = runtime.handle().clone();
    let dial: Dial =
        Arc::new(move |within| open_within(&handle, within, a_link_to(hosting.clone(), || {})));
    let connection = Arc::new(Connection::default());
    connection.ready(Arc::clone(&link), dial);
    (connection, link)
}

/// A `local-ui` link to `hosting` over an in-memory stream; `closed` is called once the host's
/// end of it ends.
async fn a_link_to<H: session::Host>(
    hosting: H,
    closed: impl FnOnce() + Send + 'static,
) -> Result<HostLink, String> {
    use purlis_session_protocol::auth::{Credentials, Scope};
    use purlis_session_protocol::link;

    static HELD: std::sync::LazyLock<Credentials> =
        std::sync::LazyLock::new(|| Credentials::mint().expect("credentials"));

    let (a, b) = tokio::io::duplex(1 << 20);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let served = served.map_err(|why| why.to_string())?;
    tokio::spawn(async move {
        let _ = session::serve(served, hosting, None).await;
        closed();
    });
    let client = client.map_err(|why| why.to_string())?;
    HostLink::over(client, tokio::runtime::Handle::current(), || {}).await
}

const SIZE: Size = Size {
    columns: 100,
    rows: 30,
};
const PATIENCE: Duration = Duration::from_secs(10);

struct Hosted {
    sessions: HostedSessions,
    link: Arc<HostLink>,
    runs: StandIns,
    savers: Savers,
    root: std::path::PathBuf,
    _dir: tempfile::TempDir,
    _runtime: tokio::runtime::Runtime,
}

fn a_project() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").unwrap();
    (dir, root)
}

/// A project's chats in a session host whose programs are stand-ins, over an in-memory link.
fn hosted() -> Hosted {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let runs = StandIns::default();
    let savers = Savers::default();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        runs.clone(),
    )
    .saving_with(savers.clone());
    let (connection, link) = in_memory(hosting, &runtime);
    let (dir, root) = a_project();
    Hosted {
        sessions: HostedSessions::new(connection, &root),
        link,
        runs,
        savers,
        root,
        _dir: dir,
        _runtime: runtime,
    }
}

fn a_shell(script: &str) -> Opening {
    Opening {
        program: Some("/bin/sh".to_owned()),
        args: vec!["-c".to_owned(), script.to_owned()],
        cwd: Some("/tmp".to_owned()),
        size: SIZE,
        env: vec![("PURLIS_ROOT".to_owned(), "/home/me/project".to_owned())],
        env_strip: Vec::new(),
        harness: None,
        declared_env: Vec::new(),
        env_pass: Vec::new(),
        operator_shell: false,
        git_hooks: None,
        handed: Default::default(),
        confinement: None,
    }
}

fn value(spec: &purlis_core::session::Spec, name: &str) -> Option<String> {
    spec.env
        .iter()
        .rev()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.to_string_lossy().into_owned())
}

fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// What a pane is sent, as it is sent.
fn a_pane() -> (Sink, Arc<Mutex<String>>) {
    let seen = Arc::new(Mutex::new(String::new()));
    let sink = {
        let seen = Arc::clone(&seen);
        Box::new(move |text: String| seen.lock().unwrap().push_str(&text)) as Sink
    };
    (sink, seen)
}

#[test]
fn two_clones_of_one_project_in_one_window_both_start_chats() {
    // #2178: one project id, two folders, one window's connection to its host.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let runs = StandIns::default();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        runs.clone(),
    );
    let (connection, _window) = in_memory(hosting, &runtime);
    let clone = || {
        let (dir, root) = a_project();
        std::fs::write(
            root.join(purlis_core::plane::MANIFEST),
            "[project]\nid = \"01J9PRJCT00000000000000000\"\n",
        )
        .unwrap();
        (dir, root)
    };
    let ((_one, first), (_two, second)) = (clone(), clone());
    let first = HostedSessions::new(Arc::clone(&connection), &first);
    let second = HostedSessions::new(connection, &second);

    let started = (
        first.open(None, &a_shell("true"), &|_| {}),
        second.open(None, &a_shell("true"), &|_| {}),
    );

    assert_eq!(started, (Ok(1), Ok(1)), "each clone deals its own numbers");
    assert_eq!(runs.ran().len(), 2);
    drop((first, second));
    drop(runtime);
}

#[test]
fn chats_run_in_the_host_unless_the_switch_is_set_to_zero() {
    // #2004: the session host is the default, and `0` is the way back to the window's own
    // process until #2005 deletes it.
    assert!(!switched_on(Some("0")));
    for on in [None, Some(""), Some("1"), Some("true"), Some("no")] {
        assert!(switched_on(on), "{on:?}");
    }
}

#[test]
fn a_chat_its_profile_hands_credentials_is_never_started_in_the_host() {
    // #2036: what a profile hands its chat is set on a program the window starts, and never
    // goes over the link.
    let (_dir, root) = a_project();
    let sessions = HostedSessions::new(Arc::new(Connection::default()), &root);
    let announced = Mutex::new(Vec::new());
    let handed = Opening {
        handed: purlis_core::profilesignin::Handed::of(vec![(
            "AWS_REGION".to_owned(),
            "a-region-for-this-test".to_owned(),
        )]),
        ..a_shell("true")
    };

    let refused = sessions
        .open(None, &handed, &|id| announced.lock().unwrap().push(id))
        .unwrap_err();

    assert_eq!(refused, crate::hostedsessions::NOT_HANDED_IN_THE_HOST);
    assert!(announced.lock().unwrap().is_empty());
}

#[test]
fn with_no_host_a_chat_is_refused_with_why_and_never_announced() {
    let (_dir, root) = a_project();
    let connection = Arc::new(Connection::default());
    connection.gone("another session host already serves this user on this device");
    let sessions = HostedSessions::new(connection, &root);
    let announced = Mutex::new(Vec::new());

    let refused = sessions
        .open(None, &a_shell("true"), &|id| {
            announced.lock().unwrap().push(id)
        })
        .unwrap_err();

    assert!(
        refused.contains("it is not running: another session host already serves"),
        "{refused}"
    );
    assert!(announced.lock().unwrap().is_empty());
    assert!(sessions.running().is_empty());
}

#[test]
fn a_start_hands_the_host_what_the_window_composed_and_the_host_composes_it_as_the_window_would() {
    let hosted = hosted();
    let announced = Mutex::new(Vec::new());

    let id = hosted
        .sessions
        .open(None, &a_shell("echo hi"), &|id| {
            announced.lock().unwrap().push(id)
        })
        .expect("it starts");

    assert_eq!(id, 1);
    assert_eq!(*announced.lock().unwrap(), [1]);
    let (spec, _) = &hosted.runs.ran()[0];
    assert_eq!(spec.program, "/bin/sh");
    assert_eq!(spec.args, ["-c", "echo hi"]);
    assert_eq!(spec.cwd.as_deref(), Some(std::path::Path::new("/tmp")));
    assert_eq!(spec.size, SIZE);
    assert!(spec.env_clear);
    assert_eq!(value(spec, "PURLIS_SESSION_ID").as_deref(), Some("1"));
    assert_eq!(
        value(spec, "PURLIS_ROOT").as_deref(),
        Some("/home/me/project")
    );
    let in_the_window = purlis_core::chatspec::spec(
        std::env::vars_os(),
        &purlis_core::chatspec::Terminal {
            program: "/bin/sh",
            args: &["-c".to_owned(), "echo hi".to_owned()],
            cwd: Some("/tmp"),
            size: SIZE,
            starting: purlis_core::chatenv::Starting {
                harness: None,
                operator: &[],
                strip: &[],
                set: &[("PURLIS_ROOT".to_owned(), "/home/me/project".to_owned())],
                git_hooks: None,
                declared: &[],
            },
            number: 1,
            reporting: None,
        },
    );
    assert_eq!(spec.env, in_the_window.env, "the same environment and PATH");
    assert_eq!(
        purlis_core::scaffold::planefile::project_id(&hosted.root),
        None,
        "holding the project wrote no id into it (#2004)"
    );
}

#[test]
fn a_chat_keeps_the_number_it_already_answers_to_and_numbers_are_dealt_past_it() {
    let hosted = hosted();

    let kept = hosted
        .sessions
        .open(Some(7), &a_shell("true"), &|_| {})
        .unwrap();
    let next = hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .unwrap();

    assert_eq!((kept, next), (7, 8));
    assert_eq!(hosted.sessions.running(), [7, 8]);
    assert_eq!(hosted.sessions.dealt(), 8);
    assert_eq!(hosted.sessions.process_id(7), Some(4242));
}

#[test]
fn a_pane_draws_the_screen_then_what_follows_and_is_told_when_the_program_ends() {
    let hosted = hosted();
    let ends = Arc::new(Mutex::new(Vec::new()));
    hosted.sessions.when_one_ends({
        let ends = Arc::clone(&ends);
        Arc::new(move |id, exit| ends.lock().unwrap().push((id, exit)))
    });
    let id = hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .unwrap();
    let program = hosted.runs.last();
    program.writes(b"ready> ");
    let (sink, seen) = a_pane();

    let watching = hosted.sessions.watch(id, sink).expect("a view opens");
    assert_eq!(watching.size, SIZE);
    until("the screen is drawn", || {
        seen.lock().unwrap().contains("ready> ")
    });
    hosted.sessions.input(id, b"hello").unwrap();
    until("the typing is drawn", || {
        seen.lock().unwrap().ends_with("ready> hello")
    });
    assert_eq!(program.seen().typed, b"hello");

    program.ends(Exit::Code(3));

    until("the pane is told", || seen.lock().unwrap().ends_with(ENDED));
    until("whoever asked is told", || !ends.lock().unwrap().is_empty());
    assert_eq!(
        *ends.lock().unwrap(),
        [(id, purlis_core::session::Exit::Code(3))]
    );
    assert_eq!(hosted.sessions.process_id(id), None);
    assert_eq!(hosted.sessions.running(), [id], "the tab stays");
}

#[test]
fn a_resize_and_readiness_go_to_the_host() {
    let hosted = hosted();
    let id = hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .unwrap();

    hosted
        .sessions
        .resize(
            id,
            Size {
                columns: 120,
                rows: 40,
            },
        )
        .unwrap();
    let readiness = hosted.sessions.readiness(id).unwrap();

    assert_eq!(
        hosted.runs.last().seen().sizes,
        [Size {
            columns: 120,
            rows: 40
        }]
    );
    assert_eq!(
        readiness,
        Readiness {
            edits_lines: Some(false),
            quiet_for: Duration::from_millis(1500),
        }
    );
}

#[test]
fn a_closed_chat_is_ended_in_the_host_and_forgotten() {
    let hosted = hosted();
    let id = hosted
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .unwrap();

    hosted.sessions.close(id).unwrap();

    assert!(hosted.sessions.running().is_empty());
    let program = hosted.runs.last();
    until("the host ends it", || program.seen().stopped);
    assert!(hosted.sessions.input(id, b"x").is_err());
}

#[test]
fn stopping_every_program_stops_each_in_the_host_and_keeps_its_tab() {
    let hosted = hosted();
    for _ in 0..3 {
        hosted
            .sessions
            .open(None, &a_shell("true"), &|_| {})
            .unwrap();
    }

    let asked = hosted.sessions.stop_every_program();

    assert_eq!(asked, 3);
    assert!(hosted.runs.ran().iter().all(|(_, run)| run.seen().stopped));
    assert_eq!(hosted.sessions.running(), [1, 2, 3]);
}

#[test]
fn ending_every_chat_closes_each_in_the_host() {
    let hosted = hosted();
    for _ in 0..2 {
        hosted
            .sessions
            .open(None, &a_shell("true"), &|_| {})
            .unwrap();
    }

    hosted.sessions.end_all();

    assert!(hosted.sessions.running().is_empty());
    assert!(hosted.runs.ran().iter().all(|(_, run)| run.seen().stopped));
}

#[test]
fn a_closed_chats_end_does_not_end_the_chat_put_back_under_its_number() {
    let hosted = hosted();
    let ends = Arc::new(Mutex::new(Vec::new()));
    hosted.sessions.when_one_ends({
        let ends = Arc::clone(&ends);
        Arc::new(move |id, exit| ends.lock().unwrap().push((id, exit)))
    });
    hosted.runs.hold_on();
    hosted
        .sessions
        .open(Some(5), &a_shell("true"), &|_| {})
        .unwrap();
    let closed = hosted.runs.last();
    hosted.sessions.close(5).unwrap();

    let again = hosted
        .sessions
        .open(Some(5), &a_shell("true"), &|_| {})
        .unwrap();
    closed.ends(Exit::Code(0));
    std::thread::sleep(Duration::from_millis(200));

    assert_eq!(again, 5);
    assert_eq!(
        hosted.sessions.process_id(5),
        Some(4242),
        "the new one runs on"
    );
    assert!(
        ends.lock().unwrap().is_empty(),
        "{:?}",
        ends.lock().unwrap()
    );
}

#[test]
fn a_start_the_host_numbers_otherwise_fails_and_is_closed() {
    let hosted = hosted();
    let (link, project) = hosted.sessions.link().unwrap();
    // A program of the project's that the window does not know already has number 5.
    link.call(Command::StartProgram(Box::new(StartProgram {
        number: Some(5),
        launch: Some(launch_of(&a_shell("true"), "/bin/sh".to_owned())),
        ..StartProgram::of(project, ProgramKind::Shell)
    })))
    .unwrap();

    let refused = hosted
        .sessions
        .open(Some(5), &a_shell("true"), &|_| {})
        .unwrap_err();

    assert!(refused.contains("not 5"), "{refused}");
    assert!(hosted.sessions.running().is_empty());
    let started = hosted.runs.last();
    until("the host closes it", || started.seen().stopped);
    drop(hosted.link);
}

/// A project's hook channel standing in for the host's (#2000): what it hands on goes over the
/// link exactly as the real one's does, and a test hears as its listener would through
/// [`HooksHeard::hearing`], each line taken as admitted.
#[derive(Clone, Default)]
struct HooksHeard {
    relaying: Arc<Mutex<Option<Arc<purlis_core::hookwire::relay::Relaying>>>>,
    issued: Arc<Mutex<Vec<u32>>>,
    /// The project's board, kept as the window's binary keeps it in the host (#2194).
    board: crate::sessionhost::KeptBoard,
}

impl HooksHeard {
    /// What the project's listener would hear an admitted line with.
    fn hearing(&self) -> purlis_core::hookwire::Hearing {
        self.relaying
            .lock()
            .unwrap()
            .clone()
            .expect("the project is held")
            .hearing(Arc::new(purlis_core::hookwire::ChatTokens::default()))
    }
}

impl purlis_host::Hear for HooksHeard {
    fn open(
        &self,
        _project: &str,
        _path: &std::path::Path,
        hand_on: purlis_host::HandOn,
    ) -> Result<Option<Arc<dyn purlis_host::Channel>>, String> {
        let relaying = purlis_core::hookwire::relay::Relaying::new(self.board.handing_on(hand_on));
        *self.relaying.lock().unwrap() = Some(Arc::clone(&relaying));
        Ok(Some(Arc::new(StandInChannel {
            relaying,
            issued: Arc::clone(&self.issued),
            board: self.board.clone(),
        })))
    }
}

struct StandInChannel {
    relaying: Arc<purlis_core::hookwire::relay::Relaying>,
    issued: Arc<Mutex<Vec<u32>>>,
    board: crate::sessionhost::KeptBoard,
}

impl purlis_host::Channel for StandInChannel {
    fn report_to(&self, number: u32) -> Result<session::Reporting, String> {
        self.issued.lock().unwrap().push(number);
        Ok(session::Reporting {
            socket: "/the/project/hooks.sock".into(),
            token: format!("issued-by-the-host-{number}"),
        })
    }

    fn bind(&self, _number: u32, _pid: u32) {}

    fn ended(&self, _number: u32, _pid: u32) {}

    fn forget(&self, _number: u32) {}

    fn replied(&self, id: u64, reply: serde_json::Value) {
        self.relaying
            .replied(id, serde_json::from_value(reply).expect("a reply"));
    }

    fn moved(&self, number: u32, moved: serde_json::Value) {
        self.board.moved(number, moved);
    }

    fn board(&self) -> Result<serde_json::Value, String> {
        self.board.whole()
    }

    fn ended_with_no_window(&self, number: u32, exit: &session::Exit) {
        self.board.ended(number, exit);
    }
}

/// A project's chats in a session host that owns its hook channel, heard by the window's own
/// hooks: what each chat's board says, as the window is told.
struct Heard {
    hosted: Hosted,
    hooks: crate::hooks::Hooks,
    channel: HooksHeard,
    moved: Arc<Mutex<Vec<crate::hooks::Moved>>>,
    hosting: Hosting,
}

fn heard_in_the_window() -> Heard {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let runs = StandIns::default();
    let channel = HooksHeard::default();
    let hosting = Hosting::hearing_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        runs.clone(),
        channel.clone(),
    );
    let (dir, root) = a_project();
    let (link, hooks, sessions, moved) = a_window_of(&hosting, &root, &runtime);
    Heard {
        hosted: Hosted {
            sessions,
            link,
            runs,
            savers: Savers::default(),
            root,
            _dir: dir,
            _runtime: runtime,
        },
        hooks,
        channel,
        moved,
        hosting,
    }
}

/// A window on `hosting`, over a link of its own, whose hooks for the project at `root` hear
/// what the host hands on.
fn a_window_of(
    hosting: &Hosting,
    root: &std::path::Path,
    runtime: &tokio::runtime::Runtime,
) -> (
    Arc<HostLink>,
    crate::hooks::Hooks,
    HostedSessions,
    Arc<Mutex<Vec<crate::hooks::Moved>>>,
) {
    let (connection, link) = in_memory(hosting.clone(), runtime);
    let moved: Arc<Mutex<Vec<crate::hooks::Moved>>> = Arc::default();
    let plane: crate::planes::PlaneId =
        serde_json::from_str(&serde_json::to_string(root).unwrap()).unwrap();
    let hooks = crate::hooks::Hooks::relayed(
        plane,
        &crate::hooks::socket_for(Some(root)),
        {
            let moved = Arc::clone(&moved);
            Arc::new(move |what| moved.lock().unwrap().push(what))
        },
        Arc::new(|_| {}),
    );
    let sessions = HostedSessions::new(connection, root);
    sessions.hear_through(hooks.relayed_from_the_host().expect("relayed"));
    (link, hooks, sessions, moved)
}

impl Heard {
    /// A chat started in the host, announced to the window's board as the window does.
    fn a_chat(&self) -> u32 {
        self.hosted
            .sessions
            .open(None, &a_shell("true"), &|id| {
                self.hooks.board().opened(id, None, None);
            })
            .expect("it starts")
    }
}

fn reported(chat: u32, event: purlis_core::state::Event) -> purlis_core::hookwire::Report {
    purlis_core::hookwire::Report {
        chat,
        event,
        conversation: purlis_core::hookwire::Conversation::Unknown,
        pid: None,
        agent: None,
        detail: purlis_core::state::Detail::default(),
    }
}

#[test]
fn the_windows_launch_never_says_where_a_chat_reports() {
    let launch = launch_of(&a_shell("true"), "/bin/sh".to_owned());

    assert_eq!(launch.reporting, None);
}

#[test]
fn the_host_issues_a_chat_its_token_and_the_window_never_holds_one() {
    let heard = heard_in_the_window();

    let id = heard.a_chat();

    assert_eq!(*heard.channel.issued.lock().unwrap(), [id]);
    let (spec, _) = &heard.hosted.runs.ran()[0];
    assert_eq!(
        value(spec, purlis_core::hookwire::TOKEN_ENV).as_deref(),
        Some(format!("issued-by-the-host-{id}").as_str())
    );
    assert_eq!(
        value(spec, purlis_core::hookwire::SOCKET_ENV).as_deref(),
        Some("/the/project/hooks.sock")
    );
    assert_eq!(
        heard.hosted.sessions.reports_to(),
        Some(crate::hooks::socket_for(Some(&heard.hosted.root)).socket),
        "the chat's sandbox lets it reach the socket the host binds"
    );
}

#[test]
fn a_report_the_host_hears_moves_the_windows_board_and_its_needs_you() {
    let heard = heard_in_the_window();
    let id = heard.a_chat();

    let taken = (heard.channel.hearing().each)(reported(id, purlis_core::state::Event::Stop));

    assert!(taken.is_ok(), "{taken:?}");
    let moved = heard.moved.lock().unwrap().clone();
    let last = moved.last().expect("the window is told");
    assert_eq!((last.session, last.needs_you), (id, true));
    assert_eq!(last.queue, [id]);
}

#[test]
fn a_permission_ask_the_host_hears_is_held_in_the_window_and_answered_there() {
    let heard = heard_in_the_window();
    let id = heard.a_chat();

    let waiting = (heard.channel.hearing().permission)(purlis_core::hookwire::PermissionAsked {
        chat: id,
        permission_request: purlis_core::harness::hooked::Source::ClaudeCode,
        payload: serde_json::json!({"tool_name": "Bash",
            "tool_input": {"command": "npm test"}}),
        ran_for_ms: None,
    })
    .expect("the window holds it");
    let held = heard.hooks.asks().pending(Instant::now());
    let ask = held.first().expect("the window lists it");
    let allow = ask
        .ask
        .options
        .iter()
        .find(|option| option.kind == purlis_core::harness::model::ChoiceKind::Allow)
        .expect("an option to allow it")
        .clone();
    heard
        .hooks
        .answer(id, &ask.id.to_string(), &allow.id)
        .expect("answered in the window");

    assert_eq!(
        waiting
            .answered
            .recv_timeout(PATIENCE)
            .map(|chosen| chosen.id),
        Ok(allow.id)
    );
}

fn told_nothing() -> crate::autosave::Told {
    Arc::new(|_| {})
}

#[test]
fn the_project_is_saved_in_the_host_and_the_window_answers_who_is_mid_turn_from_its_board() {
    let hosted = hosted();
    let saving = hosted.sessions.keep_saving(&hosted.root, told_nothing());
    until("the host saves the project", || {
        hosted.savers.running() == 1
    });
    saving.asks_mid_turn_of(Arc::new(|workspace: &str| vec![format!("{workspace} 1")]));

    assert_eq!(
        hosted.savers.asks_the_window("alpha"),
        Some(vec!["alpha 1".to_owned()]),
        "the host did not hear the window's board"
    );
    assert_eq!(hosted.savers.started()[0].1, hosted.root);
}

#[test]
fn a_quit_that_the_host_does_not_answer_gives_up_on_it_by_the_deadline() {
    let hosted = hosted();
    let saving = hosted.sessions.keep_saving(&hosted.root, told_nothing());
    until("the host saves the project", || {
        hosted.savers.running() == 1
    });
    // Its save never lets go: the host never answers the stop.
    hosted.savers.mid_save();

    let started = Instant::now();
    let stopped = saving.stop_by(started + Duration::from_millis(500));

    assert!(!stopped, "a stop the host never answered was taken as done");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "the quit waited {:?} for a host that does not answer",
        started.elapsed()
    );
    hosted.savers.finish_saving();
}

/// The board the host keeps, as a window takes it.
fn the_hosts_board(heard: &Heard) -> purlis_core::state::Board {
    let mut taken = purlis_core::state::Board::new();
    taken.keep(
        serde_json::from_value(
            heard
                .channel
                .board
                .whole()
                .expect("a board within an answer"),
        )
        .expect("a whole board"),
    );
    taken
}

#[test]
fn each_move_the_window_makes_on_its_board_is_made_on_the_hosts() {
    use crate::host::ChatBoard;
    use purlis_core::state::{Event, State};
    let heard = heard_in_the_window();
    let id = heard.a_chat();
    let other = heard.a_chat();

    (heard.channel.hearing().each)(reported(id, Event::Stop)).expect("taken");
    (heard.channel.hearing().each)(reported(other, Event::Stop)).expect("taken");
    heard.hooks.ignored(other);
    heard.hooks.task_failed(
        id,
        purlis_core::state::FailedTask::new(
            "task-1",
            "the task",
            purlis_core::state::HowFailed::Failed,
            "it broke",
        ),
    );

    until("the host's board follows the window's", || {
        the_hosts_board(&heard).failed_tasks(id).len() == 1
    });
    let host = the_hosts_board(&heard);
    assert_eq!(host.needs_you(), [id]);
    assert_eq!(host.needs_you(), heard.hooks.board().needs_you());
    assert_eq!(host.state(other), State::Waiting);

    heard.hooks.closed(other);
    until("the close reaches the host's board", || {
        the_hosts_board(&heard).chats().count() == 1
    });
}

#[test]
fn a_window_that_holds_the_project_again_reads_every_chat_as_the_host_kept_it() {
    use purlis_core::state::{Event, State};
    let heard = heard_in_the_window();
    let id = heard.a_chat();
    (heard.channel.hearing().each)(reported(id, Event::Stop)).expect("taken");
    until("the host's board has the chat's end of turn", || {
        the_hosts_board(&heard).needs_you() == [id]
    });

    let (_link, hooks, sessions, moved) =
        a_window_of(&heard.hosting, &heard.hosted.root, &heard.hosted._runtime);
    sessions.hold().expect("it holds the project");

    assert_eq!(hooks.board().state(id), State::Waiting);
    assert_eq!(hooks.board().needs_you(), [id]);
    let told = moved.lock().unwrap().clone();
    let told = told.last().expect("the window is told the chat");
    assert_eq!((told.session, told.needs_you), (id, true));
    assert_eq!(told.queue, [id]);
}

#[test]
fn a_host_before_held_trust_events_is_sent_the_trust_event_first_and_no_start_taken_back() {
    // #2252: a window and a host one version apart still record the sandbox coming back on,
    // before the start's program runs, as every start's was.
    use purlis_core::eventlog::{Attributes, Began, Entry, HeldTrust};
    use purlis_session_protocol::version::Version;
    let trust = HeldTrust::of(
        &purlis_core::sandbox::Change::On,
        Some(purlis_core::harness::Harness::ClaudeCode),
        None,
    );
    let begin = |trust: Option<HeldTrust>| Entry::Begin {
        plane: "/planes/one".into(),
        number: 3,
        chat: "01K6E8ZK6V4Q9T0N3M2B1C5D7E".into(),
        run: "01K6E8ZK6V4Q9T0N3M2B1C5D7F".into(),
        cause: Began::Start,
        attributes: Attributes::default(),
        held: true,
        trust: trust.map(Box::new),
    };
    let taken_back = Entry::TakenBack {
        plane: "/planes/one".into(),
        number: 3,
    };
    let older = Version { major: 1, minor: 8 };

    assert_eq!(
        as_the_host_reads(&begin(Some(trust.clone())), older),
        [
            Entry::Trust {
                chat: "01K6E8ZK6V4Q9T0N3M2B1C5D7E".into(),
                run: "01K6E8ZK6V4Q9T0N3M2B1C5D7F".into(),
                kind: "trust.sandbox.on".into(),
                body: trust.body().clone(),
            },
            begin(None),
        ]
    );
    assert!(as_the_host_reads(&taken_back, older).is_empty());
    assert_eq!(
        as_the_host_reads(&begin(Some(trust.clone())), session::VERSION),
        [begin(Some(trust))]
    );
    assert_eq!(
        as_the_host_reads(&taken_back, session::VERSION),
        [taken_back]
    );
}

// ----- a host killed outright, and the one that replaces it (#2007) --------------------------

/// One session host on in-memory links, whose serving of every link can be ended as a kill
/// ends it.
struct OneHost {
    link: Arc<HostLink>,
    dial: Dial,
    runs: StandIns,
    savers: Savers,
    serving: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
}

impl OneHost {
    /// The host is killed outright: it serves nothing more, and its end of every link is gone.
    fn killed(&self) {
        for serving in self.serving.lock().unwrap().drain(..) {
            serving.abort();
        }
    }
}

/// A `local-ui` link to `hosting` over an in-memory stream, whose serving is kept in `serving`.
async fn a_served_link(
    hosting: Hosting,
    serving: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
) -> Result<HostLink, String> {
    use purlis_session_protocol::auth::{Credentials, Scope};
    use purlis_session_protocol::link;
    static HELD: std::sync::LazyLock<Credentials> =
        std::sync::LazyLock::new(|| Credentials::mint().expect("credentials"));
    let (a, b) = tokio::io::duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    let served = served.map_err(|why| why.to_string())?;
    let task = tokio::spawn(async move {
        let _ = session::serve(served, hosting, None).await;
    });
    serving.lock().unwrap().push(task.abort_handle());
    let client = client.map_err(|why| why.to_string())?;
    HostLink::over(client, tokio::runtime::Handle::current(), || {}).await
}

/// A host started on `runtime`, as the window's supervisor starts one: the window's link to
/// it, and one more for each project root.
fn a_host_on(runtime: &tokio::runtime::Runtime) -> OneHost {
    let runs = StandIns::default();
    let savers = Savers::default();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        runs.clone(),
    )
    .saving_with(savers.clone());
    let serving: Arc<Mutex<Vec<tokio::task::AbortHandle>>> = Arc::default();
    let link = Arc::new(
        runtime
            .block_on(a_served_link(hosting.clone(), Arc::clone(&serving)))
            .unwrap(),
    );
    let dial: Dial = {
        let (handle, serving) = (runtime.handle().clone(), Arc::clone(&serving));
        Arc::new(move |within| {
            open_within(
                &handle,
                within,
                a_served_link(hosting.clone(), Arc::clone(&serving)),
            )
        })
    };
    OneHost {
        link,
        dial,
        runs,
        savers,
        serving,
    }
}

/// A project's chats in a host that is killed and replaced as the test says.
struct Supervised {
    sessions: HostedSessions,
    connection: Arc<Connection>,
    first: OneHost,
    root: std::path::PathBuf,
    runtime: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

impl Supervised {
    /// The host that replaces the first, once the window has seen it go.
    fn replaced(&self) -> OneHost {
        let next = a_host_on(&self.runtime);
        self.connection
            .ready(Arc::clone(&next.link), Arc::clone(&next.dial));
        next
    }
}

fn supervised() -> Supervised {
    let runtime = a_runtime();
    let first = a_host_on(&runtime);
    let connection = Arc::new(Connection::default());
    connection.ready(Arc::clone(&first.link), Arc::clone(&first.dial));
    let (dir, root) = a_project();
    Supervised {
        sessions: HostedSessions::new(Arc::clone(&connection), &root),
        connection,
        first,
        root,
        runtime,
        _dir: dir,
    }
}

#[test]
fn every_chat_in_a_host_killed_outright_ends_with_it_and_says_so() {
    let host = supervised();
    let ends = Arc::new(Mutex::new(Vec::new()));
    host.sessions.when_one_ends({
        let ends = Arc::clone(&ends);
        Arc::new(move |id, _exit| ends.lock().unwrap().push(id))
    });
    let chats: Vec<u32> = (0..20)
        .map(|_| {
            host.sessions
                .open(None, &a_shell("true"), &|_| {})
                .expect("it starts")
        })
        .collect();
    let (sink, pane) = a_pane();
    host.sessions.watch(chats[0], sink).expect("a view opens");
    let killed = Instant::now();

    host.first.killed();

    until("every chat is told it ended", || {
        ends.lock().unwrap().len() == chats.len()
    });
    assert!(killed.elapsed() < Duration::from_secs(30));
    let mut ended = ends.lock().unwrap().clone();
    ended.sort_unstable();
    assert_eq!(ended, chats, "each chat once");
    for chat in &chats {
        assert!(
            host.sessions.lost_with_its_host(*chat),
            "chat {chat} did not end with its host"
        );
        assert_eq!(host.sessions.process_id(*chat), None);
    }
    until("its pane says it ended", || {
        pane.lock().unwrap().ends_with(ENDED)
    });
    assert_eq!(host.sessions.running(), chats, "every tab stays");
}

#[test]
fn a_chat_that_ended_by_itself_did_not_end_with_its_host() {
    let host = supervised();
    let id = host
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .expect("it starts");
    host.first.runs.last().ends(Exit::Code(0));
    until("its end is heard", || {
        host.sessions.process_id(id).is_none()
    });

    host.first.killed();
    std::thread::sleep(Duration::from_millis(100));

    assert!(!host.sessions.lost_with_its_host(id));
}

#[test]
fn a_chat_lost_with_its_host_starts_again_in_the_host_that_replaces_it() {
    let host = supervised();
    let id = host
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .expect("it starts");
    host.first.killed();
    until("it ended with its host", || {
        host.sessions.lost_with_its_host(id)
    });
    host.connection.restarting();
    let next = host.replaced();

    // As Resume does: a new run of the chat starts, and then the one that was lost closes.
    let again = host
        .sessions
        .open(None, &a_shell("true"), &|_| {})
        .expect("it starts again");
    host.sessions.close(id).expect("the lost one closes");

    assert_eq!(next.runs.ran().len(), 1, "in the new host");
    assert_eq!(host.sessions.running(), [again]);
    assert!(!host.sessions.lost_with_its_host(again));
    assert!(!host.sessions.lost_with_its_host(id), "kept past its close");
    assert_eq!(host.sessions.process_id(again), Some(4242));
}

#[test]
fn a_start_while_the_host_is_started_again_waits_for_the_new_one() {
    let host = supervised();
    host.first.killed();
    host.connection.restarting();

    let starting = std::thread::scope(|scope| {
        let starting = scope.spawn(|| host.sessions.open(None, &a_shell("true"), &|_| {}));
        std::thread::sleep(Duration::from_millis(200));
        let next = host.replaced();
        (starting.join().unwrap(), next)
    });

    let (started, next) = starting;
    assert!(started.is_ok(), "{started:?}");
    assert_eq!(next.runs.ran().len(), 1);
}

#[test]
fn a_project_the_host_saved_is_saved_by_the_host_that_replaces_it() {
    let host = supervised();
    let _saving = host.sessions.keep_saving(&host.root, told_nothing());
    until("the first host saves the project", || {
        host.first.savers.running() == 1
    });

    host.first.killed();
    host.connection.restarting();
    let next = host.replaced();

    until("the new host saves the project", || {
        next.savers.running() == 1
    });
    assert_eq!(next.savers.started()[0].1, host.root);
}

fn a_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn a_call_that_cannot_wait_never_waits_behind_another_roots_link_being_opened() {
    let runtime = a_runtime();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        StandIns::default(),
    );
    let (connection, window) = in_memory(hosting, &runtime);
    let ((_one, open), (_two, opening)) = (a_project(), a_project());
    connection
        .of_project(&open, PATIENCE)
        .expect("the first root's link opens");
    // From now on another link opens only once it is let go.
    let (entered, dialed) = std::sync::mpsc::channel();
    let (let_go, go) = std::sync::mpsc::channel::<()>();
    let go = Mutex::new(go);
    connection.ready(
        window,
        Arc::new(move |within| {
            let _ = entered.send(());
            let _ = go.lock().unwrap().recv_timeout(within);
            Err("held up".to_owned())
        }),
    );
    let dialer = {
        let (connection, opening) = (Arc::clone(&connection), opening.clone());
        std::thread::spawn(move || connection.of_project(&opening, PATIENCE).map(|_| ()))
    };
    dialed
        .recv_timeout(PATIENCE)
        .expect("the second root's link is being opened");

    let asked = Instant::now();
    let already = connection.of_project(&open, Duration::ZERO);
    let not_yet = connection.of_project(&opening, Duration::ZERO);
    let waited = asked.elapsed();

    let _ = let_go.send(());
    assert!(dialer.join().unwrap().is_err());
    assert!(already.is_ok(), "the open link was not answered");
    assert!(not_yet.is_err(), "a link still opening was answered");
    assert!(
        waited < Duration::from_secs(1),
        "it waited {waited:?} behind another root's link"
    );
}

#[test]
fn a_link_that_opens_after_nobody_waits_for_it_is_closed() {
    let runtime = a_runtime();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        StandIns::default(),
    );
    let (closed, ends) = std::sync::mpsc::channel();

    let opened = open_within(runtime.handle(), Duration::from_millis(50), async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        a_link_to(hosting, move || {
            let _ = closed.send(());
        })
        .await
    });

    assert!(opened.is_err(), "the link opened in time");
    ends.recv_timeout(PATIENCE)
        .expect("the link that opened late is still open");
}

// ---------------------------------------------------------------------------------------
// a window that comes back to a host that outlived it (#2006)
// ---------------------------------------------------------------------------------------

/// One host whose programs and confinements are stand-ins, and windows that come and go on it,
/// each over a link a test cuts as a crash of the window would.
struct Comeback {
    hosting: Hosting,
    runs: StandIns,
    confines: purlis_host::standin::StandInConfines,
    root: std::path::PathBuf,
    runtime: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

/// A window on [`Comeback`]'s host: its chats, and what cuts each of its links.
struct AWindow {
    sessions: HostedSessions,
    cuts: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
}

impl Comeback {
    fn new() -> Comeback {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let runs = StandIns::default();
        let confines = purlis_host::standin::StandInConfines::default();
        let (dir, root) = a_project();
        Comeback {
            hosting: Hosting::confining(
                crate::sessionhost::Composes::as_the_tests_ship(),
                confines.clone(),
                runs.clone(),
            ),
            runs,
            confines,
            root,
            runtime,
            _dir: dir,
        }
    }

    fn window(&self) -> AWindow {
        let cuts: Arc<Mutex<Vec<tokio::task::AbortHandle>>> = Arc::default();
        let link = self
            .runtime
            .block_on(crate::behaviour::a_cuttable_link(
                self.hosting.clone(),
                Arc::clone(&cuts),
            ))
            .unwrap();
        let dial: Dial = {
            let (hosting, cuts, handle) = (
                self.hosting.clone(),
                Arc::clone(&cuts),
                self.runtime.handle().clone(),
            );
            Arc::new(move |within| {
                open_within(
                    &handle,
                    within,
                    crate::behaviour::a_cuttable_link(hosting.clone(), Arc::clone(&cuts)),
                )
            })
        };
        let connection = Arc::new(Connection::default());
        connection.ready(Arc::new(link), dial);
        AWindow {
            sessions: HostedSessions::new(connection, &self.root),
            cuts,
        }
    }
}

impl AWindow {
    /// The window crashes: every link of its is cut, and nothing of it says goodbye.
    fn crashes(self) {
        for cut in self.cuts.lock().unwrap().iter() {
            cut.abort();
        }
        std::mem::forget(self.sessions);
    }
}

#[test]
fn a_chat_left_running_is_reattached_under_its_number_and_drawn_as_it_is_now() {
    let host = Comeback::new();
    let first = host.window();
    first
        .sessions
        .open(Some(3), &a_shell("true"), &|_| {})
        .unwrap();
    let program = host.runs.last();
    program.writes(b"before. ");
    first.crashes();
    program.writes(b"while away. ");

    let back = host.window();
    assert!(back.sessions.left_running(3));
    assert!(!back.sessions.left_running(4));
    let announced = Mutex::new(Vec::new());
    let id = back
        .sessions
        .reattach(3, &|id| announced.lock().unwrap().push(id))
        .expect("it is reattached");

    assert_eq!(id, 3);
    assert_eq!(*announced.lock().unwrap(), [3]);
    assert_eq!(back.sessions.running(), [3]);
    assert_eq!(back.sessions.process_id(3), Some(4242));
    assert!(!back.sessions.left_running(3), "reattached once");
    let (sink, seen) = a_pane();
    back.sessions.watch(3, sink).expect("a view opens");
    until("the screen as it is now is drawn", || {
        seen.lock().unwrap().contains("before. while away. ")
    });
    back.sessions.input(3, b"typed").unwrap();
    until("the typing reaches it", || program.seen().typed == b"typed");
    assert_eq!(host.runs.ran().len(), 1, "nothing was started again");
}

#[test]
fn a_chat_that_ended_with_no_window_there_is_reattached_and_told_ended() {
    let host = Comeback::new();
    let first = host.window();
    first
        .sessions
        .open(Some(2), &a_shell("true"), &|_| {})
        .unwrap();
    let program = host.runs.last();
    first.crashes();
    program.ends(Exit::Code(3));

    let back = host.window();
    let ends = Arc::new(Mutex::new(Vec::new()));
    back.sessions.when_one_ends({
        let ends = Arc::clone(&ends);
        Arc::new(move |id, exit| ends.lock().unwrap().push((id, exit)))
    });
    assert!(back.sessions.left_running(2));
    back.sessions
        .reattach(2, &|_| {})
        .expect("it is reattached");

    until("it is told ended", || !ends.lock().unwrap().is_empty());
    assert_eq!(
        *ends.lock().unwrap(),
        [(2, purlis_core::session::Exit::Code(3))]
    );
    assert_eq!(back.sessions.process_id(2), None);
    assert_eq!(back.sessions.running(), [2], "its tab stays");
}

#[test]
fn what_no_put_back_reattached_is_closed_and_what_it_did_runs_on() {
    let host = Comeback::new();
    let first = host.window();
    for number in [1, 2] {
        first
            .sessions
            .open(Some(number), &a_shell("true"), &|_| {})
            .unwrap();
    }
    let (kept, left) = (host.runs.ran()[0].1.clone(), host.runs.ran()[1].1.clone());
    first.crashes();

    let back = host.window();
    back.sessions
        .reattach(1, &|_| {})
        .expect("it is reattached");
    back.sessions.close_what_was_not_reattached();

    until("the one nothing reattached is ended", || {
        left.seen().stopped
    });
    assert!(!kept.seen().stopped);
    assert!(!back.sessions.left_running(2));
    assert_eq!(back.sessions.running(), [1]);
}

#[test]
fn a_chats_confinement_is_reattached_and_its_proxy_heard_by_the_window_that_came_back() {
    use purlis_core::sandbox::egress::{PLAIN_PORTS, Refusals, TUNNEL_PORTS};
    use purlis_core::sandbox::plan::{Beside, Hearing, Plan};
    use purlis_core::sandbox::reach::Reach;
    let refused_by = |heard: &Arc<Mutex<Vec<String>>>| Hearing {
        refusals: Refusals::telling({
            let heard = Arc::clone(heard);
            Arc::new(move |host: &str, port: u16| {
                heard.lock().unwrap().push(format!("{host}:{port}"));
            })
        }),
        more: None,
        reached: None,
        asks: None,
    };
    let plan = Plan {
        reach: Reach::of(Vec::new()).written(),
        tunnel_ports: TUNNEL_PORTS.to_vec(),
        plain_ports: PLAIN_PORTS.to_vec(),
        any_port: false,
        beside: Beside::Wrapped,
        given: Vec::new(),
    };
    let host = Comeback::new();
    let first = host.window();
    let heard_first = Arc::new(Mutex::new(Vec::new()));
    let confined = first
        .sessions
        .confine(&plan, refused_by(&heard_first))
        .expect("confined");
    let id = confined.held_as().expect("held in the host").to_owned();
    first
        .sessions
        .open(
            Some(4),
            &Opening {
                confinement: Some(id.clone()),
                ..a_shell("true")
            },
            &|_| {},
        )
        .unwrap();
    std::mem::forget(confined);
    first.crashes();

    let back = host.window();
    let heard_back = Arc::new(Mutex::new(Vec::new()));
    let again = back
        .sessions
        .confined_again(4, refused_by(&heard_back))
        .expect("it runs in one")
        .expect("it is reattached");
    host.confines.proxy_did(
        &id,
        purlis_session_protocol::session::SandboxTold::Refused {
            host: "tracker.example.com".into(),
            port: 443,
        },
    );

    assert_eq!(again.held_as(), Some(id.as_str()));
    until("the window that came back hears its proxy", || {
        !heard_back.lock().unwrap().is_empty()
    });
    assert_eq!(*heard_back.lock().unwrap(), ["tracker.example.com:443"]);
    assert!(!host.confines.ended(&id), "it runs on");
    assert!(
        back.sessions
            .confined_again(5, refused_by(&heard_back))
            .is_none()
    );
}

/// A window that comes back puts its record back as a launch does (#2006): each chat the host
/// still runs is reattached under its number and none is started again, a chat the host no
/// longer has is started, and what the host runs that the record does not name is closed.
#[test]
fn a_record_put_back_reattaches_what_the_host_still_runs_and_starts_the_rest() {
    let host = Comeback::new();
    let first = host.window();
    for number in [1, 2] {
        first
            .sessions
            .open(Some(number), &a_shell("true"), &|_| {})
            .unwrap();
    }
    let (kept, left) = (host.runs.ran()[0].1.clone(), host.runs.ran()[1].1.clone());
    first.crashes();
    let back = host.window();
    let chats =
        crate::chats::Chats::on_host(Box::new(|_| {}), Box::new(back.sessions), host.root.clone());
    let chat = |number: u32| purlis_core::reopen::Chat {
        program: "/bin/sh".to_owned(),
        args: vec!["-c".to_owned(), "true".to_owned()],
        name: format!("chat {number}"),
        number: Some(number),
        identity: purlis_core::reopen::Identity {
            id: Some(format!("01K6E8ZK6V4Q9T0N3M2B1C5D{number:02}")),
            ..Default::default()
        },
        ..Default::default()
    };
    let record = purlis_core::reopen::Record {
        chats: vec![chat(1), chat(3)],
        ..Default::default()
    };

    let open = chats.put_back(&record, SIZE);
    chats.close_what_was_not_reattached();

    let mut sessions: Vec<u32> = open.iter().map(|one| one.session).collect();
    sessions.sort_unstable();
    assert_eq!(sessions, [1, 3]);
    assert_eq!(host.runs.ran().len(), 3, "only chat 3 was started");
    until("the one the record does not name is ended", || {
        left.seen().stopped
    });
    assert!(!kept.seen().stopped, "the reattached one runs on");
}
