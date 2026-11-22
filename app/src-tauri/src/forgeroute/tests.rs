// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use purlis_core::forge::proxy::{Kind, NoCredentials, RequestHead, TokenKind};
use purlis_core::secrets::keyring::Secret;

use super::*;

const GITHUB_TOKEN: &str = "REAL-github-token-7731";
const GITLAB_TOKEN: &str = "REAL-gitlab-token-5520";

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- fakes: each stands where a slice that has not merged will ----

/// A listener that makes a socket and a CA file in the run's folder, as A2's will, keeps the
/// handler it was given (the one door a request comes in by), and counts live guards.
#[derive(Default)]
struct FakeListener {
    fail: AtomicBool,
    alive: Arc<AtomicUsize>,
    proxies: Mutex<HashMap<String, Arc<ForgeProxy>>>,
}

struct Alive(Arc<AtomicUsize>);
impl Drop for Alive {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl FakeListener {
    fn proxy_of(&self, run: &str) -> Arc<ForgeProxy> {
        lock(&self.proxies).get(run).expect("a bound run").clone()
    }
}

impl Listener for FakeListener {
    fn bind(
        &self,
        run: &str,
        dir: &Path,
        proxy: Arc<ForgeProxy>,
        _push: Arc<dyn PushGate>,
    ) -> io::Result<Bound> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(io::Error::other("cannot bind"));
        }
        let hosts = proxy.terminable_hosts(&ChatId::new(run));
        assert!(
            !hosts.is_empty(),
            "the listener is told the hosts the CA may name"
        );
        // A real socket, as A2's will be: a path too long for one fails here as it would there.
        #[cfg(unix)]
        let bound = std::os::unix::net::UnixListener::bind(dir.join(SOCKET_NAME))?;
        #[cfg(not(unix))]
        let bound = std::fs::File::create(dir.join(SOCKET_NAME))?;
        std::fs::write(dir.join("ca.pem"), b"PEM")?;
        lock(&self.proxies).insert(run.to_owned(), proxy);
        self.alive.fetch_add(1, Ordering::SeqCst);
        Ok(Bound {
            socket: dir.join(SOCKET_NAME),
            egress_url: "http://127.0.0.1:9/".to_owned(),
            ca: dir.join("ca.pem"),
            door: std::net::SocketAddr::from(([127, 0, 0, 1], 9)),
            handoff: purlis_core::sandbox::egress::handoff::Secret::random()?,
            hosts,
            guard: Box::new((Alive(self.alive.clone()), bound)),
        })
    }
    fn ready(&self) -> bool {
        !self.fail.load(Ordering::SeqCst)
    }
}

/// A recorded repo: its forge, host and path.
type Recorded3 = (Kind, &'static str, &'static str);
/// A fake sign-in: its kind and token.
type Held = (TokenKind, &'static str);

/// Recorded repos and bindings, by project. **Strict about the host**: it answers for exactly
/// the host it is given, so a host that was not made by `forge_host` finds no account.
#[derive(Default)]
struct FakeRecorded {
    repos: Mutex<HashMap<PathBuf, Vec<Recorded3>>>,
    accounts: Mutex<HashMap<(PathBuf, String), String>>,
    /// The branches purlis cut for a chat, by chat.
    cut: Mutex<Vec<(String, String)>>,
}

impl FakeRecorded {
    fn cut_for(&self, chat: &str, branch: &str) {
        lock(&self.cut).push((chat.to_owned(), branch.to_owned()));
    }
    fn record(&self, project: &str, repos: Vec<Recorded3>) {
        lock(&self.repos).insert(PathBuf::from(project), repos);
    }
    fn bind(&self, project: &str, host: &str, account: &str) {
        lock(&self.accounts).insert(
            (PathBuf::from(project), host.to_owned()),
            account.to_owned(),
        );
    }
}

impl Recorded for FakeRecorded {
    fn own_branches(&self, _: &Path, chat: &str, _: &AllowedRepo) -> Vec<String> {
        lock(&self.cut)
            .iter()
            .filter(|(by, _)| by == chat)
            .map(|(_, branch)| branch.clone())
            .collect()
    }
    fn repos(&self, project: &Path) -> Vec<AllowedRepo> {
        lock(&self.repos)
            .get(project)
            .into_iter()
            .flatten()
            .map(|(kind, host, path)| AllowedRepo::new(*kind, host, 443, path, "main").unwrap())
            .collect()
    }
    fn account(&self, project: &Path, host: &str) -> Option<String> {
        lock(&self.accounts)
            .get(&(project.to_path_buf(), host.to_owned()))
            .cloned()
    }
}

/// Sign-ins by `(host, account)`; strict about the host, and every read is counted.
#[derive(Default)]
struct FakeSignIns {
    tokens: Mutex<HashMap<(String, String), Held>>,
    failing: Mutex<HashMap<(String, String), SignInState>>,
    asked: Mutex<Vec<(String, String)>>,
}

impl FakeSignIns {
    fn sign_in(&self, host: &str, account: &str, token: &'static str) {
        lock(&self.tokens).insert(
            (host.to_owned(), account.to_owned()),
            (TokenKind::Pat, token),
        );
    }
    fn fail(&self, host: &str, account: &str, state: SignInState) {
        lock(&self.failing).insert((host.to_owned(), account.to_owned()), state);
    }
    fn lookup(&self, host: &str, account: &str) -> Result<&'static str, SignInFailure> {
        let key = (host.to_owned(), account.to_owned());
        let failed = |state| SignInFailure {
            state,
            said: sign_in_words(host, state),
        };
        if let Some(state) = lock(&self.failing).get(&key) {
            return Err(failed(*state));
        }
        lock(&self.tokens)
            .get(&key)
            .map(|(_, t)| *t)
            .ok_or_else(|| failed(SignInState::Missing))
    }
}

impl Credentials for FakeSignIns {
    fn credential(&self, host: &str, account: &str) -> Result<Credential, SignInFailure> {
        lock(&self.asked).push((host.to_owned(), account.to_owned()));
        self.lookup(host, account).map(|token| Credential {
            token_kind: TokenKind::Pat,
            secret: Secret::new(token.to_owned()),
        })
    }
    fn check(&self, host: &str, account: &str) -> Result<(), SignInFailure> {
        self.lookup(host, account).map(|_| ())
    }
}

#[derive(Default)]
struct FakeNotices(Mutex<Vec<(String, String, SignInState)>>);
impl Notices for FakeNotices {
    fn sign_in_needed(&self, chat: &str, host: &str, state: SignInState) {
        lock(&self.0).push((chat.to_owned(), host.to_owned(), state));
    }
}

/// The pairing: the one device and key it paired, and the project each run of it belongs to.
struct FakePairing {
    paired: RunnerPeer,
    runs: Mutex<HashMap<String, PathBuf>>,
}
impl RunnerLink for FakePairing {
    fn paired(&self, peer: &RunnerPeer) -> bool {
        *peer == self.paired
    }
    fn project_of(&self, peer: &RunnerPeer, run: &str) -> Option<PathBuf> {
        (*peer == self.paired)
            .then(|| lock(&self.runs).get(run).cloned())
            .flatten()
    }
}

