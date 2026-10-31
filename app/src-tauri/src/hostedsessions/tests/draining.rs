// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A start while a pool's runner drains, and a start on a slow host** (#4248, #4264), through
//! the desktop's host as it serves each link (`Routed`).

use purlis_host::standin::StandInConfines;

use super::*;

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OLD: &str = "builds";
const NEW: &str = "builds-01234567";

/// `runners.json` under `config`: pool `builds`, its runner `builds`, and once it `followed`, that
/// one draining beside a runner of the pool's image.
fn keep_runners(config: &Path, followed: bool) {
    let row = |name: &str, extra: serde_json::Value| {
        let mut row = serde_json::json!({
            "name": name,
            "pool": "docker",
            "container": "a".repeat(64),
            "image": IMAGE,
            "user": "10001:10001",
            "home": "/home/purlis",
            "device": "01JB0RUNNER0000000000000000",
            "key": "12".repeat(32),
            "paired": 1,
        });
        for (key, value) in extra.as_object().unwrap() {
            row[key] = value.clone();
        }
        row
    };
    let runners = if followed {
        vec![
            row(OLD, serde_json::json!({ "draining": true })),
            row(NEW, serde_json::json!({ "of": OLD })),
        ]
    } else {
        vec![row(OLD, serde_json::json!({}))]
    };
    let path = purlis_core::machine::dir(config).join(purlis_core::machine::RUNNERS_FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    write_private(
        &path,
        &serde_json::json!({
            "version": 1,
            "pools": [{ "name": OLD, "kind": "docker", "image": IMAGE }],
            "runners": runners,
        })
        .to_string(),
    );
}

/// `text` at `path`, as this machine keeps its runners: readable by this user alone.
fn write_private(path: &Path, text: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

/// A window whose host relays pool `builds` to one of two runner hosts: the old until `followed`
/// says so, as `purlis_core::runners::reachable` answers, and then the old drains.
struct Pool {
    connection: Arc<Connection>,
    old: StandIns,
    new: StandIns,
    followed: Arc<std::sync::atomic::AtomicBool>,
    config: tempfile::TempDir,
    relays: Arc<Mutex<Vec<String>>>,
}

impl Pool {
    fn on(runtime: &tokio::runtime::Runtime) -> Pool {
        let (old, new) = (StandIns::default(), StandIns::default());
        let host_of = |stand_ins: &StandIns| {
            Hosting::confining(
                ComposesConfined,
                StandInConfines::default(),
                stand_ins.clone(),
            )
        };
        let (old_host, new_host) = runtime.block_on(async { (host_of(&old), host_of(&new)) });
        let followed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let relays = Arc::<Mutex<Vec<String>>>::default();
        let (desktop, runner) = (
            Arc::new(KeptKey::new("01JA0DESKT0P00000000000000")),
            Arc::new(KeptKey::new("01JB0RUNNER0000000000000000")),
        );
        let pinned = runner.pinned();
        let to_old = ToTheRunner {
            runner_host: old_host,
            runner: Arc::clone(&runner),
            desktop: desktop.pinned(),
        };
        let to_new = ToTheRunner {
            runner_host: new_host,
            runner,
            desktop: desktop.pinned(),
        };
        let looks: purlis_host::RelaysTo = Arc::new({
            let (followed, relays) = (Arc::clone(&followed), Arc::clone(&relays));
            move |name: &str| {
                relays.lock().unwrap().push(name.to_owned());
                let now = followed.load(Ordering::SeqCst);
                let (connector, runner) = if now {
                    (to_new.clone(), NEW)
                } else {
                    (to_old.clone(), OLD)
                };
                let (desktop, pinned) = (Arc::clone(&desktop), pinned.clone());
                Ok((
                    Arc::new(move || {
                        purlis_session_protocol::hostlink::keep_linked(
                            connector.clone(),
                            purlis_session_protocol::hostlink::Handshake::THIS_BUILD,
                            desktop.identity(),
                            pinned.clone(),
                            purlis_session_protocol::hostlink::Liveness::default(),
                        )
                    }) as purlis_host::Keep,
                    format!("{OLD}/{runner}"),
                ))
            }
        });
        let drains: purlis_host::DrainsTo = Arc::new({
            let followed = Arc::clone(&followed);
            move |name: &str| name == "builds/builds" && followed.load(Ordering::SeqCst)
        });
        let desktop_host = runtime.block_on(async {
            Hosting::running_with(
                crate::sessionhost::Composes::as_the_tests_ship(),
                StandIns::default(),
            )
            .relaying_with(looks)
            .draining_with(drains)
        });
        let handle = runtime.handle().clone();
        let window = Arc::new(
            runtime
                .block_on(a_link_to(desktop_host.clone(), || {}))
                .expect("the window speaks to its host"),
        );
        let dial: Dial = Arc::new({
            let (handle, host) = (handle.clone(), desktop_host.clone());
            move |within, naming| {
                open_within(&handle, within, a_named_link_to(host.clone(), naming))
            }
        });
        let relayed: RelayedDial = Arc::new(move |within, naming, pool: &str| {
            let (host, pool) = (desktop_host.clone(), pool.to_owned());
            open_within(&handle, within, async move {
                let link = a_bare_link_to(purlis_host::Routed::new(host), || {}).await?;
                HostLink::over_relayed(
                    link,
                    tokio::runtime::Handle::current(),
                    || {},
                    naming,
                    Some(&pool),
                )
                .await
            })
        });
        let connection = Arc::new(Connection::default());
        let config = tempfile::tempdir().unwrap();
        keep_runners(config.path(), false);
        connection.runners_in(config.path().to_path_buf());
        connection.naming_in(config.path().to_path_buf());
        connection.ready_relaying(window, dial, relayed);
        Pool {
            connection,
            old,
            new,
            followed,
            config,
            relays,
        }
    }

    /// An update is followed: a new runner takes the pool's new chats, and the old one drains.
    fn follows(&self) {
        keep_runners(self.config.path(), true);
        self.followed.store(true, Ordering::SeqCst);
    }
}

fn started(sessions: &HostedSessions) -> Result<u32, String> {
    sessions
        .open_named(None, &by_its_names(), Default::default(), &|_| {})
        .map(|composed| composed.number)
}

/// **A start while the pool's runner drains just works** (#4248): it runs on the pool's runner now,
/// under the window's one name for the link, and the chat that stayed on the old runner is typed
/// into, resized and closed over the link it was started on.
#[test]
fn a_start_during_a_drain_runs_on_the_pools_runner_and_the_old_chat_stays_reachable() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let (_dir, root) = a_project();
    let sessions = HostedSessions::on_a_runner(pool.connection.to_a_pool("builds"), &root);

    let stayed = started(&sessions).expect("it starts on the runner");
    until("the runner runs it", || pool.old.ran().len() == 1);

    pool.follows();
    let moved_on = started(&sessions)
        .unwrap_or_else(|why| panic!("a start during a drain just works, and it said: {why}"));
    until("the pool's runner now runs it", || {
        pool.new.ran().len() == 1
    });
    assert_eq!(
        pool.old.ran().len(),
        1,
        "the draining runner starts no chat"
    );
    assert_eq!(*pool.relays.lock().unwrap(), ["builds", "builds"]);

    sessions
        .input(stayed, b"still there\n")
        .expect("typed into");
    sessions
        .resize(
            stayed,
            Size {
                columns: 90,
                rows: 30,
            },
        )
        .expect("resized");
    sessions.input(moved_on, b"here\n").expect("typed into");
    assert_eq!(sessions.stop_every_program(), 2, "both are reached");
    sessions.close(stayed).expect("it closes on the old runner");
    sessions.close(moved_on).expect("it closes on the new one");
}

/// **Once the draining runner is kept no more its link goes**, and nothing else does (#4248).
#[test]
fn a_link_left_on_a_draining_runner_ends_once_the_runner_is_kept_no_more() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let (_dir, root) = a_project();
    let to_it = pool.connection.to_a_pool("builds");
    let sessions = HostedSessions::on_a_runner(Arc::clone(&to_it), &root);
    let stayed = started(&sessions).expect("it starts");
    pool.follows();
    started(&sessions).expect("it starts");
    assert_eq!(to_it.links_of(&root).len(), 2);
    sessions.close(stayed).expect("it closes");

    let path =
        purlis_core::machine::dir(pool.config.path()).join(purlis_core::machine::RUNNERS_FILE);
    let mut kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    kept["runners"].as_array_mut().unwrap().remove(0);
    write_private(&path, &kept.to_string());

    to_it.of_project(&root, Duration::from_secs(5)).unwrap();
    assert_eq!(to_it.links_of(&root).len(), 1, "only the pool's runner's");
}

