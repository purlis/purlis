// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The desktop portal, asked before GTK asks it (charter-app#24).
//!
//! On a Linux session whose D-Bus session bus can *activate* `org.freedesktop.portal.Desktop`
//! but whose portal cannot come up — CI runners, containers with desktop packages, a remote X
//! session or a bare tiling window manager with no desktop session behind it — a launch used to
//! take 26 to 31 seconds. GTK, inside `tauri::Builder::build()`, creates a proxy for the portal
//! with auto-start on and waits out D-Bus's 25 s default; WebKitGTK then asks the same portal
//! for the colour scheme and waits out its own 5 s. charter makes neither call and cannot
//! switch either off (ADR 0026, amended).
//!
//! What charter CAN do is ask first, and briefly. Before anything touches GTK, [`listen`]
//! makes the call GTK's proxy makes — it asks the session bus to start the portal — and
//! gives it [`BUDGET`]. The bus answers "already running" at once when the portal is up, and
//! "no such service" at once when nothing can start it. Only a portal the bus is still trying
//! to start leaves it silent — and that silence is the 25 s GTK is about to wait out.
//!
//! When it is silent, the app starts again, in place (`exec`, same process), with
//! `DBUS_SESSION_BUS_ADDRESS` pointing at nothing. A machine with no session bus starts in
//! under 0.6 s, because every bus call fails at once. The bus itself is healthy, so the
//! address it had is kept in [`SESSION_BUS_KEPT`] and every chat gets that one
//! (`purlis_core::chatenv`), as does every program the app starts (`forklock::spawn`). What
//! the run loses — the tray, notifications, a keyring vault from the window, and handing a
//! second launch over — is said on standard error and in the window ([`SessionBus`]);
//! `instance.rs` is what keeps two apps off one plane without the bus. After the launch the
//! kept bus is asked again, backing off, until it answers (`listen_again`), and then the
//! window offers to start again on it. It never does so by itself.
//!
//! **Once per login session.** A silence is remembered in `$XDG_RUNTIME_DIR` until the next
//! login, or until the kept bus answers, so only the session's first launch pays [`BUDGET`] for
//! it ([`LoginSession`], #2432).
//!
//! **X11 without a session bus.** With no `DBUS_SESSION_BUS_ADDRESS` and no
//! `$XDG_RUNTIME_DIR/bus` — `startx` into i3, say — GIO does not give up: it runs
//! `dbus-launch --autolaunch` and uses the bus the X display holds, starting one if there is
//! none. A notification daemon started from the window manager's config is on that bus. GTK
//! would find it and zbus — which the tray, notifications and single-instance use — would not,
//! so the launch looks for it the way GIO does, asks the portal there, and starts again on it
//! ([`Start::OnTheBus`]). Measured in a container (Ubuntu 24.04, i3 on Xvfb,
//! `xdg-desktop-portal` and its GTK backend installed): `dbus-launch` answered in 11 ms, and
//! the portal it could activate there came up in 114–202 ms. Each gets [`BUDGET`], so this path
//! costs at most about 600 ms and never hangs. Only when there is no bus to be had, or its
//! portal is silent too, does the launch start without one; the `dbus-daemon` `dbus-launch`
//! started then stays with the X display, as GIO leaves it.

use std::time::Duration;

pub use purlis_core::chatenv::{NO_SESSION_BUS, SESSION_BUS, SESSION_BUS_KEPT};

/// How long the bus is given to answer. For a running portal it answers in milliseconds;
/// the ticket (FR-8) and ADR 0026's 2 s cold-start limit leave room for this much and no more.
pub const BUDGET: Duration = Duration::from_millis(300);

/// How long after the launch — and after each ask that went unanswered — the kept bus is asked
/// again, and how long it is given each time. A portal that was slow at the launch (right after
/// login, a cold disk) has come up by the first; one still silent after the last is the one the
/// launch was right not to wait for, and it is left alone.
pub const ASKED_AGAIN_AFTER: [Duration; 3] = [
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(45),
];
pub const ASKED_AGAIN_FOR: Duration = Duration::from_secs(5);

