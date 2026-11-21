// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's real `git`, composed as the forge wiring composes it, answers the sentinel for the
//! chat's recorded hosts and for no other, and no login of the person's is reachable**
//! (ADR 0128 D3c, slice B).
//!
//! The person is made for the test: a home whose git config names a credential helper, a system
//! git config that names another (Apple's `osxkeychain` is one), a clone whose own config names a
//! third, and a `gh` login file. Each helper logs that it was started, and answers as the person.
//! The chat's `git` is the real one, started with the environment the wiring composes, and the
//! helper it runs is the real `purlis` binary (`purlis-git-sentinel`), so what is checked is what
//! git does with the pairs and what the helper answers, not a stand-in for either.

use std::path::{Path, PathBuf};
use std::process::Output;

use purlis_core::chatenv::{self, Starting};
use purlis_core::forgelock::{self, PERSON};
use purlis_core::forgewire::{self, ForgeKind, Repo, Route, SecretString, Wiring};
use purlis_core::harness::Harness;

use crate::support;

const SENTINEL: &str = "SENTINEL-9d8c7b6a5f4e3d2c1b0a9f8e7d6c5b4a";

struct Person {
    dir: tempfile::TempDir,
}

impl Person {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a folder");
        let root = dir.path();
        let home = root.join("home");
        std::fs::create_dir_all(home.join(".config/gh")).unwrap();
        std::fs::write(
            home.join(".config/gh/hosts.yml"),
            format!("github.com:\n    user: {PERSON}\n    oauth_token: {PERSON}-gh\n"),
        )
        .unwrap();
        let helper = root.join("person-helper.sh");
        // Logs its start, reads git's input and answers as the person.
        std::fs::write(
            &helper,
            format!(
                "#!/bin/sh\necho \"$1\" >> '{log}'\ncat >> '{log}'\n\
                 if [ \"$1\" = get ]; then echo username={PERSON}; echo password={PERSON}-pw; fi\n",
                log = root.join("person-helper.log").display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let helper_line = format!("!'{}'", helper.display());
        std::fs::write(
            home.join(".gitconfig"),
            format!("[user]\n\tname = {PERSON}\n[credential]\n\thelper = {helper_line}\n"),
        )
        .unwrap();
        std::fs::write(
            root.join("system.gitconfig"),
            format!("[credential]\n\thelper = {helper_line}\n"),
        )
        .unwrap();
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            let out = support::command("git")
                .args(args)
                .current_dir(&repo)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .output()
                .expect("git");
            assert!(out.status.success(), "{args:?}");
        };
        git(&["init", "-q"]);
        git(&["config", "credential.helper", &helper_line]);
        // A helper scoped to the very host the chat will be sent to.
        git(&[
            "config",
            "credential.https://github.com.helper",
            &helper_line,
        ]);
        Self { dir }
    }

    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    fn system(&self) -> PathBuf {
        self.dir.path().join("system.gitconfig")
    }

    fn repo(&self) -> PathBuf {
        self.dir.path().join("repo")
    }

    /// What the person's helpers logged: empty where none was ever started.
    fn logged(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("person-helper.log")).unwrap_or_default()
    }
}

fn wiring(scratch: &Path) -> Wiring {
    // The chat's own copy of the authority is made from it, so the file is there.
    std::fs::write(scratch.join("ca.pem"), "-----BEGIN CERTIFICATE-----\n").expect("a CA");
    forgewire::compose(
        &Route {
            egress_url: "http://127.0.0.1:44123".to_owned(),
            gh_socket: scratch.join("s/gh.sock"),
            ca_cert: scratch.join("ca.pem"),
            sentinel: SecretString::from(SENTINEL),
            helper_exe: PathBuf::from(support::PURLIS),
            repos: vec![
                Repo::new("github.com", "acme", "app", ForgeKind::GitHub).unwrap(),
                Repo::new("git.example.org", "grp", "lib", ForgeKind::GitLab).unwrap(),
            ],
        },
        scratch,
    )
    .expect("the wiring")
}