fn runner() -> RunnerPeer {
    RunnerPeer {
        device: "runner-1".to_owned(),
        key: [7; 32],
    }
}

struct Scene {
    _root: tempfile::TempDir,
    dir: PathBuf,
    routes: ForgeRoutes,
    listener: Arc<FakeListener>,
    recorded: Arc<FakeRecorded>,
    sign_ins: Arc<FakeSignIns>,
    notices: Arc<FakeNotices>,
    pairing: Arc<FakePairing>,
}

impl Scene {
    fn new() -> Self {
        Self::with_gate(None)
    }

    fn with_gate(gate: Option<Arc<dyn Gate>>) -> Self {
        Self::with_gate_and_pin(gate, Seams::stubs().pin)
    }

    /// A scene where policy pins `hosts` as never allowed.
    fn pinning(hosts: &'static [&'static str]) -> Self {
        struct Pins(&'static [&'static str]);
        impl purlis_core::forge::pin::Pin for Pins {
            fn pinned(&self, host: &str, _: u16) -> Option<String> {
                self.0
                    .contains(&host)
                    .then(|| format!("Policy never allows {host}."))
            }
        }
        Self::with_gate_and_pin(None, Arc::new(move |_| Arc::new(Pins(hosts))))
    }

    fn with_gate_and_pin(gate: Option<Arc<dyn Gate>>, pin: PinOf) -> Self {
        let root = tempfile::tempdir().expect("a temp dir");
        let dir = root.path().join("purlis-forge");
        let listener = Arc::new(FakeListener::default());
        let recorded = Arc::new(FakeRecorded::default());
        let sign_ins = Arc::new(FakeSignIns::default());
        let notices = Arc::new(FakeNotices::default());
        let pairing = Arc::new(FakePairing {
            paired: runner(),
            runs: Mutex::new(HashMap::new()),
        });
        recorded.record(
            "/p/one",
            vec![
                (Kind::GitHub, "github.com", "acme/app"),
                (Kind::GitLab, "gitlab.com", "group/proj"),
            ],
        );
        recorded.bind("/p/one", "github.com", "alice");
        recorded.bind("/p/one", "gitlab.com", "alice-gl");
        sign_ins.sign_in("github.com", "alice", GITHUB_TOKEN);
        sign_ins.sign_in("gitlab.com", "alice-gl", GITLAB_TOKEN);
        let routes = ForgeRoutes::new(
            dir.clone(),
            PathBuf::from("/app/purlis-forge-helper"),
            Seams {
                listener: listener.clone(),
                credentials: sign_ins.clone(),
                recorded: recorded.clone(),
                notices: notices.clone(),
                held: Arc::new(NoHeldSinks),
                runner_link: pairing.clone(),
                gate,
                pin,
            },
        );
        Self {
            _root: root,
            dir,
            routes,
            listener,
            recorded,
            sign_ins,
            notices,
            pairing,
        }
    }

    fn start(&self, chat: &str, run: &str, project: &str) -> RouteParams {
        self.routes
            .start(chat, run, Path::new(project))
            .expect("the route starts")
            .expect("the project records repos")
    }

    /// The folder the route of `run` is made in, while it is live.
    fn folder_of(&self, run: &str) -> PathBuf {
        self.routes.folder_of(run).expect("a live route")
    }

    /// What is in the root: the folders of every route made and not yet removed.
    fn folders(&self) -> usize {
        std::fs::read_dir(&self.dir).map_or(0, Iterator::count)
    }
}

/// A request from the run that holds `sentinel`, through `handler` (the one door).
fn send_via(
    proxy: &ForgeProxy,
    run: &str,
    sentinel: &str,
    method: &str,
    host: &str,
    target: &str,
) -> Outcome {
    let head =
        RequestHead::new(method, host, 443, target).with_content_type(Some("application/json"));
    let headers = vec![
        ("Host".to_owned(), host.to_owned()),
        ("Authorization".to_owned(), format!("Bearer {sentinel}")),
        ("Content-Type".to_owned(), "application/json".to_owned()),
    ];
    let body: &[u8] = b"{\"body\":\"hello\"}";
    proxy.handle(
        &ChatId::new(run),
        &Inbound {
            head: &head,
            headers: &headers,
            body,
        },
    )
}

/// A request with its own body, from the run that holds `sentinel`.
fn send_body(
    scene: &Scene,
    run: &str,
    sentinel: &str,
    host: &str,
    target: &str,
    body: &str,
) -> Outcome {
    let head =
        RequestHead::new("POST", host, 443, target).with_content_type(Some("application/json"));
    let headers = vec![
        ("Host".to_owned(), host.to_owned()),
        ("Authorization".to_owned(), format!("Bearer {sentinel}")),
        ("Content-Type".to_owned(), "application/json".to_owned()),
    ];
    scene.listener.proxy_of(run).handle(
        &ChatId::new(run),
        &Inbound {
            head: &head,
            headers: &headers,
            body: body.as_bytes(),
        },
    )
}

/// A request with any method, content type and bytes, from the run that holds `sentinel`.
fn send_raw(
    scene: &Scene,
    run: &str,
    sentinel: &str,
    (method, host, target): (&str, &str, &str),
    content_type: &str,
    body: &[u8],
) -> Outcome {
    let head = RequestHead::new(method, host, 443, target).with_content_type(Some(content_type));
    let headers = vec![
        ("Host".to_owned(), host.to_owned()),
        ("Authorization".to_owned(), format!("Bearer {sentinel}")),
        ("Content-Type".to_owned(), content_type.to_owned()),
    ];
    scene.listener.proxy_of(run).handle(
        &ChatId::new(run),
        &Inbound {
            head: &head,
            headers: &headers,
            body,
        },
    )
}

/// A push's command list: a pkt-line per `(old is zero, new is zero, ref)` command, then a flush.
fn push_commands(commands: &[(bool, bool, &str)]) -> Vec<u8> {
    let id = |zero: bool| if zero { "0".repeat(40) } else { "a".repeat(40) };
    let mut body = Vec::new();
    for (n, (create, delete, name)) in commands.iter().enumerate() {
        let caps = if n == 0 { "\0 report-status" } else { "" };
        let line = format!("{} {} {name}{caps}\n", id(*create), id(*delete));
        body.extend(format!("{:04x}{line}", line.len() + 4).as_bytes());
    }
    body.extend(b"0000");
    body
}

/// A GitLab GraphQL mutation that accepts a merge request, naming `project` and the request's `iid`.
fn gitlab_merge_mutation(project: &str, iid: u32) -> String {
    serde_json::json!({
        "query": format!(
            "mutation {{ mergeRequestAccept(input: {{projectPath: \"{project}\", iid: \"{iid}\"}}) {{ clientMutationId }} }}"
        )
    })
    .to_string()
}