/// The name GTK and WebKitGTK ask for.
const PORTAL: &str = "org.freedesktop.portal.Desktop";

/// What the session bus said when it was asked to start the portal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    /// A reply of any kind inside the budget: the portal answered, or the bus answered that it
    /// cannot start it. GTK's own call will be answered as quickly.
    Answer,
    /// Nothing inside the budget: the bus is still starting a portal that is not coming up.
    Silence,
    /// No session bus could be reached at all.
    NoBus,
}

/// How this launch goes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// With the session bus as it found it.
    AsItIs,
    /// Again, in place, on the bus at this address: the X display's, which GIO would have
    /// used and zbus cannot find.
    OnTheBus(String),
    /// Again, in place, with the session bus pointed at [`NO_SESSION_BUS`]. `kept` is the bus
    /// a chat gets instead ([`SESSION_BUS_KEPT`]): empty when there is none to give, or when a
    /// chat finds it by the standard lookup. Why is said by the launch it starts
    /// ([`SessionBus::say`]), not before it: the app's log holds its first line until its
    /// redaction rules are built, about 300 ms of a debug build, which a launch that skips the
    /// ask ([`LoginSession`]) would pay instead.
    WithoutTheBus { kept: String },
}

/// What a launch without the bus loses, in the words the window and standard error both use.
///
/// The keyring is named because the window's own reveal and copy of a keyring vault go through
/// the Secret Service on the session bus, in this process, which has none; a chat, and every
/// program the app starts, is given the bus and still reaches it.
const WHAT_IS_OFF: &str = "For this run there is no tray icon and no desktop notifications, \
     a keyring vault cannot be opened from the window (chats still reach the keyring), and a \
     second launch is refused instead of being handed to this one.";

/// What a launch does about what it heard.
///
/// `address` is `DBUS_SESSION_BUS_ADDRESS` as this process was given it, and `heard` what the
/// bus the standard lookup finds said. `x_session_bus` looks for the bus GIO would autolaunch
/// on the X display and asks its portal; it is called only when nothing else is there.
pub fn decide(
    address: Option<&str>,
    heard: Heard,
    x_session_bus: &mut dyn FnMut() -> Option<(String, Heard)>,
) -> Start {
    // A launch that is already the restart never restarts again, whatever it hears.
    if address == Some(NO_SESSION_BUS) {
        return Start::AsItIs;
    }
    match heard {
        Heard::Answer => Start::AsItIs,
        // Found by the standard lookup when nothing named it, which a chat does as well.
        Heard::Silence => without_the_bus_keeping(address.unwrap_or_default()),
        // An address that names a dead bus already fails fast, in GIO too.
        Heard::NoBus if address.is_some() => Start::AsItIs,
        Heard::NoBus => match x_session_bus() {
            Some((bus, Heard::Answer)) => Start::OnTheBus(bus),
            Some((bus, Heard::Silence)) => without_the_bus_keeping(&bus),
            Some((_, Heard::NoBus)) | None => without_the_bus_keeping(""),
        },
    }
}

/// The login session's memory of a silent portal (#2432, ADR 0026 as amended 2026-10-17).
///
/// A portal that was silent at one launch is silent at the next on most machines that have one
/// (CI runners, containers, a bare window manager), and each launch would pay [`BUDGET`] to hear
/// it again. So the first silence in a login session is written down in `$XDG_RUNTIME_DIR`,
/// which is per login and emptied at logout, and later launches in the session start without
/// the bus at once. A portal that comes up later is still found: `listen_again` forgets the
/// silence when the kept bus answers, and the next login starts over. With no runtime directory
/// nothing is remembered, and every launch asks.
///
/// What is written is the address the silence was heard on (empty for the bus the standard
/// lookup finds), and nothing else: the bus a chat is given is never read back from the file.
#[derive(Debug, Clone)]
pub struct LoginSession {
    marker: Option<std::path::PathBuf>,
}