/// The chat's environment as a start composes it, for `harness`: the lock, then the wiring.
fn locked_chat(person: &Person, harness: Harness, scratch: &Path) -> Vec<(String, String)> {
    let line = forgelock::apply_to_line(
        Vec::new(),
        Some(scratch),
        Some(&person.home()),
        harness == Harness::ClaudeCode,
    )
    .expect("the lock");
    let line = forgewire::apply(line, &wiring(scratch));
    let hooks = purlis_core::githooks::GitHooks::at(scratch.join("hooks"));
    let starting = Starting {
        harness: Some(harness),
        operator: &[],
        strip: &[],
        set: &line,
        git_hooks: Some(&hooks),
        declared: &[],
    };
    let app = vec![
        ("HOME".into(), person.home().into()),
        ("PATH".into(), "/usr/bin:/bin".into()),
    ];
    chatenv::compose(app, &starting)
        .into_iter()
        .map(|(n, v)| {
            (
                n.to_string_lossy().into_owned(),
                v.to_string_lossy().into_owned(),
            )
        })
        .collect()
}

/// The wiring alone, over a chat whose git still reads the person's global and system config:
/// the reset must do what the lock's own files do not have to.
fn wired_only(person: &Person, scratch: &Path) -> Vec<(String, String)> {
    let env = vec![
        ("HOME".to_owned(), person.home().display().to_string()),
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        (
            "GIT_CONFIG_GLOBAL".to_owned(),
            person.home().join(".gitconfig").display().to_string(),
        ),
        (
            "GIT_CONFIG_SYSTEM".to_owned(),
            person.system().display().to_string(),
        ),
        ("GIT_TERMINAL_PROMPT".to_owned(), "0".to_owned()),
    ];
    forgewire::apply(env, &wiring(scratch))
}

fn git(env: &[(String, String)], cwd: &Path, args: &[&str], input: &str) -> Output {
    use std::io::Write;
    let mut run = support::command("git");
    run.args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(env.iter().map(|(n, v)| (n, v)))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = run.spawn().expect("git");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().expect("git ended")
}

fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn fill(env: &[(String, String)], cwd: &Path, protocol: &str, host: &str) -> Output {
    git(
        env,
        cwd,
        &["credential", "fill"],
        &format!("protocol={protocol}\nhost={host}\n\n"),
    )
}

/// Every composition the chat is started with: each harness under the lock, and the wiring alone.
fn compositions(person: &Person, scratch: &Path) -> Vec<(String, Vec<(String, String)>)> {
    let mut all: Vec<_> = Harness::ALL
        .into_iter()
        .map(|h| {
            (
                format!("{h:?} under the lock"),
                locked_chat(person, h, scratch),
            )
        })
        .collect();
    all.push(("the wiring alone".to_owned(), wired_only(person, scratch)));
    all
}

#[test]
fn a_recorded_host_is_answered_with_the_sentinel_and_never_with_a_login_of_the_persons() {
    let person = Person::new();
    let scratch = tempfile::tempdir().unwrap();
    for (name, env) in compositions(&person, scratch.path()) {
        for host in ["github.com", "git.example.org"] {
            let out = fill(&env, &person.repo(), "https", host);
            let text = said(&out);
            assert!(out.status.success(), "{name} {host}: {text}");
            assert!(
                text.contains(&format!("username=x-access-token\npassword={SENTINEL}\n")),
                "{name} {host}: {text}"
            );
            assert!(!text.contains(PERSON), "{name} {host}: {text}");
        }
    }
    assert_eq!(person.logged(), "", "a helper of the person's was started");
}

#[test]
fn the_sentinel_is_answered_for_no_host_the_chat_has_no_repo_on() {
    let person = Person::new();
    let scratch = tempfile::tempdir().unwrap();
    for (name, env) in compositions(&person, scratch.path()) {
        for (protocol, host) in [
            ("https", "evil.example.com"),
            ("https", "github.com.evil.example"),
            ("https", "evilgithub.com"),
            ("https", "gitlab.com"),
            ("https", "api.github.com"),
            ("https", "codeload.github.com"),
            ("https", "objects.githubusercontent.com"),
            ("http", "github.com"),
            ("ssh", "github.com"),
        ] {
            let out = fill(&env, &person.repo(), protocol, host);
            let text = said(&out);
            // Git has no credential to give and may not prompt: it fails, saying nothing of ours.
            assert!(!out.status.success(), "{name} {protocol}://{host}: {text}");
            assert!(
                !text.contains(SENTINEL),
                "{name} {protocol}://{host}: {text}"
            );
            assert!(!text.contains(PERSON), "{name} {protocol}://{host}: {text}");
        }
    }
    assert_eq!(person.logged(), "", "a helper of the person's was started");
}

