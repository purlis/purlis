// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
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

/// A listener that makes a socket and a CA file in the chat's folder, as A2's will, and counts
/// the guards it handed out that are still alive.
#[derive(Default)]
struct FakeListener {
    fail: AtomicBool,
    alive: Arc<AtomicUsize>,
}

struct Alive(Arc<AtomicUsize>);
impl Drop for Alive {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Listener for FakeListener {
    fn bind(&self, _: &ChatId, dir: &Path, _: Arc<ForgeProxy>, hosts: &[String]) -> io::Result<Bound> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(io::Error::other("cannot bind"));
        }
        assert!(!hosts.is_empty(), "the listener is told the hosts the CA may name");
        std::fs::write(dir.join("proxy.sock"), b"")?;
        std::fs::write(dir.join("ca.pem"), b"PEM")?;
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

/// Recorded repos and bindings, by project.
#[derive(Default)]
struct FakeRecorded {
    repos: Mutex<HashMap<PathBuf, Vec<(Kind, &'static str, &'static str)>>>,
    accounts: Mutex<HashMap<(PathBuf, String), String>>,
}

impl FakeRecorded {
    fn record(&self, project: &str, repos: Vec<(Kind, &'static str, &'static str)>) {
        lock(&self.repos).insert(PathBuf::from(project), repos);
    }
    fn bind(&self, project: &str, host: &str, account: &str) {
        lock(&self.accounts).insert((PathBuf::from(project), host.to_owned()), account.to_owned());
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
        // The API host answers for its forge's host, as a request names it.
        let host = host.strip_prefix("api.").unwrap_or(host);
        lock(&self.accounts)
            .get(&(project.to_path_buf(), host.to_owned()))
            .cloned()
    }
}

/// Sign-ins by `(host, account)`; every read is counted.
#[derive(Default)]
struct FakeSignIns {
    tokens: Mutex<HashMap<(String, String), (TokenKind, &'static str)>>,
    ended: Mutex<Vec<(String, String)>>,
    asked: Mutex<Vec<(String, String)>>,
}

impl FakeSignIns {
    fn sign_in(&self, host: &str, account: &str, token: &'static str) {
        lock(&self.tokens).insert((host.to_owned(), account.to_owned()), (TokenKind::Pat, token));
    }
    fn end(&self, host: &str, account: &str) {
        lock(&self.ended).push((host.to_owned(), account.to_owned()));
    }
    fn lookup(&self, host: &str, account: &str) -> Result<&'static str, SignInState> {
        let host = host.strip_prefix("api.").unwrap_or(host).to_owned();
        let key = (host, account.to_owned());
        if lock(&self.ended).contains(&key) {
            return Err(SignInState::Ended);
        }
        lock(&self.tokens).get(&key).map(|(_, t)| *t).ok_or(SignInState::Missing)
    }
}

impl Credentials for FakeSignIns {
    fn credential(&self, host: &str, account: &str) -> Result<Credential, SignInState> {
        lock(&self.asked).push((host.to_owned(), account.to_owned()));
        self.lookup(host, account).map(|token| Credential {
            token_kind: TokenKind::Pat,
            secret: Secret::new(token.to_owned()),
        })
    }
    fn check(&self, host: &str, account: &str) -> Result<(), SignInState> {
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

struct FakeLink(AtomicBool);
impl RunnerLink for FakeLink {
    fn reachable(&self, _: &str) -> bool {
        self.0.load(Ordering::SeqCst)
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
    link: Arc<FakeLink>,
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
        let link = Arc::new(FakeLink(AtomicBool::new(false)));
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
                runner_link: link.clone(),
            },
        );
        Self { _root: root, dir, routes, listener, recorded, sign_ins, notices, held, link }
    }

    fn start(&self, chat: &str, project: &str) -> RouteParams {
        self.routes
            .start(chat, Path::new(project))
            .expect("the route starts")
            .expect("the project records repos")
    }
}

/// A comment on acme/app, from the chat that holds `sentinel`.
fn send(scene: &Scene, chat: &str, sentinel: &str, method: &str, host: &str, target: &str) -> Outcome {
    let head = RequestHead::new(method, host, 443, target).with_content_type(Some("application/json"));
    let headers = vec![
        ("Host".to_owned(), host.to_owned()),
        ("Authorization".to_owned(), format!("Bearer {sentinel}")),
        ("Content-Type".to_owned(), "application/json".to_owned()),
    ];
    let body: &[u8] = b"{\"body\":\"hello\"}";
    scene.routes.handle(chat, &Inbound { head: &head, headers: &headers, body })
}

fn comment(scene: &Scene, chat: &str, sentinel: &str) -> Outcome {
    send(scene, chat, sentinel, "POST", "api.github.com", "/repos/acme/app/issues/7/comments")
}

fn bearer(outcome: &Outcome) -> Option<String> {
    let Outcome::Forward(forwarded) = outcome else { return None };
    forwarded
        .headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.clone())
}