/// A host that answers a program's start only after `after`, and counts what it closes.
#[derive(Clone)]
struct SlowToStart<H> {
    inner: H,
    after: Duration,
    closed: Arc<Mutex<Vec<String>>>,
}

impl<H: session::Host + Clone + Send + Sync + 'static> session::Host for SlowToStart<H> {
    async fn call(
        &self,
        command: session::Command,
        peer: &session::Peer,
    ) -> Result<Answer, session::Refusal> {
        match &command {
            session::Command::StartProgram(_) => tokio::time::sleep(self.after).await,
            session::Command::Close { program } => {
                self.closed.lock().unwrap().push(program.clone());
            }
            _ => {}
        }
        self.inner.call(command, peer).await
    }

    async fn link_ended(&self, peer: &session::Peer) {
        self.inner.link_ended(peer).await;
    }

    async fn subscribe(
        &self,
        since: session::Since,
        peer: &session::Peer,
    ) -> Result<tokio::sync::mpsc::Receiver<session::Pushed>, session::Refusal> {
        self.inner.subscribe(since, peer).await
    }
}

/// A window whose project links reach `host`, each start waiting at most `within`.
fn a_window_waiting(
    host: SlowToStart<Hosting>,
    within: Option<Duration>,
    runtime: &tokio::runtime::Runtime,
) -> Arc<Connection> {
    let window = Arc::new(
        runtime
            .block_on(a_link_to(host.clone(), || {}))
            .expect("the window speaks to the host"),
    );
    let handle = runtime.handle().clone();
    let dial: Dial = Arc::new(move |_, naming| {
        let host = host.clone();
        open_within(&handle, Duration::from_secs(10), async move {
            let link = a_named_link_to(host, naming).await?;
            Ok(match within {
                Some(within) => link.starts_waiting(within),
                None => link,
            })
        })
    });
    let connection = Arc::new(Connection::default());
    connection.ready(window, dial);
    connection
}