fn send(
    scene: &Scene,
    run: &str,
    sentinel: &str,
    method: &str,
    host: &str,
    target: &str,
) -> Outcome {
    send_via(
        scene.listener.proxy_of(run).as_ref(),
        run,
        sentinel,
        method,
        host,
        target,
    )
}

fn comment(scene: &Scene, run: &str, sentinel: &str) -> Outcome {
    send(
        scene,
        run,
        sentinel,
        "POST",
        "api.github.com",
        "/repos/acme/app/issues/7/comments",
    )
}

/// A merge that pins the head it merges, as an ask requires.
const MERGE_BODY: &str = "{\"sha\":\"abc123\"}";

fn merge(scene: &Scene, run: &str, sentinel: &str) -> Outcome {
    send_raw(
        scene,
        run,
        sentinel,
        ("PUT", "api.github.com", "/repos/acme/app/pulls/7/merge"),
        "application/json",
        MERGE_BODY.as_bytes(),
    )
}

fn bearer(outcome: &Outcome) -> Option<String> {
    let Outcome::Forward(forwarded) = outcome else {
        return None;
    };
    forwarded
        .headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.clone())
}

// ---- lifecycle ----

#[test]
fn a_run_that_starts_is_handed_its_socket_ca_sentinel_helper_and_recorded_repos() {
    let scene = Scene::new();

    let params = scene.start("chat-1", "run-1", "/p/one");

    assert!(params.socket.exists() && params.ca.exists());
    assert!(params.sentinel.starts_with("purlis-sentinel-"));
    assert_eq!(params.helper_exe, PathBuf::from("/app/purlis-forge-helper"));
    assert_eq!(
        params.repos,
        vec![
            ("github.com".to_owned(), "acme/app".to_owned()),
            ("gitlab.com".to_owned(), "group/proj".to_owned())
        ]
    );
    assert!(
        !format!("{params:?}").contains(&params.sentinel),
        "Debug shows no sentinel"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for dir in [scene.folder_of("run-1"), scene.dir.clone()] {
            let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{dir:?} is the app's alone");
        }
    }
}

#[test]
fn a_run_whose_project_records_no_repo_gets_no_route_and_stops_no_other() {
    let scene = Scene::new();
    let other = scene.start("chat-1", "run-1", "/p/one");

    let started = scene
        .routes
        .start("chat-1", "run-2", Path::new("/p/none"))
        .expect("no error");

    assert!(started.is_none());
    assert!(!scene.routes.is_live("run-2"));
    assert!(
        scene.routes.is_live("run-1") && other.socket.exists(),
        "a start with nothing to proxy takes nothing down"
    );
}

#[test]
fn a_run_that_ends_leaves_no_socket_no_ca_no_listener_and_a_dead_sentinel() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");
    assert!(matches!(
        comment(&scene, "run-1", &params.sentinel),
        Outcome::Forward(_)
    ));
    let door = scene.listener.proxy_of("run-1");
    let folder = scene.folder_of("run-1");
    assert_eq!(scene.listener.alive.load(Ordering::SeqCst), 1);

    scene.routes.stop("run-1");

    assert!(!params.socket.exists() && !params.ca.exists());
    assert!(!folder.exists());
    assert_eq!(
        scene.listener.alive.load(Ordering::SeqCst),
        0,
        "the listener is closed"
    );
    assert!(matches!(
        send_via(
            door.as_ref(),
            "run-1",
            &params.sentinel,
            "POST",
            "api.github.com",
            "/repos/acme/app/issues/7/comments"
        ),
        Outcome::Refused(Refusal::UnknownChat)
    ));
}

#[test]
fn a_listener_that_cannot_bind_leaves_nothing_behind() {
    let scene = Scene::new();
    scene.listener.fail.store(true, Ordering::SeqCst);

    let started = scene.routes.start("chat-1", "run-1", Path::new("/p/one"));

    assert!(started.is_err());
    assert!(!scene.routes.is_live("run-1"));
    assert_eq!(scene.folders(), 0, "no folder is left");
}

#[test]
fn the_same_run_starting_again_replaces_its_route_and_the_old_sentinel_stops() {
    let scene = Scene::new();
    let first = scene.start("chat-1", "run-1", "/p/one");

    let second = scene.start("chat-1", "run-1", "/p/one");

    assert_ne!(first.sentinel, second.sentinel);
    assert_eq!(scene.listener.alive.load(Ordering::SeqCst), 1);
    assert!(matches!(
        comment(&scene, "run-1", &first.sentinel),
        Outcome::Refused(Refusal::WrongSentinel)
    ));
    assert!(matches!(
        comment(&scene, "run-1", &second.sentinel),
        Outcome::Forward(_)
    ));
}

#[test]
fn a_restart_keeps_the_chat_and_the_old_runs_end_leaves_the_new_runs_route() {
    let scene = Scene::new();
    // Session 1, run A of chat-1: started, open.
    let old = scene.routes.guard("chat-1", "run-a", Path::new("/p/one"));
    old.commit(1);
    // The restart: the same chat, a new run B, started before the old one ends.
    let new = scene.routes.guard("chat-1", "run-b", Path::new("/p/one"));
    let sentinel_b = new.params().expect("a route").sentinel.clone();
    new.commit(2);
    assert!(scene.routes.is_live("run-a") && scene.routes.is_live("run-b"));

    // The old run's program ends, and then its session is closed.
    scene.routes.stop_session(1);
    scene.routes.stop_session(1);

    assert!(!scene.routes.is_live("run-a"));
    assert!(
        scene.routes.is_live("run-b"),
        "the new run still has its route"
    );
    assert!(matches!(
        comment(&scene, "run-b", &sentinel_b),
        Outcome::Forward(_)
    ));
}

#[test]
fn a_start_that_returns_early_stops_its_route_and_one_that_opens_keeps_it_until_its_session_ends() {
    let scene = Scene::new();

    // An early `Err` in the start drops the guard.
    {
        let early = scene.routes.guard("chat-1", "run-1", Path::new("/p/one"));
        assert!(early.params().is_some());
        assert!(scene.routes.is_live("run-1"));
    }
    assert!(
        !scene.routes.is_live("run-1"),
        "a dropped guard stops the route"
    );
    assert_eq!(scene.folders(), 0);
    assert_eq!(scene.listener.alive.load(Ordering::SeqCst), 0);

    // An open chat keeps it until its session closes or its program ends.
    scene
        .routes
        .guard("chat-1", "run-2", Path::new("/p/one"))
        .commit(9);
    assert!(scene.routes.is_live("run-2"));
    scene.routes.stop_session(9);
    assert!(!scene.routes.is_live("run-2"));

    // A project with nothing recorded has a guard with no route, and committing it is harmless.
    let none = scene.routes.guard("chat-1", "run-3", Path::new("/p/none"));
    assert!(none.params().is_none());
    none.commit(10);
    scene.routes.stop_session(10);
}

