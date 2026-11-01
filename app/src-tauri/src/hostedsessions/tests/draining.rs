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
    desktop_host: Hosting,
    handle: tokio::runtime::Handle,
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
                // A draining runner is reached by its own name, whatever the pool's runner is.
                let now = followed.load(Ordering::SeqCst) && !name.contains('/');
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
        let config = tempfile::tempdir().unwrap();
        keep_runners(config.path(), false);
        let connection = a_window(&desktop_host, &handle, config.path(), runtime);
        Pool {
            connection,
            old,
            new,
            followed,
            config,
            relays,
            desktop_host,
            handle,
        }
    }

    /// The window starts again: a connection of its own, on the host that kept running, by the
    /// names it keeps. Its old links end.
    fn window_restarts(&self, runtime: &tokio::runtime::Runtime) -> Arc<Connection> {
        self.connection.links_to_pool_end("builds");
        a_window(
            &self.desktop_host,
            &self.handle,
            self.config.path(),
            runtime,
        )
    }

    /// An update is followed: a new runner takes the pool's new chats, and the old one drains.
    fn follows(&self) {
        keep_runners(self.config.path(), true);
        self.followed.store(true, Ordering::SeqCst);
    }
}

/// A window on `desktop_host`, keeping its runners and link names in `config`.
struct RefusesItsPrograms<H> {
    inner: H,
    refused: bool,
}

