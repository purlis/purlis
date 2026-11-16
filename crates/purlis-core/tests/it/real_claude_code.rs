// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the pinned Claude Code does with what purlis writes** (T1a-15; spec Testing Decisions,
//! B-3, C-12): the real binary, at the version `tests/fixtures/harness-versions/claude.txt`
//! names, which CI installs from npm with no install scripts run and hands in
//! `PURLIS_CLAUDE_CODE`. The adapter's own output is tested on `crates/fake-harness`
//! (`adapter_writes_…`); what Claude Code then does with it is tested here, by name.
//!
//! CI runs this module by name on Linux and macOS (`ci.yml`'s *Claude Code, real binary* steps),
//! through `tools/tests-ran.sh`, which fails when it ran none, and with [`REQUIRED`] set, so a
//! test that would skip fails instead. Elsewhere, without `PURLIS_CLAUDE_CODE`, each test says
//! why it skipped and passes: a local run only.
//!
//! No model is called and no credential is used: the model is a stand-in Messages server on a
//! loopback port the test binds, recording every request, and the run's token is a literal only
//! it reads.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use purlis_core::harness::claude::route;
use purlis_core::harness::{Harness, Kit, StateHooks};
use purlis_core::keyroute::{KeyRoute, Listener};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Where the pinned `claude` program is, as CI hands it.
const PINNED: &str = "PURLIS_CLAUDE_CODE";

/// Set in the CI job that installs the pin: a test here that would skip fails instead.
const REQUIRED: &str = "PURLIS_REAL_BINARY_REQUIRED";

/// The version of Claude Code purlis supports, as it answers `--version`.
const SUPPORTED: &str = include_str!("../../../../tests/fixtures/harness-versions/claude.txt");

/// The CI workflow, which runs this module by name.
const CI: &str = include_str!("../../../../.github/workflows/ci.yml");

/// How long one run of the pinned Claude Code may take before the test ends it.
const PATIENCE: Duration = Duration::from_secs(120);

/// **The pinned Claude Code**, or `None` where it is not handed in and this is a local run,
/// having said so. In the job that installs the pin ([`REQUIRED`]), a missing or wrong binary
/// fails the test.
fn pinned(test: &str) -> Option<PathBuf> {
    let required = std::env::var_os(REQUIRED).is_some_and(|v| !v.is_empty());
    let Some(path) = std::env::var_os(PINNED).filter(|p| !p.is_empty()) else {
        assert!(
            !required,
            "{test}: {REQUIRED} is set and {PINNED} is not, so this real-binary test would skip"
        );
        println!("{test}: skipped, {PINNED} is not set (a local run only)");
        return None;
    };
    let path = PathBuf::from(path);
    assert!(path.is_file(), "{PINNED} names no file: {}", path.display());
    Some(path)
}

/// The pinned binary's bytes, or `None` where it is not handed in.
fn pinned_bytes(test: &str) -> Option<Vec<u8>> {
    let path = pinned(test)?;
    Some(std::fs::read(&path).expect("the pinned binary reads"))
}

/// R-248 (C-40): the floor names every `ANTHROPIC_GOOGLE_CLOUD_` variable in the pinned binary.
#[test]
fn the_floor_names_every_google_cloud_variable_in_the_pinned_binary() {
    let Some(binary) =
        pinned_bytes("the_floor_names_every_google_cloud_variable_in_the_pinned_binary")
    else {
        return;
    };

    let found = route::names_in(&binary, route::GOOGLE_CLOUD_PREFIX);

    assert!(!found.is_empty(), "no Google Cloud variable in the binary");
    assert_eq!(
        route::google_cloud_names_not_in_the_floor(&binary),
        Vec::<String>::new(),
        "the pinned binary reads these and the floor doesn't blank them"
    );
}

/// R-248 (X8): every `CLAUDE_CODE_USE_` name in the pinned binary is blanked or known not to be
/// a route.
#[test]
fn every_use_switch_in_the_pinned_binary_is_blanked_or_known_not_a_route() {
    let Some(binary) =
        pinned_bytes("every_use_switch_in_the_pinned_binary_is_blanked_or_known_not_a_route")
    else {
        return;
    };

    let found = route::names_in(&binary, route::SWITCH_PREFIX);

    for (blanked, _) in route::BLANKED_SWITCHES {
        assert!(found.contains(blanked), "{blanked} is not in the binary");
    }
    assert_eq!(
        route::switches_in_neither_list(&binary),
        Vec::<String>::new(),
        "a switch in neither list: blank it, or say why it routes nothing"
    );
}

/// One Messages answer, streamed: a turn that says `ok` and ends.
const TURN: &str = "event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_fixture\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-fixture\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":3,\"output_tokens\":1}}}\n\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n\
event: content_block_stop\n\
data: {\"type\":\"content_block_stop\",\"index\":0}\n\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":1}}\n\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\n";