impl LoginSession {
    /// The login session whose runtime directory is `runtime` (`$XDG_RUNTIME_DIR`).
    pub fn at(runtime: Option<&std::path::Path>) -> Self {
        Self {
            marker: runtime.map(|dir| {
                dir.join(format!(
                    "{}.portal-silent",
                    purlis_core::names::BUNDLE_ID.write
                ))
            }),
        }
    }

    /// This login session's, from the environment.
    pub fn this_one() -> Self {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|dir| !dir.is_empty())
            .map(std::path::PathBuf::from);
        Self::at(runtime.as_deref())
    }

    /// Whether the portal on `address` was silent earlier in this session.
    fn was_silent(&self, address: Option<&str>) -> bool {
        self.marker
            .as_ref()
            .and_then(|marker| std::fs::read_to_string(marker).ok())
            .is_some_and(|heard_on| heard_on == address.unwrap_or_default())
    }

    /// The portal on `address` was silent. A marker that cannot be written costs only the next
    /// launch's [`BUDGET`].
    fn remember_silence(&self, address: Option<&str>) {
        if let Some(marker) = &self.marker {
            let _ = std::fs::write(marker, address.unwrap_or_default());
        }
    }

    /// The portal answers after all: the next launch asks it again.
    pub fn forget(&self) {
        if let Some(marker) = &self.marker {
            let _ = std::fs::remove_file(marker);
        }
    }
}

/// [`decide`], asking the portal only when `session` does not remember it silent, and
/// remembering a silence on the bus the launch was given or the standard lookup found. `ask`
/// asks that bus's portal.
pub fn launch(
    address: Option<&str>,
    session: &LoginSession,
    ask: &mut dyn FnMut() -> Heard,
    x_session_bus: &mut dyn FnMut() -> Option<(String, Heard)>,
) -> Start {
    if address == Some(NO_SESSION_BUS) {
        return Start::AsItIs;
    }
    if session.was_silent(address) {
        return without_the_bus_keeping(address.unwrap_or_default());
    }
    let heard = ask();
    if heard == Heard::Silence {
        session.remember_silence(address);
    }
    decide(address, heard, x_session_bus)
}

fn without_the_bus_keeping(kept: &str) -> Start {
    Start::WithoutTheBus {
        kept: kept.to_owned(),
    }
}

