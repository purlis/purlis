// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A2: the real `gh`, `glab` and `git`, each with the environment a chat would
//! be given, through the real listener, to a fake forge. A tool is satisfied when it gets an answer;
//! the proxy is satisfied when **it refused nothing the tools sent** and the forge was only ever
//! sent the real token, never the sentinel.
//!
//! The tools are on the CI runners (`.github/workflows/one-login-proxy-listener.yml` installs
//! `glab`); where one is missing the test says so and passes, unless `PURLIS_PROXY_REAL_CLIENTS` is
//! set, which is the workflow's way of saying every one must be there. With
//! `PURLIS_CAPTURE_GRAPHQL=<dir>` every GraphQL document the proxy forwarded is written there, which
//! is how `tests/fixtures/one-login-graphql/` is re-captured.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::forge::pin::{NoPin, Pin};
use purlis_core::forge::proxy::listener::{
    ForgeListener, PushGate, PushRead, PushVerdict, Upstream, UpstreamError, UpstreamRequest,
    UpstreamResponse,
};
use purlis_core::forge::proxy::{EndpointKind, Verdict};
use purlis_core::sandbox::egress::ForgeRoute;
use purlis_core::sandbox::egress::relay::DoorRelay;

use crate::support::forge_listener::{REAL, World};

/// A fake forge that answers what the tools ask, with git served by `git` itself from a bare repo.
pub(crate) struct Forge {
    bare: PathBuf,
    pub(crate) seen: Mutex<Vec<UpstreamRequest>>,
    captured: AtomicUsize,
}

impl Forge {
    fn json(&self, body: &str) -> Reply {
        Reply::new(200, "application/json", body.as_bytes().to_vec())
    }

