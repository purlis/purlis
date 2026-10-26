// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project's chats run in the session host, the default since #2004 (#1998).
//!
//! **One build holds both ways of running a chat, for the migration only** (spec #1974, ruled
//! 2026-10-14): in the session host the window started ([`HostedSessions`], the default), or in
//! the window's own process ([`crate::sessions::Sessions`]) where the start-time switch says so
//! ([`SWITCH`] set to `0`), chosen once as the app starts ([`chats_run_in_the_host`]) and never
//! switched while it runs. #2005 deletes the window's own path. With no host of the window's
//! own, the window runs no chats and says why: there is no fallback to the window's own process
//! (ADR 0068, Ruled 3).
//!
//! **Above the seam nothing changes.** The window composes a chat's start as it always has (its
//! sandbox, its hooks, its `PATH`), and hands the host that start as its [`Launch`];
//! the host composes the chat's environment from it with the same code the window's own path
//! runs (`purlis_core::chatspec`), against the environment the window started it with. The host
//! runs the program in a terminal of its own and the window draws it through a view, types into
//! it, resizes and stops it over the session protocol.
//!
//! **The host owns each project's hook channel** (#2000). It deals each chat's number, which the
//! window asks for before the start so the board knows the chat before its first hook; it binds
//! the project's hook socket, issues each chat its token as it starts it, and admits each line
//! as the window did. Every line it admits comes to the window on the link and is heard with the
//! window's own hearing ([`crate::hooks::Relayed`]), so the board, Needs you and the asks move
//! as before; the reply goes back the same way. The token never reaches the window.
//!
//! **The host keeps each project's board** (#2194). As the window holds a project it takes the
//! host's board whole, and tells the host each move it makes on its own from then on, so the
//! host's board outlives the window and a window that holds the project again reads every chat's
//! state and Needs you as the host kept them.
//!
//! **The host holds each chat's confinement** (#1999, `crate::hostedconfinement`): the window
//! plans a chat's egress proxy from the sandbox it compiled, and the host starts and holds it,
//! so it outlives the window's link.
//!
//! **The kill switch is the host's too** (#2001, ADR 0071): the window's switch reaches it over
//! the same link ([`Connection`] is its [`crate::killswitch::Authority`]), so a stop ends every
//! program the host runs and holds across the host's restart, and only the window re-arms.
//!
//! **The event log is the host's too** (#2003): the window opens none, and hands the host each
//! write it decides ([`record_in_the_host`]); what its hooks' hearing would write of a line goes
//! back with the line's reply. The host keeps what a program's end leaves, window or no window.
//!
//! **A link for each project root.** The window's own link to its host carries the kill switch
//! and the event log's writes; each project root the window opens is held on a link of its own
//! ([`Connection::of_project`]), which carries that root's chats, its browsers and its saving:
//! their views, word that a program ended, its hooks' lines and what each look of its auto-save
//! did. The host knows a project by its stable id, which every clone of it shares, and tells two
//! clones apart by the link that holds each (#2178).
//!
//! **The host saves the project too** (#2002): the window asks it to as the project opens
//! ([`HostedSessions`]'s `keep_saving`), and starts no auto-save of its own, so exactly one
//! saves it. What each look did comes back over the link, and the window reads its standing
//! again as it did when its own worker told it. A quit stops the host's saving before the
//! window's own save at quit runs; a window that is gone without a quit leaves it saving.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use purlis_core::engine::Size;
use purlis_session_protocol::link::{Acceptor, Link};
use purlis_session_protocol::session::{
    self, Answer, Client, Command, Events, Exit, HeldProgram, Launch, ProgramKind, Pushed,
    StartProgram, Told, ViewKind,
};
use purlis_session_protocol::view::{Chunk, Opened, Reader, Viewer};

use purlis_core::hookwire::relay::{Heard, Replied, Reply};

use crate::hooks::Relayed;
use crate::host::{Ends, Named, Opening, Readiness, Reconnecting, SessionHost, Sink, Watching};
use crate::reconnecting::Reconnection;
use crate::sessions::{ENDED, STOPPED, Utf8Stream};

/// The start-time switch: set to `0`, it runs this launch's chats in the window's own process,
/// the way back while the window's own path is still built (#2004). Read once, as the app
/// starts; internal to the migration, and gone with it (#2005, spec #1974).
pub const SWITCH: &str = "PURLIS_CHATS_IN_THE_HOST";

/// Whether this launch runs its chats in the session host: the default, and anything but
/// [`SWITCH`] set to `0`, which is the window's own process.
pub fn chats_run_in_the_host() -> bool {
    switched_on(purlis_core::envvar::var(SWITCH).as_deref())
}

fn switched_on(value: Option<&str>) -> bool {
    value != Some("0")
}

/// **What a launch whose chats run in the session host makes of its planes** (#1998 to #2003):
/// every project's chats run in the host `connection` reaches, over a link of each root's own;
/// their hooks are heard through the host, which owns each project's hook channel, so nothing
/// in the window reports anywhere (#2000); and the host writes the event log, the window
/// handing it every write it decides (#2003). The app's launch and the upgrade test (#2004)
/// both build it here, so what the test upgrades to is what a person's purlis runs.
pub fn chats_in_the_host(
    planes: crate::planes::Planes,
    connection: &Arc<Connection>,
) -> crate::planes::Planes {
    let events = crate::hooks::Events::in_the_host({
        let connection = Arc::clone(connection);
        move |entry| record_in_the_host(&connection, &entry)
    });
    let connection = Arc::clone(connection);
    // Where each workspace's chats run, read from this machine's store as each starts (#2046).
    let pools_in = planes.pools_in();
    planes
        .recording_events(Some(events))
        .running_sessions_on(Arc::new(move |_reporting, root: &Path| {
            let here = Box::new(HostedSessions::new(Arc::clone(&connection), root))
                as Box<dyn SessionHost>;
            match &pools_in {
                Some(config) => Box::new(routed(here, &connection, config, root)),
                None => here,
            }
        }))
        .hearing_hooks_through_the_host()
}

/// **The project at `root`'s chats, each on the host its workspace runs them on** (#2046):
/// `here`, or the runner of the pool `config`'s store names for its workspace, reached through
/// a link the window's host relays to it.
pub(crate) fn routed(
    here: Box<dyn SessionHost>,
    connection: &Arc<Connection>,
    config: &Path,
    root: &Path,
) -> crate::routedsessions::RoutedSessions {
    let (config, plane) = (config.to_path_buf(), root.to_path_buf());
    let runs_on: crate::routedsessions::RunsOn = {
        let (config, plane) = (config.clone(), plane.clone());
        Arc::new(move |workspace| purlis_core::pools::runs_on(&config, &plane, Some(workspace)))
    };
    let on_pools: crate::routedsessions::OnPools = {
        let (config, plane) = (config.clone(), plane.clone());
        Arc::new(move || purlis_core::pools::workspaces_on_pools(&config, &plane))
    };
    let to_a_pool: crate::routedsessions::ToAPool = {
        let (connection, plane) = (Arc::clone(connection), plane.clone());
        Arc::new(move |pool| {
            purlis_core::runners::runner_for(&config, pool).map_err(|why| format!("{why}."))?;
            Ok(Box::new(HostedSessions::on_a_pools_runner(
                connection.to_a_pool(pool),
                &plane,
            )) as Box<dyn SessionHost>)
        })
    };
    crate::routedsessions::RoutedSessions::new(root, here, runs_on, on_pools, to_a_pool)
}

/// Why a chat whose profile hands it credentials is not started in the session host (#2036).
pub const NOT_HANDED_IN_THE_HOST: &str = "this chat's profile signs it in with credentials from \
     your vault, and purlis hands those only to a chat the window starts, never over the link to \
     the session host, so nothing was started. Start purlis with PURLIS_CHATS_IN_THE_HOST=0 to \
     run it in the window.";

/// Why the window's own launch of a harness is not sent to a runner (#2446): a runner composes
/// its harness chats, with the sandbox it compiles for itself (ADR 0068, amended for #2217).
pub const COMPOSED_ON_THE_RUNNER: &str = "a chat's harness on a runner is started by what the \
     chat is named, and the runner composes it; the window's own launch of it was not sent";

/// Why a person's opt-out of the sandbox is not sent to a runner (#2446): a runner starts its
/// chats in their sandbox, and takes an opt-out from no link but its own window's.
pub const SANDBOXED_ON_A_RUNNER: &str = "a chat on a runner starts in its sandbox, so this one \
     was not started without it; start it sandboxed there, or on this machine to run it without \
     the sandbox";

/// How long a start waits for the host's link to be ready, before it says why there is none.
const LINK_WITHIN: Duration = Duration::from_secs(30);

/// How long a command waits for the host's answer.
pub(crate) const ANSWER_WITHIN: Duration = Duration::from_secs(30);

/// How a chat whose program ended with its session host is said to have ended: by the hang-up
/// its terminal's close gives it (#2007).
pub const ENDED_WITH_THE_HOST: &str = "Hangup";

/// What a view is written before the screen of its next epoch: a full reset, so a pane drawn
/// from a fresh snapshot after its view fell behind starts from blank, as the snapshot assumes.
const RESET: &str = "\x1bc";

/// **Writes `entry` in the event log the session host holds** (#2003), once the link is ready:
/// `Ok` once the host has written it durably, or why it did not. What the window's
/// [`crate::hooks::Events`] hands every write it decides to while chats run in the host.
pub fn record_in_the_host(
    connection: &Connection,
    entry: &purlis_core::eventlog::Entry,
) -> Result<(), String> {
    let link = connection.wait(LINK_WITHIN)?;
    for entry in as_the_host_reads(entry, link.version()) {
        let entry = serde_json::to_value(&entry)
            .map_err(|why| format!("the write could not be put in words for the host: {why}"))?;
        match link.call(Command::Record { entry })? {
            Answer::Done => {}
            other => return Err(format!("the session host answered a write with {other:?}")),
        }
    }
    Ok(())
}

/// The session protocol version whose host holds a start's trust event with its held run and
/// is told a held start was taken back (#2252).
const HOLDS_TRUST_WITH_A_RUN: purlis_session_protocol::version::Version =
    purlis_session_protocol::version::Version { major: 1, minor: 9 };

/// What `entry` is sent as, in order, to a host that agreed on `version`. A host before
/// [`HOLDS_TRUST_WITH_A_RUN`] is sent a held start's trust event on its own, before the start's
/// `Begin`, as every start's was, and is not told a start was taken back.
pub(crate) fn as_the_host_reads(
    entry: &purlis_core::eventlog::Entry,
    version: purlis_session_protocol::version::Version,
) -> Vec<purlis_core::eventlog::Entry> {
    use purlis_core::eventlog::Entry;
    if version >= HOLDS_TRUST_WITH_A_RUN {
        return vec![entry.clone()];
    }
    match entry {
        Entry::Begin {
            trust: Some(trust),
            chat,
            run,
            ..
        } => {
            let mut begin = entry.clone();
            if let Entry::Begin { trust, .. } = &mut begin {
                *trust = None;
            }
            vec![
                Entry::Trust {
                    chat: chat.clone(),
                    run: run.clone(),
                    kind: trust.kind().to_owned(),
                    body: trust.body().clone(),
                },
                begin,
            ]
        }
        Entry::TakenBack { .. } => Vec::new(),
        _ => vec![entry.clone()],
    }
}

/// Where the window's link to its session host stands: being made, ready, or not to be had and
/// why; and the link each project root is held on. Shared by every project's
/// [`HostedSessions`].
#[derive(Default)]
pub struct Connection {
    state: Mutex<Linked>,
    changed: Condvar,
    /// Opens another link to the same host, once the window's is ready.
    dial: Mutex<Option<Dial>>,
    /// The link each project root is held on, by the root (#2178).
    projects: Mutex<HashMap<PathBuf, Arc<HostLink>>>,
    /// Held while a root's link is being opened, by the root: one root never gets two links,
    /// and nothing waits for another root's.
    opening: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
    /// The config home whose session host's folder keeps the name the window gives each root's
    /// link (`purlis_core::windownames`, #2623), where the window names its links.
    names_in: Mutex<Option<PathBuf>>,
    /// Opens another link to the same host that its host relays to a pool's runner (`relay_to`,
    /// 1.33), once the window's is ready (#2046).
    relayed: Mutex<Option<RelayedDial>>,
    /// The window's own connection and the pool, for a connection whose every project link its
    /// host relays to that pool's runner (#2046): it waits on, and dials through, that one.
    to_a_pool: Option<(Arc<Connection>, String)>,
    /// The window's own link that this connection's project links were opened beside: one to a
    /// host started again after it ended holds every root on a link of its own anew (#2007).
    window_was: Mutex<Option<u64>>,
    /// The window's connection to each pool's runner, made the first time it is asked for
    /// ([`Connection::to_a_pool`]): what a pool's chats and their browsers share (#2050).
    pools: Mutex<HashMap<String, Arc<Connection>>>,
}

/// [`Dial`], for a link the window's host relays to the runner of the pool it names (#2046):
/// the link's first commands are the window's name, then `relay_to`.
pub type RelayedDial = Arc<
    dyn Fn(
            Duration,
            Option<purlis_session_protocol::browsing::Secret>,
            &str,
        ) -> Result<HostLink, String>
        + Send
        + Sync,
>;

/// Opens another `local-ui` link to the window's host, waiting at most the time it is given: one
/// for each project root the window holds there (#2178).
///
/// It is given the name the window keeps for the project root the link is for (#2623), where
/// it names its links: the link's first command says it ([`HostLink::over_relayed`]).
pub type Dial = Arc<
    dyn Fn(Duration, Option<purlis_session_protocol::browsing::Secret>) -> Result<HostLink, String>
        + Send
        + Sync,
>;

