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

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
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
                Some(config) => Box::new(crate::routedsessions::RoutedSessions::of_the_project(
                    here,
                    &connection,
                    config,
                    root,
                )),
                None => here,
            }
        }))
        .hearing_hooks_through_the_host()
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

/// What a command no host answered in time says.
const NOT_IN_TIME: &str = "the session host did not answer in time";

/// **What one send of code is held to** (spec #4275): the most bytes, the longest time, what the
/// host is told of them, and the limit a send that ran out of time hit.
struct SendHeld {
    most: u64,
    within: Duration,
    given: Option<session::CodeHeld>,
    late: Option<purlis_core::poollimits::LimitHit>,
}

impl SendHeld {
    /// A host's own bounds, which no limit moves: a moving chat's conversation.
    fn fixed(most: u64) -> SendHeld {
        SendHeld {
            most,
            within: purlis_host::TRANSFER_WITHIN,
            given: None,
            late: None,
        }
    }
}

/// **How often a chat's start looks whether the person cancelled it** (#4264). A start has no limit
/// of its own: a busy host is still coming up, so it waits until the host answers, its link ends,
/// which is the host having failed, or the person cancels.
const LOOKS_FOR_A_CANCEL_EVERY: Duration = Duration::from_millis(100);

/// What a window's link and a start's dial wait: as long as the supervisor takes to say the host
/// is ready or gone, which it always comes to say.
const UNTIL_THE_HOST_SAYS: Duration = Duration::from_secs(24 * 60 * 60);

/// What a start the person cancelled says: never shown, since its dialog has closed.
pub(crate) const START_CANCELLED: &str = "the person cancelled this start";

/// The code of a start that was cancelled, which is the window's own, never a host's.
const CANCELLED: &str = "cancelled";

/// What anything but a start waiting for it says while a start waits for its runner (#4415): it
/// waits on nothing, so the window's commands never wait on a runner coming back.
const THE_RUNNER_COMES_BACK: &str =
    "this project's runner is on its way back, so it is not held yet";

std::thread_local! {
    /// The person's Cancel of the start this thread runs, while one can be cancelled (#4264).
    static CANCEL: std::cell::RefCell<Option<Arc<std::sync::atomic::AtomicBool>>> =
        const { std::cell::RefCell::new(None) };
    /// Whether this thread runs a dispatch's start, which holds no lock around it (#4415).
    static A_DISPATCHS_START: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs `start` as a dispatch's start: it waits for a runner on its way back as a person's does,
/// with no Cancel, for at most [`a_dispatchs_hold_within`] (#4415).
pub(crate) fn a_dispatchs_start<T>(start: impl FnOnce() -> T) -> T {
    struct Forgets;
    impl Drop for Forgets {
        fn drop(&mut self) {
            A_DISPATCHS_START.set(false);
        }
    }
    A_DISPATCHS_START.set(true);
    let _forgets = Forgets;
    start()
}

/// Whether this thread's start may wait for a runner on its way back: the person's, and a
/// dispatch's. Any other may run under a lock window commands wait on.
fn waits_for_its_runner() -> bool {
    can_be_cancelled() || A_DISPATCHS_START.get()
}

/// Runs `start` with `cancel` able to cancel it: a start from the person's dialog.
pub(crate) fn cancelled_by<T>(
    cancel: Arc<std::sync::atomic::AtomicBool>,
    start: impl FnOnce() -> T,
) -> T {
    struct Forgets;
    impl Drop for Forgets {
        fn drop(&mut self) {
            CANCEL.with_borrow_mut(|held| *held = None);
        }
    }
    CANCEL.with_borrow_mut(|held| *held = Some(cancel));
    let _forgets = Forgets;
    start()
}

/// Whether this thread runs a start its person can cancel: only such a start waits without limit,
/// since nothing else can end its wait.
fn can_be_cancelled() -> bool {
    CANCEL.with_borrow(Option::is_some)
}

/// The starts each project's window has running that its person can cancel, by the project's root.
static CANCELS: std::sync::LazyLock<
    Mutex<HashMap<PathBuf, Vec<std::sync::Weak<std::sync::atomic::AtomicBool>>>>,
> = std::sync::LazyLock::new(Mutex::default);

/// A new start the window's person can cancel, in the project at `root`.
pub(crate) fn a_cancellable_start(root: &Path) -> Arc<std::sync::atomic::AtomicBool> {
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut starts = lock(&CANCELS);
    let of_it = starts.entry(root.to_path_buf()).or_default();
    of_it.retain(|one| one.strong_count() > 0);
    of_it.push(Arc::downgrade(&cancel));
    cancel
}

/// **The person pressed Cancel on a start in the project at `root`** (#4264).
pub(crate) fn cancel_starts(root: &Path) {
    if let Some(starts) = lock(&CANCELS).remove(root) {
        for start in starts.iter().filter_map(std::sync::Weak::upgrade) {
            start.store(true, Ordering::SeqCst);
        }
    }
}

fn was_cancelled() -> bool {
    CANCEL.with_borrow(|held| held.as_ref().is_some_and(|one| one.load(Ordering::SeqCst)))
}

/// How a chat whose program ended with its session host is said to have ended: by the hang-up
/// its terminal's close gives it (#2007).
pub const ENDED_WITH_THE_HOST: &str = "Hangup";

/// The signal a host said a program ended by. It is free text from the host, and
/// [`ITS_RUNNER_WENT`] is the window's word alone, so a host saying it says something else.
fn a_hosts_signal(signal: String) -> String {
    if signal == ITS_RUNNER_WENT {
        format!("{signal} (as its host said)")
    } else {
        signal
    }
}

/// What is told of a chat that already ended with its host when its runner then goes for good
/// (#4421): its program did not end again, and whoever hears this only learns the second part.
pub const ITS_RUNNER_WENT: &str = "RunnerGone";

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
    /// The links each project root was held on until their runner began to drain (#4248): each
    /// still carries what was started over it, until its runner is kept no more.
    left_on_draining: Mutex<HashMap<PathBuf, Vec<Arc<HostLink>>>>,
    /// Each project on this connection, heard from when one of its runners goes (#4421).
    hear_of_gone_runners: Mutex<Vec<HearsOfGoneRunners>>,
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
    /// host relays to that pool's runner (#2046): it waits on, and dials through, that one. Or
    /// `<pool>/<runner>`, for one that reaches a draining runner of the pool (#4271).
    to_a_pool: Option<(Arc<Connection>, String)>,
    /// The window's own link that this connection's project links were opened beside: one to a
    /// host started again after it ended holds every root on a link of its own anew (#2007).
    window_was: Mutex<Option<u64>>,
    /// The window's connection to each pool's runner, made the first time it is asked for
    /// ([`Connection::to_a_pool`]): what a pool's chats and their browsers share (#2050).
    pools: Mutex<HashMap<String, Arc<Connection>>>,
    /// How many times this connection's pool was removed ([`Connection::pool_removed`]): a link
    /// dialled before the last of them is not kept.
    removals: AtomicU64,
    /// The config home this machine keeps its pools' runners in (`runners.json`): where a runner
    /// a chat ran on without the sandbox is kept as one (D-2050-27). Set on the window's own.
    runners_in: Mutex<Option<PathBuf>>,
    /// The pools a chat ran on, or may have run on, without the sandbox since the window
    /// started, kept on the window's own: what holds them untrusted where `runners_in` could not
    /// be written (D-2050-27).
    untrusted: Mutex<std::collections::HashSet<String>>,
    /// Ends the browsers the window started on a pool once it is untrusted (D-2050-28), on the
    /// window's own: told the pool, off no link's thread.
    ending_browsers: Mutex<Option<EndingBrowsers>>,
}

/// The pool of what a connection's links name: a pool, or `<pool>/<runner>` (#4271).
fn pool_named(named: &str) -> &str {
    named.split_once('/').map_or(named, |(pool, _)| pool)
}

/// What ends the browsers the window started on the pool it is told (D-2050-28).
pub type EndingBrowsers = Arc<dyn Fn(&str) + Send + Sync>;

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

/// A chat whose program ended with its host.
struct LostChat {
    /// That host was a runner gone for good, then or since (#4350, #4421).
    with_the_runner: bool,
    /// The runner it ended on, as [`HostLink::reached_runner`] names it, where its host said.
    on: Option<String>,
}

/// The chats of one project whose program ended with their host, by number.
type LostChats = Arc<Mutex<HashMap<u32, LostChat>>>;

