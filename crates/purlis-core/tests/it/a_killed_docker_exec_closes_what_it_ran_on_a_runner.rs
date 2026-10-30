// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a runner's end of a sign-in stands on, for real** (#4043): ending a `docker exec -i`
//! client ends nothing it started in the container, but the input of what it started closes, and
//! that is what the runner's own `purlis` ends a login by. Without this a cancelled sign-in would
//! run on there. Run as the Docker pool's other tests are: it needs Docker and a runner image of
//! this build, named in `DOCKER_POOL_TEST_IMAGE`, and without the image it runs nothing and passes.

#![cfg(unix)]

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use purlis_core::dockerpool::{Docker, Image, Made, Runner};

const PATIENCE: Duration = Duration::from_secs(30);

/// Removes what the test made, however it ends.
struct Removes(Docker, Option<Made>);

impl Drop for Removes {
    fn drop(&mut self) {
        if let Some(made) = self.1.take() {
            let _ = self.0.remove(&made);
        }
    }
}

/// `docker exec -i` of `script` in `made`'s container, as its runner's user, with its input held.
fn exec(docker: &Docker, made: &Made, script: &str) -> Child {
    let (env, cleared, _held) = docker.prepared().expect("prepared").into_parts();
    let mut command = Command::new(docker.program());
    command
        // `exec -i --user <its user> <its container>`, as every word to a runner starts.
        .args(&made.version_args()[..5])
        .args(["/bin/sh", "-c", script])
        .envs(env.iter().map(|(n, v)| (n, v)))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for name in &cleared {
        command.env_remove(name);
    }
    purlis_core::forklock::spawn(&mut command).expect("docker exec runs")
}

/// Whether `path` in `made`'s container comes to be within [`PATIENCE`].
fn comes_to_be(docker: &Docker, made: &Made, path: &str) -> bool {
    let until = Instant::now() + PATIENCE;
    while Instant::now() < until {
        let mut look = exec(docker, made, &format!("test -e {path}"));
        if look.wait().expect("docker exec ends").success() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

#[test]
fn a_killed_docker_exec_closes_the_input_of_what_it_ran() {
    purlis_core::unsteered!();
    let Ok(image) = std::env::var("DOCKER_POOL_TEST_IMAGE") else {
        return;
    };
    let settings = tempfile::tempdir().unwrap();
    let docker = Docker::at(std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()))
        .with_settings_in(settings.path().join("docker-settings"));
    let image = Image::pinned(&image).expect("the image is named by digest");
    let (user, home) = docker
        .declared(&image)
        .expect("the image declares its user and home");
    let name = format!("test-{}", ulid::Ulid::generate().to_string().to_lowercase());
    let runner = Runner::new(&name, image, user, &home).unwrap();
    let mut removes = Removes(docker.clone(), None);
    let made = docker
        .make(&runner)
        .expect("the runner's container is made and confined");
    removes.1 = Some(made.clone());

    let mut client = exec(
        &docker,
        &made,
        &format!("touch {home}/started; cat > /dev/null; touch {home}/input-closed"),
    );
    // Held open, so what closes the input in the container is the client going, not its input.
    let _input = client.stdin.take();
    assert!(comes_to_be(&docker, &made, &format!("{home}/started")));

    client.kill().expect("the client is ended");
    let _ = client.wait();

    assert!(
        comes_to_be(&docker, &made, &format!("{home}/input-closed")),
        "what the client ran never saw its input close"
    );
}
