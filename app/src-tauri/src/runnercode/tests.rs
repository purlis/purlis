// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project held on a pool's runner, for real git (#2046): the window's link to its own host,
//! which relays it over the link between hosts to a runner that keeps its projects in a folder of
//! its own and moves code with the core, as `purlis serve` does.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use purlis_host::Hosting;
use purlis_host::standin::StandIns;

use crate::host::{Named, SessionHost};
use crate::hostedsessions::HostedSessions;

/// git for the test's own fixtures: no template, no signer, a test's own identity.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = purlis_core::forklock::output(
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "init.templateDir=",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "core.excludesFile=/dev/null",
            ])
            .args(args),
    )
    .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn committed(dir: &Path, file: &str, text: &str) -> String {
    if let Some(parent) = dir.join(file).parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", file]);
    git(dir, &["commit", "-q", "-m", file]);
    git(dir, &["rev-parse", "HEAD"])
}

struct World {
    _dir: tempfile::TempDir,
    root: PathBuf,
    projects: PathBuf,
    sessions: HostedSessions,
    _runtime: tokio::runtime::Runtime,
}

impl World {
    /// A project that is a git repository, with workspace `app` and its clone `web` (which the
    /// project's own repository ignores), held through this machine's host on a pool's runner.
    fn new() -> World {
        let dir = tempfile::tempdir().unwrap();
        let top = std::fs::canonicalize(dir.path()).unwrap();
        let root = top.join("project");
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "-b", "main"]);
        // As a LIVE workspace's lines keep it: its file and its memory go with the project, its
        // clones do not.
        std::fs::write(
            root.join(".gitignore"),
            "/.purlis/\n/workspaces/*/*\n!/workspaces/*/workspace.md\n!/workspaces/*/memory\n",
        )
        .unwrap();
        committed(&root, purlis_core::plane::MANIFEST, "");
        committed(&root, "workspaces/app/workspace.md", "# app\n");
        let web = root.join("workspaces/app/web");
        std::fs::create_dir_all(&web).unwrap();
        git(&web, &["init", "-q", "-b", "main"]);
        committed(&web, "README", "web\n");
        git(&web, &["checkout", "-q", "-b", "fix-login"]);

        let projects = top.join("runner/projects");
        std::fs::create_dir_all(&projects).unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let runner = Hosting::running_with(
            crate::sessionhost::Composes::as_the_tests_ship(),
            StandIns::default(),
        )
        .keeping_projects_in(projects.clone())
        .moving_code_with(purlis_core::codelink::Moves);
        let relay = runtime.block_on(async move {
            purlis_host::Relay::new(Arc::new(move || {
                let runner = runner.clone();
                Box::pin(async move { Ok(crate::behaviour::linked_to(runner).await) })
            }))
        });
        let (connection, _link) = crate::hostedsessions::tests::in_memory(relay, &runtime);
        let sessions = HostedSessions::on_a_pools_runner(connection, &root);
        World {
            _dir: dir,
            root,
            projects,
            sessions,
            _runtime: runtime,
        }
    }

    /// Where the runner keeps this project.
    fn kept(&self) -> PathBuf {
        let id = purlis_core::scaffold::planefile::id_to_hold_by(&self.root).unwrap();
        self.projects.join(id)
    }
}

#[test]
fn a_project_held_on_a_pools_runner_is_checked_out_where_it_keeps_it_with_its_uncommitted_work() {
    let world = World::new();
    // A memory written here and not saved yet, and what is this machine's alone.
    std::fs::create_dir_all(world.root.join("workspaces/app/memory")).unwrap();
    std::fs::write(
        world.root.join("workspaces/app/memory/runners.md"),
        "a memory not yet saved\n",
    )
    .unwrap();
    std::fs::create_dir_all(world.root.join(".purlis/vaults")).unwrap();
    std::fs::write(world.root.join(".purlis/vaults/forge.json"), "{}").unwrap();
    std::fs::write(world.root.join("purlis.local.toml"), "[sandbox]\n").unwrap();

    world.sessions.hold().expect("held on the runner");

    let kept = world.kept();
    assert!(
        kept.join(purlis_core::plane::MANIFEST).is_file(),
        "checked out there"
    );
    assert_eq!(
        std::fs::read_to_string(kept.join("workspaces/app/memory/runners.md")).unwrap(),
        "a memory not yet saved\n"
    );
    assert!(
        !kept.join(".purlis").exists(),
        "no state or vault of this machine's"
    );
    assert!(!kept.join("purlis.local.toml").exists());
    // The project's own branch, untouched here.
    assert_eq!(
        git(&world.root, &["symbolic-ref", "--short", "HEAD"]),
        "main"
    );
}