    fn git(&self, rpc: &str, advertise: bool, body: &[u8]) -> Reply {
        let mut command = Command::new("git");
        // `git upload-pack`, not `git git-upload-pack`.
        command
            .arg(rpc.trim_start_matches("git-"))
            .arg("--stateless-rpc");
        if advertise {
            command.arg("--advertise-refs");
        }
        command.arg(&self.bare);
        command
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = purlis_core::forklock::spawn(&mut command).expect("git");
        let mut stdin = child.stdin.take().unwrap();
        let body = body.to_vec();
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(&body);
        });
        let out = child.wait_with_output().expect("git ran");
        let _ = writer.join();
        if !out.status.success() {
            // The fake forge says what went wrong, so a failure is a reading and not a guess.
            return Reply::new(
                500,
                "text/plain",
                format!("git {rpc} failed: {}", String::from_utf8_lossy(&out.stderr)).into_bytes(),
            );
        }
        let service = rpc.trim_start_matches("git-").to_string();
        let mut bytes = Vec::new();
        let kind = if advertise {
            let line = format!("# service=git-{service}\n");
            bytes.extend(format!("{:04x}{line}0000", line.len() + 4).bytes());
            "advertisement"
        } else {
            "result"
        };
        bytes.extend(out.stdout);
        Reply::new(200, &format!("application/x-git-{service}-{kind}"), bytes)
    }

    fn answer(&self, request: &UpstreamRequest) -> Reply {
        let url = request.url.as_str();
        let path = url.split_once("://").map_or(url, |(_, rest)| rest);
        let (path, _) = path.split_once('?').map_or((path, ""), |(p, q)| (p, q));
        let body = String::from_utf8_lossy(&request.body).into_owned();
        if let Some(dir) = std::env::var_os("PURLIS_CAPTURE_GRAPHQL")
            && path.ends_with("/graphql")
        {
            let n = self.captured.fetch_add(1, Ordering::SeqCst);
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(Path::new(&dir).join(format!("{n:03}.json")), &request.body);
        }
        match (request.method.as_str(), path) {
            ("GET", "api.github.com/user") => self.json(r#"{"login":"octocat","id":1}"#),
            ("GET", "api.github.com/repos/acme/app") => self.json(
                r#"{"id":1,"name":"app","full_name":"acme/app","default_branch":"main","private":false}"#,
            ),
            ("GET", "api.github.com/repos/acme/app/readme") => {
                Reply::new(404, "application/json", br#"{"message":"Not Found"}"#.to_vec())
            }
            ("POST", "api.github.com/graphql") => self.json(&graphql_answer(&body)),
            ("PUT", "api.github.com/repos/acme/app/pulls/1/merge") => {
                self.json(r#"{"merged":true,"sha":"abc","message":"Pull Request successfully merged"}"#)
            }
            ("PUT", "gitlab.com/api/v4/projects/group%2Fproj/merge_requests/1/merge") => {
                self.json(r#"{"iid":1,"state":"merged"}"#)
            }
            ("GET", "gitlab.com/api/v4/user") => self.json(r#"{"id":1,"username":"tanuki"}"#),
            ("GET", p) if p.starts_with("gitlab.com/api/v4/projects/group%2Fproj/merge_requests") => {
                self.json("[]")
            }
            // What `glab ci` reads and retries.
            ("GET", "gitlab.com/api/v4/projects/group%2Fproj/pipelines") => self.json(
                r#"[{"id":9,"iid":3,"project_id":42,"status":"failed","ref":"main","sha":"abc","web_url":"https://gitlab.com/group/proj/-/pipelines/9"}]"#,
            ),
            ("GET", "gitlab.com/api/v4/projects/group%2Fproj/jobs/5") => self.json(JOB),
            ("GET", "gitlab.com/api/v4/projects/group%2Fproj/jobs/5/trace") => {
                Reply::new(200, "text/plain", b"Running with the runner\nJOBLOGLINE\n".to_vec())
            }
            ("POST", "gitlab.com/api/v4/projects/group%2Fproj/pipelines/9/retry") => {
                self.json(r#"{"id":9,"project_id":42,"status":"pending","ref":"main"}"#)
            }
            ("GET", "gitlab.com/api/v4/projects/group%2Fproj") => self.json(
                r#"{"id":42,"name":"proj","path_with_namespace":"group/proj","default_branch":"main"}"#,
            ),
            ("GET", "github.com/acme/app.git/info/refs") => {
                if url.ends_with("service=git-receive-pack") {
                    self.git("git-receive-pack", true, b"")
                } else {
                    self.git("git-upload-pack", true, b"")
                }
            }
            ("POST", "github.com/acme/app.git/git-upload-pack") => {
                self.git("git-upload-pack", false, &request.body)
            }
            ("POST", "github.com/acme/app.git/git-receive-pack") => {
                self.git("git-receive-pack", false, &request.body)
            }
            _ => Reply::new(
                404,
                "application/json",
                format!(r#"{{"message":"the fake forge has no {} {path}"}}"#, request.method)
                    .into_bytes(),
            ),
        }
    }
}

/// A finished job, as GitLab answers one.
const JOB: &str = r#"{"id":5,"name":"build","stage":"test","status":"success","ref":"main","web_url":"https://gitlab.com/group/proj/-/jobs/5","pipeline":{"id":9,"project_id":42,"status":"success","ref":"main"}}"#;

struct Reply {
    status: u16,
    content_type: String,
    body: Vec<u8>,
}

impl Reply {
    fn new(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type: content_type.to_string(),
            body,
        }
    }
}

/// What a document asks for, answered with the least a tool will read as an answer.
fn graphql_answer(body: &str) -> String {
    // Feature detection: `Name: __type(name: "X"){ fields { name } }`, one answer for each.
    if body.contains("__type(") {
        let mut data = serde_json::Map::new();
        let mut rest = body;
        while let Some(at) = rest.find("__type(") {
            let before = rest[..at].trim_end().trim_end_matches(':').trim_end();
            let alias: String = before
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let key = if alias.is_empty() || !rest[..at].trim_end().ends_with(':') {
                "__type".to_string()
            } else {
                alias
            };
            data.insert(
                key,
                serde_json::json!({"fields": [], "inputFields": [], "enumValues": []}),
            );
            rest = &rest[at + 7..];
        }
        return serde_json::json!({ "data": data }).to_string();
    }
    if body.contains("issueOrPullRequest(") {
        return r#"{"data":{"repository":{"hasIssuesEnabled":true,"issue":{"__typename":"Issue","id":"I_1","number":1,"title":"t","state":"OPEN","body":"b","url":"https://github.com/acme/app/issues/1","createdAt":"2020-01-01T00:00:00Z","author":{"login":"octocat"},"assignees":{"nodes":[],"totalCount":0},"labels":{"nodes":[],"totalCount":0},"comments":{"nodes":[],"totalCount":0},"reactionGroups":[]}}}}"#.to_string();
    }
    let page = r#""totalCount":0,"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":""}"#;
    let repo = if body.contains("pullRequests(") {
        format!(r#"{{"pullRequests":{{{page}}}}}"#)
    } else if body.contains("pullRequest(number") {
        r#"{"pullRequest":{"id":"PR_1","number":1,"title":"t","state":"OPEN","body":"b","url":"https://github.com/acme/app/pull/1","author":{"login":"octocat"},"baseRefName":"main","headRefName":"fix","isDraft":false,"mergeable":"MERGEABLE","additions":1,"deletions":0,"changedFiles":1,"isCrossRepository":false,"commits":{"totalCount":1,"nodes":[]},"reviews":{"nodes":[],"totalCount":0},"comments":{"nodes":[],"totalCount":0},"labels":{"nodes":[],"totalCount":0},"assignees":{"nodes":[],"totalCount":0},"reviewRequests":{"nodes":[]},"projectCards":{"nodes":[],"totalCount":0},"statusCheckRollup":{"nodes":[]}}}"#.to_string()
    } else if body.contains("issues(") {
        format!(r#"{{"hasIssuesEnabled":true,"issues":{{{page}}}}}"#)
    } else if body.contains("issue(number") {
        r#"{"hasIssuesEnabled":true,"issue":{"id":"I_1","number":1,"title":"t","state":"OPEN","body":"b","url":"https://github.com/acme/app/issues/1","author":{"login":"octocat"},"comments":{"nodes":[],"totalCount":0},"labels":{"nodes":[],"totalCount":0},"assignees":{"nodes":[],"totalCount":0},"projectCards":{"nodes":[],"totalCount":0}}}"#.to_string()
    } else if body.contains("releases(") {
        r#"{"releases":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":""}}}"#.to_string()
    } else {
        r#"{"id":"R_acme_app","name":"app","nameWithOwner":"acme/app","owner":{"login":"acme"},"description":"d","url":"https://github.com/acme/app","isPrivate":false,"isFork":false,"isArchived":false,"hasIssuesEnabled":true,"hasWikiEnabled":false,"viewerPermission":"WRITE","defaultBranchRef":{"name":"main"},"mergeCommitAllowed":true,"squashMergeAllowed":true,"rebaseMergeAllowed":true}"#.to_string()
    };
    if body.contains("viewer") && !body.contains("repository(") {
        return r#"{"data":{"viewer":{"login":"octocat"}}}"#.to_string();
    }
    format!(r#"{{"data":{{"repository":{repo}}}}}"#)
}

impl Upstream for Forge {
    fn send(&self, request: UpstreamRequest) -> Result<UpstreamResponse, UpstreamError> {
        let reply = self.answer(&request);
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request);
        Ok(UpstreamResponse {
            status: reply.status,
            headers: vec![
                ("Content-Type".into(), reply.content_type),
                ("Content-Length".into(), reply.body.len().to_string()),
            ],
            body: Box::new(std::io::Cursor::new(reply.body)),
        })
    }
}

pub(crate) fn tool(name: &str) -> bool {
    let present = purlis_core::forklock::status(
        Command::new(name)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
    .is_ok_and(|s| s.success());
    if !present {
        assert!(
            std::env::var_os("PURLIS_PROXY_REAL_CLIENTS").is_none(),
            "{name} is not installed, and PURLIS_PROXY_REAL_CLIENTS says it must be"
        );
        let _ = writeln!(std::io::stderr(), "skipping: {name} is not installed");
    }
    present
}

/// A synthetic address for the commits a test makes.
pub(crate) fn author_email() -> String {
    format!("user.email=t{}example.invalid", '@')
}

pub(crate) fn git_ok(dir: &Path, args: &[&str]) {
    let out = purlis_core::forklock::output(
        Command::new("git")
            .args(["-c", "user.name=t", "-c", &author_email()])
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Every request the listener read, with its body: what a tool sent, refused or not.
#[derive(Default)]
struct Seen(Mutex<Vec<(String, String, String)>>);

impl purlis_core::forge::proxy::listener::Observe for Seen {
    fn request(&self, head: &purlis_core::forge::proxy::RequestHead, body: &[u8]) {
        let body = String::from_utf8_lossy(body).into_owned();
        if let Some(dir) = std::env::var_os("PURLIS_CAPTURE_GRAPHQL")
            && head.target.ends_with("/graphql")
        {
            let n = self.0.lock().unwrap_or_else(PoisonError::into_inner).len();
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(Path::new(&dir).join(format!("seen-{n:03}.json")), &body);
        }
        self.0.lock().unwrap_or_else(PoisonError::into_inner).push((
            head.method.clone(),
            head.target.clone(),
            body,
        ));
    }
}

/// A fake forge serving `acme/app` from a bare repo made in `dir`: one commit on `main`.
pub(crate) fn make_forge(dir: &Path) -> Arc<Forge> {
    // A repo to serve: one commit on `main`.
    let work = dir.join("work");
    std::fs::create_dir(&work).unwrap();
    git_ok(&work, &["init", "-q", "-b", "main"]);
    std::fs::write(work.join("README.md"), "hello\n").unwrap();
    git_ok(&work, &["add", "-A"]);
    git_ok(&work, &["commit", "-q", "-m", "one"]);
    let bare = dir.join("forge.git");
    git_ok(
        dir,
        &[
            "clone",
            "-q",
            "--bare",
            work.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    git_ok(&bare, &["config", "http.receivepack", "true"]);

    Arc::new(Forge {
        bare,
        seen: Mutex::new(Vec::new()),
        captured: AtomicUsize::new(0),
    })
}

/// The listeners are dropped before the folder the socket is in: a listener woken through a
/// socket that is gone is a listener that is not woken.
struct Setup {
    world: World,
    forge: Arc<Forge>,
    socket: ForgeListener,
    /// Held so the `CONNECT` door stays open for as long as the setup does.
    #[allow(dead_code, reason = "held for its drop")]
    connect: ForgeListener,
    /// The relay a chat's egress proxy stands for: git's `HTTPS_PROXY`, in front of the door.
    relay: u16,
    /// **The production relay the app's own git is pointed at** ([`DoorRelay`]), in front of the
    /// same door. Never the test-support relay above: that one is the chat's egress proxy.
    app_relay: DoorRelay,
    seen: Arc<Seen>,
    ca_file: PathBuf,
    dir: tempfile::TempDir,
}

fn setup() -> Setup {
    setup_with(None)
}

/// The relay the app's git is given for a run, as the app makes it (`forgeroute`), for `door`.
fn app_relay(door: std::net::SocketAddr, world: &World, pin: Arc<dyn Pin>) -> DoorRelay {
    DoorRelay::start(
        ForgeRoute {
            addr: door,
            handoff: world.wiring.handoff.clone(),
            hosts: vec!["github.com".to_owned(), "gitlab.com".to_owned()],
        },
        pin,
        std::time::Duration::from_secs(30),
    )
    .expect("the app's relay")
}

/// [`setup`], its listeners checking every push with `gate`.
fn setup_with(gate: Option<Arc<dyn PushGate>>) -> Setup {
    setup_in(gate, World::new())
}

/// [`setup_with`], on `world`.
fn setup_in(gate: Option<Arc<dyn PushGate>>, world: World) -> Setup {
    let dir = tempfile::Builder::new()
        .prefix("fp")
        .tempdir_in("/tmp")
        .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let forge = make_forge(dir.path());
    let mut world = world;
    world.wiring.upstream = forge.clone();
    let seen = Arc::new(Seen::default());
    world.wiring.observe = Some(seen.clone());
    if let Some(gate) = gate {
        world.wiring.push = gate;
    }
    let socket =
        ForgeListener::serve_socket(world.wiring.clone(), &dir.path().join("sock")).unwrap();
    let connect = ForgeListener::serve_connect(world.wiring.clone()).unwrap();
    let ca_file = dir.path().join("ca.pem");
    std::fs::write(&ca_file, connect.ca_certificate_pem().unwrap()).unwrap();
    let EndpointKind::Loopback(door) = connect.kind().clone() else {
        unreachable!()
    };
    let relay = crate::support::forge_listener::relay_to_door(door, world.wiring.handoff.clone());
    let app_relay = app_relay(door, &world, Arc::new(NoPin));
    Setup {
        world,
        forge,
        socket,
        connect,
        relay,
        app_relay,
        seen,
        ca_file,
        dir,
    }
}

pub(crate) fn clean(command: &mut Command, home: &Path) {
    command
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("TMPDIR", std::env::temp_dir())
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .stdin(Stdio::null());
}

/// How long a tool may take before it is ended and said to have: a tool that waits for something
/// that never comes is a finding, not a hung job.
const TOOL_TIME: std::time::Duration = std::time::Duration::from_secs(60);

pub(crate) fn run(mut command: Command) -> (bool, String) {
    use std::io::Read;
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = purlis_core::forklock::spawn(&mut command).expect("the tool ran");
    let mut readers = Vec::new();
    for pipe in [
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    ] {
        readers.push(std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            bytes
        }));
    }
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() < TOOL_TIME => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let text: String = readers
        .into_iter()
        .map(|r| String::from_utf8_lossy(&r.join().unwrap_or_default()).into_owned())
        .collect();
    match status {
        Some(status) => (status.success(), text),
        None => (
            false,
            format!("TIMED OUT after {TOOL_TIME:?}, ended\n{text}"),
        ),
    }
}

impl Setup {
    /// The port the app's own git is pointed at: the production relay's.
    fn app_port(&self) -> u16 {
        self.app_relay.addr().port()
    }

    fn port(&self) -> u16 {
        self.relay
    }

    fn gh(&self, args: &[&str]) -> (bool, String) {
        let EndpointKind::UnixSocket(socket) = self.socket.kind().clone() else {
            unreachable!()
        };
        let home = self.dir.path().join("gh-home");
        let config = home.join("gh");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join("config.yml"),
            format!("http_unix_socket: {}\n", socket.display()),
        )
        .unwrap();
        let sentinel = self.world.sentinel.expose();
        let mut command = Command::new("gh");
        clean(&mut command, &home);
        command
            .args(args)
            .env("GH_CONFIG_DIR", &config)
            .env("GH_HOST", "github.localhost")
            .env("GH_TOKEN", sentinel)
            .env("GH_ENTERPRISE_TOKEN", sentinel)
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .env("GH_NO_EXTENSION_UPDATE_NOTIFIER", "1");
        run(command)
    }

    fn glab(&self, args: &[&str]) -> (bool, String) {
        let home = self.dir.path().join("glab-home");
        std::fs::create_dir_all(&home).unwrap();
        let proxy = format!("http://127.0.0.1:{}", self.port());
        let mut command = Command::new("glab");
        clean(&mut command, &home);
        command
            .args(args)
            .env("GLAB_CONFIG_DIR", home.join("glab"))
            .env("GITLAB_TOKEN", self.world.sentinel.expose())
            .env("GITLAB_HOST", "gitlab.com")
            .env("HTTPS_PROXY", &proxy)
            .env("https_proxy", &proxy)
            .env("GLAB_CA_CERT", &self.ca_file)
            .env("NO_PROMPT", "1")
            .env("GLAB_CHECK_UPDATE", "false")
            .env("GLAB_SEND_TELEMETRY", "false")
            .env("DO_NOT_TRACK", "1");
        run(command)
    }

    /// `git` as a chat is given it: the proxy, the chat's authority for the recorded host, and one
    /// credential helper that answers the sentinel.
    fn git(&self, cwd: &Path, args: &[&str]) -> (bool, String) {
        let home = self.dir.path().join("git-home");
        std::fs::create_dir_all(&home).unwrap();
        let proxy = format!("http://127.0.0.1:{}", self.port());
        let helper = format!(
            "!f() {{ echo username=x-access-token; echo password={}; }}; f",
            self.world.sentinel.expose()
        );
        let mut command = Command::new("git");
        clean(&mut command, &home);
        command
            .current_dir(cwd)
            .args(["-c", "user.name=t", "-c", &author_email()])
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_TERMINAL_PROMPT", "0")
            // On a failure the output is the evidence: what curl did, step by step.
            .env("GIT_CURL_VERBOSE", "1")
            .env("GIT_TRACE", "1")
            .env("HTTPS_PROXY", &proxy)
            .env("https_proxy", &proxy)
            .env("GIT_CONFIG_COUNT", "4")
            .env("GIT_CONFIG_KEY_0", "protocol.version")
            .env("GIT_CONFIG_VALUE_0", "0")
            .env("GIT_CONFIG_KEY_1", "http.https://github.com/.sslCAInfo")
            .env("GIT_CONFIG_VALUE_1", &self.ca_file)
            .env("GIT_CONFIG_KEY_2", "credential.helper")
            .env("GIT_CONFIG_VALUE_2", "")
            .env("GIT_CONFIG_KEY_3", "credential.https://github.com.helper")
            .env("GIT_CONFIG_VALUE_3", helper);
        run(command)
    }

    /// Nothing the tools sent was refused, and the forge saw the real token and never the sentinel.
    fn assert_clean(&self) {
        self.assert_clean_but(false);
    }

    /// [`Self::assert_clean`] for `git`, which sends its first request with no credential and is
    /// **challenged** (`401`) for the helper's: each such challenge is audited as
    /// `Refused(NoSentinel)` and is how git is meant to be answered, so it is not a refusal here.
    /// A wrong credential, or any other refusal, still is.
    fn assert_clean_after_git_challenges(&self) {
        self.assert_clean_but(true);
    }

    fn assert_clean_but(&self, challenges_are_fine: bool) {
        use purlis_core::forge::proxy::Refusal;
        let refused: Vec<_> = self
            .world
            .proxy
            .audit(&self.world.chat)
            .into_iter()
            .filter(|r| match r.verdict {
                Verdict::Refused(Refusal::NoSentinel) => !challenges_are_fine,
                Verdict::Refused(_) => true,
                _ => false,
            })
            .collect();
        assert!(
            refused.is_empty(),
            "the proxy refused what a tool sent: {refused:?}"
        );
        let sentinel = self.world.sentinel.expose();
        for request in self.forge.seen.lock().unwrap().iter() {
            let text = format!("{request:?}");
            assert!(
                !text.contains(sentinel),
                "the sentinel reached the forge: {text}"
            );
        }
    }

    /// Whether anything the forge was sent names `needle`, in its address or its body.
    fn forge_was_sent(&self, needle: &str) -> bool {
        let needle = needle.to_ascii_lowercase();
        self.forge.seen.lock().unwrap().iter().any(|r| {
            r.url.to_ascii_lowercase().contains(&needle)
                || String::from_utf8_lossy(&r.body)
                    .to_ascii_lowercase()
                    .contains(&needle)
        })
    }

    /// How many requests the proxy has refused so far.
    fn refusals(&self) -> usize {
        self.world
            .proxy
            .audit(&self.world.chat)
            .into_iter()
            .filter(|r| matches!(r.verdict, Verdict::Refused(_)))
            .count()
    }

    fn forge_saw_the_real_token(&self) -> bool {
        self.forge.seen.lock().unwrap().iter().any(|r| {
            r.headers.iter().any(|(n, v)| {
                n.eq_ignore_ascii_case("authorization")
                    && (v.contains(REAL)
                        || v.strip_prefix("Basic ")
                            .and_then(|b| data_encoding::BASE64.decode(b.as_bytes()).ok())
                            .is_some_and(|d| String::from_utf8_lossy(&d).ends_with(REAL)))
            })
        })
    }
}

#[test]
fn gh_reads_pull_requests_issues_releases_and_repos_through_the_socket() {
    purlis_core::unsteered!();
    if !tool("gh") {
        return;
    }
    let s = setup();
    let mut outcomes = Vec::new();
    let mut refused = Vec::new();
    for args in [
        vec!["api", "user"],
        vec!["api", "graphql", "-f", "query={ viewer { login } }"],
        vec!["api", "repos/acme/app"],
        vec!["pr", "list", "-R", "github.localhost/acme/app"],
        vec!["pr", "view", "1", "-R", "github.localhost/acme/app"],
        vec!["pr", "checks", "1", "-R", "github.localhost/acme/app"],
        vec!["pr", "status", "-R", "github.localhost/acme/app"],
        vec!["issue", "list", "-R", "github.localhost/acme/app"],
        vec!["issue", "view", "1", "-R", "github.localhost/acme/app"],
        vec!["release", "list", "-R", "github.localhost/acme/app"],
        vec!["repo", "view", "github.localhost/acme/app"],
    ] {
        let (ok, text) = s.gh(&args);
        outcomes.push(format!(
            "gh {}: {} {}",
            args.join(" "),
            if ok { "ok" } else { "failed" },
            text.trim()
        ));
        if text.contains("purlis refused") {
            refused.push(format!("gh {}: {}", args.join(" "), text.trim()));
        }
    }
    // Every document a refused command sent, so that one run says everything the list lacks.
    if !refused.is_empty() {
        let documents: Vec<String> = s
            .seen
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, target, _)| target.ends_with("/graphql"))
            .map(|(_, _, body)| body.clone())
            .collect();
        panic!(
            "the proxy refused what gh sent:\n{}\n--- every GraphQL document gh sent:\n{}\n--- outcomes:\n{}",
            refused.join("\n"),
            documents.join("\n---\n"),
            outcomes.join("\n")
        );
    }
    s.assert_clean();
    assert!(s.forge_saw_the_real_token(), "{outcomes:#?}");
    // The calls that cannot depend on a fake's fidelity must have worked.
    for wanted in [
        "gh api user: ok",
        "gh api graphql -f query={ viewer { login } }: ok",
        "gh pr list -R github.localhost/acme/app: ok",
        "gh issue list -R github.localhost/acme/app: ok",
    ] {
        assert!(
            outcomes.iter().any(|o| o.starts_with(wanted)),
            "{wanted}: {outcomes:#?}"
        );
    }
}

#[test]
fn glab_reaches_the_forge_through_connect_and_the_chats_own_authority() {
    purlis_core::unsteered!();
    if !tool("glab") {
        return;
    }
    let s = setup();
    let (ok, text) = s.glab(&["api", "user"]);
    assert!(ok && text.contains("tanuki"), "{text}");
    let (ok, text) = s.glab(&["api", "projects/group%2Fproj"]);
    assert!(ok && text.contains("group/proj"), "{text}");
    let (_, text) = s.glab(&["mr", "list", "-R", "group/proj"]);
    assert!(!text.contains("purlis refused"), "{text}");
    s.assert_clean();
    assert!(s.forge_saw_the_real_token());
}

#[test]
fn glab_lists_pipelines_reads_a_job_log_and_retries_a_pipeline_through_the_route() {
    purlis_core::unsteered!();
    // `tool` fails a run that sets `PURLIS_PROXY_REAL_CLIENTS` (CI does) when `glab` is missing.
    if !tool("glab") {
        return;
    }
    let s = setup();
    let (ok, text) = s.glab(&["ci", "list", "-R", "group/proj"]);
    assert!(ok, "glab ci list: {text}");
    let (ok, text) = s.glab(&["ci", "trace", "5", "-R", "group/proj"]);
    assert!(ok && text.contains("JOBLOGLINE"), "glab ci trace: {text}");
    let (ok, text) = s.glab(&[
        "api",
        "-X",
        "POST",
        "projects/group%2Fproj/pipelines/9/retry",
    ]);
    assert!(ok, "glab api pipeline retry: {text}");
    assert!(
        s.forge_was_sent("pipelines/9/retry"),
        "the retry did not reach the forge"
    );
    // Nothing a tool sent was refused, and the forge saw the real token only.
    s.assert_clean();
    assert!(s.forge_saw_the_real_token());
    // A job's retry can start a manual job at once, so it is not carried: `glab ci retry` fails
    // and the forge never hears of it.
    let (ok, text) = s.glab(&["ci", "retry", "5", "-R", "group/proj"]);
    assert!(!ok, "glab ci retry of a job succeeded: {text}");
    assert!(
        !s.forge_was_sent("jobs/5/retry"),
        "a job retry reached the forge"
    );
}

#[test]
fn gh_and_glab_are_refused_another_repo_a_merge_and_the_token() {
    purlis_core::unsteered!();
    let have_gh = tool("gh");
    let have_glab = tool("glab");
    if !have_gh && !have_glab {
        return;
    }
    let s = setup();
    if have_gh {
        for (what, args, needle) in [
            (
                "another repo, by api",
                vec!["api", "repos/acme/other"],
                "acme/other",
            ),
            (
                "another repo, by pr list",
                vec!["pr", "list", "-R", "github.localhost/acme/other"],
                "other",
            ),
            (
                "a merge, by the REST route",
                vec!["api", "-X", "PUT", "repos/acme/app/pulls/1/merge"],
                "/merge",
            ),
            (
                "a merge, by a mutation",
                vec![
                    "api",
                    "graphql",
                    "-f",
                    "query=mutation{ mergePullRequest(input:{pullRequestId:\"PR_kwDO_1\"}){ clientMutationId } }",
                ],
                "mergepullrequest",
            ),
        ] {
            let before = s.refusals();
            let (ok, text) = s.gh(&args);
            assert!(!ok, "gh, {what}, succeeded: {text}");
            assert!(s.refusals() > before, "gh, {what}, was not refused: {text}");
            assert!(!s.forge_was_sent(needle), "gh, {what}, reached the forge");
            assert!(!text.contains(REAL), "{text}");
        }
        // `gh auth token` prints what the chat holds, which is the sentinel and never the token.
        let (_, text) = s.gh(&["auth", "token"]);
        assert!(
            !text.contains(REAL),
            "gh auth token showed the token: {text}"
        );
    }
    if have_glab {
        for (what, args, needle) in [
            (
                "another repo, by api",
                vec!["api", "projects/other%2Fproj"],
                "other%2fproj",
            ),
            (
                "a merge",
                vec![
                    "api",
                    "-X",
                    "PUT",
                    "projects/group%2Fproj/merge_requests/1/merge",
                ],
                "/merge",
            ),
            (
                "another project's job log, by its numeric id",
                vec!["api", "projects/43/jobs/5/trace"],
                "projects/43",
            ),
            (
                "a manual job's play",
                vec!["api", "-X", "POST", "projects/group%2Fproj/jobs/5/play"],
                "/play",
            ),
            (
                "a protected branch's removal",
                vec![
                    "api",
                    "-X",
                    "DELETE",
                    "projects/group%2Fproj/protected_branches/main",
                ],
                "protected_branches",
            ),
        ] {
            let before = s.refusals();
            let (ok, text) = s.glab(&args);
            assert!(!ok, "glab, {what}, succeeded: {text}");
            assert!(
                s.refusals() > before,
                "glab, {what}, was not refused: {text}"
            );
            assert!(!s.forge_was_sent(needle), "glab, {what}, reached the forge");
            assert!(!text.contains(REAL), "{text}");
        }
    }
}

/// `guards.merge` = `log`: the same merges the test above sees refused go through, each recorded
/// once for the chat and the repo, and what stays refused is still refused.
#[test]
fn gh_and_glab_merge_where_guards_merge_is_log_and_are_still_refused_another_repo_and_the_token() {
    purlis_core::unsteered!();
    let have_gh = tool("gh");
    let have_glab = tool("glab");
    if !have_gh && !have_glab {
        return;
    }
    let lines: Arc<Mutex<Vec<String>>> = Arc::default();
    let kept = lines.clone();
    let stance = purlis_core::forge::mergestance::PolicyMergeStance::reading(
        || purlis_core::enforcement::Mode::Log,
        move |_, said| kept.lock().unwrap().push(said.to_owned()),
    );
    let s = setup_in(None, World::with_merge_stance(Arc::new(stance)));
    if have_gh {
        let (ok, text) = s.gh(&["api", "-X", "PUT", "repos/acme/app/pulls/1/merge"]);
        assert!(ok, "gh, a merge under log: {text}");
        assert!(s.forge_was_sent("pulls/1/merge"), "the merge was not sent");
        // Another repo, and the token, stay out of reach.
        let before = s.refusals();
        let (ok, text) = s.gh(&["api", "-X", "PUT", "repos/acme/other/pulls/1/merge"]);
        assert!(!ok && s.refusals() > before, "another repo's merge: {text}");
        assert!(!s.forge_was_sent("acme/other"));
        let (_, text) = s.gh(&["auth", "token"]);
        assert!(
            !text.contains(REAL),
            "gh auth token showed the token: {text}"
        );
    }
    if have_glab {
        let (ok, text) = s.glab(&[
            "api",
            "-X",
            "PUT",
            "projects/group%2Fproj/merge_requests/1/merge",
        ]);
        assert!(
            !text.contains("purlis refused"),
            "glab, a merge under log: {text}"
        );
        assert!(s.forge_was_sent("merge_requests/1/merge"), "{ok} {text}");
    }
    let expected = usize::from(have_gh) + usize::from(have_glab);
    let lines = lines.lock().unwrap();
    assert_eq!(lines.len(), expected, "one line for each merge: {lines:?}");
    assert!(
        lines
            .iter()
            .all(|said| said.ends_with("acme/app") || said.ends_with("group/proj"))
    );
}

#[test]
fn git_clones_fetches_and_pushes_a_branch_it_made_but_not_the_default_branch() {
    purlis_core::unsteered!();
    if !tool("git") {
        return;
    }
    let s = setup();
    let work = s.dir.path().join("clone");
    let (ok, text) = s.git(
        s.dir.path(),
        &[
            "clone",
            "https://github.com/acme/app.git",
            work.to_str().unwrap(),
        ],
    );
    assert!(ok, "{text}");
    assert!(work.join("README.md").exists());
    assert!(s.forge_saw_the_real_token());

    // A new branch of the chat's own.
    std::fs::write(work.join("a.txt"), "a\n").unwrap();
    git_ok(&work, &["add", "-A"]);
    git_ok(&work, &["commit", "-q", "-m", "a"]);
    let (ok, text) = s.git(&work, &["push", "origin", "HEAD:refs/heads/mine"]);
    assert!(ok, "{text}");
    // Moving it again is allowed: the forge took the first push, so it is the chat's own.
    std::fs::write(work.join("b.txt"), "b\n").unwrap();
    git_ok(&work, &["add", "-A"]);
    git_ok(&work, &["commit", "-q", "-m", "b"]);
    let (ok, text) = s.git(&work, &["push", "origin", "HEAD:refs/heads/mine"]);
    assert!(ok, "{text}");
    let refs = purlis_core::forklock::output(
        Command::new("git")
            .args(["branch", "--list"])
            .current_dir(&s.forge.bare),
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&refs.stdout).contains("mine"));

    // The default branch, a tag and a delete are refused by the proxy, and nothing reaches the forge.
    let before = s.forge.seen.lock().unwrap().len();
    git_ok(&work, &["tag", "v1"]);
    for args in [
        vec!["push", "origin", "HEAD:refs/heads/main"],
        vec!["push", "origin", "v1"],
        vec!["push", "origin", ":refs/heads/mine"],
        vec!["push", "origin", "HEAD:refs/heads/release"],
    ] {
        let (ok, text) = s.git(&work, &args);
        assert!(!ok, "git {args:?} was allowed: {text}");
        // Git says why, in its own output, and not "RPC failed; HTTP 403".
        assert!(
            text.contains("[remote rejected]") && !text.contains("RPC failed"),
            "git {args:?}: {text}"
        );
    }
    let posts = s
        .forge
        .seen
        .lock()
        .unwrap()
        .iter()
        .skip(before)
        .filter(|r| r.method == "POST")
        .count();
    assert_eq!(posts, 0, "a refused push reached the forge");
}

/// A gate that holds a push of a branch named `ci-…`, as the app's holds a push that changes CI
/// files, and lets every other through. The app's own gate reads the push's objects in a
/// quarantine; this is the listener's side of it: a held push is answered and sent nowhere.
struct HoldsCiBranches;

impl PushGate for HoldsCiBranches {
    fn check(&self, _: &purlis_core::forge::proxy::ChatId, push: &PushRead) -> PushVerdict {
        if push
            .updates
            .iter()
            .any(|u| u.name.starts_with("refs/heads/ci-"))
        {
            PushVerdict::Held("Held: Ask 1, a CI-file push in acme/app.".to_owned())
        } else {
            PushVerdict::Forward
        }
    }
}

#[test]
fn the_apps_own_git_for_a_chat_pushes_through_the_listener_and_a_held_push_reaches_the_forge_never()
{
    purlis_core::unsteered!();
    if !tool("git") {
        return;
    }
    use purlis_core::worktree::git::{Isolated, isolated, run_network};
    let s = setup_with(Some(Arc::new(HoldsCiBranches)));
    let work = s.dir.path().join("clone");
    let (ok, text) = s.git(
        s.dir.path(),
        &[
            "clone",
            "https://github.com/acme/app.git",
            work.to_str().unwrap(),
        ],
    );
    assert!(ok, "{text}");
    std::fs::write(work.join("a.txt"), "a\n").unwrap();
    git_ok(&work, &["add", "-A"]);
    git_ok(&work, &["commit", "-q", "-m", "a"]);

    // What the app's git is given for the chat (`forgewire::git_network`), with the test's own
    // sentinel helper in place of purlis's binary: the proxy, the chat's authority, no login.
    let route = production_route(&s, s.app_port(), &s.ca_file);
    let isolation = Isolated::default().through_route(route);
    let push = |to: &str| {
        isolated(&isolation, || {
            // The helper a caller names is ignored under a route: the CLI's login is never asked.
            run_network(
                &work,
                Some("!'/nonexistent/gh' auth git-credential"),
                &["push", "origin", &format!("HEAD:refs/heads/{to}")],
            )
        })
        .expect("git ran")
    };
    let sent = push("brokered");
    assert!(sent.ok(), "{}", sent.err);
    assert!(
        s.forge_saw_the_real_token(),
        "the forge was not sent the real token"
    );
    let refs = purlis_core::forklock::output(
        Command::new("git")
            .args(["branch", "--list"])
            .current_dir(&s.forge.bare),
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&refs.stdout).contains("brokered"));

    // A push the gate holds is answered by the listener and reaches the forge never.
    let before = s
        .forge
        .seen
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.method == "POST")
        .count();
    let held = push("ci-workflow");
    assert!(!held.ok(), "a held push was sent");
    // Git prints purlis's reason, not "RPC failed; HTTP 403" (#7486).
    assert!(
        held.err.contains("[remote rejected]")
            && held
                .err
                .contains("Held: Ask 1, a CI-file push in acme/app.")
            && !held.err.contains("RPC failed"),
        "{}",
        held.err
    );
    let after = s
        .forge
        .seen
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.method == "POST")
        .count();
    assert_eq!(before, after, "a held push reached the forge");
}

/// #7693: a command a chat runs itself (`purlis change push`) reads the chat's route out of the
/// environment the chat was given, and its git pushes through the chat's own proxy, trusting the
/// chat's own authority, as the chat's git does: it is not refused as "unable to get local issuer
/// certificate", and the forge is sent the real token, never the sentinel.
#[test]
fn a_chats_own_route_read_from_its_environment_pushes_through_the_listener_trusting_its_authority()
{
    purlis_core::unsteered!();
    if !tool("git") {
        return;
    }
    use purlis_core::forgewire::{ForgeKind, Repo, Route, SecretString, apply, compose_at};
    use purlis_core::worktree::git::{Isolated, isolated, run_network};
    let s = setup();
    let work = s.dir.path().join("clone");
    let (ok, text) = s.git(
        s.dir.path(),
        &[
            "clone",
            "https://github.com/acme/app.git",
            work.to_str().unwrap(),
        ],
    );
    assert!(ok, "{text}");
    std::fs::write(work.join("a.txt"), "a\n").unwrap();
    git_ok(&work, &["add", "-A"]);
    git_ok(&work, &["commit", "-q", "-m", "a"]);

    // The chat's environment, as the app wires it for the chat's route; the test's sentinel
    // helper stands in for purlis's own binary, as `production_route` has it.
    let helper = sentinel_helper(&s);
    let route = Route {
        egress_url: format!("http://127.0.0.1:{}", s.port()),
        gh_socket: s.dir.path().join("unused.sock"),
        ca_cert: s.ca_file.clone(),
        sentinel: SecretString::from(s.world.sentinel.expose().to_owned()),
        helper_exe: PathBuf::from("/nonexistent/purlis"),
        repos: vec![Repo::new("github.com", "acme", "app", ForgeKind::GitHub).unwrap()],
    };
    let wiring = compose_at(&route, &s.dir.path().join("wire")).expect("the chat's wiring");
    let env = apply(Vec::new(), &wiring);
    let lookup = |name: &str| {
        env.iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    let read = purlis_core::forgewire::this_chats_git_route(&lookup, &helper)
        .expect("the chat's environment holds a route")
        .expect("a route git can use");
    let pushed = isolated(&Isolated::default().through_route(read), || {
        run_network(
            &work,
            Some("!'/nonexistent/gh' auth git-credential"),
            &["push", "origin", "HEAD:refs/heads/from-the-chats-command"],
        )
    })
    .expect("git ran");
    assert!(pushed.ok(), "{}", pushed.err);
    assert!(
        s.forge_saw_the_real_token(),
        "the forge was not sent the real token"
    );
}

/// A gate that refuses a push of a branch named `later-…` as it refuses one whose base is not
/// readable yet, and holds one named `ci-…`: either verdict, for any ref of the push, refuses the
/// whole push.
struct RefusesAndHolds;

impl PushGate for RefusesAndHolds {
    fn check(&self, _: &purlis_core::forge::proxy::ChatId, push: &PushRead) -> PushVerdict {
        for update in &push.updates {
            if update.name.starts_with("refs/heads/later-") {
                return PushVerdict::Refuse(
                    "purlis did not send this push: its base is not read yet, which is temporary; try again in a minute."
                        .to_owned(),
                );
            }
            if update.name.starts_with("refs/heads/ci-") {
                return PushVerdict::Held(
                    "purlis holds this push: it changes CI files and waits in Asks.".to_owned(),
                );
            }
        }
        PushVerdict::Forward
    }
}

#[test]
fn a_push_the_gate_refuses_or_holds_says_why_in_gits_own_output_for_each_ref_and_exits_non_zero() {
    purlis_core::unsteered!();
    if !tool("git") {
        return;
    }
    use purlis_core::worktree::git::{Isolated, isolated, run_network};
    let s = setup_with(Some(Arc::new(RefusesAndHolds)));
    let work = s.dir.path().join("clone");
    let (ok, text) = s.git(
        s.dir.path(),
        &[
            "clone",
            "https://github.com/acme/app.git",
            work.to_str().unwrap(),
        ],
    );
    assert!(ok, "{text}");
    std::fs::write(work.join("a.txt"), "a\n").unwrap();
    git_ok(&work, &["add", "-A"]);
    git_ok(&work, &["commit", "-q", "-m", "a"]);
    let route = production_route(&s, s.app_port(), &s.ca_file);
    let isolation = Isolated::default().through_route(route);
    // Every protocol git speaks to a server here: v0 (what a push uses by default) and v2.
    for version in ["0", "2"] {
        let config = format!("protocol.version={version}");
        let push = |refs: &[&str]| {
            let mut args = vec!["-c", config.as_str(), "push", "origin"];
            args.extend(refs);
            isolated(&isolation, || {
                run_network(&work, Some("!'/nonexistent/gh' auth git-credential"), &args)
            })
            .expect("git ran")
        };
        let refused = push(&["HEAD:refs/heads/later-one"]);
        assert!(!refused.ok(), "v{version}: a refused push was sent");
        assert!(
            refused.err.contains("[remote rejected]")
                && refused
                    .err
                    .contains("which is temporary; try again in a minute")
                && !refused.err.contains("RPC failed")
                && !refused.err.contains("Everything up-to-date"),
            "v{version}: {}",
            refused.err
        );
        let held = push(&["HEAD:refs/heads/ci-one"]);
        assert!(!held.ok(), "v{version}: a held push was sent");
        assert!(
            held.err.contains("[remote rejected]")
                && held.err.contains("waits in Asks")
                && !held.err.contains("RPC failed"),
            "v{version}: {}",
            held.err
        );
        // Two refs at once: git names each of them.
        let both = push(&["HEAD:refs/heads/later-two", "HEAD:refs/heads/later-three"]);
        assert!(!both.ok(), "v{version}: a refused push was sent");
        assert!(
            both.err.contains("later-two") && both.err.contains("later-three"),
            "v{version}: {}",
            both.err
        );
    }
}

/// Bytes no compression makes smaller, so a commit holding them makes a pack at least as big.
fn incompressible(len: usize) -> Vec<u8> {
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state.to_le_bytes()[0]
        })
        .collect()
}

#[test]
fn a_held_push_of_an_explicit_sha_to_a_branch_says_why_whatever_its_size() {
    purlis_core::unsteered!();
    if !tool("git") {
        return;
    }
    use purlis_core::worktree::git::{Isolated, isolated, run_network};
    let s = setup_with(Some(Arc::new(RefusesAndHolds)));
    // The pull request's branch is on the forge already, so `<sha>:ci-pr` names it.
    git_ok(&s.forge.bare, &["branch", "ci-pr", "main"]);
    let work = s.dir.path().join("clone");
    let (ok, text) = s.git(
        s.dir.path(),
        &[
            "clone",
            "https://github.com/acme/app.git",
            work.to_str().unwrap(),
        ],
    );
    assert!(ok, "{text}");
    std::fs::create_dir_all(work.join(".github/workflows")).unwrap();
    std::fs::write(work.join(".github/workflows/ci.yml"), "on: push\n").unwrap();
    // A pack larger than git's post buffer is announced by a probe request (a lone flush packet)
    // before the push itself is sent.
    std::fs::write(work.join("big.bin"), incompressible(300 * 1024)).unwrap();
    git_ok(&work, &["add", "-A"]);
    git_ok(&work, &["commit", "-q", "-m", "ci"]);
    let sha = {
        let out = purlis_core::forklock::output(
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&work),
        )
        .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    let route = production_route(&s, s.app_port(), &s.ca_file);
    let isolation = Isolated::default().through_route(route);
    for version in ["0", "2"] {
        let config = format!("protocol.version={version}");
        for buffer in ["1m", "64k"] {
            let buffer = format!("http.postBuffer={buffer}");
            for refspec in [format!("{sha}:ci-pr"), format!("{sha}:refs/heads/ci-pr")] {
                let args = [
                    "-c",
                    config.as_str(),
                    "-c",
                    buffer.as_str(),
                    "push",
                    "origin",
                    refspec.as_str(),
                ];
                let held = isolated(&isolation, || {
                    run_network(&work, Some("!'/nonexistent/gh' auth git-credential"), &args)
                })
                .expect("git ran");
                let case = format!("v{version}, {buffer}, {refspec}");
                assert!(!held.ok(), "{case}: a held push was sent");
                assert!(
                    held.err.contains("[remote rejected]")
                        && held.err.contains("waits in Asks")
                        && !held.err.contains("hung up")
                        && !held.err.contains("Everything up-to-date"),
                    "{case}: {}",
                    held.err
                );
            }
        }
    }
    let tip = purlis_core::forklock::output(
        Command::new("git")
            .args(["rev-parse", "refs/heads/ci-pr"])
            .current_dir(&s.forge.bare),
    )
    .unwrap();
    assert_ne!(
        String::from_utf8_lossy(&tip.stdout).trim(),
        sha,
        "the held push reached the forge"
    );
}

/// A pin on `github.com`, as policy would set it.
struct PinsGithub;

impl Pin for PinsGithub {
    fn pinned(&self, host: &str, _: u16) -> Option<String> {
        (host == "github.com").then(|| {
            "Policy never allows github.com. Locked by policy, set by root in /etc/purlis/policy.toml."
                .to_owned()
        })
    }
}

#[test]
fn the_apps_own_git_for_a_chat_is_refused_a_pinned_forge_and_the_door_is_never_dialled() {
    purlis_core::unsteered!();
    if !tool("git") {
        return;
    }
    use purlis_core::worktree::git::{Isolated, isolated, run_network};
    let mut s = setup();
    let EndpointKind::Loopback(door) = s.connect.kind().clone() else {
        unreachable!()
    };
    // The production relay, with the administrator's pin on github.com.
    s.app_relay = app_relay(door, &s.world, Arc::new(PinsGithub));
    let work = s.dir.path().join("clone");
    let (ok, text) = s.git(
        s.dir.path(),
        &[
            "clone",
            "https://github.com/acme/app.git",
            work.to_str().unwrap(),
        ],
    );
    assert!(ok, "{text}");
    std::fs::write(work.join("a.txt"), "a\n").unwrap();
    git_ok(&work, &["add", "-A"]);
    git_ok(&work, &["commit", "-q", "-m", "a"]);
    let route = production_route(&s, s.app_port(), &s.ca_file);
    let isolation = Isolated::default().through_route(route);
    let before = s.forge.seen.lock().unwrap().len();
    let pushed = isolated(&isolation, || {
        run_network(
            &work,
            Some("!'/nonexistent/gh' auth git-credential"),
            &["push", "origin", "HEAD:refs/heads/pinned"],
        )
    })
    .expect("git ran");
    assert!(!pushed.ok(), "a push to a pinned forge was carried");
    assert!(
        pushed.err.contains("403"),
        "refused by the relay's pin, not by something else: {}",
        pushed.err
    );
    assert_eq!(
        before,
        s.forge.seen.lock().unwrap().len(),
        "the forge saw a request for a pinned forge"
    );
}

#[test]
fn the_apps_relay_answers_a_pinned_forge_and_its_api_host_with_the_pin_and_dials_no_door() {
    purlis_core::unsteered!();
    // A door that notes every connection.
    let door = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    door.set_nonblocking(true).unwrap();
    let world = World::new();
    let relay = DoorRelay::start(
        ForgeRoute {
            addr: door.local_addr().unwrap(),
            handoff: world.wiring.handoff.clone(),
            hosts: vec!["github.com".to_owned(), "api.github.com".to_owned()],
        },
        Arc::new(PinsAllOfGithub),
        std::time::Duration::from_secs(5),
    )
    .unwrap();
    for host in ["github.com", "api.github.com"] {
        use std::io::Read;
        let mut client = std::net::TcpStream::connect(relay.addr()).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        write!(
            client,
            "CONNECT {host}:443 HTTP/1.1\r\nHost: {host}:443\r\n\r\n"
        )
        .unwrap();
        let mut said = String::new();
        let _ = client.read_to_string(&mut said);
        assert!(said.starts_with("HTTP/1.1 403"), "{host}: {said}");
        assert!(
            said.contains("signing in does not change"),
            "{host}: {said}"
        );
    }
    assert!(
        door.accept().is_err(),
        "the door was dialled for a pinned forge"
    );
}

/// The pin as the policy answers it: `github.com` pinned, so its forge's other names are too.
struct PinsAllOfGithub;

impl Pin for PinsAllOfGithub {
    fn pinned(&self, host: &str, _: u16) -> Option<String> {
        (host == "github.com").then(|| {
            "Policy never allows github.com. Locked by policy, set by root in x. \
             An administrator pinned this forge, so no chat reaches it and signing in does not \
             change that."
                .to_owned()
        })
    }
}

/// More commits than a scan reads for a remote it cannot ask (`UNASKED_AT_MOST`), made on top of
/// `main` in `work` with one `fast-import`, as `refs/heads/many`: the commit it ends at.
fn more_commits_than_an_unasked_scan_reads(work: &Path) -> String {
    let mut stream = String::new();
    for n in 0..=purlis_core::diffscan::UNASKED_AT_MOST {
        let from = if n == 0 { "from refs/heads/main\n" } else { "" };
        stream.push_str(&format!(
            "commit refs/heads/many\ncommitter t <t{}example.invalid> {n} +0000\ndata 2\n{}\n{from}M 100644 inline f.txt\ndata 2\n{}\n\n",
            '@',
            n % 10,
            n % 10
        ));
    }
    let mut child = purlis_core::forklock::spawn(
        Command::new("git")
            .args(["fast-import", "--quiet"])
            .current_dir(work)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped()),
    )
    .expect("git fast-import");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stream.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let id = purlis_core::forklock::output(
        Command::new("git")
            .args(["rev-parse", "refs/heads/many"])
            .current_dir(work),
    )
    .unwrap();
    String::from_utf8_lossy(&id.stdout).trim().to_owned()
}

/// What the app's git is given for a chat, **made by production's own builder**
/// (`forgewire::git_network`), so a test proves the real route and not a copy of it. Only two
/// things are the test's: the helper, a script that answers the sentinel from the variable the
/// route passes (in place of purlis's binary), and the proxy and authority, which are the test's
/// listeners'.
fn production_route(
    s: &Setup,
    proxy_port: u16,
    ca: &Path,
) -> purlis_core::worktree::git::NetworkRoute {
    use purlis_core::forgewire::{ForgeKind, Repo, Route, SecretString, git_network};
    let helper = sentinel_helper(s);
    let route = Route {
        egress_url: format!("http://127.0.0.1:{proxy_port}"),
        gh_socket: s.dir.path().join("unused.sock"),
        ca_cert: ca.to_path_buf(),
        sentinel: SecretString::from(s.world.sentinel.expose().to_owned()),
        helper_exe: helper,
        repos: vec![Repo::new("github.com", "acme", "app", ForgeKind::GitHub).unwrap()],
    };
    git_network(&route).expect("production's route for the app's git")
}

/// The test's credential helper, in place of purlis's binary: a script that answers the sentinel
/// from the variable the route passes.
fn sentinel_helper(s: &Setup) -> PathBuf {
    use purlis_core::forge::gitcredential::SENTINEL_ENV;
    let helper = s.dir.path().join("sentinel-helper.sh");
    if !helper.exists() {
        std::fs::write(
            &helper,
            format!(
                "#!/bin/sh\ncat >/dev/null\necho username=x-access-token\necho \"password=${SENTINEL_ENV}\"\n"
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    helper
}

/// The route of the chat's own git stand-in (the test-support relay).
fn the_chats_route(s: &Setup, ca: &Path) -> purlis_core::worktree::git::NetworkRoute {
    production_route(s, s.port(), ca)
}

/// How many times the forge has been asked which commits it has. A test's own `clone` asks it
/// too, so a test compares the count before and after the call it means to test.
fn times_the_forge_was_asked_which_commits_it_has(s: &Setup) -> usize {
    s.forge
        .seen
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.url.contains("info/refs") && r.url.ends_with("service=git-upload-pack"))
        .count()
}

/// #7302: the app asks the forge for a chat's hook the way the chat's git reaches it, and what it
/// tells the hook is what the scan leaves out: a branch of more commits than an unasked scan reads
/// is scanned knowing the remote's tips.
#[test]
fn the_tips_the_app_asks_through_the_chats_route_are_what_a_pushs_scan_leaves_out() {
    purlis_core::unsteered!();
    if !tool("git") {
        return;
    }
    let s = setup();
    let work = s.dir.path().join("clone");
    let (ok, text) = s.git(
        s.dir.path(),
        &[
            "clone",
            "https://github.com/acme/app.git",
            work.to_str().unwrap(),
        ],
    );
    assert!(ok, "{text}");
    let tip = more_commits_than_an_unasked_scan_reads(&work);
    let updates = format!("refs/heads/many {tip} refs/heads/many {}\n", "0".repeat(40));
    let route = the_chats_route(&s, &s.ca_file);
    let url = "https://github.com/acme/app.git";

    let asked_before = times_the_forge_was_asked_which_commits_it_has(&s);
    let tips = purlis_core::diffscan::remote_tips_through(url, route)
        .expect("the app asked the forge on the chat's route");
    assert!(!tips.is_empty());
    assert!(
        times_the_forge_was_asked_which_commits_it_has(&s) > asked_before,
        "the app never reached the forge through the route"
    );
    let scan = purlis_core::diffscan::pushed_told(&work, url, &updates, &|_| Some(tips.clone()))
        .expect("the remote's tips were told, so what it has is left out of the scan");
    assert!(scan.refused.is_empty(), "{:?}", scan.refused);
    // The ask carried the sentinel (it was challenged, then answered): the forge was sent the
    // real token and the proxy refused nothing but git's own first, credential-less request.
    assert!(
        s.forge_saw_the_real_token(),
        "the forge was not sent the real token"
    );
    s.assert_clean_after_git_challenges();
}

/// A route that is not the chat's own authority answers nothing, so nothing is told, and a hook
/// that is told nothing reads the push in full and refuses, as it always has.
#[test]
fn a_route_that_fails_the_chats_authority_tells_no_tips_and_the_push_is_refused() {
    purlis_core::unsteered!();
    if !tool("git") {
        return;
    }
    let s = setup();
    let work = s.dir.path().join("clone");
    let (ok, text) = s.git(
        s.dir.path(),
        &[
            "clone",
            "https://github.com/acme/app.git",
            work.to_str().unwrap(),
        ],
    );
    assert!(ok, "{text}");
    let tip = more_commits_than_an_unasked_scan_reads(&work);
    let updates = format!("refs/heads/many {tip} refs/heads/many {}\n", "0".repeat(40));
    // An authority that is not the chat's: the listener's certificate is not signed under it.
    let forged = s.dir.path().join("forged-ca.pem");
    let elsewhere = ForgeListener::serve_connect(World::new().wiring.clone()).unwrap();
    std::fs::write(&forged, elsewhere.ca_certificate_pem().unwrap()).unwrap();
    let route = the_chats_route(&s, &forged);
    let url = "https://github.com/acme/app.git";

    assert!(purlis_core::diffscan::remote_tips_through(url, route).is_none());
    // Nothing told: the push is read in full. This is what `pushed_told` does with no answer, but
    // it is called directly: `pushed_told` would next ask github.com itself with `gh auth
    // git-credential`, which reads the OS keyring (Secret Service over D-Bus on Linux) and, in CI,
    // starts a session bus no test may leave behind.
    let refused = purlis_core::diffscan::pushed_without_asking(&work, &updates);
    assert!(refused.is_err(), "{refused:?}");
}

/// What a repository's own configuration says of the forge (a proxy, an authority, a helper) is
/// not what the scan asks it by: a chat can write it.
#[test]
fn a_proxy_and_an_authority_a_chat_wrote_into_the_repository_are_not_what_the_scan_asks_by() {
    purlis_core::unsteered!();
    if !tool("git") {
        return;
    }
    let s = setup();
    let work = s.dir.path().join("clone");
    let (ok, text) = s.git(
        s.dir.path(),
        &[
            "clone",
            "https://github.com/acme/app.git",
            work.to_str().unwrap(),
        ],
    );
    assert!(ok, "{text}");
    let proxy = format!("http://127.0.0.1:{}", s.port());
    for (key, value) in [
        ("http.https://github.com/.proxy", proxy.as_str()),
        ("http.https://git.example.test/.proxy", proxy.as_str()),
        ("http.proxy", proxy.as_str()),
        ("http.sslCAInfo", s.ca_file.to_str().unwrap()),
        (
            "http.https://github.com/.sslCAInfo",
            s.ca_file.to_str().unwrap(),
        ),
        (
            "http.https://git.example.test/.sslCAInfo",
            s.ca_file.to_str().unwrap(),
        ),
    ] {
        git_ok(&work, &["config", key, value]);
    }
    let tip = more_commits_than_an_unasked_scan_reads(&work);
    let updates = format!("refs/heads/many {tip} refs/heads/many {}\n", "0".repeat(40));

    let asked_before = times_the_forge_was_asked_which_commits_it_has(&s);
    // Asked of an address on no forge purlis knows: for a forge's it would run `gh auth
    // git-credential` against github.com, which reads the OS keyring (D-Bus on Linux).
    let scanned =
        purlis_core::diffscan::pushed(&work, "https://git.example.test/acme/app.git", &updates);

    assert_eq!(
        times_the_forge_was_asked_which_commits_it_has(&s),
        asked_before,
        "the scan asked through what the repository's config named"
    );
    assert!(scanned.is_err(), "{scanned:?}");
}