// ---- lifecycle ----

#[test]
fn a_chat_that_starts_is_handed_its_socket_ca_sentinel_helper_and_recorded_repos() {
    let scene = Scene::new();

    let params = scene.start("chat-1", "/p/one");

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
    assert!(!format!("{params:?}").contains(&params.sentinel), "Debug shows no sentinel");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(params.socket.parent().unwrap()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "the chat's folder is the app's alone");
    }
}

#[test]
fn a_project_that_records_no_repo_gets_no_route() {
    let scene = Scene::new();

    let started = scene.routes.start("chat-1", Path::new("/p/none")).expect("no error");

    assert!(started.is_none());
    assert!(!scene.routes.is_live("chat-1"));
    assert!(!scene.dir.join("chat-1").exists());
}

#[test]
fn a_chat_that_ends_leaves_no_socket_no_ca_no_listener_and_a_dead_sentinel() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "/p/one");
    assert!(matches!(comment(&scene, "chat-1", &params.sentinel), Outcome::Forward(_)));
    assert_eq!(scene.listener.alive.load(Ordering::SeqCst), 1);

    scene.routes.stop("chat-1");

    assert!(!params.socket.exists() && !params.ca.exists());
    assert!(!params.socket.parent().unwrap().exists());
    assert_eq!(scene.listener.alive.load(Ordering::SeqCst), 0, "the listener is closed");
    assert!(matches!(
        comment(&scene, "chat-1", &params.sentinel),
        Outcome::Refused(Refusal::UnknownChat)
    ));
}

#[test]
fn a_listener_that_cannot_bind_leaves_nothing_behind() {
    let scene = Scene::new();
    scene.listener.fail.store(true, Ordering::SeqCst);

    let started = scene.routes.start("chat-1", Path::new("/p/one"));

    assert!(started.is_err());
    assert!(!scene.routes.is_live("chat-1"));
    assert!(!scene.dir.join("chat-1").exists());
}

#[test]
fn a_chat_that_starts_again_replaces_its_route_and_the_old_sentinel_stops() {
    let scene = Scene::new();
    let first = scene.start("chat-1", "/p/one");

    let second = scene.start("chat-1", "/p/one");

    assert_ne!(first.sentinel, second.sentinel);
    assert_eq!(scene.listener.alive.load(Ordering::SeqCst), 1);
    assert!(matches!(
        comment(&scene, "chat-1", &first.sentinel),
        Outcome::Refused(Refusal::WrongSentinel)
    ));
    assert!(matches!(comment(&scene, "chat-1", &second.sentinel), Outcome::Forward(_)));
}

#[test]
fn an_app_that_crashed_leaves_folders_the_next_start_sweeps() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "/p/one");
    let leftover = params.socket.parent().unwrap().to_owned();
    // A crash runs no destructor.
    let Scene { routes, _root, .. } = scene;
    std::mem::forget(routes);
    assert!(leftover.exists(), "the crash left its folder");

    let restarted = ForgeRoutes::new(leftover.parent().unwrap().to_owned(), PathBuf::new(), Seams::stubs());

    assert!(!leftover.exists(), "the next app removes what no chat can use");
    drop(restarted);
    drop(_root);
}

#[test]
fn quitting_stops_every_chats_route_and_dropping_the_routes_does_too() {
    let scene = Scene::new();
    let a = scene.start("chat-1", "/p/one");
    let b = scene.start("chat-2", "/p/one");

    scene.routes.stop_all();

    assert!(!a.socket.exists() && !b.socket.exists());
    assert_eq!(scene.listener.alive.load(Ordering::SeqCst), 0);
    assert!(std::fs::read_dir(&scene.dir).unwrap().next().is_none());

    let c = scene.start("chat-3", "/p/one");
    let folder = c.socket.parent().unwrap().to_owned();
    let Scene { routes, _root, .. } = scene;
    drop(routes);
    assert!(!folder.exists(), "a dropped app leaves no folder");
    drop(_root);
}

#[test]
fn stub_seams_fail_closed_a_chat_has_no_route_without_a_listener() {
    let root = tempfile::tempdir().unwrap();
    let routes = ForgeRoutes::new(root.path().join("r"), PathBuf::new(), Seams::stubs());

    // Nothing is recorded, so there is no route; and were something recorded, NoListener refuses.
    assert!(routes.start("c", Path::new("/p")).unwrap().is_none());
    assert!(NoListener.bind(&ChatId::new("c"), root.path(), Arc::new(ForgeProxy::new(Box::new(NoSignInsSource), Box::new(NoObjects))), &[]).is_err());
}