#[test]
fn closing_a_project_stops_every_route_of_that_project_and_no_other() {
    let scene = Scene::new();
    scene
        .recorded
        .record("/p/two", vec![(Kind::GitHub, "github.com", "acme/app")]);
    scene.start("chat-1", "run-1", "/p/one");
    scene.start("chat-2", "run-2", "/p/one");
    scene.start("chat-3", "run-3", "/p/two");

    scene.routes.stop_project(Path::new("/p/one"));

    assert!(!scene.routes.is_live("run-1") && !scene.routes.is_live("run-2"));
    assert!(scene.routes.is_live("run-3"));
}

#[test]
fn an_app_that_crashed_leaves_folders_the_next_start_sweeps_when_its_pid_is_gone() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");
    let leftover = params.socket.parent().unwrap().to_owned();
    // A pid that is gone: a child that has run and been reaped.
    let mut child =
        purlis_core::forklock::spawn(&mut std::process::Command::new("true")).expect("true");
    let gone = child.id();
    child.wait().unwrap();
    let from_a_dead_app = scene.dir.join(format!("{gone}-0a0a0a0a"));
    std::fs::create_dir(&from_a_dead_app).unwrap();
    std::fs::write(from_a_dead_app.join("proxy.sock"), b"").unwrap();
    // An app that is running (this test's parent), a name that is not ours, and a link.
    #[cfg(unix)]
    let parent = std::os::unix::process::parent_id();
    #[cfg(not(unix))]
    let parent = 1;
    let from_a_live_app = scene.dir.join(format!("{parent}-0b0b0b0b"));
    std::fs::create_dir(&from_a_live_app).unwrap();
    let not_ours = scene.dir.join("notes");
    std::fs::create_dir(&not_ours).unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(elsewhere.path().join("keep"), b"x").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(elsewhere.path(), scene.dir.join(format!("{gone}-0c0c0c0c")))
        .unwrap();
    // A crash runs no destructor.
    let Scene { routes, _root, .. } = scene;
    std::mem::forget(routes);
    assert!(leftover.exists(), "the crash left its folder");

    let restarted = ForgeRoutes::new(
        leftover.parent().unwrap().to_owned(),
        PathBuf::new(),
        Seams::stubs(),
    );

    assert!(!from_a_dead_app.exists(), "a gone app's folder is removed");
    assert!(from_a_live_app.exists(), "a running app's folder is not");
    assert!(not_ours.exists(), "nor is a name this module never made");
    assert!(
        elsewhere.path().join("keep").exists(),
        "a link is never followed"
    );
    // This process made `leftover`, and it is running.
    assert!(leftover.exists());
    drop(restarted);
    drop(_root);
}

#[test]
fn folder_names_are_pid_then_eight_hex_and_only_those_are_read_back() {
    assert_eq!(pid_of_folder("123-0a1b2c3d"), Some(123));
    for not_ours in [
        "notes",
        "-0a1b2c3d",
        "12a-0a1b2c3d",
        "123-",
        "123-run-1",
        "123-0A1B2C3D",
        "123-0a1b2c3",
        "123-0a1b2c3d4",
        "123-../0a1b",
        "../123-0a1b2c3d",
    ] {
        assert_eq!(pid_of_folder(not_ours), None, "{not_ours}");
    }
    let made = folder_name().unwrap();
    assert_eq!(pid_of_folder(&made), Some(std::process::id()), "{made}");
    assert_ne!(made, folder_name().unwrap(), "each folder is its own");
}

#[cfg(unix)]
#[test]
fn a_pid_this_process_may_not_signal_is_a_running_process_and_only_no_such_process_is_gone() {
    use rustix::io::Errno;
    assert!(probe_says_running(Ok(())));
    assert!(
        probe_says_running(Err(Errno::PERM)),
        "EPERM: another user's, or another namespace's, process is alive"
    );
    assert!(
        probe_says_running(Err(Errno::INVAL)),
        "anything else is alive"
    );
    assert!(!probe_says_running(Err(Errno::SRCH)), "only ESRCH is gone");
    // And the real probe: this process runs, a reaped child does not, a pid past i32 is left.
    assert!(process_may_run(std::process::id()));
    let mut child =
        purlis_core::forklock::spawn(&mut std::process::Command::new("true")).expect("true");
    let gone = child.id();
    child.wait().unwrap();
    assert!(!process_may_run(gone));
    assert!(process_may_run(u32::MAX));
}

#[cfg(unix)]
#[test]
fn a_sweep_that_cannot_see_an_apps_pid_removes_nothing_of_it() {
    let scene = Scene::new();
    scene.start("chat-1", "run-1", "/p/one");
    let root = scene.dir.clone();
    // Folders of an app this process may not signal (another user's, or the macOS sandbox): the
    // probe answers EPERM. (Another pid namespace answers ESRCH; the one-per-user instance lock
    // is what protects there.)
    let unseen = root.join("4242-0a0a0a0a");
    std::fs::create_dir(&unseen).unwrap();
    let gone = root.join("4343-0b0b0b0b");
    std::fs::create_dir(&gone).unwrap();

    sweep_with(&root, &|pid| {
        probe_says_running(if pid == 4343 {
            Err(rustix::io::Errno::SRCH)
        } else {
            Err(rustix::io::Errno::PERM)
        })
    });

    assert!(unseen.exists(), "an unreadable pid is not a gone one");
    assert!(!gone.exists(), "a pid that is gone still is swept");
    assert_eq!(
        scene.folders(),
        2,
        "this app's own route and the unseen app's"
    );
}