/// The run's token: a literal only the stand-in reads.
const RUN_TOKEN: &str = "run-token-for-the-real-binary-3e9a";

/// The prefix of a claude.ai credential: an access token's and a refresh token's both start so.
const CLAUDE_AI_PREFIX: &str = "sk-ant-o";

/// The saved claude.ai login's access token, in the throwaway config folder: a fixture shaped as
/// one, built here so no file holds a token-shaped literal.
fn saved_login() -> String {
    format!("{CLAUDE_AI_PREFIX}at01-fixture-saved-login-never-sent")
}

/// A claude.ai login saved in `config`, as Claude Code keeps one where no keychain holds it.
fn save_a_login(config: &Path) {
    std::fs::create_dir_all(config).expect("the config folder");
    let credentials = serde_json::json!({
        "claudeAiOauth": {
            "accessToken": saved_login(),
            "refreshToken": format!("{CLAUDE_AI_PREFIX}rt01-fixture-saved-refresh-never-sent"),
            "expiresAt": 4_102_444_800_000_u64,
            "scopes": ["user:inference", "user:profile"],
            "subscriptionType": "max"
        }
    });
    std::fs::write(config.join(".credentials.json"), credentials.to_string())
        .expect("the saved login");
    std::fs::write(
        config.join(".claude.json"),
        serde_json::json!({ "hasCompletedOnboarding": true }).to_string(),
    )
    .expect("the config file");
}

/// Runs `command` until it ends or [`PATIENCE`] passes, and ends it then.
fn run_bounded(mut command: Command) -> std::process::ExitStatus {
    command.stdin(Stdio::null());
    let mut child =
        purlis_core::forklock::spawn(&mut command).expect("the pinned Claude Code starts");
    let began = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("it can be waited on") {
            return status;
        }
        if began.elapsed() > PATIENCE {
            let _ = child.kill();
            return child.wait().expect("it ends");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The stand-in Messages server on a loopback port, answering every model call with [`TURN`],
/// and the runtime it runs on.
fn a_stand_in() -> (tokio::runtime::Runtime, MockServer) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let server = rt.block_on(MockServer::start());
    rt.block_on(
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(TURN),
            )
            .mount(&server),
    );
    (rt, server)
}

/// Asserts `claude` is the version purlis supports.
fn the_supported_version(claude: &Path) {
    let version = purlis_core::forklock::output(Command::new(claude).arg("--version"))
        .expect("it answers its version");
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        SUPPORTED.trim(),
        "{PINNED} is not the version purlis supports"
    );
}