struct NoSignInsSource;
impl CredentialSource for NoSignInsSource {
    fn credential_for(&self, _: &CredentialRequest) -> Result<Credential, CredentialError> {
        Err(CredentialError::Unavailable)
    }
}

// ---- credential lookup ----

#[test]
fn each_host_is_answered_with_the_credential_of_the_account_its_project_is_bound_to() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "/p/one");

    let github = comment(&scene, "chat-1", &params.sentinel);
    let gitlab = send(
        &scene,
        "chat-1",
        &params.sentinel,
        "POST",
        "gitlab.com",
        "/api/v4/projects/group%2Fproj/issues/3/notes",
    );

    assert!(bearer(&github).is_some_and(|v| v.contains(GITHUB_TOKEN)), "{github:?}");
    let gitlab_headers = match &gitlab {
        Outcome::Forward(f) => f.headers.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>().join("|"),
        other => panic!("{other:?}"),
    };
    assert!(gitlab_headers.contains(GITLAB_TOKEN), "{gitlab_headers}");
    assert_eq!(
        *lock(&scene.sign_ins.asked),
        vec![
            ("api.github.com".to_owned(), "alice".to_owned()),
            ("gitlab.com".to_owned(), "alice-gl".to_owned())
        ]
    );
}

#[test]
fn two_projects_on_one_host_use_their_own_accounts() {
    let scene = Scene::new();
    scene.recorded.record("/p/two", vec![(Kind::GitHub, "github.com", "acme/app")]);
    scene.recorded.bind("/p/two", "github.com", "bob");
    scene.sign_ins.sign_in("github.com", "bob", "REAL-bob-token-1");
    let one = scene.start("chat-1", "/p/one");
    let two = scene.start("chat-2", "/p/two");

    let for_one = comment(&scene, "chat-1", &one.sentinel);
    let for_two = comment(&scene, "chat-2", &two.sentinel);

    assert!(bearer(&for_one).unwrap().contains(GITHUB_TOKEN));
    assert!(bearer(&for_two).unwrap().contains("REAL-bob-token-1"));
}

#[test]
fn a_missing_sign_in_is_refused_in_words_that_name_the_sign_in_step_and_noticed_once() {
    let scene = Scene::new();
    scene.sign_ins.tokens.lock().unwrap().clear();
    let params = scene.start("chat-1", "/p/one");

    for _ in 0..3 {
        let outcome = comment(&scene, "chat-1", &params.sentinel);
        assert!(matches!(outcome, Outcome::Refused(Refusal::NoCredential)), "{outcome:?}");
    }

    let said = scene.routes.refusal_message("chat-1", Refusal::NoCredential);
    assert!(said.contains("github.com") && said.contains("Sign in") && said.contains("Accounts"), "{said}");
    assert_eq!(
        *lock(&scene.notices.0),
        vec![("chat-1".to_owned(), "api.github.com".to_owned(), SignInState::Missing)],
        "one Notice for the chat, however many requests"
    );
}

#[test]
fn an_ended_sign_in_says_it_ended_and_a_host_with_no_bound_account_is_missing() {
    let scene = Scene::new();
    scene.sign_ins.end("github.com", "alice");
    let params = scene.start("chat-1", "/p/one");

    assert!(matches!(
        comment(&scene, "chat-1", &params.sentinel),
        Outcome::Refused(Refusal::NoCredential)
    ));
    assert!(scene.routes.refusal_message("chat-1", Refusal::NoCredential).contains("has ended"));

    scene.recorded.accounts.lock().unwrap().clear();
    let other = scene.start("chat-2", "/p/one");
    assert!(matches!(
        comment(&scene, "chat-2", &other.sentinel),
        Outcome::Refused(Refusal::NoCredential)
    ));
    assert!(scene.routes.refusal_message("chat-2", Refusal::NoCredential).contains("no sign-in"));
    assert_eq!(lock(&scene.notices.0).len(), 2, "each chat is told once");
}

// ---- held acts ----