impl<H: session::Host + Clone + Send + Sync + 'static> session::Host for RefusesItsPrograms<H> {
    async fn call(
        &self,
        command: session::Command,
        peer: &session::Peer,
    ) -> Result<Answer, session::Refusal> {
        if self.refused && matches!(command, session::Command::ProgramsOf { .. }) {
            return Err(session::Refusal {
                code: session::NOT_ALLOWED.into(),
                why: "this runner does not say".into(),
            });
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

fn a_window(
    desktop_host: &Hosting,
    handle: &tokio::runtime::Handle,
    config: &Path,
    runtime: &tokio::runtime::Runtime,
) -> Arc<Connection> {
    a_window_told(desktop_host, handle, config, runtime, false)
}

/// [`a_window`], its relayed links refused `programs_of` where `refused`, as a runner that does
/// not say its programs refuses it.
fn a_window_told(
    desktop_host: &Hosting,
    handle: &tokio::runtime::Handle,
    config: &Path,
    runtime: &tokio::runtime::Runtime,
    refused: bool,
) -> Arc<Connection> {
    let window = Arc::new(
        runtime
            .block_on(a_link_to(desktop_host.clone(), || {}))
            .expect("the window speaks to its host"),
    );
    let dial: Dial = Arc::new({
        let (handle, host) = (handle.clone(), desktop_host.clone());
        move |within, naming| open_within(&handle, within, a_named_link_to(host.clone(), naming))
    });
    let relayed: RelayedDial = Arc::new({
        let (handle, host) = (handle.clone(), desktop_host.clone());
        move |within, naming, pool: &str| {
            let (host, pool) = (host.clone(), pool.to_owned());
            open_within(&handle, within, async move {
                let host = RefusesItsPrograms {
                    inner: purlis_host::Routed::new(host),
                    refused,
                };
                let link = a_bare_link_to(host, || {}).await?;
                HostLink::over_relayed(
                    link,
                    tokio::runtime::Handle::current(),
                    || {},
                    naming,
                    Some(&pool),
                )
                .await
            })
        }
    });
    let connection = Arc::new(Connection::default());
    connection.runners_in(config.to_path_buf());
    connection.naming_in(config.to_path_buf());
    connection.ready_relaying(window, dial, relayed);
    connection
}

fn started(sessions: &dyn SessionHost) -> Result<u32, String> {
    sessions
        .open_named(None, &by_its_names(), Default::default(), &|_| {})
        .map(|composed| composed.number)
}

/// The project's chats, every start on pool `builds`, as the window routes them.
fn routed(pool: &Pool, root: &Path) -> crate::routedsessions::RoutedSessions {
    routed_on(&pool.connection, root)
}

fn routed_on(window: &Arc<Connection>, root: &Path) -> crate::routedsessions::RoutedSessions {
    let connection = Arc::clone(window);
    let place = root.to_path_buf();
    crate::routedsessions::RoutedSessions::new(
        root,
        Box::new(HostedSessions::new(Arc::clone(window), root)),
        Arc::new(|_| Ok(Some("builds".to_owned()))),
        Arc::new(Default::default),
        Arc::new(move |name| {
            Ok(Box::new(HostedSessions::on_a_runner(
                connection.to_a_pool(name),
                &place,
            )) as Box<dyn SessionHost>)
        }),
    )
    .every_start_on("builds")
}

/// **A start while the pool's runner drains just works** (#4248): it runs on the pool's runner now,
/// under the window's one name for the link, and the chat that stayed on the old runner is typed
/// into, resized and closed over the link it was started on. Its end comes to the window once.
#[test]
fn a_start_during_a_drain_runs_on_the_pools_runner_and_the_old_chat_stays_reachable() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let (_dir, root) = a_project();
    let sessions = routed(&pool, &root);

    let stayed = started(&sessions).expect("it starts on the runner");
    until("the runner runs it", || pool.old.ran().len() == 1);

    pool.follows();
    let moved_on = started(&sessions)
        .unwrap_or_else(|why| panic!("a start during a drain just works, and it said: {why}"));
    assert_ne!(stayed, moved_on, "one numbering across both runners");
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

/// **A window that restarts during a drain takes back the chat on the draining runner** (#4271),
/// on a link of its own under the name it always kept, and the one on the pool's runner as before.
/// Each is reached where it runs, and no new chat goes to the draining runner.
#[test]
fn a_window_that_restarts_during_a_drain_takes_back_both_runners_chats() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let (_dir, root) = a_project();
    let before = routed(&pool, &root);
    let stayed = started(&before).expect("it starts on the runner");
    pool.follows();
    let moved_on = started(&before).expect("it starts on the pool's runner");
    until("both run", || {
        pool.old.ran().len() == 1 && pool.new.ran().len() == 1
    });

    let window = pool.window_restarts(&runtime);
    let sessions = routed_on(&window, &root)
        .every_start_on("builds")
        .taking_back_from(Arc::new(|_| vec![format!("{OLD}/{OLD}")]));
    until("the pool's runner gives its chat back", || {
        sessions.left_running_in(moved_on, None)
    });
    until("the draining runner gives its chat back", || {
        sessions.left_running_in(stayed, None)
    });
    assert!(
        pool.relays
            .lock()
            .unwrap()
            .iter()
            .any(|name| name == "builds/builds")
    );
    assert!(
        !sessions.left_running_in(stayed + moved_on + 7, None),
        "neither has a chat it did not start"
    );

    sessions.reattach(stayed, &|_| {}).expect("taken back");
    sessions.reattach(moved_on, &|_| {}).expect("taken back");
    sessions
        .input(stayed, b"still there\n")
        .expect("typed into");
    sessions.input(moved_on, b"here\n").expect("typed into");
    assert_eq!(pool.old.ran().len(), 1, "no chat started again");
    assert_eq!(pool.new.ran().len(), 1, "no chat started again");

    started(&sessions).expect("a new chat starts on the pool's runner");
    until("it runs there", || pool.new.ran().len() == 2);
    assert_eq!(
        pool.old.ran().len(),
        1,
        "the draining runner starts no chat"
    );
    assert_eq!(sessions.pool_of(stayed).as_deref(), Some(OLD));
}

/// **A draining runner's log is read beside the pool's runner's** (#4271), said once for the pool.
#[test]
fn a_pools_log_is_read_from_each_link_still_open() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let (_dir, root) = a_project();
    let sessions = routed(&pool, &root);
    started(&sessions).expect("it starts");
    pool.follows();
    started(&sessions).expect("it starts");

    let logs = sessions.runner_logs();

    assert_eq!(logs.len(), 1, "one line for the pool: {logs:?}");
    assert_eq!(logs[0].0, OLD);
    assert!(!logs[0].1.down, "{logs:?}");

    // Only the draining runner's link is said to be down, so only reading it says so.
    let links = pool.connection.to_a_pool("builds").links_of(&root);
    let draining = links
        .iter()
        .find(|link| link.reached_runner() == Some(OLD))
        .expect("its link is kept");
    lock(&draining.shared.runner_log).link(true);
    assert!(sessions.runner_logs()[0].1.down);
}

/// **The window keeps one name for a pool** (#4271): a link to a draining runner takes back only
/// what it started under the pool's name, and a name of its own would take back nothing.
#[test]
fn a_draining_runner_is_linked_to_under_its_pools_name() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let (_dir, root) = a_project();
    let names = |of: &str| {
        purlis_core::windownames::name_for(
            pool.config.path(),
            &root.join(RELAYED_NAMES).join(of),
            &[],
        )
        .unwrap()
        .expose()
        .to_owned()
    };
    let named = pool
        .connection
        .to_a_pool("builds/builds")
        .name_for(&root)
        .expect("it names its links");
    assert_eq!(named.expose(), names("builds"));
}

