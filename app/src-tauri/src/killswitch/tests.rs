// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::mpsc;

use super::*;
use purlis_core::halt::{Entry, Event};

fn a_config_home() -> tempfile::TempDir {
    tempfile::tempdir().expect("a config home")
}

fn switch_in(config: &Path) -> Arc<KillSwitch> {
    KillSwitch::kept_in(Some(config.to_path_buf()))
}

fn events(config: &Path) -> Vec<(Event, Actor)> {
    halt::journal(config)
        .into_iter()
        .map(|Entry { event, by, .. }| (event, by))
        .collect()
}

#[test]
fn a_stop_another_process_wrote_is_in_force_before_anything_looked_at_it() {
    let config = a_config_home();
    let switch = switch_in(config.path());

    halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

    assert!(switch.is_stopped());
}

#[test]
fn an_app_started_after_a_stop_starts_stopped() {
    let config = a_config_home();
    halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

    assert!(switch_in(config.path()).is_stopped());
}

#[test]
fn an_app_started_after_a_stop_whose_marker_was_removed_starts_stopped() {
    let config = a_config_home();
    halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");
    std::fs::remove_file(halt::marker(config.path())).expect("removed by an agent");

    assert!(switch_in(config.path()).is_stopped());
}

#[test]
fn a_marker_removed_while_stopped_does_not_re_arm_and_is_put_back_as_a_tamper() {
    let config = a_config_home();
    let switch = switch_in(config.path());
    switch.stop(Actor::Window).expect("stopped");

    std::fs::remove_file(halt::marker(config.path())).expect("removed by an agent");
    // Even with a forged re-arm in the journal: the switch in memory is the authority.
    let forged = serde_json::to_string(&Entry {
        at: 2,
        event: Event::Rearm,
        by: Actor::Window,
    })
    .unwrap();
    let journal = halt::journal_path(config.path());
    let mut text = std::fs::read_to_string(&journal).unwrap();
    text.push_str(&forged);
    text.push('\n');
    std::fs::write(&journal, text).unwrap();

    assert_eq!(switch.look(), Moved::Tampered);
    assert!(switch.is_stopped());
    assert!(
        halt::marker_present(config.path()),
        "the marker was not put back"
    );
    assert_eq!(
        events(config.path()).last(),
        Some(&(Event::Tamper, Actor::App))
    );
}

#[test]
fn a_look_after_a_stop_on_disk_stops_this_process_and_the_marker_then_cannot_undo_it() {
    let config = a_config_home();
    let switch = switch_in(config.path());
    halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

    assert_eq!(switch.look(), Moved::Stopped);
    assert_eq!(switch.look(), Moved::Nothing, "a second look moves nothing");
}

#[test]
fn only_a_re_arm_lets_chats_start_again_and_its_own_removal_is_no_tamper() {
    let config = a_config_home();
    let switch = switch_in(config.path());
    switch.stop(Actor::Window).expect("stopped");

    assert!(switch.rearm().expect("re-armed"));

    assert!(!switch.is_stopped());
    assert_eq!(switch.look(), Moved::Nothing);
    assert_eq!(
        events(config.path()),
        [(Event::Stop, Actor::Window), (Event::Rearm, Actor::Window)]
    );
}

#[test]
fn the_window_s_stop_holds_with_no_config_home_and_says_it_will_not_outlive_the_app() {
    let switch = KillSwitch::unthrown();
    let said = switch
        .stop(Actor::Window)
        .expect_err("nothing to keep it in");
    assert!(said.contains("a relaunch will start chats again"), "{said}");
    assert!(switch.is_stopped());
    assert!(switch.rearm().expect("nothing to remove"));
    assert!(!switch.is_stopped());
}

#[cfg(unix)]
#[test]
fn a_stop_defeated_in_advance_by_a_chmod_still_stops_this_app_and_says_so() {
    use std::os::unix::fs::PermissionsExt;
    let config = a_config_home();
    let dir = purlis_core::machine::dir(config.path());
    std::fs::create_dir_all(&dir).expect("charter's directory");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).expect("chmod");
    let switch = switch_in(config.path());

    let said = switch.stop(Actor::Window);

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    let said = said.expect_err("nothing could be written");
    assert!(
        said.contains("every agent is stopped in this app"),
        "{said}"
    );
    assert!(switch.is_stopped());
}

