// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Pool setup reads a real Docker pool** (#4478, spec #4469): each step's state against real
//! Docker, with work done outside it picked up on the next read. Run by the parity job.

use purlis_core::dockerpool::Docker;
use purlis_core::poolsetup::{self, ActionId, Here, Setup, StepId, StepState};

fn state(setup: &Setup, id: StepId) -> StepState {
    setup.step(id).expect("a Docker step").state
}

fn next(setup: &Setup, id: StepId) -> ActionId {
    setup
        .step(id)
        .and_then(|step| step.next.as_ref())
        .unwrap_or_else(|| panic!("{id:?} has a next action: {setup:?}"))
        .id
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
    let config = tempfile::tempdir().expect("the desktop's config home");
    let docker = Docker::at(std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()))
        .with_settings_in(config.path().join("docker-settings"));
    let here = Here {
        config: config.path(),
        plane: None,
        docker: Some(&docker),
        engine: Docker::engine_on_this_machine(),
    };

    // Before there is a pool: real Docker answers, so it is recommended, and nothing is chosen.
    let before = poolsetup::read(&here, None).expect("read");
    assert_eq!(before.at, Some(StepId::Where), "{before:?}");
    assert_eq!(next(&before, StepId::Where), ActionId::UseDocker);
    poolsetup::choose_kind(config.path(), poolsetup::Kind::Docker).expect("kept");
    let chose = poolsetup::read(&here, None).expect("read");
    assert_eq!(chose.at, Some(StepId::Pool), "{chose:?}");

    // A pool made outside Pool setup is picked up on the next read.
    let machine = crate::runnerlinks::Machine {
        config: config.path().to_path_buf(),
        docker: docker.clone(),
        keyring: std::sync::Arc::new(purlis_core::secrets::keyring::FileStore::at(
            config.path().join("keyring.json"),
        )),
    };
    let (name, removes) = super::a_pool_on(&machine, &image);
    let added = poolsetup::read(&here, Some(&name)).expect("read");
    assert_eq!(state(&added, StepId::Where), StepState::Done, "{added:?}");
    assert_eq!(state(&added, StepId::Pool), StepState::Done, "{added:?}");
    assert_eq!(added.at, Some(StepId::Signins), "{added:?}");
    // With no project open, a pool that chose no profile can't say which it keeps.
    assert_eq!(next(&added, StepId::Signins), ActionId::OpenProject);

    // A profile the runner doesn't keep is asked of the real runner.
    purlis_core::runners::set_pool_profiles(config.path(), &name, Some(vec!["work".into()]))
        .expect("chosen");
    let asked = poolsetup::read(&here, Some(&name)).expect("read");
    assert_eq!(
        state(&asked, StepId::Signins),
        StepState::Current,
        "{asked:?}"
    );
    assert_eq!(next(&asked, StepId::Signins), ActionId::ChooseProfiles);

    // A skip moves on, and the skipped step says so.
    poolsetup::skip(config.path(), &name, StepId::Signins).expect("kept");
    let skipped = poolsetup::read(&here, Some(&name)).expect("read");
    assert_eq!(state(&skipped, StepId::Signins), StepState::Skipped);
    assert_eq!(skipped.at, Some(StepId::Identity), "{skipped:?}");
    poolsetup::unskip(config.path(), &name, StepId::Signins).expect("kept");

    // A sleeping runner is said to be asleep, and reading doesn't wake it.
    let kept = purlis_core::runners::runner_for(config.path(), &name).expect("kept");
    let stopped =
        purlis_core::forklock::output(std::process::Command::new(docker.program()).args([
            "container",
            "stop",
            kept.made.container.as_str(),
        ]))
        .expect("docker ran");
    assert!(stopped.status.success(), "{stopped:?}");
    let asleep = poolsetup::read(&here, Some(&name)).expect("read");
    assert_eq!(next(&asleep, StepId::Signins), ActionId::WakeRunner);
    assert!(
        !docker.running(&kept.made).expect("docker answers"),
        "a read woke the runner"
    );

    // A runner removed by hand is picked up on the next read, on the Pool step alone.
    let removed =
        purlis_core::forklock::output(std::process::Command::new(docker.program()).args([
            "container",
            "rm",
            "--force",
            kept.made.container.as_str(),
        ]))
        .expect("docker ran");
    assert!(removed.status.success(), "{removed:?}");
    let gone = poolsetup::read(&here, Some(&name)).expect("read");
    assert_eq!(state(&gone, StepId::Where), StepState::Done, "{gone:?}");
    assert_eq!(state(&gone, StepId::Pool), StepState::Attention, "{gone:?}");
    assert_eq!(next(&gone, StepId::Pool), ActionId::MakeRunnerAgain);
    drop(removes);
}