#[derive(Default)]
enum Linked {
    #[default]
    Making,
    Ready(Arc<HostLink>),
    Gone(String),
}

impl Connection {
    /// The link is ready, and `dial` opens another to the same host: the first, or the one to
    /// a host started again after its last ended (#2007), whose projects are each held again
    /// on a link of their own as they are next asked for.
    pub fn ready(&self, link: Arc<HostLink>, dial: Dial) {
        *lock(&self.dial) = Some(dial);
        let mut state = lock(&self.state);
        // Another host: every project root is held on a link of its own to it, opened anew.
        if !matches!(&*state, Linked::Ready(was) if was.id == link.id) {
            lock(&self.projects).clear();
        }
        *state = Linked::Ready(link);
        drop(state);
        self.changed.notify_all();
    }

    /// [`Self::ready`], with `relayed` opening the links the window's host relays to a pool's
    /// runner (#2046).
    pub fn ready_relaying(&self, link: Arc<HostLink>, dial: Dial, relayed: RelayedDial) {
        *lock(&self.relayed) = Some(relayed);
        self.ready(link, dial);
    }

    /// **The window's connection to the runner of `pool`** (#2046): each project root is held
    /// on a link of its own that the window's host relays to that pool's runner, named apart
    /// from its link to this machine's host. It waits on the window's own link, and is dialled
    /// as that one is; the kill switch and the event log stay on the window's own link.
    pub fn relaying_to(self: &Arc<Self>, pool: &str) -> Arc<Connection> {
        Arc::new(Connection {
            to_a_pool: Some((Arc::clone(self), pool.to_owned())),
            ..Connection::default()
        })
    }

    /// **The window's one connection to the runner of `pool`** ([`Self::relaying_to`]), made the
    /// first time it is asked for and the same after (#2050): a runner chat and its browsers are
    /// held on one link per project root, so its browser runs beside it, and one name the window
    /// keeps for a root's link to the pool is never two links'.
    pub fn to_a_pool(self: &Arc<Self>, pool: &str) -> Arc<Connection> {
        Arc::clone(
            lock(&self.pools)
                .entry(pool.to_owned())
                .or_insert_with(|| self.relaying_to(pool)),
        )
    }

    /// Name the window on each project root's link by the name it keeps for that root in
    /// `config`'s session host folder (#2623), so a host that relays the link to a runner names
    /// the window the same way after it restarts, and the window takes back its runner chats.
    pub fn naming_in(&self, config: PathBuf) {
        *lock(&self.names_in) = Some(config);
    }

    /// The host ended unexpectedly and is being started again (#2007): a start waits for the
    /// new one, as the first start waits for the first.
    pub fn restarting(&self) {
        *lock(&self.state) = Linked::Making;
        self.changed.notify_all();
    }

    /// The link the project at `root` is held on: its own, opened the first time it is asked
    /// for, so two clones of one project are held apart (#2178). Waits at most `within` for the
    /// window's link, and for the new one; with no time to wait, only a link already open is
    /// answered, whatever link is being opened meanwhile.
    pub(crate) fn of_project(
        &self,
        root: &Path,
        within: Duration,
    ) -> Result<Arc<HostLink>, String> {
        let window = self.wait(within)?;
        if self.to_a_pool.is_some() {
            let mut was = lock(&self.window_was);
            if *was != Some(window.id) {
                lock(&self.projects).clear();
                *was = Some(window.id);
            }
        }
        if let Some(link) = lock(&self.projects).get(root) {
            return Ok(Arc::clone(link));
        }
        if within.is_zero() {
            return Err(format!(
                "{} is not held in the session host",
                root.display()
            ));
        }
        let opening = Arc::clone(lock(&self.opening).entry(root.to_path_buf()).or_default());
        let _opening = lock(&opening);
        // Another call may have opened it while this one waited its turn.
        if let Some(link) = lock(&self.projects).get(root) {
            return Ok(Arc::clone(link));
        }
        // Named on the link as its first command, before it subscribes (#2623).
        let naming = self.name_for(root);
        let link = Arc::new(match &self.to_a_pool {
            Some((window, pool)) => {
                let relayed = lock(&window.relayed).clone().ok_or_else(|| {
                    no_host("there is no way to reach a pool's runner through it")
                })?;
                relayed(within, naming, pool).map_err(|why| {
                    format!(
                        "This workspace's chats run on pool {pool}, and its runner was not \
                         reached, so nothing was sent: {why}"
                    )
                })?
            }
            None => {
                let dial = lock(&self.dial)
                    .clone()
                    .ok_or_else(|| no_host("there is no way to open another link to it"))?;
                dial(within, naming).map_err(|why| no_host(&why))?
            }
        });
        if let Some(why) = &link.unnamed {
            tracing::warn!(
                "purlis: the session host did not take the window's name for the link of {}: \
                 {why}",
                root.display()
            );
        }
        lock(&self.projects).insert(root.to_path_buf(), Arc::clone(&link));
        Ok(link)
    }

    /// **The name the window keeps for `root`'s link** (`purlis_core::windownames`, #2623),
    /// where the window names its links: never evicted while a link of this window holds a root.
    /// A name that cannot be kept leaves the link unnamed, one the window takes nothing back on
    /// after a restart. The name is never said.
    fn name_for(&self, root: &Path) -> Option<purlis_session_protocol::browsing::Secret> {
        // A link to a pool's runner is named apart from the root's link to this machine's host,
        // and the same each time: one name is never two links' (#2046).
        let (config, keyed) = match &self.to_a_pool {
            Some((window, pool)) => (
                lock(&window.names_in).clone()?,
                root.join(RELAYED_NAMES).join(pool),
            ),
            None => (lock(&self.names_in).clone()?, root.to_path_buf()),
        };
        let mut open: Vec<PathBuf> = lock(&self.projects).keys().cloned().collect();
        if let Some(pool) = self.to_a_pool.as_ref().map(|(_, pool)| pool) {
            open = open
                .into_iter()
                .map(|root| root.join(RELAYED_NAMES).join(pool))
                .collect();
        }
        match purlis_core::windownames::name_for(&config, &keyed, &open) {
            Ok(name) => Some(purlis_session_protocol::browsing::Secret::new(
                name.expose().to_owned(),
            )),
            Err(why) => {
                tracing::warn!(
                    "purlis: the window keeps no name for the link of {}: {why}",
                    root.display()
                );
                None
            }
        }
    }

    /// There is no link, for `why`: every chat start says so from now on.
    pub fn gone(&self, why: impl Into<String>) {
        debug_assert!(self.to_a_pool.is_none(), "only the window's own link goes");
        *lock(&self.state) = Linked::Gone(why.into());
        self.changed.notify_all();
    }

    /// The link, once it is ready, waiting at most `within`; or why there is none.
    pub(crate) fn wait(&self, within: Duration) -> Result<Arc<HostLink>, String> {
        if let Some((window, _)) = &self.to_a_pool {
            return window.wait(within);
        }
        let deadline = Instant::now() + within;
        let mut state = lock(&self.state);
        loop {
            match &*state {
                Linked::Ready(link) => return Ok(Arc::clone(link)),
                Linked::Gone(why) => return Err(no_host(why)),
                Linked::Making => {}
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(no_host("the session host did not answer in time"));
            }
            state = self
                .changed
                .wait_timeout(state, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// The window's switch reaches the host's over the window's link, as `local-ui`: the one link
/// whose `rearm` the host takes.
impl crate::killswitch::Authority for Connection {
    fn stop(&self) -> Result<Option<String>, String> {
        let switch = self.kill_switch_after(Command::StopAll)?;
        if !switch.stopped {
            return Err(switch
                .why
                .unwrap_or_else(|| "it does not hold every agent stopped".into()));
        }
        Ok(switch.why)
    }

    fn rearm(&self) -> Result<(), String> {
        let switch = self.kill_switch_after(Command::Rearm)?;
        if switch.stopped {
            return Err(switch
                .why
                .unwrap_or_else(|| "it still holds every agent stopped".into()));
        }
        Ok(())
    }

    fn stopped(&self) -> Result<bool, String> {
        Ok(self.kill_switch_after(Command::KillSwitch)?.stopped)
    }
}

impl Connection {
    /// Sends one of the kill switch's commands, and answers the switch as the host holds it.
    fn kill_switch_after(&self, command: Command) -> Result<session::KillSwitch, String> {
        match self.wait(LINK_WITHIN)?.call(command)? {
            Answer::KillSwitch(switch) => Ok(switch),
            other => Err(format!(
                "the session host answered the kill switch with {other:?}"
            )),
        }
    }
}

/// What a transfer says when the host answered without opening its code stream.
fn no_code_stream() -> String {
    "the host did not open a stream for the code".to_owned()
}

/// The session protocol version whose host moves code (1.34, #2046).
const MOVES_CODE: purlis_session_protocol::version::Version =
    purlis_session_protocol::version::Version {
        major: 1,
        minor: 34,
    };

/// What a transfer says when the host refused it: in purlis's words where the host, or the
/// runner it relays to, is before [`MOVES_CODE`] and does not know the command.
fn code_refused(why: session::CallError) -> String {
    match why.refusal() {
        Some(refusal) if refusal.code == session::UNKNOWN_COMMAND => moves_no_code(),
        _ => why.to_string(),
    }
}

/// What a transfer says to a host before [`MOVES_CODE`].
fn moves_no_code() -> String {
    format!(
        "the host this project's chats run on is from a purlis before session protocol {}.{}, \
         which moves no code, so nothing was moved. Update it to this purlis's version.",
        MOVES_CODE.major, MOVES_CODE.minor
    )
}

/// What a link to a pool's runner is named by in the window's names (#2046), beside its root:
/// never a folder on the disk, only a key no root's own link has.
const RELAYED_NAMES: &str = ".purlis-pool";

/// What a chat start says when there is no host to run it (ADR 0068, Ruled 3).
fn no_host(why: &str) -> String {
    format!(
        "Chats run in purlis's session host on this machine, and it is not running: {why}. \
         Restart purlis to start it again."
    )
}

/// Where a browser view's frames go, each a JPEG image, whole (#2012), with what it shows where
/// the frame says so (#2014: the page and document, as `p1:5`). An empty one says the view has
/// ended for good.
pub type FrameSink = Box<dyn FnMut(Option<String>, bytes::Bytes) + Send>;

/// Where a browser view's word of its page's origin goes (1.35, #2300): the origin, or `None`
/// for no page, said where it changes and before the first image of each snapshot.
pub type OriginSink = Box<dyn FnMut(Option<String>) + Send>;

/// What a browser view is drawn into: its frames, and its page's origin; and, for a view of a
/// browser on another host, the window's own hold on its frames ([`HeldFrames`]).
pub struct FrameSinks {
    pub frames: FrameSink,
    pub origins: OriginSink,
    pub held: Option<HeldFrames>,
}

/// **The window's own hold on a runner browser's frames** (#2050): the runner's host is held to
/// the link's frames, and the window holds it to them too, whatever it sends. A frame larger than
/// [`MOST_HELD_FRAME_BYTES`], one whose header says it is wider than the link's widest or
/// higher than [`MOST_HELD_FRAME_HEIGHT`] (read before anything decodes it), or one that comes
/// sooner than [`HELD_FRAMES_AT_LEAST`] after the last drawn, is acknowledged and not drawn.
#[derive(Debug, Default)]
pub struct HeldFrames {
    last: Option<Instant>,
}

/// The largest frame of a runner's browser the window draws.
pub const MOST_HELD_FRAME_BYTES: usize = 4 << 20;

/// The highest frame of a runner's browser the window draws, in pixels.
pub const MOST_HELD_FRAME_HEIGHT: u16 = 8192;

/// The least time between two frames of a runner's browser the window draws: a little under
/// the link's 15 a second, so a frame the link bunched is not lost.
pub const HELD_FRAMES_AT_LEAST: Duration = Duration::from_millis(50);

impl FrameSinks {
    /// Whether `image` is drawn: every frame, but for a view the window holds (#2050).
    fn held_draw(&mut self, image: &[u8]) -> bool {
        match &mut self.held {
            Some(held) => held.draws(image, Instant::now()),
            None => true,
        }
    }
}

impl HeldFrames {
    /// Whether `image`, coming `now`, is drawn.
    pub fn draws(&mut self, image: &[u8], now: Instant) -> bool {
        if image.len() > MOST_HELD_FRAME_BYTES {
            return false;
        }
        let Some((width, height)) = purlis_session_protocol::view::frames::jpeg_size(image) else {
            return false;
        };
        if width > purlis_host::LINK_MOST_FRAME_WIDTH || height > MOST_HELD_FRAME_HEIGHT {
            return false;
        }
        if self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) < HELD_FRAMES_AT_LEAST)
        {
            return false;
        }
        self.last = Some(now);
        true
    }
}

/// What a view's stream is drawn into: a pane's terminal, as text, or a browser view's canvas,
/// a whole frame at a time.
#[derive(Clone)]
enum Draws {
    Text(Arc<Mutex<Sink>>),
    Frames(Arc<Mutex<FrameSinks>>),
}

/// What draws one view: the pane's sink, and where its stream stands.
struct Drawing {
    sink: Draws,
    /// The host said the view ended for good.
    ended: bool,
    /// The view's current stream has ended.
    drained: bool,
    /// [`ENDED`] has been written.
    finished: bool,
}

/// The window's link to its session host: the client its commands go through, the views the
/// host opens on it, and word of each program's end. One for every project.
///
/// **A link that ends takes its programs with it** (#2007): the window never ends it but to
/// quit, so a link that ends is a host that went, and every program the link was to be told the
/// end of has ended with its terminal. Each is told so ([`Ending::WithTheHost`]), and every pane
/// drawn from it is told its view ended.
pub struct HostLink {
    /// Which link this is, never another's: a project held on one is held again on the next.
    id: u64,
    client: Arc<Client>,
    runtime: tokio::runtime::Handle,
    shared: Arc<Shared>,
    /// What hears the host on it, which keeps it open: the link closes once this ends.
    hearing: tokio::task::AbortHandle,
    /// Why the host did not take the window's name on it, where it was named and did not
    /// (#2623): said by whoever opened it, never with the name.
    unnamed: Option<String>,
    /// The most bytes one bundle it moves may carry, either way (1.34, #2046).
    most_bundle_bytes: u64,
}

/// A link nobody holds any more is closed: one that opened after nobody waited for it, among
/// them.
impl Drop for HostLink {
    fn drop(&mut self) {
        self.hearing.abort();
    }
}

/// How a program the host ran ended, as the window is told it.
pub(crate) enum Ending {
    /// The host said it ended, and how.
    Exited(Exit),
    /// The link to its host ended first: the host went, and the program with it (#2007).
    WithTheHost,
}

#[derive(Default)]
struct Shared {
    /// The views, under one lock, so a stream and its pane always meet.
    views: Mutex<Views>,
    /// Who is told as each program ends, by its id.
    ends: Mutex<HashMap<String, TellEnd>>,
    /// How each program ended that nobody was waiting to be told of yet, by its id: one a
    /// window that comes back reattaches a moment after the host said so (#2006).
    unclaimed_ends: Mutex<HashMap<String, Exit>>,
    /// What hears each held project's hook lines, by the project's stable id (#2000).
    hearing: Mutex<HashMap<String, Arc<Relayed>>>,
    /// Who is told what each confinement's proxy did, by its id (#1999).
    sandboxes: Mutex<HashMap<String, Arc<crate::hostedconfinement::Hears>>>,
    /// Who is told what each look of a project's auto-save did, by the project's id (#2002).
    savings: Mutex<HashMap<String, crate::autosave::Told>>,
    /// Who says which chats are mid-turn in a workspace of a project the host saves, by the
    /// project's id: its plane's board (#2002).
    turns: Mutex<HashMap<String, Turns>>,
    /// Who is told what this link hears of the runner its host relays it to (#2496).
    reconnects: Mutex<Vec<Reconnects>>,
    /// Who holds each project again once the host says the runner it relays this link to no
    /// longer holds it, by the project's id (#2645).
    not_held: Mutex<HashMap<String, NotHeld>>,
    /// Who takes each code stream the host opens, by the transfer number the window named in its
    /// `take_code` or `give_code` (1.34, #2046).
    transfers:
        Mutex<HashMap<u64, tokio::sync::oneshot::Sender<purlis_session_protocol::link::Stream>>>,
}

/// What a link hears of the runner its host relays it to (#2496).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reconnect {
    /// The host says the link to the runner is down.
    Started,
    /// The host says it is back; or the window cannot tell any more, because it fell behind and
    /// subscribed again from now, or its link to the host ended.
    Over,
    /// The view the host numbers so drew a snapshot: a first one or a new one, which the host
    /// opens only while the link to the runner is up.
    Drawn(u32),
    /// The host says the runner answered with another key than the one pinned for it
    /// (`key_changed`, #2752): it does not link to it again until it is removed and added anew.
    KeyChanged,
}

/// Told what a link hears of the runner its host relays it to.
pub(crate) type Reconnects = Arc<dyn Fn(Reconnect) + Send + Sync>;

/// Told, in the runner's words, that the runner a link's host relays it to no longer holds a
/// project (#2645). Called off the runtime: it holds the project again over the link.
type NotHeld = Arc<dyn Fn(String) + Send + Sync>;

/// The most a reply to a hook line may be on the link, its frame's envelope aside: a larger
/// answer is refused with a sentence rather than sent.
const A_REPLY_IS_AT_MOST: usize = purlis_session_protocol::link::MOST_CONTROL_FRAME_BYTES - 4096;

/// Which chats are mid-turn in a workspace, or `None` where the window cannot say.
type Turns = Arc<dyn Fn(&str) -> Option<Vec<String>> + Send + Sync>;

/// Told once, as a program ends.
pub(crate) type TellEnd = Box<dyn FnOnce(Ending) + Send>;

/// The most ends of programs nobody waits for yet a window keeps (#2006).
const MOST_UNCLAIMED_ENDS: usize = 1024;

#[derive(Default)]
struct Views {
    /// Each view being drawn, by the number the host gave it.
    drawing: HashMap<u32, Drawing>,
    /// A view's stream that came before its pane was registered.
    waiting: HashMap<u32, Reader>,
}

impl HostLink {
    /// The session protocol version the host agreed on.
    pub fn version(&self) -> purlis_session_protocol::version::Version {
        self.client.version()
    }