/// An event log in `data`, for a switch to record into.
fn a_log_in(data: &Path) -> crate::hooks::Events {
    use purlis_core::eventlog::{ArgsKey, Log, Recorder};
    crate::hooks::Events::here(Recorder::new(
        Log::open(data, "01J9ZZDEVICE0000000000000").expect("a log"),
        ArgsKey::open(data).expect("a key"),
    ))
}

/// The kill-switch events in the log in `data` once there are `n` of them, as (kind,
/// actor_kind), or what there were after the wait.
fn logged(data: &Path, n: usize) -> Vec<(String, String)> {
    let until = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let found: Vec<(String, String)> = purlis_core::eventlog::read(data)
            .expect("a log")
            .into_iter()
            .filter(|event| event.kind.starts_with("killswitch."))
            .map(|event| {
                let kind = event.body["actor_kind"].as_str().unwrap_or("").to_owned();
                (event.kind, kind)
            })
            .collect();
        if found.len() >= n || std::time::Instant::now() > until {
            return found;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn said(kind: &str, actor_kind: &str) -> (String, String) {
    (kind.to_owned(), actor_kind.to_owned())
}

#[test]
fn the_window_s_stop_and_re_arm_are_recorded_as_the_operator_s() {
    // #1375: after the switch has acted, never before it.
    let config = a_config_home();
    let data = tempfile::tempdir().expect("a data home");
    let switch = switch_in(config.path());
    switch.record_into(a_log_in(data.path()));

    switch.stop(Actor::Window).expect("stopped");
    switch.rearm().expect("re-armed");

    assert_eq!(
        logged(data.path(), 2),
        [
            said("killswitch.stop", "human"),
            said("killswitch.rearm", "human")
        ]
    );
}

#[test]
fn a_stop_another_process_wrote_is_recorded_as_unattributed_once() {
    let config = a_config_home();
    let data = tempfile::tempdir().expect("a data home");
    let switch = switch_in(config.path());
    switch.record_into(a_log_in(data.path()));
    halt::stop(config.path(), Actor::Cli, halt::now()).expect("stopped from a terminal");

    assert_eq!(switch.look(), Moved::Stopped);
    switch.noted(Noted::Read);

    assert_eq!(
        logged(data.path(), 1),
        [said("killswitch.stop", "unattributed")]
    );
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(logged(data.path(), 2).len(), 1, "read twice, one event");
}

#[cfg(unix)]
#[test]
fn a_tamper_is_recorded_even_when_neither_file_could_be_written() {
    use std::os::unix::fs::PermissionsExt;
    let config = a_config_home();
    let data = tempfile::tempdir().expect("a data home");
    let switch = switch_in(config.path());
    switch.record_into(a_log_in(data.path()));
    switch.stop(Actor::Window).expect("stopped");
    std::fs::remove_file(halt::marker(config.path())).expect("removed by an agent");
    let dir = purlis_core::machine::dir(config.path());
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).expect("chmod");

    let moved = switch.look();

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    assert_eq!(moved, Moved::Tampered);
    assert!(switch.is_stopped());
    assert_eq!(
        logged(data.path(), 2),
        [
            said("killswitch.stop", "human"),
            said("killswitch.tamper", "host")
        ]
    );
}

#[test]
fn a_marker_made_and_taken_away_inside_one_burst_is_still_looked_at() {
    use crate::watchset::raw::{Raw, raw};
    use notify::event::{CreateKind, EventKind, RemoveKind};
    let config = a_config_home();
    let switch = switch_in(config.path());
    let dir = switch.directory().expect("charter's directory");
    let (tell, heard) = mpsc::channel();
    let _watch = watch_in::<Raw>(&dir, Arc::clone(&switch), move |moved| {
        let _ = tell.send(moved);
    })
    .expect("watching");
    halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

    // What the platform reports of it is only the marker, made and at once taken away.
    let marker = halt::marker(config.path());
    raw(EventKind::Create(CreateKind::File), &marker);
    raw(EventKind::Remove(RemoveKind::File), &marker);

    assert_eq!(
        heard.recv_timeout(HEARD_WITHIN),
        Ok(Moved::Stopped),
        "the watch never looked"
    );
}

#[test]
fn a_platform_that_lost_track_is_a_look_at_the_switch() {
    use crate::watchset::raw::{Raw, event};
    let config = a_config_home();
    let switch = switch_in(config.path());
    let dir = switch.directory().expect("charter's directory");
    let (tell, heard) = mpsc::channel();
    let _watch = watch_in::<Raw>(&dir, Arc::clone(&switch), move |moved| {
        let _ = tell.send(moved);
    })
    .expect("watching");
    halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

    event(notify::Event::new(notify::EventKind::Other).set_flag(notify::event::Flag::Rescan));

    assert_eq!(
        heard.recv_timeout(HEARD_WITHIN),
        Ok(Moved::Stopped),
        "the watch never looked"
    );
}

#[test]
fn a_watch_that_hears_nothing_still_sees_a_stop_within_its_period() {
    // D-88l: the watcher's events are cut off entirely (Raw is told nothing), as a watch
    // the platform has lost would be; the stop is still seen, a period late at most.
    use crate::watchset::raw::Raw;
    let config = a_config_home();
    let switch = switch_in(config.path());
    let dir = switch.directory().expect("charter's directory");
    let (tell, heard) = mpsc::channel();
    let every = Duration::from_millis(200);
    let _watch = watch_in_every::<Raw>(
        &dir,
        Arc::clone(&switch),
        move |moved| {
            let _ = tell.send(moved);
        },
        every,
    )
    .expect("watching");

    halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

    assert_eq!(
        heard.recv_timeout(HEARD_WITHIN),
        Ok(Moved::Stopped),
        "a watch that heard nothing never looked"
    );
    assert!(switch.is_stopped());
}

/// [`watch_in_every`] on [`Raw`](crate::watchset::raw::Raw), looking every 50 ms, and how
/// many times `dir` has been watched so far.
fn watching_counted(
    config: &Path,
) -> (
    Watch<crate::watchset::raw::Raw>,
    PathBuf,
    impl Fn() -> usize,
) {
    let switch = switch_in(config);
    let dir = switch.directory().expect("charter's directory");
    let watch = watch_in_every::<crate::watchset::raw::Raw>(
        &dir,
        switch,
        |_| {},
        Duration::from_millis(50),
    )
    .expect("watching");
    let counted = dir.clone();
    (watch, dir, move || crate::watchset::raw::watched(&counted))
}

/// Waits until `count` says `want`, or ten seconds have passed, and says what it said.
fn reaches(count: impl Fn() -> usize, want: usize) -> usize {
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while count() < want && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    count()
}

#[test]
fn a_directory_the_platform_says_was_removed_is_watched_again_even_as_the_same_inode() {
    // Linux: removed and made again at once, the directory can have the same device and
    // inode, and its watch is gone all the same. What says so is the platform's removal
    // of the directory itself; here nothing on disk changes at all.
    use notify::event::{EventKind, RemoveKind};
    let config = a_config_home();
    let (_watch, dir, watches) = watching_counted(config.path());
    assert_eq!(watches(), 1);

    crate::watchset::raw::raw(EventKind::Remove(RemoveKind::Folder), &dir);

    assert_eq!(
        reaches(&watches, 2),
        2,
        "the directory the platform removed was not watched again"
    );
}

#[test]
fn a_directory_made_again_as_another_is_watched_again_with_nothing_said() {
    // A watch that dies without a word: the directory is another one, and the idle look
    // sees it. The old one is kept aside, so the new one cannot have its inode.
    let config = a_config_home();
    let (_watch, dir, watches) = watching_counted(config.path());
    assert_eq!(watches(), 1);

    std::fs::rename(&dir, dir.with_extension("old")).expect("moved aside");
    std::fs::create_dir_all(&dir).expect("made again");

    assert_eq!(
        reaches(&watches, 2),
        2,
        "the directory made again was not watched again"
    );
}

#[test]
fn the_watch_hears_charter_stop_all_and_puts_back_a_marker_taken_away() {
    let config = a_config_home();
    let switch = switch_in(config.path());
    let (tell, heard) = mpsc::channel();
    let _watch = watch(Arc::clone(&switch), move |moved| {
        let _ = tell.send(moved);
    })
    .expect("a directory to watch")
    .expect("watching");
    // The real watcher, at the real period: on a busy macOS machine its events can arrive
    // late or not at all, and the idle look is what still hears the stop (#1006).
    let next = |wanted: Moved| loop {
        let moved = heard
            .recv_timeout(HEARD_WITHIN)
            .expect("the watch said nothing");
        if moved == wanted {
            break;
        }
    };

    halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");
    next(Moved::Stopped);
    assert!(switch.is_stopped());

    std::fs::remove_file(halt::marker(config.path())).expect("removed by an agent");
    next(Moved::Tampered);
    assert!(halt::marker_present(config.path()));
    assert!(switch.is_stopped());
}

/// A session host's hold on the switch, as the app's switch reaches it (#2001).
#[derive(Default)]
struct AHost {
    stopped: Mutex<bool>,
    refuses: bool,
    /// Its stops fail, for as long as this says so.
    stops_fail: Mutex<bool>,
    /// It holds each stop but cannot keep it across its restart.
    cannot_keep: bool,
    heard: Mutex<Vec<&'static str>>,
}

impl AHost {
    fn stopped() -> Arc<AHost> {
        let host = AHost::default();
        *host.stopped.lock().unwrap() = true;
        Arc::new(host)
    }

    fn refusing() -> Arc<AHost> {
        Arc::new(AHost {
            refuses: true,
            ..AHost::default()
        })
    }

    fn is_stopped(&self) -> bool {
        *self.stopped.lock().unwrap()
    }

    fn heard(&self) -> Vec<&'static str> {
        self.heard.lock().unwrap().clone()
    }
}

