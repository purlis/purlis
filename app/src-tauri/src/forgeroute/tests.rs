// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use purlis_core::forge::proxy::{Kind, RequestHead, TokenKind};
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
    handlers: Mutex<HashMap<String, Arc<dyn Handler>>>,
}

struct Alive(Arc<AtomicUsize>);
impl Drop for Alive {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl FakeListener {
    fn handler_of(&self, run: &str) -> Arc<dyn Handler> {
        lock(&self.handlers).get(run).expect("a bound run").clone()
    }
}

impl Listener for FakeListener {
    fn bind(
        &self,
        run: &str,
        dir: &Path,
        handler: Arc<dyn Handler>,
        hosts: &[String],
    ) -> io::Result<Bound> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(io::Error::other("cannot bind"));
        }
        assert!(
            !hosts.is_empty(),
            "the listener is told the hosts the CA may name"
        );
        std::fs::write(dir.join("proxy.sock"), b"")?;
        std::fs::write(dir.join("ca.pem"), b"PEM")?;
        lock(&self.handlers).insert(run.to_owned(), handler);
        self.alive.fetch_add(1, Ordering::SeqCst);
        Ok(Bound {
            socket: dir.join("proxy.sock"),
            egress_url: "http://127.0.0.1:9/".to_owned(),
            ca: dir.join("ca.pem"),
            guard: Box::new(Alive(self.alive.clone())),
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
}

impl FakeRecorded {
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

#[derive(Default)]
struct FakeHeld {
    asks: Mutex<Vec<HeldAsk>>,
    performed: Mutex<Vec<(HeldAsk, String)>>,
}
impl HeldActs for FakeHeld {
    fn raise(&self, ask: &HeldAsk) {
        lock(&self.asks).push(ask.clone());
    }
    fn perform(&self, ask: &HeldAsk, credential: Credential) -> Result<(), String> {
        lock(&self.performed).push((ask.clone(), credential.secret.into_inner()));
        Ok(())
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
    held: Arc<FakeHeld>,
    pairing: Arc<FakePairing>,
}

impl Scene {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("a temp dir");
        let dir = root.path().join("purlis-forge");
        let listener = Arc::new(FakeListener::default());
        let recorded = Arc::new(FakeRecorded::default());
        let sign_ins = Arc::new(FakeSignIns::default());
        let notices = Arc::new(FakeNotices::default());
        let held = Arc::new(FakeHeld::default());
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
                held: held.clone(),
                runner_link: pairing.clone(),
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
            held,
            pairing,
        }
    }

    fn start(&self, chat: &str, run: &str, project: &str) -> RouteParams {
        self.routes
            .start(chat, run, Path::new(project))
            .expect("the route starts")
            .expect("the project records repos")
    }

    /// The folder the route of `run` is made in.
    fn folder_of(&self, run: &str) -> PathBuf {
        self.dir.join(folder_name(run))
    }
}

/// A request from the run that holds `sentinel`, through `handler` (the one door).
fn send_via(
    handler: &dyn Handler,
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
    handler.handle(&Inbound {
        head: &head,
        headers: &headers,
        body,
    })
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
        scene.listener.handler_of(run).as_ref(),
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

fn merge(scene: &Scene, run: &str, sentinel: &str) -> Outcome {
    send(
        scene,
        run,
        sentinel,
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge?sha=zzz",
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
    let door = scene.listener.handler_of("run-1");
    assert_eq!(scene.listener.alive.load(Ordering::SeqCst), 1);

    scene.routes.stop("run-1");

    assert!(!params.socket.exists() && !params.ca.exists());
    assert!(!scene.folder_of("run-1").exists());
    assert_eq!(
        scene.listener.alive.load(Ordering::SeqCst),
        0,
        "the listener is closed"
    );
    assert!(matches!(
        send_via(
            door.as_ref(),
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
    assert!(!scene.folder_of("run-1").exists());
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
    assert!(!scene.folder_of("run-1").exists());
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
    let from_a_dead_app = scene.dir.join(format!("{gone}-run-old"));
    std::fs::create_dir(&from_a_dead_app).unwrap();
    std::fs::write(from_a_dead_app.join("proxy.sock"), b"").unwrap();
    // An app that is running (this test's parent), a name that is not ours, and a link.
    #[cfg(unix)]
    let parent = std::os::unix::process::parent_id();
    #[cfg(not(unix))]
    let parent = 1;
    let from_a_live_app = scene.dir.join(format!("{parent}-run-other"));
    std::fs::create_dir(&from_a_live_app).unwrap();
    let not_ours = scene.dir.join("notes");
    std::fs::create_dir(&not_ours).unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(elsewhere.path().join("keep"), b"x").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(elsewhere.path(), scene.dir.join(format!("{gone}-run-link")))
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
fn folder_names_are_pid_then_a_safe_run_and_only_those_are_read_back() {
    assert_eq!(pid_of_folder("123-run-1"), Some(123));
    assert_eq!(pid_of_folder("123-a_b"), Some(123));
    for not_ours in [
        "notes", "-run", "12a-run", "123-", "123-a/b", "123-..", "../123-x",
    ] {
        assert_eq!(pid_of_folder(not_ours), None, "{not_ours}");
    }
    assert!(!safe_name("../../etc").contains('/'));
}

#[cfg(unix)]
#[test]
fn a_root_that_is_a_link_is_refused_and_what_it_points_at_is_left_alone() {
    let scene = Scene::new();
    let target = tempfile::tempdir().unwrap();
    let victim = target.path().join("1-run-victim");
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
    let another = scene.dir.join(format!("{parent}-run-other"));
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
            .bind("r", root.path(), Arc::new(Nothing), &[])
            .is_err()
    );
    assert!(matches!(
        routes.for_runner(&runner(), "c", "r"),
        RunnerForward::Refused
    ));
}

struct Nothing;
impl Handler for Nothing {
    fn handle(&self, _: &Inbound<'_>) -> Outcome {
        Outcome::Refused(Refusal::UnknownChat)
    }
    fn body_need(&self, _: &RequestHead) -> BodyNeed {
        BodyNeed::Nothing
    }
    fn note_advertised(&self, _: &str, _: &str, _: Vec<String>) {}
    fn confirm_push(&self, _: &Forwarded) {}
}

// ---- credential lookup ----

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
fn a_held_act_registers_and_raises_one_ask_through_the_door_and_stays_refused_until_performed() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");

    // Twice: a chat that retries is one ask. The request came through the listener's handler.
    for _ in 0..2 {
        let outcome = merge(&scene, "run-1", &params.sentinel);
        assert!(
            matches!(outcome, Outcome::Refused(Refusal::Merge)),
            "{outcome:?}"
        );
    }

    let asks = lock(&scene.held.asks).clone();
    assert_eq!(asks.len(), 1, "{asks:?}");
    assert_eq!(asks[0].act, HeldAct::Merge);
    assert_eq!(asks[0].method, "PUT");
    assert_eq!(
        asks[0].host, "github.com",
        "the forge's host, not the API's"
    );
    assert_eq!(
        asks[0].path, "/repos/acme/app/pulls/7/merge",
        "no query in an ask"
    );
    assert_eq!(asks[0].repo.as_deref(), Some("acme/app"));
    assert_eq!(scene.routes.pending_asks("run-1"), asks);
    assert!(
        lock(&scene.held.performed).is_empty(),
        "the ask performs nothing"
    );
    assert!(
        scene
            .routes
            .refusal_message("run-1", Refusal::Merge)
            .contains("approval")
    );

    scene.routes.perform_held(asks[0].id).expect("performed");

    let done = lock(&scene.held.performed).clone();
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].1, GITHUB_TOKEN, "with the person's own credential");
    assert_eq!(
        lock(&scene.sign_ins.asked).last().unwrap(),
        &("github.com".to_owned(), "alice".to_owned()),
        "asked for the forge's host and the project's account"
    );
    assert!(
        scene.routes.perform_held(asks[0].id).is_err(),
        "an ask is performed once"
    );
    assert_eq!(lock(&scene.held.performed).len(), 1);
}

#[test]
fn only_a_registered_ask_is_performed_never_a_hand_built_one_or_one_of_an_ended_run() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");

    // An id nobody raised.
    assert!(scene.routes.perform_held(41).is_err());
    assert!(lock(&scene.held.performed).is_empty());

    // An ask whose run ended performs nothing.
    merge(&scene, "run-1", &params.sentinel);
    let id = lock(&scene.held.asks)[0].id;
    scene.routes.stop("run-1");
    assert!(scene.routes.perform_held(id).is_err());
    assert!(lock(&scene.held.performed).is_empty());
}

#[test]
fn a_press_without_a_usable_sign_in_says_so_and_performs_nothing() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");
    merge(&scene, "run-1", &params.sentinel);
    let id = lock(&scene.held.asks)[0].id;
    scene
        .sign_ins
        .fail("github.com", "alice", SignInState::Ended);

    let said = scene.routes.perform_held(id).expect_err("an ended sign-in");

    assert!(
        said.contains("has ended") && said.contains("Accounts"),
        "{said}"
    );
    assert!(lock(&scene.held.performed).is_empty());
}

#[test]
fn a_plain_refusal_raises_no_ask() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");

    let other_repo = send(
        &scene,
        "run-1",
        &params.sentinel,
        "POST",
        "api.github.com",
        "/repos/evil/app/issues/1/comments",
    );

    assert!(matches!(other_repo, Outcome::Refused(Refusal::OtherRepo)));
    assert!(lock(&scene.held.asks).is_empty());
    assert!(scene.routes.pending_asks("run-1").is_empty());
}

// ---- runners ----

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
