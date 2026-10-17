// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const RUN_USER_BUS: &str = "unix:path=/run/user/1000/bus";
const AUTOLAUNCHED: &str = "unix:path=/tmp/dbus-3Awemqh910,guid=6eda85db4fed2c76c15d1ae76abd3017";

/// No X session to autolaunch a bus on: what `decide` is handed when it must not be asked.
fn not_asked() -> Option<(String, Heard)> {
    panic!("the X session's bus was looked for when it did not have to be")
}

fn nothing_there() -> Option<(String, Heard)> {
    None
}

fn says_what_is_off(said: &str) {
    assert!(
        said.contains("tray") && said.contains("notifications"),
        "{said}"
    );
    assert!(said.contains("second launch"), "{said}");
    assert!(said.contains("charter-app#24"), "{said}");
}

#[test]
fn a_portal_that_answered_leaves_the_launch_as_it_is() {
    assert_eq!(
        decide(Some(RUN_USER_BUS), Heard::Answer, &mut not_asked),
        Start::AsItIs
    );
    assert_eq!(decide(None, Heard::Answer, &mut not_asked), Start::AsItIs);
}

#[test]
fn a_silent_portal_starts_the_launch_again_without_the_bus_keeping_the_bus_for_chats() {
    let Start::WithoutTheBus { say, kept } =
        decide(Some(RUN_USER_BUS), Heard::Silence, &mut not_asked)
    else {
        panic!("a silent portal did not restart the launch");
    };
    assert!(say.contains("300 ms"), "{say}");
    says_what_is_off(&say);
    assert_eq!(kept, RUN_USER_BUS);
}

/// Found by the standard lookup (`$XDG_RUNTIME_DIR/bus`), which a chat does too.
#[test]
fn a_silent_portal_on_a_bus_nothing_named_keeps_no_address() {
    let Start::WithoutTheBus { kept, .. } = decide(None, Heard::Silence, &mut not_asked) else {
        panic!("a silent portal did not restart the launch");
    };
    assert_eq!(kept, "");
}

#[test]
fn a_named_bus_that_is_not_there_already_fails_fast_and_is_left_alone() {
    assert_eq!(
        decide(Some("unix:path=/nowhere"), Heard::NoBus, &mut not_asked),
        Start::AsItIs
    );
}

/// `startx` into i3: no bus named and none at `$XDG_RUNTIME_DIR/bus`. GIO would autolaunch
/// one on the X display — the one a notification daemon started from the i3 config is
/// already on — and zbus, which the tray, notifications and single-instance use, would
/// not find it. So the launch looks for it as GIO does, and starts again on it.
#[test]
fn with_no_bus_named_the_launch_starts_again_on_the_bus_of_the_x_session() {
    let mut autolaunched = || Some((AUTOLAUNCHED.to_owned(), Heard::Answer));

    assert_eq!(
        decide(None, Heard::NoBus, &mut autolaunched),
        Start::OnTheBus(AUTOLAUNCHED.to_owned())
    );
}

#[test]
fn a_silent_portal_on_the_bus_of_the_x_session_starts_without_it_and_keeps_it_for_chats() {
    let mut autolaunched = || Some((AUTOLAUNCHED.to_owned(), Heard::Silence));

    let Start::WithoutTheBus { say, kept } = decide(None, Heard::NoBus, &mut autolaunched) else {
        panic!("a silent portal did not restart the launch");
    };
    says_what_is_off(&say);
    assert_eq!(kept, AUTOLAUNCHED);
}

#[test]
fn with_no_bus_anywhere_the_launch_says_what_that_costs_and_keeps_no_address() {
    let mut unreachable = || Some((AUTOLAUNCHED.to_owned(), Heard::NoBus));
    for autolaunched in [
        &mut nothing_there as &mut dyn FnMut() -> _,
        &mut unreachable,
    ] {
        let Start::WithoutTheBus { say, kept } = decide(None, Heard::NoBus, autolaunched) else {
            panic!("a launch with no bus went on as if it had one");
        };
        assert!(say.contains("no session bus"), "{say}");
        says_what_is_off(&say);
        assert_eq!(kept, "");
    }
}

#[test]
fn a_launch_that_is_already_the_restart_never_restarts_again() {
    for heard in [Heard::Answer, Heard::Silence, Heard::NoBus] {
        assert_eq!(
            decide(Some(NO_SESSION_BUS), heard, &mut not_asked),
            Start::AsItIs,
            "{heard:?}"
        );
    }
}