    /// Speak the session protocol on `link`, which the host admitted as `local-ui`: subscribe
    /// to word of programs ending, and take every view the host opens. Run on `runtime`;
    /// `ended` is called once the link ends. The tests' link, named nowhere: the window's own
    /// opens each with [`Self::over_named`].
    #[cfg(test)]
    pub(crate) async fn over(
        link: Link,
        runtime: tokio::runtime::Handle,
        ended: impl FnOnce() + Send + 'static,
    ) -> Result<HostLink, String> {
        Self::over_named(link, runtime, ended, None).await
    }

    /// [`Self::over`], the window naming itself first by `naming`, the name it keeps for the
    /// project root this link is for (`name_window`, #2623): before it subscribes, so a host
    /// that relays the link names the window by it to the runner. Only to a host that knows the
    /// word. A refusal leaves the link unnamed, and is said without the name.
    #[cfg(test)]
    pub async fn over_named(
        link: Link,
        runtime: tokio::runtime::Handle,
        ended: impl FnOnce() + Send + 'static,
        naming: Option<purlis_session_protocol::browsing::Secret>,
    ) -> Result<HostLink, String> {
        Self::over_relayed(link, runtime, ended, naming, None).await
    }

    /// [`Self::over_named`], the link then relayed by the host to the runner it keeps as
    /// `relaying`, where one is named (`relay_to`, 1.33), before it subscribes.
    ///
    /// **A window that restarted before its host let go of its old link is relayed on a second
    /// try** (#2744): the old link still holds the window's name, so the host refuses the
    /// `relay_to` `not_allowed`, and keeps the name for this link. The window sends the same
    /// `relay_to` once more, [`NAMED_AGAIN_AFTER`] later, once the old link has ended. A second
    /// refusal is the link's: it is not opened.
    pub async fn over_relayed(
        link: Link,
        runtime: tokio::runtime::Handle,
        ended: impl FnOnce() + Send + 'static,
        naming: Option<purlis_session_protocol::browsing::Secret>,
        relaying: Option<&str>,
    ) -> Result<HostLink, String> {
        let (client, views, events) = Client::new(link);
        let client = Arc::new(client);
        let named = naming.is_some() && client.version() >= NAMES_ITSELF;
        let unnamed = match naming.filter(|_| client.version() >= NAMES_ITSELF) {
            Some(name) => client
                .call(Command::NameWindow { name })
                .await
                .err()
                .map(|why| why.to_string()),
            None => None,
        };
        if let Some(runner) = relaying {
            let mut relayed = client.relay_to(runner).await;
            let name_held = |sent: &Result<(), session::CallError>| {
                sent.as_ref()
                    .err()
                    .and_then(session::CallError::refusal)
                    .is_some_and(|refusal| refusal.code == session::NOT_ALLOWED)
            };
            if named && unnamed.is_none() && name_held(&relayed) {
                tokio::time::sleep(NAMED_AGAIN_AFTER).await;
                relayed = client.relay_to(runner).await;
            }
            relayed.map_err(|why| {
                format!("the session host did not relay the link to runner {runner}: {why}")
            })?;
        }
        client
            .subscribe(session::Since::new())
            .await
            .map_err(|why| format!("the session host refused to tell of its programs: {why}"))?;
        static LINKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = LINKS.fetch_add(1, Ordering::Relaxed);
        let shared = Arc::new(Shared::default());
        runtime.spawn(take_views(views, Arc::clone(&shared)));
        let hearing = hear(
            events,
            Arc::clone(&client),
            Arc::clone(&shared),
            HookLines {
                link: id,
                client: Arc::clone(&client),
                runtime: runtime.clone(),
            },
        );
        let lost = Arc::clone(&shared);
        let hearing = runtime
            .spawn(async move {
                hearing.await;
                reconnect(&lost, Reconnect::Over);
                // Off the runtime: each end told reads and moves the window's board.
                let _ =
                    tokio::task::spawn_blocking(move || everything_ended_with_the_host(&lost, id))
                        .await;
                ended();
            })
            .abort_handle();
        Ok(HostLink {
            id,
            client,
            runtime,
            shared,
            hearing,
            unnamed,
            most_bundle_bytes: purlis_session_protocol::code::MOST_BUNDLE_BYTES,
        })
    }

    /// This link, moving no bundle past `most` bytes: what a test makes a large one with.
    #[cfg(test)]
    pub(crate) fn holding_bundles_to(mut self, most: u64) -> HostLink {
        self.most_bundle_bytes = most;
        self
    }

    /// Send `command` and wait for the host's answer, from any thread.
    pub(crate) fn call(&self, command: Command) -> Result<Answer, String> {
        self.call_within(command, ANSWER_WITHIN)
    }

    /// [`Self::call`], waiting at most `within`.
    fn call_within(&self, command: Command, within: Duration) -> Result<Answer, String> {
        self.call_refused_within(command, within)
            .map_err(|refused| refused.why)
    }

    /// [`Self::call_within`], refused with the host's code where it gave one: what a browser's
    /// act, which may wait on its page, is asked with (#2013).
    pub(crate) fn call_refused_within(
        &self,
        command: Command,
        within: Duration,
    ) -> Result<Answer, purlis_session_protocol::session::Refusal> {
        let (answered, answer) = std::sync::mpsc::channel();
        let client = Arc::clone(&self.client);
        self.runtime.spawn(async move {
            let _ = answered.send(client.call(command).await);
        });
        let unanswered = |why: String| purlis_session_protocol::session::Refusal {
            code: String::new(),
            why,
        };
        match answer.recv_timeout(within) {
            Ok(Ok(answer)) => Ok(answer),
            Ok(Err(why)) => Err(why
                .refusal()
                .cloned()
                .unwrap_or_else(|| unanswered(why.to_string()))),
            Err(_) => Err(unanswered(
                "the session host did not answer in time".to_owned(),
            )),
        }
    }

    /// Send `command` and do not wait for its answer: what it refuses is logged.
    pub(crate) fn send(&self, command: Command) {
        let client = Arc::clone(&self.client);
        self.runtime.spawn(async move {
            let word = command.word();
            if let Err(why) = client.call(command).await {
                tracing::warn!("purlis: the session host refused `{word}`: {why}");
            }
        });
    }

    /// **Has the host take `refs` into `repo` of `project`, from the bundle at `bundle`** (1.34,
    /// #2046): an empty code stream where there is none, so the refs are moved from what the
    /// repository holds. `checkout` is checked out where the repository has nothing checked out
    /// yet. Answers every branch and snapshot the repository holds after it, from any thread but
    /// the link's runtime.
    pub(crate) fn send_code(
        &self,
        project: &str,
        repo: &str,
        refs: Vec<session::CodeRef>,
        checkout: Option<String>,
        bundle: Option<&Path>,
    ) -> Result<Vec<session::CodeRef>, String> {
        if self.version() < MOVES_CODE {
            return Err(moves_no_code());
        }
        let transfer = self.next_transfer();
        let stream = self.stream_for(transfer);
        let client = Arc::clone(&self.client);
        let most = self.most_bundle_bytes;
        let (project, repo, bundle) = (
            project.to_owned(),
            repo.to_owned(),
            bundle.map(Path::to_path_buf),
        );
        self.transfer(transfer, async move {
            let asked = client.take_code(&project, &repo, transfer, refs, checkout);
            let writing = async move {
                let mut stream = stream.await.map_err(|_| no_code_stream())?;
                if let Some(bundle) = bundle {
                    let mut file = tokio::fs::File::open(&bundle)
                        .await
                        .map_err(|why| format!("the bundle could not be read: {why}"))?;
                    purlis_session_protocol::code::copy_held_to(&mut file, &mut stream, most)
                        .await
                        .map_err(|why| why.to_string())?;
                }
                tokio::io::AsyncWriteExt::shutdown(&mut stream)
                    .await
                    .map_err(|why| format!("the code stream broke: {why}"))
            };
            // The host's answer comes as soon as it refuses, stream or none: a refusal is
            // said at once, never after the bound on a transfer.
            tokio::pin!(asked, writing);
            tokio::select! {
                answered = &mut asked => {
                    let heads = answered.map_err(code_refused)?;
                    writing.await?;
                    Ok(heads)
                }
                written = &mut writing => {
                    written?;
                    asked.await.map_err(code_refused)
                }
            }
        })
    }

    /// **Has the host write a bundle of `repo`'s branches in `project` into `into`** (1.34,
    /// #2046), leaving out what `have` reaches. Answers its branches, and whether a bundle came:
    /// none where the window lacks nothing.
    pub(crate) fn fetch_code(
        &self,
        project: &str,
        repo: &str,
        have: Vec<String>,
        into: &Path,
    ) -> Result<(Vec<session::CodeRef>, bool), String> {
        if self.version() < MOVES_CODE {
            return Err(moves_no_code());
        }
        let transfer = self.next_transfer();
        let stream = self.stream_for(transfer);
        let client = Arc::clone(&self.client);
        let most = self.most_bundle_bytes;
        let (project, repo, into) = (project.to_owned(), repo.to_owned(), into.to_path_buf());
        self.transfer(transfer, async move {
            let asked = client.give_code(&project, &repo, transfer, have);
            let reading = async move {
                let mut stream = stream.await.map_err(|_| no_code_stream())?;
                let mut file = tokio::fs::File::create(&into)
                    .await
                    .map_err(|why| format!("the bundle could not be kept: {why}"))?;
                let read =
                    purlis_session_protocol::code::copy_held_to(&mut stream, &mut file, most)
                        .await
                        .map_err(|why| why.to_string())?;
                tokio::io::AsyncWriteExt::flush(&mut file)
                    .await
                    .map_err(|why| format!("the bundle could not be kept: {why}"))?;
                Ok::<u64, String>(read)
            };
            tokio::pin!(asked, reading);
            tokio::select! {
                answered = &mut asked => {
                    let heads = answered.map_err(code_refused)?;
                    Ok((heads, reading.await? > 0))
                }
                read = &mut reading => {
                    let read = read?;
                    let heads = asked.await.map_err(code_refused)?;
                    Ok((heads, read > 0))
                }
            }
        })
    }

