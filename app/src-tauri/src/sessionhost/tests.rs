// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::*;

#[test]
fn the_window_starts_its_own_binary_as_its_session_host_holding_its_input() {
    let command = command(Path::new(
        "/Applications/purlis.app/Contents/MacOS/purlis-app",
    ));
    assert_eq!(
        command.get_program(),
        "/Applications/purlis.app/Contents/MacOS/purlis-app"
    );
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(args, ["purlis-serve-for-the-window"]);
}

#[test]
fn a_host_that_found_another_serving_leaves_the_window_to_reattach_to_that_one() {
    assert_eq!(not_ready(Some(3)), NotReady::AnotherServes);
    for code in [None, Some(0), Some(1)] {
        assert_eq!(
            not_ready(code),
            NotReady::Gone("the session host ended before it was ready"),
            "{code:?}"
        );
    }
}

// ----- the supervisor and its breaker (#2007) ---------------------------------------------

/// A host that ends at once, as one that crashes on its way up does, writing a line to its
/// log and a line to `started` each time it is started.
fn a_host_that_crashes_as_it_starts(
    started: &Path,
) -> impl Fn() -> Result<Command, String> + Send + Sync + use<> {
    let started = started.to_path_buf();
    move || {
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("echo once >> \"$1\"; echo 'purlisd: it went wrong' >&2; exit 1")
            .arg("sh")
            .arg(&started)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Ok(command)
    }
}

/// A place of a host's own, in `home`.
fn a_place(home: &Path) -> purlis_host::Place {
    let run = home.join("run");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::set_permissions(&run, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    purlis_host::Place::at(run.join("purlisd.sock"), home.join("credentials"))
}

fn starts_in(started: &Path) -> usize {
    std::fs::read_to_string(started)
        .map(|said| said.lines().count())
        .unwrap_or(0)
}

fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_host_that_keeps_ending_is_started_again_until_three_exits_trip_the_breaker() {
    let home = tempfile::tempdir().unwrap();
    let started = home.path().join("started");
    let host = SessionHost::default();
    let failing = host.failing();

    host.start_at(
        a_place(home.path()),
        a_host_that_crashes_as_it_starts(&started),
    );

    until("the breaker trips", || failing.now().is_some());
    assert_eq!(
        starts_in(&started),
        breaker::EXITS,
        "started once more after each exit"
    );
    let said = failing.now().unwrap();
    assert_eq!((said.exits, said.within_minutes), (3, 5));
    assert!(
        said.said
            .iter()
            .any(|line| line == "purlisd: it went wrong"),
        "{:?}",
        said.said
    );
    let refused = host
        .connection()
        .wait(Duration::ZERO)
        .err()
        .unwrap_or_default();
    assert!(refused.contains("stopped starting it again"), "{refused}");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        starts_in(&started),
        breaker::EXITS,
        "started after the breaker tripped"
    );
    host.end();
}

#[test]
fn try_again_starts_a_failing_host_once_more_with_the_count_started_over() {
    let home = tempfile::tempdir().unwrap();
    let started = home.path().join("started");
    let host = SessionHost::default();
    let failing = host.failing();
    host.start_at(
        a_place(home.path()),
        a_host_that_crashes_as_it_starts(&started),
    );
    until("the breaker trips", || failing.now().is_some());

    failing.try_again().expect("tried again");

    until("it trips again", || {
        starts_in(&started) == 2 * breaker::EXITS && failing.now().is_some()
    });
    host.end();
}

#[test]
fn a_host_the_window_ends_is_never_started_again() {
    let home = tempfile::tempdir().unwrap();
    let started = home.path().join("started");
    let host = SessionHost::default();
    host.end();

    host.start_at(
        a_place(home.path()),
        a_host_that_crashes_as_it_starts(&started),
    );
    std::thread::sleep(Duration::from_millis(500));

    assert_eq!(starts_in(&started), 0);
}

/// A host that finds another serving leaves the window to reattach to that one (#2006), never
/// to be started again while its link holds. One that cannot be reached counts as an
/// unexpected end, so a host that is never reached trips the breaker rather than looping.
#[test]
fn a_host_that_found_another_serving_is_reattached_to_and_one_never_reached_trips_the_breaker() {
    let home = tempfile::tempdir().unwrap();
    let started = home.path().join("started");
    let host = SessionHost::default();
    let failing = host.failing();
    let another_serves = {
        let started = started.clone();
        move || {
            let mut command = Command::new("/bin/sh");
            command
                .arg("-c")
                .arg("echo once >> \"$1\"; exit 3")
                .arg("sh")
                .arg(&started)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            Ok(command)
        }
    };

    host.start_at(a_place(home.path()), another_serves);
    until("the breaker trips", || failing.now().is_some());

    assert_eq!(starts_in(&started), breaker::EXITS);
    host.end();
}