#[cfg(unix)]
#[test]
fn a_root_that_is_a_link_is_refused_and_what_it_points_at_is_left_alone() {
    let scene = Scene::new();
    let target = tempfile::tempdir().unwrap();
    let victim = target.path().join("1-0e0e0e0e");
    std::fs::create_dir(&victim).unwrap();
    let linked = scene._root.path().join("linked-root");
    std::os::unix::fs::symlink(target.path(), &linked).unwrap();

    let routes = ForgeRoutes::new(linked.clone(), PathBuf::new(), {
        let mut seams = Seams::stubs();
        seams.recorded = scene.recorded.clone();
        seams.listener = scene.listener.clone();
        seams
    });
    let started = routes.start("chat-1", "run-1", Path::new("/p/one"));

    assert!(started.is_err(), "a route is never made through a link");
    assert!(victim.exists(), "the sweep never followed the link");
    assert_eq!(std::fs::read_dir(target.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn a_root_with_loose_permissions_is_made_private_before_a_route_is_made_in_it() {
    use std::os::unix::fs::PermissionsExt;
    let scene = Scene::new();
    let loose = scene._root.path().join("loose");
    std::fs::create_dir(&loose).unwrap();
    std::fs::set_permissions(&loose, std::fs::Permissions::from_mode(0o755)).unwrap();
    let routes = ForgeRoutes::new(loose.clone(), PathBuf::new(), {
        let mut seams = Seams::stubs();
        seams.recorded = scene.recorded.clone();
        seams.listener = scene.listener.clone();
        seams
    });

    routes
        .start("chat-1", "run-1", Path::new("/p/one"))
        .expect("starts")
        .expect("a route");

    let mode = std::fs::metadata(&loose).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
}

#[test]
fn quitting_stops_every_route_of_this_app_and_leaves_another_apps_folders() {
    let scene = Scene::new();
    let a = scene.start("chat-1", "run-1", "/p/one");
    let b = scene.start("chat-2", "run-2", "/p/one");
    #[cfg(unix)]
    let parent = std::os::unix::process::parent_id();
    #[cfg(not(unix))]
    let parent = 1;
    let another = scene.dir.join(format!("{parent}-0d0d0d0d"));
    std::fs::create_dir(&another).unwrap();

    scene.routes.stop_all();

    assert!(!a.socket.exists() && !b.socket.exists());
    assert_eq!(scene.listener.alive.load(Ordering::SeqCst), 0);
    assert!(another.exists(), "only this app's own folders go");

    let c = scene.start("chat-3", "run-3", "/p/one");
    let folder = c.socket.parent().unwrap().to_owned();
    let Scene { routes, _root, .. } = scene;
    drop(routes);
    assert!(
        !folder.exists(),
        "a dropped app leaves no folder of its own"
    );
    drop(_root);
}

#[test]
fn stub_seams_fail_closed_a_run_has_no_route_without_a_listener() {
    let root = tempfile::tempdir().unwrap();
    let routes = ForgeRoutes::new(root.path().join("r"), PathBuf::new(), Seams::stubs());

    assert!(routes.start("c", "r", Path::new("/p")).unwrap().is_none());
    assert_eq!(
        NoSignIns.check("github.com", "a").unwrap_err().state,
        SignInState::Missing
    );
    assert!(
        NoListener
            .bind(
                "r",
                root.path(),
                Arc::new(ForgeProxy::new(
                    Box::new(NoCredentials),
                    Box::new(NoObjects)
                )),
                Arc::new(BlindGate)
            )
            .is_err()
    );
    assert!(matches!(
        routes.for_runner(&runner(), "c", "r"),
        RunnerForward::Refused
    ));
}

#[test]
fn the_forge_host_of_a_request_is_made_in_one_place() {
    assert_eq!(forge_host("api.github.com"), "github.com");
    assert_eq!(forge_host(" GitHub.com "), "github.com");
    assert_eq!(forge_host("ghe.corp.example"), "ghe.corp.example");
    assert_eq!(forge_host("gitlab.com"), "gitlab.com");
}

#[test]
fn each_host_is_answered_with_the_credential_of_the_account_its_project_is_bound_to() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");

    let github = comment(&scene, "run-1", &params.sentinel);
    let gitlab = send(
        &scene,
        "run-1",
        &params.sentinel,
        "POST",
        "gitlab.com",
        "/api/v4/projects/group%2Fproj/issues/3/notes",
    );

    assert!(
        bearer(&github).is_some_and(|v| v.contains(GITHUB_TOKEN)),
        "{github:?}"
    );
    let gitlab_headers = match &gitlab {
        Outcome::Forward(f) => f
            .headers
            .iter()
            .map(|(_, v)| v.clone())
            .collect::<Vec<_>>()
            .join("|"),
        other => panic!("{other:?}"),
    };
    assert!(gitlab_headers.contains(GITLAB_TOKEN), "{gitlab_headers}");
    assert_eq!(
        *lock(&scene.sign_ins.asked),
        vec![
            ("github.com".to_owned(), "alice".to_owned()),
            ("gitlab.com".to_owned(), "alice-gl".to_owned())
        ]
    );
}

#[test]
fn two_projects_on_one_host_use_their_own_accounts() {
    let scene = Scene::new();
    scene
        .recorded
        .record("/p/two", vec![(Kind::GitHub, "github.com", "acme/app")]);
    scene.recorded.bind("/p/two", "github.com", "bob");
    scene
        .sign_ins
        .sign_in("github.com", "bob", "REAL-bob-token-1");
    let one = scene.start("chat-1", "run-1", "/p/one");
    let two = scene.start("chat-2", "run-2", "/p/two");

    let for_one = comment(&scene, "run-1", &one.sentinel);
    let for_two = comment(&scene, "run-2", &two.sentinel);

    assert!(bearer(&for_one).unwrap().contains(GITHUB_TOKEN));
    assert!(bearer(&for_two).unwrap().contains("REAL-bob-token-1"));
}

#[test]
fn another_runs_sentinel_is_refused_at_this_runs_endpoint() {
    let scene = Scene::new();
    let a = scene.start("chat-1", "run-a", "/p/one");
    let b = scene.start("chat-2", "run-b", "/p/one");

    let crossed = comment(&scene, "run-a", &b.sentinel);

    assert!(
        matches!(crossed, Outcome::Refused(Refusal::WrongSentinel)),
        "{crossed:?}"
    );
    assert!(matches!(
        comment(&scene, "run-a", &a.sentinel),
        Outcome::Forward(_)
    ));
}

#[test]
fn a_missing_sign_in_is_refused_in_words_that_name_the_sign_in_step_and_noticed_once_per_chat() {
    let scene = Scene::new();
    lock(&scene.sign_ins.tokens).clear();
    let params = scene.start("chat-1", "run-1", "/p/one");

    for _ in 0..3 {
        let outcome = comment(&scene, "run-1", &params.sentinel);
        assert!(
            matches!(outcome, Outcome::Refused(Refusal::NoCredential)),
            "{outcome:?}"
        );
    }

    let said = scene.routes.refusal_message("run-1", Refusal::NoCredential);
    assert!(
        said.contains("github.com") && said.contains("Sign in") && said.contains("Accounts"),
        "{said}"
    );
    assert_eq!(
        *lock(&scene.notices.0),
        vec![(
            "chat-1".to_owned(),
            "github.com".to_owned(),
            SignInState::Missing
        )],
        "one Notice for the chat, however many requests"
    );

    // The chat restarts while it still has the first run: its second run is not told again.
    let second = scene.start("chat-1", "run-2", "/p/one");
    comment(&scene, "run-2", &second.sentinel);
    assert_eq!(lock(&scene.notices.0).len(), 1);
}

#[test]
fn an_ended_sign_in_says_it_ended_and_a_host_with_no_bound_account_is_missing() {
    let scene = Scene::new();
    scene
        .sign_ins
        .fail("github.com", "alice", SignInState::Ended);
    let params = scene.start("chat-1", "run-1", "/p/one");

    assert!(matches!(
        comment(&scene, "run-1", &params.sentinel),
        Outcome::Refused(Refusal::NoCredential)
    ));
    assert!(
        scene
            .routes
            .refusal_message("run-1", Refusal::NoCredential)
            .contains("has ended")
    );

    lock(&scene.recorded.accounts).clear();
    let other = scene.start("chat-2", "run-2", "/p/one");
    assert!(matches!(
        comment(&scene, "run-2", &other.sentinel),
        Outcome::Refused(Refusal::NoCredential)
    ));
    assert!(
        scene
            .routes
            .refusal_message("run-2", Refusal::NoCredential)
            .contains("no sign-in")
    );
    assert_eq!(lock(&scene.notices.0).len(), 2, "each chat is told once");
}

#[test]
fn a_forge_that_could_not_be_reached_is_unreachable_not_ended_and_raises_no_notice() {
    let scene = Scene::new();
    scene
        .sign_ins
        .fail("github.com", "alice", SignInState::Unavailable);
    let params = scene.start("chat-1", "run-1", "/p/one");

    let outcome = comment(&scene, "run-1", &params.sentinel);

    assert!(matches!(outcome, Outcome::Refused(Refusal::NoCredential)));
    let said = scene.routes.refusal_message("run-1", Refusal::NoCredential);
    assert!(
        said.contains("could not reach") && !said.contains("ended"),
        "{said}"
    );
    assert!(
        lock(&scene.notices.0).is_empty(),
        "a network failure is not the person's to fix"
    );
}

// ---- held acts ----

#[test]
fn a_paired_runners_run_is_forwarded_to_this_apps_proxy_for_the_pairings_own_project() {
    let scene = Scene::new();
    lock(&scene.pairing.runs).insert("job-1".to_owned(), PathBuf::from("/p/one"));

    let forward = scene.routes.for_runner(&runner(), "chat-9", "job-1");

    let RunnerForward::Desktop(params) = forward else {
        panic!("{forward:?}")
    };
    assert!(scene.routes.is_live("job-1"));
    assert!(matches!(
        comment(&scene, "job-1", &params.sentinel),
        Outcome::Forward(_)
    ));
    scene.routes.stop_peer("runner-1");
    assert!(
        !scene.routes.is_live("job-1"),
        "its link went, so did its route"
    );
    assert!(!params.socket.exists());
}

#[test]
fn an_unpaired_caller_or_a_run_the_pairing_does_not_know_is_refused_and_no_route_is_made() {
    let scene = Scene::new();
    lock(&scene.pairing.runs).insert("job-1".to_owned(), PathBuf::from("/p/one"));
    let stranger = RunnerPeer {
        device: "runner-1".to_owned(),
        key: [8; 32], // the right name, another key
    };
    let other_device = RunnerPeer {
        device: "runner-2".to_owned(),
        key: [7; 32],
    };

    for peer in [&stranger, &other_device] {
        let forward = scene.routes.for_runner(peer, "chat-9", "job-1");
        assert!(matches!(forward, RunnerForward::Refused), "{forward:?}");
    }
    let unknown_run = scene.routes.for_runner(&runner(), "chat-9", "job-404");
    assert!(
        matches!(unknown_run, RunnerForward::Refused),
        "{unknown_run:?}"
    );

    assert!(!scene.routes.is_live("job-1") && !scene.routes.is_live("job-404"));
    assert_eq!(scene.listener.alive.load(Ordering::SeqCst), 0);
}

#[test]
fn a_paired_runner_with_nothing_to_proxy_or_no_proxy_keeps_token_connections() {
    let scene = Scene::new();
    lock(&scene.pairing.runs).insert("job-1".to_owned(), PathBuf::from("/p/none"));
    lock(&scene.pairing.runs).insert("job-2".to_owned(), PathBuf::from("/p/one"));

    let nothing = scene.routes.for_runner(&runner(), "chat-9", "job-1");
    assert!(
        matches!(nothing, RunnerForward::TokenConnections),
        "{nothing:?}"
    );

    scene.listener.fail.store(true, Ordering::SeqCst);
    let no_proxy = scene.routes.for_runner(&runner(), "chat-9", "job-2");
    assert!(
        matches!(no_proxy, RunnerForward::TokenConnections),
        "{no_proxy:?}"
    );
    assert!(!scene.routes.is_live("job-1") && !scene.routes.is_live("job-2"));
}

// ---- audit and doctor ----

#[test]
fn the_audit_has_a_line_per_request_and_never_a_token_or_a_path() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");
    comment(&scene, "run-1", &params.sentinel);
    merge(&scene, "run-1", &params.sentinel);

    let lines = scene.routes.audit_lines("run-1");

    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("POST api.github.com acme/app") && lines[0].contains("Allowed"));
    assert!(lines[1].contains("Refused(Merge)"));
    let all = lines.join("\n");
    for secret in [GITHUB_TOKEN, params.sentinel.as_str(), "zzz", "/pulls/7"] {
        assert!(!all.contains(secret), "{secret} is in the audit: {all}");
    }
}