    /// A transfer number this link has not named before.
    fn next_transfer(&self) -> u64 {
        static TRANSFERS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        TRANSFERS.fetch_add(1, Ordering::Relaxed)
    }

    /// The code stream the host will open for `transfer`, once it does: asked for before the
    /// command goes, so a stream that comes first is never missed.
    fn stream_for(
        &self,
        transfer: u64,
    ) -> tokio::sync::oneshot::Receiver<purlis_session_protocol::link::Stream> {
        let (taker, stream) = tokio::sync::oneshot::channel();
        lock(&self.shared.transfers).insert(transfer, taker);
        stream
    }

    /// Run one transfer on the link's runtime and wait for it, at most the host's own bound
    /// on a transfer; the stream it waited for is let go of whatever the outcome.
    fn transfer<T: Send + 'static>(
        &self,
        transfer: u64,
        moving: impl std::future::Future<Output = Result<T, String>> + Send + 'static,
    ) -> Result<T, String> {
        let (moved, outcome) = std::sync::mpsc::channel();
        self.runtime.spawn(async move {
            let _ = moved.send(moving.await);
        });
        let outcome = outcome
            .recv_timeout(purlis_host::TRANSFER_WITHIN + ANSWER_WITHIN)
            .unwrap_or_else(|_| Err("the host did not finish moving the code in time".into()));
        lock(&self.shared.transfers).remove(&transfer);
        outcome
    }

    /// Draw view `view` with `sink`: its stream may already be here.
    fn draw(&self, view: u32, sink: Sink) {
        self.draw_into(view, Draws::Text(Arc::new(Mutex::new(sink))));
    }

    /// Which link this is, never another's.
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    /// Draw browser view `view`'s frames into `sink`, each whole (#2012), and its page's origin
    /// into `origins` (#2300): its stream may already be here.
    /// The window holds the view to the link's frames itself where `held` (#2050): a view of a
    /// browser on a runner.
    pub(crate) fn draw_frames(&self, view: u32, sink: FrameSink, origins: OriginSink, held: bool) {
        let sinks = FrameSinks {
            frames: sink,
            origins,
            held: held.then(HeldFrames::default),
        };
        self.draw_into(view, Draws::Frames(Arc::new(Mutex::new(sinks))));
    }

    fn draw_into(&self, view: u32, sink: Draws) {
        let waiting = {
            let mut views = lock(&self.shared.views);
            views.drawing.insert(
                view,
                Drawing {
                    sink,
                    ended: false,
                    drained: false,
                    finished: false,
                },
            );
            views.waiting.remove(&view)
        };
        if let Some(reader) = waiting {
            self.runtime
                .spawn(draw_from(reader, Arc::clone(&self.shared)));
        }
    }

    /// Stop drawing view `view`.
    pub(crate) fn undraw(&self, view: u32) {
        let mut views = lock(&self.shared.views);
        views.drawing.remove(&view);
        views.waiting.remove(&view);
    }

    /// Tell `hears` what confinement `id`'s proxy does (#1999).
    fn hear(&self, id: &str, hears: crate::hostedconfinement::Hears) {
        lock(&self.shared.sandboxes).insert(id.to_owned(), Arc::new(hears));
    }

    /// Tell nobody what confinement `id`'s proxy does any more.
    pub(crate) fn unhear(&self, id: &str) {
        lock(&self.shared.sandboxes).remove(id);
    }

    /// Tell `tell` once program `program` ends.
    pub(crate) fn when_it_ends(&self, program: &str, tell: TellEnd) {
        let ended = lock(&self.shared.unclaimed_ends).remove(program);
        match ended {
            Some(exit) => self.told_ended(tell, exit),
            None => {
                lock(&self.shared.ends).insert(program.to_owned(), tell);
            }
        }
    }

    /// Program `program` ended, `exit`, before this window was there to be told (#2006): whoever
    /// waits for its end is told, off the caller's thread.
    fn ended_before(&self, program: &str, exit: Exit) {
        let tell = lock(&self.shared.ends).remove(program);
        match tell {
            Some(tell) => self.told_ended(tell, exit),
            None => {
                lock(&self.shared.unclaimed_ends).insert(program.to_owned(), exit);
            }
        }
    }

    /// Tells `tell` how a program ended, on the link's runtime, as the host's word would be.
    fn told_ended(&self, tell: TellEnd, exit: Exit) {
        self.runtime
            .spawn(async move { tell(Ending::Exited(exit)) });
    }

    /// The window quits (#2006): the host ends every chat as a quit and stops, and answers
    /// first, within `within`.
    pub(crate) fn quit_within(&self, within: Duration) -> Result<(), String> {
        self.call_within(Command::Quit, within).map(|_| ())
    }

    /// What tells the host each move the window makes on `project`'s board (#2194): sent at
    /// once, behind the one before, under the board's hold, so the host makes them in the order
    /// the window did. What the host refuses is logged.
    fn board_keeper(&self, project: &str) -> purlis_core::state::Keeper {
        let client = Arc::clone(&self.client);
        let runtime = self.runtime.clone();
        let project = project.to_owned();
        Arc::new(move |chat, moved| {
            let moved = match serde_json::to_value(moved) {
                Ok(moved) => moved,
                Err(why) => {
                    tracing::warn!("purlis: a move of chat {chat} could not be said: {why}");
                    return;
                }
            };
            let sent = client.call_now(Command::BoardMoved {
                project: project.clone(),
                chat,
                moved,
            });
            runtime.spawn(async move {
                if let Err(why) = sent.await {
                    tracing::warn!("purlis: the session host refused a move of its board: {why}");
                }
            });
        })
    }

    /// Hear the lines `project`'s hooks write with `relayed`, from now on.
    fn hear_project(&self, project: &str, relayed: Arc<Relayed>) {
        relayed.linked(self.id);
        lock(&self.shared.hearing).insert(project.to_owned(), relayed);
    }

    /// Tell `tell` what this link hears of the runner its host relays it to, from now on: a
    /// project's chats (#2496) and the window's browser views (#2542) each follow their link.
    pub(crate) fn when_reconnecting(&self, tell: Reconnects) {
        let mut reconnects = lock(&self.shared.reconnects);
        // A project held again on the same link is told once (#2645).
        if !reconnects.iter().any(|told| Arc::ptr_eq(told, &tell)) {
            reconnects.push(tell);
        }
    }

    /// Hold `project` again with `hold` once the host says the runner it relays this link to no
    /// longer holds it (#2645), from now on.
    fn when_not_held(&self, project: &str, hold: NotHeld) {
        lock(&self.shared.not_held).insert(project.to_owned(), hold);
    }
}

/// Whoever asked is told what the link heard of the runner its host relays it to (#2496).
fn reconnect(shared: &Shared, heard: Reconnect) {
    let tell: Vec<Reconnects> = lock(&shared.reconnects).clone();
    for tell in tell {
        tell(heard);
    }
}

/// **The link ended, and every program on it with its host** (#2007): each program still to be
/// told its end is told it ended with the host, and every view drawn from the link ends, its
/// pane told so.
fn everything_ended_with_the_host(shared: &Shared, link: u64) {
    // What each project's hook lines kept on this link is kept no longer (#2271).
    let projects: Vec<Arc<Relayed>> = lock(&shared.hearing).values().cloned().collect();
    for relayed in projects {
        relayed.unlinked(link);
    }
    let ends: Vec<TellEnd> = lock(&shared.ends).drain().map(|(_, tell)| tell).collect();
    for tell in ends {
        tell(Ending::WithTheHost);
    }
    // Nothing more comes from a host that has gone: each pane is told now, once, and drawn
    // into no more.
    let views: Vec<u32> = {
        let mut views = lock(&shared.views);
        views.waiting.clear();
        views.drawing.keys().copied().collect()
    };
    for view in views {
        finish_view(shared, view);
        lock(&shared.views).drawing.remove(&view);
    }
}

/// Where the hook lines the host pushes are heard: each by what hears its project's lines
/// ([`Relayed::take`]), each reply going back on the link in the order it was made.
struct HookLines {
    /// The link they come over.
    link: u64,
    client: Arc<Client>,
    runtime: tokio::runtime::Handle,
}

impl HookLines {
    /// Hear line `id` that `project`'s hooks wrote, with what hears that project's lines.
    fn hear(&self, shared: &Shared, project: String, id: u64, line: serde_json::Value) {
        let Some(relayed) = lock(&shared.hearing).get(&project).cloned() else {
            tracing::warn!("purlis: the session host told a hook line of a project not open here");
            return;
        };
        let heard: Heard = match serde_json::from_value(line) {
            Ok(heard) => heard,
            Err(why) => {
                tracing::warn!(
                    "purlis: a hook line the session host told could not be read: {why}"
                );
                return;
            }
        };
        let reply = self.reply(project, id);
        relayed.take(self.link, id, heard, reply);
    }

    /// What replies to line `id` of `project`: each reply sent at once, behind the one before.
    fn reply(&self, project: String, id: u64) -> Reply {
        let client = Arc::clone(&self.client);
        let runtime = self.runtime.clone();
        Arc::new(move |replied: Replied| {
            let reply = within_a_frame(replied);
            let sent = client.call_now(Command::ReplyHeard {
                project: project.clone(),
                heard: id,
                reply,
            });
            runtime.spawn(async move {
                if let Err(why) = sent.await {
                    tracing::warn!("purlis: the session host refused a hook line's reply: {why}");
                }
            });
        })
    }
}

/// `replied`, as JSON no larger than the link carries: an answer too large is refused in a
/// sentence instead.
fn within_a_frame(replied: Replied) -> serde_json::Value {
    let too_large = |replied: &Replied| {
        serde_json::to_vec(replied).map_or(true, |said| said.len() > A_REPLY_IS_AT_MOST)
    };
    let replied = if too_large(&replied) {
        let why = format!(
            "the answer was larger than the session host's link carries ({A_REPLY_IS_AT_MOST} \
             bytes)"
        );
        match replied {
            Replied::Answer(_) => Replied::Answer(purlis_core::hookwire::Answer::No { why }),
            _ => Replied::NotTaken { why },
        }
    } else {
        replied
    };
    serde_json::to_value(replied).unwrap_or(serde_json::Value::Null)
}

/// Take each view's stream the host opens, and hand it to its pane, or keep it until the pane
/// is registered: the host opens the stream before it answers the attach.
async fn take_views(mut views: Acceptor, shared: Arc<Shared>) {
    let mut viewer = Viewer::default();
    while let Ok(stream) = views.accept().await {
        let reader = match viewer.accept_any(stream).await {
            Ok(Opened::View(reader)) => reader,
            // A code stream goes to the transfer that asked for it (1.34, #2046); one nobody
            // asked for is dropped, which ends it for the host.
            Ok(Opened::Code(header, stream)) => {
                if let Some(taker) = lock(&shared.transfers).remove(&header.transfer) {
                    let _ = taker.send(stream);
                }
                continue;
            }
            Err(_) => continue,
        };
        let view = reader.view().0;
        let mut views = lock(&shared.views);
        match views.drawing.get_mut(&view) {
            Some(drawing) => {
                drawing.drained = false;
                drop(views);
                tokio::spawn(draw_from(reader, Arc::clone(&shared)));
            }
            None => {
                views.waiting.insert(view, reader);
            }
        }
    }
}

/// Draw one stream of a view into its pane, as text, acknowledging what was handed over. A
/// stream after the first (the view fell behind, and came back at its next epoch) starts its
/// pane over.
async fn draw_from(mut reader: Reader, shared: Arc<Shared>) {
    let view = reader.view().0;
    let frames = lock(&shared.views)
        .drawing
        .get(&view)
        .and_then(|drawing| match &drawing.sink {
            Draws::Frames(sink) => Some(Arc::clone(sink)),
            Draws::Text(_) => None,
        });
    if let Some(sink) = frames {
        return draw_frames_from(reader, &shared, &sink).await;
    }
    let again = reader.epoch().0 > 1;
    let mut text = Utf8Stream::default();
    if again {
        give(&shared, view, RESET.to_owned());
    }
    let mut first = true;
    while let Some(Ok(chunk)) = reader.next().await {
        let bytes = match chunk {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => bytes,
        };
        give(&shared, view, text.push(&bytes));
        // It draws a snapshot, which comes only over a link that is up: whatever was said of its
        // runner's link, it is reconnected (#2496).
        if std::mem::take(&mut first) {
            reconnect(&shared, Reconnect::Drawn(view));
        }
        if reader.ack(bytes.len()).await.is_err() {
            break;
        }
    }
    let finish = lock(&shared.views)
        .drawing
        .get_mut(&view)
        .is_some_and(|drawing| {
            drawing.drained = true;
            drawing.ended
        });
    if finish {
        finish_view(&shared, view);
    }
}

