// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Try it, the whole of it, on a real Docker pool** (#4977, #4483): a project's window presses
//! Try it as Pool setup's last step does (`poolsetuptab::try_it`). It chooses the profile whose
//! harness reports its start, starts a chat in a workspace that runs its chats on this machine,
//! routed to the pool by the start's own `runs_on` alone, hears the harness's `SessionStart` hook
//! cross from the runner (ADR 0076, nothing printed is read), closes the chat and keeps that it
//! started in `pool-setup.json`. The harness waits before it reports its start, so Try it looks
//! for the chat in the host's running list first, and a pool's host that didn't list it yet would
//! end the try. Run by the parity job.

use super::*;

/// The workspace Try it starts its chat in: one set to run its chats on this machine, so only
/// the start's own choice of the pool puts it on the runner.
const WORKSPACE: &str = "app";

/// The profile, the project's default.
const PROFILE: &str = "work";

/// What the harness leaves on the runner's container once it runs there, and nowhere else.
const RAN_ON_THE_RUNNER: &str = "/tmp/try-it-ran-in-a-container";

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
    let runner = target.runner.as_ref().expect("a runner's target");
    let step = |what: &str| {
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), "Try it, whole: {what}");
    };

    step("a workspace that runs its chats on this machine, and a Claude Code profile");
    let folder = root.join("workspaces").join(WORKSPACE);
    std::fs::create_dir_all(&folder).expect("its folder");
    std::fs::write(folder.join("workspace.md"), format!("# {WORKSPACE}\n")).expect("written");
    {
        use std::io::Write as _;
        let mut ignores = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(root.join(".gitignore"))
            .expect("its ignores");
        writeln!(ignores, "/{}", purlis_core::profiles::LOCAL_FILE).expect("its ignores");
    }
    super::git(&root, &["add", "workspaces", ".gitignore"]);
    super::git(
        &root,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-q",
            "-m",
            "a workspace",
        ],
    );
    purlis_core::machine::update(&machine.config, |store| {
        store.remember(&root, 1);
        store
            .set_chats_run_here(&root, WORKSPACE)
            .expect("the workspace's setting");
    })
    .expect("kept");
    // Claude Code's start, as its hook reports it, a while after it is spawned, and only where
    // it runs in a container: Docker writes `/.dockerenv` in every container it runs.
    let bin = tempfile::tempdir().expect("a folder for the harness");
    let harness = stand_in::claude_code(
        bin.path(),
        "claude",
        &format!(
            "#!/bin/sh\n[ -e /.dockerenv ] && : > {RAN_ON_THE_RUNNER}\nsleep 2\n\
             PURLIS_HARNESS=claude '{}' hook sessionstart </dev/null\nread line\n",
            purlis()
        ),
    );
    a_profile(&target, PROFILE, &harness);
    // The project's default, which the pool keeps when none was chosen for it, so its sign-in
    // is read there (the runner's `claude auth status` says signed in, `tests/parity`).
    let declared =
        std::fs::read_to_string(root.join(purlis_core::profiles::LOCAL_FILE)).expect("its profile");
    std::fs::write(
        root.join(purlis_core::profiles::LOCAL_FILE),
        format!("[harness]\ndefault = \"{PROFILE}\"\n\n{declared}"),
    )
    .expect("its default");

    step("the project, held as the window holds it, its chats routed as the settings say");
    again.cut();
    let again = Arc::new(again);
    let planes = crate::planes::Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None)
        .running_sessions_on(Arc::new({
            let again = Arc::clone(&again);
            move |_, root: &Path| Box::new(again.routed_sessions_of(root)) as Box<dyn SessionHost>
        }))
        .hearing_hooks_through_the_host()
        .keeping_pools_in(Some(machine.config.clone()));
    let plane = planes.open(&root);
    let held = planes.held(&plane).expect("the project is held");

    step("Try it");
    let tried = crate::poolsetuptab::try_it(&held, &machine.config, &pool, Some(WORKSPACE))
        .expect("Try it answers");
    assert_eq!(
        tried,
        crate::poolsetuptab::PoolSetupTried {
            failed: None,
            cancelled: false,
            identity: None,
            workspace: Some(WORKSPACE.to_owned()),
        },
        "Try it passed"
    );

    step("its chat ran on the pool's runner, is closed, and its start is kept");
    assert_eq!(
        runner
            .prints(&format!(
                "if [ -e {RAN_ON_THE_RUNNER} ]; then echo there; fi"
            ))
            .trim(),
        "there",
        "the chat ran on the pool's runner, by the start's choice alone"
    );
    assert_eq!(
        held.chats().sessions().running(),
        Vec::<u32>::new(),
        "Try it's chat is closed"
    );
    let kept = purlis_core::poolsetup::read_choices(&machine.config).expect("pool-setup.json");
    assert!(
        kept.tried.contains_key(&pool),
        "pool-setup.json keeps that a chat started on {pool}: {:?}",
        kept.tried
    );
    drop(held);
    planes.close(&plane).expect("the project is let go");
}
