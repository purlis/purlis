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
    let machine = a_machine(
        config.path(),
        Docker::at("/nonexistent/docker").with_settings_in(config.path().join("docker-settings")),
    );
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
    let machine = a_machine(
        config.path(),
        Docker::at("/nonexistent/docker").with_settings_in(config.path().join("docker-settings")),
    );
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
    let config = tempfile::tempdir().unwrap();
    let docker = Docker::at(std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()))
        .with_settings_in(config.path().join("docker-settings"));
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

/// **A runner's version is asked of it once for each container and image** (#2749): a link
/// after the first reads what the first was told; one that could not be read is asked again.
#[test]
fn a_runners_version_is_read_once_for_its_container_and_image() {
    let asked = std::cell::Cell::new(0);
    let read = |answer: Result<&str, &str>| {
        asked.set(asked.get() + 1);
        answer.map(str::to_owned).map_err(str::to_owned)
    };
    let (container, image) = ("c".repeat(64), "ghcr.io/purlis/purlis-runner@sha256:once");
    assert!(once_for(&container, image, || read(Err("not up yet"))).is_err());
    assert_eq!(
        once_for(&container, image, || read(Ok("0.4.2-dev.9"))),
        Ok("0.4.2-dev.9".into())
    );
    assert_eq!(
        once_for(&container, image, || read(Ok("another"))),
        Ok("0.4.2-dev.9".into())
    );
    assert_eq!(asked.get(), 2, "read once it answered, and not again");
    // Another image in the same container is read anew.
    assert_eq!(
        once_for(
            &container,
            "ghcr.io/purlis/purlis-runner@sha256:other",
            || read(Ok("0.4.3"))
        ),
        Ok("0.4.3".into())
    );
}

/// **Only a runner kept here and not draining takes a new chat** (#2865): a runners file that
/// cannot be read, a runner not kept, and one that drains each refuse the start.
#[test]
fn a_start_is_refused_unless_its_runner_is_kept_and_not_draining() {
    let config = tempfile::tempdir().unwrap();
    // Nothing kept.
    assert!(drains(config.path(), "builds/builds"));
    assert!(drains(config.path(), "not a pool and runner"));

    // A file that cannot be read refuses every start.
    let file = purlis_core::machine::dir(config.path()).join(purlis_core::machine::RUNNERS_FILE);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "not runners").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(purlis_core::runners::read(config.path()).is_err());
    assert!(drains(config.path(), "builds/builds"));
    std::fs::remove_file(&file).unwrap();

    // A runner kept and not draining takes chats; once it drains, it does not.
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
        container: purlis_core::dockerpool::ContainerId::new(&"b".repeat(64)).unwrap(),
        runner: purlis_core::dockerpool::Runner::new(
            "builds",
            image.clone(),
            purlis_core::dockerpool::User::new(10001, 10001).unwrap(),
            "/home/purlis",
        )
        .unwrap(),
    };
    let ours = purlis_session_protocol::hostlink::Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: purlis_session_protocol::hostlink::LinkKey::generate()
            .unwrap()
            .public(),
    };
    let answered = |device: &str| purlis_session_protocol::hostlink::Pinned {
        device: device.into(),
        key: purlis_session_protocol::hostlink::LinkKey::generate()
            .unwrap()
            .public(),
    };
    let locked = purlis_core::runners::lock(config.path());
    purlis_core::runners::keep_for_its_pool(
        config.path(),
        &made,
        &answered("01JB0RUNNER0000000000000000"),
        &ours,
        &locked,
    )
    .unwrap();
    assert!(!drains(config.path(), "builds/builds"));
    let next = Image::pinned(&format!(
        "ghcr.io/purlis/purlis-runner@sha256:{}",
        "1".repeat(64)
    ))
    .unwrap();
    purlis_core::runners::succeed_locked(
        config.path(),
        "builds",
        &purlis_core::dockerpool::Made {
            container: purlis_core::dockerpool::ContainerId::new(&"c".repeat(64)).unwrap(),
            runner: purlis_core::dockerpool::Runner::new(
                "builds-11111111",
                next,
                purlis_core::dockerpool::User::new(10001, 10001).unwrap(),
                "/home/purlis",
            )
            .unwrap(),
        },
        &answered("01JB0RUNNER0000000000000001"),
        &ours,
        &locked,
    )
    .unwrap();
    assert!(drains(config.path(), "builds/builds"));
    assert!(!drains(config.path(), "builds/builds-11111111"));
}

