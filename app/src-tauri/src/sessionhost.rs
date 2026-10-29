// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session host the window starts, and is the first client of (ADR 0068 §1, #1997).
//!
//! **The window's own binary runs the host.** The app starts itself again with
//! [`purlis_host::SERVE_FOR_THE_WINDOW`] (`main.rs` answers it before anything of the app
//! starts), so the host is the same program, with the same Keychain identity, and nothing
//! prompts (ADR 0068 §1, amended 2026-10-14). One host serves every project open in the window.
//!
//! **It outlives a window that crashes, for the grace period** (ADR 0068 §2, #2006). A quit
//! says so to the host (`quit`), which ends every chat as a quit and stops
//! ([`SessionHost::end`]). A window that goes without a word crashed or was killed: the host
//! keeps its chats for ten minutes, and the next window that starts finds it serving and
//! reattaches to it, rather than starting another ([`ANOTHER_SERVES`]). The host runs in a
//! process group of its own, so a signal to the window's group (an interrupt typed in the
//! terminal purlis was started from) does not end it with the window.
//!
//! **The window supervises it** (ADR 0068 §3, #2007): a host that ends without the window
//! asking, or whose link to the window ends, is started again, until three such ends within
//! five minutes trip the breaker ([`breaker`]); then the window says the host is failing, with
//! its last lines and Try again (`crate::hostfailing`). Every chat the host ran ends with it and
//! comes back as failed or hibernated with Resume (`crate::hostlost`), and the new host settles
//! what the last one left from its host record (the core's
//! [`purlis_core::hookchannel::EventLog`], #2494).
//!
//! **Its folder is made first, the host is started off the first paint.** The socket's folder
//! is made this user's alone on the window's own start, before any chat can start, so every
//! chat's sandbox denies a folder that is there; the host then starts on a thread of its own,
//! and nothing the window draws waits for it. Once it says it is ready, the window connects as `local-ui` with the
//! credential the host minted, and holds the link.
//!
//! **Chats run in it behind the start-time switch** (#1998, `crate::hostedsessions`): when the
//! switch is on, each project root's chats run over a link of its own, opened as the window's
//! is (#2178), and with no host of the window's own, those chats are refused with the reason
//! ([`Connection`]). Wherever
//! chats run, the host takes every process below the window as inside a chat and refuses it a
//! person's scope (`purlis_host::Window`). What this host says goes to the window's log.
//!
//! **While chats run in it, it writes the event log** (#2003): the core's
//! [`purlis_core::hookchannel::EventLog`], which `purlis serve` keeps too (#2494). It opens the
//! log as it first holds a project, which only a window running its chats here asks, records each
//! line its hook channels hear with what the window's hearing made of it, drains their spools,
//! writes what the window decided, and keeps what each program's end leaves, window or no window.
//! The window then opens no log at all.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use purlis_session_protocol::auth::{Credential, Scope};
use purlis_session_protocol::{link, session};

use crate::hostedsessions::{Connection, HostLink};

/// How long the window waits for its host to say it is ready.
const READY_WITHIN: Duration = Duration::from_secs(20);

/// How long a quit waits for the host to end once its standard input is closed, before it is
/// killed.
const ENDS_WITHIN: Duration = Duration::from_secs(2);

/// How long a quit waits for the host to say every chat has ended: the host gives its programs
/// five seconds together (`purlis_host::ENDS_WITHIN`).
const QUITS_WITHIN: Duration = Duration::from_secs(7);

/// The exit code a host gives when another host of this user already serves on this device
/// (`purlis_host::HostError::AlreadyServing`).
const ANOTHER_SERVES: i32 = 3;

/// Where this user's session host listens and keeps its credentials on this device: the place
/// the window's host takes and `purlis serve` takes, so each finds the other.
pub fn place() -> Option<purlis_host::Place> {
    let config = purlis_core::machine::config_root()?;
    Some(purlis_host::Place::at(
        purlis_core::hostsocket::here(),
        purlis_core::machine::charterd(&config),
    ))
}

/// The command that starts `binary`, the window's own, as its session host: the host's
/// standard input is the window's to hold, and what it says is read for the window's log. It
/// runs in a process group of its own, so it outlives a window that crashes (#2006).
pub fn command(binary: &Path) -> Command {
    use std::os::unix::process::CommandExt as _;
    let mut command = Command::new(binary);
    command
        .arg(purlis_host::SERVE_FOR_THE_WINDOW)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    command
}