/// Draw one stream of a browser view (#2012): each whole frame handed over as it completes,
/// and acknowledged once handed. A stream after the first starts on a whole frame too: the
/// host's snapshot is the latest frame.
async fn draw_frames_from(mut reader: Reader, shared: &Arc<Shared>, sink: &Arc<Mutex<FrameSinks>>) {
    let view = reader.view().0;
    let mut frames = purlis_session_protocol::view::frames::Frames::default();
    let mut first = true;
    while let Some(Ok(chunk)) = reader.next().await {
        let bytes = match chunk {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => bytes,
        };
        let Ok(whole) = frames.push(&bytes) else {
            break;
        };
        // Its stream's first frame is the host's snapshot, which comes only over a link that is
        // up: whatever was said of its runner's link, the view is reconnected (#2542).
        if !whole.is_empty() && std::mem::take(&mut first) {
            reconnect(shared, Reconnect::Drawn(view));
        }
        for frame in whole {
            use purlis_session_protocol::view::frames::{AIMED_JPEG, JPEG, ORIGIN};
            match frame.format {
                JPEG if !frame.image.is_empty() => {
                    let mut sinks = lock(sink);
                    if sinks.held_draw(&frame.image) {
                        (sinks.frames)(None, frame.image);
                    }
                }
                // What it shows, as the window echoes it back with the person's input (#2014).
                AIMED_JPEG => {
                    if let Some((aim, image)) = frame.aimed()
                        && !image.is_empty()
                    {
                        let mut sinks = lock(sink);
                        if sinks.held_draw(&image) {
                            (sinks.frames)(Some(crate::browsers::aim_said(&aim)), image);
                        }
                    }
                }
                // Where its page is, its origin alone (#2300), as this window reads one.
                ORIGIN => {
                    if let Some(origin) = frame.origin() {
                        (lock(sink).origins)(an_origin(origin));
                    }
                }
                _ => {}
            }
        }
        if reader.ack(bytes.len()).await.is_err() {
            break;
        }
    }
    let finish = lock(&shared.views)
        .drawing
        .get_mut(&view)
        .is_some_and(|drawing| {
            drawing.drained = true;
            drawing.ended
        });
    if finish {
        finish_view(shared, view);
    }
}

/// **What a host said of a page's origin, as the window takes it** (#2300): an origin, exactly
/// as [`purlis_session_protocol::browsing::origin_of`] writes one, or none. A view may be drawn
/// from a runner's host (#2542), and the chip is purlis's own words outside the page: anything
/// else a host sends (text, a path, a long string, a character that turns the line around) is
/// read as no origin.
pub(crate) fn an_origin(said: Option<String>) -> Option<String> {
    said.filter(|origin| {
        purlis_session_protocol::browsing::origin_of(origin).as_deref() == Some(origin.as_str())
    })
}

/// Hand `text` to view `view`'s pane, if it still has one.
fn give(shared: &Shared, view: u32, text: String) {
    if text.is_empty() {
        return;
    }
    let sink = lock(&shared.views)
        .drawing
        .get(&view)
        .and_then(|drawing| match &drawing.sink {
            Draws::Text(sink) => Some(Arc::clone(sink)),
            Draws::Frames(_) => None,
        });
    if let Some(sink) = sink {
        (lock(&sink))(text);
    }
}

/// The view ended for good, and its last bytes are drawn: its pane is told, once.
fn finish_view(shared: &Shared, view: u32) {
    let sink = {
        let mut views = lock(&shared.views);
        match views.drawing.get_mut(&view) {
            Some(one) if !one.finished => {
                one.finished = true;
                Some(one.sink.clone())
            }
            _ => None,
        }
    };
    match sink {
        Some(Draws::Text(sink)) => (lock(&sink))(ENDED.to_owned()),
        Some(Draws::Frames(sink)) => (lock(&sink).frames)(None, bytes::Bytes::new()),
        None => {}
    }
}

/// What the host pushes: a program's end, a view's, and its projects' hook lines.
async fn hear(mut events: Events, client: Arc<Client>, shared: Arc<Shared>, lines: HookLines) {
    while let Some(told) = events.recv().await {
        match told {
            Told::Pushed(Pushed::Heard {
                project,
                heard,
                line,
            }) => lines.hear(&shared, project, heard, line),
            Told::Pushed(Pushed::Ended { program, exit, .. }) => {
                let tell = lock(&shared.ends).remove(&program);
                match tell {
                    Some(tell) => tell(Ending::Exited(exit)),
                    // Nobody waits for it yet: a program a window that comes back is about to
                    // reattach is told it once it does (#2006). Kept within bounds.
                    None => {
                        let mut unclaimed = lock(&shared.unclaimed_ends);
                        if unclaimed.len() >= MOST_UNCLAIMED_ENDS {
                            unclaimed.clear();
                        }
                        unclaimed.insert(program, exit);
                    }
                }
            }
            Told::Pushed(Pushed::Saving {
                project,
                saved,
                moved_tree,
            }) => {
                let told = lock(&shared.savings).get(&project).cloned();
                if let Some(told) = told {
                    // What it tells reads the plane's model and the window: off the runtime.
                    tokio::task::spawn_blocking(move || {
                        told(crate::autosave::Looked { saved, moved_tree });
                    });
                }
            }
            Told::Pushed(Pushed::WhoIsMidTurn {
                question,
                project,
                workspace,
            }) => {
                let turns = lock(&shared.turns).get(&project).cloned();
                let client = Arc::clone(&client);
                tokio::spawn(async move {
                    // The board is read off the runtime; a project the window does not save
                    // in the host is one it cannot say anything of.
                    let chats = match turns {
                        Some(turns) => tokio::task::spawn_blocking(move || turns(&workspace))
                            .await
                            .ok()
                            .flatten(),
                        None => None,
                    };
                    let _ = client.call(Command::MidTurn { question, chats }).await;
                });
            }
            Told::Pushed(Pushed::Sandbox { confinement, told }) => {
                let hears = lock(&shared.sandboxes).get(&confinement).cloned();
                if let Some(hears) = hears {
                    hears.told(told);
                }
            }
            Told::Pushed(Pushed::LinkDown { key_changed, .. }) => {
                reconnect(&shared, Reconnect::Started);
                if key_changed {
                    reconnect(&shared, Reconnect::KeyChanged);
                }
            }
            Told::Pushed(Pushed::LinkBack {}) => reconnect(&shared, Reconnect::Over),
            // A browser engine's first download, as it goes (1.37, #2207).
            Told::Pushed(Pushed::EngineDownload {
                program,
                done,
                total,
            }) => crate::enginedownload::here().heard(&program, done, total),
            // The hold goes over the link and waits for its answer: off the runtime.
            Told::Pushed(Pushed::NotHeld { project, why }) => {
                let hold = lock(&shared.not_held).get(&project).cloned();
                if let Some(hold) = hold {
                    tokio::task::spawn_blocking(move || hold(why));
                }
            }
            Told::Pushed(Pushed::ViewEnded { view, .. }) => {
                let drained = lock(&shared.views)
                    .drawing
                    .get_mut(&view)
                    .is_some_and(|drawing| {
                        drawing.ended = true;
                        drawing.drained
                    });
                if drained {
                    finish_view(&shared, view);
                }
            }
            // The window keeps no events of the host's log yet: only the notices matter, so a
            // reader that fell behind subscribes again from now.
            Told::FellBehind { .. } => {
                // Whether the runner's link came back meanwhile is not known: nothing is left
                // dimmed on a word that may have been missed (#2496).
                reconnect(&shared, Reconnect::Over);
                let _ = client.subscribe(session::Since::new()).await;
            }
            Told::Pushed(_) => {}
        }
    }
}

/// One chat the host runs for this project.
struct Program {
    /// Its id in the host.
    id: String,
    /// Its process, until it has ended.
    process: Option<u32>,
    /// The views the window has open on it, by the number the host gave each.
    views: Vec<u32>,
}

/// A project's chats, run in the session host (#1998): [`SessionHost`] over the window's link.
pub struct HostedSessions {
    connection: Arc<Connection>,
    root: PathBuf,
    /// The project's stable id, once the host holds it, and the link it is held on: a host
    /// started again holds it again (#2007).
    held: Arc<Mutex<Option<Held>>>,
    /// What hears the lines the host hands on from the project's hook channel (#2000).
    relayed: Arc<Mutex<Option<Arc<Relayed>>>>,
    running: Arc<Mutex<HashMap<u32, Program>>>,
    /// The highest number the window knows went out: the host deals them (#2000), and this is
    /// what the window's record keeps and what it tells the host went out before it held the
    /// project. It only rises.
    opened: Arc<AtomicU32>,
    ended: Mutex<Option<Ends>>,
    kill_switch: Arc<crate::killswitch::KillSwitch>,
    /// The chats whose program ended with their host, until each is started again or closed
    /// (#2007).
    lost: Arc<Mutex<std::collections::HashSet<u32>>>,
    /// Each program the host still had for this project as the window held it, from before
    /// this window, that nothing has reattached yet, by its number (#2006).
    left: Arc<Mutex<HashMap<u32, HeldProgram>>>,
    /// Why the board the host kept could not be taken as the window held the project, where it
    /// could not (#2006).
    untaken: Arc<Mutex<Option<String>>>,
    /// Which chats are reconnecting, while the link to the runner they run on is down, and who
    /// is told (#2496).
    reconnecting: Arc<Mutex<Dimmed>>,
    /// What tells whoever asked which of the chats are reconnecting: one for the project, so a
    /// link that holds it again tells it once ([`reconnects_of`]).
    reconnects: Reconnects,
    /// Whether the host runs this project's chats on a runner, through the link its host relays
    /// the window's link over (#2446): it composes each harness chat, and its proxies listen on
    /// the runner's ports.
    on_a_runner: bool,
    /// Whether the runner holds the project where it keeps its projects, sent the project's code
    /// by the window (#2046): a pool's runner. A runner a test runs on this machine's disk holds
    /// it at the window's own path, and is sent nothing.
    kept_on_the_runner: bool,
    /// What the runner held of each repository after the window last sent it (#2046).
    sent: Arc<crate::runnercode::Sent>,
}

/// The chats the window was told are reconnecting, until each is reconnected, and who is told
/// (#2496).
#[derive(Default)]
struct Dimmed {
    chats: std::collections::BTreeSet<u32>,
    tell: Option<Reconnecting>,
}

/// A project held on a link: its stable id, and which link.
#[derive(Clone)]
struct Held {
    project: String,
    on: u64,
}

impl HostedSessions {
    /// The chats of the project at `root`, run in the host `connection` reaches.
    pub fn new(connection: Arc<Connection>, root: &Path) -> Self {
        let (running, reconnecting) = (Arc::default(), Arc::default());
        HostedSessions {
            connection,
            root: root.to_path_buf(),
            held: Arc::default(),
            relayed: Arc::default(),
            reconnects: reconnects_of(&running, &reconnecting),
            running,
            opened: Arc::default(),
            ended: Mutex::new(None),
            kill_switch: crate::killswitch::KillSwitch::unthrown(),
            lost: Arc::default(),
            left: Arc::default(),
            untaken: Arc::default(),
            reconnecting,
            on_a_runner: false,
            kept_on_the_runner: false,
            sent: Arc::default(),
        }
    }

    /// [`Self::new`], for a host that runs the project's chats on a runner (#2446): each harness
    /// chat is started there by its names, and each confinement is held as the runner's.
    pub fn on_a_runner(connection: Arc<Connection>, root: &Path) -> Self {
        HostedSessions {
            on_a_runner: true,
            ..Self::new(connection, root)
        }
    }

    /// [`Self::on_a_runner`], for a pool's runner (#2046): the runner holds the project where it
    /// keeps its projects, `<data>/projects/<project id>`, and the window sends it the project's
    /// code as it holds it there and each workspace's clones before a chat starts in it.
    pub fn on_a_pools_runner(connection: Arc<Connection>, root: &Path) -> Self {
        HostedSessions {
            kept_on_the_runner: true,
            ..Self::on_a_runner(connection, root)
        }
    }

    /// The confinement the host placed at `placed`, held through `held`: on the window's machine,
    /// or on the runner's, whose ports are not this machine's (#2446).
    fn held(
        &self,
        placed: purlis_core::sandbox::Placed,
        held: crate::hostedconfinement::HeldInTheHost,
    ) -> std::io::Result<purlis_core::sandbox::Confinement> {
        if self.on_a_runner {
            Ok(purlis_core::sandbox::Confinement::held_on_another_machine(
                placed,
                Box::new(held),
            ))
        } else {
            purlis_core::sandbox::Confinement::held_elsewhere(placed, Box::new(held))
        }
    }

    fn relayed(&self) -> Option<Arc<Relayed>> {
        lock(&self.relayed).clone()
    }

    /// This project's own link to the host, waiting at most `within`.
    fn own_link(&self, within: Duration) -> Result<Arc<HostLink>, String> {
        self.connection.of_project(&self.root, within)
    }

    /// The link, with this project held on it.
    fn link(&self) -> Result<(Arc<HostLink>, String), String> {
        held_on(&self.holding())
    }

    /// What holds this project on its own link to the host.
    fn holding(&self) -> Holding {
        Holding {
            connection: Arc::clone(&self.connection),
            held: Arc::clone(&self.held),
            relayed: Arc::clone(&self.relayed),
            opened: Arc::clone(&self.opened),
            left: Arc::clone(&self.left),
            untaken: Arc::clone(&self.untaken),
            root: self.root.clone(),
            reconnects: Arc::clone(&self.reconnects),
            kept_on_the_runner: self.kept_on_the_runner,
            sent: Arc::clone(&self.sent),
        }
    }

    /// Holds the project in the session host now, and takes the board the host keeps for it
    /// (#2194): what a window that comes back does first, so every chat's state and Needs you
    /// are as the host kept them. Only the tests hold one so until a window comes back to a
    /// running host (#2006).
    #[cfg(test)]
    pub(crate) fn hold(&self) -> Result<(), String> {
        self.link().map(|_| ())
    }