/// The address in what `dbus-launch --sh-syntax` prints, when it printed one.
pub fn autolaunched_address(printed: &str) -> Option<String> {
    printed.lines().find_map(|line| {
        let value = line
            .strip_prefix(SESSION_BUS)?
            .strip_prefix("='")?
            .strip_suffix("';")?;
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// What came of asking something on a thread of its own, within a budget ([`in_time`]).
#[cfg(target_os = "linux")]
pub(crate) enum Asked<T> {
    /// It answered in time.
    Answered(T),
    /// No answer in time. The thread still waiting is left behind, and ends with the process.
    OutOfTime,
    /// There was no thread to ask on, or it ended without answering.
    NoThread,
}

/// Runs `ask` on a thread of its own and gives it `budget`.
#[cfg(target_os = "linux")]
pub(crate) fn in_time<T: Send + 'static>(
    budget: Duration,
    ask: impl FnOnce() -> T + Send + 'static,
) -> Asked<T> {
    use std::sync::mpsc::{self, RecvTimeoutError};

    let (said, heard) = mpsc::channel();
    let asking = std::thread::Builder::new()
        .name("charter-portal-probe".into())
        .spawn(move || {
            let _ = said.send(ask());
        });
    if asking.is_err() {
        return Asked::NoThread;
    }
    match heard.recv_timeout(budget) {
        Ok(answer) => Asked::Answered(answer),
        Err(RecvTimeoutError::Timeout) => Asked::OutOfTime,
        Err(RecvTimeoutError::Disconnected) => Asked::NoThread,
    }
}

/// Asks the session bus to start the portal — the bus at `address`, or the one this process
/// would find by the standard lookup when `None` — and says what came back within `budget`.
///
/// It is the call GTK's proxy makes, so it waits on exactly what GTK would. The budget covers
/// reaching the bus as well: a bus that does not even accept a connection in time is as silent
/// as a portal.
#[cfg(target_os = "linux")]
pub fn listen(address: Option<&str>, budget: Duration) -> Heard {
    let address = address.map(str::to_owned);
    match in_time(budget, move || start_the_portal(address.as_deref())) {
        Asked::Answered(heard) => heard,
        Asked::OutOfTime => Heard::Silence,
        // No thread to ask on: go on as if it answered, which is what every launch did before.
        Asked::NoThread => Heard::Answer,
    }
}

#[cfg(target_os = "linux")]
fn start_the_portal(address: Option<&str>) -> Heard {
    use zbus::blocking::connection::Builder;

    let builder = match address {
        Some(address) => Builder::address(address),
        None => Builder::session(),
    };
    let Ok(bus) = builder.and_then(Builder::build) else {
        return Heard::NoBus;
    };
    // The call GTK's proxy makes (the bus trace in ADR 0026 shows it), and one the BUS
    // answers rather than the portal: "already running" at once when the portal is up,
    // `ServiceUnknown` at once when nothing can start it, "started" as soon as it has taken its
    // name. Any reply is an answer. Only no reply is silence, and that is the caller's to time.
    let _ = bus.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "StartServiceByName",
        &(PORTAL, 0u32),
    );
    Heard::Answer
}

/// The machine's id, from the first of `places` that holds one, as GIO reads it for
/// `--autolaunch`. A file that is there and empty — a container's `/etc/machine-id` often
/// is — holds none.
fn machine_id(places: &[&std::path::Path]) -> Option<String> {
    places.iter().find_map(|path| {
        let id = std::fs::read_to_string(path).ok()?;
        let id = id.trim();
        (!id.is_empty()).then(|| id.to_owned())
    })
}

/// The bus GIO would autolaunch on this X display, found or started the way GIO does it —
/// `dbus-launch --autolaunch=<machine id>`, which hands back the bus the display already holds
/// — and what its portal said. `None` with no X display, no machine id or no `dbus-launch`.
#[cfg(target_os = "linux")]
fn the_x_sessions_bus() -> Option<(String, Heard)> {
    std::env::var_os("DISPLAY").filter(|display| !display.is_empty())?;
    let machine = machine_id(&[
        std::path::Path::new("/etc/machine-id"),
        std::path::Path::new("/var/lib/dbus/machine-id"),
    ])?;
    let Asked::Answered(Ok(printed)) = in_time(BUDGET, move || {
        purlis_core::forklock::output(
            std::process::Command::new("dbus-launch")
                .arg(format!("--autolaunch={machine}"))
                .args(["--sh-syntax", "--close-stderr"])
                .stdin(std::process::Stdio::null()),
        )
    }) else {
        return None;
    };
    if !printed.status.success() {
        return None;
    }
    let bus = autolaunched_address(&String::from_utf8_lossy(&printed.stdout))?;
    let heard = listen(Some(&bus), BUDGET);
    Some((bus, heard))
}

/// Asks the portal, and when it is silent starts this launch again without the session bus.
/// A portal silent earlier in this login session is not asked again ([`LoginSession`]).
///
/// Returns when the launch goes on as it is. When it restarts, it does not return: `exec`
/// replaces this process with the same binary, the same arguments and the same process id,
/// and the only difference is the session bus. A restart that cannot happen is said on
/// standard error and the launch goes on, slow, as it always did.
///
/// `reached` is the launch log's: the restart is a step of its own, said before the `exec`.
#[cfg(target_os = "linux")]
pub fn start_clear_of_a_silent_portal(reached: impl Fn(&str)) {
    use std::os::unix::process::CommandExt;

    let address = std::env::var(SESSION_BUS).ok();
    if address.as_deref() == Some(NO_SESSION_BUS) {
        return;
    }
    let again = match launch(
        address.as_deref(),
        &LoginSession::this_one(),
        &mut || listen(None, BUDGET),
        &mut the_x_sessions_bus,
    ) {
        Start::AsItIs => return,
        Start::OnTheBus(bus) => Bus::At(bus),
        Start::WithoutTheBus { kept } => Bus::Without { kept },
    };
    reached(&again.starting_again());
    let failed = match this_launch_again(&again) {
        Ok(mut command) => command.exec(),
        Err(err) => err,
    };
    tracing::warn!("purlis: could not start again ({failed}); going on with the bus as it was.");
}

/// Which session bus a launch of this binary is started on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bus {
    /// This one; nothing kept aside.
    At(String),
    /// The default one, which the standard lookup finds at `$XDG_RUNTIME_DIR/bus`: no address
    /// named, nothing kept aside.
    Default,
    /// None, with `kept` kept aside for the chats.
    Without { kept: String },
}

