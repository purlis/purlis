// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill install | update | rm` at the person's terminal on an org's project, against
//! `purlis serve` (SK-11a, #6030): each change is audited in the host's event log, as the person's
//! at the terminal, before it lands; and a command run inside a chat audits and changes nothing.

#![cfg(unix)]

use crate::support;

use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

struct Ends(Child);

impl Drop for Ends {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// An org's project, a git source its table allows, and `purlis serve` once it says it is ready.
struct Place {
    _dir: tempfile::TempDir,
    at: PathBuf,
    _host: Ends,
}

/// git for the fixture source alone, reading no config of the machine's.
fn git(at: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "safe.bareRepository=all", "-c", "user.name=Fixture"])
        .args([
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(at)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn homes(command: &mut Command, at: &Path) {
    command
        .env("HOME", at.join("home"))
        .env("PURLIS_CONFIG_HOME", at.join("config"))
        .env("PURLIS_DATA_HOME", at.join("data"))
        .env("XDG_DATA_HOME", at.join("data"))
        .env("XDG_RUNTIME_DIR", at.join("run"))
        .env("TMPDIR", at);
}

impl Place {
    fn new() -> Place {
        let dir = tempfile::tempdir().unwrap();
        let at = dir.path().canonicalize().unwrap();
        for folder in ["run", "config", "data", "home", "plane"] {
            std::fs::create_dir_all(at.join(folder)).unwrap();
        }
        std::fs::set_permissions(at.join("run"), std::fs::Permissions::from_mode(0o700)).unwrap();
        git(&at, &["init", "--bare", "-q", "-b", "main", "source.git"]);
        let url = at.join("source.git").display().to_string();
        std::fs::write(
            at.join("plane/charter.toml"),
            format!(
                "schema = 1\n\n[skill_policy]\nallow = [{{ source = \"{url}\" }}]\nmin_age_days = 0\n"
            ),
        )
        .unwrap();
        let mut serve = support::purlis();
        serve
            .arg("serve")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        homes(&mut serve, &at);
        let mut host = Ends(serve.spawn().expect("purlis serve starts"));
        let mut line = String::new();
        BufReader::new(host.0.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        if line.trim_end() != "ready" {
            let _ = host.0.kill();
            let mut said = String::new();
            let _ = host.0.stderr.take().unwrap().read_to_string(&mut said);
            panic!("the host did not say it was ready: {line:?}\n{said}");
        }
        let place = Place {
            _dir: dir,
            at,
            _host: host,
        };
        place.publish("Tells the team.");
        place
    }

    fn root(&self) -> PathBuf {
        self.at.join("plane")
    }

    fn url(&self) -> String {
        self.at.join("source.git").display().to_string()
    }

    /// Commits `skills/notify` to the source's `main`, described as `description`.
    fn publish(&self, description: &str) {
        let work = self.at.join("work");
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(work.join("skills/notify")).unwrap();
        std::fs::write(
            work.join("skills/notify/SKILL.md"),
            format!("---\nname: notify\ndescription: {description}\n---\n\nPost it.\n"),
        )
        .unwrap();
        let dirs = [
            format!("--git-dir={}", self.at.join("source.git").display()),
            format!("--work-tree={}", work.display()),
        ];
        let with = |more: &[&str]| {
            let mut args: Vec<&str> = dirs.iter().map(String::as_str).collect();
            args.extend_from_slice(more);
            git(&self.at, &args);
        };
        with(&["add", "-A"]);
        with(&["commit", "-q", "-m", "skills"]);
    }

    /// `purlis skill` with `args` at the person's own terminal, with `env` on top.
    fn run(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let root = self.root();
        let mut command = support::purlis();
        command
            .arg("skill")
            .args(args)
            .current_dir(&root)
            .env("CHARTER_ROOT", &root)
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        homes(&mut command, &self.at);
        support::at_a_terminal(&mut command);
        for (key, value) in env {
            command.env(key, value);
        }
        command.output().unwrap()
    }

    /// Each skill change the host's event log audits: its kind and body.
    fn audited(&self) -> Vec<(String, serde_json::Value)> {
        let Ok(devices) = std::fs::read_dir(self.at.join("data/events")) else {
            return Vec::new();
        };
        devices
            .flatten()
            .flat_map(|device| purlis_core::eventlog::read(&device.path()).unwrap_or_default())
            .filter(|event| event.kind.starts_with("skill."))
            .map(|event| (event.kind, event.body))
            .collect()
    }
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// The commit `main` of the source is at.
fn head_of(place: &Place) -> String {
    let out = Command::new("git")
        .args(["-c", "safe.bareRepository=all", "rev-parse", "main"])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(place.at.join("source.git"))
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn an_install_an_update_and_a_removal_at_the_terminal_are_each_audited_by_the_host_and_land() {
    let place = Place::new();
    let root = place.root();
    let url = place.url();
    let first = head_of(&place);

    let installed = place.run(
        &["install", &url, "notify", "--folder", "skills", "--yes"],
        &[],
    );
    assert!(installed.status.success(), "{}", text(&installed));
    assert!(root.join("skills/notify/SKILL.md").is_file());

    place.publish("Tells the whole team.");
    let second = head_of(&place);
    let updated = place.run(&["update", "notify", "--yes"], &[]);
    assert!(updated.status.success(), "{}", text(&updated));

    let removed = place.run(&["rm", "notify"], &[]);
    assert!(removed.status.success(), "{}", text(&removed));
    assert!(!root.join("skills/notify").exists());

    // A folder of this machine is named by its last name alone, never its path.
    let audited: Vec<(String, serde_json::Value)> = place
        .audited()
        .into_iter()
        .map(|(kind, mut body)| {
            let source = body["source"].as_str().unwrap_or_default().to_owned();
            assert!(
                source.starts_with("local:") && !source.contains('/'),
                "{source}"
            );
            body["source"] = "local".into();
            (kind, body)
        })
        .collect();
    let source = "local";
    let said = |pin: &str| {
        serde_json::json!({
            "actor_kind": "human",
            "actor": "operator",
            "scope": "terminal",
            "level": "project",
            "skills": ["notify"],
            "source": source,
            "pin": pin,
        })
    };
    assert_eq!(
        audited,
        [
            ("skill.install".to_owned(), said(&first)),
            ("skill.update".to_owned(), said(&second)),
            ("skill.remove".to_owned(), said(&second)),
        ]
    );
}

#[test]
fn a_command_run_inside_a_chat_audits_nothing_and_installs_nothing() {
    let place = Place::new();
    let url = place.url();

    for mark in [
        purlis_core::hookwire::CHAT_ENV,
        purlis_core::hookwire::SANDBOXED_ENV,
    ] {
        let refused = place.run(
            &["install", &url, "notify", "--folder", "skills", "--yes"],
            &[(mark, "1")],
        );

        // Refused before any change is built: a chat leaves drafts, and never reaches the audit.
        // The audit's own refusal inside a chat is `skillaudit`'s unit test; that a chat can
        // never be admitted as `terminal` is the session protocol's
        // (`a_client_started_inside_a_chat_is_refused_every_human_scope`).
        assert!(!refused.status.success(), "{mark}: {}", text(&refused));
        assert!(
            text(&refused).contains("A chat does not change skills itself"),
            "{mark}: {}",
            text(&refused)
        );
        assert!(!place.root().join("skills/notify").exists(), "{mark}");
    }
    assert!(place.audited().is_empty(), "{:?}", place.audited());
}