/// One launch in `session` on `address`: how often its portal was asked, and how it went on.
fn launched(
    session: &LoginSession,
    address: Option<&str>,
    heard: Heard,
    x_session_bus: &mut dyn FnMut() -> Option<(String, Heard)>,
) -> (usize, Start) {
    let mut asked = 0;
    let start = launch(
        address,
        session,
        &mut || {
            asked += 1;
            heard
        },
        x_session_bus,
    );
    (asked, start)
}

/// A silent portal is asked once per login session (#2432): the first launch gives it
/// [`BUDGET`], and the next launch in the same session starts without the bus at once.
#[test]
fn a_portal_silent_once_in_a_login_session_is_not_asked_again_in_it() {
    let runtime = tempfile::tempdir().expect("a directory");
    let session = LoginSession::at(Some(runtime.path()));

    let (asked, first) = launched(&session, Some(RUN_USER_BUS), Heard::Silence, &mut not_asked);
    assert_eq!(asked, 1, "the first launch did not ask");
    assert!(matches!(first, Start::WithoutTheBus { .. }), "{first:?}");

    let (asked, next) = launched(&session, Some(RUN_USER_BUS), Heard::Answer, &mut not_asked);
    assert_eq!(asked, 0, "a launch later in the session asked again");
    let Start::WithoutTheBus { say, kept } = next else {
        panic!("a launch later in the session went on with the bus: {next:?}");
    };
    assert!(say.contains("login session"), "{say}");
    says_what_is_off(&say);
    assert_eq!(kept, RUN_USER_BUS, "the chats lost the bus");
}

#[test]
fn a_launch_that_remembers_nothing_asks() {
    let runtime = tempfile::tempdir().expect("a directory");
    let session = LoginSession::at(Some(runtime.path()));

    let (asked, start) = launched(&session, None, Heard::Answer, &mut not_asked);

    assert_eq!(asked, 1);
    assert_eq!(start, Start::AsItIs);
}

#[test]
fn a_portal_that_answered_is_asked_again_at_the_next_launch() {
    let runtime = tempfile::tempdir().expect("a directory");
    let session = LoginSession::at(Some(runtime.path()));

    launched(&session, Some(RUN_USER_BUS), Heard::Answer, &mut not_asked);
    let (asked, _) = launched(&session, Some(RUN_USER_BUS), Heard::Answer, &mut not_asked);

    assert_eq!(asked, 1, "an answered portal was remembered as silent");
    assert_eq!(
        std::fs::read_dir(runtime.path())
            .expect("the runtime directory")
            .count(),
        0,
        "an answered portal left something in the runtime directory"
    );
}

/// Without `$XDG_RUNTIME_DIR` there is nowhere that ends with the login session, so every launch
/// asks, as before.
#[test]
fn with_no_runtime_directory_every_launch_asks() {
    let session = LoginSession::at(None);

    for _ in 0..2 {
        let (asked, start) = launched(&session, Some(RUN_USER_BUS), Heard::Silence, &mut not_asked);
        assert_eq!(asked, 1);
        assert!(matches!(start, Start::WithoutTheBus { .. }), "{start:?}");
    }
}

/// The silence was heard on one bus; a launch given another asks that one.
#[test]
fn a_launch_on_another_bus_asks_its_own_portal() {
    let runtime = tempfile::tempdir().expect("a directory");
    let session = LoginSession::at(Some(runtime.path()));
    launched(&session, Some(RUN_USER_BUS), Heard::Silence, &mut not_asked);

    let (asked, start) = launched(&session, None, Heard::Answer, &mut not_asked);

    assert_eq!(asked, 1);
    assert_eq!(start, Start::AsItIs);
}

/// `listen_again` heard the portal answer after the launch: the next launch asks it again.
#[test]
fn a_portal_that_answers_later_is_asked_again_at_the_next_launch() {
    let runtime = tempfile::tempdir().expect("a directory");
    let session = LoginSession::at(Some(runtime.path()));
    launched(&session, Some(RUN_USER_BUS), Heard::Silence, &mut not_asked);

    session.forget();
    let (asked, start) = launched(&session, Some(RUN_USER_BUS), Heard::Answer, &mut not_asked);

    assert_eq!(asked, 1);
    assert_eq!(start, Start::AsItIs);
}