    /// The next number the host deals in this project.
    fn dealt_by_the_host(&self) -> Result<u32, String> {
        let (link, project) = self.link()?;
        match link.call(Command::Deal { project })? {
            Answer::Dealt { number } => {
                self.opened.fetch_max(number, Ordering::Relaxed);
                Ok(number)
            }
            other => Err(format!("the session host answered a deal with {other:?}")),
        }
    }

    /// The link, and the host's id of chat `id`.
    fn program(&self, id: u32) -> Result<(Arc<HostLink>, String), String> {
        let program = lock(&self.running)
            .get(&id)
            .map(|program| program.id.clone())
            .ok_or_else(|| gone(id))?;
        Ok((self.own_link(LINK_WITHIN)?, program))
    }

    /// The number a chat opens under: `wanted` where it can still be had, else the next the
    /// host deals (`crate::sessions::Sessions`'s rule, the host dealing).
    fn number_for(&self, wanted: Option<u32>) -> Result<u32, String> {
        let free = wanted.filter(|wanted| *wanted > 0 && !lock(&self.running).contains_key(wanted));
        match free {
            Some(wanted) => {
                self.opened.fetch_max(wanted, Ordering::Relaxed);
                Ok(wanted)
            }
            None => self.dealt_by_the_host(),
        }
    }

    /// Every chat's id in the host, and the link.
    fn every_program(&self) -> Option<(Arc<HostLink>, Vec<String>)> {
        let programs: Vec<String> = lock(&self.running)
            .values()
            .map(|program| program.id.clone())
            .collect();
        let link = self.own_link(Duration::ZERO).ok()?;
        Some((link, programs))
    }

    /// Sends `command` for each program at once, and waits for every answer.
    fn to_every_program(&self, command: impl Fn(String) -> Command) -> usize {
        let Some((link, programs)) = self.every_program() else {
            return 0;
        };
        let asked = programs.len();
        let answers: Vec<_> = programs
            .into_iter()
            .map(|program| {
                let (answered, answer) = std::sync::mpsc::channel();
                let client = Arc::clone(&link.client);
                let command = command(program);
                link.runtime.spawn(async move {
                    let _ = answered.send(client.call(command).await);
                });
                answer
            })
            .collect();
        for answer in answers {
            let _ = answer.recv_timeout(ANSWER_WITHIN);
        }
        asked
    }
}

/// What tells whoever asked which of this project's chats are reconnecting (#2496). While
/// the link the host relays the project's link over is down every chat is, because they
/// all run where that link goes; each chat dimmed is reconnected once the link is back or
/// the window cannot tell, or alone once a view of it draws a snapshot.
fn reconnects_of(
    running: &Arc<Mutex<HashMap<u32, Program>>>,
    dimmed: &Arc<Mutex<Dimmed>>,
) -> Reconnects {
    let (running, dimmed) = (Arc::clone(running), Arc::clone(dimmed));
    Arc::new(move |heard| {
        let (chats, reconnection) = match heard {
            Reconnect::Started => {
                let chats: Vec<u32> = lock(&running).keys().copied().collect();
                lock(&dimmed).chats.extend(chats.iter().copied());
                (chats, Reconnection::Reconnecting)
            }
            Reconnect::Over => {
                let chats = std::mem::take(&mut lock(&dimmed).chats);
                (chats.into_iter().collect(), Reconnection::Reconnected)
            }
            // Said whatever chats there are: the window says it once for the project.
            Reconnect::KeyChanged => {
                let chats: Vec<u32> = lock(&dimmed).chats.iter().copied().collect();
                if let Some(tell) = lock(&dimmed).tell.clone() {
                    tell(chats, Reconnection::KeyChanged);
                }
                return;
            }
            Reconnect::Drawn(view) => {
                let chat = lock(&running)
                    .iter()
                    .find_map(|(id, program)| program.views.contains(&view).then_some(*id));
                let Some(chat) = chat.filter(|chat| lock(&dimmed).chats.remove(chat)) else {
                    return;
                };
                (vec![chat], Reconnection::Reconnected)
            }
        };
        let mut chats = chats;
        chats.sort_unstable();
        let tell = lock(&dimmed).tell.clone();
        if let (false, Some(tell)) = (chats.is_empty(), tell) {
            tell(chats, reconnection);
        }
    })
}

/// The project at `root`'s own link to the host `connection` reaches, with the project held on
/// it (#2178): `held` keeps its id
/// once it is. Before the hold the host's lines for the project are heard through `relayed`,
/// and after it the host is told the numbers `opened` has dealt (#2000); the window takes the
/// board the host kept, or keeps why it could not in `untaken` (#2194, #2006), and what the
/// host still runs of the project from before this window in `left` (#2006).
fn held_on(holding: &Holding) -> Result<(Arc<HostLink>, String), String> {
    let Holding {
        connection,
        held,
        relayed,
        opened,
        left,
        untaken,
        root,
        reconnects,
        kept_on_the_runner,
        sent,
    } = holding;
    let link = connection.of_project(root, LINK_WITHIN)?;
    let mut held = lock(held);
    if let Some(Held { project, on }) = &*held
        && *on == link.id
    {
        return Ok((link, project.clone()));
    }
    let project = purlis_core::scaffold::planefile::id_to_hold_by(root)
        .map_err(|why| format!("this project has no id the session host can name it by: {why}"))?;
    // Before the hold, so the host's first line for the project is heard.
    if let Some(relayed) = lock(relayed).clone() {
        link.hear_project(&project, relayed);
    }
    link.when_reconnecting(Arc::clone(reconnects));
    // A runner holds it where it keeps its projects, `<data>/projects/<project id>`, and is
    // sent the project's own code to check out there (#2046).
    link.call(Command::HoldProject {
        project: project.clone(),
        project_path: if *kept_on_the_runner {
            String::new()
        } else {
            root.display().to_string()
        },
    })
    .map_err(|why| format!("{NOT_HELD} {why}"))?;
    if *kept_on_the_runner {
        crate::runnercode::send(&link, &project, root, "", sent)?;
    }
    link.when_not_held(&project, holds_again(holding, link.id, &project));
    // The numbers that went out before the host held the project are never dealt again.
    let dealt = opened.load(Ordering::Relaxed);
    if dealt > 0 {
        link.call(Command::DealtAlready {
            project: project.clone(),
            dealt,
        })?;
    }
    // The board the host keeps, taken whole, and each move the window makes from now on told
    // to it (#2194). A board the host does not answer is none: the window's own goes on, and is
    // told from now on. A pool's runner keeps none: this machine's host keeps the board of every
    // chat of the project, those on runners included (#2046).
    if let (false, Some(relayed)) = (*kept_on_the_runner, lock(relayed).clone()) {
        let kept = match link.call(Command::Board {
            project: project.clone(),
        }) {
            Ok(Answer::Board(board)) if !board.is_null() => serde_json::from_value(board)
                .inspect_err(|why| {
                    tracing::warn!(
                        "purlis: the board the session host keeps could not be read: {why}"
                    );
                    *lock(untaken) = Some(format!(
                        "the board the session host kept could not be read ({why})"
                    ));
                })
                .ok(),
            Ok(_) => None,
            Err(why) => {
                tracing::warn!("purlis: the session host did not answer its board: {why}");
                // Said on each chat reattached: its state is not the host's (#2006).
                *lock(untaken) = Some(why);
                None
            }
        };
        relayed.keep_with(link.board_keeper(&project), kept);
    }
    // What the host still runs, or ran, in the project from before this window: each chat a
    // put back finds here is reattached rather than started again (#2006). A host started
    // again after its last ended has none.
    if link.version() >= REATTACHES {
        match link.call(Command::ProgramsOf {
            project: project.clone(),
        }) {
            Ok(Answer::Programs(programs)) => {
                *lock(left) = programs
                    .into_iter()
                    .filter(|program| program.kind != ProgramKind::Browser)
                    .map(|program| (program.number, program))
                    .collect();
            }
            Ok(other) => {
                tracing::warn!("purlis: the session host answered its programs with {other:?}");
            }
            Err(why) => tracing::warn!("purlis: the session host did not say its programs: {why}"),
        }
    }
    *held = Some(Held {
        project: project.clone(),
        on: link.id,
    });
    // Held: a mark that a runner would not hold it goes (#2846).
    crate::notheld::held(root);
    Ok((link, project))
}

/// What the window says of a project its host could not hold, before the host's words (#2645).
pub(crate) const NOT_HELD: &str = "The host this project's chats run on does not hold it, so \
     nothing was sent. purlis asks it again the next time you start or use a chat here. The host \
     said:";

/// **What holds the project again once the runner no longer holds it** (#2645): the host that
/// relays the project's link `on` to a runner says, in the runner's words, that the runner's link
/// could not hold it after a re-link. The window stops counting it as held there, and holds it
/// again over the link, so its chats and their hooks carry on. Where the runner refuses
/// again, the project stays not held, and every command that needs the hold tries again and is
/// refused in the runner's words ([`NOT_HELD`]). A project held again since on another link is
/// left as it is.
fn holds_again(holding: &Holding, on: u64, project: &str) -> NotHeld {
    let holding = holding.clone();
    // The connection holds the link that holds this: it is not kept alive by it.
    let connection = Arc::downgrade(&holding.connection);
    let Holding {
        held,
        relayed,
        opened,
        left,
        untaken,
        root,
        reconnects,
        kept_on_the_runner,
        sent,
        ..
    } = holding;
    let project = project.to_owned();
    Arc::new(move |why: String| {
        {
            let mut held = lock(&held);
            if !held
                .as_ref()
                .is_some_and(|held| held.on == on && held.project == project)
            {
                return;
            }
            *held = None;
        }
        // A runner that holds it again may hold none of what was sent before.
        crate::runnercode::forget(&sent);
        tracing::warn!(
            "purlis: the runner no longer holds project {project} after a re-link ({why}); \
             holding it again"
        );
        let Some(connection) = connection.upgrade() else {
            return;
        };
        let holding = Holding {
            connection,
            held: Arc::clone(&held),
            relayed: Arc::clone(&relayed),
            opened: Arc::clone(&opened),
            left: Arc::clone(&left),
            untaken: Arc::clone(&untaken),
            root: root.clone(),
            reconnects: Arc::clone(&reconnects),
            kept_on_the_runner,
            sent: Arc::clone(&sent),
        };
        if let Err(again) = held_on(&holding) {
            tracing::warn!("purlis: project {project} is not held where its chats run: {again}");
            // Marked in the Pools group until a hold is taken (#2846), in the runner's words:
            // only a refusal of the hold, never what purlis itself could not do.
            if let Some(words) = again.strip_prefix(NOT_HELD) {
                crate::notheld::not_held(&holding.root, words.trim_start());
            }
        }
    })
}

/// How long a window waits to send its `relay_to` once more, where the host refused it because
/// another link holds the window's name (#2744): long enough for the host to see the end of the
/// window's link from before a restart, short enough that a person does not notice it.
const NAMED_AGAIN_AFTER: Duration = Duration::from_millis(500);

/// The session protocol version whose host takes the window's name for a link (#2623).
const NAMES_ITSELF: purlis_session_protocol::version::Version =
    purlis_session_protocol::version::Version {
        major: 1,
        minor: 31,
    };

/// The session protocol version whose host says what a window that comes back reattaches
/// (#2006).
const REATTACHES: purlis_session_protocol::version::Version =
    purlis_session_protocol::version::Version {
        major: 1,
        minor: 13,
    };

/// What holds a project on its own link to the host, for a thread of its own: [`held_on`]'s.
#[derive(Clone)]
struct Holding {
    connection: Arc<Connection>,
    held: Arc<Mutex<Option<Held>>>,
    relayed: Arc<Mutex<Option<Arc<Relayed>>>>,
    opened: Arc<AtomicU32>,
    left: Arc<Mutex<HashMap<u32, HeldProgram>>>,
    /// Why the board the host kept could not be taken, where it could not (#2006).
    untaken: Arc<Mutex<Option<String>>>,
    root: PathBuf,
    /// Told what the project's link hears of the runner its host relays it to (#2496).
    reconnects: Reconnects,
    /// Whether the project is held on a runner where it keeps it, with its code sent (#2046).
    kept_on_the_runner: bool,
    /// What the runner held of each repository after the window last sent it.
    sent: Arc<crate::runnercode::Sent>,
}

/// A project the session host saves for the window (#2002): asked for once the link is ready,
/// off the thread that opened the project, and stopped when this is dropped.
struct SavedInTheHost {
    asked: Arc<Mutex<Asked>>,
    poke: std::sync::mpsc::Sender<crate::autosave::Poke>,
    /// Who says which chats are mid-turn: the plane, once it is built.
    mid_turn: Arc<Mutex<Option<crate::autosave::MidTurn>>>,
    /// A quit already stopped it, answered or not: its drop asks nothing more of the host.
    stopped_by_a_quit: bool,
}

#[derive(Default)]
struct Asked {
    /// Dropped: nothing more is asked of the host.
    stopped: bool,
    /// The host saves the project, on this link.
    saving: Option<(Arc<HostLink>, String)>,
    /// The window's link to the host that was last asked, whatever it answered: a host started
    /// again is asked once, on its own (#2007).
    asked_on: Option<u64>,
}

