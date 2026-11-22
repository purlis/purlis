// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis gitlab access` (#1349 part 2): the command line of GitLab access, from the one module
//! the window uses. A chat whose sandbox holds the person's own credentials back gets a GitLab
//! token only from a vault, so this keeps one in a persona's vault and declares `glab` for it, on
//! the host the project's clone has a remote on. Every token in here is fabricated.

#![cfg(unix)]

use crate::support;

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const TOKEN: &str = "fabricated-not-a-gitlab-token";
const HOST: &str = "git.acme.internal";

/// A project with a clone whose remote is on [`HOST`] (declared a GitLab), a persona `dev` and a
/// plain-file vault tagged for it. Returns the folder the plane is in.
fn plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("plane");
    for folder in ["personas/dev", "inventory", "workspaces/alpha/svc", "tmp"] {
        std::fs::create_dir_all(if folder == "tmp" {
            dir.path().join(folder)
        } else {
            root.join(folder)
        })
        .expect("a folder");
    }
    std::fs::write(
        root.join("personas/dev/persona.md"),
        "---\nname: dev\n---\n# Dev\n",
    )
    .unwrap();
    std::fs::write(
        root.join("charter.toml"),
        format!(
            "schema = 1\n[[forge]]\nkind = \"gitlab\"\nhost = \"{HOST}\"\n\n[sandbox]\n\n\
             [sandbox.personas.dev]\n"
        ),
    )
    .unwrap();
    let clone = root.join("workspaces/alpha/svc");
    for args in [
        vec!["init", "-q", "-b", "main", "."],
        vec![
            "remote",
            "add",
            "origin",
            "https://git.acme.internal/acme/svc.git",
        ],
    ] {
        let out = Command::new("git")
            .arg("-C")
            .arg(&clone)
            .args(&args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {args:?}");
    }
    std::fs::create_dir_all(root.join(".charter/vaults")).unwrap();
    std::fs::write(root.join(".charter/vaults/dev.json"), "{}").unwrap();
    std::fs::write(
        root.join(".charter/vaults.json"),
        r#"{"vaults": {"dev": {"provider": "plain-file", "persona": "dev",
            "config": {"file": ".charter/vaults/dev.json"}}}}"#,
    )
    .unwrap();
    dir
}

fn gitlab(tmp: &tempfile::TempDir, args: &[&str], stdin: Option<&str>) -> Output {
    let root = tmp.path().join("plane");
    let mut command = support::purlis();
    command
        .args(["gitlab", "access"])
        .args(args)
        .current_dir(&root)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", tmp.path())
        .env("TMPDIR", tmp.path().join("tmp"))
        .env("CHARTER_ROOT", &root)
        .env("CHARTER_SESSION_ID", "gitlab-access-test")
        .env("CHARTER_PLANE_FENCE", tmp.path())
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("the binary runs");
    if let Some(text) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }
    child.wait_with_output().expect("it ends")
}

fn said(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn it_names_each_gitlab_host_the_projects_clones_are_on_and_whether_chats_have_a_token() {
    let tmp = plane();
    let out = gitlab(&tmp, &[], None);
    assert!(out.status.success(), "{}", said(&out));
    let err = said(&out);
    assert!(
        err.contains(&format!("{HOST}: no token for chats")),
        "{err}"
    );
    assert!(
        err.contains(&format!("purlis gitlab access {HOST} --persona")),
        "{err}"
    );
}

#[test]
fn it_keeps_the_token_in_the_personas_vault_and_declares_glab_and_says_what_is_left_to_allow() {
    let tmp = plane();
    let root = tmp.path().join("plane");
    let out = gitlab(
        &tmp,
        &[HOST, "--persona", "dev", "--stdin"],
        Some(&format!("{TOKEN}\n")),
    );
    let err = said(&out);
    assert!(out.status.success(), "{err}");
    assert!(!err.contains(TOKEN), "{err}");
    assert!(!String::from_utf8_lossy(&out.stdout).contains(TOKEN));
    // The vault holds it, under the host's key; the project's file holds the line, not the token.
    assert!(read(&root.join(".charter/vaults/dev.json")).contains(TOKEN));
    let file = read(&root.join("charter.toml"));
    assert!(!file.contains(TOKEN), "{file}");
    assert!(file.contains("glab = {"), "{file}");
    assert!(file.contains("as = \"GITLAB_TOKEN env\""), "{file}");
    assert!(file.contains(&format!("host = \"{HOST}\"")), "{file}");
    // The Allow is the person's, in the window.
    assert!(err.contains("Allow glab for dev"), "{err}");
    let listed = gitlab(&tmp, &[], None);
    assert!(
        said(&listed).contains(&format!("{HOST}: set up for dev")),
        "{}",
        said(&listed)
    );
    // And it comes away again, token and line.
    let out = gitlab(&tmp, &[HOST, "--remove"], None);
    assert!(out.status.success(), "{}", said(&out));
    assert!(!read(&root.join(".charter/vaults/dev.json")).contains(TOKEN));
    assert!(!read(&root.join("charter.toml")).contains("glab"));
}

#[test]
fn it_sets_up_nothing_for_a_host_the_project_is_not_on_or_a_token_that_is_not_one() {
    let tmp = plane();
    let root = tmp.path().join("plane");
    let before = read(&root.join("charter.toml"));
    for (args, stdin, words) in [
        (
            &["gitlab.com", "--persona", "dev", "--stdin"][..],
            TOKEN,
            "isn't a GitLab host the project's clones are on",
        ),
        (
            &[HOST, "--persona", "dev", "--stdin"],
            "two words",
            "isn't a GitLab token",
        ),
        (&[HOST, "--stdin"], TOKEN, "Name the persona"),
    ] {
        let out = gitlab(&tmp, args, Some(stdin));
        let err = said(&out);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {err}");
        assert!(err.contains(words), "{args:?}: {err}");
        assert!(!err.contains(TOKEN), "{args:?}: {err}");
    }
    assert_eq!(read(&root.join("charter.toml")), before);
    assert_eq!(read(&root.join(".charter/vaults/dev.json")), "{}");
}

/// A sandboxed chat never gives itself a token: the command is the person's, refused before any
/// vault or project file is read, and the vault and the file are as they were.
#[test]
fn a_sandboxed_chat_is_refused_and_changes_nothing() {
    let tmp = plane();
    let root = tmp.path().join("plane");
    let before = read(&root.join("charter.toml"));
    let mut command = support::purlis();
    command
        .args(["gitlab", "access", HOST, "--persona", "dev", "--stdin"])
        .current_dir(&root)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", tmp.path())
        .env("CHARTER_ROOT", &root)
        .env("CHARTER_PLANE_FENCE", tmp.path())
        .env("PURLIS_SANDBOXED", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("the binary runs");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(TOKEN.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let err = said(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("sandboxed chat"), "{err}");
    assert!(err.contains("Settings › GitLab access"), "{err}");
    assert!(err.contains("own terminal"), "{err}");
    assert!(!err.contains(TOKEN), "{err}");
    assert_eq!(read(&root.join("charter.toml")), before);
    assert_eq!(read(&root.join(".charter/vaults/dev.json")), "{}");
}