/// The X display's bus is found by `dbus-launch` at each launch and is not written down, so a
/// silence there is asked about again.
#[test]
fn a_silence_on_the_bus_of_the_x_session_is_not_remembered() {
    let runtime = tempfile::tempdir().expect("a directory");
    let session = LoginSession::at(Some(runtime.path()));
    let mut autolaunched = || Some((AUTOLAUNCHED.to_owned(), Heard::Silence));

    for _ in 0..2 {
        let (asked, start) = launched(&session, None, Heard::NoBus, &mut autolaunched);
        assert_eq!(asked, 1);
        assert!(matches!(start, Start::WithoutTheBus { .. }), "{start:?}");
    }
}

#[test]
fn the_restart_never_asks_and_never_remembers() {
    let runtime = tempfile::tempdir().expect("a directory");
    let session = LoginSession::at(Some(runtime.path()));

    let (asked, start) = launched(
        &session,
        Some(NO_SESSION_BUS),
        Heard::Silence,
        &mut not_asked,
    );

    assert_eq!((asked, start), (0, Start::AsItIs));
    let (asked, _) = launched(&session, Some(RUN_USER_BUS), Heard::Answer, &mut not_asked);
    assert_eq!(asked, 1);
}

/// What a restart's environment says about the bus: set, removed, or left as it was.
fn bus_of(again: &std::process::Command) -> Vec<(String, Option<String>)> {
    let mut said: Vec<(String, Option<String>)> = again
        .get_envs()
        .map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    said.sort();
    said
}

fn again(bus: &Bus) -> std::process::Command {
    launch_on(
        "/usr/bin/charter-app".into(),
        Some("charter-app".into()),
        ["--no-restore".into()],
        bus,
    )
}

#[test]
fn a_restart_without_the_bus_points_it_at_nothing_and_keeps_the_bus_it_had() {
    let without = again(&Bus::Without {
        kept: RUN_USER_BUS.to_owned(),
    });

    assert_eq!(
        bus_of(&without),
        [
            (SESSION_BUS.to_owned(), Some(NO_SESSION_BUS.to_owned())),
            (SESSION_BUS_KEPT.to_owned(), Some(RUN_USER_BUS.to_owned())),
        ]
    );
    assert_eq!(
        without.get_args().collect::<Vec<_>>(),
        ["--no-restore"],
        "the restart lost this launch's arguments"
    );
}

#[test]
fn a_restart_onto_a_bus_names_it_and_keeps_nothing_aside() {
    assert_eq!(
        bus_of(&again(&Bus::At(AUTOLAUNCHED.to_owned()))),
        [
            ("CHARTER_SESSION_BUS_KEPT".to_owned(), None),
            (SESSION_BUS.to_owned(), Some(AUTOLAUNCHED.to_owned())),
            (SESSION_BUS_KEPT.to_owned(), None),
        ]
    );
    assert_eq!(
        bus_of(&again(&Bus::Default)),
        [
            ("CHARTER_SESSION_BUS_KEPT".to_owned(), None),
            (SESSION_BUS.to_owned(), None),
            (SESSION_BUS_KEPT.to_owned(), None),
        ]
    );
}

#[test]
fn a_launch_on_the_bus_has_nothing_to_tell_the_window() {
    let bus = SessionBus::of(Some(RUN_USER_BUS), None, None);

    assert_eq!(bus.now(), None);
    assert_eq!(bus.where_to_ask_again(), None);
}

#[test]
fn a_launch_away_from_a_silent_portal_tells_the_window_why_and_what_is_off() {
    let bus = SessionBus::of(Some(NO_SESSION_BUS), Some(RUN_USER_BUS), None);

    let Some(told) = bus.now() else {
        panic!("the window was not told the launch has no bus");
    };
    assert!(
        told.says.contains("desktop portal did not answer"),
        "{told:?}"
    );
    for off in ["tray icon", "notifications", "second launch", "keyring"] {
        assert!(told.says.contains(off), "{off} is not named: {told:?}");
    }
    assert!(!told.can_restart, "{told:?}");
}

#[test]
fn a_launch_with_no_bus_at_all_tells_the_window_so() {
    for kept in [Some(""), None] {
        let told = SessionBus::of(Some(NO_SESSION_BUS), kept, None)
            .now()
            .expect("the window is told");
        assert!(told.says.contains("no session bus"), "{told:?}");
        assert!(told.says.contains("notifications"), "{told:?}");
    }
}