/// What the window does with a host it started that did not become ready, by its exit.
#[derive(Debug, PartialEq, Eq)]
pub enum NotReady {
    /// Another host of this user serves on this device: one a window that crashed left, within
    /// its grace period. This window reattaches to it (#2006).
    AnotherServes,
    /// There is no host, for this reason.
    Gone(&'static str),
}

/// What a host that did not become ready said by its exit.
pub fn not_ready(code: Option<i32>) -> NotReady {
    match code {
        Some(ANOTHER_SERVES) => NotReady::AnotherServes,
        _ => NotReady::Gone("the session host ended before it was ready"),
    }
}

/// **How the window's host composes each start** (#2174, #2171): the window's launch by its
/// words, or, for a start with no launch, the launch the window would compose, from the
/// project's own files, with what this app ships ([`purlis_core::hoststart::spec`]).
pub struct Composes(pub purlis_core::hoststart::Kit);

impl Composes {
    /// With what this app ships, found as the host starts: its `purlis`, its bundled plugin and
    /// the git hooks the window wrote at its launch.
    pub fn shipped() -> Composes {
        let binary = crate::charter_binary();
        let plugin = crate::bundled_plugin_here();
        Composes(purlis_core::hoststart::Kit::found(binary, plugin))
    }

    /// With what a test's build ships ([`crate::Shipped::default`]): a stand-in `purlis` and an
    /// empty plugin.
    #[cfg(test)]
    pub fn as_the_tests_ship() -> Composes {
        let shipped = crate::Shipped::default();
        Composes(purlis_core::hoststart::Kit {
            binary: shipped.binary,
            plugin: shipped.plugin,
            git_hooks: shipped.git_hooks,
            skill_sets: None,
            chat_settings: None,
        })
    }
}

impl purlis_host::Compose for Composes {
    fn compose(
        &self,
        starting: &purlis_host::ToCompose<'_>,
    ) -> Result<purlis_core::session::Spec, String> {
        let beside = starting.beside;
        purlis_core::hoststart::spec(
            &purlis_core::hoststart::Starting {
                root: starting.project_path,
                number: starting.number,
                start: starting.start,
                reporting: starting.reporting,
                started_by: starting.started_by.word(),
            },
            starting.size,
            &self.0,
            &(
                |plan: &session::ConfinePlan| beside.confine(plan),
                |entry| beside.record(entry),
                // A chat's router extension, asked at its start with this program's own
                // executor (SK-15b).
                std::sync::Arc::new(
                    purlis_core::skills::router::OnThisMachine::with_this_programs(
                        starting.project_path,
                    ),
                ) as std::sync::Arc<dyn purlis_core::skills::router::AskRouter>,
                // The chat's confinement as it is now, which its tool's runs here are held to
                // (#3407).
                |confinement: &str| beside.reach_of(confinement),
            ),
        )
    }