#[test]
fn a_host_started_as_the_window_ends_it_is_ended_and_not_left_running() {
    // The window's end lands after the supervisor looked and before the host it is starting is
    // held: the end finds nothing to end, so the start must end it.
    let home = tempfile::tempdir().unwrap();
    let pids = home.path().join("pids");
    let host = SessionHost::default();
    let ender = host.ender();
    let starts = {
        let pids = pids.clone();
        move || {
            ender();
            let mut command = Command::new("/bin/sh");
            command
                .arg("-c")
                .arg("echo $$ >> \"$1\"; echo ready; exec cat")
                .arg("sh")
                .arg(&pids)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            Ok(command)
        }
    };

    host.start_at(a_place(home.path()), starts);

    until("the host is started", || starts_in(&pids) == 1);
    let pid: u32 = std::fs::read_to_string(&pids)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    until("the host it started is ended", || {
        !purlis_core::process::alive(pid)
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        starts_in(&pids),
        1,
        "started again after the window ended it"
    );
}

// ----- the service manager runs the host (FD-17, #655) ------------------------------------

/// A service manager that runs nothing: what the window asked of it, and whether its own limit
/// tripped.
#[derive(Default)]
struct AManager {
    asked: Mutex<Vec<&'static str>>,
    tripped: std::sync::atomic::AtomicBool,
    /// Install fails.
    refuses: bool,
}

impl AManager {
    /// A manager whose install fails, as launchd's does with purlis's background item
    /// switched off in System Settings.
    fn refusing() -> AManager {
        AManager {
            refuses: true,
            ..AManager::default()
        }
    }

    fn asked(&self) -> Vec<&'static str> {
        self.asked.lock().unwrap().clone()
    }

    fn ask(&self, what: &'static str) -> Result<(), String> {
        self.asked.lock().unwrap().push(what);
        Ok(())
    }
}