#[test]
fn the_restart_is_offered_only_once_the_kept_bus_answers_and_goes_back_onto_it() {
    let bus = SessionBus::of(Some(NO_SESSION_BUS), Some(RUN_USER_BUS), None);
    assert_eq!(bus.back_onto(), None, "offered before anything answered");
    assert!(bus.restart_asked().is_err(), "a restart onto a silent bus");
    assert_eq!(bus.restart_wanted(), None);

    bus.answered();

    assert!(bus.now().expect("still told").can_restart);
    assert_eq!(bus.restart_wanted(), None, "restarted without being asked");
    bus.restart_asked().expect("the operator's restart");
    assert_eq!(bus.restart_wanted(), Some(Bus::At(RUN_USER_BUS.to_owned())));
}

#[test]
fn the_kept_bus_is_asked_again_where_the_launch_found_it() {
    let runtime = tempfile::tempdir().expect("a directory");
    std::fs::write(runtime.path().join("bus"), "").expect("a socket's stand-in");

    assert_eq!(
        SessionBus::of(Some(NO_SESSION_BUS), Some(RUN_USER_BUS), None).where_to_ask_again(),
        Some(RUN_USER_BUS.to_owned())
    );
    let found = SessionBus::of(Some(NO_SESSION_BUS), Some(""), Some(runtime.path()));
    assert_eq!(
        found.where_to_ask_again(),
        Some(format!(
            "unix:path={}",
            runtime.path().join("bus").display()
        ))
    );
    found.answered();
    assert_eq!(found.back_onto(), Some(Bus::Default));

    let nowhere = tempfile::tempdir().expect("a directory");
    let none = SessionBus::of(Some(NO_SESSION_BUS), Some(""), Some(nowhere.path()));
    assert_eq!(none.where_to_ask_again(), None);
    none.answered();
    assert_eq!(none.back_onto(), None, "a restart offered onto no bus");
}

/// A container's `/etc/machine-id` is often there and empty; D-Bus's own copy is the next
/// place GIO looks, and so is it here.
#[test]
fn the_machine_id_is_the_first_one_that_is_not_empty() {
    let dir = tempfile::tempdir().expect("a directory");
    let (etc, dbus) = (dir.path().join("etc"), dir.path().join("dbus"));
    std::fs::write(&etc, "").expect("an empty id");
    std::fs::write(&dbus, "c157283747a2ad020fa749a86abd2fb4\n").expect("an id");

    assert_eq!(
        machine_id(&[&etc, &dbus]).as_deref(),
        Some("c157283747a2ad020fa749a86abd2fb4")
    );
    assert_eq!(machine_id(&[&etc, &dir.path().join("none")]), None);
}

#[test]
fn the_bus_is_asked_again_with_growing_waits_until_it_answers() {
    let mut waited = Vec::new();
    let mut heard = [Heard::Silence, Heard::Answer].into_iter();

    let answered = ask_until_answered(
        &ASKED_AGAIN_AFTER,
        &mut |wait| waited.push(wait),
        &mut || heard.next().expect("asked once too often"),
    );

    assert!(answered);
    assert_eq!(waited, ASKED_AGAIN_AFTER[..2]);
}

#[test]
fn a_bus_that_never_answers_is_asked_each_time_and_then_left_alone() {
    let mut asked = 0;

    let answered = ask_until_answered(&ASKED_AGAIN_AFTER, &mut |_| {}, &mut || {
        asked += 1;
        Heard::Silence
    });

    assert!(!answered);
    assert_eq!(asked, ASKED_AGAIN_AFTER.len());
    assert!(ASKED_AGAIN_AFTER.len() > 1, "only one ask after the launch");
    assert!(
        ASKED_AGAIN_AFTER.windows(2).all(|w| w[0] < w[1]),
        "the waits do not back off"
    );
}