#[test]
fn the_doctor_row_says_proxy_up_and_a_sign_in_for_every_recorded_host() {
    let scene = Scene::new();
    let project = Path::new("/p/one");

    let ok = scene.routes.row(project);
    assert!(!ok.problem, "{ok:?}");
    assert!(
        ok.detail.contains("github.com") && ok.detail.contains("gitlab.com"),
        "{ok:?}"
    );

    scene
        .sign_ins
        .fail("gitlab.com", "alice-gl", SignInState::Ended);
    let one_missing = scene.routes.row(project);
    assert!(
        one_missing.problem && one_missing.detail.contains("gitlab.com"),
        "{one_missing:?}"
    );
    assert!(!one_missing.detail.contains("github.com"));

    scene.listener.fail.store(true, Ordering::SeqCst);
    let down = scene.routes.row(project);
    assert!(
        down.problem && down.detail.contains("not running"),
        "{down:?}"
    );

    assert!(
        !scene.routes.row(Path::new("/p/none")).problem,
        "no recorded repo, nothing to check"
    );
}

#[test]
fn the_real_sign_ins_answer_missing_with_the_step_named_when_no_account_is_listed() {
    let Some(scope) = purlis_core::forge::route::HostScope::claim() else {
        return; // another test of this process holds the scope; this one asks nothing then
    };
    let root = tempfile::tempdir().unwrap();
    let real = AccountCredentials::new(purlis_core::forge::credential::SignedIn::new(
        &scope,
        root.path(),
    ));

    let failure = real
        .credential("github.com", "no-such-account")
        .expect_err("no account");

    assert!(
        failure.said.to_lowercase().contains("sign in")
            || failure.said.contains("Choose an account"),
        "{}",
        failure.said
    );
}

// ---- a program that exits before its start commits ----

#[test]
fn a_program_that_exits_before_its_start_commits_has_its_route_stopped_by_the_commit() {
    let scene = Scene::new();
    // The start has opened session 7 and holds its route; the exit arrives on another thread.
    let held = scene.routes.guard("chat-1", "run-1", Path::new("/p/one"));
    let folder = scene.folder_of("run-1");
    assert!(scene.routes.is_live("run-1"));

    scene.routes.stop_session(7);
    assert!(
        scene.routes.is_live("run-1"),
        "nothing is mapped yet, so nothing is stopped by the exit alone"
    );
    held.commit(7);

    assert!(!scene.routes.is_live("run-1"), "the commit stops it");
    assert!(!folder.exists());
    assert_eq!(scene.listener.alive.load(Ordering::SeqCst), 0);
    assert!(
        scene.routes.sessions().exited.is_empty(),
        "nothing is remembered once nothing is pending"
    );
}

