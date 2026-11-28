// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use purlis_core::forge::proxy::{ForgeRepoId, Kind, NoCredentials, RequestHead, TokenKind};
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
    /// How many binds from now fail before they bind again.
    fail_next: AtomicUsize,
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
        if self
            .fail_next
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
        {
            return Err(io::Error::other("cannot bind, this once"));
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
    /// The lines the app recorded, oldest first: `(project, chat, branch, tip)`. For one branch
    /// the last line decides, as the real record does; a line with no chat is a `gone` line.
    cut: Mutex<Vec<(PathBuf, String, String, String)>>,
}

impl FakeRecorded {
    /// The branches whose last recorded line is `chat`'s.
    fn own_of(&self, chat: &str) -> Vec<String> {
        let lines = lock(&self.cut);
        let mut branches: Vec<String> = Vec::new();
        for (_, _, branch, _) in lines.iter() {
            if !branches.contains(branch) {
                branches.push(branch.clone());
            }
        }
        branches
            .into_iter()
            .filter(|branch| {
                lines
                    .iter()
                    .rev()
                    .find(|(_, _, b, _)| b == branch)
                    .is_some_and(|(_, by, _, _)| by == chat)
            })
            .collect()
    }
    /// The tip `chat` last left on `branch`, as `None` if the last line is not `chat`'s.
    fn tip_of(&self, project: &Path, chat: &str, branch: &str) -> Option<String> {
        lock(&self.cut)
            .iter()
            .rev()
            .find(|(_, _, b, _)| b == branch)
            .filter(|(of, by, _, _)| of == project && by == chat)
            .map(|(_, _, _, tip)| tip.clone())
    }
    /// `chat`'s route made `branch` earlier, and left it at `a` * 40, the id a pushed update names.
    fn cut_for(&self, project: &str, chat: &str, branch: &str) {
        self.cut_at(project, chat, branch, &"a".repeat(40));
    }
    fn cut_at(&self, project: &str, chat: &str, branch: &str, tip: &str) {
        lock(&self.cut).push((
            PathBuf::from(project),
            chat.to_owned(),
            branch.to_owned(),
            tip.to_owned(),
        ));
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
    fn own_tip(
        &self,
        project: &Path,
        chat: &str,
        _host: &str,
        _repo_id: u64,
        branch: &str,
    ) -> Option<purlis_core::forge::createdbranches::OwnedTip> {
        self.tip_of(project, chat, branch).map(|tip| {
            purlis_core::forge::createdbranches::OwnedTip {
                tip,
                fast_forward_only: false,
            }
        })
    }
    fn own_branches(&self, project: &Path, chat: &str, _host: &str, _repo_id: u64) -> Vec<String> {
        self.own_of(chat)
            .into_iter()
            .filter(|branch| self.tip_of(project, chat, branch).is_some())
            .collect()
    }
    fn record_created(&self, project: &Path, chat: &str, created: &[CreatedBranch]) {
        // As the real record: a branch whose repo id the route does not know is not recorded.
        for made in created.iter().filter(|made| made.repo_id.is_some()) {
            lock(&self.cut).push((
                project.to_owned(),
                chat.to_owned(),
                made.branch.clone(),
                made.tip.clone(),
            ));
        }
    }
    fn forget_created(&self, gone: &[CreatedBranch]) {
        for ended in gone {
            lock(&self.cut).push((
                PathBuf::new(),
                String::new(),
                ended.branch.clone(),
                String::new(),
            ));
        }
    }
    fn repos(&self, project: &Path) -> Vec<AllowedRepo> {
        lock(&self.repos)
            .get(project)
            .into_iter()
            .flatten()
            .map(|(kind, host, path)| {
                // As the app's own: a recorded repo carries the forge's id, the record's key.
                AllowedRepo::new(*kind, host, 443, path, "main")
                    .unwrap()
                    .with_forge_id(ForgeRepoId::for_a_test(
                        7_000 + u64::from(*kind == Kind::GitLab),
                    ))
            })
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
            account: Some(account.to_owned()),
        })
    }
    fn check(&self, host: &str, account: &str) -> Result<(), SignInFailure> {
        self.lookup(host, account).map(|_| ())
    }
}

#[derive(Default)]
struct FakeNotices {
    needed: Mutex<Vec<(u32, String, SignInState)>>,
    ok: Mutex<Vec<(u32, String)>>,
    /// Whether the window takes what it is told (false: no pane to say it on).
    refuse: Mutex<bool>,
}
impl Notices for FakeNotices {
    fn sign_in_needed(&self, _: &Path, session: u32, host: &str, state: SignInState) -> bool {
        if *lock(&self.refuse) {
            return false;
        }
        lock(&self.needed).push((session, host.to_owned(), state));
        true
    }

    fn sign_in_ok(&self, _: &Path, session: u32, host: &str) {
        lock(&self.ok).push((session, host.to_owned()));
    }
}

/// The pairing: the one device and key it paired, and the project each run of it belongs to.
struct FakePairing {
    paired: RunnerPeer,
    runs: Mutex<HashMap<String, PathBuf>>,
    /// The one chat the runner is paired for; none serves any chat.
    chat: Mutex<Option<String>>,
}
impl RunnerLink for FakePairing {
    fn paired(&self, peer: &RunnerPeer) -> bool {
        *peer == self.paired
    }
    fn project_of(&self, peer: &RunnerPeer, chat: &str, run: &str) -> Option<PathBuf> {
        (*peer == self.paired && lock(&self.chat).as_deref().is_none_or(|own| own == chat))
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
        Self::with_seams(gate, pin, Seams::stubs().merges)
    }