/// What a connection tells a project's chats through when a runner goes (#4421). Weak: a project
/// let go of hears no more, and its next launch reads its tasks from their records.
struct HearsOfGoneRunners {
    lost: std::sync::Weak<Mutex<HashMap<u32, LostChat>>>,
    ended: std::sync::Weak<Mutex<Option<Ends>>>,
}

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

    /// **The pool `pool` was removed from this machine**: every link the window holds to its
    /// runner ends, each chat on it told it ended with that runner (#4350), and whatever comes
    /// next is dialled anew, by the name, to the runner kept under it now. What the window held
    /// against that name goes too: a pool added under it is a new runner.
    pub fn pool_removed(&self, pool: &str) {
        lock(&self.untrusted).remove(pool);
        for link in self.links_of_pool(pool) {
            link.end_with_its_runner();
        }
        let of_it = lock(&self.pools).get(pool).cloned();
        if let Some(of_it) = of_it {
            of_it.runner_went(&|_| true);
        }
    }

    /// `lost` and `ended`, one project's, hear when a runner of this connection goes.
    fn hears_of_gone_runners(&self, lost: &LostChats, ended: &Arc<Mutex<Option<Ends>>>) {
        let mut hearing = lock(&self.hear_of_gone_runners);
        hearing.retain(|one| one.lost.strong_count() > 0);
        hearing.push(HearsOfGoneRunners {
            lost: Arc::downgrade(lost),
            ended: Arc::downgrade(ended),
        });
    }

    /// **A runner went for good, and chats that ended earlier were on it** (#4421): `went` says
    /// which, by the runner each ended on. Each is marked and told once. Nothing resumes it now.
    fn runner_went(&self, went: &dyn Fn(Option<&str>) -> bool) {
        let hearing: Vec<(LostChats, Arc<Mutex<Option<Ends>>>)> = lock(&self.hear_of_gone_runners)
            .iter()
            .filter_map(|one| Some((one.lost.upgrade()?, one.ended.upgrade()?)))
            .collect();
        for (lost, ended) in hearing {
            let marked: Vec<u32> = lock(&lost)
                .iter_mut()
                .filter(|(_, chat)| !chat.with_the_runner && went(chat.on.as_deref()))
                .map(|(id, chat)| {
                    chat.with_the_runner = true;
                    *id
                })
                .collect();
            let tell = lock(&ended).clone();
            if let Some(tell) = tell.filter(|_| !marked.is_empty()) {
                // Off the caller's thread: each tell writes files and types into the asking chat.
                let told = std::thread::Builder::new()
                    .name("purlis-runner-went".into())
                    .spawn(move || {
                        for id in marked {
                            tell(
                                id,
                                purlis_core::session::Exit::Signal(ITS_RUNNER_WENT.to_owned()),
                            );
                        }
                    });
                if let Err(why) = told {
                    tracing::warn!("purlis: chats on a gone runner were not told: {why}");
                }
            }
        }
    }

    /// **Each link to a runner kept no more ends** (#4168): made again in its place, or removed
    /// outside purlis as it drained, its chats do not exist on any runner now, so each is told it
    /// ended with that runner (#4350) rather than staying dimmed as if it would come back. Only for a Docker pool whose
    /// runners this machine reads; a list that cannot be read, or is empty, says nothing.
    pub fn links_to_gone_runners_end(&self, config: &Path) {
        let Some(kept) = purlis_core::runners::read(config)
            .ok()
            .filter(|kept| !kept.is_empty())
        else {
            return;
        };
        let gone = |link: &Arc<HostLink>| {
            link.reached_runner()
                .is_some_and(|runner| !kept.iter().any(|one| one.name() == runner))
        };
        let pools: Vec<(String, Arc<Connection>)> = lock(&self.pools)
            .iter()
            .map(|(pool, of_it)| (pool.clone(), Arc::clone(of_it)))
            .collect();
        for (pool, of_it) in pools {
            if purlis_core::runners::runner_for(config, pool_named(&pool)).is_err() {
                continue;
            }
            let mut ended = Vec::new();
            lock(&of_it.projects).retain(|_, link| {
                let keep = !gone(link);
                if !keep {
                    ended.push(Arc::clone(link));
                }
                keep
            });
            for left in lock(&of_it.left_on_draining).values_mut() {
                left.retain(|link| {
                    let keep = !gone(link);
                    if !keep {
                        ended.push(Arc::clone(link));
                    }
                    keep
                });
            }
            for link in ended {
                link.end_with_its_runner();
            }
            of_it.runner_went(&|on| {
                on.is_some_and(|runner| !kept.iter().any(|one| one.name() == runner))
            });
        }
    }

    /// Every link the window holds to the runner of `pool` ends, each chat on it told it ended
    /// with its host, and whatever comes next is dialled anew.
    pub fn links_to_pool_end(&self, pool: &str) {
        for link in self.links_of_pool(pool) {
            link.end();
        }
    }

    /// Every link the window holds to a runner of `pool`, its draining ones too (#4271), let go
    /// of for a removal.
    fn links_of_pool(&self, pool: &str) -> Vec<Arc<HostLink>> {
        let of_the_pool: Vec<Arc<Connection>> = lock(&self.pools)
            .iter()
            .filter(|(named, _)| pool_named(named) == pool)
            .map(|(_, of_it)| Arc::clone(of_it))
            .collect();
        let mut links = Vec::new();
        for of_it in of_the_pool {
            of_it.removals.fetch_add(1, Ordering::SeqCst);
            links.extend(lock(&of_it.projects).drain().map(|(_, link)| link));
            links.extend(lock(&of_it.left_on_draining).drain().flat_map(|(_, on)| on));
        }
        links
    }

    /// How many times the pool `pool` was removed.
    #[cfg(test)]
    pub(crate) fn removals_of(&self, pool: &str) -> u64 {
        lock(&self.pools)
            .get(pool)
            .map_or(0, |of_it| of_it.removals.load(Ordering::SeqCst))
    }

    /// Keep this machine's pools' runners, as `runners.json` is read, in `config` (D-2050-27).
    pub fn runners_in(&self, config: PathBuf) {
        *lock(&self.runners_in) = Some(config);
    }

    /// Where this machine keeps its pools' runners: the window's own connection's, for one to
    /// a pool. None where the window keeps none, as a test's host does.
    pub(crate) fn runners_config(&self) -> Option<PathBuf> {
        match &self.to_a_pool {
            Some((window, _)) => lock(&window.runners_in).clone(),
            None => lock(&self.runners_in).clone(),
        }
    }

    /// The window's own connection: this one, or the one a connection to a pool waits on.
    fn window(&self) -> &Connection {
        match &self.to_a_pool {
            Some((window, _)) => window,
            None => self,
        }
    }

    /// The pool whose runner this connection's links reach, where they reach one.
    pub(crate) fn pool(&self) -> Option<&str> {
        self.to_a_pool.as_ref().map(|(_, named)| pool_named(named))
    }

    /// The draining runner this connection's links name, where they name one (#4271).
    fn draining_runner(&self) -> Option<&str> {
        let (_, named) = self.to_a_pool.as_ref()?;
        named.split_once('/').map(|(_, runner)| runner)
    }

    /// Whether this connection names a draining runner that is kept no more (#4271).
    fn asked_a_runner_kept_no_more(&self) -> bool {
        let (Some(runner), Some(config)) = (self.draining_runner(), self.runners_config()) else {
            return false;
        };
        matches!(purlis_core::runners::named(&config, runner), Ok(None))
    }

    /// Whether a runner of this connection's pool drains now: neither runner's fetch removes what
    /// only the other has (#4248), after a window's restart too (#4271).
    fn pool_drains(&self) -> bool {
        let (Some(pool), Some(config)) = (self.pool(), self.runners_config()) else {
            return false;
        };
        purlis_core::runners::draining(&config)
            .is_ok_and(|draining| draining.iter().any(|kept| kept.pool() == pool))
    }

    /// `end` ends the browsers the window started on a pool once a chat ran there, or may have
    /// run there, without the sandbox (D-2050-28).
    pub fn ending_browsers_with(&self, end: EndingBrowsers) {
        *lock(&self.window().ending_browsers) = Some(end);
    }

    /// **A chat ran, or may have run, without the sandbox on this connection's pool's runner**
    /// (D-2050-27, D-2050-28): every runner kept for the pool is kept as one, the window holds the
    /// pool untrusted whatever that answers, and the browsers it started there are ended, their
    /// data folders with them. Nothing for a connection to this machine's host. Answers why the
    /// runners could not be kept so.
    pub(crate) fn ran_unsandboxed(&self) -> Result<(), String> {
        let Some((window, named)) = &self.to_a_pool else {
            return Ok(());
        };
        let pool = pool_named(named);
        lock(&window.untrusted).insert(pool.to_owned());
        let kept = match window.runners_config() {
            Some(config) => purlis_core::runnertaint::mark_pool(&config, pool),
            None => Ok(()),
        };
        let ending = lock(&window.ending_browsers).clone();
        if let Some(end) = ending {
            end(pool);
        }
        kept
    }

    /// Whether the window holds `pool` untrusted since a chat ran, or may have run, there without
    /// the sandbox (D-2050-27): what is kept in `runners_in` aside.
    pub(crate) fn untrusts(&self, pool: &str) -> bool {
        lock(&self.window().untrusted).contains(pool)
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
                lock(&self.left_on_draining).clear();
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
        let removals = self.removals.load(Ordering::SeqCst);
        // Named on the link as its first command, before it subscribes (#2623).
        let naming = self.name_for(root);
        let link = Arc::new(match &self.to_a_pool {
            Some((window, pool)) => {
                let relayed = lock(&window.relayed).clone().ok_or_else(|| {
                    no_host("there is no way to reach a pool's runner through it")
                })?;
                relayed(within, naming, pool)
                    .map_err(|why| {
                        format!(
                            "This workspace's chats run on pool {pool}, and its runner was not \
                             reached, so nothing was sent: {why}"
                        )
                    })?
                    .with_pools_in(self.runners_config())
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
        let mut projects = lock(&self.projects);
        // A link dialled across its pool's removal is not kept.
        if self.removals.load(Ordering::SeqCst) != removals {
            return Err(format!(
                "pool {} was removed as its runner was being reached, so nothing was sent",
                self.pool().unwrap_or_default()
            ));
        }
        projects.insert(root.to_path_buf(), Arc::clone(&link));
        drop(projects);
        Ok(link)
    }

    /// **`root`'s link reached a runner that drains now, or is kept no more, and its pool has a
    /// runner to go to**
    /// (#4248): the link is answered and kept apart for what was started over it, and the next ask
    /// dials the pool's name, which reaches the pool's runner now. Each link kept apart whose
    /// runner is kept no more ends. A list that cannot be read, or that is empty, says nothing.
    fn leaves_a_draining_runner(&self, root: &Path) -> Option<Arc<HostLink>> {
        let (pool, config) = (self.pool()?, self.runners_config()?);
        let kept = purlis_core::runners::read(&config)
            .ok()
            .filter(|kept| !kept.is_empty())?;
        let mut ended = Vec::new();
        if let Some(left) = lock(&self.left_on_draining).get_mut(root) {
            left.retain(|link| {
                let keep = link
                    .reached_runner()
                    .is_none_or(|runner| kept.iter().any(|one| one.name() == runner));
                if !keep {
                    ended.push(Arc::clone(link));
                }
                keep
            });
        }
        for link in ended {
            link.end();
        }
        let drains = lock(&self.projects)
            .get(root)
            .and_then(|link| link.reached_runner().map(str::to_owned))
            .is_some_and(|runner| {
                // A runner made again in its place leaves the link naming one kept no more, which
                // the desktop's host refuses a start on as it does a draining one's (#4149).
                !kept.iter().any(|one| one.name() == runner)
                    || kept
                        .iter()
                        .any(|one| one.draining && one.pool() == pool && one.name() == runner)
            });
        if !drains || purlis_core::runners::runner_for(&config, pool).is_err() {
            return None;
        }
        let link = lock(&self.projects).remove(root)?;
        tracing::info!(
            "purlis: pool {pool}'s runner {} drains: a project's next chats go to the pool's \
             runner now, and its link stays for the chats already there",
            link.reached_runner().unwrap_or_default()
        );
        lock(&self.left_on_draining)
            .entry(root.to_path_buf())
            .or_default()
            .push(Arc::clone(&link));
        Some(link)
    }

    /// Link `id` left on a draining runner is let go of.
    fn lets_go_of_left(&self, root: &Path, id: u64) {
        if let Some(left) = lock(&self.left_on_draining).get_mut(root) {
            left.retain(|link| link.id != id);
        }
    }

    /// Every link open for `root` now: the one it is held on, and each left on a draining runner.
    pub(crate) fn links_of(&self, root: &Path) -> Vec<Arc<HostLink>> {
        let left = lock(&self.left_on_draining)
            .get(root)
            .cloned()
            .unwrap_or_default();
        lock(&self.projects)
            .get(root)
            .cloned()
            .into_iter()
            .chain(left)
            .collect()
    }

    /// **The name the window keeps for `root`'s link** (`purlis_core::windownames`, #2623),
    /// where the window names its links: never evicted while a link of this window holds a root.
    /// A name that cannot be kept leaves the link unnamed, one the window takes nothing back on
    /// after a restart. The name is never said.
    fn name_for(&self, root: &Path) -> Option<purlis_session_protocol::browsing::Secret> {
        // A link to a pool's runner is named apart from the root's link to this machine's host,
        // and the same each time: one name is never two links' (#2046).
        let (config, keyed) = match &self.to_a_pool {
            // The one name for a pool, its draining runners too: a name takes back only what that
            // runner started for it, and each of them started under this one (#4271).
            Some((window, named)) => (
                lock(&window.names_in).clone()?,
                root.join(RELAYED_NAMES).join(pool_named(named)),
            ),
            None => (lock(&self.names_in).clone()?, root.to_path_buf()),
        };
        let mut open: Vec<PathBuf> = lock(&self.projects).keys().cloned().collect();
        if let Some(pool) = self.pool() {
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

    fn runners(&self) -> Result<Vec<String>, String> {
        Ok(self.kill_switch_after(Command::KillSwitch)?.runners)
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

/// What a moving chat's conversation is named by on a link (#2048).
fn carried(chat: &crate::host::Moving, file: &str) -> purlis_host::Carried {
    purlis_host::Carried {
        workspace: chat.workspace.clone(),
        profile: chat.profile.clone(),
        conversation: chat.conversation.clone(),
        file: file.to_owned(),
    }
}

/// A moving chat's conversation as this machine keeps it, in the project at `root`.
fn of_a_chat<'a>(
    root: &'a Path,
    chat: &'a crate::host::Moving,
) -> purlis_core::conversationmove::OfAChat<'a> {
    purlis_core::conversationmove::OfAChat {
        root,
        workspace: &chat.workspace,
        profile: &chat.profile,
        conversation: &chat.conversation,
    }
}

/// A transfer's command, as it goes to the host and is answered.
type Transferred<T> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<T, session::CallError>> + Send + 'static>,
>;

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
    /// Where a view of a browser's DevTools is told why they closed, its last frame (#2028).
    pub closed: Option<ClosedSink>,
}

/// Where a view of a browser's DevTools is told why they closed (1.45, #2028): one of
/// `view::frames::CLOSED_WHY`.
pub type ClosedSink = Box<dyn FnMut(&'static str) + Send>;

/// **The window's own hold on a runner browser's frames** (#2050): the runner's host is held to
/// the link's frames, and the window holds it to them too, whatever it sends. A frame larger than
/// [`MOST_HELD_FRAME_BYTES`], one whose header says it is wider than the link's widest or
/// higher than [`MOST_HELD_FRAME_HEIGHT`] (read before anything decodes it), or one that comes
/// sooner than [`HeldFrames::at_least`] after the last drawn, is acknowledged and not drawn. The
/// link's frames are its pool's own where its runner was given them (#3297), else the defaults.
#[derive(Debug, Default)]
pub struct HeldFrames {
    last: Option<Instant>,
    link: purlis_session_protocol::browsing::LinkFrames,
}

/// The largest frame of a runner's browser the window draws.
pub const MOST_HELD_FRAME_BYTES: usize = 4 << 20;

/// The highest frame of a runner's browser the window draws, in pixels.
pub const MOST_HELD_FRAME_HEIGHT: u16 = 8192;

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
    /// The window's hold on a browser whose views over the link are held to `link` (#3297).
    pub fn to(link: purlis_session_protocol::browsing::LinkFrames) -> HeldFrames {
        HeldFrames { last: None, link }
    }

    /// The least time between two frames drawn: three quarters of the link's own, so a frame
    /// the link bunched is not lost (50 ms at 15 a second).
    pub fn at_least(&self) -> Duration {
        Duration::from_millis(750) / self.link.frames.max(1)
    }

    /// Whether `image`, coming `now`, is drawn.
    pub fn draws(&mut self, image: &[u8], now: Instant) -> bool {
        if image.len() > MOST_HELD_FRAME_BYTES {
            return false;
        }
        let Some((width, height)) = purlis_session_protocol::view::frames::jpeg_size(image) else {
            return false;
        };
        if width > self.link.width || height > MOST_HELD_FRAME_HEIGHT {
            return false;
        }
        let at_least = self.at_least();
        if self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) < at_least)
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
    /// Whether the window ended it ([`HostLink::end`]): what hears it then runs to its end.
    ended: std::sync::atomic::AtomicBool,
    /// Whether it ended because its runner is gone for good ([`HostLink::end_with_its_runner`]):
    /// its chats' conversations went with that runner's home (#4350).
    runner_gone: Arc<std::sync::atomic::AtomicBool>,
    /// Why the host did not take the window's name on it, where it was named and did not
    /// (#2623): said by whoever opened it, never with the name.
    unnamed: Option<String>,
    /// The most bytes one bundle a host writes back may carry (1.34, #2046): fixed, whatever a
    /// pool's limits on sends say, so raising them widens nothing a runner can make this machine
    /// take.
    most_bundle_bytes: u64,
    /// The pool whose runner this link reaches, whose limits it follows (spec #4275).
    pool: Option<String>,
    /// Where this machine keeps that pool ([`Connection::runners_config`]): none reads purlis's
    /// defaults, as a test's host does.
    pools_in: Option<PathBuf>,
    /// The runner this link reached, as `<pool>/<runner>`, as its host said it (1.48, #3417).
    reached: Option<String>,
}

/// A link nobody holds any more is closed: one that opened after nobody waited for it, among
/// them.
impl Drop for HostLink {
    fn drop(&mut self) {
        if !self.ended.load(Ordering::SeqCst) {
            self.hearing.abort();
        }
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
    /// What this link holds of the event log of the runner its host relays it to (#3393).
    runner_log: Mutex<crate::runnerlog::RunnerLog>,
    /// How many starts are waiting, off the hold's lock, for the runner to come back (#4415).
    holds_waiting: AtomicUsize,
    /// Who logs each skill run that runner's log says started or ended, by the project's id: each
    /// project held on it in the runner's own checkout (SK-20b, #3084).
    skill_runs: Mutex<HashMap<String, crate::host::SkillRunsHeard>>,
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
    /// The host says the runner was removed outside purlis (`gone`, #4149): it does not link to
    /// it again until it is made again.
    Gone,
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

/// The folder name of each clone a host holds in a workspace, and the `origin` URL it names
/// for each it can, by folder name (1.54, #3985).
pub(crate) type ClonesHeld = (Vec<String>, BTreeMap<String, String>);

impl HostLink {
    /// What this link holds of its runner's event log, and whether the link to it is down
    /// (#3393).
    pub(crate) fn runner_log(&self) -> crate::runnerlog::RunnerLogNow {
        lock(&self.shared.runner_log).now()
    }

    /// The session protocol version the host agreed on.
    pub fn version(&self) -> purlis_session_protocol::version::Version {
        self.client.version()
    }

    /// The runner this link reached, by its name in its pool (1.48, #3417): none on a link to this
    /// machine's host, or from a host that names none.
    pub(crate) fn reached_runner(&self) -> Option<&str> {
        let (_, runner) = self.reached.as_deref()?.split_once('/')?;
        Some(runner.split_once('@').map_or(runner, |(runner, _)| runner))
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
        // The runner the link reached, as its host says (1.48, #3417): a draining one too.
        let mut reached = None;
        if let Some(runner) = relaying {
            let mut relayed = client.relay_to(runner).await;
            let name_held = |sent: &Result<Option<String>, session::CallError>| {
                sent.as_ref()
                    .err()
                    .and_then(session::CallError::refusal)
                    .is_some_and(|refusal| refusal.code == session::NOT_ALLOWED)
            };
            if named && unnamed.is_none() && name_held(&relayed) {
                tokio::time::sleep(NAMED_AGAIN_AFTER).await;
                relayed = client.relay_to(runner).await;
            }
            reached = relayed.map_err(|why| {
                format!("the session host did not relay the link to runner {runner}: {why}")
            })?;
        }
        let shared = Arc::new(Shared::default());
        // A runner's own event log is read from where it ends now, with a cursor for its device
        // (#3393): a runner before 1.41 says no end, and none of its log is read.
        if relaying.is_some()
            && let Ok((device, seq)) = client.log_end().await
        {
            lock(&shared.runner_log).starts_at(device, seq);
        }
        let since = lock(&shared.runner_log).since();
        client
            .subscribe(since)
            .await
            .map_err(|why| format!("the session host refused to tell of its programs: {why}"))?;
        static LINKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = LINKS.fetch_add(1, Ordering::Relaxed);
        runtime.spawn(take_views(views, Arc::clone(&shared)));
        let hearing = hear(
            events,
            Arc::clone(&client),
            Arc::clone(&shared),
            HookLines {
                link: id,
                client: Arc::clone(&client),
                runtime: runtime.clone(),
                reached: reached.clone(),
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
            ended: std::sync::atomic::AtomicBool::new(false),
            runner_gone: Arc::default(),
            unnamed,
            most_bundle_bytes: purlis_session_protocol::code::MOST_BUNDLE_BYTES,
            pool: relaying.map(str::to_owned),
            pools_in: None,
            reached,
        })
    }

    /// This link, moving no bundle past `most` bytes: what a test makes a large one with.
    #[cfg(test)]
    pub(crate) fn holding_bundles_to(mut self, most: u64) -> HostLink {
        self.most_bundle_bytes = most;
        self
    }

    /// **Ends this link from the window's side, whoever still holds it**: the host sees it close,
    /// each program on it is told it ended with its host and each pane is told, as when the link
    /// goes, and every later call on it is refused.
    pub(crate) fn end(&self) {
        self.ended.store(true, Ordering::SeqCst);
        self.client.end_link();
    }

    /// [`Self::end`], **its runner being gone for good**: each chat on it says it ended with that
    /// runner, not that nothing was lost (#4350).
    pub(crate) fn end_with_its_runner(&self) {
        self.runner_gone.store(true, Ordering::SeqCst);
        self.end();
    }

    /// This link, reading its pool's limits from `config`.
    fn with_pools_in(mut self, config: Option<PathBuf>) -> HostLink {
        self.pools_in = config;
        self
    }

    /// **This link's pool's limits, as they stand now** (spec #4275): read at each attempt, so a
    /// changed value applies with nothing restarted. Purlis's defaults on a link to no pool, or
    /// where they can't be read.
    fn limits(&self) -> purlis_core::poollimits::Chosen {
        self.pool
            .as_deref()
            .zip(self.pools_in.as_deref())
            .and_then(|(pool, config)| purlis_core::runners::chosen_limits_of(config, pool).ok())
            .unwrap_or_default()
    }

    /// `hit` in words, kept for the window to ask of where this link is a pool's.
    fn limit_said(&self, hit: purlis_core::poollimits::LimitHit) -> String {
        match &self.pool {
            Some(pool) => purlis_core::poollimits::hit_on(pool, hit),
            None => hit.said(),
        }
    }

    /// Send `command` and wait for the host's answer, from any thread: on a link to a pool's
    /// runner, for as long as the pool's limit says (spec #4275). Only the wait changes.
    pub(crate) fn call(&self, command: Command) -> Result<Answer, String> {
        use purlis_core::poollimits::LimitId::AnswerWait;
        if self.pool.is_none() {
            return self.call_within(command, ANSWER_WITHIN);
        }
        let secs = self.limits().in_force(AnswerWait);
        // Only this machine's own clock says the wait ran out: no words of a runner can.
        match self.call_timed(command, Duration::from_secs(secs)) {
            Some(answered) => answered.map_err(|refused| refused.why),
            None => {
                let hit = AnswerWait.limit().waited(secs, None);
                Err(format!(
                    "the runner did not answer in time: {}",
                    self.limit_said(hit)
                ))
            }
        }
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
        self.call_timed(command, within).unwrap_or_else(|| {
            Err(purlis_session_protocol::session::Refusal {
                code: String::new(),
                why: NOT_IN_TIME.to_owned(),
            })
        })
    }

    /// The host's answer to `command`, or `None` where `within` ran out first.
    fn call_timed(
        &self,
        command: Command,
        within: Duration,
    ) -> Option<Result<Answer, purlis_session_protocol::session::Refusal>> {
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
            Ok(answered) => Some(answered.map_err(|why| {
                why.refusal()
                    .cloned()
                    .unwrap_or_else(|| unanswered(why.to_string()))
            })),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Some(Err(unanswered("the session host's link ended".to_owned())))
            }
        }
    }

    /// [`Self::call`], for a command of a chat's start (#4264): waits until the host answers, its
    /// link ends or the person cancels.
    ///
    /// A hold or a deal nobody can cancel, a dispatch's, may run under the lock the window's own
    /// commands wait on, so it keeps a quick command's limit.
    fn call_of_a_start(&self, command: Command) -> Result<Answer, String> {
        if !can_be_cancelled() {
            // It may run under a lock window commands wait on: never a pool's raised wait.
            return self.call_within(command, ANSWER_WITHIN);
        }
        self.call_until_answered(command)
    }

    /// Waits until the host answers, its link ends or the person cancels, where one can.
    fn call_until_answered(&self, command: Command) -> Result<Answer, String> {
        let (answered, answer) = std::sync::mpsc::channel();
        let client = Arc::clone(&self.client);
        self.runtime.spawn(async move {
            let _ = answered.send(client.call(command).await);
        });
        loop {
            match answer.recv_timeout(LOOKS_FOR_A_CANCEL_EVERY) {
                Ok(said) => return said.map_err(|why| why.to_string()),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("the session host's link ended".to_owned());
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) if was_cancelled() => {
                    return Err(START_CANCELLED.to_owned());
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }

    /// [`Self::call_of_a_start`] for a command whose answer must be heard, or what it made is left
    /// behind or misread: a Cancel does not end this wait, and the start stops right after it.
    fn call_heard_in_a_start(&self, command: Command) -> Result<Answer, String> {
        if can_be_cancelled() {
            self.call_within(command, UNTIL_THE_HOST_SAYS)
        } else {
            self.call(command)
        }
    }

    /// **Starts a program, waiting as a start does** (#4264). Where the person cancels, what the
    /// host answers after goes to `late`: the window's giving up and the answer's arrival are
    /// decided under one lock, so a chat that comes up after is never left running unseen.
    fn call_start(
        &self,
        start: StartProgram,
        late: impl FnOnce(Result<Answer, session::CallError>, Arc<Client>) + Send + 'static,
    ) -> Result<Answer, session::Refusal> {
        let (answered, answer) = std::sync::mpsc::channel();
        let waited_for = Arc::new(Mutex::new(Some(answered)));
        let client = Arc::clone(&self.client);
        self.runtime.spawn({
            let waited_for = Arc::clone(&waited_for);
            async move {
                let said = client.call(Command::StartProgram(Box::new(start))).await;
                let waiting = lock(&waited_for).take();
                match waiting {
                    Some(answered) => {
                        let _ = answered.send(said);
                    }
                    None => {
                        let _ = tokio::task::spawn_blocking(move || late(said, client)).await;
                    }
                }
            }
        });
        let said = loop {
            match answer.recv_timeout(LOOKS_FOR_A_CANCEL_EVERY) {
                Ok(said) => break said,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(session::Refusal {
                        code: String::new(),
                        why: "the session host's link ended".to_owned(),
                    });
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) if was_cancelled() => {
                    // Taken before this gave up: its answer is on its way.
                    if lock(&waited_for).take().is_some() {
                        return Err(session::Refusal {
                            code: CANCELLED.to_owned(),
                            why: START_CANCELLED.to_owned(),
                        });
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
        };
        said.map_err(|why| {
            why.refusal().cloned().unwrap_or_else(|| session::Refusal {
                code: String::new(),
                why: why.to_string(),
            })
        })
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
        let (project, repo) = (project.to_owned(), repo.to_owned());
        let held = self.held_for_a_send(bundle)?;
        let given = held.given;
        self.write_on_a_stream(bundle, held, move |client, transfer| {
            Box::pin(async move {
                client
                    .take_code_held(&project, &repo, transfer, refs, checkout, given)
                    .await
            })
        })
    }

    /// **Has the host take a moving chat's work tree over** (1.44, #2048): `refs`, the branch
    /// `checkout` and its snapshot, from the bundle at `bundle`. Refused in purlis's words by a
    /// host before 1.44, which would not read it as a move.
    pub(crate) fn send_work(
        &self,
        project: &str,
        repo: &str,
        refs: Vec<session::CodeRef>,
        checkout: String,
        bundle: Option<&Path>,
    ) -> Result<Vec<session::CodeRef>, String> {
        self.moves_chats()?;
        let (project, repo) = (project.to_owned(), repo.to_owned());
        let held = self.held_for_a_send(bundle)?;
        let given = held.given;
        self.write_on_a_stream(bundle, held, move |client, transfer| {
            Box::pin(async move {
                client
                    .take_work_held(&project, &repo, transfer, refs, checkout, given)
                    .await
            })
        })
    }

    /// **Has the host keep a moving chat's conversation** (1.44, #2048), from `file`.
    pub(crate) fn send_conversation(
        &self,
        project: &str,
        carried: &purlis_host::Carried,
        file: &Path,
    ) -> Result<(), String> {
        self.moves_chats()?;
        let (project, carried) = (project.to_owned(), carried.clone());
        let held = SendHeld::fixed(self.most_bundle_bytes);
        self.write_on_a_stream(Some(file), held, move |client, transfer| {
            Box::pin(async move {
                client
                    .take_conversation(
                        &project,
                        transfer,
                        &carried.workspace,
                        &carried.profile,
                        &carried.conversation,
                        &carried.file,
                    )
                    .await
            })
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
        let (project, repo) = (project.to_owned(), repo.to_owned());
        self.read_from_a_stream(into, move |client, transfer| {
            Box::pin(async move { client.give_code(&project, &repo, transfer, have).await })
        })
        .map(|(heads, read)| (heads, read > 0))
    }

    /// [`Self::fetch_code`], with the work tree's uncommitted work as its branch's snapshot, as
    /// a chat moves off the host (1.44, #2048).
    pub(crate) fn fetch_work(
        &self,
        project: &str,
        repo: &str,
        have: Vec<String>,
        into: &Path,
    ) -> Result<(Vec<session::CodeRef>, bool), String> {
        self.moves_chats()?;
        let (project, repo) = (project.to_owned(), repo.to_owned());
        self.read_from_a_stream(into, move |client, transfer| {
            Box::pin(async move { client.give_work(&project, &repo, transfer, have).await })
        })
        .map(|(heads, read)| (heads, read > 0))
    }

    /// **Has the host write a moving chat's conversation into `into`** (1.44, #2048). Answers
    /// where its harness keeps it there.
    pub(crate) fn fetch_conversation(
        &self,
        project: &str,
        carried: &purlis_host::Carried,
        into: &Path,
    ) -> Result<String, String> {
        self.moves_chats()?;
        let (project, carried) = (project.to_owned(), carried.clone());
        self.read_from_a_stream(into, move |client, transfer| {
            Box::pin(async move {
                client
                    .give_conversation(
                        &project,
                        transfer,
                        &carried.workspace,
                        &carried.profile,
                        &carried.conversation,
                    )
                    .await
            })
        })
        .map(|(file, _)| file)
    }

    /// **The clones the host holds in `workspace`** of `project` (1.51, #3811), by their folder
    /// names, with the `origin` URL it names for each it can (1.54, #3985), its word alone:
    /// `None` from a host before 1.51, which cannot say, so a clone it made stays there.
    pub(crate) fn clones_of(
        &self,
        project: &str,
        workspace: &str,
    ) -> Result<Option<ClonesHeld>, String> {
        if !self.names_its_clones() {
            return Ok(None);
        }
        match self.call(Command::ClonesOf {
            project: project.to_owned(),
            workspace: workspace.to_owned(),
        })? {
            Answer::Clones { repos, origins } => Ok(Some((repos, origins))),
            _ => Err("the runner did not say which clones it holds".to_owned()),
        }
    }

    /// Whether the host names where each clone it holds came from (1.54, #3985), so a clone it
    /// made comes back with its remote.
    pub(crate) fn names_clone_origins(&self) -> bool {
        self.version() >= session::KNOWS_CLONE_ORIGINS
    }

    /// Whether the host names the clones it holds in a workspace (1.51, #3811), so a chat moving
    /// off it brings back a clone it made.
    pub(crate) fn names_its_clones(&self) -> bool {
        self.version() >= session::KNOWS_CLONES_OF
    }

    /// Refused in purlis's words where the host is before 1.44 and moves no chat.
    pub(crate) fn moves_chats(&self) -> Result<(), String> {
        if self.version() < session::MOVES_CHATS {
            return Err(format!(
                "the host this chat would move to or from is from a purlis before session \
                 protocol {}.{}, which moves no chat. Update it to this purlis's version.",
                session::MOVES_CHATS.major,
                session::MOVES_CHATS.minor
            ));
        }
        Ok(())
    }

    /// **What a send of the bundle at `bundle` is held to**, by the pool's limits as they stand
    /// (spec #4275), or the limit it is already over: measured here, never a number a runner
    /// said, so nothing has gone yet. A runner before 1.65 holds a send to its own bounds
    /// whatever it is told, so they are the highest it can be there.
    fn held_for_a_send(&self, bundle: Option<&Path>) -> Result<SendHeld, String> {
        use purlis_core::poollimits::LimitId::{CodeSendTime, CodeSize};
        use purlis_session_protocol::code::MOST_BUNDLE_BYTES;
        const MIB: u64 = 1 << 20;
        let limits = self.limits();
        let older = self.version() < session::KNOWS_CODE_HELD;
        let size_highest = older.then_some(MOST_BUNDLE_BYTES / MIB);
        let time_highest = older.then_some(purlis_host::TRANSFER_WITHIN.as_secs() / 60);
        let held_to =
            |value: u64, highest: Option<u64>| highest.map_or(value, |top| top.min(value));
        let mib = held_to(limits.in_force(CodeSize), size_highest);
        let minutes = held_to(limits.in_force(CodeSendTime), time_highest);
        let most = mib.saturating_mul(MIB);
        let size = bundle
            .and_then(|bundle| std::fs::metadata(bundle).ok())
            .map_or(0, |file| file.len());
        if size > most {
            let hit = CodeSize.limit().hit(mib, size.div_ceil(MIB), size_highest);
            return Err(self.limit_said(hit));
        }
        let within = Duration::from_secs(minutes.saturating_mul(60));
        Ok(SendHeld {
            most,
            within,
            given: (!older).then_some(session::CodeHeld {
                most_bytes: most,
                within_secs: within.as_secs(),
            }),
            late: Some(CodeSendTime.limit().waited(minutes, time_highest)),
        })
    }

    /// One transfer the client writes: `ask` names it, and `file`'s bytes, where there is one,
    /// go on the code stream the host opens for it, at most what `held` says. Run out of its
    /// time, by this machine's own clock, it says the limit it hit, whatever a runner said.
    fn write_on_a_stream<T: Send + 'static>(
        &self,
        file: Option<&Path>,
        held: SendHeld,
        ask: impl FnOnce(Arc<Client>, u64) -> Transferred<T>,
    ) -> Result<T, String> {
        let transfer = self.next_transfer();
        let stream = self.stream_for(transfer);
        let SendHeld {
            most, within, late, ..
        } = held;
        let asked = ask(Arc::clone(&self.client), transfer);
        let file = file.map(Path::to_path_buf);
        let graced = tokio::time::Instant::now() + within + ANSWER_WITHIN;
        // Past `graced`, so this send's own clock says it ran out, not this bound.
        let sent = self.transfer(transfer, within + ANSWER_WITHIN, async move {
            let writing = async move {
                let mut stream = stream.await.map_err(|_| no_code_stream())?;
                if let Some(file) = file {
                    let mut file = tokio::fs::File::open(&file)
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
            // This machine's own clock, started before the host's: `Ok(Err(_))` is this send's
            // wait ending it, and nothing a runner says is. An answer before its time says
            // itself (#4398). A send the host finishes within the same grace as any answer
            // still counts.
            let time_up = tokio::time::sleep(within);
            tokio::pin!(time_up);
            tokio::select! {
                biased;
                answered = &mut asked => {
                    let answer = answered.map_err(code_refused)?;
                    writing.await?;
                    Ok(Ok(answer))
                }
                written = &mut writing => {
                    written?;
                    match tokio::time::timeout_at(graced, &mut asked).await {
                        Ok(answered) => answered.map(Ok).map_err(code_refused),
                        Err(_) => Ok(Err(None)),
                    }
                }
                () = &mut time_up => {
                    // Its time is up, and only that is decided here. The stream stays open and
                    // keeps writing until the grace ends: dropped now, the host could read the
                    // reset as the end of what was sent. The host's own bound ends it first. A
                    // refusal by then may be that bound, so it is said beside the limit.
                    let last = async {
                        tokio::select! {
                            biased;
                            answered = &mut asked => answered,
                            _ = &mut writing => (&mut asked).await,
                        }
                    };
                    match tokio::time::timeout_at(graced, last).await {
                        Ok(Ok(answer)) => Ok(Ok(answer)),
                        Ok(Err(why)) => Ok(Err(Some(code_refused(why)))),
                        Err(_) => Ok(Err(None)),
                    }
                }
            }
        });
        let also = match sent? {
            Ok(answer) => return Ok(answer),
            Err(also) => also,
        };
        let said = late.map_or_else(
            || "the host did not finish moving the code in time".to_owned(),
            |hit| self.limit_said(hit),
        );
        Err(match also {
            Some(also) => format!("{said}. The host also said: {also}"),
            None => said,
        })
    }

    /// One transfer the host writes: `ask` names it, and the bytes the host writes on the code
    /// stream it opens for it go into `into`, at most the link's bound. Answers the host's
    /// answer and how many bytes came.
    fn read_from_a_stream<T: Send + 'static>(
        &self,
        into: &Path,
        ask: impl FnOnce(Arc<Client>, u64) -> Transferred<T>,
    ) -> Result<(T, u64), String> {
        let transfer = self.next_transfer();
        let stream = self.stream_for(transfer);
        let most = self.most_bundle_bytes;
        let asked = ask(Arc::clone(&self.client), transfer);
        let into = into.to_path_buf();
        self.transfer(transfer, purlis_host::TRANSFER_WITHIN, async move {
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
                    let answer = answered.map_err(code_refused)?;
                    Ok((answer, reading.await?))
                }
                read = &mut reading => {
                    let read = read?;
                    let answer = asked.await.map_err(code_refused)?;
                    Ok((answer, read))
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

    /// Run one transfer on the link's runtime and wait for it, at most `within`, the host's
    /// bound on it, and the time to hear its answer; the stream it waited for is let go of
    /// whatever the outcome.
    fn transfer<T: Send + 'static>(
        &self,
        transfer: u64,
        within: Duration,
        moving: impl std::future::Future<Output = Result<T, String>> + Send + 'static,
    ) -> Result<T, String> {
        let (moved, outcome) = std::sync::mpsc::channel();
        self.runtime.spawn(async move {
            let _ = moved.send(moving.await);
        });
        let outcome = outcome
            .recv_timeout(within + ANSWER_WITHIN)
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
    /// The window holds the view to the link's frames itself where `held` names them (#2050,
    /// #3297): a view of a browser on a runner.
    pub(crate) fn draw_frames(
        &self,
        view: u32,
        sink: FrameSink,
        origins: OriginSink,
        held: Option<purlis_session_protocol::browsing::LinkFrames>,
    ) {
        let sinks = FrameSinks {
            frames: sink,
            origins,
            held: held.map(HeldFrames::to),
            closed: None,
        };
        self.draw_into(view, Draws::Frames(Arc::new(Mutex::new(sinks))));
    }

    /// Draw view `view` of a browser's DevTools (#2028): its frames to `sink`, and why they
    /// closed, its last frame, to `closed`.
    pub(crate) fn draw_devtools_frames(&self, view: u32, sink: FrameSink, closed: ClosedSink) {
        let sinks = FrameSinks {
            frames: sink,
            origins: Box::new(|_| {}),
            held: None,
            closed: Some(closed),
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

    /// Tell `tell` each skill run the log of the runner this link is relayed to says started or
    /// ended, for `project`, held there in the runner's own checkout (SK-20b, #3084), from now on.
    pub(crate) fn hear_skill_runs(&self, project: &str, tell: crate::host::SkillRunsHeard) {
        lock(&self.shared.skill_runs).insert(project.to_owned(), tell);
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
/// ([`Relayed::take_reached`]), each reply going back on the link in the order it was made.
struct HookLines {
    /// The link they come over.
    link: u64,
    /// The runner the link reached, as its host said (1.48, #3417): `None` for this machine's
    /// host's link, and for a host before 1.48.
    reached: Option<String>,
    client: Arc<Client>,
    runtime: tokio::runtime::Handle,
}

impl HookLines {
    /// Hear line `id` that `project`'s hooks wrote, with what hears that project's lines.
    /// `pathless`: the host records it under the project as it holds it, and is sent no path
    /// (1.40, #3400).
    fn hear(
        &self,
        shared: &Shared,
        project: String,
        id: u64,
        line: serde_json::Value,
        pathless: bool,
    ) {
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
        let reply = self.reply(project, id, pathless);
        relayed.take_reached(self.link, self.reached.clone(), id, heard, reply);
    }

    /// What replies to line `id` of `project`: each reply sent at once, behind the one before,
    /// naming no project path where the host said it uses none (`pathless`, #3400).
    fn reply(&self, project: String, id: u64, pathless: bool) -> Reply {
        let client = Arc::clone(&self.client);
        let runtime = self.runtime.clone();
        Arc::new(move |replied: Replied| {
            let replied = if pathless {
                replied.without_path()
            } else {
                replied
            };
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

/// **Whether `replied` is no larger than the link carries** ([`within_a_frame`]): what a run's
/// values are measured by before they are handed on (#3406). What is measured may carry them
/// (`Replied::Handed`, #2047), so it is wiped once it is (#3420).
pub(crate) fn fits_a_reply(replied: &Replied) -> bool {
    serde_json::to_vec(replied)
        .is_ok_and(|said| purlis_core::secrets::Zeroizing::new(said).len() <= A_REPLY_IS_AT_MOST)
}

/// `replied`, as JSON no larger than the link carries: an answer too large is refused in a
/// sentence instead.
fn within_a_frame(replied: Replied) -> serde_json::Value {
    let replied = if !fits_a_reply(&replied) {
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
            use purlis_session_protocol::view::frames::{AIMED_JPEG, CLOSED, JPEG, ORIGIN};
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
                // Why a browser's DevTools closed, the last frame of their view (#2028).
                CLOSED => {
                    if let Some(why) = frame.closed()
                        && let Some(closed) = lock(sink).closed.as_mut()
                    {
                        closed(why);
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
                pathless,
            }) => lines.hear(&shared, project, heard, line, pathless),
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
            // A grade a terminal asked for (SK-61, #3241): the window runs it as it runs a press
            // of Grade, off the runtime, since it reads the skill's copy.
            Told::Pushed(Pushed::GradeAsked {
                question,
                project,
                project_path,
                ask,
            }) => {
                let client = Arc::clone(&client);
                let runtime = tokio::runtime::Handle::current();
                tokio::spawn(async move {
                    let steps = {
                        let client = Arc::clone(&client);
                        move |step: purlis_session_protocol::session::GradeStepTold| {
                            let client = Arc::clone(&client);
                            let grade = step.grade.clone();
                            // Told in the order the grade's thread goes through them. Nobody to
                            // tell, or a host that does not answer, stops the grade.
                            let told = runtime.block_on(async move {
                                tokio::time::timeout(
                                    Duration::from_secs(10),
                                    client.call(Command::GradeStep { question, step }),
                                )
                                .await
                            });
                            if !matches!(told, Ok(Ok(_))) {
                                crate::skillgrade::stop_for_a_terminal(&grade);
                            }
                        }
                    };
                    let answer = tokio::task::spawn_blocking(move || {
                        crate::skillgrade::asked_at_a_terminal(
                            &project,
                            &project_path,
                            ask,
                            Box::new(steps),
                        )
                    })
                    .await
                    .unwrap_or_else(|why| {
                        purlis_session_protocol::session::GradeAnswer::Refused {
                            why: format!("The purlis window could not answer ({why})."),
                        }
                    });
                    let _ = client.call(Command::AnswerGrade { question, answer }).await;
                });
            }
            Told::Pushed(Pushed::StopGrade { grade }) => {
                crate::skillgrade::stop_for_a_terminal(&grade);
            }
            // The asking link's, never a window's.
            Told::Pushed(Pushed::GradeStep { .. }) => {}
            Told::Pushed(Pushed::Sandbox { confinement, told }) => {
                let hears = lock(&shared.sandboxes).get(&confinement).cloned();
                if let Some(hears) = hears {
                    hears.told(told);
                }
            }
            // A line of the runner's own event log, or a gap in it (#3393).
            Told::Pushed(Pushed::Event(event)) => {
                let taken = lock(&shared.runner_log).heard(&event);
                // A runner chat's skill run, logged in this machine's plane (SK-20b, #3084):
                // once, as its line is first taken, and off the runtime, as it writes a file.
                if taken && let Some(run) = purlis_core::skilluse::relayed::Relayed::of(&event) {
                    let told: Vec<crate::host::SkillRunsHeard> =
                        lock(&shared.skill_runs).values().cloned().collect();
                    if !told.is_empty() {
                        tokio::task::spawn_blocking(move || {
                            for tell in told {
                                tell(None, &run);
                            }
                        });
                    }
                }
            }
            Told::Pushed(Pushed::Missed {
                device, resumes_at, ..
            }) => lock(&shared.runner_log).missed(&device, resumes_at),
            Told::Pushed(Pushed::LinkDown {
                key_changed, gone, ..
            }) => {
                // The runner let go of every line it handed on over the link that went: each
                // command of its that waits here has gone, its Ask with it (#3408).
                let projects: Vec<Arc<Relayed>> = lock(&shared.hearing).values().cloned().collect();
                for relayed in projects {
                    relayed.lost_its_host(lines.link);
                }
                lock(&shared.runner_log).link(true);
                reconnect(&shared, Reconnect::Started);
                if key_changed {
                    reconnect(&shared, Reconnect::KeyChanged);
                }
                if gone {
                    reconnect(&shared, Reconnect::Gone);
                }
            }
            Told::Pushed(Pushed::LinkBack {}) => {
                lock(&shared.runner_log).link(false);
                reconnect(&shared, Reconnect::Over);
                // A runner that said no end of its log as the link came up is asked again now,
                // and read from there (#3393).
                if !lock(&shared.runner_log).started() {
                    let (client, shared) = (Arc::clone(&client), Arc::clone(&shared));
                    tokio::spawn(async move {
                        if let Ok((device, seq)) = client.log_end().await {
                            lock(&shared.runner_log).starts_at(device, seq);
                            let since = lock(&shared.runner_log).since();
                            let _ = client.subscribe(since).await;
                        }
                    });
                }
            }
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
            // A reader that fell behind subscribes again: from now for what the host says, and
            // from its cursor for the runner's own log (#3393).
            Told::FellBehind { .. } => {
                // Whether the runner's link came back meanwhile is not known: nothing is left
                // dimmed on a word that may have been missed (#2496).
                lock(&shared.runner_log).link(false);
                reconnect(&shared, Reconnect::Over);
                let since = lock(&shared.runner_log).since();
                let _ = client.subscribe(since).await;
            }
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
    /// The one link these chats stay on, once their runner drains and the project's next chats
    /// are started by another of these, on the pool's runner now (#4248).
    stays_on: Arc<Mutex<Option<Arc<HostLink>>>>,
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
    ended: Arc<Mutex<Option<Ends>>>,
    kill_switch: Arc<crate::killswitch::KillSwitch>,
    /// The chats whose program ended with their host, until each is started again or closed
    /// (#2007), each with whether that host was a runner gone for good (#4350).
    lost: LostChats,
    /// Each program the host still had for this project as the window held it, from before
    /// this window, that nothing has reattached yet, by its number (#2006).
    left: Arc<Mutex<HashMap<u32, HeldProgram>>>,
    /// The prompt key the host answered each chat this window launched with (#3369), until the
    /// start asks for it ([`SessionHost::prompt_key_started`]): none from a host before 1.60,
    /// which set none in the chat's environment.
    keys_started: Mutex<HashMap<u32, Option<purlis_core::skillspan::PromptSalt>>>,
    /// Each browser the host still had for this project as the window first held it, from
    /// before this window, until the put back takes them (#2025): `None` until that first hold.
    browsers_left: Arc<Mutex<Option<Vec<HeldProgram>>>>,
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
    /// Whether a chat was asked after while a start waited for the runner, so that what the host
    /// has left running was not known then, and none of it is closed as not reattached (#4415).
    left_unread: std::sync::atomic::AtomicBool,
    /// Whether the runner holds the project where it keeps its projects, sent the project's code
    /// by the window (#2046): a pool's runner. A runner a test runs on this machine's disk holds
    /// it at the window's own path, and is sent nothing.
    kept_on_the_runner: bool,
    /// What the runner held of each repository after the window last sent it (#2046).
    sent: Arc<crate::runnercode::Sent>,
    /// Who logs the skill runs of the project's chats a runner keeps in its own checkout
    /// (SK-20b, #3084), told to each link the project is held on there.
    skill_runs: Arc<Mutex<Option<crate::host::SkillRunsHeard>>>,
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
    /// The runners of the pool it may be held on, by name, where this machine keeps any: the
    /// pool's, and each one draining, which a link already on it stays on.
    runners: Vec<String>,
}

impl HostedSessions {
    /// The chats of the project at `root`, run in the host `connection` reaches.
    pub fn new(connection: Arc<Connection>, root: &Path) -> Self {
        let (running, reconnecting) = (Arc::default(), Arc::default());
        let (lost, ended) = (LostChats::default(), Arc::default());
        connection.hears_of_gone_runners(&lost, &ended);
        HostedSessions {
            connection,
            root: root.to_path_buf(),
            stays_on: Arc::default(),
            held: Arc::default(),
            relayed: Arc::default(),
            reconnects: reconnects_of(&running, &reconnecting),
            running,
            opened: Arc::default(),
            ended,
            kill_switch: crate::killswitch::KillSwitch::unthrown(),
            lost,
            left: Arc::default(),
            keys_started: Mutex::default(),
            browsers_left: Arc::default(),
            untaken: Arc::default(),
            reconnecting,
            on_a_runner: false,
            left_unread: std::sync::atomic::AtomicBool::new(false),
            kept_on_the_runner: false,
            sent: Arc::default(),
            skill_runs: Arc::default(),
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
        if let Some(link) = lock(&self.stays_on).clone() {
            return Ok(link);
        }
        self.connection.of_project(&self.root, within)
    }

    /// The link, with this project held on it.
    fn link(&self) -> Result<(Arc<HostLink>, String), String> {
        held_on(&self.holding(), false)
    }

    /// [`Self::link`] for a chat's start, the one thing that wakes an idle runner: a project held
    /// on a link that is down is held again, and that hold waits for the runner (#4403, #4415).
    fn link_of_a_start(&self) -> Result<(Arc<HostLink>, String), String> {
        held_on(&self.holding(), true)
    }

    /// [`Self::moving_link`], for the tests of what moves over it.
    #[cfg(test)]
    pub(crate) fn moving_link_in_a_test(&self) -> Result<(Arc<HostLink>, String), String> {
        self.moving_link()
    }

    /// The link a chat moves over (#2048): this project held on a pool's runner that keeps it,
    /// of a purlis that moves chats.
    fn moving_link(&self) -> Result<(Arc<HostLink>, String), String> {
        if !self.kept_on_the_runner {
            return Err(crate::host::MOVES_NO_CHAT.to_owned());
        }
        let (link, project) = self.link()?;
        link.moves_chats()?;
        Ok((link, project))
    }

    /// What holds this project on its own link to the host.
    fn holding(&self) -> Holding {
        Holding {
            connection: Arc::clone(&self.connection),
            stays_on: Arc::clone(&self.stays_on),
            held: Arc::clone(&self.held),
            relayed: Arc::clone(&self.relayed),
            opened: Arc::clone(&self.opened),
            left: Arc::clone(&self.left),
            browsers_left: Arc::clone(&self.browsers_left),
            untaken: Arc::clone(&self.untaken),
            root: self.root.clone(),
            reconnects: Arc::clone(&self.reconnects),
            kept_on_the_runner: self.kept_on_the_runner,
            sent: Arc::clone(&self.sent),
            skill_runs: Arc::clone(&self.skill_runs),
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
        let deal = || {
            link.call_of_a_start(Command::Deal {
                project: project.clone(),
            })
        };
        match deal()? {
            Answer::Dealt { number } => {
                // A number went out here that the host had not heard of, as `deal` deals while a
                // hold waits: it is told, and deals past it.
                let went_out = self.opened.load(Ordering::Relaxed);
                if number <= went_out {
                    link.call_of_a_start(Command::DealtAlready {
                        project: project.clone(),
                        dealt: went_out,
                    })?;
                    return match deal()? {
                        Answer::Dealt { number } => {
                            self.opened.fetch_max(number, Ordering::Relaxed);
                            Ok(number)
                        }
                        other => Err(format!("the session host answered a deal with {other:?}")),
                    };
                }
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
            Reconnect::KeyChanged | Reconnect::Gone => {
                let chats: Vec<u32> = lock(&dimmed).chats.iter().copied().collect();
                let said = if heard == Reconnect::Gone {
                    Reconnection::RunnerGone
                } else {
                    Reconnection::KeyChanged
                };
                if let Some(tell) = lock(&dimmed).tell.clone() {
                    tell(chats, said);
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
///
/// `for_a_start` is whether a chat is starting. Only then is a project on a link that is down held
/// again. A person's or dispatch's start waits for a runner on its way back before it takes the
/// hold's lock, so the window's commands, which take that lock, do not wait on the runner (#4415).
fn held_on(holding: &Holding, for_a_start: bool) -> Result<(Arc<HostLink>, String), String> {
    let Holding {
        connection,
        stays_on,
        held,
        relayed,
        opened,
        left,
        browsers_left,
        untaken,
        root,
        reconnects,
        kept_on_the_runner,
        sent,
        skill_runs,
    } = holding;
    let stays = lock(stays_on).clone();
    let link = match stays {
        Some(link) => link,
        None if can_be_cancelled() => connection.of_project(root, UNTIL_THE_HOST_SAYS)?,
        None => connection.of_project(root, LINK_WITHIN)?,
    };
    // A runner this was held on is kept no more (Make the runner again, a drain's end): the
    // relay holds the project on the one after it, which answers yes and holds none of its
    // code, so no word comes back that it does not hold it. While its link is down a start holds
    // it again too: a hold is the step that wakes an idle runner and waits for it, and what
    // comes next on the link would only be refused (#4403).
    let stays_held = |on: &Held| {
        on.on == link.id
            && !(*kept_on_the_runner
                && ((for_a_start && lock(&link.shared.runner_log).link_is_down())
                    || a_runner_is_gone(connection, &on.runners)))
    };
    let waits = for_a_start && *kept_on_the_runner && waits_for_its_runner();
    // Decided once: a link that came back during the wait still needs what a hold does after it,
    // its code sent above all.
    let waited = waits && !lock(held).as_ref().is_some_and(stays_held);
    if waited {
        waits_off_the_lock(holding, &link)?;
    }
    let mut held = lock(held);
    if !waited
        && let Some(on) = &*held
        && stays_held(on)
    {
        return Ok((link, on.project.clone()));
    }
    // Another start waits for the runner: this would hold the lock until a quick command's limit.
    if !waits && link.shared.holds_waiting.load(Ordering::SeqCst) > 0 {
        return Err(THE_RUNNER_COMES_BACK.to_owned());
    }
    if held.as_ref().is_some_and(|on| on.on == link.id) {
        crate::runnercode::forget(sent);
    }
    // Read before the hold and the send, so a runner removed during them is seen as gone next.
    let runners = if *kept_on_the_runner {
        runners_of_the_link(connection, &link)
    } else {
        Vec::new()
    };
    let project = purlis_core::scaffold::planefile::id_to_hold_by(root)
        .map_err(|why| format!("this project has no id the session host can name it by: {why}"))?;
    // Before the hold, so the host's first line for the project is heard.
    if let Some(relayed) = lock(relayed).clone() {
        link.hear_project(&project, relayed);
    }
    // A runner that keeps the project in its own checkout logs its chats' skill runs there: the
    // window logs them here from its log (#3084). One on this machine's disk logs them here.
    if *kept_on_the_runner && let Some(tell) = lock(skill_runs).clone() {
        link.hear_skill_runs(&project, tell);
    }
    link.when_reconnecting(Arc::clone(reconnects));
    // A runner holds it where it keeps its projects, `<data>/projects/<project id>`, and is
    // sent the project's own code to check out there (#2046).
    link.call_of_a_start(Command::HoldProject {
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
        link.call_of_a_start(Command::DealtAlready {
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
        let within = if can_be_cancelled() {
            UNTIL_THE_HOST_SAYS
        } else {
            ANSWER_WITHIN
        };
        match link.call_refused_within(
            Command::ProgramsOf {
                project: project.clone(),
            },
            within,
        ) {
            Ok(Answer::Programs(programs)) => {
                // Its browsers, from a host of 1.39 or later (#2025), taken back with their
                // chats once the put back has settled: those of the first hold alone, so a
                // re-link never hands back a browser this window started since.
                let (mut browsers, mut chats): (Vec<HeldProgram>, Vec<HeldProgram>) = programs
                    .into_iter()
                    .partition(|program| program.kind == ProgramKind::Browser);
                // A chat taken back that runs without the sandbox on a pool's runner leaves it
                // untrusted (D-2050-27), and the browsers there end, not taken back
                // (D-2050-28): the host clears a browser's data folder as it ends.
                let taints = |chat: &HeldProgram| {
                    purlis_core::runnertaint::taints(chat.kind, chat.sandbox.as_ref())
                };
                if connection.pool().is_some()
                    && chats.iter().any(taints)
                    && untrust_taking_back(connection, &link, &mut browsers).is_err()
                {
                    // Where that cannot be kept, such a chat does not run on, as a start whose
                    // mark cannot be kept does not (#3702).
                    for chat in chats.extract_if(.., |chat| taints(chat)) {
                        link.send(Command::Close {
                            program: chat.program,
                        });
                    }
                }
                // On a pool's runner, what the first hold found alone: a hold again while a put
                // back goes on must not ask to close a chat it already took back (#4271).
                let first_hold = lock(browsers_left).is_none();
                if first_hold || connection.pool().is_none() {
                    *lock(left) = chats
                        .into_iter()
                        .map(|program| (program.number, program))
                        .collect();
                }
                lock(browsers_left).get_or_insert(browsers);
            }
            // Whether a chat runs there without the sandbox is not known: on a pool's runner it
            // counts as one that does (D-2050-27).
            Ok(other) => {
                tracing::warn!("purlis: the session host answered its programs with {other:?}");
                let _ = untrust_taking_back(connection, &link, &mut Vec::new());
            }
            Err(refused) => {
                tracing::warn!(
                    "purlis: the session host did not say its programs: {}",
                    refused.why
                );
                // A draining runner removed while it was asked took what it ran with it.
                if !connection.asked_a_runner_kept_no_more() {
                    let _ = untrust_taking_back(connection, &link, &mut Vec::new());
                }
            }
        }
    }
    *held = Some(Held {
        project: project.clone(),
        on: link.id,
        runners,
    });
    // Held: a mark that a runner would not hold it goes (#2846).
    crate::notheld::held(root);
    Ok((link, project))
}

/// The names of the runners `connection`'s pool has now: its own and each one draining. None
/// where this machine keeps none, or they cannot be read.
fn runners_of_the_pool(connection: &Connection) -> Vec<String> {
    let (Some(config), Some(pool)) = (connection.runners_config(), connection.pool()) else {
        return Vec::new();
    };
    let draining = purlis_core::runners::draining(&config).unwrap_or_default();
    purlis_core::runners::runner_for(&config, pool)
        .into_iter()
        .chain(draining.into_iter().filter(|kept| kept.pool() == pool))
        .map(|kept| kept.name().to_owned())
        .collect()
}

/// **The runners a project held on `link` depends on**: the one the link reached, as its host said
/// it, while this machine keeps it, read or not; else, for a runner made again in its place or a
/// host that names none, the pool's.
fn runners_of_the_link(connection: &Connection, link: &HostLink) -> Vec<String> {
    let reached = link.reached_runner().filter(|runner| {
        connection
            .runners_config()
            .is_some_and(|config| !matches!(purlis_core::runners::named(&config, runner), Ok(None)))
    });
    match reached {
        Some(runner) => vec![runner.to_owned()],
        None => runners_of_the_pool(connection),
    }
}

/// Whether this machine no longer keeps one of `runners`. A list that cannot be read says nothing
/// is gone: a resend of the whole project waits for an answer.
fn a_runner_is_gone(connection: &Connection, runners: &[String]) -> bool {
    let Some(config) = connection.runners_config() else {
        return false;
    };
    runners
        .iter()
        .any(|name| matches!(purlis_core::runners::named(&config, name), Ok(None)))
}

/// **A chat the window takes back on a pool's runner ran, or may have run, without the sandbox**
/// (D-2050-27, D-2050-28): the pool is untrusted, and each browser left there, `browsers`, is
/// ended over `link` rather than taken back. Nothing for a connection to this machine's host.
/// Answers why the pool's runners could not be kept so.
fn untrust_taking_back(
    connection: &Connection,
    link: &HostLink,
    browsers: &mut Vec<HeldProgram>,
) -> Result<(), String> {
    if connection.pool().is_none() {
        return Ok(());
    }
    let kept = connection.ran_unsandboxed().inspect_err(|why| {
        tracing::warn!("purlis: {why}");
    });
    for browser in browsers.drain(..) {
        link.send(Command::Close {
            program: browser.program,
        });
    }
    kept
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
        stays_on,
        held,
        relayed,
        opened,
        left,
        browsers_left,
        untaken,
        root,
        reconnects,
        kept_on_the_runner,
        sent,
        skill_runs,
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
            stays_on: Arc::clone(&stays_on),
            held: Arc::clone(&held),
            relayed: Arc::clone(&relayed),
            opened: Arc::clone(&opened),
            left: Arc::clone(&left),
            browsers_left: Arc::clone(&browsers_left),
            untaken: Arc::clone(&untaken),
            root: root.clone(),
            reconnects: Arc::clone(&reconnects),
            kept_on_the_runner,
            sent: Arc::clone(&sent),
            skill_runs: Arc::clone(&skill_runs),
        };
        if let Err(again) = held_on(&holding, false) {
            tracing::warn!("purlis: project {project} is not held where its chats run: {again}");
            // Marked in the Pools group until a hold is taken (#2846), in the runner's words:
            // only a refusal of the hold, never what purlis itself could not do.
            if let Some(words) = again.strip_prefix(NOT_HELD) {
                crate::notheld::not_held(&holding.root, words.trim_start());
            }
        }
    })
}

/// **How long a dispatch's first hold is waited for** (#4415): a cluster pool's `vouchWait`, which
/// is what the host waits, and a quick command's limit after it. A dispatch has no Cancel, and
/// nothing else would end this wait where a runner keeps its link and never answers.
fn a_dispatchs_hold_within(connection: &Connection) -> Duration {
    let vouching = connection.runners_config().and_then(|config| {
        let pool = connection.pool()?;
        purlis_core::runners::cluster_pools(&config)
            .ok()?
            .into_iter()
            .find(|one| one.name() == pool)
            .map(|one| one.vouch_wait())
    });
    vouching.unwrap_or_default() + ANSWER_WITHIN
}

/// **A start's first hold, off the hold's lock** (#4415): the host's answer waits while the runner
/// comes back, and nothing else waits on it. The person's Cancel ends a person's wait, and
/// [`a_dispatchs_hold_within`] a dispatch's. Holding again is harmless, so [`held_on`] goes on as
/// it does for any hold, and is answered at once.
fn waits_off_the_lock(holding: &Holding, link: &HostLink) -> Result<(), String> {
    struct Waited<'a>(&'a AtomicUsize);
    impl Drop for Waited<'_> {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }
    let project = purlis_core::scaffold::planefile::id_to_hold_by(&holding.root)
        .map_err(|why| format!("this project has no id the session host can name it by: {why}"))?;
    // Before the hold, as in `held_on`, so the host's first line for the project is heard.
    if let Some(relayed) = lock(&holding.relayed).clone() {
        link.hear_project(&project, relayed);
    }
    link.when_reconnecting(Arc::clone(&holding.reconnects));
    link.shared.holds_waiting.fetch_add(1, Ordering::SeqCst);
    let _waited = Waited(&link.shared.holds_waiting);
    let hold = Command::HoldProject {
        project,
        project_path: String::new(),
    };
    if can_be_cancelled() {
        link.call_until_answered(hold)
    } else {
        link.call_within(hold, a_dispatchs_hold_within(&holding.connection))
    }
    .map(|_| ())
    .map_err(|why| format!("{NOT_HELD} {why}"))
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
    stays_on: Arc<Mutex<Option<Arc<HostLink>>>>,
    held: Arc<Mutex<Option<Held>>>,
    relayed: Arc<Mutex<Option<Arc<Relayed>>>>,
    opened: Arc<AtomicU32>,
    left: Arc<Mutex<HashMap<u32, HeldProgram>>>,
    browsers_left: Arc<Mutex<Option<Vec<HeldProgram>>>>,
    /// Why the board the host kept could not be taken, where it could not (#2006).
    untaken: Arc<Mutex<Option<String>>>,
    root: PathBuf,
    /// Told what the project's link hears of the runner its host relays it to (#2496).
    reconnects: Reconnects,
    /// Whether the project is held on a runner where it keeps it, with its code sent (#2046).
    kept_on_the_runner: bool,
    /// What the runner held of each repository after the window last sent it.
    sent: Arc<crate::runnercode::Sent>,
    /// Who logs the skill runs of the project's chats there (#3084).
    skill_runs: Arc<Mutex<Option<crate::host::SkillRunsHeard>>>,
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
    let (link, project) = match held_on(holding, false) {
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
        prompt_key: opening
            .prompt_key
            .as_ref()
            .and_then(purlis_core::skillspan::PromptSalt::key),
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
        let kind = start.kind;
        // A start whose answer is lost may have started it: on a pool's runner, a chat of a kind
        // that runs without the sandbox counts as one that did (D-2050-27). A host that refused
        // it with a code that says so ([`NOTHING_STARTED`]) started nothing.
        let unclear = |why: String| {
            if purlis_core::runnertaint::taints(kind, None)
                && let Err(kept) = self.connection.ran_unsandboxed()
            {
                tracing::warn!("purlis: {kept}");
            }
            why
        };
        if was_cancelled() {
            return Err(START_CANCELLED.to_owned());
        }
        // What a chat that comes up after the person cancelled comes to: closed, and on a pool's
        // runner counted as it ran. One whose start is lost counts as one that may have run.
        let late = {
            let connection = Arc::clone(&self.connection);
            move |said: Result<Answer, session::CallError>, client: Arc<Client>| {
                let (ran, closes) = match said {
                    Ok(Answer::ProgramStarted {
                        program, sandbox, ..
                    }) => (
                        purlis_core::runnertaint::taints(kind, sandbox.as_ref()),
                        Some(program),
                    ),
                    Err(why)
                        if why
                            .refusal()
                            .is_some_and(|said| NOTHING_STARTED.contains(&said.code.as_str())) =>
                    {
                        (false, None)
                    }
                    _ => (purlis_core::runnertaint::taints(kind, None), None),
                };
                if ran && let Err(kept) = connection.ran_unsandboxed() {
                    tracing::warn!("purlis: {kept}");
                }
                if let Some(program) = closes {
                    tracing::info!("purlis: a chat came up after its start was cancelled: closed");
                    let handle = tokio::runtime::Handle::current();
                    if let Err(why) = handle.block_on(client.call(Command::Close { program })) {
                        tracing::warn!("purlis: it was not closed: {why}");
                    }
                }
            }
        };
        let started = match link.call_start(start, late) {
            // The host runs it under the number the window announced, or the window would key
            // it under one its hooks do not carry: it is closed, and the start fails.
            Ok(Answer::ProgramStarted {
                program,
                number,
                sandbox,
                ..
            }) if number != Some(id) => {
                link.send(Command::Close { program });
                // It ran, however briefly.
                if purlis_core::runnertaint::taints(kind, sandbox.as_ref())
                    && let Err(kept) = self.connection.ran_unsandboxed()
                {
                    tracing::warn!("purlis: {kept}");
                }
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
                prompt_key,
                ..
            }) => (program, process, confined, sandbox, prompt_key),
            Ok(other) => {
                return Err(unclear(format!(
                    "the session host answered a start with {other:?}"
                )));
            }
            // Only a refusal whose code says nothing started is taken at its word (#3702).
            Err(refused)
                if refused.code == CANCELLED
                    || NOTHING_STARTED.contains(&refused.code.as_str()) =>
            {
                return Err(refused.why);
            }
            Err(refused) => return Err(unclear(refused.why)),
        };
        let (program, pid, confined, ran, prompt_key) = started;
        // A chat that runs on a pool's runner with nothing holding its files leaves that runner
        // trusted with no browser and no login until it is made again (D-2050-27): kept here,
        // where no runner reaches, before it is kept as running. Where that cannot be kept, it
        // does not run.
        if let Some(pool) = self.connection.pool()
            && purlis_core::runnertaint::taints(kind, ran.as_ref())
            && let Err(why) = self.connection.ran_unsandboxed()
        {
            link.send(Command::Close { program });
            return Err(format!(
                "this chat runs without the sandbox on pool {pool}'s runner, and {why}, so it \
                 was closed"
            ));
        }
        self.runs(link, id, &program, pid);
        lock(&self.lost).remove(&id);
        // A stop that landed while this program was being started is carried out now.
        if !operator_shell && self.kill_switch.is_stopped() {
            link.send(Command::Stop { chat: program });
        }
        Ok(Begun {
            id,
            confined,
            ran,
            prompt_key: prompt_key
                .as_ref()
                .and_then(purlis_core::skillspan::PromptSalt::answered),
        })
    }
}

/// **The refusals of a start that say nothing started** (#3702): any other, or none, may have
/// started it, and on a pool's runner a chat that may have run without the sandbox counts as one
/// that did (D-2050-27).
const NOTHING_STARTED: &[&str] = &[
    session::UNKNOWN_COMMAND,
    session::MALFORMED,
    session::NO_SUCH_PROJECT,
    session::NOT_ALLOWED,
    session::NOT_STARTED,
    session::STOPPED,
    session::KEY_CHANGED,
];

/// What [`HostedSessions::started`] heard of a start: its number, the confinement the host named
/// to this link where it named one, and how the host applied the sandbox to its run (1.29).
struct Begun {
    id: u32,
    confined: Option<session::Confined>,
    ran: Option<session::RunSandbox>,
    /// The chat's prompt key, as the host said it (1.60, #3369).
    prompt_key: Option<purlis_core::skillspan::PromptSalt>,
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
        let (link, project) = self.link_of_a_start()?;
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
            // A host before 1.42 ignores it, and this window ends the chat as it comes back.
            ends_with_its_window: opening.ends_with_its_window,
            ..StartProgram::of(project, kind_of(opening))
        };
        self.started(&link, id, start, opening.operator_shell, announce)
            .map(|begun| {
                lock(&self.keys_started).insert(begun.id, begun.prompt_key);
                begun.id
            })
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
        let (link, project) = self.link_of_a_start()?;
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
            tools_allowed: named.tools_allowed.clone(),
            plane_skills: Some(named.plane_skills),
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
        let Begun {
            id,
            confined,
            ran,
            prompt_key,
        } = self
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
                prompt_key,
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
            prompt_key,
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

    /// What the link the project is held on holds of its runner's log (#3393): none where the
    /// project's chats run here, or no link to the runner is open.
    fn runner_log(&self) -> Option<crate::runnerlog::RunnerLogNow> {
        if !self.on_a_runner {
            return None;
        }
        self.own_link(Duration::ZERO)
            .ok()
            .map(|link| link.runner_log())
    }

    fn lost_with_its_host(&self, id: u32) -> bool {
        lock(&self.lost).contains_key(&id)
    }

    fn lost_with_its_runner(&self, id: u32) -> bool {
        lock(&self.lost)
            .get(&id)
            .is_some_and(|lost| lost.with_the_runner)
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
        // A hold another start waits on may take as long as its host does, and a deal runs under
        // the lock the window's own commands wait on: this number is dealt here then.
        if matches!(
            self.held.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        ) {
            return self
                .opened
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .map_or(u32::MAX, |was| was + 1);
        }
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
        if let (Some(Held { project, on, .. }), Ok(link)) = (held, self.own_link(Duration::ZERO))
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

    /// Told to each link the project is held on in a runner's own checkout from its next hold
    /// ([`held_on`]): a pool's runner is told this as it is made, before its first chat.
    fn when_skill_runs_heard(&self, tell: crate::host::SkillRunsHeard) {
        *lock(&self.skill_runs) = Some(Arc::clone(&tell));
        // And to the link it is held on now, where it is held already: no hold comes for it.
        if !self.kept_on_the_runner {
            return;
        }
        let held = lock(&self.held).clone();
        if let Some(Held { project, on, .. }) = held
            && let Ok(link) = self.connection.of_project(&self.root, Duration::ZERO)
            && link.id == on
        {
            link.hear_skill_runs(&project, tell);
        }
    }

    fn hold_here(&self) {
        if let Err(why) = self.link() {
            tracing::warn!("purlis: the session host does not hold the project yet: {why}");
        }
    }

    /// Fetched over this project's own link to the runner, into each clone of `workspace`
    /// (#2046). Nothing where the runner shares this machine's disk, or the project is not
    /// held on a pool's runner.
    /// Whether the runner these chats stayed on is kept no more: its link ends with it.
    fn runner_is_gone(&self) -> bool {
        let link = lock(&self.stays_on).clone();
        let runner = match &link {
            Some(link) => link.reached_runner().unwrap_or_default().to_owned(),
            // Taken back after a restart (#4271): its links end as it goes
            // ([`Connection::links_to_gone_runners_end`]).
            None => match self.connection.draining_runner() {
                Some(runner) => runner.to_owned(),
                None => return false,
            },
        };
        let gone = self.connection.runners_config().is_some_and(|config| {
            !runner.is_empty()
                && matches!(purlis_core::runners::named(&config, &runner), Ok(None))
                && purlis_core::runners::read(&config).is_ok_and(|kept| !kept.is_empty())
        });
        if let (true, Some(link)) = (gone, link) {
            self.connection.lets_go_of_left(&self.root, link.id());
            link.end();
        }
        gone
    }

    /// Asked as a chat starts: these chats stay where they run, and the project's link goes on to
    /// the pool's runner now.
    fn stays_on_a_draining_runner(&self) -> bool {
        if lock(&self.stays_on).is_some() {
            return true;
        }
        match self.connection.leaves_a_draining_runner(&self.root) {
            Some(link) => {
                *lock(&self.stays_on) = Some(link);
                true
            }
            None => false,
        }
    }

    /// While a runner of the pool drains, neither's fetch removes what only the other has
    /// (#4248).
    fn fetch_back(&self, workspace: &str, pool: &str) -> Vec<String> {
        if !self.kept_on_the_runner {
            return Vec::new();
        }
        match self.link() {
            Ok((link, project)) => {
                let beside = self.connection.links_of(&self.root).len() > 1
                    || lock(&self.stays_on).is_some()
                    || self.connection.pool_drains();
                crate::runnercode::fetch_workspace(
                    &link, &project, &self.root, workspace, pool, beside,
                )
            }
            Err(why) => vec![why],
        }
    }

    /// Only of a link already open: a tab drawn never opens one to the runner.
    fn clones_come_back(&self, _id: u32) -> bool {
        self.kept_on_the_runner
            && self
                .own_link(Duration::ZERO)
                .is_ok_and(|link| link.names_its_clones())
    }

    /// The project held on the pool's runner, and each clone of `workspace` sent ahead
    /// (#2048). Only where the runner keeps the project and moves chats.
    fn move_ahead(&self, _pool: &str, workspace: &str) -> Result<(), String> {
        let (link, project) = self.moving_link()?;
        crate::runnercode::send_ahead(&link, &project, &self.root, workspace, &self.sent)
    }

    fn move_in(&self, _pool: &str, chats: &[crate::host::Moving]) -> Result<(), String> {
        let Some(workspace) = chats.first().map(|chat| chat.workspace.as_str()) else {
            return Ok(());
        };
        let (link, project) = self.moving_link()?;
        // Each conversation is found first: where one is not, nothing has moved.
        let mut found = Vec::new();
        for chat in chats {
            let here = of_a_chat(&self.root, chat)
                .find()
                .map_err(|why| format!("its conversation did not move to the runner: {why}"))?;
            found.push((chat, here));
        }
        crate::runnercode::move_workspace_to(&link, &project, &self.root, workspace, &self.sent)?;
        for (chat, here) in found {
            link.send_conversation(&project, &carried(chat, &here.shape), &here.path)
                .map_err(|why| format!("its conversation did not move to the runner: {why}"))?;
        }
        Ok(())
    }

    fn move_out(
        &self,
        pool: &str,
        chats: &[crate::host::Moving],
    ) -> Result<crate::host::CameBack, String> {
        let Some(workspace) = chats.first().map(|chat| chat.workspace.as_str()) else {
            return Ok(crate::host::CameBack::default());
        };
        let (link, project) = self.moving_link()?;
        let scratch = tempfile::Builder::new()
            .prefix("purlis-conversation-")
            .tempdir()
            .map_err(|why| {
                format!("there is no room on this machine for its conversation: {why}")
            })?;
        // Every conversation first: where one does not come, nothing of this machine's clones
        // has changed, and the chats stay on the runner as they were.
        let mut came = Vec::new();
        for (at, chat) in chats.iter().enumerate() {
            let file = scratch.path().join(format!("conversation-{at}"));
            let shape = link
                .fetch_conversation(&project, &carried(chat, ""), &file)
                .map_err(|why| format!("its conversation did not come back: {why}"))?;
            came.push((chat, shape, file));
        }
        let came_back =
            crate::runnercode::move_workspace_back(&link, &project, &self.root, workspace, pool)?;
        for (chat, shape, file) in came {
            of_a_chat(&self.root, chat)
                .keep(&shape, &file)
                .map_err(|why| format!("its conversation could not be kept here: {why}"))?;
        }
        Ok(came_back)
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
        let answered = link.call_heard_in_a_start(Command::Confine(Box::new(session::Confine {
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
        match self.link() {
            Ok(_) => lock(&self.left).contains_key(&number),
            Err(why) => {
                // Not known yet, rather than not running: nothing is closed for it (#4415).
                if why == THE_RUNNER_COMES_BACK {
                    self.left_unread.store(true, Ordering::SeqCst);
                }
                false
            }
        }
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

    fn prompt_key_left(&self, number: u32) -> Option<purlis_core::skillspan::PromptSalt> {
        self.link().ok()?;
        lock(&self.left)
            .get(&number)?
            .prompt_key
            .as_ref()
            .and_then(purlis_core::skillspan::PromptSalt::answered)
    }

    fn prompt_key_started(
        &self,
        number: u32,
        minted: Option<purlis_core::skillspan::PromptSalt>,
    ) -> Option<purlis_core::skillspan::PromptSalt> {
        lock(&self.keys_started).remove(&number).unwrap_or(minted)
    }

    /// How the host says it applied the sandbox to chat `number`'s run: none from a host before
    /// 1.28 (#2636).
    fn ran_in(&self, number: u32) -> Option<purlis_session_protocol::session::RunSandbox> {
        self.link().ok()?;
        lock(&self.left).get(&number)?.sandbox.clone()
    }

    fn browsers_left(&self) -> Vec<HeldProgram> {
        if self.link().is_err() {
            return Vec::new();
        }
        lock(&self.browsers_left)
            .replace(Vec::new())
            .unwrap_or_default()
    }

    fn close_what_was_not_reattached(&self) {
        if self.link().is_err() || self.left_unread.load(Ordering::SeqCst) {
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
        let runner_gone = Arc::clone(&link.runner_gone);
        let on = link.reached_runner().map(str::to_owned);
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
                        purlis_core::session::Exit::Signal(a_hosts_signal(signal))
                    }
                    // Its terminal closed with its host, which hangs its program up.
                    Ending::WithTheHost => {
                        lock(&lost).insert(
                            id,
                            LostChat {
                                with_the_runner: runner_gone.load(Ordering::SeqCst),
                                on: on.clone(),
                            },
                        );
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