impl SavedInTheHost {
    fn start(holding: Holding, told: crate::autosave::Told) -> SavedInTheHost {
        let asked: Arc<Mutex<Asked>> = Arc::default();
        let (poke, poked) = std::sync::mpsc::channel();
        let asking = Arc::clone(&asked);
        let mid_turn: Arc<Mutex<Option<crate::autosave::MidTurn>>> = Arc::default();
        let board = Arc::clone(&mid_turn);
        let started = std::thread::Builder::new()
            .name("saved in the session host".into())
            .spawn(move || {
                if !ask_to_save(&holding, &asking, &told, &board) {
                    return;
                }
                // What the window pokes: a fetch is the host's to make. A chat's end the host
                // hears for itself, as its program ends. And a host started again after its
                // last ended is asked again, on its own link, as soon as it is there (#2007).
                loop {
                    let poke = match poked.recv_timeout(LOOK_FOR_A_NEW_HOST_EVERY) {
                        Ok(poke) => Some(poke),
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                    };
                    let asked = lock(&asking);
                    if asked.stopped {
                        return;
                    }
                    let on = asked.asked_on;
                    let now = holding
                        .connection
                        .wait(Duration::ZERO)
                        .ok()
                        .map(|link| link.id);
                    if now.is_some() && now != on {
                        drop(asked);
                        if !ask_to_save(&holding, &asking, &told, &board) {
                            return;
                        }
                        continue;
                    }
                    if let (Some(crate::autosave::Poke::Fetch), Some((link, project))) =
                        (poke, &asked.saving)
                    {
                        link.send(Command::FetchProject {
                            project: project.clone(),
                        });
                    }
                }
            });
        if let Err(why) = started {
            tracing::warn!("purlis: the project's saving was not asked of the session host: {why}");
        }
        SavedInTheHost {
            asked,
            poke,
            mid_turn,
            stopped_by_a_quit: false,
        }
    }

    /// Stops the host's saving, waiting at most `within` for its answer: whether it answered.
    /// Nothing asked of the host, or nothing more to stop, is stopped.
    fn stop_within(&self, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        // A start still being asked holds this until the host answers it: waited for only
        // until the deadline.
        let mut asked = loop {
            match self.asked.try_lock() {
                Ok(asked) => break asked,
                Err(std::sync::TryLockError::Poisoned(poisoned)) => break poisoned.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    tracing::warn!("purlis: the session host did not answer the project's saving");
                    return false;
                }
            }
        };
        asked.stopped = true;
        let answered = match asked.saving.take() {
            Some((link, project)) => {
                forget(&link, &project);
                let left = deadline.saturating_duration_since(Instant::now());
                match link.call_within(Command::StopSaving { project }, left) {
                    Ok(_) => true,
                    Err(why) => {
                        tracing::warn!("purlis: the session host did not stop saving: {why}");
                        false
                    }
                }
            }
            None => true,
        };
        drop(asked);
        // Wakes the poke thread, which sees it is stopped.
        let _ = self.poke.send(crate::autosave::Poke::Fetch);
        answered
    }
}

/// How often a project the host saves looks for a host started again after its last ended.
const LOOK_FOR_A_NEW_HOST_EVERY: Duration = Duration::from_millis(250);

/// **Asks the host the link reaches now to save the project `holding` holds** (#2002), holding
/// it there first: told what each look did, and asking the window's board who is mid-turn.
/// Answers whether to go on, which is not once the saving was stopped.
fn ask_to_save(
    holding: &Holding,
    asking: &Mutex<Asked>,
    told: &crate::autosave::Told,
    board: &Arc<Mutex<Option<crate::autosave::MidTurn>>>,
) -> bool {
    let root = &holding.root;
    let (link, project) = match held_on(holding) {
        Ok(held) => held,
        Err(why) => {
            tracing::warn!(
                "purlis: {} is not saved by itself on this launch: {why}",
                root.display()
            );
            return true;
        }
    };
    // The window reads the standing again as the host says it moved, as it did when its own
    // worker said so.
    lock(&link.shared.savings).insert(project.clone(), {
        let (root, told) = (root.clone(), Arc::clone(told));
        Arc::new(move |looked| {
            purlis_core::planegit::touch(&root);
            told(looked);
        })
    });
    // And the host asks the window's board who is mid-turn.
    let board = Arc::clone(board);
    lock(&link.shared.turns).insert(
        project.clone(),
        Arc::new(move |workspace: &str| {
            let ask = lock(&board).clone();
            ask.map(|ask| ask(workspace))
        }),
    );
    let mut asked = lock(asking);
    if asked.stopped {
        forget(&link, &project);
        return false;
    }
    // By the window's own link: a host started again is a new one of those.
    asked.asked_on = holding
        .connection
        .wait(Duration::ZERO)
        .ok()
        .map(|window| window.id);
    match link.call(Command::StartSaving {
        project: project.clone(),
    }) {
        Ok(_) => asked.saving = Some((link, project)),
        Err(why) => {
            forget(&link, &project);
            tracing::warn!(
                "purlis: the session host does not save {}: {why}",
                root.display()
            );
        }
    }
    true
}

/// Nobody on this window is told of `project`'s saving, or asked of its turns, any more.
fn forget(link: &HostLink, project: &str) {
    lock(&link.shared.savings).remove(project);
    lock(&link.shared.turns).remove(project);
}

impl crate::autosave::Saver for SavedInTheHost {
    fn poker(&self) -> std::sync::mpsc::Sender<crate::autosave::Poke> {
        self.poke.clone()
    }

    /// What the host asks of this window's board ([`Pushed::WhoIsMidTurn`]).
    fn asks_mid_turn_of(&self, ask: crate::autosave::MidTurn) {
        *lock(&self.mid_turn) = Some(ask);
    }

    fn stop_by(mut self: Box<Self>, deadline: Instant) -> bool {
        self.stopped_by_a_quit = true;
        self.stop_within(deadline.saturating_duration_since(Instant::now()))
    }
}

/// Stops the host's saving and answers once the host has: a close's or a quit's own save comes
/// after, so the project is never saved by both. A quit stops it sooner ([`Self::stop_within`]).
impl Drop for SavedInTheHost {
    fn drop(&mut self) {
        if !self.stopped_by_a_quit {
            self.stop_within(ANSWER_WITHIN);
        }
    }
}

/// What kind of program the host is asked to start: a shell is the operator's, or the person's
/// own login shell with no harness in it; anything else, a profile's own command among them, is
/// a harness. The host lets the window's shell through the kill switch (ADR 0071, ruled 3), and
/// the window refuses, before it asks, every shell that is not the operator's.
fn kind_of(opening: &Opening) -> ProgramKind {
    let the_persons_shell = opening.harness.is_none() && opening.program.is_none();
    if opening.operator_shell || the_persons_shell {
        ProgramKind::Shell
    } else {
        ProgramKind::Harness
    }
}

/// The window's own start, as the host is handed it. Where the chat's hooks report is the
/// host's to say (#2000), so the launch says nothing of it.
fn launch_of(opening: &Opening, program: String) -> Launch {
    let confinement = opening.confinement.clone();
    Launch {
        program,
        args: opening.args.clone(),
        cwd: opening.cwd.clone(),
        set: opening.env.clone(),
        strip: opening.env_strip.clone(),
        pass: opening.env_pass.clone(),
        declared: opening.declared_env.clone(),
        harness: opening.harness.map(|harness| harness.name().to_owned()),
        git_hooks: opening
            .git_hooks
            .as_ref()
            .map(|hooks| hooks.dir().display().to_string()),
        reporting: None,
        confinement,
    }
}

impl HostedSessions {
    /// Starts `start` in the host as chat `id`, over `link`, and keeps it: what [`SessionHost::open`]
    /// and [`SessionHost::open_named`] share once each has said what the start is.
    fn started(
        &self,
        link: &Arc<HostLink>,
        id: u32,
        start: StartProgram,
        operator_shell: bool,
        announce: &dyn Fn(u32),
    ) -> Result<Begun, String> {
        // Before the program exists, so its very first hook lands somewhere.
        announce(id);
        let started = match link.call(Command::StartProgram(Box::new(start))) {
            // The host runs it under the number the window announced, or the window would key
            // it under one its hooks do not carry: it is closed, and the start fails.
            Ok(Answer::ProgramStarted {
                program, number, ..
            }) if number != Some(id) => {
                link.send(Command::Close { program });
                return Err(format!(
                    "the session host started this chat as number {}, not {id}, so it was \
                     closed; start it again",
                    number.map_or_else(|| "none".to_owned(), |number| number.to_string())
                ));
            }
            Ok(Answer::ProgramStarted {
                program,
                process,
                confined,
                sandbox,
                ..
            }) => (program, process, confined, sandbox),
            Ok(other) => {
                return Err(format!("the session host answered a start with {other:?}"));
            }
            Err(why) => return Err(why),
        };
        let (program, pid, confined, ran) = started;
        self.runs(link, id, &program, pid);
        lock(&self.lost).remove(&id);
        // A stop that landed while this program was being started is carried out now.
        if !operator_shell && self.kill_switch.is_stopped() {
            link.send(Command::Stop { chat: program });
        }
        Ok(Begun { id, confined, ran })
    }
}

/// What [`HostedSessions::started`] heard of a start: its number, the confinement the host named
/// to this link where it named one, and how the host applied the sandbox to its run (1.29).
struct Begun {
    id: u32,
    confined: Option<session::Confined>,
    ran: Option<session::RunSandbox>,
}

/// Each call is documented once, on [`SessionHost`].
impl SessionHost for HostedSessions {
    fn open(
        &self,
        wanted: Option<u32>,
        opening: &Opening,
        announce: &dyn Fn(u32),
    ) -> Result<u32, String> {
        if !opening.operator_shell && self.kill_switch.is_stopped() {
            return Err(STOPPED.to_owned());
        }
        // A profile's credentials are handed to a program the window starts, and never over the
        // link (#2036): a chat that is handed some is not started here.
        if !opening.handed.is_empty() {
            return Err(NOT_HANDED_IN_THE_HOST.to_owned());
        }
        if self.on_a_runner && opening.harness.is_some() {
            return Err(COMPOSED_ON_THE_RUNNER.to_owned());
        }
        let (link, project) = self.link()?;
        let program = opening
            .program
            .clone()
            .unwrap_or_else(crate::sessions::shell);
        let id = self.number_for(wanted)?;
        // The run this start begins, as the window minted it (ADR 0066).
        let start = StartProgram {
            number: Some(id),
            run: opening.run.clone(),
            cols: Some(opening.size.columns),
            rows: Some(opening.size.rows),
            launch: Some(launch_of(opening, program)),
            ..StartProgram::of(project, kind_of(opening))
        };
        self.started(&link, id, start, opening.operator_shell, announce)
            .map(|begun| begun.id)
    }

    fn open_named(
        &self,
        wanted: Option<u32>,
        named: &Named,
        hearing: purlis_core::sandbox::plan::Hearing,
        announce: &dyn Fn(u32),
    ) -> Result<crate::host::Composed, String> {
        use crate::hostedconfinement::{Hears, HeldInTheHost, placed};
        if self.kill_switch.is_stopped() {
            return Err(STOPPED.to_owned());
        }
        if self.on_a_runner && named.without_sandbox {
            return Err(SANDBOXED_ON_A_RUNNER.to_owned());
        }
        let (link, project) = self.link()?;
        // The workspace's clones as they are here, uncommitted work and all, before the chat
        // starts on them there (#2046).
        if let (true, Some(workspace)) = (self.kept_on_the_runner, &named.workspace) {
            crate::runnercode::send_workspace(&link, &project, &self.root, workspace, &self.sent)?;
        }
        let id = self.number_for(wanted)?;
        // The confinement the host composes for it, named to this window where its proxy is to
        // ask here (#2407), and heard before the start, so nothing its proxy does is missed.
        let confinement = hearing.asks.is_some().then(purlis_core::reopen::mint);
        if let Some(confinement) = &confinement {
            link.hear(confinement, Hears::of(hearing));
        }
        // Its own chat and the run this start begins (ADR 0066), which its trust event is
        // written under in the host.
        let start = StartProgram {
            number: Some(id),
            cols: Some(named.size.columns),
            rows: Some(named.size.rows),
            program: Some(named.chat.clone().unwrap_or_else(purlis_core::reopen::mint)),
            run: Some(named.run.clone().unwrap_or_else(purlis_core::reopen::mint)),
            persona: named.persona.clone(),
            profile: named.profile.clone(),
            workspace: named.workspace.clone(),
            resume: named.resume.clone(),
            first_message: named.first_message.clone(),
            without_sandbox: named.without_sandbox,
            // Asked about where the host leaves it out, so said only where it asks this window.
            alone: if confinement.is_some() {
                named.alone.clone()
            } else {
                Vec::new()
            },
            confinement: confinement.clone(),
            ..StartProgram::of(project, ProgramKind::Harness)
        };
        let unheard = |confinement: &Option<String>| {
            if let Some(confinement) = confinement {
                link.unhear(confinement);
            }
        };
        // How the host applied the sandbox to the run it composed, by its own record (#2692).
        let Begun { id, confined, ran } = self
            .started(&link, id, start, false, announce)
            .inspect_err(|_| unheard(&confinement))?;
        // Held from here, where the host named it to this window: dropping it, on any path, lets
        // the host end it. A host that named none tells this window nothing of it.
        let named_here =
            confined.filter(|confined| confinement.as_ref() == Some(&confined.confinement));
        let (Some(confinement), Some(confined)) = (confinement.clone(), named_here) else {
            unheard(&confinement);
            return Ok(crate::host::Composed {
                number: id,
                confinement: None,
                ran,
            });
        };
        let held = HeldInTheHost::new(confinement, Arc::clone(&link), true);
        // One the window cannot hold is let go of, and the chat that runs in it is closed with
        // it: a chat is never left running behind a start that says it did not start.
        let confinement = match self.held(placed(&confined), held) {
            Ok(confinement) => confinement,
            Err(why) => {
                let _ = self.close(id);
                return Err(format!(
                    "purlis could not hold the network the session host started for this \
                         chat ({why}), so it closed the chat: start it again"
                ));
            }
        };
        Ok(crate::host::Composed {
            number: id,
            confinement: Some(confinement),
            ran,
        })
    }

    fn input(&self, id: u32, bytes: &[u8]) -> Result<(), String> {
        let (link, program) = self.program(id)?;
        link.call(Command::Write {
            chat: program,
            bytes: bytes.to_vec(),
        })
        .map(|_| ())
    }