impl Bus {
    /// The launch step a restart in place onto this bus is (`PURLIS_LAUNCH_LOG`). The launch
    /// log shows `run() entered` again after it, and the time before it is the time the portal
    /// was given ([`BUDGET`]): on a bus whose portal is silent, that is what a launch pays
    /// before the app's clock starts again (#2389).
    pub fn starting_again(&self) -> String {
        let onto = match self {
            Self::At(address) => format!("on the session bus at {address}"),
            Self::Default => "on the session bus the standard lookup finds".to_owned(),
            Self::Without { .. } => "without the session bus".to_owned(),
        };
        format!("the launch starts again in place, {onto}")
    }

    /// The address to ask this bus's portal at: `runtime` is `$XDG_RUNTIME_DIR`, where the
    /// default one is looked for. `None` for no bus, and for a default one that is not there.
    pub fn where_to_ask(&self, runtime: Option<&std::path::Path>) -> Option<String> {
        match self {
            Self::At(address) => Some(address.clone()),
            Self::Default => runtime
                .map(|dir| dir.join("bus"))
                .filter(|bus| bus.exists())
                .map(|bus| format!("unix:path={}", bus.display())),
            Self::Without { .. } => None,
        }
    }
}

/// This binary with this launch's arguments, on `bus`.
pub fn this_launch_again(bus: &Bus) -> std::io::Result<std::process::Command> {
    let mut args = std::env::args_os();
    Ok(launch_on(std::env::current_exe()?, args.next(), args, bus))
}

/// `exe`, called `name`, with `args`, on `bus`.
fn launch_on(
    exe: std::path::PathBuf,
    name: Option<std::ffi::OsString>,
    args: impl IntoIterator<Item = std::ffi::OsString>,
    bus: &Bus,
) -> std::process::Command {
    let mut again = std::process::Command::new(exe);
    #[cfg(unix)]
    if let Some(name) = name {
        use std::os::unix::process::CommandExt;
        again.arg0(name);
    }
    #[cfg(not(unix))]
    let _ = name;
    again.args(args);
    // The kept bus under either of its names (V93k): an app started by an older one may
    // have inherited it as `CHARTER_SESSION_BUS_KEPT`.
    let forget_the_kept = |again: &mut std::process::Command| {
        for kept in purlis_core::envvar::spellings(SESSION_BUS_KEPT) {
            again.env_remove(kept);
        }
    };
    match bus {
        Bus::At(address) => {
            again.env(SESSION_BUS, address);
            forget_the_kept(&mut again);
        }
        Bus::Default => {
            again.env_remove(SESSION_BUS);
            forget_the_kept(&mut again);
        }
        Bus::Without { kept } => {
            again
                .env(SESSION_BUS, NO_SESSION_BUS)
                .env(SESSION_BUS_KEPT, kept);
        }
    }
    again
}

/// What the window is told about a launch without the session bus.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BusNotice {
    /// The line it draws: why there is no bus, and what is off for the run.
    pub says: String,
    /// Whether the bus answers now, so a restart onto it is worth offering.
    pub can_restart: bool,
}