/// **A project reaches a pool's runner through the window, for real** (#2046): the window's
/// chats of a workspace set to the pool are routed to a link the desktop's host relays to the
/// pool's runner through `docker exec`; the runner holds the project where it keeps it, and the
/// window sends it the project's own repository, which the runner checks out there, less what is
/// this machine's alone. It needs Docker and a runner image of this build named by digest in
/// `DOCKER_POOL_TEST_IMAGE`; without the image it runs nothing and passes. The project keeps its
/// git directory beside its tree, so it runs where a sandbox refuses a folder's own
/// `.git/config`.
#[test]
fn a_project_reaches_a_pools_runner_through_the_window() {
    use crate::host::SessionHost as _;
    let Ok(image) = std::env::var("DOCKER_POOL_TEST_IMAGE") else {
        return;
    };
    let config = tempfile::tempdir().unwrap();
    let docker = Docker::at(std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()))
        .with_settings_in(config.path().join("docker-settings"));
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
    let _removes = scopeguard(config.path().to_path_buf(), docker.clone(), name.clone());

    // The project: a repository whose git directory is beside it, with a memory not saved yet
    // and this machine's own state.
    let top = tempfile::tempdir().unwrap();
    let top = top.path().canonicalize().unwrap();
    let root = top.join("project");
    std::fs::create_dir_all(root.join("workspaces/app")).unwrap();
    let git = |args: &[&str]| {
        let out = purlis_core::forklock::output(
            std::process::Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(["-c", "commit.gpgsign=false", "-c", "user.name=T", "-c"])
                .args(["user.email=t@example.invalid"])
                .args(args),
        )
        .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    };
    let gitdir = format!(
        "--separate-git-dir={}",
        top.join("project.gitdir").display()
    );
    git(&["init", "-q", "-b", "main", &gitdir, "."]);
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").unwrap();
    std::fs::write(root.join("workspaces/app/workspace.md"), "# app\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "a project"]);
    std::fs::write(root.join("workspaces/app/memory.md"), "not saved yet\n").unwrap();
    // This machine's state folder, where its vaults are kept.
    std::fs::create_dir_all(root.join(".purlis")).unwrap();
    std::fs::write(root.join(".purlis/this-machine.json"), "{}").unwrap();
    purlis_core::machine::update(config.path(), |store| {
        store.remember(&root, 1);
        store.set_chats_run_on(&root, "app", Some(&name)).unwrap();
    })
    .unwrap();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let host = runtime.block_on(async {
        Hosting::running_with(Echo, StandIns::default()).relaying_with(relaying({
            let machine = machine.clone();
            move || Ok(machine.clone())
        }))
    });
    let handle = runtime.handle().clone();
    let link_to = {
        let (host, handle) = (host.clone(), handle.clone());
        move |naming, pool: Option<String>| {
            let host = host.clone();
            let inner = handle.clone();
            crate::hostedsessions::open_within(&handle, PATIENCE, async move {
                let link = a_link_of(host).await;
                crate::hostedsessions::HostLink::over_relayed(
                    link,
                    inner,
                    || {},
                    naming,
                    pool.as_deref(),
                )
                .await
            })
        }
    };
    let connection = Arc::new(crate::hostedsessions::Connection::default());
    let window = link_to(None, None).expect("the window's own link");
    let dial: crate::hostedsessions::Dial = {
        let link_to = link_to.clone();
        Arc::new(move |_within, naming| link_to(naming, None))
    };
    let relayed: crate::hostedsessions::RelayedDial = {
        let link_to = link_to.clone();
        Arc::new(move |_within, naming, pool: &str| link_to(naming, Some(pool.to_owned())))
    };
    connection.ready_relaying(Arc::new(window), dial, relayed);
    let routed = crate::routedsessions::RoutedSessions::of_the_project(
        Box::new(crate::hostedsessions::HostedSessions::new(
            Arc::clone(&connection),
            &root,
        )),
        &connection,
        config.path(),
        &root,
    );

    // The window's start of a chat in `app` holds the project on the runner, with its code.
    let started = routed.open_named(
        None,
        &crate::host::Named {
            workspace: Some("app".into()),
            ..crate::hostedsessions::tests::by_its_names()
        },
        Default::default(),
        &|_| {},
    );
    let id = purlis_core::scaffold::planefile::id_to_hold_by(&root).unwrap();
    let made = purlis_core::runners::runner_for(config.path(), &name)
        .unwrap()
        .made;
    let in_the_runner = |script: &str| {
        let out =
            purlis_core::forklock::output(std::process::Command::new(docker.program()).args([
                "exec",
                made.container.as_str(),
                "sh",
                "-c",
                script,
            ]))
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    let kept = format!(
        "$(ls -d {}/{}/{id})",
        purlis_core::datahome::IN_A_RUNNER_CONTAINER,
        purlis_core::datahome::PROJECTS
    );
    assert_eq!(
        in_the_runner(&format!("cat {kept}/workspaces/app/memory.md")),
        "not saved yet",
        "the project is on the runner, uncommitted work and all ({:?})",
        started.as_ref().err()
    );
    assert_eq!(
        in_the_runner(&format!(
            "test -e {kept}/.purlis && echo there || echo absent"
        )),
        "absent",
        "nothing of this machine's state or vaults"
    );
    drop(routed);
    runtime.shutdown_timeout(Duration::from_secs(5));
}

/// A `local-ui` link to `host`, served as the desktop's host serves each link it admits.
async fn a_link_of(host: Hosting) -> link::Link {
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
    client.unwrap()
}

/// **Only a runner of 1.43 or later is asked to keep a profile** (#2049): one before it is sent
/// nothing, and its chats run on the profiles it has of its own.
#[test]
fn only_a_runner_of_1_43_or_later_keeps_a_desktops_profiles() {
    use purlis_session_protocol::version::Version;
    assert!(!keeps_profiles(Version {
        major: 1,
        minor: 42
    }));
    assert!(keeps_profiles(Version {
        major: 1,
        minor: 43
    }));
    assert!(keeps_profiles(Version {
        major: 1,
        minor: 50
    }));
}

/// The desktop's profile `work`, its settings in `folder`.
fn work_with_settings_in(folder: &std::path::Path) -> purlis_core::profilestore::onrunner::Kept {
    purlis_core::profilestore::onrunner::Kept {
        name: "work".into(),
        harness: "claude".into(),
        sandbox_only: false,
        pinned: None,
        settings_in: Some(folder.to_path_buf()),
    }
}

fn version(minor: u16) -> purlis_session_protocol::version::Version {
    purlis_session_protocol::version::Version { major: 1, minor }
}

/// **A profile's settings go only to a runner of 1.50 or later** (#3448): one before it ignores
/// them, so it is sent the profile alone and keeps the settings it has.
#[test]
fn only_a_runner_of_1_50_or_later_is_sent_a_profiles_settings() {
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(folder.path().join("CLAUDE.md"), "Be brief.").unwrap();
    let kept = work_with_settings_in(folder.path());

    let older = for_a_runner_of(version(49), &kept).unwrap();
    let newer = for_a_runner_of(version(50), &kept).unwrap();

    assert_eq!(older.profile.settings, None);
    assert_eq!(older.profile.name, "work");
    let sent = newer.profile.settings.expect("sent");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].path, "CLAUDE.md");
}