#[test]
fn an_exit_before_the_commit_stops_only_the_run_of_that_session() {
    let scene = Scene::new();
    scene
        .routes
        .guard("chat-1", "run-a", Path::new("/p/one"))
        .commit(1);
    let held = scene.routes.guard("chat-2", "run-b", Path::new("/p/one"));

    // Another chat's session ends while run-b is still being opened as session 2.
    scene.routes.stop_session(1);
    scene.routes.stop_session(2);
    held.commit(2);

    assert!(!scene.routes.is_live("run-a") && !scene.routes.is_live("run-b"));

    // A run that opens after an exit of another session keeps its route.
    let held = scene.routes.guard("chat-3", "run-c", Path::new("/p/one"));
    scene.routes.stop_session(3);
    held.commit(4);
    assert!(
        scene.routes.is_live("run-c"),
        "session 3 was not its session"
    );
    scene.routes.stop_session(4);
    assert!(!scene.routes.is_live("run-c"));
}

#[test]
fn exits_of_chats_with_no_route_are_not_remembered_when_nothing_is_pending() {
    let scene = Scene::new();
    for session in 0..50 {
        scene.routes.stop_session(session);
    }
    assert!(scene.routes.sessions().exited.is_empty());

    // A paired runner's route is never committed to a session, so it never makes one pending.
    scene
        .pairing
        .runs
        .lock()
        .unwrap()
        .insert("run-r".to_owned(), PathBuf::from("/p/one"));
    let forwarded = scene.routes.for_runner(&runner(), "chat-r", "run-r");
    assert!(matches!(forwarded, RunnerForward::Desktop(_)));
    scene.routes.stop_session(99);
    assert!(scene.routes.sessions().exited.is_empty());
    assert!(scene.routes.is_live("run-r"));
}

#[test]
fn a_run_started_again_with_nothing_recorded_stops_its_old_route() {
    let scene = Scene::new();
    let old = scene.start("chat-1", "run-1", "/p/one");
    let folder = scene.folder_of("run-1");
    scene.recorded.record("/p/one", Vec::new());

    let again = scene
        .routes
        .start("chat-1", "run-1", Path::new("/p/one"))
        .expect("no error");

    assert!(again.is_none());
    assert!(!scene.routes.is_live("run-1"), "the old route is gone");
    assert!(!folder.exists() && !old.socket.exists());
    assert!(matches!(
        comment(&scene, "run-1", &old.sentinel),
        Outcome::Refused(Refusal::UnknownChat)
    ));
}

// ---- held acts are bound to the run's repos ----

#[test]
fn a_run_folder_makes_a_socket_path_within_the_limit_and_a_real_socket_binds_in_it() {
    let scene = Scene::new();

    let params = scene.start(
        "a-run-with-a-very-long-id-0123456789-0123456789-0123456789",
        "run-1",
        "/p/one",
    );

    let length = params.socket.as_os_str().len();
    assert!(length <= MAX_SOCKET_PATH, "{length}: {:?}", params.socket);
    assert!(params.socket.exists(), "the fake bound a real socket there");
    let name = scene.folder_of("run-1");
    assert_eq!(
        pid_of_folder(name.file_name().unwrap().to_str().unwrap()),
        Some(std::process::id()),
        "the run's id is not in the folder's name"
    );
}

#[test]
fn a_root_too_deep_for_a_socket_path_is_refused_by_name_before_anything_is_made() {
    let scene = Scene::new();
    let deep = scene._root.path().join("d".repeat(MAX_SOCKET_PATH));
    let routes = ForgeRoutes::new(deep.clone(), PathBuf::new(), {
        let mut seams = Seams::stubs();
        seams.recorded = scene.recorded.clone();
        seams.listener = scene.listener.clone();
        seams
    });

    let refused = routes
        .start("chat-1", "run-1", Path::new("/p/one"))
        .expect_err("a path no socket can hold");

    let said = refused.to_string();
    assert!(
        said.contains("socket path") && said.contains(&MAX_SOCKET_PATH.to_string()),
        "{said}"
    );
    assert!(!routes.is_live("run-1"));
    assert_eq!(
        std::fs::read_dir(&deep).map_or(0, Iterator::count),
        0,
        "no folder was made"
    );
    assert_eq!(scene.listener.alive.load(Ordering::SeqCst), 0);
}

#[test]
fn the_route_folder_of_this_machine_leaves_room_for_the_longest_pid_and_user() {
    let Some(root) = route_dir_in(
        None,
        None,
        std::env::temp_dir(),
        &u32::MAX.to_string(),
        false,
    ) else {
        panic!("a temporary directory always gives one");
    };
    let socket = root
        .join(format!("{}-{}", u32::MAX, "0a1b2c3d"))
        .join(SOCKET_NAME);
    assert!(
        socket.as_os_str().len() <= MAX_SOCKET_PATH,
        "{} bytes: {socket:?}",
        socket.as_os_str().len()
    );
}

#[test]
fn without_a_runtime_directory_linux_uses_the_users_own_machine_folder_never_tmp() {
    let runtime = tempfile::tempdir().unwrap();
    let machine = PathBuf::from("/home/u/.config/purlis");
    let tmp = PathBuf::from("/tmp");

    let with = route_dir_in(
        Some(runtime.path().to_owned()),
        Some(machine.clone()),
        tmp.clone(),
        "1000",
        true,
    );
    let without = route_dir_in(None, Some(machine.clone()), tmp.clone(), "1000", true);
    let gone = route_dir_in(
        Some(PathBuf::from("/no/such/runtime")),
        Some(machine.clone()),
        tmp.clone(),
        "1000",
        true,
    );
    let relative = route_dir_in(
        Some(PathBuf::from("run")),
        Some(machine.clone()),
        tmp.clone(),
        "1000",
        true,
    );
    let nowhere = route_dir_in(None, None, tmp.clone(), "1000", true);
    let mac = route_dir_in(None, Some(machine.clone()), tmp.clone(), "501", false);

    assert_eq!(with, Some(runtime.path().join("pf-1000")));
    for fallback in [without, gone, relative] {
        assert_eq!(fallback, Some(machine.join("forge-route")));
    }
    assert_eq!(nowhere, None, "no route rather than one in /tmp");
    assert_eq!(mac, Some(tmp.join("pf-501")));
}

// ---- held asks: body, cap, and the checks at the act ----

/// Slice G's gate, as a test stands it: answers with its token, or nothing, and notes who enrolled.
#[derive(Default)]
struct FakeGate {
    token: Mutex<Option<&'static str>>,
    enrolled: Mutex<Vec<String>>,
    left: Mutex<Vec<String>>,
}

impl Gate for FakeGate {
    fn enrol(&self, run: &str, _: &Path) {
        lock(&self.enrolled).push(run.to_owned());
    }
    fn leave(&self, run: &str) {
        lock(&self.left).push(run.to_owned());
    }
    fn credential(&self, _: &CredentialRequest) -> Result<Credential, CredentialError> {
        lock(&self.token)
            .map(|token| Credential {
                token_kind: TokenKind::Oauth,
                secret: Secret::new(token.to_owned()),
            })
            .ok_or(CredentialError::Unavailable)
    }
}

