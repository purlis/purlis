// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A Docker pool's runner, for real (#2044): a container of a runner image, made confined,
//! paired with a key of its own, and linked through `docker exec -i` and its bridge to its own
//! `purlis serve`, which serves the session protocol as `remote-link` to the desktop it pinned
//! and refuses any other. Everything it made is removed, whatever the outcome.
//!
//! It needs Docker and a runner image of this build for the machine's architecture, named by
//! digest in `DOCKER_POOL_TEST_IMAGE` (`docker` on `PATH`, or the command `DOCKER_POOL_TEST_DOCKER` names). Neither
//! is spelled as purlis's own variables are, so a test that runs again without the shell's
//! steering keeps them.
//! Without the image it runs nothing and passes: a sandboxed agent cannot run it, and the parity
//! suite against Docker (#2045) is what builds the image and runs it on every merge.

#![cfg(unix)]

use std::sync::Arc;
use std::time::Duration;

use purlis_core::dockerpool::{Docker, Image, Made, Runner};
use purlis_host::CommandConnector;
use purlis_session_protocol::hostlink::{
    self, Handshake, Identity, LinkKey, Linked, Liveness, Pinned,
};
use purlis_session_protocol::session::Client;

const PATIENCE: Duration = Duration::from_secs(60);

/// Removes what the test made, however it ends.
struct Removes(Docker, Option<Made>);

impl Drop for Removes {
    fn drop(&mut self) {
        if let Some(made) = self.1.take() {
            let _ = self.0.remove(&made);
        }
    }
}

fn desktop(device: &str) -> Identity {
    Identity {
        device: device.into(),
        key: LinkKey::generate().unwrap(),
    }
}

async fn linked(
    docker: &Docker,
    made: &Made,
    ours: Identity,
    runner: Pinned,
) -> Result<Client, String> {
    let connector = CommandConnector::new(docker.program(), made.connect_args(), {
        let (docker, made) = (docker.clone(), made.clone());
        Arc::new(move || docker.check(&made).map_err(|why| why.to_string()))
    })
    .preparing({
        let docker = docker.clone();
        Arc::new(move || {
            let (env, cleared, held) = docker
                .prepared()
                .map_err(|why| why.to_string())?
                .into_parts();
            Ok(purlis_host::Prepared { env, cleared, held })
        })
    });
    let mut links = hostlink::keep_linked(
        connector,
        Handshake::THIS_BUILD,
        ours,
        runner,
        Liveness::default(),
    );
    match tokio::time::timeout(PATIENCE, links.next()).await {
        Ok(Some(Linked::Up(link))) => {
            let (client, _views, _events) = Client::new(*link);
            // Keep the kept link going for as long as the client is used.
            tokio::spawn(async move { while links.next().await.is_some() {} });
            Ok(client)
        }
        Ok(Some(Linked::Down(down))) => Err(down.why),
        Ok(None) => Err("the kept link ended".into()),
        Err(_) => Err("no link within the test's patience".into()),
    }
}

#[test]
fn a_runner_container_is_linked_to_the_desktop_it_paired_and_to_no_other() {
    purlis_core::unsteered!();
    let Ok(image) = std::env::var("DOCKER_POOL_TEST_IMAGE") else {
        // No runner image of this build is named: nothing is run (the module's docs say where
        // it runs).
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
    let runner = Runner::new(&name, image.clone(), user, &home).unwrap();
    let mut removes = Removes(docker.clone(), None);
    let made = docker
        .make(&runner)
        .expect("the runner's container is made and confined");
    removes.1 = Some(made.clone());

    let ours = desktop("01JA0DESKT0P00000000000000");
    let pinned = Pinned {
        device: ours.device.clone(),
        key: ours.key.public(),
    };
    let runner_pinned = docker.pair(&made, &pinned).expect("the runner is paired");
    // A runner is paired once: again with the same desktop, or with another, is refused.
    assert!(docker.pair(&made, &pinned).is_err());
    let stranger = desktop("01JC0STRANGER00000000000000");
    let stranger_pinned = Pinned {
        device: stranger.device.clone(),
        key: stranger.key.public(),
    };
    assert!(docker.pair(&made, &stranger_pinned).is_err());

    // Another runner of the same image mints a device id of its own at its pairing (#2539).
    let second = Runner::new(&format!("{name}-2"), image, user, &home).unwrap();
    let mut removes_second = Removes(docker.clone(), None);
    let second = docker
        .make(&second)
        .expect("a second runner of the image is made");
    removes_second.1 = Some(second.clone());
    let second_pinned = docker
        .pair(&second, &pinned)
        .expect("the second runner is paired");
    assert_ne!(second_pinned.device, runner_pinned.device);
    assert_ne!(second_pinned.key, runner_pinned.key);
    drop(removes_second);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let ours_again = Identity {
        device: ours.device.clone(),
        key: LinkKey::from_private(ours.key.private_half().unwrap()).unwrap(),
    };
    let ours_last = Identity {
        device: ours.device.clone(),
        key: LinkKey::from_private(ours.key.private_half().unwrap()).unwrap(),
    };
    runtime.block_on(async {
        let client = linked(&docker, &made, ours, runner_pinned.clone())
            .await
            .expect("the paired desktop is linked");
        assert_eq!(client.list().await.expect("listed").len(), 0);

        let refused = linked(&docker, &made, stranger, runner_pinned.clone())
            .await
            .err()
            .expect("another desktop's key is refused");
        assert!(refused.contains("refused"), "{refused}");

        // The desktop keeps the runner's key once (#2525).
        let config = tempfile::tempdir().unwrap();
        purlis_core::runners::keep(config.path(), &made, &runner_pinned, &pinned)
            .expect("the runner is kept");
        let changed = Pinned {
            device: runner_pinned.device.clone(),
            key: LinkKey::generate().unwrap().public(),
        };
        assert!(matches!(
            purlis_core::runners::keep(config.path(), &made, &changed, &pinned),
            Err(purlis_core::runners::Refused::KeyChanged { .. })
        ));

        // Removing it unpairs it over the link, then removes its container and home.
        let unpaired = linked(&docker, &made, ours_again, runner_pinned.clone())
            .await
            .expect("linked to unpair")
            .unpair()
            .await;
        assert!(unpaired.is_ok(), "{unpaired:?}");
        assert!(
            linked(&docker, &made, ours_last, runner_pinned)
                .await
                .is_err(),
            "an unpaired runner links its desktop no more"
        );
        let removed =
            purlis_core::runners::remove(config.path(), &docker, made.runner.name(), None)
                .expect("the runner is removed");
        assert_eq!(removed.left(), None, "{removed:?}");
        assert!(
            purlis_core::runners::read(config.path())
                .unwrap()
                .is_empty()
        );
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
}