#[test]
fn what_git_stores_or_erases_never_reaches_a_helper_of_the_persons_or_a_file() {
    let person = Person::new();
    let scratch = tempfile::tempdir().unwrap();
    for (name, env) in compositions(&person, scratch.path()) {
        let input = format!(
            "protocol=https\nhost=github.com\nusername=x-access-token\npassword={SENTINEL}\n\n"
        );
        for verb in ["approve", "reject"] {
            let out = git(&env, &person.repo(), &["credential", verb], &input);
            assert!(out.status.success(), "{name} {verb}: {}", said(&out));
        }
    }
    // The helper of the person's, the system's and the clone's own: none was started, so none
    // could keep the sentinel (the keychain's `store` is what the reset keeps it from).
    assert_eq!(
        person.logged(),
        "",
        "a helper of the person's was asked to store or erase"
    );
    for entry in walk(person.dir.path()) {
        let Ok(text) = std::fs::read_to_string(&entry) else {
            continue;
        };
        assert!(
            !text.contains(SENTINEL),
            "{} holds the sentinel",
            entry.display()
        );
    }
}

#[test]
fn the_chats_ca_is_named_for_each_recorded_host_alone_and_never_for_another() {
    let person = Person::new();
    let scratch = tempfile::tempdir().unwrap();
    for (name, env) in compositions(&person, scratch.path()) {
        // The chat's git trusts its own copy of the authority, made in its wire folder.
        let ca = env
            .iter()
            .find(|(k, _)| k == "GLAB_CA_CERT")
            .map(|(_, v)| v.clone())
            .expect("a CA copy");
        let matched = |url: &str| {
            let out = git(
                &env,
                &person.repo(),
                &["config", "--get-urlmatch", "http.sslCAInfo", url],
                "",
            );
            (
                out.status.success(),
                String::from_utf8_lossy(&out.stdout).trim().to_owned(),
            )
        };
        for url in [
            "https://github.com/acme/app.git",
            "https://git.example.org/grp/lib.git",
        ] {
            assert_eq!(matched(url), (true, ca.clone()), "{name} {url}");
        }
        // Another host trusts what git trusts, never the chat's CA; and a download-only host is
        // tunnelled with its own certificate.
        for url in [
            "https://evil.example.com/",
            "https://gitlab.com/o/r.git",
            "https://codeload.github.com/o/r/tar.gz/main",
            "https://release-assets.githubusercontent.com/x",
            "https://github.com.evil.example/",
        ] {
            assert_eq!(matched(url), (false, String::new()), "{name} {url}");
        }
        let all = git(
            &env,
            &person.repo(),
            &["config", "--get", "http.sslCAInfo"],
            "",
        );
        assert!(
            !all.status.success(),
            "{name}: a chat-wide sslCAInfo is set"
        );
    }
}

#[test]
fn the_chats_gh_folder_holds_no_login_and_reads_the_sockets_setting_if_gh_is_installed() {
    let Ok(version) = support::command("gh").arg("--version").output() else {
        eprintln!("gh is not installed here: its config folder was not read with gh");
        return;
    };
    if !version.status.success() {
        return;
    }
    let person = Person::new();
    let scratch = tempfile::tempdir().unwrap();
    let env = locked_chat(&person, Harness::ClaudeCode, scratch.path());
    let gh = |args: &[&str], keep_tokens: bool| {
        let mut run = support::command("gh");
        run.args(args)
            .env_clear()
            .envs(
                env.iter()
                    .filter(|(n, _)| {
                        keep_tokens || !matches!(n.as_str(), "GH_TOKEN" | "GH_ENTERPRISE_TOKEN")
                    })
                    .map(|(n, v)| (n, v)),
            )
            .env("PATH", "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin")
            // `gh auth token` asks the system keyring; with no bus to reach it starts none, where
            // it would leave a session `dbus-daemon` running after the test (the no-orphans check).
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                "unix:path=/nonexistent/no-session-bus",
            );
        run.output().expect("gh")
    };
    let socket = gh(&["config", "get", "http_unix_socket"], true);
    assert_eq!(
        String::from_utf8_lossy(&socket.stdout).trim(),
        scratch.path().join("s/gh.sock").display().to_string(),
        "{}",
        said(&socket)
    );
    // With the sentinel's own variables taken away there is nothing to sign in with: the
    // person's `hosts.yml` in their home is not read.
    let token = gh(&["auth", "token", "--hostname", "github.com"], false);
    assert!(!token.status.success(), "{}", said(&token));
    assert!(!said(&token).contains(PERSON), "{}", said(&token));
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}