    fn resize(&self, id: u32, size: Size) -> Result<(), String> {
        let (link, program) = self.program(id)?;
        link.call(Command::Resize {
            chat: program,
            cols: size.columns,
            rows: size.rows,
        })
        .map(|_| ())
    }

    fn watch(&self, id: u32, sink: Sink) -> Result<Watching, String> {
        let (link, program) = self.program(id)?;
        let attached = match link.call(Command::AttachView {
            program,
            kind: ViewKind::Terminal,
            role: None,
            rate: None,
        })? {
            Answer::Attached(attached) => attached,
            other => return Err(format!("the session host answered a view with {other:?}")),
        };
        // Listed before it draws, so its first snapshot finds whose view it is (#2496).
        if let Some(program) = lock(&self.running).get_mut(&id) {
            program.views.push(attached.view);
        }
        link.draw(attached.view, sink);
        Ok(Watching {
            view: attached.view,
            size: Size {
                columns: attached.cols,
                rows: attached.rows,
            },
        })
    }

    fn unwatch(&self, id: u32, view: u32) -> Result<(), String> {
        let (link, program) = self.program(id)?;
        let had = lock(&self.running).get_mut(&id).is_some_and(|program| {
            let had = program.views.contains(&view);
            program.views.retain(|one| *one != view);
            had
        });
        if !had {
            return Err(format!("session {id} has no view {view}"));
        }
        link.undraw(view);
        link.send(Command::DetachView { program, view });
        Ok(())
    }

    fn close(&self, id: u32) -> Result<(), String> {
        if let Some(relayed) = self.relayed() {
            relayed.ended(id, None);
        }
        let program = lock(&self.running).remove(&id).ok_or_else(|| gone(id))?;
        lock(&self.lost).remove(&id);
        // A chat put back under its number starts reconnected (#2496).
        let dimmed = {
            let mut dimmed = lock(&self.reconnecting);
            dimmed
                .chats
                .remove(&id)
                .then(|| dimmed.tell.clone())
                .flatten()
        };
        if let Some(tell) = dimmed {
            tell(vec![id], Reconnection::Reconnected);
        }
        if let Ok(link) = self.own_link(Duration::ZERO) {
            for view in &program.views {
                link.undraw(*view);
            }
            // Answered once the host has let go of it, before its program has ended: its
            // number is free for a chat put back under it.
            if let Err(why) = link.call(Command::Close {
                program: program.id,
            }) {
                tracing::warn!("purlis: the session host did not close chat {id}: {why}");
            }
        }
        Ok(())
    }

    /// Every program is stopped, which answers once it is gone, then closed.
    fn end_all(&self) {
        self.to_every_program(|chat| Command::Stop { chat });
        self.to_every_program(|program| Command::Close { program });
        lock(&self.running).clear();
    }

    fn stop_every_program(&self) -> usize {
        self.to_every_program(|chat| Command::Stop { chat })
    }

    fn stopped_by(&mut self, switch: Arc<crate::killswitch::KillSwitch>) {
        self.kill_switch = switch;
    }

    fn process_id(&self, id: u32) -> Option<u32> {
        lock(&self.running).get(&id)?.process
    }

    fn when_one_ends(&self, tell: Ends) {
        *lock(&self.ended) = Some(tell);
    }

    fn when_reconnecting(&self, tell: Reconnecting) {
        lock(&self.reconnecting).tell = Some(tell);
    }

    fn reconnecting(&self) -> Vec<u32> {
        lock(&self.reconnecting).chats.iter().copied().collect()
    }

    fn lost_with_its_host(&self, id: u32) -> bool {
        lock(&self.lost).contains(&id)
    }

    fn running(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = lock(&self.running).keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    fn readiness(&self, id: u32) -> Result<Readiness, String> {
        let (link, program) = self.program(id)?;
        match link.call(Command::Readiness { program })? {
            Answer::Readiness(readiness) => Ok(Readiness {
                edits_lines: readiness.edits_lines,
                quiet_for: Duration::from_millis(readiness.quiet_for_ms),
            }),
            other => Err(format!(
                "the session host answered readiness with {other:?}"
            )),
        }
    }

    /// The host deals it (#2000). With no host there is no chat to start under it, so a
    /// number past every one the window knows is taken, and the start says why it cannot run.
    fn deal(&self) -> u32 {
        self.dealt_by_the_host().unwrap_or_else(|why| {
            tracing::warn!("purlis: the session host dealt no number ({why})");
            // Never wrapped round to a chat's that runs: past the last number, the last.
            self.opened
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .map_or(u32::MAX, |was| was + 1)
        })
    }

    fn dealt(&self) -> u32 {
        self.opened.load(Ordering::Relaxed)
    }

    /// Kept, and told to the host as it holds the project: a project the host holds already
    /// is told now.
    fn already_dealt(&self, dealt: u32) {
        self.opened.fetch_max(dealt, Ordering::Relaxed);
        let held = lock(&self.held).clone();
        if let (Some(Held { project, on }), Ok(link)) = (held, self.own_link(Duration::ZERO))
            && on == link.id
        {
            link.send(Command::DealtAlready { project, dealt });
        }
    }

    fn reports_to(&self) -> Option<PathBuf> {
        self.relayed().map(|relayed| relayed.socket().to_path_buf())
    }

    fn hear_through(&self, relayed: Arc<Relayed>) {
        *lock(&self.relayed) = Some(relayed);
    }

    fn hold_here(&self) {
        if let Err(why) = self.link() {
            tracing::warn!("purlis: the session host does not hold the project yet: {why}");
        }
    }

    /// Fetched over this project's own link to the runner, into each clone of `workspace`
    /// (#2046). Nothing where the runner shares this machine's disk, or the project is not
    /// held on a pool's runner.
    fn fetch_back(&self, workspace: &str, pool: &str) -> Vec<String> {
        if !self.kept_on_the_runner {
            return Vec::new();
        }
        match self.link() {
            Ok((link, project)) => {
                crate::runnercode::fetch_workspace(&link, &project, &self.root, workspace, pool)
            }
            Err(why) => vec![why],
        }
    }

    /// Saved in the session host, where this project's chats run: the window starts no worker
    /// of its own (#2002).
    fn keep_saving(
        &self,
        root: &Path,
        told: crate::autosave::Told,
    ) -> Box<dyn crate::autosave::Saver> {
        Box::new(SavedInTheHost::start(
            Holding {
                root: root.to_path_buf(),
                ..self.holding()
            },
            told,
        ))
    }

    /// Started in the session host from the same plan, and held there for as long as the chat
    /// runs: what its proxy does is told to `hearing` as the host pushes it (#1999).
    fn confine(
        &self,
        plan: &purlis_core::sandbox::plan::Plan,
        hearing: purlis_core::sandbox::plan::Hearing,
    ) -> Result<purlis_core::sandbox::Confinement, String> {
        use crate::hostedconfinement::{Hears, HeldInTheHost, placed, wire};
        let (link, project) = self.link()?;
        let id = purlis_core::reopen::mint();
        let asks = hearing.asks.is_some();
        // Heard before it is asked for, so nothing its proxy does is missed.
        link.hear(&id, Hears::of(hearing));
        let answered = link.call(Command::Confine(Box::new(session::Confine {
            confinement: id.clone(),
            project,
            plan: wire(plan),
            asks,
        })));
        let confined = match answered {
            Ok(Answer::Confined(confined)) => confined,
            other => {
                link.unhear(&id);
                return Err(match other {
                    Ok(other) => format!("the session host answered a confinement with {other:?}"),
                    Err(why) => why,
                });
            }
        };
        // Held from here: dropping it, on any path, lets the host end it.
        let held = HeldInTheHost::new(id, Arc::clone(&link), asks);
        self.held(placed(&confined), held)
            .map_err(|why| why.to_string())
    }

    fn composes_its_harness_chats(&self) -> bool {
        self.on_a_runner
    }

    fn state_not_reattached(&self) -> Option<String> {
        lock(&self.untaken).clone()
    }

    /// Held in the host from before this window, which holds the project now to say.
    fn left_running(&self, number: u32) -> bool {
        self.link().is_ok() && lock(&self.left).contains_key(&number)
    }

    fn reattach(&self, number: u32, announce: &dyn Fn(u32)) -> Result<u32, String> {
        let (link, _) = self.link()?;
        let program = lock(&self.left)
            .remove(&number)
            .ok_or_else(|| format!("chat {number} is not in the session host to be reattached"))?;
        self.opened.fetch_max(number, Ordering::Relaxed);
        lock(&self.lost).remove(&number);
        // Before anything is told of it, as for a start.
        announce(number);
        let pid = program.exit.is_none().then_some(program.process).flatten();
        self.runs(&link, number, &program.program, pid);
        // It ended while no window was there: told so now, as the host told it then.
        if let Some(exit) = program.exit {
            link.ended_before(&program.program, exit);
        }
        Ok(number)
    }

    /// Its proxy heard by this window from now on, and held by it: closing the chat lets the
    /// host end it, as for one this window started.
    fn confined_again(
        &self,
        number: u32,
        hearing: purlis_core::sandbox::plan::Hearing,
    ) -> Option<Result<purlis_core::sandbox::Confinement, String>> {
        use crate::hostedconfinement::{Hears, HeldInTheHost, placed};
        let (link, _) = self.link().ok()?;
        let confined = lock(&self.left).get(&number)?.confined.clone()?;
        let id = confined.confinement.clone();
        let asks = hearing.asks.is_some();
        link.hear(&id, Hears::of(hearing));
        let held = HeldInTheHost::new(id, Arc::clone(&link), asks);
        Some(
            self.held(placed(&confined), held)
                .map_err(|why| why.to_string()),
        )
    }

    /// The plan the window that confined it sent: one the host's record cannot be read back
    /// into is none, which the window takes as a sandbox that changed.
    fn ran_with(&self, number: u32) -> Option<purlis_core::sandbox::plan::Plan> {
        self.link().ok()?;
        let plan = lock(&self.left).get(&number)?.plan.clone()?;
        crate::hostedconfinement::plan_of(&plan)
            .inspect_err(|why| {
                tracing::warn!("purlis: the plan chat {number} ran in could not be read: {why}");
            })
            .ok()
    }

    /// How the host says it applied the sandbox to chat `number`'s run: none from a host before
    /// 1.28 (#2636).
    fn ran_in(&self, number: u32) -> Option<purlis_session_protocol::session::RunSandbox> {
        self.link().ok()?;
        lock(&self.left).get(&number)?.sandbox.clone()
    }

    fn close_what_was_not_reattached(&self) {
        if self.link().is_err() {
            return;
        }
        let left: Vec<HeldProgram> = lock(&self.left).drain().map(|(_, left)| left).collect();
        let Ok(link) = self.own_link(Duration::ZERO) else {
            return;
        };
        for left in left {
            tracing::info!(
                "purlis: chat {} was left running in the session host and nothing reattached \
                 it, so it is closed",
                left.number
            );
            link.send(Command::Close {
                program: left.program,
            });
        }
    }
}

impl HostedSessions {
    /// Chat `id` runs as `program` in the host, `pid` its process while it runs: what hears its
    /// lines reads them from it, and its end is told as the host says it (#1998), whether it
    /// was started here or reattached (#2006).
    fn runs(&self, link: &HostLink, id: u32, program: &str, pid: Option<u32>) {
        // Its program, which the host reads every line for the chat from: what a caller that
        // waited compares (qw381).
        let relayed = self.relayed();
        if let (Some(relayed), Some(pid)) = (&relayed, pid) {
            relayed.started(id, pid);
        }
        // Listed first, so its end, told at once for one that ended before, finds it.
        lock(&self.running).insert(
            id,
            Program {
                id: program.to_owned(),
                process: pid,
                views: Vec::new(),
            },
        );
        // No hook reports a program dying: the host says so, as the operating system told it.
        let tell = lock(&self.ended).clone();
        let running = Arc::clone(&self.running);
        let lost = Arc::clone(&self.lost);
        let ended = program.to_owned();
        link.when_it_ends(
            program,
            Box::new(move |ending| {
                // Only the program that ended: a chat closed and put back under its number is
                // another program, which runs on.
                match lock(&running).get_mut(&id) {
                    Some(chat) if chat.id == ended => chat.process = None,
                    _ => return,
                }
                if let Some(relayed) = &relayed {
                    relayed.ended(id, pid);
                }
                let exit = match ending {
                    Ending::Exited(Exit::Code(code)) => purlis_core::session::Exit::Code(code),
                    Ending::Exited(Exit::Signal(signal)) => {
                        purlis_core::session::Exit::Signal(signal)
                    }
                    // Its terminal closed with its host, which hangs its program up.
                    Ending::WithTheHost => {
                        lock(&lost).insert(id);
                        purlis_core::session::Exit::Signal(ENDED_WITH_THE_HOST.to_owned())
                    }
                };
                if let Some(tell) = &tell {
                    tell(id, exit);
                }
            }),
        );
    }
}

fn gone(id: u32) -> String {
    format!("session {id} is not running")
}

fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A link to the window's host, opened by `opening` on `runtime` and waited for at most
/// `within`, from any thread but the runtime's own: what a [`Dial`] answers.
pub(crate) fn open_within(
    runtime: &tokio::runtime::Handle,
    within: Duration,
    opening: impl std::future::Future<Output = Result<HostLink, String>> + Send + 'static,
) -> Result<HostLink, String> {
    let (opened, link) = std::sync::mpsc::channel();
    runtime.spawn(async move {
        // A link that opens once nobody waits for it any more is dropped here.
        let _ = opened.send(opening.await);
    });
    link.recv_timeout(within)
        .map_err(|_| "it did not open another link in time".to_owned())?
}

#[cfg(test)]
pub(crate) mod tests;