    /// How the run of `spec` is sandboxed, read by the core from what is started (#2636).
    fn sandbox_of(
        &self,
        spec: &purlis_core::session::Spec,
        confined: bool,
    ) -> purlis_host::protocol::session::RunSandbox {
        purlis_core::sandbox::ran::applied_to(
            &spec.program,
            &spec.args,
            confined,
            purlis_core::sandbox::Os::this(),
        )
    }
}

/// **What hears each held project's hooks in the window's session host** (#2000): the core's
/// hook channels ([`purlis_core::hookchannel::HookChannels`]), which `purlis serve` hears with
/// too (#2447), recording in the event log this host keeps
/// ([`purlis_core::hookchannel::EventLog`]), each chat's spool looked at as the window's own
/// hearing looks at it.
pub fn hook_channels(
    log: &purlis_core::hookchannel::EventLog,
) -> purlis_core::hookchannel::HookChannels {
    purlis_core::hookchannel::HookChannels::keeping(log.clone())
        .looking_at_spools_after(crate::hooks::A_SPOOL_IS_LOOKED_AT_AFTER)
}

/// The window's session host, while it runs, and its supervisor (#2007).
#[derive(Default)]
pub struct SessionHost {
    held: Arc<Mutex<Option<Held>>>,
    /// The link to it, for the chats that run in it.
    connection: Arc<Connection>,
    supervision: Arc<Supervision>,
}

struct Held {
    child: Child,
    /// The host's standard input: closing it tells the host this window has gone.
    stdin: Option<ChildStdin>,
}

/// **The window supervises its host** (ADR 0068 §3, #2007): no service manager may be used
/// for a host that is not a login item, so the window is the one that starts it again. What the
/// supervisor shares with the window's other threads.
#[derive(Default)]
struct Supervision {
    /// The window is ending its host: nothing starts it again.
    ending: std::sync::atomic::AtomicBool,
    /// What wakes the supervisor.
    woken: Mutex<Woken>,
    changed: std::sync::Condvar,
    breaker: Mutex<breaker::Breaker>,
    /// The last lines the host wrote to its log, oldest first.
    said: Mutex<std::collections::VecDeque<String>>,
    /// What the window says while the breaker holds the host stopped.
    failing: Arc<crate::hostfailing::HostFailing>,
    /// A host found serving is ended, not reattached to (#2004): the window runs its chats in
    /// its own process.
    ends_a_host_it_finds: std::sync::atomic::AtomicBool,
    /// A host found serving could not be ended, and may still run a window's chats (#2004).
    left_serving: std::sync::atomic::AtomicBool,
    /// Where the host serves: what the way back asks whether another host holds (#2004).
    place: Mutex<Option<purlis_host::Place>>,
}

#[derive(Default)]
struct Woken {
    /// The window's link to the host ended without the window ending it.
    link_lost: bool,
    /// The person pressed Try again.
    try_again: bool,
}

/// How many of the host's last lines the window keeps, to say them when it is failing.
const LINES_KEPT: usize = 20;

/// How often the supervisor looks at its host when nothing wakes it.
const LOOK_EVERY: Duration = Duration::from_millis(100);

impl Supervision {
    fn ending(&self) -> bool {
        self.ending.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The window is ending its host: nothing starts it again, and the supervisor is woken.
    fn end_now(&self) {
        self.ending.store(true, std::sync::atomic::Ordering::SeqCst);
        self.wake(|_| {});
    }

    fn wake(&self, how: impl FnOnce(&mut Woken)) {
        how(&mut self.woken.lock().unwrap_or_else(PoisonError::into_inner));
        self.changed.notify_all();
    }

    /// The link ended: a host the window cannot speak to is ended, and started again.
    fn link_lost(&self) {
        self.wake(|woken| woken.link_lost = true);
    }

    fn heard(&self, line: &str) {
        let mut said = self.said.lock().unwrap_or_else(PoisonError::into_inner);
        if said.len() == LINES_KEPT {
            said.pop_front();
        }
        said.push_back(line.to_owned());
    }

    /// Waits up to [`LOOK_EVERY`] for something to wake it, and answers what did.
    fn rest(&self) -> Woken {
        let woken = self.woken.lock().unwrap_or_else(PoisonError::into_inner);
        let (mut woken, _) = self
            .changed
            .wait_timeout_while(woken, LOOK_EVERY, |woken| {
                !woken.link_lost && !woken.try_again && !self.ending()
            })
            .unwrap_or_else(PoisonError::into_inner);
        std::mem::take(&mut *woken)
    }
}

impl SessionHost {
    /// Makes the host's socket folder this user's alone now, on the calling thread, so that
    /// every chat's sandbox, compiled from here on, denies a folder that is there; then starts
    /// the host on a thread of its own and connects to it once it is ready, and supervises it
    /// from there. Nothing waits for the host; what goes wrong is said in the log.
    pub fn start(&self) {
        let Some(place) = place() else {
            let why = "there is no home folder to keep its credentials in";
            tracing::warn!("purlis: no session host: {why}");
            self.connection.gone(why);
            return;
        };
        // Each project's link names the window by the name it keeps for it (#2623).
        if let Some(config) = purlis_core::machine::config_root() {
            self.connection.naming_in(config.clone());
            // Where a runner a chat ran on without the sandbox is kept as one (D-2050-27).
            self.connection.runners_in(config);
        }
        self.start_at(place, || {
            std::env::current_exe()
                .map(|binary| command(&binary))
                .map_err(|why| format!("this program's path is unknown: {why}"))
        });
    }