fn a_host_slow_by(after: Duration, runtime: &tokio::runtime::Runtime) -> SlowToStart<Hosting> {
    SlowToStart {
        inner: runtime.block_on(async {
            Hosting::confining(
                ComposesConfined,
                StandInConfines::default(),
                StandIns::default(),
            )
        }),
        after,
        closed: Arc::default(),
    }
}

/// **A start the host answers after a quick command's limit runs** (#4264): the person's Start was
/// failed at [`ANSWER_WITHIN`] though the host was only slow. Waits that long, for real.
#[test]
fn a_start_slower_than_a_quick_commands_limit_still_runs() {
    let runtime = a_runtime();
    let host = a_host_slow_by(ANSWER_WITHIN + Duration::from_secs(1), &runtime);
    let connection = a_window_waiting(host, None, &runtime);
    let (_dir, root) = a_project();
    let sessions = HostedSessions::new(connection, &root);

    started(&sessions).unwrap_or_else(|why| panic!("a slow host is waited for: {why}"));
}

/// **A start that outlasts even a start's limit says what to do, and the chat that comes up after
/// is closed** (#4264): none runs that the person was told did not start.
#[test]
fn a_start_past_its_limit_says_what_to_do_and_a_late_chat_is_closed() {
    let runtime = a_runtime();
    let host = a_host_slow_by(Duration::from_millis(800), &runtime);
    let closed = Arc::clone(&host.closed);
    let connection = a_window_waiting(host, Some(Duration::from_millis(100)), &runtime);
    let (_dir, root) = a_project();
    let sessions = HostedSessions::new(connection, &root);

    let said = started(&sessions).expect_err("it stopped waiting");
    assert_eq!(said, START_NOT_IN_TIME);
    until("the chat that came up late is closed", || {
        closed.lock().unwrap().len() == 1
    });
}
