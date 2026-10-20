// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use purlis_core::dockerpool::Image;
use purlis_core::runners::{Pool, Set};
use purlis_core::secrets::keyring::FileStore;
use purlis_host::standin::StandIns;
use purlis_host::{Compose, Hosting, Routed, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client};

use super::*;

const PATIENCE: Duration = Duration::from_secs(60);

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<purlis_core::session::Spec, String> {
        Ok(purlis_core::session::Spec::new("/bin/echo", starting.size))
    }
}

/// A machine keeping its runners in `config`, its link key in a keyring file of the test's.
fn a_machine(config: &Path, docker: Docker) -> Machine {
    Machine {
        config: config.to_path_buf(),
        docker,
        keyring: Arc::new(FileStore::at(config.join("keyring.json"))),
    }
}

/// A `local-ui` link to `host`, served as the desktop's host serves each link it admits.
async fn a_window_of(host: Hosting) -> Client {
    let held = Credentials::mint().unwrap();
    let (a, b) = tokio::io::duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            held.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &held)
    );
    tokio::spawn(session::serve(served.unwrap(), Routed::new(host), None));
    Client::new(client.unwrap()).0
}

#[test]
fn a_pool_this_machine_does_not_keep_is_never_reached() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path(), Docker("/nonexistent/docker".into()));
    let refused = ToARunner::of(&machine, "builds").err().unwrap();
    assert!(refused.contains("not on this machine"), "{refused}");
    // A pool kept with no runner reaches nothing either.
    let image = Image::pinned(&format!(
        "ghcr.io/purlis/purlis-runner@sha256:{}",
        "0".repeat(64)
    ))
    .unwrap();
    purlis_core::runners::record_pool(
        config.path(),
        &Pool::new("builds", image, Set::default()).unwrap(),
    )
    .unwrap();
    let refused = ToARunner::of(&machine, "builds").err().unwrap();
    assert!(refused.contains("has no runner"), "{refused}");
}

#[test]
fn a_link_to_a_runner_reads_this_desktops_key_and_never_makes_one() {
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path(), Docker("/nonexistent/docker".into()));
    let image = Image::pinned(&format!(
        "ghcr.io/purlis/purlis-runner@sha256:{}",
        "0".repeat(64)
    ))
    .unwrap();
    purlis_core::runners::record_pool(
        config.path(),
        &Pool::new("builds", image.clone(), Set::default()).unwrap(),
    )
    .unwrap();
    let made = purlis_core::dockerpool::Made {
        container: purlis_core::dockerpool::ContainerId::new(&"a".repeat(64)).unwrap(),
        runner: purlis_core::dockerpool::Runner::new(
            "builds",
            image,
            purlis_core::dockerpool::User::new(10001, 10001).unwrap(),
            "/home/purlis",
        )
        .unwrap(),
    };
    let runner = purlis_session_protocol::hostlink::Pinned {
        device: "01JB0RUNNER0000000000000000".into(),
        key: purlis_session_protocol::hostlink::LinkKey::generate()
            .unwrap()
            .public(),
    };
    let desktop = purlis_session_protocol::hostlink::Pinned {
        device: purlis_core::machine::device_id(config.path()).unwrap(),
        key: purlis_session_protocol::hostlink::LinkKey::generate()
            .unwrap()
            .public(),
    };
    let locked = purlis_core::runners::lock(config.path());
    purlis_core::runners::keep_for_its_pool(config.path(), &made, &runner, &desktop, &locked)
        .unwrap();
    drop(locked);
    let refused = ToARunner::of(&machine, "builds").err().unwrap();
    assert!(refused.contains("not in your keychain"), "{refused}");
    assert!(
        !config.path().join("keyring.json").exists(),
        "a link made a key"
    );
}

/// **A pool added on this machine, reached by the window through its own host** (#2679): the
/// window's link is relayed by the desktop's host to the pool's runner, through `docker exec`
/// and the key pinned at its pairing. It needs Docker and a runner image of this build named by
/// digest in `DOCKER_POOL_TEST_IMAGE` (`docker` on `PATH`, or the command
/// `DOCKER_POOL_TEST_DOCKER` names); without the image it runs nothing and passes, as
/// `purlis-core`'s `a_docker_pool_runner_is_linked_through_docker_exec` does.
#[test]
fn a_window_reaches_a_pool_added_on_this_machine_through_its_own_host() {
    let Ok(image) = std::env::var("DOCKER_POOL_TEST_IMAGE") else {
        return;
    };
    let docker = Docker(
        std::env::var_os("DOCKER_POOL_TEST_DOCKER")
            .unwrap_or("docker".into())
            .into(),
    );
    let config = tempfile::tempdir().unwrap();
    let machine = a_machine(config.path(), docker.clone());
    let name = format!(
        "test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let pool = Pool::new(&name, Image::pinned(&image).unwrap(), Set::default()).unwrap();
    crate::dockerpools::add_on(&machine, &pool).expect("the pool is added and its runner paired");
    let removes = scopeguard(config.path().to_path_buf(), docker.clone(), name.clone());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let here = Hosting::running_with(Echo, StandIns::default());
        let host = here.clone().relaying_with(relaying({
            let machine = machine.clone();
            move || Ok(machine.clone())
        }));
        let window = a_window_of(host.clone()).await;
        tokio::time::timeout(PATIENCE, window.relay_to(&name))
            .await
            .unwrap()
            .expect("the window's link is relayed to the pool's runner");
        assert!(
            tokio::time::timeout(PATIENCE, window.list())
                .await
                .unwrap()
                .unwrap()
                .is_empty()
        );
        // The runner's switch is thrown, and this machine's is not.
        let switch = tokio::time::timeout(PATIENCE, window.stop_all())
            .await
            .unwrap()
            .unwrap();
        assert!(switch.stopped);
        let mine = a_window_of(host).await;
        assert!(!mine.kill_switch().await.unwrap().stopped);
        // A pool this machine does not keep is refused.
        let other = a_window_of(here.relaying_with(relaying({
            let machine = machine.clone();
            move || Ok(machine.clone())
        })))
        .await;
        assert!(other.relay_to("not-kept").await.is_err());
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
    // Its runner answers with the key pinned for it, and removing the pool unpairs it over a
    // link of its own, then removes its container, home and record.
    assert_eq!(
        link_state(&ToARunner::of(&machine, &name).unwrap()),
        Ok(crate::dockerpools::PoolLink::Linked)
    );
    let to = ToARunner::of(&machine, &name).unwrap();
    assert_eq!(
        crate::dockerpools::remove_on(&machine, &name),
        Ok(None),
        "the pool is removed whole"
    );
    assert!(
        purlis_core::runners::pools(config.path())
            .unwrap()
            .is_empty()
    );
    assert!(!matches!(
        link_state(&to),
        Ok(crate::dockerpools::PoolLink::Linked)
    ));
    std::mem::forget(removes);
}

/// Removes the pool `name` and its runner however the test ends.
fn scopeguard(config: PathBuf, docker: Docker, name: String) -> impl Drop {
    struct Removes(PathBuf, Docker, String);
    impl Drop for Removes {
        fn drop(&mut self) {
            let removed = purlis_core::pools::remove(&self.0, &self.1, &self.2, None);
            assert!(
                removed
                    .as_ref()
                    .is_ok_and(|removed| removed.as_ref().is_none_or(|removed| removed.forgotten)),
                "{removed:?}"
            );
        }
    }
    Removes(config, docker, name)
}