impl Authority for AHost {
    fn stop(&self) -> Result<Option<String>, String> {
        self.heard.lock().unwrap().push("stop");
        if self.refuses || *self.stops_fail.lock().unwrap() {
            return Err("the session host is not running".into());
        }
        *self.stopped.lock().unwrap() = true;
        Ok(self
            .cannot_keep
            .then(|| "the stop could not be kept across its restart".to_owned()))
    }

    fn rearm(&self) -> Result<(), String> {
        self.heard.lock().unwrap().push("rearm");
        if self.refuses {
            return Err("the session host is not running".into());
        }
        *self.stopped.lock().unwrap() = false;
        Ok(())
    }

    fn stopped(&self) -> Result<bool, String> {
        if self.refuses {
            return Err("the session host is not running".into());
        }
        Ok(self.is_stopped())
    }
}

fn held_by(config: &Path, host: &Arc<AHost>) -> Arc<KillSwitch> {
    let switch = switch_in(config);
    switch.held_by(Arc::clone(host) as Arc<dyn Authority>);
    switch
}

#[test]
fn the_window_s_stop_is_the_session_host_s_too() {
    let config = a_config_home();
    let host = Arc::new(AHost::default());
    let switch = held_by(config.path(), &host);

    switch.stop(Actor::Window).expect("stopped");

    assert!(host.is_stopped());
    assert!(halt::stopped_on_disk(config.path()));
}