/// **A key chat of the pinned Claude Code, as its start arms it** (fix round, finding 4): the
/// whole argv `Harness::ClaudeCode.state_hooks` builds for `route`, so its one `--settings` is
/// the adapter's merge, then `-p`. Its environment is composed as a chat's is
/// (`chatenv::compose`) from `launch`, the app's own, the operator passing each of `launch`'s
/// names, and started without what `hoststart::started_without` names for a key chat.
fn a_key_chat(claude: &Path, route: &KeyRoute, home: &Path, launch: &[(&str, &str)]) -> Command {
    let config = home.join(".claude");
    let plugin = home.join("purlis-plugin");
    std::fs::create_dir_all(&plugin).expect("a plugin folder");
    let StateHooks::ThisSessionOnly { args, env, .. } = Harness::ClaudeCode.state_hooks(
        Kit {
            // Its hooks and its MCP server run a program that answers nothing and ends.
            binary: Path::new("/usr/bin/true"),
            plugin: Some(&plugin),
            persona: None,
            skills: None,
            graded: false,
            judging: false,
            writes_its_piece: false,
            runs_a_shell: false,
            own: &[],
            laid_out: None,
            key_route: Some(route),
        },
        Some(home),
        &purlis_core::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("Claude Code is armed per session");
    };
    let path = std::env::var("PATH").unwrap_or_default();
    let home_text = home.display().to_string();
    let app: Vec<(std::ffi::OsString, std::ffi::OsString)> =
        [("PATH", path.as_str()), ("HOME", home_text.as_str())]
            .iter()
            .chain(launch)
            .map(|(name, value)| ((*name).into(), (*value).into()))
            .collect();
    let operator: Vec<String> = launch.iter().map(|(name, _)| (*name).to_owned()).collect();
    let strip = purlis_core::hoststart::started_without(home, Some(Harness::ClaudeCode), Some(route));
    let mut set = env;
    set.push(("CLAUDE_CONFIG_DIR".to_owned(), config.display().to_string()));
    let env = purlis_core::chatenv::compose(
        app,
        &purlis_core::chatenv::Starting {
            harness: Some(Harness::ClaudeCode),
            operator: &operator,
            strip: &strip,
            set: &set,
            git_hooks: None,
            declared: &[],
        },
    );
    let mut command = Command::new(claude);
    command
        .args(&args)
        .args(["-p", "say ok"])
        .env_clear()
        .envs(env)
        .current_dir(home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// The route through the stand-in `server`, with [`RUN_TOKEN`].
fn the_route(server: &MockServer) -> KeyRoute {
    KeyRoute::new(
        Listener {
            port: server.address().port(),
            token: RUN_TOKEN.to_owned(),
        },
        Some("claude-fixture-background"),
        |_| None,
    )
}

/// Asserts a model call reached the stand-in, and that every call carried the run's token and
/// no other credential.
fn every_call_carried_the_run_token(
    requests: &[wiremock::Request],
    status: std::process::ExitStatus,
) {
    assert!(
        requests
            .iter()
            .any(|r| r.method.as_str() == "POST" && r.url.path() == "/v1/messages"),
        "no model call reached the route ({status}): {requests:?}"
    );
    for request in requests {
        let said = format!("{} {}", request.method, request.url.path());
        let header = |name: &str| {
            request
                .headers
                .get_all(name)
                .iter()
                .map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
                .collect::<Vec<_>>()
        };
        assert!(header("x-api-key").is_empty(), "{said} carried x-api-key");
        if request.url.path().starts_with("/v1/") {
            assert_eq!(
                header("authorization"),
                [format!("Bearer {RUN_TOKEN}")],
                "{said} carried another credential"
            );
        }
        for (name, value) in &request.headers {
            let value = String::from_utf8_lossy(value.as_bytes());
            assert!(
                !value.contains(&saved_login()),
                "{said} carried the login in {name}"
            );
            assert!(
                !value.contains(CLAUDE_AI_PREFIX),
                "{said} carried a claude.ai credential in {name}"
            );
        }
    }
}

/// R-256: with a claude.ai login saved in its config folder, a key chat of the pinned Claude
/// Code carries the run's token on every call, and no other credential.
#[test]
fn a_key_chat_never_falls_back_to_a_saved_login() {
    let Some(claude) = pinned("a_key_chat_never_falls_back_to_a_saved_login") else {
        return;
    };
    the_supported_version(&claude);
    let (rt, server) = a_stand_in();
    let home = tempfile::tempdir().expect("a home");
    save_a_login(&home.path().join(".claude"));
    let route = the_route(&server);

    let status = run_bounded(a_key_chat(&claude, &route, home.path(), &[]));

    let requests = rt.block_on(server.received_requests()).unwrap_or_default();
    every_call_carried_the_run_token(&requests, status);
}

/// Fix round, finding 1: a socket path in the launch environment, which Claude Code reads from
/// there alone and would send every call and the run's token to, around the route: a key chat is
/// started without it, so every call still reaches the route and none reaches the socket.
#[cfg(unix)]
#[test]
fn a_key_chat_reaches_its_route_with_a_unix_socket_in_the_launch_environment() {
    let Some(claude) =
        pinned("a_key_chat_reaches_its_route_with_a_unix_socket_in_the_launch_environment")
    else {
        return;
    };
    the_supported_version(&claude);
    let (rt, server) = a_stand_in();
    let home = tempfile::tempdir().expect("a home");
    save_a_login(&home.path().join(".claude"));
    let route = the_route(&server);
    let socket = home.path().join("around.sock");
    let around = std::os::unix::net::UnixListener::bind(&socket).expect("a socket to go around by");
    around.set_nonblocking(true).expect("non-blocking");
    let socket_text = socket.display().to_string();

    let status = run_bounded(a_key_chat(
        &claude,
        &route,
        home.path(),
        &[("ANTHROPIC_UNIX_SOCKET", socket_text.as_str())],
    ));

    let requests = rt.block_on(server.received_requests()).unwrap_or_default();
    every_call_carried_the_run_token(&requests, status);
    match around.accept() {
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
        Ok(_) => panic!("a call went to the socket, around the route ({status})"),
        Err(error) => panic!("the socket could not be read: {error}"),
    }
}

/// R-256: the CI step runs this module by name on Linux and on macOS, with the pin installed,
/// through the guard that fails when it ran none, and with a skip turned into a failure.
#[test]
fn the_ci_step_ran_some_real_binary_tests() {
    let steps: Vec<&str> = CI
        .split("\n      - name: ")
        .filter(|step| step.contains("real_claude_code::"))
        .collect();
    for system in ["linux", "darwin"] {
        let step = steps
            .iter()
            .find(|step| step.contains(&format!("@anthropic-ai/claude-code-{system}-")))
            .unwrap_or_else(|| panic!("no CI step runs real_claude_code:: on {system}"));
        assert!(
            step.contains("tools/tests-ran.sh --"),
            "the {system} step doesn't fail when it ran none"
        );
        assert!(
            step.contains(&format!("{PINNED}=")),
            "the {system} step installs no pin"
        );
        assert!(
            step.contains(&format!("{REQUIRED}=1")),
            "the {system} step lets a real-binary test skip"
        );
        assert!(
            step.contains("tests/fixtures/harness-versions/claude.txt"),
            "the {system} step doesn't install the pinned version"
        );
    }
}