    fn with_seams(gate: Option<Arc<dyn Gate>>, pin: PinOf, merges: MergesOf) -> Self {
        let root = tempfile::tempdir().expect("a temp dir");
        let dir = root.path().join("purlis-forge");
        let listener = Arc::new(FakeListener::default());
        let recorded = Arc::new(FakeRecorded::default());
        let sign_ins = Arc::new(FakeSignIns::default());
        let notices = Arc::new(FakeNotices::default());
        let pairing = Arc::new(FakePairing {
            paired: runner(),
            runs: Mutex::new(HashMap::new()),
            chat: Mutex::new(None),
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
                merges,
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

    /// A route started and open as chat session `session`, as a committed run's is.
    fn open(&self, chat: &str, run: &str, project: &str, session: u32) -> RouteParams {
        let params = self.start(chat, run, project);
        self.routes
            .sessions()
            .of_run
            .insert(session, run.to_owned());
        params
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

// ---- the merge stance is made per chat ----

#[test]
fn a_chats_merge_stance_is_made_with_the_policy_of_the_chain_that_dispatched_it() {
    let chains: Arc<Mutex<Vec<Option<String>>>> = Arc::default();
    let kept = chains.clone();
    let scene = Scene::with_seams(
        None,
        Seams::stubs().pin,
        Arc::new(move |_, chain| {
            kept.lock().unwrap().push(chain.map(str::to_owned));
            Arc::new(purlis_core::forge::mergestance::HoldMerges)
        }),
    );

    let _two = scene
        .routes
        .guard("chat-1", "run-1", Path::new("/p/one"), None);
    let _two = scene.routes.guard(
        "chat-2",
        "run-2",
        Path::new("/p/one"),
        Some("merge=enforce"),
    );

    assert_eq!(
        *chains.lock().unwrap(),
        [None, Some("merge=enforce".to_owned())]
    );
}

#[test]
fn a_runners_chat_is_made_its_merge_stance_with_the_strict_chain() {
    // A runner's chat carries no record of its chain here, so `for_runner` holds its merges: the
    // chain it names is `strict`, and no project policy, opted in and accepted or not, loosens it.
    let chains: Arc<Mutex<Vec<Option<String>>>> = Arc::default();
    let kept = chains.clone();
    let scene = Scene::with_seams(
        None,
        Seams::stubs().pin,
        Arc::new(move |_, chain| {
            kept.lock().unwrap().push(chain.map(str::to_owned));
            Arc::new(purlis_core::forge::mergestance::HoldMerges)
        }),
    );
    scene
        .pairing
        .runs
        .lock()
        .unwrap()
        .insert("run-r".to_owned(), PathBuf::from("/p/one"));

    let forwarded = scene.routes.for_runner(&runner(), "chat-r", "run-r");

    assert!(matches!(forwarded, RunnerForward::Desktop(_)));
    let strict = purlis_core::enforcement::Resolved::strict().spelled();
    assert_eq!(*chains.lock().unwrap(), [Some(strict.clone())]);
    let opened_up = purlis_core::enforcement::Resolved::all(purlis_core::enforcement::Mode::Off)
        .for_chat(Some(&strict));
    assert_eq!(
        opened_up.mode(purlis_core::enforcement::Family::GuardsMerge),
        purlis_core::enforcement::Mode::Enforce
    );
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
    let old = scene
        .routes
        .guard("chat-1", "run-a", Path::new("/p/one"), None);
    old.commit(1);
    // The restart: the same chat, a new run B, started before the old one ends.
    let new = scene
        .routes
        .guard("chat-1", "run-b", Path::new("/p/one"), None);
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
        let early = scene
            .routes
            .guard("chat-1", "run-1", Path::new("/p/one"), None);
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
        .guard("chat-1", "run-2", Path::new("/p/one"), None)
        .commit(9);
    assert!(scene.routes.is_live("run-2"));
    scene.routes.stop_session(9);
    assert!(!scene.routes.is_live("run-2"));

    // A project with nothing recorded has a guard with no route, and committing it is harmless.
    let none = scene
        .routes
        .guard("chat-1", "run-3", Path::new("/p/none"), None);
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
                    Box::new(purlis_core::forge::proxy::NoObjects)
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
    let params = scene.open("chat-1", "run-1", "/p/one", 5);

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
        *lock(&scene.notices.needed),
        vec![(5, "github.com".to_owned(), SignInState::Missing)],
        "one Notice for the chat, however many requests"
    );

    // The chat restarts while it still has the first run: its new session has a pane of its own
    // that shows nothing yet, so it is told, once.
    let second = scene.open("chat-1", "run-2", "/p/one", 6);
    comment(&scene, "run-2", &second.sentinel);
    comment(&scene, "run-2", &second.sentinel);
    assert_eq!(
        *lock(&scene.notices.needed),
        vec![
            (5, "github.com".to_owned(), SignInState::Missing),
            (6, "github.com".to_owned(), SignInState::Missing),
        ]
    );
}

fn gitlab_note(scene: &Scene, run: &str, sentinel: &str) -> Outcome {
    send(
        scene,
        run,
        sentinel,
        "POST",
        "gitlab.com",
        "/api/v4/projects/group%2Fproj/issues/3/notes",
    )
}

#[test]
fn a_second_host_and_a_sign_in_that_ends_later_are_each_told() {
    let scene = Scene::new();
    lock(&scene.sign_ins.tokens).clear();
    let params = scene.open("chat-1", "run-1", "/p/one", 5);

    comment(&scene, "run-1", &params.sentinel);
    gitlab_note(&scene, "run-1", &params.sentinel);
    comment(&scene, "run-1", &params.sentinel);
    // github.com's sign-in was missing; now it is one that ended.
    scene
        .sign_ins
        .fail("github.com", "alice", SignInState::Ended);
    comment(&scene, "run-1", &params.sentinel);

    assert_eq!(
        *lock(&scene.notices.needed),
        vec![
            (5, "github.com".to_owned(), SignInState::Missing),
            (5, "gitlab.com".to_owned(), SignInState::Missing),
            (5, "github.com".to_owned(), SignInState::Ended),
        ]
    );
}

#[test]
fn a_successful_sign_in_takes_the_notice_down_and_a_later_loss_is_told_again() {
    let scene = Scene::new();
    lock(&scene.sign_ins.tokens).clear();
    let params = scene.open("chat-1", "run-1", "/p/one", 5);
    comment(&scene, "run-1", &params.sentinel);
    assert!(lock(&scene.notices.ok).is_empty());

    // The person signs in: the next request is given a credential, and the Notice goes.
    scene.sign_ins.sign_in("github.com", "alice", GITHUB_TOKEN);
    let outcome = comment(&scene, "run-1", &params.sentinel);
    assert!(bearer(&outcome).is_some(), "{outcome:?}");
    comment(&scene, "run-1", &params.sentinel);
    assert_eq!(
        *lock(&scene.notices.ok),
        vec![(5, "github.com".to_owned())],
        "taken down once, not on every request"
    );

    // It ends again later: that is told again.
    scene
        .sign_ins
        .fail("github.com", "alice", SignInState::Ended);
    comment(&scene, "run-1", &params.sentinel);
    assert_eq!(lock(&scene.notices.needed).len(), 2);
}

#[test]
fn one_hosts_sign_in_leaves_another_hosts_told_state_alone() {
    let scene = Scene::new();
    lock(&scene.sign_ins.tokens).clear();
    let params = scene.open("chat-1", "run-1", "/p/one", 5);
    comment(&scene, "run-1", &params.sentinel);
    gitlab_note(&scene, "run-1", &params.sentinel);
    // gitlab.com is signed in: its Notice goes, github.com's is still missing.
    scene
        .sign_ins
        .sign_in("gitlab.com", "alice-gl", GITLAB_TOKEN);
    gitlab_note(&scene, "run-1", &params.sentinel);
    assert_eq!(*lock(&scene.notices.ok), vec![(5, "gitlab.com".to_owned())]);
    comment(&scene, "run-1", &params.sentinel);
    assert_eq!(
        lock(&scene.notices.needed).len(),
        2,
        "github.com's is not told again while it stands"
    );
}

#[test]
fn missing_then_ended_then_missing_on_one_host_is_told_each_time() {
    let scene = Scene::new();
    lock(&scene.sign_ins.tokens).clear();
    let params = scene.open("chat-1", "run-1", "/p/one", 5);
    comment(&scene, "run-1", &params.sentinel);
    scene
        .sign_ins
        .fail("github.com", "alice", SignInState::Ended);
    comment(&scene, "run-1", &params.sentinel);
    lock(&scene.sign_ins.failing).clear();
    lock(&scene.sign_ins.tokens).clear();
    comment(&scene, "run-1", &params.sentinel);
    assert_eq!(
        *lock(&scene.notices.needed),
        vec![
            (5, "github.com".to_owned(), SignInState::Missing),
            (5, "github.com".to_owned(), SignInState::Ended),
            (5, "github.com".to_owned(), SignInState::Missing),
        ]
    );
}

#[test]
fn a_notice_with_no_pane_to_say_it_on_is_not_counted_as_told() {
    let scene = Scene::new();
    lock(&scene.sign_ins.tokens).clear();
    let params = scene.open("chat-1", "run-1", "/p/one", 5);
    *lock(&scene.notices.refuse) = true;
    comment(&scene, "run-1", &params.sentinel);
    assert!(lock(&scene.notices.needed).is_empty());

    *lock(&scene.notices.refuse) = false;
    comment(&scene, "run-1", &params.sentinel);
    assert_eq!(
        lock(&scene.notices.needed).len(),
        1,
        "told on the next request"
    );
}

#[test]
fn an_ended_sign_in_is_told_as_ended_and_a_chat_not_yet_open_is_told_when_it_opens() {
    let scene = Scene::new();
    scene
        .sign_ins
        .fail("github.com", "alice", SignInState::Ended);

    // Before the chat opens there is no pane to say it on: nothing is told, and the chat is not
    // counted as told.
    let params = scene.start("chat-1", "run-1", "/p/one");
    comment(&scene, "run-1", &params.sentinel);
    assert!(lock(&scene.notices.needed).is_empty());

    // Once it is open, its next request tells it once, as ended.
    scene.routes.sessions().of_run.insert(3, "run-1".to_owned());
    comment(&scene, "run-1", &params.sentinel);
    comment(&scene, "run-1", &params.sentinel);
    assert_eq!(
        *lock(&scene.notices.needed),
        vec![(3, "github.com".to_owned(), SignInState::Ended)]
    );
}

#[test]
fn an_ended_sign_in_says_it_ended_and_a_host_with_no_bound_account_is_missing() {
    let scene = Scene::new();
    scene
        .sign_ins
        .fail("github.com", "alice", SignInState::Ended);
    let params = scene.open("chat-1", "run-1", "/p/one", 1);

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
    let other = scene.open("chat-2", "run-2", "/p/one", 2);
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
    assert_eq!(
        lock(&scene.notices.needed).len(),
        2,
        "each chat is told once"
    );
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
        lock(&scene.notices.needed).is_empty(),
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
fn a_paired_runner_of_one_chat_asking_as_another_is_refused_and_no_route_is_made() {
    let scene = Scene::new();
    lock(&scene.pairing.runs).insert("job-1".to_owned(), PathBuf::from("/p/one"));
    *lock(&scene.pairing.chat) = Some("chat-x".to_owned());

    let other = scene.routes.for_runner(&runner(), "chat-y", "job-1");
    assert!(matches!(other, RunnerForward::Refused), "{other:?}");
    assert!(!scene.routes.is_live("job-1"));

    let own = scene.routes.for_runner(&runner(), "chat-x", "job-1");
    assert!(matches!(own, RunnerForward::Desktop(_)), "{own:?}");
}

#[test]
fn a_run_live_for_a_desktop_chat_or_another_peer_is_not_taken_over_by_a_runner() {
    let scene = Scene::new();
    lock(&scene.pairing.runs).insert("job-1".to_owned(), PathBuf::from("/p/one"));
    lock(&scene.pairing.runs).insert("job-2".to_owned(), PathBuf::from("/p/one"));

    // A desktop chat's run: the runner asking for its id is refused, and the route stays.
    let desktop = scene
        .routes
        .start("chat-d", "job-1", Path::new("/p/one"))
        .unwrap()
        .expect("a route");
    let taken = scene.routes.for_runner(&runner(), "chat-9", "job-1");
    assert!(matches!(taken, RunnerForward::Refused), "{taken:?}");
    assert!(scene.routes.is_live("job-1"));
    assert!(matches!(
        comment(&scene, "job-1", &desktop.sentinel),
        Outcome::Forward(_)
    ));

    // Another peer's run, held live by a route of its own device.
    scene
        .routes
        .start_for(
            "chat-o",
            "job-2",
            Path::new("/p/one"),
            Some("runner-2".to_owned()),
            None,
        )
        .unwrap();
    let taken = scene.routes.for_runner(&runner(), "chat-9", "job-2");
    assert!(matches!(taken, RunnerForward::Refused), "{taken:?}");
    scene.routes.stop_peer("runner-2");
    assert!(!scene.routes.is_live("job-2"));

    // Its own run, asked again, replaces its own route.
    let first = scene.routes.for_runner(&runner(), "chat-9", "job-2");
    assert!(matches!(first, RunnerForward::Desktop(_)), "{first:?}");
    let again = scene.routes.for_runner(&runner(), "chat-9", "job-2");
    assert!(matches!(again, RunnerForward::Desktop(_)), "{again:?}");
}

#[test]
fn two_peers_asking_for_one_run_at_once_leave_it_to_exactly_one() {
    // The check of who owns a run and the insert of its route are one step under `starting`: of
    // two peers starting the same run together, one makes the route and the other finds it taken,
    // and the route that stays is the winner's. Without the lock both pass the check.
    for _ in 0..25 {
        let scene = Scene::new();
        lock(&scene.pairing.runs).insert("job-1".to_owned(), PathBuf::from("/p/one"));
        let barrier = std::sync::Barrier::new(2);
        let outcomes: Vec<(&str, bool)> = std::thread::scope(|scope| {
            let routes = &scene.routes;
            let barrier = &barrier;
            ["runner-a", "runner-b"]
                .into_iter()
                .map(|device| {
                    scope.spawn(move || {
                        barrier.wait();
                        let started = routes
                            .start_for(
                                "chat-9",
                                "job-1",
                                Path::new("/p/one"),
                                Some(device.to_owned()),
                                None,
                            )
                            .unwrap();
                        (device, matches!(started, Started::Route(Some(_))))
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect()
        });
        let made: Vec<&str> = outcomes
            .iter()
            .filter(|(_, made)| *made)
            .map(|(device, _)| *device)
            .collect();
        assert_eq!(made.len(), 1, "{outcomes:?}");
        assert_eq!(
            scene.routes.live().get("job-1").unwrap().peer.as_deref(),
            Some(made[0])
        );
    }
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
    assert!(lines[0].contains("POST api.github.com acme/app") && lines[0].contains("allowed"));
    assert!(
        lines[1].contains("blocked (merging is not allowed from a chat)"),
        "{lines:?}"
    );
    assert!(!lines.join("\n").contains("Refused"), "{lines:?}");
    let all = lines.join("\n");
    for secret in [GITHUB_TOKEN, params.sentinel.as_str(), "zzz", "/pulls/7"] {
        assert!(!all.contains(secret), "{secret} is in the audit: {all}");
    }
}

#[test]
fn the_doctor_lists_what_a_projects_running_chats_were_refused_and_never_a_token() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");
    let quiet = scene.routes.refusals_row(Path::new("/p/one"));
    assert!(quiet.detail.contains("no running chat"), "{quiet:?}");

    comment(&scene, "run-1", &params.sentinel);
    merge(&scene, "run-1", &params.sentinel);
    let row = scene.routes.refusals_row(Path::new("/p/one"));

    // The refusal and its rule, under the chat's name; the allowed comment is not listed. A
    // merge is refused before its repo and kind are read, so both show as `-`.
    assert!(
        row.detail.contains("chat-1: PUT api.github.com - -")
            && row
                .detail
                .contains("blocked (merging is not allowed from a chat)"),
        "{row:?}"
    );
    assert!(
        !row.detail.contains("allowed\n") && !row.detail.contains(" allowed;"),
        "{row:?}"
    );
    assert!(
        !row.detail.contains("Refused") && !row.detail.contains("Merge)"),
        "no internal type names: {row:?}"
    );
    let all = format!("{} {}", row.detail, row.hint);
    for secret in [GITHUB_TOKEN, params.sentinel.as_str(), "zzz", "/pulls/7"] {
        assert!(!all.contains(secret), "{secret} is in the row: {all}");
    }
    // Another project's row says nothing of this chat.
    let other = scene.routes.refusals_row(Path::new("/p/two"));
    assert!(!other.detail.contains("chat-1"), "{other:?}");
}

#[test]
fn a_hostile_host_is_never_shown_in_the_audit_or_the_doctor_row() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");
    let hostile = [
        "evil.example/pulls/7?x=1",
        "api.github.com; chat-9: PUT api.github.com acme/app merge allowed",
        "a\u{202e}moc.buhtig.ipa",
        "api.github.com:99999999",
        "api.github.com:80/x",
        "",
        params.sentinel.as_str(),
    ];
    for host in hostile {
        send_raw(
            &scene,
            "run-1",
            &params.sentinel,
            ("GET", host, "/user"),
            "application/json",
            b"",
        );
    }
    let lines = scene.routes.audit_lines("run-1");
    let row = scene.routes.refusals_row(Path::new("/p/one"));
    for text in [lines.join("\n"), row.detail.clone()] {
        for bad in [
            "evil",
            "pulls",
            "chat-9",
            "\u{202e}",
            "99999999",
            "/x",
            "purlis-sentinel-",
        ] {
            assert!(!text.contains(bad), "{bad:?} got through: {text}");
        }
    }
    assert!(lines.len() >= 3, "{lines:?}");
    assert!(
        lines.iter().all(|line| line.contains("(unreadable host)")),
        "{lines:?}"
    );
    // A plain host name, with a port, is shown as it is.
    assert_eq!(shown_host("git.example.com:8443"), "git.example.com:8443");
    assert_eq!(shown_host("api.github.com"), "api.github.com");
}

#[test]
fn an_audit_line_shows_a_host_only_when_it_is_a_host_name_and_names_no_internal_type() {
    let record = |host: &str, verdict: Verdict| AuditRecord {
        method: "GET".to_owned(),
        host: host.to_owned(),
        repo: None,
        class: Some(Class::GraphqlMutation("secretName".to_owned())),
        verdict,
    };
    for host in [
        "evil.example/pulls/7?x=1",
        "api.github.com; chat-9: PUT x merge allowed",
        "a\u{202e}moc.buhtig.ipa",
        "api.github.com:99999999",
        "api.github.com:80/x",
        "api.github.com:",
        "",
    ] {
        let line = audit_line(&record(host, Verdict::Refused(Refusal::HostNotAllowed)));
        assert_eq!(
            line,
            "GET (unreadable host) - GraphQL mutation blocked (the host is not one of the chat's \
             repos' hosts)",
            "{host:?}"
        );
    }
    let line = audit_line(&record("git.example.com:8443", Verdict::Allowed));
    assert_eq!(line, "GET git.example.com:8443 - GraphQL mutation allowed");
    assert!(!line.contains("secretName"), "{line}");
    let odd = AuditRecord {
        method: "G\u{202e}T".to_owned(),
        ..record("api.github.com", Verdict::Allowed)
    };
    assert!(audit_line(&odd).starts_with("(unreadable method) api.github.com"));
}

#[test]
fn the_doctor_lists_only_the_newest_refusals_and_says_how_many_it_left_out() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");
    for _ in 0..(MOST_REFUSALS + 3) {
        merge(&scene, "run-1", &params.sentinel);
    }
    let row = scene.routes.refusals_row(Path::new("/p/one"));
    assert_eq!(
        row.detail
            .matches("blocked (merging is not allowed")
            .count(),
        MOST_REFUSALS
    );
    assert!(
        row.detail.contains("3 older blocked requests not shown"),
        "{row:?}"
    );
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
    let held = scene
        .routes
        .guard("chat-1", "run-1", Path::new("/p/one"), None);
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
        .guard("chat-1", "run-a", Path::new("/p/one"), None)
        .commit(1);
    let held = scene
        .routes
        .guard("chat-2", "run-b", Path::new("/p/one"), None);

    // Another chat's session ends while run-b is still being opened as session 2.
    scene.routes.stop_session(1);
    scene.routes.stop_session(2);
    held.commit(2);

    assert!(!scene.routes.is_live("run-a") && !scene.routes.is_live("run-b"));

    // A run that opens after an exit of another session keeps its route.
    let held = scene
        .routes
        .guard("chat-3", "run-c", Path::new("/p/one"), None);
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
                account: None,
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
    let guard = scene
        .routes
        .guard("chat-1", "run-1", Path::new("/p/one"), None);
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
fn the_refs_of_a_remote_are_asked_only_for_a_repo_recorded_for_the_chats_own_run() {
    let scene = Scene::new();
    let own = "https://github.com/acme/app.git";
    // No run is committed to the session yet.
    assert_eq!(
        scene.routes.remote_refs(7, own).unwrap_err(),
        "this chat has no forge route"
    );
    let guard = scene
        .routes
        .guard("chat-1", "run-1", Path::new("/p/one"), None);
    assert!(guard.params().is_some());
    guard.commit(7);
    let not_recorded = "that remote is not a repository recorded for this chat";
    // Another host or repository is refused before anything is asked, however it is spelled.
    for url in [
        "https://github.com/acme/other.git",
        "https://github.com/other/app.git",
        "https://gitlab.example/acme/app.git",
        "https://evil.example/github.com/acme/app.git",
        "http://github.com/acme/app.git",
        "https://user@github.com/acme/app.git",
        "https://github.com/acme/app.git?x=1",
        "git@github.com:acme/app.git",
        "-u",
        "",
    ] {
        assert_eq!(
            scene.routes.remote_refs(7, url).unwrap_err(),
            not_recorded,
            "{url}"
        );
    }
    // Another chat's number has no route of its own to ask by.
    assert_eq!(
        scene.routes.remote_refs(8, own).unwrap_err(),
        "this chat has no forge route"
    );
    // The run's own repo passes the check and is asked on the run's route, never refused as
    // unrecorded. The ask is a stand-in that answers nothing: git is never run here, so none of
    // these can reach a real forge on the runner's network.
    for url in [
        own,
        "https://github.com/acme/app",
        "https://GitHub.com/acme/app/",
    ] {
        let mut asked = 0;
        let why = scene
            .routes
            .remote_refs_asking(7, url, |_, _| {
                asked += 1;
                None
            })
            .unwrap_err();
        assert_ne!(why, not_recorded, "{url}");
        assert_eq!(asked, 1, "{url}");
    }
    scene.routes.stop("run-1");
    assert_eq!(
        scene.routes.remote_refs(7, own).unwrap_err(),
        "this chat has no forge route"
    );
}

#[test]
fn the_refs_a_recorded_remote_has_are_what_the_ask_returns_on_the_runs_route() {
    let scene = Scene::new();
    let guard = scene
        .routes
        .guard("chat-1", "run-1", Path::new("/p/one"), None);
    assert!(guard.params().is_some());
    guard.commit(7);
    let tips = vec!["a".repeat(40), "b".repeat(40)];
    let mut asked = Vec::new();
    let got =
        scene
            .routes
            .remote_refs_asking(7, "https://GitHub.com/Acme/App.git", |url, route| {
                asked.push((url.to_owned(), format!("{route:?}")));
                Some(tips.clone())
            });
    assert_eq!(got, Ok(tips));
    let [(url, route)] = asked.as_slice() else {
        panic!("asked once: {asked:?}")
    };
    assert_eq!(url, "https://GitHub.com/Acme/App.git");
    assert!(route.contains("HTTPS_PROXY"), "{route}");
    // An ask that fails is said, and one for an unrecorded repo is never made.
    assert_eq!(
        scene
            .routes
            .remote_refs_asking(7, "https://github.com/acme/app", |_, _| None),
        Err("the remote could not be asked which commits it has".to_owned())
    );
    assert!(
        scene
            .routes
            .remote_refs_asking(7, "https://github.com/acme/other", |_, _| panic!("asked"))
            .is_err()
    );
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
    let guard = scene
        .routes
        .guard("chat-1", "run-1", Path::new("/p/one"), None);
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
    scene.recorded.cut_for("/p/one", "chat-1", "task/forms");
    let params = scene.start("chat-1", "run-1", "/p/one");

    let pushed = push_update(&scene, "run-1", &params.sentinel, "task/forms");

    assert!(matches!(pushed, Outcome::Forward(_)), "{pushed:?}");
}

#[test]
fn another_branch_the_default_branch_and_another_chats_branch_stay_protected() {
    let scene = Scene::new();
    scene.recorded.cut_for("/p/one", "chat-1", "task/forms");
    scene.recorded.cut_for("/p/one", "chat-2", "task/other");
    let params = scene.start("chat-1", "run-1", "/p/one");

    // The default branch is protected. Another chat's branch and a colleague's are not this
    // chat's: refused as such (and, where its chat is open, asked about in the window).
    let pushed = push_update(&scene, "run-1", &params.sentinel, "main");
    assert!(
        matches!(pushed, Outcome::Refused(Refusal::ProtectedRef)),
        "main: {pushed:?}"
    );
    for branch in ["task/other", "colleague"] {
        let pushed = push_update(&scene, "run-1", &params.sentinel, branch);
        assert!(
            matches!(pushed, Outcome::Refused(Refusal::NotYourBranch)),
            "{branch}: {pushed:?}"
        );
    }
}

#[test]
fn another_projects_chat_with_the_same_id_owns_none_of_a_branch_cut_in_this_one() {
    let scene = Scene::new();
    scene.recorded.cut_for("/p/two", "chat-1", "task/forms");
    scene
        .recorded
        .record("/p/two", vec![(Kind::GitHub, "github.com", "acme/app")]);
    scene.recorded.bind("/p/two", "github.com", "alice");
    let params = scene.start("chat-1", "run-1", "/p/one");

    let pushed = push_update(&scene, "run-1", &params.sentinel, "task/forms");

    assert!(
        matches!(pushed, Outcome::Refused(Refusal::NotYourBranch)),
        "{pushed:?}"
    );
}

/// A push that makes `branch`, which the forge's advertisement does not have, through the run's
/// route.
fn push_create(scene: &Scene, run: &str, sentinel: &str, branch: &str) -> Outcome {
    let proxy = lock_live(scene, run);
    proxy.note_advertised(
        &ChatId::new(run),
        "github.com",
        "acme/app",
        vec!["main".into()],
    );
    send_raw(
        scene,
        run,
        sentinel,
        ("POST", "github.com", "/acme/app.git/git-receive-pack"),
        "application/x-git-receive-pack-request",
        &push_commands(&[(true, false, &format!("refs/heads/{branch}"))]),
    )
}

fn lock_live(scene: &Scene, run: &str) -> Arc<ForgeProxy> {
    scene
        .routes
        .live()
        .get(run)
        .expect("a live route")
        .proxy
        .clone()
}

#[test]
fn a_branch_is_recorded_only_when_the_forge_confirms_the_chat_made_it_and_only_for_that_chat() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");
    let Outcome::Forward(made) = push_create(&scene, "run-1", &params.sentinel, "task/new") else {
        panic!("a push that makes a branch is forwarded");
    };
    // Sent is not made: nothing is recorded until the forge says so.
    assert!(scene.recorded.own_of("chat-1").is_empty());
    lock_live(&scene, "run-1").confirm_push(&ChatId::new("run-1"), &made);
    assert_eq!(scene.recorded.own_of("chat-1"), ["task/new"]);

    // The same chat id, in a new run, owns it from the start: it may move it.
    scene.routes.stop("run-1");
    let again = scene.start("chat-1", "run-2", "/p/one");
    let moved = push_update(&scene, "run-2", &again.sentinel, "task/new");
    assert!(matches!(moved, Outcome::Forward(_)), "{moved:?}");

    // A chat with another id (a resumed chat gets one) owns nothing: the route refuses its update.
    let other = scene.start("chat-2", "run-3", "/p/one");
    let refused = push_update(&scene, "run-3", &other.sentinel, "task/new");
    assert!(
        matches!(refused, Outcome::Refused(Refusal::NotYourBranch)),
        "{refused:?}"
    );
}

/// The forge's advertisement for a push to the repo, as the listener reads it: `main` only, so a
/// push that makes a branch is decided against it.
fn advertise_main(scene: &Scene, run: &str) {
    lock_live(scene, run).note_advertised(
        &ChatId::new(run),
        "github.com",
        "acme/app",
        vec!["main".into()],
    );
}

/// A push of `branch` from `old` to `new` through the run's route, as `git push` sends it.
fn push_ids(
    scene: &Scene,
    run: &str,
    sentinel: &str,
    branch: &str,
    old: &str,
    new: &str,
) -> Outcome {
    let line = format!("{old} {new} refs/heads/{branch}\0 report-status\n");
    let mut body = format!("{:04x}{line}", line.len() + 4).into_bytes();
    body.extend(b"0000");
    send_raw(
        scene,
        run,
        sentinel,
        ("POST", "github.com", "/acme/app.git/git-receive-pack"),
        "application/x-git-receive-pack-request",
        &body,
    )
}

#[test]
fn a_live_run_may_not_push_a_branch_another_chat_made_again_through_the_route() {
    let scene = Scene::new();
    let (one, two) = ("1".repeat(40), "2".repeat(40));
    let a = scene.start("chat-1", "run-1", "/p/one");
    let b = scene.start("chat-2", "run-2", "/p/one");

    // Chat 1 makes `task/new` through its route (after the advertisement a push is decided by);
    // the forge takes it.
    advertise_main(&scene, "run-1");
    let Outcome::Forward(made) = push_ids(
        &scene,
        "run-1",
        &a.sentinel,
        "task/new",
        &"0".repeat(40),
        &one,
    ) else {
        panic!("a push that makes a branch is forwarded");
    };
    lock_live(&scene, "run-1").confirm_push(&ChatId::new("run-1"), &made);
    assert_eq!(scene.recorded.own_of("chat-1"), ["task/new"]);

    // A colleague deletes it. Chat 2's route reads the advertisement without the name and makes
    // it again, at another commit, and the forge takes that push.
    advertise_main(&scene, "run-2");
    let Outcome::Forward(again) = push_ids(
        &scene,
        "run-2",
        &b.sentinel,
        "task/new",
        &"0".repeat(40),
        &two,
    ) else {
        panic!("chat 2 may make the name again");
    };
    lock_live(&scene, "run-2").confirm_push(&ChatId::new("run-2"), &again);
    assert_eq!(scene.recorded.own_of("chat-2"), ["task/new"]);
    assert!(scene.recorded.own_of("chat-1").is_empty());

    // Chat 1's run is still live, and read its ownership at its start. It never saw an
    // advertisement without the name; its update from the tip it left is refused all the same.
    let refused = push_ids(
        &scene,
        "run-1",
        &a.sentinel,
        "task/new",
        &one,
        &"3".repeat(40),
    );
    assert!(
        matches!(refused, Outcome::Refused(Refusal::NotYourBranch)),
        "{refused:?}"
    );
    // And chat 2, whose push is the last, moves it from the tip it left.
    let moved = push_ids(
        &scene,
        "run-2",
        &b.sentinel,
        "task/new",
        &two,
        &"3".repeat(40),
    );
    assert!(matches!(moved, Outcome::Forward(_)), "{moved:?}");
}

#[test]
fn consecutive_confirmed_pushes_each_move_the_tip_and_a_stale_one_is_refused() {
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");
    let (one, two, three) = ("1".repeat(40), "2".repeat(40), "3".repeat(40));
    let confirm = |out: Outcome| {
        let Outcome::Forward(sent) = out else {
            panic!("forwarded: {out:?}");
        };
        lock_live(&scene, "run-1").confirm_push(&ChatId::new("run-1"), &sent);
    };
    advertise_main(&scene, "run-1");
    confirm(push_ids(
        &scene,
        "run-1",
        &params.sentinel,
        "task/new",
        &"0".repeat(40),
        &one,
    ));
    confirm(push_ids(
        &scene,
        "run-1",
        &params.sentinel,
        "task/new",
        &one,
        &two,
    ));
    // The first tip is stale now.
    let stale = push_ids(&scene, "run-1", &params.sentinel, "task/new", &one, &three);
    assert!(
        matches!(stale, Outcome::Refused(Refusal::ProtectedRef)),
        "{stale:?}"
    );
    confirm(push_ids(
        &scene,
        "run-1",
        &params.sentinel,
        "task/new",
        &two,
        &three,
    ));
}

#[test]
fn a_branch_the_forge_no_longer_lists_ends_its_record_and_the_next_update_is_refused() {
    let scene = Scene::new();
    scene.recorded.cut_for("/p/one", "chat-1", "task/forms");
    let params = scene.start("chat-1", "run-1", "/p/one");
    lock_live(&scene, "run-1").note_advertised(
        &ChatId::new("run-1"),
        "github.com",
        "acme/app",
        vec!["main".into()],
    );
    assert!(scene.recorded.own_of("chat-1").is_empty());
    let refused = push_update(&scene, "run-1", &params.sentinel, "task/forms");
    assert!(
        matches!(refused, Outcome::Refused(Refusal::NotYourBranch)),
        "{refused:?}"
    );
}

#[test]
fn an_update_of_a_branch_the_open_chat_has_no_record_of_raises_one_ask_in_the_window_and_the_chat_is_told()
 {
    let scene = Scene::new();
    let project = "/p/let-chat-push";
    scene
        .recorded
        .record(project, vec![(Kind::GitHub, "github.com", "acme/app")]);
    scene.recorded.bind(project, "github.com", "alice");
    let params = scene.open("chat-1", "run-1", project, 5);
    // The forge lists the branch: an ask is raised only for a branch the repo's advertisement
    // names.
    lock_live(&scene, "run-1").note_advertised(
        &ChatId::new("run-1"),
        "github.com",
        "acme/app",
        vec!["main".into(), "colleague".into()],
    );

    let said = |pushed: Outcome| match pushed {
        Outcome::Held(held) => held.said,
        other => panic!("an ask is raised and the chat told: {other:?}"),
    };
    // A branch pushed before the route existed: the chat is told an ask is waiting, however
    // often it tries again, and the window has the one ask.
    let told = said(push_update(&scene, "run-1", &params.sentinel, "colleague"));
    assert!(
        told.contains("Let this chat push it is waiting in the window"),
        "{told}"
    );
    said(push_update(&scene, "run-1", &params.sentinel, "colleague"));
    let raised = purlis_core::windowonly::every(Path::new(project));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].session, 5);
    assert!(
        matches!(
            &raised[0].control,
            purlis_core::windowonly::Control::LetChatPush { chat, branch, .. }
                if chat == "chat-1" && branch == "colleague"
        ),
        "{raised:?}"
    );

    // The default branch raises no ask, and no other is added.
    let main = push_update(&scene, "run-1", &params.sentinel, "main");
    assert!(
        matches!(main, Outcome::Refused(Refusal::ProtectedRef)),
        "{main:?}"
    );
    assert_eq!(purlis_core::windowonly::every(Path::new(project)).len(), 1);
    purlis_core::windowonly::project_ended(Path::new(project));
}

// ---- a chat put back from the session host after a restart (#7694) ----

#[test]
fn a_chat_put_back_after_a_restart_has_the_apps_git_routed_for_it_and_is_handed_nothing() {
    // The app went and came back; the session host kept the chat's program, which is the run it
    // was. Its route went with the old app, so `purlis worktree push` found none.
    let scene = Scene::new();

    let guard = scene
        .routes
        .guard_reattached("chat-1", "run-1", Path::new("/p/one"), None);
    assert!(
        guard.params().is_none(),
        "a program already running is handed no new sentinel"
    );
    guard.commit(7);

    let route = scene
        .routes
        .git_route(7)
        .expect("a route for the session put back")
        .expect("one git can use");
    assert!(format!("{route:?}").contains("HTTPS_PROXY"), "{route:?}");
    // It goes with the session, as a started chat's does.
    scene.routes.stop_session(7);
    assert!(!scene.routes.is_live("run-1"));
}

#[test]
fn a_chat_put_back_is_given_a_route_only_where_a_started_one_would_be() {
    // Nothing on a pinned forge is enrolled, put back or not: the same gates as a start.
    let scene = Scene::pinning(&["github.com", "gitlab.com"]);

    let guard = scene
        .routes
        .guard_reattached("chat-1", "run-1", Path::new("/p/one"), None);
    guard.commit(7);

    assert!(scene.routes.git_route(7).is_none());
    assert!(!scene.routes.is_live("run-1"));
}

#[test]
fn a_chat_put_back_whose_route_is_still_live_keeps_it_as_it_was() {
    // A route this app already holds for the run is the one the chat's sandbox is wired to:
    // starting another would retire the sentinel the chat's own git sends.
    let scene = Scene::new();
    let params = scene.start("chat-1", "run-1", "/p/one");
    let folder = scene.folder_of("run-1");

    let guard = scene
        .routes
        .guard_reattached("chat-1", "run-1", Path::new("/p/one"), None);
    assert!(guard.params().is_none());
    guard.commit(7);

    assert_eq!(scene.folder_of("run-1"), folder, "the same route");
    assert!(scene.routes.git_route(7).is_some());
    let served = comment(&scene, "run-1", &params.sentinel);
    assert!(
        bearer(&served).is_some(),
        "the old sentinel still works: {served:?}"
    );
}

// ---- a chat put back whose program holds the old app's sentinel (#7962) ----

#[test]
fn a_chat_put_back_on_a_route_made_anew_is_one_whose_program_holds_no_live_sentinel() {
    // The old app's route went with it, sentinel and all: the program the session host kept
    // still sends that sentinel, and this app's route for the same run refuses it.
    let scene = Scene::new();
    let gone = scene.start("chat-1", "run-1", "/p/one");
    scene.routes.stop("run-1");

    let guard = scene
        .routes
        .guard_reattached("chat-1", "run-1", Path::new("/p/one"), None);
    assert!(
        guard.leaves_its_program_without_a_route(),
        "its program needs a start of its own to be handed a sentinel"
    );
    guard.commit(7);

    assert!(matches!(
        comment(&scene, "run-1", &gone.sentinel),
        Outcome::Refused(Refusal::WrongSentinel)
    ));
}

#[test]
fn a_chat_put_back_whose_route_is_still_live_or_that_has_none_needs_no_new_one() {
    // Still live: the program's sentinel is the route's own.
    let scene = Scene::new();
    let _live = scene.start("chat-1", "run-1", "/p/one");
    let guard = scene
        .routes
        .guard_reattached("chat-1", "run-1", Path::new("/p/one"), None);
    assert!(!guard.leaves_its_program_without_a_route());
    guard.commit(7);

    // Nothing recorded on an unpinned forge: a start would have had no route either.
    let pinned = Scene::pinning(&["github.com", "gitlab.com"]);
    let guard = pinned
        .routes
        .guard_reattached("chat-1", "run-1", Path::new("/p/one"), None);
    assert!(!guard.leaves_its_program_without_a_route());
    guard.commit(7);
}

#[test]
fn a_chat_restarted_on_its_conversation_after_it_was_put_back_pushes_its_own_branch_and_the_old_sentinel_is_refused()
 {
    let scene = Scene::new();
    scene.recorded.cut_for("/p/one", "chat-1", "task/forms");
    // The old app's run, whose sentinel the program the session host kept still sends.
    let old = scene.start("chat-1", "run-1", "/p/one");
    scene.routes.stop("run-1");
    let put_back = scene
        .routes
        .guard_reattached("chat-1", "run-1", Path::new("/p/one"), None);
    put_back.commit(7);
    assert!(matches!(
        push_update(&scene, "run-1", &old.sentinel, "task/forms"),
        Outcome::Refused(Refusal::WrongSentinel)
    ));

    // The restart on its conversation: the same chat, a new run, a route handed to its program.
    let restarted = scene
        .routes
        .guard("chat-1", "run-2", Path::new("/p/one"), None);
    let fresh = restarted
        .params()
        .expect("a restart is handed a route")
        .sentinel
        .clone();
    assert!(!restarted.leaves_its_program_without_a_route());
    assert_ne!(fresh, old.sentinel, "a new sentinel, never the old one");
    restarted.commit(8);
    // The put-back run ends as the restart replaces it.
    scene.routes.stop_session(7);
    assert!(!scene.routes.is_live("run-1"));

    // Its own branch, by the ownership record kept under the chat's id, through its own git…
    let pushed = push_update(&scene, "run-2", &fresh, "task/forms");
    assert!(matches!(pushed, Outcome::Forward(_)), "{pushed:?}");
    // …and through the app's git (`purlis worktree push`).
    assert!(matches!(scene.routes.git_route(8), Some(Ok(_))));
    // The old sentinel reaches nothing on the new run.
    assert!(matches!(
        push_update(&scene, "run-2", &old.sentinel, "task/forms"),
        Outcome::Refused(Refusal::WrongSentinel)
    ));
}

// ---- what a forge refusal counts (#7779) ----

#[test]
fn a_forge_refusal_the_route_counts_is_one_line_of_guards_forge_in_the_chats_enforcement_log() {
    use purlis_core::enforcement::{self, Family};
    use purlis_core::forge::proxy::Refused;
    let project = tempfile::tempdir().expect("a project");
    let sink = refusals_counted_in(project.path().to_owned());

    sink(
        &ChatId::new("run-1"),
        &Refused {
            rule: enforcement::RULE_FORGE_MERGE,
            host: "api.github.com:443".to_owned(),
        },
    );

    let lines = enforcement::read_log(project.path(), "run-1");
    assert_eq!(lines.len(), 1, "{lines:?}");
    let line = &lines[0];
    assert_eq!(line.rule, "forge.not-allowed.merge");
    assert_eq!(line.family, Family::GuardsForge.key());
    assert_eq!(line.chat, "run-1");
    assert_eq!(line.host.as_deref(), Some("api.github.com:443"));
    // It is a line the window puts on the page: a known rule of its family, about a `host:port`.
    assert!(enforcement::known_rule(Family::GuardsForge, &line.rule));
    assert!(enforcement::is_host_and_port(line.host.as_deref().unwrap()));
}

/// A chat the chat with stable id `parent` started, working in `lane`, on its first start.
fn handed(parent: &str, lane: Option<(&str, &str)>) -> Handed {
    Handed {
        parent: Some(parent.to_owned()),
        lane: lane.map(|(host, path)| (host.to_owned(), path.to_owned())),
        first_start: true,
    }
}

#[test]
fn a_chat_started_by_a_chat_with_a_route_gets_one_too_or_is_refused_before_it_starts() {
    // #7870: of two lanes one turn dispatched, one started with no forge route, so it could not
    // fetch, update or push, and nothing it could do gave it one.
    let scene = Scene::new();
    let parent = scene.open("parent", "run-p", "/p/one", 1);
    let project = Path::new("/p/one");

    let lane = scene
        .routes
        .guard_handed("lane-1", "run-1", project, None, &handed("parent", None))
        .expect("a lane of a routed parent starts");
    assert!(lane.params().is_some(), "it has a route as its parent does");

    scene.listener.fail.store(true, Ordering::SeqCst);
    let refused = scene
        .routes
        .guard_handed("lane-2", "run-2", project, None, &handed("parent", None))
        .err()
        .expect("a lane whose route could not be made is not started");
    assert_eq!(refused, NO_ROUTE_LIKE_ITS_PARENT);
    assert!(!scene.routes.is_live("run-2"), "nothing of it is left");
    assert!(
        scene.routes.is_live("run-p") && parent.socket.exists(),
        "the parent's route is untouched"
    );

    // Started again later, it is warned of and started as it is, as before.
    let again = Handed {
        first_start: false,
        ..handed("parent", None)
    };
    let restarted = scene
        .routes
        .guard_handed("lane-2", "run-2b", project, None, &again)
        .expect("a later start is never refused");
    assert!(restarted.params().is_none());
}

#[test]
fn a_lane_whose_route_fails_once_is_started_again_and_gets_one() {
    // #7870: several starts at once meet a listener or a sign-in read that is busy for a moment.
    let scene = Scene::new();
    scene.open("parent", "run-p", "/p/one", 1);
    scene.listener.fail_next.store(1, Ordering::SeqCst);

    let started = std::time::Instant::now();
    let lane = scene
        .routes
        .guard_handed(
            "lane",
            "run-1",
            Path::new("/p/one"),
            None,
            &handed("parent", None),
        )
        .expect("the second start makes the route");
    assert!(lane.params().is_some());
    assert!(
        started.elapsed() >= ROUTE_RETRY_AFTER,
        "the second start waits its backoff first"
    );
    assert_eq!(scene.listener.fail_next.load(Ordering::SeqCst), 0);
}

#[test]
fn a_lane_is_held_to_its_parent_by_id_never_by_a_number_dealt_again() {
    // #7870: a chat's number is dealt again once it closes, so a lane whose parent closed must
    // not be held to whichever chat has that number now.
    let scene = Scene::new();
    scene.open("someone-else", "run-other", "/p/one", 1);
    scene.listener.fail.store(true, Ordering::SeqCst);

    let lane = scene
        .routes
        .guard_handed(
            "lane",
            "run-1",
            Path::new("/p/one"),
            None,
            &handed("closed-parent", None),
        )
        .expect("a lane whose own parent has no route is started");
    assert!(lane.params().is_none());

    // And a parent found by id, whatever session it is open as.
    scene.listener.fail.store(false, Ordering::SeqCst);
    scene.open("parent", "run-p", "/p/one", 7);
    scene.listener.fail.store(true, Ordering::SeqCst);
    assert!(
        scene
            .routes
            .guard_handed(
                "lane",
                "run-2",
                Path::new("/p/one"),
                None,
                &handed("parent", None)
            )
            .is_err(),
        "held to the parent it names"
    );
}

#[test]
fn a_lane_whose_route_leaves_out_its_own_repo_is_refused_where_its_parent_has_it() {
    // #7870: a route is live yet without the lane's repo, as the recorded repos are read one by
    // one at each start and one can drop out.
    let scene = Scene::new();
    scene.open("parent", "run-p", "/p/one", 1);
    scene
        .recorded
        .record("/p/one", vec![(Kind::GitLab, "gitlab.com", "group/proj")]);
    let project = Path::new("/p/one");

    let refused = scene
        .routes
        .guard_handed(
            "lane",
            "run-1",
            project,
            None,
            &handed("parent", Some(("GitHub.com", "acme/app"))),
        )
        .err()
        .expect("a route without the lane's repo is no route for it");
    assert_eq!(refused, NO_ROUTE_LIKE_ITS_PARENT);

    let other = scene
        .routes
        .guard_handed(
            "lane",
            "run-2",
            project,
            None,
            &handed("parent", Some(("gitlab.com", "group/proj"))),
        )
        .expect("a lane in a repo its route covers starts");
    assert!(other.params().is_some());

    // A repo the parent's route does not cover holds the lane to nothing.
    let unheld = scene
        .routes
        .guard_handed(
            "lane",
            "run-3",
            project,
            None,
            &handed("parent", Some(("github.com", "acme/elsewhere"))),
        )
        .expect("nothing to be held to");
    assert!(unheld.params().is_some());
}