    /// [`SessionHost::start`], the host serving at `place` and started, each time, by the
    /// command `starts` gives, on the host's own thread: the window's own binary as [`command`]
    /// makes it, or, for the behaviour suite, that binary in an environment of the test's
    /// (#2170).
    pub(crate) fn start_at(
        &self,
        place: purlis_host::Place,
        starts: impl Fn() -> Result<Command, String> + Send + Sync + 'static,
    ) {
        *self
            .supervision
            .place
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(place.clone());
        let connection = &self.connection;
        if let Err(why) = purlis_core::hostsocket::make_the_folder(&place.socket) {
            let why = format!(
                "its folder {} could not be made this user's alone: {why}",
                place.socket.parent().unwrap_or(&place.socket).display()
            );
            tracing::warn!("purlis: no session host: {why}");
            connection.gone(why);
            return;
        }
        let held = Arc::clone(&self.held);
        let connection = Arc::clone(connection);
        let supervision = Arc::clone(&self.supervision);
        let try_again = Arc::clone(&self.supervision);
        self.supervision.failing.tried_again_by(Arc::new(move || {
            try_again.wake(|woken| woken.try_again = true)
        }));
        let spawned = std::thread::Builder::new()
            .name("session host".into())
            .spawn({
                let connection = Arc::clone(&connection);
                move || supervise(&held, &place, &starts, &connection, &supervision)
            });
        if let Err(why) = spawned {
            tracing::warn!("purlis: the session host was not started: {why}");
            connection.gone(format!("it was not started: {why}"));
        }
    }

    /// **A host found serving is ended, not reattached to** (#2004): for a window whose chats
    /// run in its own process (`PURLIS_CHATS_IN_THE_HOST=0`), which puts back each chat its
    /// record holds in that process. A host a window that crashed left serving still runs that
    /// window's chats for the grace period, so the window tells it `quit`, which ends each of
    /// them as a quit, and only then starts a host of its own and connects: a chat is never
    /// run by the host and the window at once. Said before [`Self::start`].
    pub fn ending_a_host_it_finds(&self) {
        self.supervision
            .ends_a_host_it_finds
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// **What a window whose chats run in its own process waits on before it puts any back**
    /// (#2004, with [`Self::ending_a_host_it_finds`]): its own host serving, at most `within`,
    /// so a host a crashed window left has ended its chats first. Its own host serving holds the
    /// place's lock, so no other host runs them. Short of that, it puts chats back only when it
    /// knows no other host serves there: none holds the place's lock, and none it found was left
    /// serving. A lock held while its own host is not serving may be that host's, started again
    /// after a start that failed, so it waits on its own host again, up to `within` in all,
    /// before it refuses, with why.
    pub fn before_chats_are_put_back(&self, within: Duration) -> crate::planes::Gate {
        let (connection, supervision) =
            (Arc::clone(&self.connection), Arc::clone(&self.supervision));
        Arc::new(move || {
            let deadline = Instant::now() + within;
            loop {
                if connection
                    .wait(deadline.saturating_duration_since(Instant::now()))
                    .is_ok()
                {
                    return Ok(());
                }
                let left_serving = supervision
                    .left_serving
                    .load(std::sync::atomic::Ordering::SeqCst);
                let place = supervision
                    .place
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone();
                if !left_serving && !place.is_some_and(|place| another_holds(&place.lock)) {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(
                        "another session host may still run these chats; quit purlis and start \
                         it again"
                            .to_owned(),
                    );
                }
                // A connection that is gone answers at once: look again in a moment.
                std::thread::sleep(LOOK_EVERY);
            }
        })
    }

    /// The link to the host, for the chats that run in it: ready once the host is, or gone with
    /// the reason there is none.
    pub fn connection(&self) -> Arc<Connection> {
        Arc::clone(&self.connection)
    }

    /// What kills the host running now outright, as a crash or `kill -9` ends it, leaving the
    /// window to see it go: the behaviour suite's crash test (#2007).
    #[cfg(test)]
    pub(crate) fn killer(&self) -> Arc<dyn Fn() + Send + Sync> {
        let held = Arc::clone(&self.held);
        Arc::new(move || {
            if let Some(host) = held.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
                let _ = host.child.kill();
            }
        })
    }

    /// What the window says while the host is failing, and its Try again (#2007).
    pub fn failing(&self) -> Arc<crate::hostfailing::HostFailing> {
        Arc::clone(&self.supervision.failing)
    }