/// **Settings never refuse a chat on a runner that ignores them** (#3448, F2): a settings folder
/// too large to send refuses the profile on a runner of 1.50, in words, and a runner before it
/// keeps the profile as it always did, with nothing of its settings read.
#[test]
fn settings_too_large_to_send_refuse_no_chat_on_a_runner_before_1_50() {
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(
        folder.path().join("CLAUDE.md"),
        vec![b'a'; purlis_core::profilestore::onrunner::MOST_SETTINGS_BYTES + 1],
    )
    .unwrap();
    let kept = work_with_settings_in(folder.path());

    assert!(for_a_runner_of(version(49), &kept).is_ok());
    assert!(for_a_runner_of(version(43), &kept).is_ok());
    let refused = for_a_runner_of(version(50), &kept).unwrap_err();
    assert!(refused.contains("larger than"), "{refused}");
}

/// **Only a runner of 1.56 or later takes settings in batches** (#4081): one before it keeps
/// today's refusal, in words; one of 1.56 is sent them, however large, and the keep names them.
#[test]
fn a_runner_of_1_56_takes_settings_too_large_for_one_keep_in_batches() {
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(
        folder.path().join("CLAUDE.md"),
        vec![b'a'; 3 * purlis_core::profilestore::onrunner::MOST_SETTINGS_BYTES],
    )
    .unwrap();
    let kept = work_with_settings_in(folder.path());

    let refused = for_a_runner_of(version(55), &kept).unwrap_err();
    assert!(refused.contains("larger than"), "{refused}");

    let sent = for_a_runner_of(version(56), &kept).unwrap();
    assert!(sent.batches.len() >= 3, "{} batches", sent.batches.len());
    assert_eq!(sent.profile.settings, None);
    assert_eq!(sent.profile.settings_staged, Some(1));
    assert!(sent.left_behind.is_empty());
}