#[test]
fn a_stop_from_a_terminal_the_watch_hears_reaches_the_session_host() {
    let config = a_config_home();
    let host = Arc::new(AHost::default());
    let switch = held_by(config.path(), &host);
    halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

    assert_eq!(switch.look(), Moved::Stopped);

    assert!(host.is_stopped());
}

#[test]
fn a_re_arm_is_the_session_host_s_first_and_then_the_files() {
    let config = a_config_home();
    let host = Arc::new(AHost::default());
    let switch = held_by(config.path(), &host);
    switch.stop(Actor::Window).expect("stopped");

    assert!(switch.rearm().expect("re-armed"));

    assert_eq!(host.heard(), ["stop", "rearm"]);
    assert!(!host.is_stopped());
    assert!(!switch.is_stopped());
    assert!(!halt::stopped_on_disk(config.path()));
}

#[test]
fn a_re_arm_the_session_host_did_not_take_leaves_every_agent_stopped() {
    let config = a_config_home();
    let host = AHost::refusing();
    let switch = held_by(config.path(), &host);
    let _ = switch.stop(Actor::Window);

    let refused = switch.rearm().expect_err("not re-armed");

    assert!(refused.contains("still stopped"), "{refused}");
    assert!(switch.is_stopped());
    assert!(
        halt::stopped_on_disk(config.path()),
        "the files still say stopped"
    );
}