/// **A chat on a draining runner taken back counts against its pool** (#4271): the window's mark
/// that a chat ran without the sandbox is the pool's, never a name of `<pool>/<runner>`.
#[test]
fn a_draining_runners_link_marks_its_pool_untrusted() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    keep_runners(pool.config.path(), true);
    let to_it = pool.connection.to_a_pool("builds/builds");

    to_it.ran_unsandboxed().unwrap();

    assert!(pool.connection.untrusts("builds"));
    assert!(purlis_core::runnertaint::pool_ran_unsandboxed(
        pool.config.path(),
        "builds"
    ));
}

/// **A runner of this protocol that does not say its programs marks its pool** (#4271): a refusal
/// says nothing of what runs there. An older runner is not asked (the relay's own test).
#[test]
fn a_runner_that_does_not_say_its_programs_marks_its_pool() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let window = a_window_told(
        &pool.desktop_host,
        &pool.handle,
        pool.config.path(),
        &runtime,
        true,
    );
    let (_project, root) = a_project();

    let _ = started(&routed_on(&window, &root));

    assert!(purlis_core::runnertaint::pool_ran_unsandboxed(
        pool.config.path(),
        "builds"
    ));
}

/// **A pool's removal ends a draining runner's links too** (#4271), and a link to a runner kept no
/// more ends as it goes.
#[test]
fn a_link_taken_back_ends_with_its_runner_and_with_its_pool() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let (_dir, root) = a_project();
    let before = routed(&pool, &root);
    started(&before).expect("it starts");
    pool.follows();
    let window = pool.window_restarts(&runtime);
    let to_it = window.to_a_pool("builds/builds");
    let held = until_held(&to_it, &root);
    assert_eq!(held.reached_runner(), Some(OLD));

    window.links_to_gone_runners_end(pool.config.path());
    assert_eq!(to_it.links_of(&root).len(), 1, "it is still kept");

    let path =
        purlis_core::machine::dir(pool.config.path()).join(purlis_core::machine::RUNNERS_FILE);
    let mut kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    kept["runners"].as_array_mut().unwrap().remove(0);
    write_private(&path, &kept.to_string());
    window.links_to_gone_runners_end(pool.config.path());
    assert_eq!(to_it.links_of(&root).len(), 0, "it went with its runner");

    keep_runners(pool.config.path(), true);
    until_held(&to_it, &root);
    window.links_to_pool_end("builds");
    assert_eq!(to_it.links_of(&root).len(), 0, "it went with its pool");
}

/// The link `to_it` holds `root` on, once a window's old one has let go of its name.
fn until_held(to_it: &Arc<Connection>, root: &Path) -> Arc<HostLink> {
    let held = Mutex::new(None);
    until("the runner is linked to", || {
        *held.lock().unwrap() = to_it.of_project(root, Duration::from_secs(5)).ok();
        held.lock().unwrap().is_some()
    });
    held.into_inner().unwrap().expect("it is held")
}

/// **While a runner of the pool drains no fetch removes what only the other has** (#4271): after a
/// restart the pool's runner has a link of its own and nothing else to say so.
#[test]
fn a_pool_drains_as_its_runners_are_kept() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let to_it = pool.connection.to_a_pool("builds");
    assert!(!to_it.pool_drains());
    keep_runners(pool.config.path(), true);
    assert!(to_it.pool_drains());
    assert!(pool.connection.to_a_pool("builds/builds").pool_drains());
    assert!(!pool.connection.pool_drains(), "this machine's host's");
}

/// **A runner made again in its place leaves the link naming one kept no more** (#4149, #4248): the
/// desktop's host refuses a start on such a link, so the next start goes to the pool's runner.
#[test]
fn a_start_after_a_runner_was_made_again_runs_on_the_new_one() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let (_dir, root) = a_project();
    let sessions = routed(&pool, &root);
    started(&sessions).expect("it starts");
    pool.follows();
    let path =
        purlis_core::machine::dir(pool.config.path()).join(purlis_core::machine::RUNNERS_FILE);
    let mut kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    kept["runners"].as_array_mut().unwrap().remove(0);
    write_private(&path, &kept.to_string());

    started(&sessions).unwrap_or_else(|why| panic!("it starts on the new runner: {why}"));
    until("the new runner runs it", || pool.new.ran().len() == 1);
}

/// **Only a start moves on** (#4248): a look at the project's link leaves it where it is, so a
/// fetch while the runner drains still reaches the chats' work.
#[test]
fn nothing_but_a_start_moves_the_project_off_a_draining_runner() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let (_dir, root) = a_project();
    let to_it = pool.connection.to_a_pool("builds");
    let sessions = routed(&pool, &root);
    started(&sessions).expect("it starts");
    pool.follows();
    let link = to_it.of_project(&root, Duration::from_secs(5)).unwrap();
    assert_eq!(link.reached_runner(), Some(OLD));
    assert_eq!(pool.relays.lock().unwrap().len(), 1);
}