#[test]
fn a_held_act_raises_the_ask_and_stays_refused_until_the_person_presses() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "/p/one");

    let outcome = send(
        &scene,
        "chat-1",
        &params.sentinel,
        "PUT",
        "api.github.com",
        "/repos/acme/app/pulls/7/merge?token=zzz",
    );

    assert!(matches!(outcome, Outcome::Refused(Refusal::Merge)), "{outcome:?}");
    let asks = lock(&scene.held.asks).clone();
    assert_eq!(asks.len(), 1);
    assert_eq!(asks[0].act, HeldAct::Merge);
    assert_eq!(asks[0].method, "PUT");
    assert_eq!(asks[0].path, "/repos/acme/app/pulls/7/merge", "no query in an ask");
    assert!(lock(&scene.held.performed).is_empty(), "nothing is performed by the ask");
    assert!(scene.routes.refusal_message("chat-1", Refusal::Merge).contains("approval"));

    scene.routes.press(&asks[0]).expect("the press performs");

    let done = lock(&scene.held.performed).clone();
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].1, GITHUB_TOKEN, "performed with the person's own credential");
}

#[test]
fn a_plain_refusal_raises_no_ask_and_a_press_without_a_sign_in_says_so() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "/p/one");
    let other_repo = send(&scene, "chat-1", &params.sentinel, "POST", "api.github.com", "/repos/evil/app/issues/1/comments");
    assert!(matches!(other_repo, Outcome::Refused(Refusal::OtherRepo)));
    assert!(lock(&scene.held.asks).is_empty());

    let ask = HeldAsk {
        chat: "chat-1".to_owned(),
        host: "github.com".to_owned(),
        method: "PUT".to_owned(),
        path: "/repos/acme/app/pulls/7/merge".to_owned(),
        act: HeldAct::Merge,
    };
    scene.sign_ins.end("github.com", "alice");
    let said = scene.routes.press(&ask).expect_err("an ended sign-in");
    assert!(said.contains("has ended") && said.contains("Accounts"), "{said}");
    assert!(lock(&scene.held.performed).is_empty());

    scene.routes.stop("chat-1");
    assert!(scene.routes.press(&ask).is_err(), "a chat that ended has nothing to allow");
}

// ---- runners ----

#[test]
fn a_runner_chat_is_forwarded_to_the_desktop_proxy_when_its_link_is_up() {
    let scene = Scene::new();
    scene.link.0.store(true, Ordering::SeqCst);

    let forward = scene.routes.for_runner("job-1", Path::new("/p/one"));

    let RunnerForward::Desktop(params) = forward else { panic!("{forward:?}") };
    assert!(scene.routes.is_live("job-1"));
    assert!(params.sentinel.starts_with("purlis-sentinel-"));
}

#[test]
fn a_runner_chat_keeps_token_connections_without_a_link_or_a_proxy() {
    let scene = Scene::new();

    let without_link = scene.routes.for_runner("job-1", Path::new("/p/one"));
    assert!(matches!(without_link, RunnerForward::TokenConnections));
    assert!(!scene.routes.is_live("job-1"));

    scene.link.0.store(true, Ordering::SeqCst);
    scene.listener.fail.store(true, Ordering::SeqCst);
    let without_proxy = scene.routes.for_runner("job-2", Path::new("/p/one"));
    assert!(matches!(without_proxy, RunnerForward::TokenConnections));
    assert!(!scene.routes.is_live("job-2"));
}

// ---- audit and doctor ----

#[test]
fn the_audit_has_a_line_per_request_and_never_a_token_or_a_path() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "/p/one");
    comment(&scene, "chat-1", &params.sentinel);
    send(&scene, "chat-1", &params.sentinel, "PUT", "api.github.com", "/repos/acme/app/pulls/7/merge?token=zzz");

    let lines = scene.routes.audit_lines("chat-1");

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
    assert!(ok.detail.contains("github.com") && ok.detail.contains("gitlab.com"), "{ok:?}");

    scene.sign_ins.end("gitlab.com", "alice-gl");
    let one_missing = scene.routes.row(project);
    assert!(one_missing.problem && one_missing.detail.contains("gitlab.com"), "{one_missing:?}");
    assert!(!one_missing.detail.contains("github.com"));

    scene.listener.fail.store(true, Ordering::SeqCst);
    let down = scene.routes.row(project);
    assert!(down.problem && down.detail.contains("not running"), "{down:?}");

    assert!(!scene.routes.row(Path::new("/p/none")).problem, "no recorded repo, nothing to check");
}

// ---- the train switch ----

#[test]
fn the_train_switch_is_one_line_in_the_apps_manifest() {
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    let defaults: Vec<&str> = manifest
        .lines()
        .filter(|line| line.trim_start().starts_with("default = "))
        .collect();
    assert_eq!(defaults, vec!["default = [\"forge-wiring\"]"], "the one place the train is on");
    assert!(manifest.contains("\nforge-wiring = ["), "the feature is declared");
}