#[test]
fn at_exit_the_launch_on_the_bus_is_started_only_when_the_operator_asked() {
    let bus = SessionBus::of(Some(NO_SESSION_BUS), Some(RUN_USER_BUS), None);
    let mut started = Vec::new();

    restart_if_asked_with(&bus, &mut |onto| {
        started.push(onto.clone());
        Ok(())
    });
    bus.answered();
    restart_if_asked_with(&bus, &mut |onto| {
        started.push(onto.clone());
        Ok(())
    });
    assert_eq!(started, [], "started without the operator asking");

    bus.restart_asked().expect("the operator's restart");
    restart_if_asked_with(&bus, &mut |onto| {
        started.push(onto.clone());
        Ok(())
    });

    assert_eq!(started, [Bus::At(RUN_USER_BUS.to_owned())]);
}

#[test]
fn each_bus_says_where_it_is_asked() {
    let runtime = tempfile::tempdir().expect("a directory");
    std::fs::write(runtime.path().join("bus"), "").expect("a socket's stand-in");
    let nowhere = tempfile::tempdir().expect("a directory");

    assert_eq!(
        Bus::At(RUN_USER_BUS.to_owned()).where_to_ask(None),
        Some(RUN_USER_BUS.to_owned())
    );
    assert_eq!(
        Bus::Default.where_to_ask(Some(runtime.path())),
        Some(format!(
            "unix:path={}",
            runtime.path().join("bus").display()
        ))
    );
    assert_eq!(Bus::Default.where_to_ask(Some(nowhere.path())), None);
    assert_eq!(Bus::Default.where_to_ask(None), None);
    assert_eq!(
        Bus::Without {
            kept: RUN_USER_BUS.to_owned()
        }
        .where_to_ask(None),
        None
    );
}

/// The step the launch log shows before a restart in place, so the second `run() entered`
/// it shows next reads as the same launch starting again, and on which bus (#2389).
#[test]
fn a_restart_in_place_is_a_launch_step_naming_the_bus_it_starts_on() {
    assert_eq!(
        Bus::Without {
            kept: RUN_USER_BUS.to_owned()
        }
        .starting_again(),
        "the launch starts again in place, without the session bus"
    );
    assert_eq!(
        Bus::At(AUTOLAUNCHED.to_owned()).starting_again(),
        format!("the launch starts again in place, on the session bus at {AUTOLAUNCHED}")
    );
    assert_eq!(
        Bus::Default.starting_again(),
        "the launch starts again in place, on the session bus the standard lookup finds"
    );
}

#[test]
fn the_address_is_read_from_what_dbus_launch_prints() {
    let printed = format!(
        "DBUS_SESSION_BUS_ADDRESS='{AUTOLAUNCHED}';\nexport DBUS_SESSION_BUS_ADDRESS;\n\
         DBUS_SESSION_BUS_PID=31;\nDBUS_SESSION_BUS_WINDOWID=2097153;\n"
    );

    assert_eq!(
        autolaunched_address(&printed).as_deref(),
        Some(AUTOLAUNCHED)
    );
    assert_eq!(autolaunched_address(""), None);
    assert_eq!(autolaunched_address("DBUS_SESSION_BUS_ADDRESS='';\n"), None);
}

/// A private `dbus-daemon` — the one every Linux desktop runs — with the services this
/// test declares activatable, and nothing else.
#[cfg(target_os = "linux")]
struct PrivateBus {
    daemon: std::process::Child,
    address: String,
    _dir: tempfile::TempDir,
}

#[cfg(target_os = "linux")]
impl PrivateBus {
    /// `services` is `(name, program)`: the bus starts `program` when `name` is asked for.
    fn with(services: &[(&str, &str)]) -> Self {
        use std::io::BufRead;

        let dir = tempfile::tempdir().expect("a directory");
        let services_dir = dir.path().join("services");
        std::fs::create_dir(&services_dir).expect("the services directory");
        for (name, program) in services {
            std::fs::write(
                services_dir.join(format!("{name}.service")),
                format!("[D-BUS Service]\nName={name}\nExec={program}\n"),
            )
            .expect("a service file");
        }
        let config = dir.path().join("session.conf");
        std::fs::write(
            &config,
            format!(
                r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path={}</listen>
  <servicedir>{}</servicedir>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#,
                dir.path().join("bus").display(),
                services_dir.display()
            ),
        )
        .expect("a bus config");
        let mut daemon = purlis_core::forklock::spawn(
            std::process::Command::new("dbus-daemon")
                .arg(format!("--config-file={}", config.display()))
                .args(["--nofork", "--print-address"])
                .stdout(std::process::Stdio::piped()),
        )
        .expect("dbus-daemon runs (apt: dbus-daemon)");
        let mut address = String::new();
        std::io::BufReader::new(daemon.stdout.take().expect("its stdout"))
            .read_line(&mut address)
            .expect("the bus prints its address");
        Self {
            daemon,
            address: address.trim().to_owned(),
            _dir: dir,
        }
    }
}

