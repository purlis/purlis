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

use purlis_core::forge::proxy::listener::{
    ForgeListener, Upstream, UpstreamError, UpstreamRequest, UpstreamResponse,
};
use purlis_core::forge::proxy::{EndpointKind, Verdict};

use crate::support::forge_listener::{REAL, World};

/// A fake forge that answers what the tools ask, with git served by `git` itself from a bare repo.
struct Forge {
    bare: PathBuf,
    seen: Mutex<Vec<UpstreamRequest>>,
    captured: AtomicUsize,
}

impl Forge {
    fn json(&self, body: &str) -> Reply {
        Reply::new(200, "application/json", body.as_bytes().to_vec())
    }

    fn git(&self, rpc: &str, advertise: bool, body: &[u8]) -> Reply {
        let mut command = Command::new("git");
        command.arg(rpc).arg("--stateless-rpc");
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
            ("GET", "gitlab.com/api/v4/user") => self.json(r#"{"id":1,"username":"tanuki"}"#),
            ("GET", p) if p.starts_with("gitlab.com/api/v4/projects/group%2Fproj/merge_requests") => {
                self.json("[]")
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

fn tool(name: &str) -> bool {
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
fn author_email() -> String {
    format!("user.email=t{}example.invalid", '@')
}

fn git_ok(dir: &Path, args: &[&str]) {
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

struct Setup {
    world: World,
    forge: Arc<Forge>,
    dir: tempfile::TempDir,
    socket: ForgeListener,
    connect: ForgeListener,
    ca_file: PathBuf,
}

fn setup() -> Setup {
    let dir = tempfile::Builder::new()
        .prefix("fp")
        .tempdir_in("/tmp")
        .unwrap();
    // A repo to serve: one commit on `main`.
    let work = dir.path().join("work");
    std::fs::create_dir(&work).unwrap();
    git_ok(&work, &["init", "-q", "-b", "main"]);
    std::fs::write(work.join("README.md"), "hello\n").unwrap();
    git_ok(&work, &["add", "-A"]);
    git_ok(&work, &["commit", "-q", "-m", "one"]);
    let bare = dir.path().join("forge.git");
    git_ok(
        dir.path(),
        &[
            "clone",
            "-q",
            "--bare",
            work.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    git_ok(&bare, &["config", "http.receivepack", "true"]);

    let forge = Arc::new(Forge {
        bare,
        seen: Mutex::new(Vec::new()),
        captured: AtomicUsize::new(0),
    });
    let mut world = World::new();
    world.wiring.upstream = forge.clone();
    let socket =
        ForgeListener::serve_socket(world.wiring.clone(), &dir.path().join("sock")).unwrap();
    let connect = ForgeListener::serve_connect(world.wiring.clone()).unwrap();
    let ca_file = dir.path().join("ca.pem");
    std::fs::write(&ca_file, connect.ca_certificate_pem().unwrap()).unwrap();
    Setup {
        world,
        forge,
        dir,
        socket,
        connect,
        ca_file,
    }
}

fn clean(command: &mut Command, home: &Path) {
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

fn run(mut command: Command) -> (bool, String) {
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
    fn port(&self) -> u16 {
        match self.connect.kind() {
            EndpointKind::Loopback(a) => a.port(),
            _ => unreachable!(),
        }
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
        let refused: Vec<_> = self
            .world
            .proxy
            .audit(&self.world.chat)
            .into_iter()
            .filter(|r| matches!(r.verdict, Verdict::Refused(_)))
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
            panic!("gh {} was refused: {text}", args.join(" "));
        }
    }
    s.assert_clean();
    assert!(s.forge_saw_the_real_token(), "{outcomes:#?}");
    // The calls that cannot depend on a fake's fidelity must have worked.
    for wanted in [
        "gh api user: ok",
        "gh api graphql -f query={ viewer { login } }: ok",
        "gh pr list -R github.localhost/acme/app: ok",
        "gh issue list -R github.localhost/acme/app: ok",
        "gh release list -R github.localhost/acme/app: ok",
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