/// This launch and the session bus, as the window is told about it (`session_bus`) and as the
/// ask after the launch finds it (`listen_again`).
#[derive(Debug)]
pub struct SessionBus {
    /// The line the window draws, when the launch is without the bus.
    says: Option<String>,
    /// What the app's log says besides: where a launch with no bus at all looked for one.
    looked: Option<&'static str>,
    /// The bus to ask again and to go back onto: `At` the kept address, or the `Default` one.
    /// `None` when there is nothing to go back onto.
    back: Option<Bus>,
    /// `$XDG_RUNTIME_DIR`, where the default bus is asked.
    runtime: Option<std::path::PathBuf>,
    /// Whether it answered when it was asked again.
    answers: std::sync::atomic::AtomicBool,
    /// The bus the operator asked to start again on, once they have.
    restart: std::sync::Mutex<Option<Bus>>,
}

impl SessionBus {
    /// `address` and `kept` as this process was given `DBUS_SESSION_BUS_ADDRESS` and
    /// [`SESSION_BUS_KEPT`]; `runtime` is `$XDG_RUNTIME_DIR`, where the standard lookup looks.
    pub fn of(
        address: Option<&str>,
        kept: Option<&str>,
        runtime: Option<&std::path::Path>,
    ) -> Self {
        if address != Some(NO_SESSION_BUS) {
            return Self {
                says: None,
                looked: None,
                back: None,
                runtime: None,
                answers: false.into(),
                restart: std::sync::Mutex::default(),
            };
        }
        let kept = kept.unwrap_or_default();
        let back = if kept.is_empty() {
            Some(Bus::Default).filter(|bus| bus.where_to_ask(runtime).is_some())
        } else {
            Some(Bus::At(kept.to_owned()))
        };
        let looked = back.is_none().then_some(
            "It looked for one named in DBUS_SESSION_BUS_ADDRESS, at $XDG_RUNTIME_DIR/bus and on \
             the X display.",
        );
        let why = if back.is_some() {
            format!(
                "purlis started without the session bus: the desktop portal did not answer \
                 on it within {} ms, and waiting would have cost half a minute.",
                BUDGET.as_millis()
            )
        } else {
            "purlis found no session bus to start on, so this run has none.".to_owned()
        };
        Self {
            says: Some(format!("{why} {WHAT_IS_OFF}")),
            looked,
            back,
            runtime: runtime.map(std::path::Path::to_path_buf),
            answers: false.into(),
            restart: std::sync::Mutex::default(),
        }
    }

    /// Says on standard error and in the app's log why this launch is without the bus, as the
    /// window does: nothing on a launch with it. The launch before the restart says nothing,
    /// so this is said once ([`Start::WithoutTheBus`]).
    pub fn say(&self) {
        self.say_to(&mut |line| tracing::warn!("{line}"));
    }

    /// [`say`](Self::say), with where it is said handed in so a test can hear it.
    fn say_to(&self, say: &mut dyn FnMut(&str)) {
        let Some(says) = &self.says else {
            return;
        };
        match self.looked {
            Some(looked) => say(&format!("{says} {looked}")),
            None => say(says),
        }
    }

    /// What the window draws: nothing on a launch with the bus.
    pub fn now(&self) -> Option<BusNotice> {
        self.says.as_ref().map(|says| BusNotice {
            says: says.clone(),
            can_restart: self.back_onto().is_some(),
        })
    }

    /// The address to ask again after the launch, when there is a bus to go back onto.
    pub fn where_to_ask_again(&self) -> Option<String> {
        self.back
            .as_ref()
            .and_then(|bus| bus.where_to_ask(self.runtime.as_deref()))
    }