    /// Ends the host at the app's quit: nothing starts it again, and it is told the window
    /// quits, so it ends every chat as a quit and stops, waiting at most [`QUITS_WITHIN`] for its
    /// answer (#2006). Then, for a host this window started, its standard input is closed, it is
    /// given [`ENDS_WITHIN`] to go, and is killed if it has not. A host this window reattached to
    /// that does not answer ends its chats when its grace period passes, as after a crash (ADR
    /// 0068 §2).
    pub fn end(&self) {
        self.supervision.end_now();
        if let Ok(link) = self.connection.wait(Duration::ZERO)
            && let Err(why) = link.quit_within(QUITS_WITHIN)
        {
            tracing::warn!("purlis: the session host did not answer the quit: {why}");
        }
        end_held(&self.held);
    }

    /// What ends the host from elsewhere: a quit that the host did not answer in time ends it
    /// before the save at quit (#2002). The host is told the window quits, without waiting,
    /// so it ends every chat as a quit; then a host this window started is ended as
    /// [`Self::end`] ends it. Ending it twice ends it once.
    pub fn ender(&self) -> Arc<dyn Fn() + Send + Sync> {
        let held = Arc::clone(&self.held);
        let supervision = Arc::clone(&self.supervision);
        let connection = Arc::clone(&self.connection);
        Arc::new(move || {
            supervision.end_now();
            if let Ok(link) = connection.wait(Duration::ZERO) {
                link.send(purlis_session_protocol::session::Command::Quit);
            }
            end_held(&held);
        })
    }
}

/// Ends the host `held` holds, if it still does: closes its standard input, waits
/// [`ENDS_WITHIN`] for it to go, and kills it if it has not.
fn end_held(held: &Mutex<Option<Held>>) {
    let Some(held) = held.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    end_one(held);
}

/// Ends `held`'s host: closes its standard input, waits [`ENDS_WITHIN`] for it to go, and kills
/// it if it has not.
fn end_one(mut held: Held) {
    drop(held.stdin.take());
    let deadline = Instant::now() + ENDS_WITHIN;
    while Instant::now() < deadline {
        if matches!(held.child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = held.child.kill();
    let _ = held.child.wait();
}

/// How a start of the host went.
enum Started {
    /// It serves, and the window connects to it.
    Serving,
    /// Another host of this user already serves: this window reattached to it (#2006).
    AnotherServes,
    /// Another host of this user already serves, and this window ends it rather than
    /// reattaching (#2004).
    AnotherToEnd,
    /// It did not start, or ended before it was ready.
    Failed,
}

/// **The supervisor** (ADR 0068 §3, #2007), on the host's own thread: starts the host, and
/// after each unexpected exit starts it again, until its breaker trips; then says the host is
/// failing and waits for the person's Try again. Nothing starts it once the window ends it.
///
/// An unexpected exit is any end the window did not ask for: the host crashed or was killed, it
/// ended before it was ready, or the window's link to it ended, after which the host is ended
/// first. Each chat that ran in it has ended with it (`crate::hostedsessions::HostLink`), and the
/// next host settles what the last one left from its host record.
fn supervise(
    held: &Mutex<Option<Held>>,
    place: &purlis_host::Place,
    starts: &(dyn Fn() -> Result<Command, String> + Send + Sync),
    connection: &Arc<Connection>,
    supervision: &Arc<Supervision>,
) {
    let mut ended_in_a_row = 0;
    loop {
        if supervision.ending() {
            return;
        }
        match start_here(held, place, starts, connection, supervision) {
            // A host a window that went left serving, which this window reattached to (#2006):
            // it is not this window's to end, so its end is its link's; once that ends, a host
            // is started again, which finds that one gone or reattaches to it again.
            Started::AnotherServes => {
                if !wait_for_its_link_to_end(supervision) {
                    return;
                }
            }
            Started::Serving => {
                ended_in_a_row = 0;
                supervision
                    .left_serving
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                wait_for_an_unexpected_end(held, supervision);
            }
            // Ended as a quit, then a host of this window's own is started at once (#2004).
            // A host that keeps being found is an unexpected end like any other.
            Started::AnotherToEnd => match end_the_one_serving(place) {
                Ok(()) if ended_in_a_row < ENDS_IN_A_ROW => {
                    ended_in_a_row += 1;
                    continue;
                }
                Ok(()) => {}
                Err(why) => {
                    tracing::warn!(
                        "purlis: the session host a window left serving could not be ended: {why}"
                    );
                    supervision
                        .left_serving
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                }
            },
            Started::Failed => {}
        }
        if supervision.ending() {
            return;
        }
        // Every start from here waits for the next host, as the first waited for the first.
        connection.restarting();
        let after = supervision
            .breaker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .exited(Instant::now());
        if after == breaker::After::StartAgain {
            tracing::warn!("purlis: the session host ended unexpectedly; starting it again");
            continue;
        }
        let said: Vec<String> = supervision
            .said
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect();
        let why = format!(
            "it ended unexpectedly {} times within {} minutes, so purlis stopped starting it \
             again",
            breaker::EXITS,
            breaker::WITHIN.as_secs() / 60
        );
        tracing::warn!("purlis: the session host is failing: {why}");
        connection.gone(why);
        supervision.failing.failing(crate::hostfailing::Failing {
            exits: u32::try_from(breaker::EXITS).unwrap_or(u32::MAX),
            within_minutes: u32::try_from(breaker::WITHIN.as_secs() / 60).unwrap_or(u32::MAX),
            said,
        });
        loop {
            let woken = supervision.rest();
            if supervision.ending() {
                return;
            }
            if woken.try_again {
                break;
            }
        }
        tracing::info!("purlis: the session host is tried again");
        supervision
            .breaker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .reset();
        supervision.failing.not_failing();
        connection.restarting();
    }
}

/// Waits until the link to a host this window reattached to ends (#2006): whether the window
/// is still to supervise one, or is ending it.
fn wait_for_its_link_to_end(supervision: &Supervision) -> bool {
    loop {
        let woken = supervision.rest();
        if supervision.ending() {
            return false;
        }
        if woken.link_lost {
            return true;
        }
    }
}

/// Returns once the host has ended without the window ending it, or the window is ending it.
/// A host whose link to the window ended is ended here first: the window cannot speak to it.
fn wait_for_an_unexpected_end(held: &Mutex<Option<Held>>, supervision: &Supervision) {
    loop {
        let woken = supervision.rest();
        // The window is ending its host: whatever is held here is ended now, even one the
        // window's own end found nothing of.
        if supervision.ending() {
            end_held(held);
            return;
        }
        let mut held = held.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(host) = held.as_mut() else {
            // The window took it to end it.
            return;
        };
        match host.child.try_wait() {
            Ok(Some(status)) => {
                tracing::warn!("purlis: the session host ended ({status})");
                *held = None;
                return;
            }
            Ok(None) if woken.link_lost && !supervision.ending() => {
                tracing::warn!(
                    "purlis: the window's link to the session host ended while it ran; it is \
                     ended, to be started again"
                );
                let _ = host.child.kill();
            }
            Ok(None) => {}
            Err(why) => {
                tracing::warn!("purlis: the session host could not be waited for: {why}");
                let _ = host.child.kill();
            }
        }
    }
}

/// Starts the host with the command `starts` gives and connects to it, on the calling thread.
fn start_here(
    held: &Mutex<Option<Held>>,
    place: &purlis_host::Place,
    starts: &(dyn Fn() -> Result<Command, String> + Send + Sync),
    connection: &Arc<Connection>,
    supervision: &Arc<Supervision>,
) -> Started {
    let mut command = match starts() {
        Ok(command) => command,
        Err(why) => {
            tracing::warn!("purlis: no session host: {why}");
            connection.gone(why);
            return Started::Failed;
        }
    };
    let mut child = match purlis_core::forklock::spawn(&mut command) {
        Ok(child) => child,
        Err(why) => {
            tracing::warn!("purlis: the session host did not start: {why}");
            connection.gone(format!("it did not start: {why}"));
            return Started::Failed;
        }
    };
    if let Some(said) = child.stderr.take() {
        log_what_it_says(said, Arc::clone(supervision));
    }
    let stdout = child.stdout.take();
    let stdin = child.stdin.take();
    let pid = child.id();
    {
        // Held under the same lock the window's end takes it by: a window that began ending
        // while this host was starting found nothing to end, so the host is ended here.
        let mut holding = held.lock().unwrap_or_else(|e| e.into_inner());
        if supervision.ending() {
            drop(holding);
            end_one(Held { child, stdin });
            return Started::Failed;
        }
        *holding = Some(Held { child, stdin });
    }

    if !stdout.is_some_and(said_ready) {
        let code = wait_for_its_end(held, Instant::now() + ENDS_WITHIN);
        match not_ready(code) {
            // The host a window that went without a quit left serving: this window reattaches
            // to it, and every chat it kept is put back where it is (#2006). It is not started
            // again: it serves.
            NotReady::AnotherServes
                if supervision
                    .ends_a_host_it_finds
                    .load(std::sync::atomic::Ordering::SeqCst) =>
            {
                return Started::AnotherToEnd;
            }
            NotReady::AnotherServes => {
                tracing::info!(
                    "purlis: a session host already serves at {}; this window reattaches to it",
                    place.socket.display()
                );
                connect(
                    place.socket.clone(),
                    place.credentials.clone(),
                    Arc::clone(connection),
                    Arc::clone(supervision),
                );
                return Started::AnotherServes;
            }
            NotReady::Gone(why) => {
                tracing::warn!("purlis: {why} (process {pid})");
                connection.gone(why);
            }
        }
        // One that never said it was ready and still runs is ended: it is started again.
        if let Some(mut host) = held.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = host.child.kill();
            let _ = host.child.wait();
        }
        return Started::Failed;
    }
    tracing::info!(
        "purlis: the session host serves at {}",
        place.socket.display()
    );
    connect(
        place.socket.clone(),
        place.credentials.clone(),
        Arc::clone(connection),
        Arc::clone(supervision),
    );
    Started::Serving
}

/// The host's exit code, once it has ended by `deadline`.
fn wait_for_its_end(held: &Mutex<Option<Held>>, deadline: Instant) -> Option<i32> {
    while Instant::now() < deadline {
        let mut held = held.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(Ok(Some(status))) = held.as_mut().map(|held| held.child.try_wait()) {
            *held = None;
            return status.code();
        }
        drop(held);
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

/// Whether the host's first line says it is ready, within [`READY_WITHIN`]. A host that ends
/// first, or says anything else, is not.
fn said_ready(stdout: std::process::ChildStdout) -> bool {
    let (told, heard) = std::sync::mpsc::channel();
    let reading = std::thread::Builder::new()
        .name("session host ready".into())
        .spawn(move || {
            let mut line = String::new();
            let read = BufReader::new(stdout).read_line(&mut line);
            let _ = told.send(read.is_ok() && line.trim_end() == purlis_host::READY);
        });
    reading.is_ok() && heard.recv_timeout(READY_WITHIN).unwrap_or(false)
}

/// Each line the host says on its standard error, into the window's log, the last of them kept
/// to say when the host is failing.
fn log_what_it_says(said: std::process::ChildStderr, supervision: Arc<Supervision>) {
    let _ = std::thread::Builder::new()
        .name("session host log".into())
        .spawn(move || {
            for line in BufReader::new(said).lines().map_while(Result::ok) {
                tracing::info!("{line}");
                supervision.heard(&line);
            }
        });
}

/// Whether a host holds the lock at `lock` now: taken and let go of at once when it is free.
/// A lock that cannot be asked is taken as held.
fn another_holds(lock: &Path) -> bool {
    match std::fs::File::open(lock) {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => true,
        Ok(file) => match file.try_lock() {
            Ok(()) => false,
            Err(_) => true,
        },
    }
}

/// How many hosts found serving a window ends in a row before it counts the next as an
/// unexpected end (#2004).
const ENDS_IN_A_ROW: u32 = 3;

/// **Ends the host serving at `place` as a quit** (#2004): every chat it kept ends as a quit,
/// and it stops. Answers once its socket is gone, so a host started next takes its place.
fn end_the_one_serving(place: &purlis_host::Place) -> Result<(), String> {
    let credential = Credential::read(&place.credentials, Scope::LocalUi)
        .map_err(|why| format!("its credential could not be read: {why}"))?;
    // Bounded: a host that takes the connection and never answers is not waited on for good.
    let link = tauri::async_runtime::block_on(async {
        tokio::time::timeout(
            QUITS_WITHIN,
            open_link(&place.socket, &credential, || {}, None),
        )
        .await
    })
    .map_err(|_| "it did not answer".to_owned())??;
    tracing::info!(
        "purlis: chats run in the window on this launch, so the session host a window left \
         serving at {} is told to quit, ending the chats it kept",
        place.socket.display()
    );
    link.quit_within(QUITS_WITHIN)?;
    drop(link);
    let deadline = Instant::now() + QUITS_WITHIN;
    while place.socket.exists() {
        if Instant::now() > deadline {
            return Err("it answered the quit and did not stop".to_owned());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

/// Connect to the host as `local-ui`, the window's scope, and hold the link for as long as the
/// window runs: the chats that run in the host go over it, and over one more link for each
/// project root the window holds there, opened the same way ([`HostLink`], #2178). A link that
/// cannot be made, or a window's link that ends, wakes the supervisor (#2007).
fn connect(
    socket: PathBuf,
    credentials: PathBuf,
    connection: Arc<Connection>,
    supervision: Arc<Supervision>,
) {
    tauri::async_runtime::spawn(async move {
        let lost = |why: String| {
            tracing::warn!("purlis: {why}");
            connection.gone(why);
            supervision.link_lost();
        };
        let credential = match Credential::read(&credentials, Scope::LocalUi) {
            Ok(credential) => credential,
            Err(why) => return lost(format!("its credential could not be read: {why}")),
        };
        let ended = {
            let connection = Arc::clone(&connection);
            let supervision = Arc::clone(&supervision);
            move || {
                if supervision.ending() {
                    tracing::info!("purlis: the link to the session host ended");
                    connection.gone("the link to it ended");
                    return;
                }
                tracing::warn!("purlis: the link to the session host ended unexpectedly");
                connection.restarting();
                supervision.link_lost();
            }
        };
        let runtime = tokio::runtime::Handle::current();
        match open_link(&socket, &credential, ended, None).await {
            Ok(link) => {
                let dial = dialing(socket.clone(), credential.clone(), runtime.clone());
                let relayed = relaying(socket, credential, runtime);
                connection.ready_relaying(Arc::new(link), dial, relayed);
            }
            Err(why) => lost(why),
        }
    });
}

/// A `local-ui` link to the host at `socket`; `ended` is called once it ends.
async fn open_link(
    socket: &Path,
    credential: &Credential,
    ended: impl FnOnce() + Send + 'static,
    naming: Option<purlis_session_protocol::browsing::Secret>,
) -> Result<HostLink, String> {
    open_link_relayed(socket, credential, ended, naming, None).await
}

/// [`open_link`], the host relaying it to the runner of `pool` where one is named (`relay_to`,
/// #2046).
async fn open_link_relayed(
    socket: &Path,
    credential: &Credential,
    ended: impl FnOnce() + Send + 'static,
    naming: Option<purlis_session_protocol::browsing::Secret>,
    pool: Option<&str>,
) -> Result<HostLink, String> {
    let stream = tokio::net::UnixStream::connect(socket)
        .await
        .map_err(|why| format!("its socket did not answer: {why}"))?;
    let link = link::connect(stream, session::speaks(), Scope::LocalUi, credential)
        .await
        .map_err(|why| format!("it refused the window: {why}"))?;
    HostLink::over_relayed(link, tokio::runtime::Handle::current(), ended, naming, pool).await
}

/// Opens one more link to the host that the host relays to a pool's runner (#2046): what
/// [`Connection::relaying_to`] holds each project root on, for a workspace whose chats run on
/// the pool.
fn relaying(
    socket: PathBuf,
    credential: Credential,
    runtime: tokio::runtime::Handle,
) -> crate::hostedsessions::RelayedDial {
    Arc::new(move |within, naming, pool: &str| {
        let (socket, credential, pool) = (socket.clone(), credential.clone(), pool.to_owned());
        crate::hostedsessions::open_within(&runtime, within, async move {
            open_link_relayed(
                &socket,
                &credential,
                || tracing::info!("purlis: a project's link to a pool's runner ended"),
                naming,
                Some(&pool),
            )
            .await
        })
    })
}

/// Opens one more link to the host, from any thread but the runtime's own: what
/// [`Connection::of_project`] holds each project root on. Its end is the window's link's to
/// judge: a host that went ends that one too, and the supervisor hears it there.
fn dialing(
    socket: PathBuf,
    credential: Credential,
    runtime: tokio::runtime::Handle,
) -> crate::hostedsessions::Dial {
    Arc::new(move |within, naming| {
        let (socket, credential) = (socket.clone(), credential.clone());
        crate::hostedsessions::open_within(&runtime, within, async move {
            open_link(
                &socket,
                &credential,
                || {
                    tracing::info!("purlis: a project's link to the session host ended");
                },
                naming,
            )
            .await
        })
    })
}

mod breaker;

#[cfg(test)]
mod tests;