impl purlis_core::loginitem::Manager for AManager {
    fn install(&self) -> Result<(), String> {
        self.ask("install")?;
        if self.refuses {
            return Err("`launchctl bootstrap gui/501 …` failed: Input/output error".into());
        }
        Ok(())
    }
    fn start(&self) -> Result<(), String> {
        self.ask("start")
    }
    fn hold(&self) -> Result<(), String> {
        self.ask("hold")
    }
    fn tripped(&self) -> bool {
        self.tripped.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn try_again(&self) -> Result<(), String> {
        self.ask("try again")
    }
    fn remove(&self) -> Result<(), String> {
        self.ask("remove")
    }
}

/// The window's own start of a host, which a test of the manager's host never reaches.
fn a_host_the_window_must_not_start() -> impl Fn() -> Result<Command, String> + Send + Sync + use<>
{
    || Err("the window started a host of its own".into())
}

/// How long the window waits for a manager's host to answer, here: none ever does.
const NEVER_ANSWERS_WITHIN: Duration = Duration::from_millis(50);

#[test]
fn a_host_the_service_manager_runs_is_started_by_the_manager_and_never_by_the_window() {
    let home = tempfile::tempdir().unwrap();
    let manager = Arc::new(AManager::default());
    let host = SessionHost::default();
    let failing = host.failing();

    host.start_managed_at(
        a_place(home.path()),
        Arc::clone(&manager) as Arc<dyn purlis_core::loginitem::Manager>,
        None,
        NEVER_ANSWERS_WITHIN,
        a_host_the_window_must_not_start(),
    );

    until("the window's breaker trips", || failing.now().is_some());
    assert_eq!(
        failing.now().unwrap().stopped_by,
        crate::hostfailing::StoppedBy::Purlis
    );
    assert!(host.is_managed());
    assert!(
        host.held.lock().unwrap().is_none(),
        "the window started no host"
    );
    assert_eq!(
        manager.asked(),
        [
            "install", "start", "install", "start", "install", "start", "hold"
        ],
        "asked to start it after each end, and to stop once three tripped the breaker"
    );
    host.end();
}

#[test]
fn a_service_managers_own_tripped_limit_raises_the_failing_notice_at_once() {
    let home = tempfile::tempdir().unwrap();
    let manager = Arc::new(AManager::default());
    manager
        .tripped
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let host = SessionHost::default();
    let failing = host.failing();

    host.start_managed_at(
        a_place(home.path()),
        Arc::clone(&manager) as Arc<dyn purlis_core::loginitem::Manager>,
        None,
        NEVER_ANSWERS_WITHIN,
        a_host_the_window_must_not_start(),
    );

    until("the notice is raised", || failing.now().is_some());
    assert_eq!(manager.asked(), ["install", "start", "hold"]);
    let said = failing.now().unwrap();
    assert_eq!((said.exits, said.within_minutes), (3, 5));
    assert_eq!(
        said.stopped_by,
        crate::hostfailing::StoppedBy::System,
        "the Notice says the system stopped starting it, not purlis"
    );
    host.end();
}

#[test]
fn try_again_lets_the_manager_go_of_its_hold_and_starts_the_host_once_more() {
    let home = tempfile::tempdir().unwrap();
    let manager = Arc::new(AManager::default());
    manager
        .tripped
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let host = SessionHost::default();
    let failing = host.failing();
    host.start_managed_at(
        a_place(home.path()),
        Arc::clone(&manager) as Arc<dyn purlis_core::loginitem::Manager>,
        None,
        NEVER_ANSWERS_WITHIN,
        a_host_the_window_must_not_start(),
    );
    until("the notice is raised", || failing.now().is_some());

    failing.try_again().expect("tried again");

    until("it is started once more", || {
        manager
            .asked()
            .iter()
            .filter(|asked| **asked == "start")
            .count()
            == 2
    });
    assert!(manager.asked().contains(&"try again"));
    host.end();
}

#[test]
fn the_failing_notice_shows_the_last_lines_the_managers_host_wrote_to_its_log() {
    let home = tempfile::tempdir().unwrap();
    let log = home.path().join("session-host.log");
    let lines: Vec<String> = (0..30).map(|n| format!("purlisd: line {n}")).collect();
    std::fs::write(&log, lines.join("\n") + "\n").unwrap();
    let manager = Arc::new(AManager::default());
    manager
        .tripped
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let host = SessionHost::default();
    let failing = host.failing();

    host.start_managed_at(
        a_place(home.path()),
        manager as Arc<dyn purlis_core::loginitem::Manager>,
        Some(log),
        NEVER_ANSWERS_WITHIN,
        a_host_the_window_must_not_start(),
    );

    until("the notice is raised", || failing.now().is_some());
    assert_eq!(failing.now().unwrap().said, lines[10..].to_vec());
    host.end();
}

#[test]
fn a_host_the_window_ends_is_never_asked_of_its_manager_again() {
    let home = tempfile::tempdir().unwrap();
    let manager = Arc::new(AManager::default());
    let host = SessionHost::default();
    host.end();

    host.start_managed_at(
        a_place(home.path()),
        Arc::clone(&manager) as Arc<dyn purlis_core::loginitem::Manager>,
        None,
        NEVER_ANSWERS_WITHIN,
        a_host_the_window_must_not_start(),
    );
    std::thread::sleep(Duration::from_millis(300));

    assert_eq!(manager.asked(), Vec::<&str>::new());
}

#[test]
fn the_login_items_are_kept_while_something_keeps_the_host_and_forgotten_once_nothing_does() {
    let manager = Arc::new(AManager::default());
    let host = SessionHost::default();
    host.with_manager(
        Arc::clone(&manager) as Arc<dyn purlis_core::loginitem::Manager>,
        false,
    );
    assert!(!host.is_managed(), "the window runs the host this launch");

    host.keep_login_items();
    host.forget_login_items();

    assert_eq!(manager.asked(), ["install", "remove"]);
}

#[test]
fn a_manager_that_cannot_run_the_host_hands_it_back_to_the_window_and_the_window_says_why() {
    let home = tempfile::tempdir().unwrap();
    let started = home.path().join("started");
    let manager = Arc::new(AManager::refusing());
    let host = SessionHost::default();
    let failing = host.failing();

    host.start_managed_at(
        a_place(home.path()),
        Arc::clone(&manager) as Arc<dyn purlis_core::loginitem::Manager>,
        None,
        NEVER_ANSWERS_WITHIN,
        a_host_that_crashes_as_it_starts(&started),
    );

    until("the window starts a host of its own", || {
        starts_in(&started) > 0
    });
    assert!(!host.is_managed(), "the window supervises its host now");
    let why = failing
        .not_in_the_background()
        .expect("the window says why the host is not run in the background");
    assert!(why.why.contains("Input/output error"), "{why:?}");
    until("the window's own breaker trips", || failing.now().is_some());
    assert_eq!(
        manager.asked(),
        ["install"],
        "asked once, never in a loop into the breaker"
    );
    host.end();
}

#[test]
fn a_managed_host_the_quit_never_reached_is_held_by_its_manager() {
    let home = tempfile::tempdir().unwrap();
    let manager = Arc::new(AManager::default());
    let host = SessionHost::default();
    host.start_managed_at(
        a_place(home.path()),
        Arc::clone(&manager) as Arc<dyn purlis_core::loginitem::Manager>,
        None,
        Duration::from_secs(60),
        a_host_the_window_must_not_start(),
    );
    until("the manager is asked to start it", || {
        manager.asked().contains(&"start")
    });

    host.end();

    assert_eq!(
        manager.asked(),
        ["install", "start", "hold"],
        "with no link to tell it to quit, its manager stops it, so no chat outlives the quit"
    );
}