/// **Once the draining runner is kept no more its link goes**, and nothing else does (#4248).
#[test]
fn a_link_left_on_a_draining_runner_ends_once_the_runner_is_kept_no_more() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let (_dir, root) = a_project();
    let to_it = pool.connection.to_a_pool("builds");
    let sessions = routed(&pool, &root);
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

    started(&sessions).expect("it starts");
    assert_eq!(to_it.links_of(&root).len(), 1, "only the pool's runner's");
}

/// **A runner kept no more leaves no link behind, without a start to notice** (#4168): the chats
/// on it are told they ended, rather than staying dimmed.
#[test]
fn links_to_a_runner_kept_no_more_end_as_it_goes() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let (_dir, root) = a_project();
    let to_it = pool.connection.to_a_pool("builds");
    let sessions = routed(&pool, &root);
    let on_the_old = started(&sessions).expect("it starts");
    pool.follows();
    started(&sessions).expect("it starts");
    assert_eq!(to_it.links_of(&root).len(), 2);

    pool.connection
        .links_to_gone_runners_end(pool.config.path());
    assert_eq!(to_it.links_of(&root).len(), 2, "both are still kept");
    assert!(!sessions.lost_with_its_runner(on_the_old));

    let path =
        purlis_core::machine::dir(pool.config.path()).join(purlis_core::machine::RUNNERS_FILE);
    let mut kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    kept["runners"].as_array_mut().unwrap().remove(0);
    write_private(&path, &kept.to_string());

    pool.connection
        .links_to_gone_runners_end(pool.config.path());

    assert_eq!(to_it.links_of(&root).len(), 1, "only the pool's runner's");
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while !sessions.lost_with_its_runner(on_the_old) && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(sessions.lost_with_its_runner(on_the_old), "#4350");
}

/// **A chat on a removed pool's runner ended with that runner** (#4350), and not merely with its
/// host: its conversation went with the runner's home.
#[test]
fn a_chat_of_a_removed_pool_ended_with_its_runner() {
    let runtime = a_runtime();
    let pool = Pool::on(&runtime);
    let (_dir, root) = a_project();
    let sessions = routed(&pool, &root);
    let chat = started(&sessions).expect("it starts");

    assert!(!sessions.lost_with_its_runner(chat));
    pool.connection.pool_removed("builds");

    let until = std::time::Instant::now() + Duration::from_secs(10);
    while !sessions.lost_with_its_host(chat) && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(sessions.lost_with_its_host(chat), "its link ended");
    assert!(sessions.lost_with_its_runner(chat));
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

/// A window whose project links reach `host`.
fn a_window_on(host: SlowToStart<Hosting>, runtime: &tokio::runtime::Runtime) -> Arc<Connection> {
    let window = Arc::new(
        runtime
            .block_on(a_link_to(host.clone(), || {}))
            .expect("the window speaks to the host"),
    );
    let handle = runtime.handle().clone();
    let dial: Dial = Arc::new(move |within, naming| {
        open_within(&handle, within, a_named_link_to(host.clone(), naming))
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
/// failed at [`ANSWER_WITHIN`] though the host was only slow. A start has no limit of its own.
/// Waits that long, for real.
#[test]
fn a_start_slower_than_a_quick_commands_limit_still_runs() {
    let runtime = a_runtime();
    let host = a_host_slow_by(ANSWER_WITHIN + Duration::from_secs(1), &runtime);
    let connection = a_window_on(host, &runtime);
    let (_dir, root) = a_project();
    let sessions = HostedSessions::new(connection, &root);

    started(&sessions).unwrap_or_else(|why| panic!("a slow host is waited for: {why}"));
}

/// **The person's Cancel ends the wait, and the chat that comes up after is closed** (#4264).
#[test]
fn a_cancelled_start_stops_waiting_and_a_late_chat_is_closed() {
    let runtime = a_runtime();
    let host = a_host_slow_by(Duration::from_millis(800), &runtime);
    let closed = Arc::clone(&host.closed);
    let connection = a_window_on(host, &runtime);
    let (_dir, root) = a_project();
    let sessions = HostedSessions::new(connection, &root);

    let cancel = a_cancellable_start(&root);
    let cancelling = {
        let root = root.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            cancel_starts(&root);
        })
    };
    let said = cancelled_by(cancel, || started(&sessions)).expect_err("it stopped waiting");
    cancelling.join().unwrap();
    assert_eq!(said, START_CANCELLED);
    until("the chat that came up late is closed", || {
        closed.lock().unwrap().len() == 1
    });
}
