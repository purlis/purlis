// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis browser _login` (D-PE-browser-1), with Playwright's program stood in: the steps it
//! runs, the function that fills the login only on the declared origin, the environment the
//! program starts from, and what is left for the chat.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::{Asked, Steps, env_for, run, script, state_file};
use crate::repocmd::Say;

const CLI: &str = "/opt/homebrew/bin/playwright-cli";
const URL: &str = "https://shop.example.test/login";
const PASS: &str = "hunter2-p4ss";
const USER: &str = "qa-user@example.test";

/// One step as Playwright's program was asked to run it.
#[derive(Debug, Clone)]
struct Ran {
    argv: Vec<String>,
    env: Vec<(String, String)>,
    /// The function `run-code` was handed, and its file's mode, read as the step ran.
    code: Option<(String, u32)>,
}

/// Playwright's program, stood in: `run-code` writes the storage where the function would,
/// unless `fails`.
#[derive(Default)]
struct Fake {
    ran: Vec<Ran>,
    fails: bool,
}

impl Steps for Fake {
    fn run(
        &mut self,
        argv: &[String],
        _cwd: &Path,
        env: &[(String, String)],
    ) -> Result<(i32, String), String> {
        let code = argv
            .iter()
            .find_map(|word| word.strip_prefix("--filename="))
            .map(|file| {
                let text = std::fs::read_to_string(file).expect("the function");
                let mode = std::fs::metadata(file)
                    .expect("its file")
                    .permissions()
                    .mode()
                    & 0o777;
                if !self.fails {
                    let state = Path::new(file).with_file_name("state.json");
                    std::fs::write(state, r#"{"cookies":[{"name":"sid"}],"origins":[]}"#)
                        .expect("state");
                }
                (text, mode)
            });
        self.ran.push(Ran {
            argv: argv.to_vec(),
            env: env.to_vec(),
            code: code.clone(),
        });
        match (code, self.fails) {
            (Some(_), true) => Ok((1, "Error: purlis: the login page is not on https://shop.example.test, so nothing was filled".to_owned())),
            _ => Ok((0, String::new())),
        }
    }
}

/// A run's environment: the vault's two files, the run's temporary folder, and what a chat's
/// environment might also carry.
fn handed(dir: &Path) -> BTreeMap<String, String> {
    let pass = dir.join("pass");
    let user = dir.join("user");
    std::fs::write(&pass, PASS).expect("pass");
    std::fs::write(&user, USER).expect("user");
    let tmp = dir.join("tmp");
    std::fs::create_dir_all(&tmp).expect("tmp");
    [
        ("BROWSER_LOGIN_PASS", pass.display().to_string()),
        ("BROWSER_LOGIN_USER", user.display().to_string()),
        ("TMPDIR", tmp.display().to_string()),
        ("PATH", "/usr/bin:/bin".to_owned()),
        ("HOME", "/Users/person".to_owned()),
        ("HTTPS_PROXY", "http://127.0.0.1:50123".to_owned()),
        ("NODE_OPTIONS", "--require /chat/evil.js".to_owned()),
        ("PLAYWRIGHT_MCP_CONFIG", "/chat/config.json".to_owned()),
        ("PLAYWRIGHT_BROWSERS_PATH", "/chat/browsers".to_owned()),
        (
            "PLAYWRIGHT_MCP_SECRETS_FILE",
            "/chat/secrets.env".to_owned(),
        ),
        ("npm_config_registry", "https://evil.example/".to_owned()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

fn login(
    cwd: &Path,
    env: &BTreeMap<String, String>,
    url: &str,
    cli: &str,
    fake: &mut Fake,
) -> (u8, Vec<String>) {
    let mut said = Vec::new();
    let code = run(
        &Asked {
            cli: Path::new(cli),
            url,
            session: "owner",
            cwd,
        },
        &|name| env.get(name).cloned(),
        fake,
        &mut |line: Say| said.push(format!("{line:?}")),
    );
    (code, said)
}

#[test]
fn a_login_fills_only_on_its_declared_origin_in_a_browser_of_its_own() {
    let dir = tempfile::tempdir().expect("dir");
    let cwd = dir.path().join("work");
    std::fs::create_dir_all(&cwd).expect("work");
    let env = handed(dir.path());
    let mut fake = Fake::default();
    let (code, said) = login(&cwd, &env, URL, CLI, &mut fake);
    assert_eq!(code, 0, "{said:?}");

    // Open, the one function, close: each in a session of purlis's own, never the chat's.
    let steps: Vec<Vec<String>> = fake.ran.iter().map(|ran| ran.argv.clone()).collect();
    assert_eq!(steps.len(), 3, "{steps:?}");
    let session = steps[0][1].clone();
    assert!(session.starts_with("-s=purlis-login-"), "{session}");
    assert_eq!(steps[0], [CLI, session.as_str(), "open"]);
    assert_eq!(steps[1][..3], [CLI, session.as_str(), "run-code"]);
    assert!(steps[1][3].starts_with("--filename="), "{:?}", steps[1]);
    assert_eq!(steps[2], [CLI, session.as_str(), "close"]);

    // The function: the declared page, bound to its origin, and private while it lives.
    let (function, mode) = fake.ran[1].code.clone().expect("the function");
    assert_eq!(mode, 0o600);
    assert!(
        function.contains("const ORIGIN = \"https://shop.example.test\";"),
        "{function}"
    );
    assert!(
        function.contains(&format!("const LOGIN = \"{URL}\";")),
        "{function}"
    );
    assert!(
        function.contains("new URL(page.url()).origin === ORIGIN"),
        "{function}"
    );
    assert!(
        function.contains("await context.route(away, refuse);"),
        "{function}"
    );
    assert!(
        function.contains("const away = url => url.origin !== ORIGIN;"),
        "{function}"
    );
    // Its file is gone once it ran.
    let file = steps[1][3].trim_start_matches("--filename=");
    assert!(!Path::new(file).exists(), "{file}");

    // The login itself is in no step's words or environment.
    for ran in &fake.ran {
        let all = format!("{:?} {:?}", ran.argv, ran.env);
        assert!(!all.contains(PASS) && !all.contains(USER), "{all}");
    }

    // What is left for the chat: the session's storage, private, where it runs.
    let state = state_file(&cwd, "owner");
    assert_eq!(state, cwd.join(".playwright-cli/owner.login.json"));
    let mode = std::fs::metadata(&state)
        .expect("kept")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
    let text = std::fs::read_to_string(&state).expect("kept");
    assert!(text.contains("cookies") && !text.contains(PASS), "{text}");
    let told = said.join("\n");
    assert!(
        told.contains("state-load .playwright-cli/owner.login.json"),
        "{told}"
    );
    assert!(!told.contains(PASS), "{told}");
}

#[test]
fn playwright_s_program_starts_from_named_variables_alone() {
    let dir = tempfile::tempdir().expect("dir");
    let env = handed(dir.path());
    let started = env_for(&|name| env.get(name).cloned());
    let names: Vec<&str> = started.iter().map(|(k, _)| k.as_str()).collect();
    for chats in [
        "NODE_OPTIONS",
        "PLAYWRIGHT_MCP_CONFIG",
        "PLAYWRIGHT_BROWSERS_PATH",
        "PLAYWRIGHT_MCP_SECRETS_FILE",
        "npm_config_registry",
        "BROWSER_LOGIN_PASS",
        "BROWSER_LOGIN_USER",
    ] {
        assert!(!names.contains(&chats), "{chats}: {names:?}");
    }
    assert!(started.contains(&("HOME".to_owned(), "/Users/person".to_owned())));
    // The browser goes out through the run's own proxy, which carries the chat's hosts.
    assert!(started.contains(&(
        "PLAYWRIGHT_MCP_PROXY_SERVER".to_owned(),
        "http://127.0.0.1:50123".to_owned()
    )));
}

#[test]
fn a_login_that_does_not_go_through_leaves_nothing_and_closes_its_browser() {
    let dir = tempfile::tempdir().expect("dir");
    let cwd = dir.path().join("work");
    std::fs::create_dir_all(&cwd).expect("work");
    let env = handed(dir.path());
    let mut fake = Fake {
        fails: true,
        ..Fake::default()
    };
    let (code, said) = login(&cwd, &env, URL, CLI, &mut fake);
    assert_eq!(code, 1);
    assert_eq!(fake.ran.last().expect("a step").argv[2], "close");
    assert!(!state_file(&cwd, "owner").exists());
    let told = said.join("\n");
    assert!(told.contains("did not go through"), "{told}");
    assert!(
        told.contains("is not on https://shop.example.test"),
        "{told}"
    );
}

#[test]
fn a_login_with_no_password_a_page_it_cannot_bind_or_a_program_by_name_runs_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let cwd = dir.path().to_path_buf();
    let mut env = handed(dir.path());
    for (url, cli) in [
        ("http://shop.example.test/login", CLI),
        ("https://user:pw@shop.example.test/", CLI),
        (URL, "playwright-cli"),
    ] {
        let mut fake = Fake::default();
        let (code, said) = login(&cwd, &env, url, cli, &mut fake);
        assert_eq!(code, 2, "{url} {cli}: {said:?}");
        assert!(fake.ran.is_empty(), "{url} {cli}");
    }
    env.remove("BROWSER_LOGIN_PASS");
    let mut fake = Fake::default();
    let (code, said) = login(&cwd, &env, URL, CLI, &mut fake);
    assert_eq!(code, 2, "{said:?}");
    assert!(fake.ran.is_empty());
}

#[test]
fn a_link_planted_where_the_session_is_kept_is_replaced_never_followed() {
    let dir = tempfile::tempdir().expect("dir");
    let cwd = dir.path().join("work");
    std::fs::create_dir_all(cwd.join(".playwright-cli")).expect("work");
    let elsewhere: PathBuf = dir.path().join("elsewhere.json");
    std::fs::write(&elsewhere, "untouched").expect("elsewhere");
    std::os::unix::fs::symlink(&elsewhere, state_file(&cwd, "owner")).expect("link");
    let env = handed(dir.path());
    let (code, said) = login(&cwd, &env, URL, CLI, &mut Fake::default());
    assert_eq!(code, 0, "{said:?}");
    assert_eq!(
        std::fs::read_to_string(&elsewhere).expect("read"),
        "untouched"
    );
    assert!(
        !std::fs::symlink_metadata(state_file(&cwd, "owner"))
            .expect("kept")
            .file_type()
            .is_symlink()
    );
}

#[test]
fn the_function_holds_the_login_as_literals_and_a_page_without_a_user_name_skips_it() {
    let text = script(
        "https://shop.example.test",
        URL,
        None,
        "a\"b\\c</script>",
        Path::new("/run/state.json"),
    );
    assert!(text.contains("const USER = null;"), "{text}");
    assert!(
        text.contains(r#"const PASS = "a\"b\\c</script>";"#),
        "{text}"
    );
    assert!(
        text.contains(r#"const STATE = "/run/state.json";"#),
        "{text}"
    );
}