    /// The bus answered when it was asked again.
    pub fn answered(&self) {
        self.answers
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// The bus a restart goes back onto: only one that has answered.
    pub fn back_onto(&self) -> Option<Bus> {
        self.back
            .clone()
            .filter(|_| self.answers.load(std::sync::atomic::Ordering::SeqCst))
    }

    /// The operator asked to start again on the bus: refused unless it has answered.
    pub fn restart_asked(&self) -> Result<(), String> {
        let onto = self.back_onto().ok_or_else(|| {
            "the session bus has not answered since the launch, so there is nothing to start \
             again on"
                .to_owned()
        })?;
        *self
            .restart
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(onto);
        Ok(())
    }

    /// The bus to start again on at `Exit`, when the operator asked for it.
    pub fn restart_wanted(&self) -> Option<Bus> {
        self.restart
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// The event the window hears when the kept bus answers after the launch.
pub const ANSWERS: &str = "session-bus://answers";

/// Asks the kept bus again, off the main thread, after each of [`ASKED_AGAIN_AFTER`]'s waits
/// until it answers: a portal that was only slow — right after login, a cold disk — has come up
/// by then. When it answers the login session forgets its silence, so the next launch asks it
/// again, and the window is told, and offers the restart; nothing restarts by itself.
#[cfg(target_os = "linux")]
pub fn listen_again(app: &tauri::AppHandle) {
    use tauri::{Emitter, Manager};

    let Some(address) = app.state::<SessionBus>().where_to_ask_again() else {
        return;
    };
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("charter-portal-asked-again".into())
        .spawn(move || {
            let answered =
                ask_until_answered(&ASKED_AGAIN_AFTER, &mut std::thread::sleep, &mut || {
                    listen(Some(&address), ASKED_AGAIN_FOR)
                });
            if !answered {
                return;
            }
            tracing::info!(
                "purlis: the desktop portal answers on the session bus now; the window offers \
                 to restart on it."
            );
            LoginSession::this_one().forget();
            let bus = app.state::<SessionBus>();
            bus.answered();
            if let Some(now) = bus.now() {
                let _ = app.emit(ANSWERS, now);
            }
        });
}

/// What the window says about this launch and the session bus: nothing, on a launch with it.
#[tauri::command]
#[specta::specta]
pub fn session_bus(bus: tauri::State<'_, SessionBus>) -> Option<BusNotice> {
    bus.now()
}

/// Quit, and start again on the session bus, now that it answers. The operator's choice,
/// from the window's notice: charter never does this by itself, because a restart ends every
/// chat. It goes the way a quit goes — every plane writes what was open, so the launch after
/// it offers them back — and the new launch is started last, at `Exit` ([`restart_if_asked`]).
#[tauri::command]
#[specta::specta]
pub fn restart_on_the_session_bus(
    app: tauri::AppHandle,
    bus: tauri::State<'_, SessionBus>,
) -> Result<(), String> {
    bus.restart_asked()?;
    app.exit(0);
    Ok(())
}

/// Waits each of `waits` in turn and asks after it, until an ask is answered. Whether one was.
pub fn ask_until_answered(
    waits: &[Duration],
    wait: &mut dyn FnMut(Duration),
    ask: &mut dyn FnMut() -> Heard,
) -> bool {
    waits.iter().any(|&after| {
        wait(after);
        ask() == Heard::Answer
    })
}

/// At `Exit`, after everything else has let go: the launch the operator asked for, if any.
pub fn restart_if_asked(bus: &SessionBus) {
    restart_if_asked_with(bus, &mut |onto| {
        this_launch_again(onto)
            .and_then(|mut again| purlis_core::forklock::spawn(&mut again).map(drop))
    });
}

/// [`restart_if_asked`], with the start handed in so a test can watch it.
fn restart_if_asked_with(bus: &SessionBus, start: &mut dyn FnMut(&Bus) -> std::io::Result<()>) {
    let Some(onto) = bus.restart_wanted() else {
        return;
    };
    if let Err(why) = start(&onto) {
        tracing::warn!(
            "purlis: could not start again on the session bus ({why}); start purlis again"
        );
    }
}

#[cfg(test)]
mod tests;