/// How long a test that expects an answer waits for one. The answer itself is not timed:
/// the bus replies in milliseconds, but a CI runner running every crate's tests at once can
/// leave the asking thread unscheduled past [`BUDGET`], and a test that then heard
/// "silence" was failing on the runner's load, not on what the bus said (main, run
/// 36772854804). Silence is still heard within [`BUDGET`]; see the test that times it.
#[cfg(target_os = "linux")]
const AN_ANSWER_WITHIN: Duration = Duration::from_secs(10);

#[cfg(target_os = "linux")]
impl PrivateBus {
    /// The names this bus can start, as the bus itself lists them.
    fn activatable(&self) -> Vec<String> {
        let bus = zbus::blocking::connection::Builder::address(self.address.as_str())
            .expect("an address")
            .build()
            .expect("the test's bus");
        let reply = bus
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "ListActivatableNames",
                &(),
            )
            .expect("the bus lists what it can start");
        reply.body().deserialize().expect("a list of names")
    }
}

#[cfg(target_os = "linux")]
impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

/// charter-app#24's machine, in miniature: the portal is activatable, and what the bus
/// starts for it never takes the name — as `xdg-desktop-portal` does not, while it waits
/// on a backend that needs a desktop session. GTK waited 25 s on exactly this.
#[cfg(target_os = "linux")]
#[test]
fn a_portal_the_bus_cannot_bring_up_is_heard_as_silence_within_the_budget() {
    let bus = PrivateBus::with(&[(PORTAL, "/bin/sleep 60")]);

    let from = std::time::Instant::now();
    let heard = listen(Some(&bus.address), BUDGET);
    let took = from.elapsed();

    assert_eq!(heard, Heard::Silence);
    // Well short of the 25 s it replaces, with room for a loaded runner's scheduling.
    assert!(
        took < BUDGET + Duration::from_secs(2),
        "listening took {took:?} against a {BUDGET:?} budget"
    );
}

/// The premise every test below stands on: the bus a test starts can start what that test
/// declares and nothing else — whatever portals the machine running the tests has
/// installed. Its config names one service directory, its own, and no standard ones.
#[cfg(target_os = "linux")]
#[test]
fn a_test_bus_can_start_only_what_its_test_declares() {
    assert!(
        !PrivateBus::with(&[])
            .activatable()
            .iter()
            .any(|name| name == PORTAL),
        "the machine's own portal reached a test's bus"
    );
    assert!(
        PrivateBus::with(&[(PORTAL, "/bin/false")])
            .activatable()
            .iter()
            .any(|name| name == PORTAL),
        "a declared service is not activatable"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_bus_with_no_portal_to_start_answers() {
    let bus = PrivateBus::with(&[]);

    assert_eq!(listen(Some(&bus.address), AN_ANSWER_WITHIN), Heard::Answer);
}

/// A portal that exits instead of hanging: the bus says it failed as soon as it has.
#[cfg(target_os = "linux")]
#[test]
fn a_portal_that_fails_to_start_is_answered_for_by_the_bus() {
    let bus = PrivateBus::with(&[(PORTAL, "/bin/false")]);

    assert_eq!(listen(Some(&bus.address), AN_ANSWER_WITHIN), Heard::Answer);
}

#[cfg(target_os = "linux")]
#[test]
fn a_running_portal_answers() {
    let bus = PrivateBus::with(&[]);
    // Someone owns the name, as `xdg-desktop-portal` does on a working desktop.
    let _portal = zbus::blocking::connection::Builder::address(bus.address.as_str())
        .expect("an address")
        .name(PORTAL)
        .expect("a name")
        .build()
        .expect("the portal is on the bus");

    assert_eq!(listen(Some(&bus.address), AN_ANSWER_WITHIN), Heard::Answer);
}

#[cfg(target_os = "linux")]
#[test]
fn no_bus_at_the_address_is_heard_as_no_bus() {
    assert_eq!(listen(Some(NO_SESSION_BUS), BUDGET), Heard::NoBus);
}