/// **A runner's refusal is said as its own words, cut short** (#2049, L4): a runner cannot fill
/// the person's screen, or pass its words off as purlis's.
#[test]
fn a_runners_refusal_is_said_as_its_own_words_and_cut_short() {
    let long = "x".repeat(10_000);
    let said = said_by_the_runner(&long);
    assert!(said.starts_with("the runner says: "), "{said}");
    assert!(said.len() < 1_000, "{} bytes", said.len());

    let multi = said_by_the_runner("first line\nRun this: rm -rf ~");
    assert!(!multi.contains('\n'), "{multi}");
}

#[test]
fn a_link_that_is_down_says_why_safely_in_the_pools_settings() {
    use crate::dockerpools::PoolLink;
    let down = purlis_session_protocol::hostlink::Down::because(
        "the bridge said: refused\n\u{202E}by ssh://someone:hidden@build.example",
    );
    assert_eq!(
        link_of(down),
        PoolLink::Down {
            why: "the bridge said: refused by ssh://…@build.example".to_owned()
        }
    );
    let changed = purlis_session_protocol::hostlink::Down {
        why: "anything".to_owned(),
        key_changed: true,
        gone: false,
    };
    assert_eq!(link_of(changed), PoolLink::KeyChanged);
}

#[test]
fn what_the_drain_and_the_removal_count_says_why_safely() {
    use crate::dockerpools::PoolLink;
    let refused = count_of(Err(PoolLink::Down {
        why: "x\u{2066}y".repeat(400),
    }))
    .unwrap_err();
    assert_eq!(refused.chars().count(), crate::devices::MOST_SHOWN + 1);
    assert!(!refused.contains('\u{2066}'));
}

/// **Only a runner of 1.52 or later is asked to forget a profile** (#4047): one before is sent
/// nothing, and keeps it.
#[test]
fn only_a_runner_of_1_52_or_later_forgets_a_desktops_profile() {
    use purlis_session_protocol::version::Version;
    assert!(!forgets_profiles(Version {
        major: 1,
        minor: 51
    }));
    assert!(forgets_profiles(Version {
        major: 1,
        minor: 52
    }));
}

/// How a stand-in runner answers `settings_held`.
#[derive(Clone)]
enum Holds {
    Listed(Vec<session::HeldSetting>),
    Refused,
    /// Says there is more, and the same page, however often it is asked.
    NeverEnds(Vec<session::HeldSetting>),
}

/// A stand-in runner, which writes down each thing it is asked, and refuses a keep of what changed
/// where told to.
#[derive(Clone)]
struct RunnerSaying {
    holds: Holds,
    refuses_changes: bool,
    asked: Arc<std::sync::Mutex<Vec<session::Command>>>,
}

impl session::Host for RunnerSaying {
    async fn call(
        &self,
        command: session::Command,
        _peer: &session::Peer,
    ) -> Result<session::Answer, session::Refusal> {
        use session::{Answer, Command, ProfileStanding};
        self.asked.lock().unwrap().push(command.clone());
        let refused = || session::Refusal {
            code: session::NOT_STARTED.into(),
            why: "no".to_owned(),
        };
        match command {
            Command::SettingsHeld { .. } => match &self.holds {
                Holds::Listed(files) => Ok(Answer::SettingsHeld {
                    files: files.clone(),
                    more: false,
                }),
                Holds::Refused => Err(refused()),
                Holds::NeverEnds(files) => Ok(Answer::SettingsHeld {
                    files: files.clone(),
                    more: true,
                }),
            },
            Command::KeepProfile(profile)
                if self.refuses_changes && profile.settings_changed.is_some() =>
            {
                Err(refused())
            }
            Command::KeepProfile(_) => Ok(Answer::ProfileKept {
                standing: ProfileStanding::SignedIn,
            }),
            _ => Ok(Answer::Done),
        }
    }
}