#[test]
fn a_workspaces_clones_reach_the_runner_before_a_chat_starts_there_and_its_commits_come_back() {
    let world = World::new();
    let web = world.root.join("workspaces/app/web");
    std::fs::write(web.join("login.rs"), "fn log_in() {}\n").unwrap();
    let here = git(&web, &["rev-parse", "fix-login"]);

    // The start itself is the runner's to compose; the code reaches it first, whatever it says.
    let _ = world.sessions.open_named(
        None,
        &Named {
            workspace: Some("app".into()),
            ..crate::hostedsessions::tests::by_its_names()
        },
        Default::default(),
        &|_| {},
    );

    let there = world.kept().join("workspaces/app/web");
    assert_eq!(git(&there, &["rev-parse", "refs/heads/fix-login"]), here);
    assert_eq!(
        std::fs::read_to_string(there.join("login.rs")).unwrap(),
        "fn log_in() {}\n",
        "the uncommitted work reached the runner"
    );

    // A runner chat commits there; the window fetches it back under the pool alone.
    let on_the_runner = committed(&there, "login.rs", "fn log_in() { done() }\n");
    let failed = world.sessions.fetch_back("app", "builds");
    assert_eq!(failed, Vec::<String>::new());
    assert_eq!(
        git(&web, &["rev-parse", "refs/remotes/builds/fix-login"]),
        on_the_runner
    );
    assert_eq!(
        git(&web, &["rev-parse", "fix-login"]),
        here,
        "the branch here is the same"
    );
    assert_eq!(
        std::fs::read_to_string(web.join("login.rs")).unwrap(),
        "fn log_in() {}\n",
        "the work tree here is untouched"
    );
}

#[test]
fn a_runner_that_lost_a_repository_since_the_last_send_is_sent_it_whole() {
    let world = World::new();
    let web = world.root.join("workspaces/app/web");
    let start = || {
        let _ = world.sessions.open_named(
            None,
            &Named {
                workspace: Some("app".into()),
                ..crate::hostedsessions::tests::by_its_names()
            },
            Default::default(),
            &|_| {},
        );
    };
    start();
    let there = world.kept().join("workspaces/app/web");
    assert!(there.join("README").is_file(), "sent the first time");

    // The runner loses it; the next start sends it whole, not only what it lacked.
    std::fs::remove_dir_all(&there).unwrap();
    let here = committed(&web, "login.rs", "fn log_in() {}\n");
    start();

    assert_eq!(git(&there, &["rev-parse", "refs/heads/fix-login"]), here);
    assert_eq!(
        std::fs::read_to_string(there.join("README")).unwrap(),
        "web\n"
    );
}

#[test]
fn no_credential_file_of_a_clone_reaches_the_runner() {
    let world = World::new();
    let web = world.root.join("workspaces/app/web");
    std::fs::write(web.join(".env"), "TOKEN=not-for-the-runner\n").unwrap();
    std::fs::write(web.join("id_ed25519"), "a key\n").unwrap();
    std::fs::write(web.join(".env.example"), "TOKEN=\n").unwrap();
    std::fs::write(web.join("login.rs"), "fn log_in() {}\n").unwrap();

    let _ = world.sessions.open_named(
        None,
        &Named {
            workspace: Some("app".into()),
            ..crate::hostedsessions::tests::by_its_names()
        },
        Default::default(),
        &|_| {},
    );

    let there = world.kept().join("workspaces/app/web");
    assert!(
        there.join("login.rs").is_file(),
        "the work reached the runner"
    );
    assert!(there.join(".env.example").is_file());
    assert!(
        !there.join(".env").exists(),
        "nothing secret is kept on a runner"
    );
    assert!(!there.join("id_ed25519").exists());
}

#[test]
fn a_clones_send_leaves_out_the_persons_list_and_names_what_a_save_refuses() {
    let world = World::new();
    purlis_core::settings::workspace::set_send_leaves_out(
        &world.root,
        "app",
        &["local/".to_owned()],
    )
    .expect("the list is saved");
    let web = world.root.join("workspaces/app/web");
    std::fs::create_dir_all(web.join("local")).unwrap();
    std::fs::write(web.join("local/notes.txt"), "mine alone\n").unwrap();
    std::fs::write(
        web.join(".npmrc"),
        "//registry.example.invalid/:_authToken=abc123\n",
    )
    .unwrap();
    std::fs::write(web.join("login.rs"), "fn log_in() {}\n").unwrap();

    let _ = world.sessions.open_named(
        None,
        &Named {
            workspace: Some("app".into()),
            ..crate::hostedsessions::tests::by_its_names()
        },
        Default::default(),
        &|_| {},
    );

    let there = world.kept().join("workspaces/app/web");
    assert!(
        there.join("login.rs").is_file(),
        "the work reached the runner"
    );
    assert!(
        !there.join("local").exists(),
        "what the person named stays here"
    );
    assert!(
        !there.join(".npmrc").exists(),
        "what a save refuses stays here"
    );
    // Named in Changes; the person's own list is theirs to know.
    assert_eq!(
        crate::panels::left_out_of_send(&web),
        vec![".npmrc".to_owned()]
    );
}

#[test]
fn a_leave_out_list_that_cannot_be_read_refuses_the_send_in_words() {
    let world = World::new();
    std::fs::write(
        world.root.join("workspaces/app/workspace.json"),
        serde_json::json!({
            "name": "app",
            "settings": {"runners": {"leave_out": ["local/", "../out"]}},
        })
        .to_string(),
    )
    .unwrap();
    let web = world.root.join("workspaces/app/web");
    std::fs::create_dir_all(web.join("local")).unwrap();
    std::fs::write(web.join("local/notes.txt"), "mine alone\n").unwrap();

    let refused = world
        .sessions
        .open_named(
            None,
            &Named {
                workspace: Some("app".into()),
                ..crate::hostedsessions::tests::by_its_names()
            },
            Default::default(),
            &|_| {},
        )
        .map(|composed| composed.number)
        .expect_err("not sent");

    assert!(refused.contains("settings.runners.leave_out"), "{refused}");
    assert!(refused.contains("Sent to runners"), "{refused}");
    assert!(
        !world.kept().join("workspaces/app/web").exists(),
        "nothing reached the runner"
    );
}