#[test]
fn a_gate_that_serves_the_repo_decides_the_credential_and_one_that_does_not_gives_nothing() {
    let gate = Arc::new(FakeGate::default());
    let scene = Scene::with_gate(Some(gate.clone()));
    let params = scene.start("chat-1", "run-1", "/p/one");

    // The gate refuses (the repo is not confirmed, or its chosen account cannot serve it): the
    // project's own bound sign-in is not a way around it.
    let refused = comment(&scene, "run-1", &params.sentinel);
    assert!(
        matches!(refused, Outcome::Refused(Refusal::NoCredential)),
        "{refused:?}"
    );
    assert!(
        scene
            .routes
            .refusal_message("run-1", Refusal::NoCredential)
            .contains("confirmed"),
        "the sentence names what to check"
    );

    // The gate serves it: its credential is the one sent, not the bound account's.
    *lock(&gate.token) = Some("GATE-token-4417");
    let served = comment(&scene, "run-1", &params.sentinel);
    assert!(
        bearer(&served).is_some_and(|v| v.contains("GATE-token-4417") && !v.contains(GITHUB_TOKEN)),
        "{served:?}"
    );
}

#[test]
fn a_gate_is_told_of_each_run_that_starts_and_each_that_stops() {
    let gate = Arc::new(FakeGate::default());
    let scene = Scene::with_gate(Some(gate.clone()));
    scene.start("chat-1", "run-1", "/p/one");
    assert_eq!(*lock(&gate.enrolled), ["run-1"]);
    scene.routes.stop("run-1");
    assert_eq!(*lock(&gate.left), ["run-1"]);
}

#[test]
fn a_run_is_handed_what_its_sandbox_is_wired_from_and_never_shows_the_sentinel() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");
    assert_eq!(params.inputs.gh_socket, params.socket);
    assert_eq!(params.inputs.ca_cert, params.ca);
    assert!(params.inputs.hosts.iter().any(|host| host == "github.com"));
    assert_eq!(params.inputs.repos.len(), 2);
    assert!(
        !format!("{:?}", params.inputs).contains(&params.sentinel),
        "the sentinel is never printed"
    );
}

#[test]
fn the_apps_git_for_a_chat_takes_the_route_of_the_run_its_session_is_and_no_other() {
    let scene = Scene::new();
    // No run is committed to the session yet.
    assert!(scene.routes.git_route(7).is_none());
    let guard = scene.routes.guard("chat-1", "run-1", Path::new("/p/one"));
    assert!(guard.params().is_some());
    guard.commit(7);
    let route = scene
        .routes
        .git_route(7)
        .expect("a route for the session")
        .expect("one git can use");
    let shown = format!("{route:?}");
    assert!(shown.contains("HTTPS_PROXY"), "{shown}");
    assert!(
        shown.contains("credential.https://github.com.helper="),
        "{shown}"
    );
    // Another session has none, and neither has one whose route stopped.
    assert!(scene.routes.git_route(8).is_none());
    scene.routes.stop("run-1");
    assert!(scene.routes.git_route(7).is_none());
}

#[test]
fn a_repo_on_a_pinned_forge_is_not_enrolled_and_a_project_with_only_pinned_repos_has_no_route() {
    // github.com pinned: the chat's route holds the GitLab repo and none of GitHub's.
    let scene = Scene::pinning(&["github.com"]);
    let params = scene.start("chat-1", "run-1", "/p/one");
    assert_eq!(
        params.repos,
        [("gitlab.com".to_owned(), "group/proj".to_owned())]
    );
    assert!(
        params
            .inputs
            .repos
            .iter()
            .all(|repo| repo.host() != "github.com")
    );
    // Both pinned: no route at all, and nothing is left running for the run.
    let scene = Scene::pinning(&["github.com", "gitlab.com"]);
    assert!(
        scene
            .routes
            .start("chat-1", "run-1", Path::new("/p/one"))
            .expect("no error")
            .is_none()
    );
    assert!(!scene.routes.is_live("run-1"));
}

#[test]
fn a_runner_whose_every_repo_is_pinned_is_refused_and_does_not_keep_its_token_connections() {
    let scene = Scene::pinning(&["github.com", "gitlab.com"]);
    lock(&scene.pairing.runs).insert("job-1".to_owned(), PathBuf::from("/p/one"));
    assert!(matches!(
        scene.routes.for_runner(&runner(), "chat-9", "job-1"),
        RunnerForward::Refused
    ));
}

#[test]
fn the_apps_git_for_a_chat_goes_to_the_relay_that_proves_the_hand_off_and_never_the_bare_door() {
    let scene = Scene::new();
    let guard = scene.routes.guard("chat-1", "run-1", Path::new("/p/one"));
    guard.commit(7);
    let (relay, door) = {
        let live = scene.routes.live();
        let run = live.get("run-1").expect("a live route");
        (
            run.relay.as_ref().expect("the relay started").addr(),
            run.inputs.door,
        )
    };
    assert_ne!(relay, door, "the relay is not the door");
    let route = scene
        .routes
        .git_route(7)
        .expect("a route for the session")
        .expect("one git can use");
    let shown = format!("{route:?}");
    assert!(shown.contains(&format!("http://{relay}")), "{shown}");
    assert!(!shown.contains(&format!("http://{door}")), "{shown}");
}

// ---- a chat's own branches ----

/// A push of `branch` (an update, so the branch exists on the forge already) through the run's
/// route, as `git push` sends it.
fn push_update(scene: &Scene, run: &str, sentinel: &str, branch: &str) -> Outcome {
    send_raw(
        scene,
        run,
        sentinel,
        ("POST", "github.com", "/acme/app.git/git-receive-pack"),
        "application/x-git-receive-pack-request",
        &push_commands(&[(false, false, &format!("refs/heads/{branch}"))]),
    )
}

#[test]
fn a_branch_purlis_cut_for_the_chat_may_be_pushed_though_the_forge_has_it_already() {
    let scene = Scene::new();
    scene.recorded.cut_for("chat-1", "task/forms");
    let params = scene.start("chat-1", "run-1", "/p/one");

    let pushed = push_update(&scene, "run-1", &params.sentinel, "task/forms");

    assert!(matches!(pushed, Outcome::Forward(_)), "{pushed:?}");
}

#[test]
fn another_branch_the_default_branch_and_another_chats_branch_stay_protected() {
    let scene = Scene::new();
    scene.recorded.cut_for("chat-1", "task/forms");
    scene.recorded.cut_for("chat-2", "task/other");
    let params = scene.start("chat-1", "run-1", "/p/one");

    for branch in ["main", "task/other", "colleague"] {
        let pushed = push_update(&scene, "run-1", &params.sentinel, branch);
        assert!(
            matches!(pushed, Outcome::Refused(Refusal::ProtectedRef)),
            "{branch}: {pushed:?}"
        );
    }
}