/// What a stand-in runner saying `holds` was asked as the desktop kept a profile with a file in it,
/// and the keep's answer.
async fn sent_to_a_runner(
    speaks: u16,
    holds: impl FnOnce(session::HeldSetting) -> Holds,
    refuses_changes: bool,
) -> (Vec<session::Command>, Result<ProfileKept, String>) {
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(folder.path().join("CLAUDE.md"), "Be brief.").unwrap();
    let kept = work_with_settings_in(folder.path());
    let runner = RunnerSaying {
        holds: holds(session::HeldSetting {
            path: "CLAUDE.md".to_owned(),
            digest: session::settings_digest(b"Be brief."),
        }),
        refuses_changes,
        asked: Arc::default(),
    };
    let asked = Arc::clone(&runner.asked);
    let (a, b) = tokio::io::duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect_as_a_device(a, session::speaks()),
        link::serve_as_a_device(b, session::speaks(), link::ProvenDevice::stand_in())
    );
    tokio::spawn(session::serve(served.unwrap(), runner, None));
    let client = Client::new(client.unwrap()).0;
    let answer = sent_what_differs(&client, version(speaks), &kept).await;
    let asked = asked.lock().unwrap().clone();
    (asked, answer)
}

/// Each keep among `asked`: whether it said what changed, and how many stage calls came before it
/// since the last keep.
fn keeps_in(asked: &[session::Command]) -> Vec<(bool, usize)> {
    let mut stages = 0;
    let mut keeps = Vec::new();
    for one in asked {
        match one {
            session::Command::StageProfileSettings(_) => stages += 1,
            session::Command::KeepProfile(profile) => {
                keeps.push((profile.settings_changed.is_some(), stages));
                stages = 0;
            }
            _ => {}
        }
    }
    keeps
}

/// **An unchanged profile sends nothing beyond the listing** (#4088): one keep, nothing staged.
#[tokio::test]
async fn a_runner_holding_what_the_desktop_has_is_sent_nothing() {
    let (asked, answer) = sent_to_a_runner(60, |same| Holds::Listed(vec![same]), false).await;
    assert!(answer.is_ok());
    assert_eq!(keeps_in(&asked), [(true, 0)]);
}

/// **A runner that lied about what it holds is sent them all** (#4088): its keep refused, whatever
/// it said, the settings go whole on the same link, in place of whatever it holds.
#[tokio::test]
async fn a_keep_of_what_changed_refused_sends_them_all() {
    let (asked, answer) = sent_to_a_runner(60, |same| Holds::Listed(vec![same]), true).await;
    assert!(answer.is_ok());
    assert_eq!(keeps_in(&asked), [(true, 0), (false, 1)]);
}

/// **A runner that gives no usable list is sent them all** (#4088): one that refuses, one whose
/// pages never end, one that holds nothing.
#[tokio::test]
async fn a_runner_with_no_usable_list_is_sent_them_all() {
    let cases: [fn(session::HeldSetting) -> Holds; 3] = [
        |_| Holds::Refused,
        |same| Holds::NeverEnds(vec![same]),
        |_| Holds::Listed(Vec::new()),
    ];
    for holds in cases {
        let (asked, answer) = sent_to_a_runner(60, holds, false).await;
        assert!(answer.is_ok());
        assert_eq!(keeps_in(&asked), [(false, 1)]);
    }
}

/// **A runner before 1.60 is asked nothing new, and is sent them all** (#4088).
#[tokio::test]
async fn a_runner_before_1_60_is_sent_them_all_and_asked_nothing_new() {
    let (asked, answer) = sent_to_a_runner(59, |same| Holds::Listed(vec![same]), false).await;
    assert!(answer.is_ok());
    assert!(
        !asked
            .iter()
            .any(|one| matches!(one, session::Command::SettingsHeld { .. }))
    );
    assert_eq!(keeps_in(&asked), [(false, 1)]);
}
