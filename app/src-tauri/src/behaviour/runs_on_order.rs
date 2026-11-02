// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where a chat runs follows the start, then the workspace, then the project** (#4536, the
//! person's ruling of 2026-11-01), on a real Docker pool: a project that runs its chats on the
//! pool, a workspace set to this machine, and a start in that workspace that chose the pool, each
//! routed by this machine's store as a window started on it routes them. A start that chose a
//! pool that is gone starts nothing anywhere.
//! Run by the parity job.

use std::process::Command;

use purlis_core::machine::StartChoice;

use crate::host::Named;

use super::*;

/// A workspace that follows the project, and one set to this machine.
const FOLLOWS: &str = "follows";
const HERE: &str = "here";

/// `git` in `root`, as a test's own commit.
fn git(root: &Path, args: &[&str]) {
    let ran = purlis_core::forklock::output(
        Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(root),
    )
    .expect("git runs");
    assert!(ran.status.success(), "git {args:?}: {ran:?}");
}

#[test]
fn in_a_docker_pool() {
    let Ok(image) = std::env::var(super::DOCKER_POOL_IMAGE) else {
        assert!(
            std::env::var_os(super::PARITY).is_none(),
            "{} is set and {} names no runner image: the parity job built none",
            super::PARITY,
            super::DOCKER_POOL_IMAGE
        );
        return;
    };
    let docker = purlis_core::dockerpool::Docker::at(
        std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()),
    );
    let (target, machine, pool, again) = super::a_window_again_on_a_pool_of(&image, docker);
    let root = target.root.clone();
    let step = |what: &str| {
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), "runs on, in order: {what}");
    };

    step("the project runs its chats on the pool, and one workspace on this machine");
    for workspace in [FOLLOWS, HERE] {
        let folder = root.join("workspaces").join(workspace);
        std::fs::create_dir_all(&folder).expect("its folder");
        std::fs::write(folder.join("workspace.md"), format!("# {workspace}\n")).expect("written");
    }
    git(&root, &["add", "workspaces"]);
    git(&root, &["commit", "-q", "-m", "two workspaces"]);
    purlis_core::machine::update(&machine.config, |store| {
        store.remember(&root, 1);
        store
            .set_project_chats_run_on(&root, Some(pool.as_str()))
            .expect("the project's setting");
        store
            .set_chats_run_here(&root, HERE)
            .expect("the workspace's setting");
    })
    .expect("kept");
    again.cut();
    let window = again.routed_sessions_of(&root);
    let on_the_pool = Some(StartChoice::Pool(pool.clone()));
    let this_machine = Some(StartChoice::ThisMachine);

    step("each level decides as the order says");
    assert_eq!(
        window.pool_for(Some(FOLLOWS), None),
        Some(pool.clone()),
        "the project's pool"
    );
    assert_eq!(
        window.pool_for(Some(HERE), None),
        None,
        "the workspace's this machine"
    );
    assert_eq!(
        window.pool_for(Some(HERE), on_the_pool.as_ref()),
        Some(pool.clone()),
        "the start's pool, over the workspace"
    );
    assert_eq!(
        window.pool_for(Some(FOLLOWS), this_machine.as_ref()),
        None,
        "the start's this machine, over the project"
    );

    step("a start that chose the pool, in the workspace set to this machine, runs on its runner");
    let bin = tempfile::tempdir().expect("a folder for the harness");
    let harness = stand_in::claude_code(
        bin.path(),
        "claude",
        "#!/bin/sh\nprintf 'RUNS ON %s.' \"$(cat /etc/hostname)\"\nread line\n",
    );
    a_profile(&target, "work", &harness);
    let chose_the_pool = Named {
        workspace: Some(HERE.to_owned()),
        runs_on: on_the_pool.clone(),
        ..named("hello", false)
    };
    let id = window
        .open_named(None, &chose_the_pool, Hearing::default(), &|_| {})
        .expect("it starts on the pool it chose")
        .number;
    assert_eq!(window.pool_of(id).as_deref(), Some(pool.as_str()));
    Pane::of(&window, id).shows_within("RUNS ON ", Duration::from_secs(120));
    window.close(id).expect("it closes");

    step("a start that chose a pool that is gone starts nothing, here or there");
    let gone = Named {
        workspace: Some(HERE.to_owned()),
        runs_on: Some(StartChoice::Pool("gone".to_owned())),
        ..named("hello", false)
    };
    let refused = window
        .open_named(None, &gone, Hearing::default(), &|_| {})
        .map(|composed| composed.number)
        .expect_err("a pool that is gone starts nothing");
    assert!(refused.contains("pool gone"), "{refused}");
    assert!(
        window.running().is_empty(),
        "never started here in its place"
    );
}