#[test]
fn a_stop_the_session_host_was_not_told_of_still_stops_this_app_and_says_so() {
    let config = a_config_home();
    let switch = held_by(config.path(), &AHost::refusing());

    let said = switch
        .stop(Actor::Window)
        .expect_err("the host was not told");

    assert!(switch.is_stopped());
    assert!(said.contains("session host"), "{said}");
}

#[test]
fn an_app_that_finds_its_session_host_stopped_holds_every_agent_stopped() {
    let config = a_config_home();
    let switch = held_by(config.path(), &AHost::stopped());
    assert!(!switch.is_stopped(), "the files say nothing");

    assert!(switch.agree_with_its_host(), "the switch moved");

    assert!(switch.is_stopped());
}

#[test]
fn an_app_that_starts_stopped_tells_its_session_host() {
    let config = a_config_home();
    halt::stop(config.path(), Actor::Cli, 1).expect("stopped while no host ran");
    let host = Arc::new(AHost::default());
    let switch = held_by(config.path(), &host);

    assert!(!switch.agree_with_its_host(), "it was stopped already");

    assert!(host.is_stopped());
}

#[test]
fn a_switch_with_no_session_host_is_as_it_was() {
    let config = a_config_home();
    let switch = switch_in(config.path());

    assert!(!switch.agree_with_its_host());
    switch.stop(Actor::Window).expect("stopped");
    assert!(switch.rearm().expect("re-armed"));
}

#[cfg(unix)]
#[test]
fn a_re_arm_the_files_did_not_take_stops_the_host_again_or_owes_it_the_stop_until_it_does() {
    use std::os::unix::fs::PermissionsExt;
    let config = a_config_home();
    let host = Arc::new(AHost::default());
    let switch = held_by(config.path(), &host);
    switch.stop(Actor::Window).expect("stopped");
    // The files cannot take a re-arm, and the host cannot take its stop back.
    let dir = halt::directory(config.path()).expect("the folder");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).expect("chmod");
    *host.stops_fail.lock().unwrap() = true;

    let refused = switch.rearm().expect_err("not re-armed");

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    assert!(refused.contains("still stopped"), "{refused}");
    assert!(
        refused.contains("session host"),
        "the host's part is said: {refused}"
    );
    assert!(switch.is_stopped());
    assert!(!host.is_stopped(), "the host took the re-arm");

    // The host takes stops again: the next look sends it the one it is owed.
    *host.stops_fail.lock().unwrap() = false;
    let _ = switch.look();

    assert!(
        host.is_stopped(),
        "the two never stay apart towards running"
    );
}

#[test]
fn a_stop_the_session_host_did_not_take_is_sent_again_when_the_app_next_agrees_with_it() {
    let config = a_config_home();
    let host = Arc::new(AHost::default());
    *host.stops_fail.lock().unwrap() = true;
    let switch = held_by(config.path(), &host);
    let _ = switch.stop(Actor::Window);
    assert!(!host.is_stopped());

    *host.stops_fail.lock().unwrap() = false;
    switch.agree_with_its_host();

    assert!(host.is_stopped());
}

#[test]
fn a_stop_the_session_host_holds_but_cannot_keep_is_said_as_that_and_not_sent_again() {
    let config = a_config_home();
    let host = Arc::new(AHost {
        cannot_keep: true,
        ..AHost::default()
    });
    let switch = held_by(config.path(), &host);

    let said = switch
        .stop(Actor::Window)
        .expect_err("not kept by the host");
    let _ = switch.look();

    assert!(said.contains("session host said"), "{said}");
    assert!(!said.contains("not told"), "{said}");
    assert_eq!(
        host.heard(),
        ["stop"],
        "a stop the host holds is not owed to it"
    );
}
