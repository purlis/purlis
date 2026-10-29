// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The app's side of the hook channel: what a chat is doing, and which one needs you.
//!
//! A hook writes one line on a socket this owns (`purlis_core::hookwire`), which moves a
//! chat on the board and, if a reader would see a difference, tells the window. There is no
//! polling anywhere: the listener blocks on `accept`, and the window is pushed to.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use purlis_core::eventlog::Entry;
use purlis_core::hookwire::{
    Answer, Ask, ChatTokens, Hearing, Listener, NOTHING_ANSWERS, Reading, Report, SessionSaved,
    StartedByHand,
};
use purlis_core::session::Exit;
use purlis_core::state::{Board, State};

use purlis_core::harness::asks::Asks;
use purlis_core::harness::hooked::HookAsks;

use crate::host::{ChatBoard, Glance};
use crate::planes::{PlaneId, Teller};

/// The event the window listens for. One chat, its state, whether it is asking for you.
///
/// **The plane travels with it, and that is not decoration.** Every plane numbers its chats
/// from one, so a window holding two of them would be told "session 3 is waiting" twice about
/// two different chats. The pair is the identity; one half of it is a guess.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct Moved {
    pub plane: PlaneId,
    pub session: u32,
    pub state: String,
    pub needs_you: bool,
    /// Every chat asking for you, so the queue is never assembled from a series of events
    /// the window might have missed one of.
    pub queue: Vec<u32>,
    /// When this chat last moved, as a count of moves on every plane's board in this process
    /// — bigger is more recent, within a plane and across planes.
    /// `purlis_core::state::Board::moved_at` is the whole definition.
    ///
    /// **The window cannot work this out for itself, which is why it rides an event that
    /// already fires.** Charter ADR 0039 sorts the chat strip's overflow menu by last
    /// activity, and nothing in the window knows when a chat did anything: the strip is
    /// `tabs.order`, which is an opening order, and the needs-you queue is oldest-first,
    /// which is a different fact. An order computed in the window would also restart at
    /// every launch and disagree between two windows on one plane, where this one is the
    /// board's and the board is the plane's.
    ///
    /// A count rather than a clock, and a `u32` rather than a `u64`: both are argued where
    /// the field is produced, and the second is not negotiable here — `specta` refuses to
    /// export a `u64` and the app panics at startup in a debug build when one is reached
    /// for.
    pub moved_at: u32,
    /// The chats that have reported back to this one and not been read yet, by the name the
    /// operator sees them under, oldest first (charter-app#259). No needs-you item of its own
    /// (#1448): a chat in the queue for another reason says `<child> reported back` there.
    /// Empty for nearly every chat, and emptied by this chat's next prompt, which is the turn
    /// the reports are handed.
    pub reports: Vec<String>,
    /// The commits of this chat charter's `pre-commit` refused and the operator has not seen,
    /// each as the one masked line its item says, oldest first (SQ-16). Emptied by the chat's
    /// next prompt, or by Ignore.
    pub refusals: Vec<String>,
    /// The chats this one started that the operator stopped, by name, oldest first (#1448):
    /// its row says `<child> was stopped`. purlis's own word, apart from `reports`, which are
    /// what chats said. `null` for nearly every chat, and emptied as `reports` is.
    #[specta(optional)]
    pub stopped: Option<Vec<String>>,
    /// Why this chat needs the person when no hook of its own said so, oldest first (#1448):
    /// its needs-you item says the latest. `null` for nearly every chat, which needs the person
    /// for nothing of the kind, and emptied by the chat's next prompt, or by Ignore.
    #[specta(optional)]
    pub needs: Option<Vec<Need>>,
    /// Whether this chat is stopped, now, on a prompt its harness put to the person in the
    /// middle of a turn (#1601, `purlis_core::state::Board::waits_on_its_prompt`): a
    /// permission or a question, never the nudge of a chat whose turn is over, and never a
    /// task that failed. It stands past purlis's own hold of the prompt, until the prompt is
    /// answered, the chat gets past it, its turn ends or it ends. `true` or `null`, which is
    /// every other chat.
    #[specta(optional)]
    pub asking: Option<bool>,
    /// Which snapshot of the board this is — bigger was taken later (charter-app#248).
    ///
    /// **What lets the window put its events back in order.** Every `Moved` is built under the
    /// board's lock, but it is SENT after the lock is let go, on whichever thread built it: a
    /// hook's report on the socket's thread, a close on the command's. So a report taken just
    /// before a close can reach the window just after it, and the window, which keeps the last
    /// queue it was told, would put the closed chat back. The window drops any snapshot older
    /// than the one it holds (`chatState.ts`), which it can do only because this is numbered
    /// in the order the board was read. [`sequence`] is the whole definition.
    pub sequence: u32,
    /// The child agents of this chat's current run, oldest first (FD-18, W8): each sub-agent
    /// or child its harness spawned, drawn under the chat. Empty for nearly every chat.
    pub children: Vec<ChildAgent>,
    /// Whether this move is only what a chat it started did: a report that landed, or a stop
    /// below it (#1491). The window is told as of any move; nothing interrupts the person for
    /// it ([`Moved::interrupts`]). The app's own fact, and never sent.
    #[serde(skip)]
    #[specta(skip)]
    pub counts_only: bool,
    /// Whether this move raised something for the person (#1491): the chat came into the
    /// queue, or a reason was added to one already in it (`purlis_core::state::raised`). A
    /// chat that moves while its item stands raised nothing. The app's own fact, and never
    /// sent.
    #[serde(skip)]
    #[specta(skip)]
    pub raised: bool,
}

impl Moved {
    /// **Whether this move is one to interrupt the person for** (#1491, V100-15): the chat it
    /// is about is a needs-you item, and the move is its own. A task that finished changes
    /// what its asking chat's row counts, and is told as a move of that chat; it is no reason
    /// to send a system notification, whether or not that chat is already an item for
    /// something else. A task that failed is an item on the asking chat, raised as a move of
    /// its own, and that one does interrupt.
    ///
    /// **On the rising edge only**: a failure's item stands across the asking chat's own
    /// turns, and each prompt, stop and helper of that chat is a move with the item still
    /// standing. One failure is one notification.
    ///
    /// Whether the person is already looking at the chat is asked after this, and is not this
    /// rule's.
    pub fn interrupts(&self) -> bool {
        self.needs_you && self.raised && !self.counts_only
    }

    /// This move, as one that only changes what the chat's row counts.
    #[must_use]
    pub fn counting_only(mut self) -> Self {
        self.counts_only = true;
        self
    }
}

/// Why a chat needs the person, as the window says it (`purlis_core::state::Need`, #1448). A
/// dispatch grant that is needed is the next kind (#1437).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Need {
    /// Its report has nowhere to go: the chat that asked for it, `asker`, has gone.
    ReportUndelivered { asker: String },
    /// purlis refused it a git command that would rewrite the project's root from another
    /// commit, so bringing the remote's commits in is the person's, with Update (qw348).
    RootUpdateRefused,
    /// A task it asked for, `task`, came to nothing (#1491), and `why` says why in a few
    /// words, where anything does. `id` names this failure and no other: its dispatch
    /// record's id, which its finished row carries. `chat` is the task's chat while it is
    /// still open. Not emptied by the chat's next prompt: by the person's look at it, their
    /// Ignore, or the task's row being cleared. Held in memory only: a restart of the app
    /// keeps the task's finished row and not this item.
    TaskFailed {
        id: String,
        #[specta(optional)]
        chat: Option<u32>,
        task: String,
        how: HowFailed,
        why: String,
    },
}

/// How a task came to nothing (`purlis_core::state::HowFailed`), as the window is sent it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum HowFailed {
    /// It reported that it failed, or that it was blocked.
    Failed,
    /// Its program ended while it still owed its report.
    Unreported,
    /// It was asked for and never started.
    DidNotStart,
    /// Its turns ended without a report, and purlis sent its last message in its place
    /// (#1981).
    LastMessage,
}

impl From<purlis_core::state::FailedTask> for Need {
    fn from(failed: purlis_core::state::FailedTask) -> Self {
        use purlis_core::state::HowFailed as How;
        Self::TaskFailed {
            id: failed.id,
            chat: failed.chat,
            task: failed.task,
            how: match failed.how {
                How::Failed => HowFailed::Failed,
                How::Unreported => HowFailed::Unreported,
                How::DidNotStart => HowFailed::DidNotStart,
                How::LastMessage => HowFailed::LastMessage,
            },
            why: failed.why,
        }
    }
}

impl From<purlis_core::state::Need> for Need {
    fn from(need: purlis_core::state::Need) -> Self {
        match need {
            purlis_core::state::Need::ReportUndelivered { asker } => {
                Self::ReportUndelivered { asker }
            }
            purlis_core::state::Need::RootUpdateRefused => Self::RootUpdateRefused,
        }
    }
}

/// One child agent of a chat, as the window draws it under the chat (ADR 0066, ADR 0076 §6).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChildAgent {
    /// The harness's id for it, unique within the chat.
    pub agent: String,
    /// `running` while it works, `done` once its own stop is heard, or the word its chat
    /// ended as when the chat ended first: a stop of the chat stops its children.
    pub state: String,
}

/// The event the window is sent when a harness was started by hand in a shell tab (ADR 0062).
pub const BY_HAND: &str = "harness-by-hand";

/// A harness the operator started by hand in a shell tab, as the window draws its banner.
///
/// **Nothing about the chat moves.** It is not a state and not a needs-you item: the tab says
/// what happened and offers to open that harness as a chat, and the operator's click is what
/// does anything.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ByHand {
    pub plane: PlaneId,
    /// The shell tab's chat.
    pub session: u32,
    /// The harness, by the word the plane calls it — a profile's `kind`.
    pub harness: String,
    /// Where the shell was standing when it started it, which is where a chat opened in its
    /// place starts.
    pub cwd: Option<String>,
}

/// Told when a harness is started by hand in a shell tab of any plane. The event carries its
/// plane, as [`Moved`] does.
pub type ByHandTeller = Arc<dyn Fn(ByHand) + Send + Sync + 'static>;

/// What the window is told about `notice`, heard on `plane`'s socket — or nothing, for a
/// harness this app does not start. A word charter has no chat for would put a button on the
/// tab that could only be refused.
fn by_hand(plane: &PlaneId, notice: StartedByHand) -> Option<ByHand> {
    let harness = purlis_core::harness::Harness::of_kind(&notice.started_by_hand)?;
    Some(ByHand {
        plane: plane.clone(),
        session: notice.chat,
        harness: harness.name().to_owned(),
        cwd: notice.cwd.map(|cwd| cwd.display().to_string()),
    })
}

/// The board, the socket, and the thread reading it — one plane's whole side of the channel.
pub struct Hooks {
    /// The plane this listens for, stamped onto every event it sends.
    plane: PlaneId,
    /// Shared with the thread reading the socket: one board, so what the window is told and
    /// what the window can ask for can never disagree.
    board: Arc<Mutex<Board>>,
    /// The reading, while it is going on. Held in a lock rather than owned outright so that
    /// closing a plane can release the socket THEN, rather than whenever the last handle to
    /// the plane happens to be dropped — a project the operator closed has to stop listening
    /// while they are still looking at the app.
    reading: Mutex<Option<Reading>>,
    socket: Option<PathBuf>,
    /// The chats' tokens the socket checks every line against, issued as each chat starts
    /// (`crate::sessions::Reporting`).
    tokens: Option<Arc<ChatTokens>>,
    /// Who answers an ask on this socket, once there is someone to (charter-app#204).
    ///
    /// A slot filled after the fact, because the answer needs the plane's chats and the
    /// chats are built after the socket: a session's environment carries the socket's path,
    /// so the socket has to exist first. Until it is filled every ask is answered with a
    /// refusal, never with a silence an asker would have to wait out.
    answering: Arc<Mutex<Option<Answering>>>,
    /// Told each report the board took, after the window has been told what it moved — a
    /// slot filled after the fact, for `answering`'s reason: what listens needs the plane's
    /// chats, which are built after the socket. A curation chat's typed prompt waits on it
    /// (`crate::curation::Typed`).
    heard: Arc<Mutex<Option<Heard>>>,
    /// Told each time the board moves a chat onto another conversation — a slot filled after
    /// the fact, for `answering`'s reason. The chats' record listens, so the conversation a
    /// relaunch resumes is the one the chat is in now (Q10).
    following: Arc<Mutex<Option<Following>>>,
    /// The host's event log, once the app has one (FD-9) — a slot filled after the fact, so
    /// a project is opened the same way with or without it. Every line the channel hears from
    /// a chat's hooks is recorded there, whether or not it moved the board.
    events: Arc<Mutex<Option<Events>>>,
    /// Told EVERY report on this socket once the board has had it, whether it moved the board or
    /// not — a slot filled after the fact, for `answering`'s reason. A smart close queued for a
    /// turn's end waits on it (`crate::smartclose`): the `Stop` that ends a turn in which the
    /// chat asked a question moves nothing a reader sees, and is still the end of the turn.
    all_reports: Arc<Mutex<Option<Heard>>>,
    /// Asked what a chat waits on as its turn's end is applied (#1491): the tasks below it and
    /// the chat that dispatched it, which the project's records know and the board does not.
    /// A slot filled after the fact, for `answering`'s reason; until it is, a chat waits on
    /// nothing and every end of turn is the person's.
    waits: Arc<Mutex<Option<WaitsOf>>>,
    /// Told each session record a chat's `charter session record` says it saved (ADR 0064) — a
    /// slot filled after the fact, for `answering`'s reason.
    saved: Arc<Mutex<Option<SavedHeard>>>,
    /// The permission asks this project's chats hold open on their hooks (HP-6, `asking`).
    asks: Arc<HookAsks>,
    /// Told this project's asks each time they change — a slot filled after the fact, for
    /// `answering`'s reason.
    asks_told: crate::asking::Telling,
    /// Told each path a chat's file tool touched (FM-6), as the chat said it — a slot filled
    /// after the fact, for `answering`'s reason: confining it needs the chat's folder.
    /// **Never recorded**: nothing here writes it, and the event log is not told (D-86a).
    touching: Arc<Mutex<Option<Touches>>>,
    /// Told what a chat's turn put out (#1981), once the board has judged the line its own
    /// run's: a slot filled after the fact, as `touching` is. **In memory only.**
    output: Arc<Mutex<Option<Outputs>>>,
    /// Told what a chat's own tool hook says it is doing (#1981), once the board has judged the
    /// line its own run's. **In memory only.**
    doing_heard: Arc<Mutex<Option<Doings>>>,
    /// What each working chat is doing, for the one line under its name (#1493). **In memory
    /// only**, as a touched path is: nothing here writes it, and the event log is not told.
    doings: Arc<crate::doing::Doings>,
    /// The skill run open in each chat, for the chip on its tab (SK-3). **In memory only**: the
    /// event log records the run itself.
    skill_runs: Arc<crate::skillrun::SkillRuns>,
    /// Told each sandbox block a chat's hook found (#1338), once it is kept for `purlis doctor`
    /// — a slot filled after the fact, for `answering`'s reason.
    blocked: Arc<Mutex<Option<HeardBlocks>>>,
    /// Told the repeats of each block the throttle held back, once their minute is over
    /// (#1681), for the network record alone.
    repeated: Arc<Mutex<Option<Repeats>>>,
    /// The throttle every sandbox block is let through, shared with [`Self::hear_block`]: its
    /// held-back repeats are told when a chat goes quiet or ends, and when the project is let
    /// go of, not only at the next block heard.
    throttle: Arc<Mutex<purlis_core::sandboxblock::Throttle>>,
    /// The road every sandbox block takes into the app (#1338): the throttle, then
    /// [`Self::blocked`], which keeps it in the network record and shows it (#1662). Handed
    /// to what else hears a chat refused (#1663): purlis's own proxy beside a chat it wraps.
    /// None with nothing listening.
    hear_block: Option<Blocks>,
    /// Runs each brokered `secret exec` a sandboxed chat asks for (#1407) — a slot filled after
    /// the fact, for `answering`'s reason: it needs the chat's record. Empty, the asker is told
    /// nothing answers and runs the command itself.
    secret_exec: Arc<Mutex<Option<SecretExecs>>>,
    /// Where the session host owns this project's hook channel (#2000): what the window hears
    /// the lines it hands on with. None where the window listens itself, or not at all.
    relayed: Option<Arc<Relayed>>,
}

/// **A project's hook channel, owned by the session host and heard in the window** (#2000).
///
/// With chats in the session host, the host binds the project's hook socket, issues each chat
/// its token and admits each line, and hands every line it admits on to the window. The window
/// hears it here with the same hearing it listened with itself ([`Hooks::listening_on`]), so the
/// board, Needs you and the asks move exactly as before.
///
/// **The board is kept in the host** (#2194, `purlis_core::state::kept`). As the window holds
/// the project it takes the host's board whole, and from then on tells the host each move it
/// makes on its own ([`Relayed::keep_with`]). Until it has taken it, the lines the host hands
/// on wait here, so none is heard against a board the host's is about to replace.
pub struct Relayed {
    heeding: Arc<purlis_core::hookwire::relay::Heeding>,
    /// Where the lines heard at once go, to be heard one after another in the order they
    /// came, as the project's listener heard them when the window bound it (FD-9): one
    /// project's lines never wait on another's.
    in_order: Mutex<Option<std::sync::mpsc::Sender<InOrder>>>,
    /// The socket the host listens on for this project: a sandbox charter wraps a chat in lets
    /// the chat reach it.
    socket: PathBuf,
    /// Each running chat's program, as the host started it: what a line's chat is read from
    /// there, and what a caller that waited compares (qw381).
    programs: Mutex<std::collections::HashMap<u32, purlis_core::hookwire::Program>>,
    /// The project's board in the window, which the host keeps too.
    board: Arc<Mutex<Board>>,
    plane: PlaneId,
    /// Told each chat of the host's board as the window takes it.
    moved: Teller,
    /// The lines handed on before the window took the host's board, in the order they came:
    /// `None` once it has.
    waiting: Mutex<Option<Vec<InOrder>>>,
    /// The pool whose runner's lines these are, or `None` for this machine's host (#2046).
    pool: Option<String>,
    /// Which pool each chat runs on, where the project's chats are routed by workspace: a line
    /// is heard only from the host its chat runs on ([`Relayed::for_a_pool`]).
    routes: Mutex<Option<Routes>>,
}

std::thread_local! {
    /// The pool whose runner's link the ask heard on this thread came over, while it is heard:
    /// `None` for this machine's host's (#2046).
    static HEARD_FROM: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

std::thread_local! {
    /// The runner the link the ask heard on this thread came over reached, as the desktop's
    /// host said it (1.48, #3417): `None` for this machine's host's, and for a host before 1.48.
    static REACHED: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Runs `hear` with the ask it hears known to have come over `pool`'s link, or this machine's.
#[cfg(test)]
pub(crate) fn heard_from<T>(pool: Option<String>, hear: impl FnOnce() -> T) -> T {
    heard_reaching(pool, None, hear)
}

/// Runs `hear` with the ask it hears known to have come over `pool`'s link, which its host said
/// reached the runner `reached` (#3417); `None` for this machine's host's, and a host before 1.48.
pub(crate) fn heard_reaching<T>(
    pool: Option<String>,
    reached: Option<String>,
    hear: impl FnOnce() -> T,
) -> T {
    HEARD_FROM.with(|from| *from.borrow_mut() = pool);
    REACHED.with(|runner| *runner.borrow_mut() = reached);
    let heard = hear();
    HEARD_FROM.with(|from| *from.borrow_mut() = None);
    REACHED.with(|runner| *runner.borrow_mut() = None);
    heard
}

/// **The runner the link of the ask being heard reached** (#3417), as `<pool>/<runner>`, where
/// the desktop's host said it: a draining runner's, for a chat that stayed on it after its pool
/// moved on. What a runner chat is judged by (`poollogins::Bound::over_the_link`).
pub fn reached_over_the_link() -> Option<String> {
    REACHED.with(|runner| runner.borrow().clone())
}

/// **The pool whose runner's link the ask being heard came over** (#2046), or `None` where it
/// came over this machine's host's: what decides whether a brokered write is held for the
/// person, by the link and not by the chat the line names.
pub fn heard_from_a_pool() -> Option<String> {
    HEARD_FROM.with(|from| from.borrow().clone())
}

/// Which pool chat `number` runs on, or `None` for this machine (#2046).
pub type Routes = Arc<dyn Fn(u32) -> Option<String> + Send + Sync + 'static>;

/// What a line from a host its chat does not run on is refused with (#2046).
const NOT_ITS_HOST: &str =
    "this line came from a host its chat does not run on, so it was not heard";

/// A line to hear in order: the link it came over, its number, the line, and what replies to
/// it.
type InOrder = (
    u64,
    u64,
    purlis_core::hookwire::relay::Heard,
    purlis_core::hookwire::relay::Reply,
);

impl Relayed {
    /// Takes line `id` the host handed on over link `link`, replying through `reply`: a line
    /// that may wait as long as a person takes (an ask, a permission ask, a `secret exec`) is
    /// heard on a thread of its own, and every other one after the lines that came before it,
    /// on the project's own thread ([`purlis_core::hookwire::relay::Heeding::hear`]). Returns
    /// at once.
    #[cfg(test)]
    pub fn take(
        &self,
        link: u64,
        id: u64,
        heard: purlis_core::hookwire::relay::Heard,
        reply: purlis_core::hookwire::relay::Reply,
    ) {
        self.take_reached(link, None, id, heard, reply);
    }

    /// **Takes line `id` as `take` does, for a link its host said reached the runner `reached`**
    /// (1.48, #3417): what each ask heard on it is judged by. `None` for this machine's host's
    /// link, and for a host before 1.48.
    pub fn take_reached(
        &self,
        link: u64,
        reached: Option<String>,
        id: u64,
        heard: purlis_core::hookwire::relay::Heard,
        reply: purlis_core::hookwire::relay::Reply,
    ) {
        // Each host speaks only for the chats that run on it (#2046): a pool's runner never for
        // a chat of this machine's or another pool's, nor this machine's host for a runner's.
        if !self.its_own(&heard) {
            tracing::warn!(
                "purlis: a hook line for chat {} came from a host it does not run on; refused",
                heard.chat().unwrap_or_default()
            );
            heard.refuse(&reply, NOT_ITS_HOST);
            return;
        }
        if let Some(waiting) = self
            .waiting
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            waiting.push((link, id, heard, reply));
            return;
        }
        self.hand_on(link, reached, id, heard, reply);
    }

    /// Hears line `id` as [`Self::take_reached`] does, once no line waits for the host's board.
    fn hand_on(
        &self,
        link: u64,
        reached: Option<String>,
        id: u64,
        heard: purlis_core::hookwire::relay::Heard,
        reply: purlis_core::hookwire::relay::Reply,
    ) {
        use purlis_core::hookwire::relay::Heeding;
        // Taken in order, before a thread of its own hears it, so a secret exec's input that
        // comes before it begins is kept for it (#2571). One not taken is answered already.
        if !self.heeding.admit(link, id, &heard, &reply) {
            return;
        }
        if Heeding::waits(&heard) {
            let heeding = Arc::clone(&self.heeding);
            let exec = matches!(heard, purlis_core::hookwire::relay::Heard::SecretExec(_));
            let replies = Arc::clone(&reply);
            let pool = self.pool.clone();
            let spawned = std::thread::Builder::new()
                .name("purlis-hook-ask".into())
                .spawn(move || {
                    heard_reaching(pool, reached, || heeding.hear(link, id, heard, reply))
                });
            if let Err(why) = spawned {
                tracing::warn!("purlis: a chat's ask could not be heard ({why})");
                // A secret exec that will not run keeps no input, and its host is told so
                // (#2271).
                if exec {
                    self.heeding.not_begun(link, id, &replies);
                }
            }
            return;
        }
        let mut in_order = self.in_order.lock().unwrap_or_else(PoisonError::into_inner);
        if in_order.is_none() {
            let (lines, heard_in_order) = std::sync::mpsc::channel::<InOrder>();
            let heeding = Arc::clone(&self.heeding);
            let spawned = std::thread::Builder::new()
                .name("purlis-hook-lines".into())
                .spawn(move || {
                    for (link, id, heard, reply) in heard_in_order {
                        heeding.hear(link, id, heard, reply);
                    }
                });
            match spawned {
                Ok(_) => *in_order = Some(lines),
                Err(why) => {
                    tracing::warn!("purlis: a chat's hook line could not be heard ({why})");
                    return;
                }
            }
        }
        if let Some(lines) = in_order.as_ref() {
            let _ = lines.send((link, id, heard, reply));
        }
    }

    /// Whether `heard` names a chat that runs on this host, where the chats are routed: one that
    /// names none is of this host's own lines and hook connections.
    fn its_own(&self, heard: &purlis_core::hookwire::relay::Heard) -> bool {
        let Some(chat) = heard.chat() else {
            return true;
        };
        let routes = self
            .routes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        match routes {
            Some(routes) => routes(chat) == self.pool,
            // Not routed: this machine's host runs every chat, and no pool's lines come here.
            None => self.pool.is_none(),
        }
    }

    /// **The project's chats are routed by workspace** (#2046): from now on this host's lines
    /// are heard only for the chats `routes` says run on it.
    pub fn routed(&self, routes: Routes) {
        *self.routes.lock().unwrap_or_else(PoisonError::into_inner) = Some(routes);
    }

    /// **What hears the lines of `pool`'s runner** (#2046), beside this machine's host's: the
    /// same board and hearing, with links, secret execs, held asks and programs of its own, since
    /// each host numbers its own; and only for the chats `routes` says run on `pool`. The board
    /// is this machine's host's to keep, held there before any chat starts on a runner, so no
    /// line waits for it here.
    pub fn for_a_pool(&self, pool: &str, routes: Routes) -> Arc<Relayed> {
        Arc::new(Relayed {
            heeding: Arc::new(self.heeding.beside()),
            in_order: Mutex::new(None),
            socket: self.socket.clone(),
            programs: Mutex::default(),
            board: Arc::clone(&self.board),
            plane: self.plane.clone(),
            moved: Arc::clone(&self.moved),
            waiting: Mutex::new(None),
            pool: Some(pool.to_owned()),
            routes: Mutex::new(Some(routes)),
        })
    }

    /// The host's lines for this project come over link `link` from now on: a link other than
    /// the one before may be a host started again, so nothing kept for the one before is kept
    /// on (#2271).
    pub fn linked(&self, link: u64) {
        self.heeding.linked(link);
    }

    /// Link `link` to the host ended: where this project's lines came over it, nothing kept
    /// for them is kept on (#2271).
    pub fn unlinked(&self, link: u64) {
        self.heeding.unlinked(link);
    }

    /// The socket the host listens on for this project.
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// **The window holds the project in the host** (#2194): it takes `kept`, the board the
    /// host keeps, as its own, where the host keeps one, and is told every chat on it as the
    /// host kept it; from now on each move it makes is told to `keeper`, in the order made.
    /// Then the lines that waited for this are heard, in the order they came.
    pub fn keep_with(
        &self,
        keeper: purlis_core::state::Keeper,
        kept: Option<purlis_core::state::Kept>,
    ) {
        let told: Vec<Moved> = {
            let mut board = held_board(&self.board);
            board.told_to(keeper);
            match kept {
                Some(kept) => {
                    board.keep(kept);
                    let chats: Vec<u32> = board.chats().collect();
                    chats
                        .into_iter()
                        .map(|chat| seen_by(&board, &self.plane, chat))
                        .collect()
                }
                None => Vec::new(),
            }
        };
        for moved in told {
            (self.moved)(moved);
        }
        // Heard with the waiting held, so no line the host hands on now is heard before the
        // ones that came before it (#2571): a secret exec's input would otherwise find its exec
        // not yet taken, and be dropped.
        let mut waiting = self.waiting.lock().unwrap_or_else(PoisonError::into_inner);
        // Only this machine's host's lines wait for its board, and its link reaches no runner.
        for (link, id, heard, reply) in waiting.take().unwrap_or_default() {
            self.hand_on(link, None, id, heard, reply);
        }
    }

    /// Chat `chat`'s program, started by the host, is process `pid`.
    pub fn started(&self, chat: u32, pid: u32) {
        let mut programs = self.programs.lock().unwrap_or_else(PoisonError::into_inner);
        match purlis_core::hookwire::Program::of(pid) {
            Some(program) => {
                programs.insert(chat, program);
            }
            None => {
                programs.remove(&chat);
            }
        }
    }

    /// Chat `chat`'s program `pid` ended, or the chat was closed (`None`).
    pub fn ended(&self, chat: u32, pid: Option<u32>) {
        let mut programs = self.programs.lock().unwrap_or_else(PoisonError::into_inner);
        if pid.is_none_or(|pid| {
            programs
                .get(&chat)
                .is_some_and(|program| program.pid == pid)
        }) {
            programs.remove(&chat);
        }
    }
}

/// What runs a brokered `secret exec` (#1407): the ask, the connection it is held on, and what
/// is told each host the run's sandbox refused, as the asking chat's block (the same road a
/// block its own hook found takes: [`heard_block`]).
pub type SecretExecs = Arc<
    dyn Fn(
            purlis_core::secrets::brokered::Ask,
            Box<dyn std::io::BufRead + Send>,
            Box<dyn std::io::Write + Send>,
            purlis_core::secrets::brokered::Told,
        ) + Send
        + Sync
        + 'static,
>;

/// What is told each sandbox block a chat's hook found, as the hook sent it.
pub type Blocks = Arc<dyn Fn(purlis_core::hookwire::SandboxBlocked) + Send + Sync + 'static>;

/// **A sandbox block as the app heard it** (#1353): the block a chat's hook or a proxy told,
/// and what only the app's own brokered road knows of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeardBlock {
    /// The block, as the line or the proxy carried it.
    pub line: purlis_core::hookwire::SandboxBlocked,
    /// For a host refused while a brokered `secret exec` ran, the vault the run was given, by
    /// name ([`purlis_core::secrets::brokered::RunBlocked`]). Set only on that road, from the
    /// run the app authorised: no line a chat sends carries it.
    pub vault: Option<String>,
}

impl From<purlis_core::hookwire::SandboxBlocked> for HeardBlock {
    fn from(line: purlis_core::hookwire::SandboxBlocked) -> Self {
        Self { line, vault: None }
    }
}

impl From<purlis_core::secrets::brokered::RunBlocked> for HeardBlock {
    fn from(run: purlis_core::secrets::brokered::RunBlocked) -> Self {
        Self {
            line: run.block,
            vault: Some(run.vault),
        }
    }
}

/// What is told each sandbox block the app let through, as it heard it.
pub type HeardBlocks = Arc<dyn Fn(HeardBlock) + Send + Sync + 'static>;

/// The event the window is sent when a chat's sandbox blocked an operation (#1338).
pub const SANDBOX_BLOCKED: &str = "chat-sandbox-blocked";

/// The event the window is sent when a chat's proxy has refused more hosts past the ones it
/// names (#1353).
pub const SANDBOX_REFUSED_MORE: &str = "chat-sandbox-refused-more";

/// **How many hosts a chat's proxy refused past the ones it names** (#1353): the proxy keeps
/// the first few (`purlis_core::sandbox::egress::REFUSALS_KEPT`), each a block of its own, and
/// counts the rest by digest alone. Told each time the count grows, so the chat's host Notice
/// says "and N more" and no host is hidden behind the first eight. A count, never a host.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RefusedMore {
    pub plane: PlaneId,
    pub session: u32,
    /// How many, each host and port once, up to the proxy's bound.
    pub more: u32,
}

/// Told each [`RefusedMore`], as the window shows it.
pub type RefusedMoreTeller = Arc<dyn Fn(RefusedMore) + Send + Sync + 'static>;

/// A sandbox block, as the window shows it on the chat's tab (#1338): an operation and the kind
/// of path or host, by their fixed words, and the sentence purlis says about them; and what the
/// Notice can offer for it (#1342): Allow for a host or a folder, with what it would name, or
/// the way that works for what is never granted. **No argument or output**.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatBlocked {
    pub plane: PlaneId,
    pub session: u32,
    /// The operation's word: `write`, `read`, `connect`, ….
    pub operation: String,
    /// The kind's word: `project-files`, `toolchain-cache`, `host`, ….
    pub kind: String,
    /// Whether it was purlis's own operation: a purlis bug, which the Notice offers to report.
    pub ours: bool,
    /// The harness the chat runs, by the word the project calls it, when it is one purlis
    /// starts.
    pub harness: Option<String>,
    /// What was blocked, as a sentence names it: "a write to the project's own files".
    pub said: String,
    /// **The vault a brokered command ran with** when its sandbox refused this host (#1353), by
    /// name and drawn inertly, so the person can judge whether that vault belongs at the host
    /// before allowing it. Never a value: a host that carries one of the run's values raises no
    /// Notice at all. None for a block of the chat's own command.
    #[specta(optional)]
    pub vault: Option<String>,
    /// What the Notice offers (#1342).
    pub offer: BlockOffer,
    /// What Allow would name, shown whole before anyone presses it: the host the report named,
    /// or the folder a refused write was in. Null for a host the report did not name, which the
    /// person types. The app checks it again before anything is granted.
    pub target: Option<String>,
    /// For [`BlockOffer::Socket`] and [`BlockOffer::Service`], what allowing it means, honestly
    /// (D-sock-6); for [`BlockOffer::Brokered`], the way that works instead; for
    /// [`BlockOffer::Unsandboxed`], why purlis grants nothing here; for [`BlockOffer::Policy`],
    /// what policy forbids, naming the policy and who set it (#1343).
    pub route: Option<String>,
    /// For [`BlockOffer::Host`] and [`BlockOffer::Write`], the levels Allow may keep it at:
    /// each a policy does not forbid (#1343).
    pub levels: Vec<crate::sandboxing::GrantLevel>,
    /// Whether the connection is **held** while the person answers (#1666): purlis's proxy
    /// waits on this Notice, and an Allow lets the same command carry on with nothing
    /// restarting. Set by the app from the chat's own board, never from what a chat sent.
    pub held: bool,
    /// What an administrator's policy ruled out here, and who set it (#1666): a scope it
    /// removed, "Allow once" among them (#1927), or, for a host, asking while a connection
    /// waits. None where it ruled nothing out.
    pub ruled: Option<String>,
    /// **What the person may choose, where the block named nothing a grant could name**
    /// (#1884, ADR 0093 as amended 2026-10-13): a local socket, from the ones purlis finds on
    /// this machine or by a path they type. (A system service would be chosen from the ones
    /// purlis can allow; it lists none since the certificate check became a setting, R1.)
    /// Offered beside Start without the sandbox, in the window only
    /// (`choose_sandbox_block_target`), and what they choose is judged as a named one is. None
    /// where there is nothing to choose, or policy forbids every such grant.
    #[specta(optional)]
    pub choose: Option<crate::sandboxing::GrantWhat>,
    /// **For a block found in what a tool that ran nothing read back** (#1884): the sentence
    /// saying so. Its Notice is a quiet line: nothing waits on it, the chat is not told to
    /// wait, and the asks registry does not list it until the person takes it up
    /// (`take_up_sandbox_block`). None for a block of a command the chat ran.
    #[specta(optional)]
    pub quiet: Option<String>,
    /// **A port the command's own output names**, for a service on this Mac whose refusal named
    /// none ([`BlockOffer::Port`] with no [`Self::target`], qw384): the Notice pre-fills its
    /// port field with it and says it came from the command's output, for the person to check.
    /// Judged as a port is before it is sent here, and again before anything is allowed.
    #[specta(optional)]
    pub suggested: Option<String>,
}

/// The way round a connection to an address dialled past the proxy (#2423).
const PAST_THE_PROXY_ROUTE: &str = "This program connects to an internet address itself \
                                    instead of going through the sandbox's proxy, so no host \
                                    can be allowed for it. A client that goes through the \
                                    sandbox's proxy, such as curl, gh or kubectl, reaches a host \
                                    this project allows.";

/// What the quiet line of a connection dialled past the proxy says of it (#2423): nothing
/// waits on it, since nothing the person allows lets it through.
const PAST_THE_PROXY_SAID: &str = "A program connected to an internet address directly, \
                                       so nothing waits on your answer.";

/// The way round a lookup a program made itself, past the proxy (#1631): through the system's
/// name lookup, whose socket on Linux is no socket to allow (#2428).
const LOOKUP_PAST_THE_PROXY: &str = "This program looks its host up itself, through the \
                                     system's name lookup, instead of going through the \
                                     sandbox's proxy. Allowing the host would not let it \
                                     through, and letting it reach the system's name lookup \
                                     would let this chat look up any name past the proxy, so \
                                     purlis offers neither. A client that uses the proxy, such \
                                     as curl, gh or kubectl, reaches a host this project allows.";

/// What a quiet line says of where its block came from ([`ChatBlocked::quiet`]).
pub const READ_BACK_SAID: &str = "It was in the output of a command that ran earlier, so \
                                  nothing waits on your answer.";

/// What an unnamed socket's Notice says in place of "there is no socket to allow" (#1884).
const CHOOSE_SOCKET_SAID: &str = "The sandbox did not say which socket it refused, so you can \
                                  choose it. purlis checks the socket you choose as it checks \
                                  one the sandbox names.";

/// **What the person may choose for `block`** ([`ChatBlocked::choose`]), given what [`offered`]
/// and the chat's harness came to (`offer`): only where the
/// Notice would offer no Allow (Start without the sandbox), never for purlis's own operation,
/// and never where policy forbids every socket and service grant. A socket only where its grant
/// compiles (D-sock-7, macOS), also where the one the sandbox named may not be granted (one in
/// a folder a chat may write): the person may choose the one the service keeps elsewhere.
fn choosable(
    locks: &purlis_core::sandbox::policy::Locks,
    block: &purlis_core::sandboxblock::Block,
    offer: BlockOffer,
) -> Option<crate::sandboxing::GrantWhat> {
    use crate::sandboxing::GrantWhat;
    use purlis_core::sandboxblock::{Kind, Operation};
    if block.ours || offer != BlockOffer::Unsandboxed || locks.socket_grants_refused().is_some() {
        return None;
    }
    match (block.operation, block.kind) {
        (Operation::Connect, Kind::LocalSocket) if cfg!(target_os = "macos") => {
            Some(GrantWhat::Socket)
        }
        // No service is chosen: purlis lists none it can allow
        // (`services::GRANTABLE_SERVICES`). The certificate check, the one it listed, is the
        // project's setting and never a grant (R1), and its Notice says what stands. The
        // chooser's service half is kept for the next service listed there (#1927): listing
        // one also needs an arm here, `(Operation::Lookup, Kind::SystemService)` to
        // `GrantWhat::Service`, and a sentence for its Notice beside `CHOOSE_SOCKET_SAID`.
        _ => None,
    }
}

/// What the app does with a sandbox block chat `block.chat`'s hook sent (#1338), heard at `at`:
/// let through the chat's [`purlis_core::sandboxblock::Throttle`], then handed to whoever `slot`
/// holds, which records it in this machine's network record for `purlis doctor`'s count and
/// the Network views (#1662, `planes.rs`) and shows the chat's notice. A block the throttle
/// holds back is not shown; a repeat of one it let through is counted, and told to whoever
/// `repeated` holds for the record once its minute is over, here, at the next block heard
/// (#1681), or sooner: when its chat goes quiet or ends, and when the project is let go of.
/// Each listener is taken out of the lock before it runs, as an answer is.
fn heard_block(
    throttle: &Mutex<purlis_core::sandboxblock::Throttle>,
    slot: &Mutex<Option<HeardBlocks>>,
    repeated: &Mutex<Option<Repeats>>,
    block: HeardBlock,
    at: std::time::Instant,
) {
    // Keyed on the block and what it names, never on the vault: a run's vault buys no Notice
    // the same block without one would not get.
    let let_through = {
        let mut throttle = throttle.lock().unwrap_or_else(PoisonError::into_inner);
        let line = &block.line;
        // A block read back is never the same as one a command was refused (#1884).
        if line.read_back {
            throttle.lets_on_read(line.chat, &line.sandbox_blocked, line.target.as_deref(), at)
        } else {
            throttle.lets_on(line.chat, &line.sandbox_blocked, line.target.as_deref(), at)
        }
    };
    tell_repeats(throttle, repeated, |t| t.repeats_over(at));
    if !let_through {
        return;
    }
    let listener = slot.lock().unwrap_or_else(PoisonError::into_inner).clone();
    if let Some(listener) = listener {
        listener(block);
    }
}

/// The repeats `take` takes out of `throttle`, told to whoever `repeated` holds, taken out of
/// its lock before it runs, as an answer is.
fn tell_repeats(
    throttle: &Mutex<purlis_core::sandboxblock::Throttle>,
    repeated: &Mutex<Option<Repeats>>,
    take: impl FnOnce(
        &mut purlis_core::sandboxblock::Throttle,
    ) -> Vec<purlis_core::sandboxblock::Repeated>,
) {
    let over = take(&mut throttle.lock().unwrap_or_else(PoisonError::into_inner));
    if over.is_empty() {
        return;
    }
    let counter = repeated
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if let Some(counter) = counter {
        counter(over);
    }
}

/// What a brokered run's blocks are told to (#1353): the road `hear` is, each with the vault the
/// run was given.
fn run_told(hear: HeardBlocks) -> purlis_core::secrets::brokered::Told {
    Arc::new(move |run| hear(run.into()))
}

/// Told the repeats of blocks the throttle held back, once their minute is over (#1681).
pub type Repeats = Arc<dyn Fn(Vec<purlis_core::sandboxblock::Repeated>) + Send + Sync + 'static>;

/// Told each sandbox block, as the window shows it.
pub type BlockTeller = Arc<dyn Fn(ChatBlocked) + Send + Sync + 'static>;

/// What the window is told of a block a chat's hook sent on `plane`'s socket, or a brokered
/// run's proxy refused. A harness word the app does not start is no harness: the line is the
/// chat's own. `has_check`: whether the chat's sandbox was compiled with the system's
/// certificate check, by the app's own record of its start ([`blocked_on`]).
pub fn blocked_holding(plane: &PlaneId, heard: &HeardBlock, has_check: bool) -> ChatBlocked {
    let blocked = &heard.line;
    let harness = blocked
        .harness
        .as_deref()
        .and_then(purlis_core::harness::Harness::of_kind);
    let told = blocked_on(
        plane,
        blocked.chat,
        &blocked.sandbox_blocked,
        harness,
        blocked.target.as_deref(),
        has_check,
    );
    ChatBlocked {
        vault: heard.vault.as_deref().map(vault_said),
        quiet: if blocked.read_back {
            Some(READ_BACK_SAID.to_owned())
        } else {
            past_the_proxy(&blocked.sandbox_blocked).then(|| PAST_THE_PROXY_SAID.to_owned())
        },
        suggested: suggested_port(&told, blocked.suggested.as_deref()),
        ..told
    }
}

/// Whether `block` is a connection a program dialled past the proxy, which no answer lets
/// through (#2423).
fn past_the_proxy(block: &purlis_core::sandboxblock::Block) -> bool {
    use purlis_core::sandboxblock::{Kind, Operation};
    !block.ours && (block.operation, block.kind) == (Operation::Connect, Kind::Address)
}

/// The port `suggested` by a command's output, for a Notice that asks the person to type one
/// ([`ChatBlocked::suggested`], qw384): none for any other Notice, and none that is no port a
/// grant could name.
fn suggested_port(told: &ChatBlocked, suggested: Option<&str>) -> Option<String> {
    (matches!(told.offer, BlockOffer::Port | BlockOffer::Listen) && told.target.is_none())
        .then(|| purlis_core::sandbox::localport::port(suggested?).ok())
        .flatten()
        .map(|port| port.to_string())
}

/// [`blocked_holding`] for a chat that started without the certificate check.
#[cfg(test)]
pub fn blocked(plane: &PlaneId, heard: &HeardBlock) -> ChatBlocked {
    blocked_holding(plane, heard, false)
}

/// **What the window is told of `block`, refused chat `session` of `harness` on `target`**: what
/// [`blocked`] says of a block a hook sent, and what a block says once the person chose its
/// target (`crate::sandboxchoose`), so a chosen socket or service is judged, worded and offered
/// exactly as one the sandbox named (#1884). `has_check`: whether this chat's sandbox was
/// compiled with the system's certificate check as it started, which decides what a refused
/// check's Notice says ([`certificate_check_refused`]); never a word the chat sent.
pub(crate) fn blocked_on(
    plane: &PlaneId,
    session: u32,
    block: &purlis_core::sandboxblock::Block,
    harness: Option<purlis_core::harness::Harness>,
    target: Option<&str>,
    has_check: bool,
) -> ChatBlocked {
    let locks = purlis_core::sandbox::policy::Locks::of(plane.root());
    let (first, named, why) =
        for_its_harness(harness, offered_to(plane.root(), block, target, has_check));
    // Chosen by the person where nothing was named (#1884): said in place of "nothing to
    // allow". A socket named and refused keeps its own sentence, and may be chosen again.
    let choose = choosable(&locks, block, first);
    let why = match (choose, &named) {
        (Some(_), None) => Some(CHOOSE_SOCKET_SAID.to_owned()),
        _ => why,
    };
    let (offer, target, route, levels) = held(&locks, (first, named, why));
    let ruled = match offer {
        BlockOffer::Host => ruled_out(&locks, target.as_deref(), &levels),
        other => scopes_ruled_out(&locks, other),
    };
    let (offer, route, levels) = match allowed_already(plane.root(), offer, target.as_deref()) {
        Some(said) if block.kind != purlis_core::sandboxblock::Kind::RunnerService => {
            (BlockOffer::Allowed, Some(said), Vec::new())
        }
        _ => (offer, route, levels),
    };
    // A service of the runner the chat runs on is allowed once or for this chat, never wider
    // (#2510): nothing of it is kept on this machine.
    let levels = if block.kind == purlis_core::sandboxblock::Kind::RunnerService {
        if offer == BlockOffer::Port {
            vec![
                crate::sandboxing::GrantLevel::Once,
                crate::sandboxing::GrantLevel::Chat,
            ]
        } else {
            Vec::new()
        }
    } else {
        levels
    };
    ChatBlocked {
        offer,
        target,
        route,
        levels,
        held: false,
        ruled,
        choose,
        quiet: None,
        plane: plane.clone(),
        session,
        operation: block.operation.word().to_owned(),
        kind: block.kind.word().to_owned(),
        ours: block.ours,
        harness: harness.map(|harness| harness.name().to_owned()),
        said: block.said(),
        vault: None,
        suggested: None,
    }
}

/// What a runner's service whose port the block did not name says (#2510): never raised by
/// purlis's own runner, which always names the port it asks about.
const RUNNER_SERVICE_UNNAMED: &str = "The runner this chat runs on refused a service on it \
                                      without naming its port, so there is nothing to allow \
                                      here.";

/// **What the window says of a block on a service of the runner chat `told.session` runs on**
/// (#2510): its Notice names that runner, `runner`, where the window knows its name. Any other
/// block is as it was told.
pub(crate) fn on_its_runner(mut told: ChatBlocked, runner: Option<&str>) -> ChatBlocked {
    if told.kind == purlis_core::sandboxblock::Kind::RunnerService.word()
        && told.offer == BlockOffer::Port
        && let Some(port) = told
            .target
            .as_deref()
            .and_then(|port| port.parse::<u16>().ok())
    {
        told.route = Some(purlis_core::sandbox::localport::on_the_runner_said(
            port, runner,
        ));
    }
    told
}

/// **What the window says of a refused server on a port chat `told.session` serves on
/// already** (#2019): its server asked for the port itself, not through its socket, so allowing
/// it again would change nothing and restart the chat for nothing. Its Notice says how to start
/// it, with no Allow; `serving` are the ports the chat serves on. Any other block is as it was
/// told.
pub(crate) fn on_its_serving(mut told: ChatBlocked, serving: &[u16]) -> ChatBlocked {
    // The door on a port it serves on listens there, so the refusal is offered as a service in
    // use (`offered_to`): a refused listen either way.
    if told.operation == purlis_core::sandboxblock::Operation::Listen.word()
        && matches!(told.offer, BlockOffer::Listen | BlockOffer::Port)
        && let Some(port) = told
            .target
            .as_deref()
            .and_then(|port| port.parse::<u16>().ok())
        && serving.contains(&port)
    {
        told.offer = BlockOffer::Brokered;
        told.levels = Vec::new();
        told.route = Some(format!(
            "Port {port} is allowed for this chat's server: {}.",
            purlis_core::sandbox::localport::listen_said(port)
        ));
    }
    told
}

/// A vault's name as a Notice draws it (#1353): one line, every credential shape masked and
/// every character that draws as nothing written out, as an ask's line is drawn.
fn vault_said(name: &str) -> String {
    purlis_core::harness::model::Summary::of(&purlis_core::dispatchgrant::inert(name))
        .as_str()
        .to_owned()
}

/// **What a Notice says of a host, a local socket or a system service that is allowed
/// already** (#1666's fold-in; ADR 0093): no Allow is offered for it, since one would allow
/// nothing new. This chat started before it was allowed, so it reaches it once it restarts; a
/// chat on purlis's proxy takes a host's Allow at once and is never shown this, as its held
/// connection is let on as soon as it is asked (`chats::asked_by_the_proxy`). None for anything
/// else offered.
///
/// **A socket or a service you allowed for every chat of this project on this machine** is one
/// a chat takes only as it starts (D-sock-3), so a chat that started before the Allow is
/// refused it still, and so is every other chat open then. Its Notice says the Allow stands
/// and offers Restart this chat, never the same Allow over again.
fn allowed_already(
    root: &std::path::Path,
    offer: BlockOffer,
    target: Option<&str>,
) -> Option<String> {
    use purlis_core::sandbox::{grant, local};
    let locks = purlis_core::sandbox::policy::Locks::of(root);
    // For this project, or for every project on this machine (R2), at a scope policy leaves
    // open: one kept at a scope it now leaves out reaches no chat (#1927), so a restart would
    // change nothing, and the Notice offers what policy leaves open instead.
    let in_force = |what: grant::What, yours: bool, everywhere: bool| {
        [
            (grant::Level::You, yours),
            (grant::Level::Machine, everywhere),
        ]
        .into_iter()
        .find(|(level, kept)| *kept && locks.holds_kept(&what, *level).is_none())
        .map(|(level, _)| level)
    };
    let level = match offer {
        BlockOffer::Host => grant::allowed_already(root, &grant::host(target?).ok()?)?,
        BlockOffer::Socket => {
            let target = std::path::Path::new(target?);
            in_force(
                grant::What::Socket(target.into()),
                local::granted_sockets(root).iter().any(|one| one == target),
                local::sockets_everywhere().iter().any(|one| one == target),
            )?
        }
        BlockOffer::Port => {
            let port: u16 = target?.parse().ok()?;
            in_force(
                grant::What::Port(port),
                local::granted_ports(root).contains(&port),
                false,
            )?
        }
        BlockOffer::Listen => {
            let port: u16 = target?.parse().ok()?;
            in_force(
                grant::What::Listen(port),
                local::granted_listens(root).contains(&port),
                false,
            )?
        }
        BlockOffer::Service => {
            let target = target?;
            in_force(
                grant::What::Service(target.to_owned()),
                local::granted_services(root)
                    .iter()
                    .any(|one| one == target),
                local::services_everywhere().iter().any(|one| one == target),
            )?
        }
        _ => return None,
    };
    Some(format!(
        "It is allowed already, {}. This chat started before that, so it reaches it once it \
         restarts on the same conversation.",
        level.said()
    ))
}

/// **What policy ruled out on a host's Notice** (#1666): the first scope it removed, and that
/// asking while a connection waits is off, each naming who set it. None where it ruled nothing
/// out, or where it ruled out every Allow ([`held`] then says so instead).
fn ruled_out(
    locks: &purlis_core::sandbox::policy::Locks,
    target: Option<&str>,
    levels: &[crate::sandboxing::GrantLevel],
) -> Option<String> {
    use crate::sandboxing::GrantLevel;
    use purlis_core::sandbox::grant::{self, What};
    let host = target.and_then(|typed| grant::host(typed).ok());
    let removed = [GrantLevel::Chat, GrantLevel::You, GrantLevel::Project]
        .into_iter()
        .filter(|level| !levels.contains(level))
        .find_map(|level| match &host {
            Some(host) => locks.refuses_grant(&What::Host(host.clone()), level.into()),
            None => None,
        });
    let said: Vec<String> = removed
        .into_iter()
        .chain(locks.live_asks_refused())
        .collect();
    (!said.is_empty()).then(|| said.join(" "))
}

/// **What policy ruled out on a socket's, a service's or a folder's Notice** (#1927): each
/// scope the Notice would offer that `allow-scopes` removed, named, with who set it. "Allow
/// once" is the first answer such a Notice offers, so one a policy removed is said and never
/// only missing. None for any other offer, and where policy removed none.
fn scopes_ruled_out(
    locks: &purlis_core::sandbox::policy::Locks,
    offer: BlockOffer,
) -> Option<String> {
    use purlis_core::sandbox::grant::Level;
    let offered: &[Level] = match offer {
        BlockOffer::Socket | BlockOffer::Service => {
            &[Level::Once, Level::Machine, Level::Chat, Level::You]
        }
        BlockOffer::Write | BlockOffer::Port | BlockOffer::Listen => &[Level::Chat, Level::You],
        _ => return None,
    };
    locks.scopes_removed(offered)
}

/// What a block's Notice offers (#1342).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum BlockOffer {
    /// Allow a host.
    Host,
    /// Allow a folder to write.
    Write,
    /// Allow a local socket (D-sock-1): [`ChatBlocked::route`] says what allowing it means.
    Socket,
    /// Allow a system service by name (D-sock-1): [`ChatBlocked::route`] says what allowing it
    /// means.
    Service,
    /// Allow a service on this Mac by its one port (qw384): [`ChatBlocked::route`] says what
    /// allowing it means. [`ChatBlocked::target`] is null where the refusal named no port, which
    /// the person types.
    Port,
    /// Allow listening on one port of this Mac's loopback (#2019): [`ChatBlocked::route`] says
    /// what allowing it means. [`ChatBlocked::target`] is null where the refusal named no port,
    /// which the person types.
    Listen,
    /// Never granted, and there is a way that works: [`ChatBlocked::route`] says it.
    Brokered,
    /// Not something purlis grants a chat (off the allowlist, a socket it cannot name): the person may
    /// start this chat again without the sandbox; [`ChatBlocked::route`] says why.
    Unsandboxed,
    /// Nothing to allow: purlis's own operation (a bug to report), or a block with nothing a
    /// grant could name.
    None,
    /// What would be offered is forbidden by policy, and so is starting the chat without the
    /// sandbox (#1343): nothing is offered, and [`ChatBlocked::route`] names the policy and who
    /// set it, so the person knows whom to ask.
    Policy,
    /// The host is allowed already (#1666's fold-in): no Allow, since one would allow nothing
    /// new. [`ChatBlocked::route`] says so and that this chat reaches it once it restarts; the
    /// Notice offers Restart this chat.
    Allowed,
}

impl BlockOffer {
    /// **What an Allow on this offer grants**, of the kinds a block names: none for an offer
    /// with no Allow of its own.
    pub(crate) fn granted(self) -> Option<crate::sandboxing::GrantWhat> {
        use crate::sandboxing::GrantWhat;
        Some(match self {
            Self::Host => GrantWhat::Host,
            Self::Write => GrantWhat::Write,
            Self::Socket => GrantWhat::Socket,
            Self::Service => GrantWhat::Service,
            Self::Port => GrantWhat::Port,
            Self::Listen => GrantWhat::Listen,
            Self::Brokered | Self::Unsandboxed | Self::None | Self::Policy | Self::Allowed => {
                return None;
            }
        })
    }
}

/// What a block's Notice offers once `locks`, an administrator's policy (#1343), have their say
/// on what [`offered`] would offer: the strictest wins.
///
/// - Allow is offered only at the levels policy leaves open ([`ChatBlocked::levels`]); a host
///   named whole is judged as it would be kept at each level.
/// - Where policy leaves no Allow, or forbids what a Start without the sandbox stood in for,
///   the Notice offers Start without the sandbox, saying why, unless policy forbids that too:
///   then it offers nothing and says why ([`BlockOffer::Policy`]). It never dead-ends silently.
fn held(
    locks: &purlis_core::sandbox::policy::Locks,
    (offer, target, route): (BlockOffer, Option<String>, Option<String>),
) -> (
    BlockOffer,
    Option<String>,
    Option<String>,
    Vec<crate::sandboxing::GrantLevel>,
) {
    use crate::sandboxing::GrantLevel;
    use purlis_core::sandbox::grant::{self, What};
    let instead = |target: Option<String>, why: String| match locks.opt_out_refused() {
        None => (BlockOffer::Unsandboxed, target, Some(why), Vec::new()),
        Some(_) => (
            BlockOffer::Policy,
            target,
            Some(format!(
                "{why} Policy forbids starting this chat without the sandbox too."
            )),
            Vec::new(),
        ),
    };
    match offer {
        BlockOffer::Host => {
            let named = target.as_deref().and_then(|typed| grant::host(typed).ok());
            let mut why = None;
            let levels: Vec<GrantLevel> = [GrantLevel::Chat, GrantLevel::You, GrantLevel::Project]
                .into_iter()
                .filter(|level| {
                    let refused = match &named {
                        Some(host) => {
                            locks.refuses_grant(&What::Host(host.clone()), (*level).into())
                        }
                        // A host the person types is judged once typed; here only the level.
                        None => (locks.forbids_personal_hosts() && *level != GrantLevel::Project)
                            .then(|| {
                                format!("Policy forbids hosts of your own. {}", locks.locked_by())
                            }),
                    };
                    if refused.is_some() && why.is_none() {
                        why = refused.clone();
                    }
                    refused.is_none()
                })
                .collect();
            match (levels.is_empty(), why) {
                (true, Some(why)) => instead(target, why),
                _ => (offer, target, route, levels),
            }
        }
        // A socket or a service (D-sock-1, D-sock-3; R2): once, for this chat, for this project
        // on this machine, or for every project on it, each a policy leaves open; never the
        // project's.
        BlockOffer::Socket | BlockOffer::Service => {
            let named = target.as_deref().map(|target| match offer {
                BlockOffer::Socket => What::Socket(target.into()),
                _ => What::Service(target.to_owned()),
            });
            let mut why = locks.socket_grants_refused();
            let levels: Vec<GrantLevel> = [
                GrantLevel::Once,
                GrantLevel::Machine,
                GrantLevel::Chat,
                GrantLevel::You,
            ]
            .into_iter()
            .filter(|level| {
                // A block that names nothing yet is still held to the scopes policy leaves
                // open (#1927): what the person then chooses is offered at no other.
                let refused = match named.as_ref() {
                    Some(what) => locks.refuses_grant(what, (*level).into()),
                    None => locks.scope_removed((*level).into()),
                };
                if refused.is_some() && why.is_none() {
                    why = refused.clone();
                }
                refused.is_none()
            })
            .collect();
            match (levels.is_empty(), why) {
                (true, Some(why)) => instead(target, why),
                _ => (offer, target, route, levels),
            }
        }
        // A service on this Mac (qw384): for this chat, or for every chat of this project on
        // this machine, each a policy leaves open; never once, never the project's.
        // A port to listen on (#2019) at the same two scopes, held by the same lock.
        BlockOffer::Port | BlockOffer::Listen => {
            let named = target
                .as_deref()
                .and_then(|port| port.parse::<u16>().ok())
                .and_then(|port| offer.granted()?.of_port(port));
            let refused_all = || match offer {
                BlockOffer::Listen => locks.listen_grants_refused(),
                _ => locks.port_grants_refused(),
            };
            let mut why = refused_all();
            let levels: Vec<GrantLevel> = [GrantLevel::Chat, GrantLevel::You]
                .into_iter()
                .filter(|level| {
                    let refused = match named.as_ref() {
                        Some(what) => locks.refuses_grant(what, (*level).into()),
                        None => refused_all().or_else(|| locks.scope_removed((*level).into())),
                    };
                    if refused.is_some() && why.is_none() {
                        why = refused.clone();
                    }
                    refused.is_none()
                })
                .collect();
            match (levels.is_empty(), why) {
                (true, Some(why)) => instead(target, why),
                _ => (offer, target, route, levels),
            }
        }
        BlockOffer::Write => {
            let mut why = locks.write_grants_refused();
            // At the scopes policy leaves open alone (#1927): one it removed was drawn here
            // and then refused at the press.
            let levels: Vec<GrantLevel> = [GrantLevel::Chat, GrantLevel::You]
                .into_iter()
                .filter(|level| {
                    let refused = locks.scope_removed((*level).into());
                    if refused.is_some() && why.is_none() {
                        why = refused.clone();
                    }
                    refused.is_none()
                })
                .collect();
            match (locks.forbids_write_grants() || levels.is_empty(), why) {
                (true, Some(why)) => instead(target, why),
                _ => (offer, target, route, levels),
            }
        }
        BlockOffer::Unsandboxed => match locks.opt_out_refused() {
            Some(locked) => (
                BlockOffer::Policy,
                target,
                Some(
                    format!("{} {locked}", route.unwrap_or_default())
                        .trim()
                        .to_owned(),
                ),
                Vec::new(),
            ),
            None => (offer, target, route, Vec::new()),
        },
        BlockOffer::Brokered | BlockOffer::None | BlockOffer::Policy | BlockOffer::Allowed => {
            (offer, target, route, Vec::new())
        }
    }
}

/// What a block's Notice offers (#1342) for `block`, refused on `target` as the hook read it, in
/// the project at `root`: the offer, what Allow would name, and the sentence for one that is not
/// Allow.
///
/// - purlis's own operation is a purlis bug: Report, never Allow.
/// - A refused host: Allow, naming it where the report named one concrete host. A wildcard on
///   the line is never proposed (review S3): a real refusal names one host, and a person can
///   still type a wildcard in Settings.
/// - A refused write: Allow on the folder it was in, shown whole, where that folder is on the
///   allowlist ([`purlis_core::sandbox::grant::write`], D-1342-10); a denial class gets the way
///   that works; anything else gets "Start without the sandbox".
/// - A refused lookup of a host, by a program that does not use the proxy: "Start without the
///   sandbox", saying why a host grant would not help (#1631).
/// - A write purlis makes for a chat through one of its commands, by its path
///   ([`purlis_core::sandboxblock::way_out`]): that command, with no grant (#1408, #1476).
/// - purlis's state, a protected file and any read (reads are denied only for the classes no
///   person grants): the brokered route. A local socket: "Start without the sandbox".
/// - Anything else no grant names (the certificate check, a system service, starting a program,
///   a write whose folder the report did not name): "Start without the sandbox", saying why
///   (#1637). Only purlis's own block offers nothing to allow.
#[cfg(test)]
fn offered(
    root: &Path,
    block: &purlis_core::sandboxblock::Block,
    target: Option<&str>,
) -> (BlockOffer, Option<String>, Option<String>) {
    offered_to(root, block, target, false)
}

/// [`offered`] to a chat whose sandbox was compiled with the system's certificate check
/// (`has_check`) or without it: all that differs is what a refused check is told
/// ([`certificate_check_refused`]).
fn offered_to(
    root: &Path,
    block: &purlis_core::sandboxblock::Block,
    target: Option<&str>,
    has_check: bool,
) -> (BlockOffer, Option<String>, Option<String>) {
    use purlis_core::sandbox::{Class, grant, services};
    use purlis_core::sandboxblock::{Kind, Operation};
    if block.ours {
        return (BlockOffer::None, None, None);
    }
    let brokered = |class| {
        (
            BlockOffer::Brokered,
            None,
            Some(grant::brokered_route(class).to_owned()),
        )
    };
    // A write purlis makes for a chat through one of its commands (#1408, #1476): that command,
    // chosen by the path alone, and never a grant, which would let the chat past the app.
    if let (Operation::Write | Operation::File, Kind::ProjectFiles | Kind::ProtectedFile) =
        (block.operation, block.kind)
        && let Some(way) =
            target.and_then(|path| purlis_core::sandboxblock::way_out(Path::new(path), root))
    {
        return (BlockOffer::Brokered, None, Some(way.to_owned()));
    }
    match (block.operation, block.kind) {
        (Operation::Connect, Kind::Host) => (
            BlockOffer::Host,
            target.and_then(purlis_core::sandboxblock::named_host),
            None,
        ),
        // A program that looks its host up itself, not through the proxy (#1631): a host grant
        // would not let it through, so none is offered, and the Notice says why.
        // The host it looked up is shown, checked as a grant checks one (#1663), and never
        // offered: the Notice's Allow is not drawn for this offer.
        (Operation::Lookup, Kind::Host) => (
            BlockOffer::Unsandboxed,
            target.and_then(purlis_core::sandboxblock::named_host),
            Some(LOOKUP_PAST_THE_PROXY.to_owned()),
        ),
        // A program that connects to an address itself, past the proxy (#2423): as a lookup
        // past it, no host grant would let it through. The address is shown, never offered.
        (Operation::Connect, Kind::Address) => (
            BlockOffer::Unsandboxed,
            target
                .and_then(|address| address.parse::<std::net::SocketAddr>().ok())
                .map(|address| address.to_string()),
            Some(PAST_THE_PROXY_ROUTE.to_owned()),
        ),
        // A local socket (D-sock-1): Allow, naming it whole, with what allowing it means.
        (Operation::Connect, Kind::LocalSocket) => {
            let Some(path) = target else {
                return without_the_sandbox(
                    "The sandbox did not say which socket it refused, so there is no socket to \
                     allow.",
                );
            };
            // Only where a socket's grant compiles (D-sock-7): on macOS. Claude Code's sandbox
            // reads no socket path elsewhere, and purlis wraps no harness there yet.
            if !cfg!(target_os = "macos") {
                return without_the_sandbox(
                    "purlis can allow a local socket to a sandboxed chat on macOS only, so far.",
                );
            }
            let ground =
                services::SocketGround::of(root, &purlis_core::sandbox::Machine::this(), &[]);
            match services::socket(path, &ground) {
                Ok(socket) => {
                    let said = services::said_of(
                        &grant::What::Socket(socket.clone()),
                        &services::declared(root),
                    );
                    (
                        BlockOffer::Socket,
                        Some(socket.display().to_string()),
                        Some(said),
                    )
                }
                Err(grant::Refused::Never(route)) => (BlockOffer::Brokered, None, Some(route)),
                Err(grant::Refused::Outside(why) | grant::Refused::Not(why)) => {
                    (BlockOffer::Unsandboxed, Some(path.to_owned()), Some(why))
                }
            }
        }
        // A service on this Mac (qw384): Allow, naming its port where the refusal named one, or
        // the port the person types; never purlis's own.
        (Operation::Connect, Kind::LocalService) => {
            let said = format!("Allowing it {}.", purlis_core::sandbox::localport::RISK);
            match target.map(purlis_core::sandbox::localport::port) {
                None => (BlockOffer::Port, None, Some(said)),
                Some(Ok(port)) => (BlockOffer::Port, Some(port.to_string()), Some(said)),
                // A port you allowed that purlis took for a chat of its own while your service
                // was down: said as such, not as a port that is purlis's by nature.
                Some(Err(grant::Refused::Never(_)))
                    if target
                        .and_then(|port| port.trim().parse::<u16>().ok())
                        .is_some_and(|port| {
                            purlis_core::sandbox::local::granted_ports(root).contains(&port)
                        }) =>
                {
                    (
                        BlockOffer::Brokered,
                        None,
                        Some(purlis_core::sandbox::localport::TAKEN_SAID.to_owned()),
                    )
                }
                Some(Err(grant::Refused::Never(why))) => (BlockOffer::Brokered, None, Some(why)),
                Some(Err(grant::Refused::Outside(why) | grant::Refused::Not(why))) => {
                    (BlockOffer::Port, None, Some(why))
                }
            }
        }
        // A server the chat started on a port (#2019): Allow it to serve there, behind
        // purlis's door, naming its port where the refusal named one, or the port the person
        // types; never purlis's own, never below 1024, and never one something on this Mac
        // listens on, which is asked about as a service to connect to, in its own words.
        (Operation::Listen, Kind::LocalService) => {
            use purlis_core::sandbox::localport;
            let Some(target) = target else {
                return (
                    BlockOffer::Listen,
                    None,
                    Some(format!("Allowing it {}.", localport::listen_risk_unnamed())),
                );
            };
            match localport::port(target) {
                Ok(port) if port < localport::FIRST_SERVED => (
                    BlockOffer::Brokered,
                    None,
                    localport::serve_port(&port.to_string())
                        .err()
                        .map(|why| why.to_string()),
                ),
                Ok(port) if purlis_core::sandbox::door::in_use(port) => (
                    BlockOffer::Port,
                    Some(port.to_string()),
                    Some(format!(
                        "{} Allowing it as a service {}.",
                        localport::in_use_said(port),
                        localport::RISK
                    )),
                ),
                Ok(port) => (
                    BlockOffer::Listen,
                    Some(port.to_string()),
                    Some(format!("Allowing it {}.", localport::listen_risk(port))),
                ),
                Err(grant::Refused::Never(why)) => (BlockOffer::Brokered, None, Some(why)),
                Err(grant::Refused::Outside(why) | grant::Refused::Not(why)) => {
                    (BlockOffer::Listen, None, Some(why))
                }
            }
        }
        // A server that did not start, refused on no port a person could allow (#3396, #3440):
        // one the system picks, as a test suite's are, or one the sandbox named no port for. No
        // Allow names either; the Notice says so, and what to do.
        (Operation::Listen, Kind::System) => without_the_sandbox(match target {
            Some(_) => PICKED_PORT_SAID,
            None => NO_PORT_SAID,
        }),
        // A service of the runner the chat runs on, which its runner asks about (#2510): Allow,
        // naming its port, for this chat alone there. Its runner's name is said where the
        // window knows it ([`on_its_runner`]).
        (Operation::Connect, Kind::RunnerService) => {
            match target.and_then(|port| port.trim().parse::<u16>().ok()) {
                Some(port) => (
                    BlockOffer::Port,
                    Some(port.to_string()),
                    Some(purlis_core::sandbox::localport::on_the_runner_said(
                        port, None,
                    )),
                ),
                None => (
                    BlockOffer::Brokered,
                    None,
                    Some(RUNNER_SERVICE_UNNAMED.to_owned()),
                ),
            }
        }
        (_, Kind::ProjectState) => brokered(Class::Integrity),
        (_, Kind::ProtectedFile) => brokered(Class::LaterCode),
        (Operation::Read, _) => brokered(Class::Vaults),
        (
            Operation::Write | Operation::File,
            Kind::ProjectFiles | Kind::Home | Kind::ToolchainCache | Kind::System,
        ) => {
            let Some(path) = target.filter(|path| Path::new(path).is_absolute()) else {
                return without_the_sandbox(
                    "The sandbox did not say which folder it refused, so there is no folder to \
                     allow.",
                );
            };
            let folder = grant::proposed_folder(Path::new(path))
                .display()
                .to_string();
            let ground = grant::Ground::of(root, root, &purlis_core::sandbox::Machine::this());
            match grant::write(&folder, &ground.place()) {
                Ok(folder) => (BlockOffer::Write, Some(folder.display().to_string()), None),
                Err(grant::Refused::Never(route)) => (BlockOffer::Brokered, None, Some(route)),
                Err(grant::Refused::Outside(why) | grant::Refused::Not(why)) => {
                    (BlockOffer::Unsandboxed, Some(folder), Some(why))
                }
            }
        }
        // Nothing a grant names (#1637): never a Notice that only dismisses. Started without
        // the sandbox, the chat is refused none of these.
        // A system service by name (D-sock-1): Allow, naming it whole, with what allowing it
        // means. Never the certificate check's, which is no grant (R1): its Notice says what
        // stands and the way on.
        (Operation::Lookup, Kind::CertificateCheck | Kind::SystemService) => {
            match target.map(services::service) {
                Some(Ok(name)) if name == services::TRUSTD => {
                    certificate_check_refused(root, has_check)
                }
                // A refused certificate check that reached the app naming nothing (down a
                // pipe, read back later) is the certificate check all the same.
                None if block.kind == Kind::CertificateCheck => {
                    certificate_check_refused(root, has_check)
                }
                Some(Ok(name)) => {
                    let said = services::said_of(
                        &grant::What::Service(name.clone()),
                        &services::declared(root),
                    );
                    (BlockOffer::Service, Some(name), Some(said))
                }
                Some(Err(grant::Refused::Never(route))) => {
                    (BlockOffer::Brokered, None, Some(route))
                }
                // One no grant names, such as the system's name lookup (#2657): why, and the
                // chat without the sandbox.
                Some(Err(grant::Refused::Outside(why))) => (
                    BlockOffer::Unsandboxed,
                    target.map(str::to_owned),
                    Some(why),
                ),
                _ => without_the_sandbox(
                    "The sandbox did not say which system service it refused, so there is no \
                     service to allow.",
                ),
            }
        }
        (Operation::Lookup, _) => without_the_sandbox(
            "purlis will not let a sandboxed chat reach a system service by name: one could be \
             any service on this machine, so there is nothing to allow.",
        ),
        (Operation::Run, _) => without_the_sandbox(
            "purlis will not let a sandboxed chat start a program there, so there is nothing to \
             allow.",
        ),
        _ => without_the_sandbox(
            "purlis grants a chat nothing for this, so there is nothing to allow.",
        ),
    }
}

/// **What a Notice says of a refused certificate check** (the operator's ruling R1 of
/// 2026-10-13): every sandboxed chat has the check unless the project or policy turns it off,
/// so there is never an Allow for it, and never a dead end either.
///
/// - **On for the project**, and this chat started before it was (an update, or the project's
///   setting turned back on): the Notice says so and offers Restart this chat.
/// - **On for the project, and this chat started with it** (`has_check`, the app's own record
///   of what the chat's sandbox was compiled with): a restart would start it with the same
///   sandbox, so it is not offered. Something else refused the tool, and the Notice says so,
///   with Start without the sandbox as the way to tell.
/// - **Off by the project's own setting**: said, with where to turn it on (Settings › Sandbox,
///   which everyone in the project then gets) and Start without the sandbox.
/// - **Off by policy**: said, with who set it; [`held`] then offers Start without the sandbox
///   unless policy forbids that too.
fn certificate_check_refused(
    root: &Path,
    has_check: bool,
) -> (BlockOffer, Option<String>, Option<String>) {
    use purlis_core::sandbox::CertificateChecks;
    // No name beside it: the block's own words say "the system's certificate check", and the
    // service's name is nothing the person can act on.
    let target = None;
    match CertificateChecks::at(root) {
        None | Some(CertificateChecks::On) if has_check => (
            BlockOffer::Unsandboxed,
            target,
            Some(
                "This chat already has certificate checks, so restarting it would change \
                 nothing: something else refused this tool, and its own message may say what. \
                 To tell whether the sandbox is the cause, start this chat without the sandbox."
                    .to_owned(),
            ),
        ),
        None | Some(CertificateChecks::On) => (
            BlockOffer::Allowed,
            target,
            Some(
                "Certificate checks are on for this project. This chat started before they \
                 were, so tools such as gh can verify a host's certificate once it restarts on \
                 the same conversation."
                    .to_owned(),
            ),
        ),
        Some(CertificateChecks::OffByProject) => (
            BlockOffer::Unsandboxed,
            target,
            Some(
                "This project turns certificate checks off, so tools such as gh cannot verify \
                 a host's certificate in its sandboxed chats. Turn Certificate checks on in \
                 Settings › Sandbox, for everyone in the project, or start this chat without \
                 the sandbox."
                    .to_owned(),
            ),
        ),
        Some(CertificateChecks::OffByPolicy(why)) => (BlockOffer::Unsandboxed, target, Some(why)),
    }
}

/// **What `offer` comes to for a chat of `harness`** (D-sock-7): Claude Code's own sandbox
/// takes a socket by path and, of every system service, the certificate check alone
/// (`enableWeakerNetworkIsolation`), which is the project's setting and never a grant (R1). So
/// no service is one an Allow could reach there, and the Notice offers Start without the
/// sandbox, saying why. purlis's own wrap (Codex,
/// opencode) takes any service by name.
fn for_its_harness(
    harness: Option<purlis_core::harness::Harness>,
    (offer, target, route): (BlockOffer, Option<String>, Option<String>),
) -> (BlockOffer, Option<String>, Option<String>) {
    let claude = harness == Some(purlis_core::harness::Harness::ClaudeCode);
    if offer == BlockOffer::Service && claude {
        let (offer, _, why) = without_the_sandbox(
            "Claude Code's sandbox lets purlis allow no system service by name (the \
             certificate check is its own setting), so this one cannot be allowed for this chat.",
        );
        return (offer, target, why);
    }
    // A port to listen on (#2019): Claude Code's sandbox lets a command bind every loopback
    // port or none, never one, so nothing purlis allows reaches it there.
    if offer == BlockOffer::Listen && claude {
        let (offer, _, why) = without_the_sandbox(CLAUDE_CODE_LISTEN_SAID);
        return (offer, target, why);
    }
    // A service on this Mac (qw384b): Claude Code's sandbox takes no one port, so its chat is
    // carried the port by name through purlis's proxy, and the Notice says how it is reached.
    if offer == BlockOffer::Port && claude {
        let said = match route {
            Some(route) => format!("{route} {CLAUDE_CODE_PORT_SAID}"),
            None => CLAUDE_CODE_PORT_SAID.to_owned(),
        };
        return (offer, target, Some(said));
    }
    (offer, target, route)
}

/// How a Claude Code chat reaches a service on this Mac it is allowed (qw384b): its sandbox lets
/// a command reach this machine's loopback on every port or on none but its proxy's, never one
/// alone, and its commands send no address of this machine to a proxy. A name they do send, so
/// the chat's proxy carries the port by name ([`purlis_core::sandbox::localport::THIS_MAC`]).
pub const CLAUDE_CODE_PORT_SAID: &str = "In a Claude Code chat a command reaches it through this \
     chat's proxy, by the name this-mac.internal and that port, never by 127.0.0.1 or localhost: \
     curl https://this-mac.internal:<port>, or kubectl once Point kubectl at it gives this chat \
     its own copy of a context on this Mac. A program that uses no proxy, such as psql or \
     redis-cli, cannot reach it.";

/// Why a Claude Code chat cannot be allowed to listen on one port (#2019): its sandbox's
/// `allowLocalBinding` lets a command bind every port of this machine or none, never one, and
/// turning it on also lets the chat connect to every service on this machine; purlis never
/// allows every port.
pub const CLAUDE_CODE_LISTEN_SAID: &str = "Claude Code's sandbox can let a command listen on \
     every port of this Mac or on none, never on one alone, and letting it listen on every port \
     also lets this chat reach every service on this Mac. So purlis cannot allow this one port \
     for this chat. Start it without the sandbox, or start the server in a Codex or opencode \
     chat, which can be allowed its one port.";

/// Why a server on a port the system picks cannot be allowed (#3396): a test suite's servers
/// (`listen(0)`) take whatever port is free, so there is no one port to allow, and purlis
/// allows a chat's server only on one port of its own, behind its door. What to do instead.
pub const PICKED_PORT_SAID: &str = "It asked for a port the system picks, as a test suite's \
     servers often do, so there is no one port to allow: purlis lets a chat's server listen only \
     on a port of its own, from 1024. Start the server on such a port (PORT=3000, or purlis \
     listen 3000 -- <command>) to be asked about it, or start this chat without the sandbox to \
     run tests that pick their own ports.";

/// Why a server the sandbox named no port for cannot be allowed (#3440): what purlis would allow
/// is one port, and none was said.
pub const NO_PORT_SAID: &str = "The sandbox did not say which port it asked for, so there is no \
     port to allow. Start the server on a port of its own, from 1024 (PORT=3000, or purlis \
     listen 3000 -- <command>), to be asked about it, or start this chat without the sandbox.";

/// Start without the sandbox, saying `why` nothing is allowed: the way out of a block no grant
/// names (#1637), which an administrator's policy may still forbid ([`held`]).
fn without_the_sandbox(why: &str) -> (BlockOffer, Option<String>, Option<String>) {
    (BlockOffer::Unsandboxed, None, Some(why.to_owned()))
}

/// What is told each path a chat's file tool touched, unconfined.
pub type Touches = Arc<dyn Fn(purlis_core::hookwire::Touching) + Send + Sync + 'static>;

/// What hears what a chat's own tool hook says it is doing (#1981): the project's own handler,
/// which measures a task's quiet stretch from it ([`Hooks::when_doing`]).
pub type Doings = Arc<dyn Fn(u32, purlis_core::doing::Said) + Send + Sync + 'static>;

/// What hears what a chat's turn put out (#1981): the project's own handler, which keeps it
/// for a task alone ([`Hooks::when_output`]).
pub type Outputs = Arc<dyn Fn(purlis_core::hookwire::TurnOutput) + Send + Sync + 'static>;

/// The event the window is sent when a chat's file tool touches a file (FM-6).
pub const TOUCHING: &str = "chat-touching";

/// A file a chat's tool touched, as the window marks it in the tree for a few seconds (FM-6,
/// #1109). It travels in memory only and is never written anywhere (D-86a).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatTouching {
    pub plane: PlaneId,
    pub session: u32,
    /// The path inside the chat's own folder, `/`-separated, with no `..` and no `.git`:
    /// confined by `purlis_core::touching::confine` before it is sent.
    pub path: String,
}

/// Told each file a chat touched, once confined and let through the rate gate.
pub type TouchTeller = Arc<dyn Fn(ChatTouching) + Send + Sync + 'static>;

/// What the window is told of a path chat `touched.chat` said its tool touched, or nothing.
///
/// **Nothing unless it is inside the chat's own folder** (`folder`, where it works): confined
/// by core, so a chat can put no marker outside its branch. Nothing either once the chat has put
/// up its share this second, or for the same path again at once (`gate`), so no chat can flood
/// the window. A chat with no folder known marks nothing.
pub fn touched(
    plane: &PlaneId,
    folder: Option<&Path>,
    gate: &Mutex<purlis_core::touching::Gate>,
    touched: &purlis_core::hookwire::Touching,
    now: std::time::Instant,
) -> Option<ChatTouching> {
    let path = purlis_core::touching::confine(folder?, &touched.touching)?;
    gate.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .lets(touched.chat, &path, now)
        .then(|| ChatTouching {
            plane: plane.clone(),
            session: touched.chat,
            path,
        })
}

/// What is told a session record was saved.
pub type SavedHeard = Arc<dyn Fn(SessionSaved) + Send + Sync + 'static>;

/// What is told each report the board took.
pub type Heard = Arc<dyn Fn(&Report) + Send + Sync + 'static>;

/// What chat `session` waits on that is not the person, by the project's own records
/// (`purlis_core::state::Waits`, #1491): asked as a report that can end its turn is applied.
/// The second argument says the report is that turn's end, and not the nudge after one.
pub type WaitsOf = Arc<dyn Fn(u32, bool) -> purlis_core::state::Waits + Send + Sync + 'static>;

/// What is told that chat `session` is now in conversation `id`: the id its own harness
/// reported, that the board adopted or followed — and, where the move began a run (`/clear`,
/// ADR 0066), that run's id, which is the chat's current run from now on.
pub type Following = Arc<dyn Fn(u32, &str, Option<&str>) + Send + Sync + 'static>;

/// What answers an ask, told which connection it came on.
pub type Answering = Arc<dyn Fn(u64, Ask) -> Answer + Send + Sync + 'static>;

/// The host's event log, shared by every project this process holds: one writer per device
/// (ADR 0066, ADR 0068).
///
/// **Held in this process, or written in the session host** (#2003). In the window's own
/// process the window holds the log ([`Events::here`]). While chats run in the session host,
/// the host holds it and the window never opens it: each write the window decides travels to
/// the host as an [`Entry`] and is written there by the same code ([`Events::in_the_host`]),
/// and what the window's hooks would record of a line the host handed on goes back with the
/// line's reply ([`purlis_core::hookwire::relay::carry`]).
#[derive(Clone)]
pub struct Events(Writer);

#[derive(Clone)]
enum Writer {
    Here(Arc<Mutex<purlis_core::eventlog::Recorder>>),
    InTheHost(Arc<dyn Fn(Entry) -> Result<(), String> + Send + Sync + 'static>),
}

impl Events {
    /// The log `recorder` holds, written in this process: its one writer.
    pub fn here(recorder: purlis_core::eventlog::Recorder) -> Self {
        Self(Writer::Here(Arc::new(Mutex::new(recorder))))
    }

    /// The log the session host holds: each write is handed to `send`, which answers once the
    /// host has written it durably, or why it did not.
    pub fn in_the_host(send: impl Fn(Entry) -> Result<(), String> + Send + Sync + 'static) -> Self {
        Self(Writer::InTheHost(Arc::new(send)))
    }

    /// **Writes `entry` durably**, where this process holds the log or in the host that does:
    /// `Ok` once it is kept, which is when a hook is told its line is taken (FD-30). The `fsync`
    /// is outside the log's lock, so lines recorded at once share one.
    pub fn record(&self, entry: Entry) -> std::io::Result<()> {
        match &self.0 {
            Writer::Here(log) => {
                let (written, durable) = {
                    let mut log = log.lock().unwrap_or_else(PoisonError::into_inner);
                    (log.apply(entry), log.durable())
                };
                match written?.last() {
                    Some(last) => durable.through(last.seq),
                    None => Ok(()),
                }
            }
            Writer::InTheHost(send) => send(entry).map_err(std::io::Error::other),
        }
    }

    /// The log itself, where this process holds it: only its writer drains a hook spool into
    /// it, so a line and its spooled copy are told apart in one process (#952). `None` where
    /// the session host holds it.
    pub fn held(&self) -> Option<&Arc<Mutex<purlis_core::eventlog::Recorder>>> {
        match &self.0 {
            Writer::Here(log) => Some(log),
            Writer::InTheHost(_) => None,
        }
    }
}

/// Where this app listens for its sessions' hooks: one place for both binaries' hosts and the
/// window's own hearing (`purlis_core::hookchannel::socket_for`, #2447).
pub use purlis_core::hookchannel::{Where, socket_for};

impl Hooks {
    /// Nothing listening: every chat is `unknown`, which is what the spec says a harness with
    /// no state hook shows. The app runs perfectly well like this.
    pub fn deaf(plane: PlaneId) -> Self {
        let doings = crate::doing::Doings::new(plane.clone());
        let skill_runs = crate::skillrun::SkillRuns::new(plane.clone());
        Self {
            plane,
            board: Arc::new(Mutex::new(Board::new())),
            reading: Mutex::new(None),
            socket: None,
            tokens: None,
            answering: Arc::new(Mutex::new(None)),
            heard: Arc::new(Mutex::new(None)),
            following: Arc::new(Mutex::new(None)),
            all_reports: Arc::new(Mutex::new(None)),
            waits: Arc::new(Mutex::new(None)),
            saved: Arc::new(Mutex::new(None)),
            events: Arc::new(Mutex::new(None)),
            asks: Arc::new(HookAsks::new(Arc::new(Asks::new()))),
            asks_told: Arc::new(Mutex::new(None)),
            doings,
            skill_runs,
            touching: Arc::new(Mutex::new(None)),
            output: Arc::new(Mutex::new(None)),
            doing_heard: Arc::new(Mutex::new(None)),
            blocked: Arc::new(Mutex::new(None)),
            repeated: Arc::new(Mutex::new(None)),
            throttle: Arc::default(),
            hear_block: None,
            secret_exec: Arc::new(Mutex::new(None)),
            relayed: None,
        }
    }

    /// Listens on `socket`, handing each change to `moved`.
    ///
    /// A socket that cannot be opened is not worth refusing to start over: the app comes up
    /// with every chat `unknown` and says so on stderr, which is a working app with one
    /// feature missing rather than no app at all.
    pub fn listening_on(
        plane: PlaneId,
        at: &Where,
        moved: Teller,
        told_by_hand: ByHandTeller,
    ) -> std::io::Result<Self> {
        let listener = Listener::bind(&at.within, &at.socket)?;
        let socket = listener.path().to_path_buf();
        let tokens = listener.tokens();
        let (mut hooks, hearing) = Self::wired(plane, socket, moved, told_by_hand);
        hooks.tokens = Some(tokens);
        hooks.reading = Mutex::new(Some(listener.hear(hearing)));
        Ok(hooks)
    }

    /// Hears what the session host hands on from the hook channel it owns at `at` (#2000):
    /// nothing is bound here, and every line is heard as [`Self::listening_on`] hears one.
    pub fn relayed(plane: PlaneId, at: &Where, moved: Teller, told_by_hand: ByHandTeller) -> Self {
        let (mut hooks, hearing) = Self::wired(
            plane.clone(),
            at.socket.clone(),
            Arc::clone(&moved),
            told_by_hand,
        );
        hooks.relayed = Some(Arc::new(Relayed {
            heeding: Arc::new(purlis_core::hookwire::relay::Heeding::new(hearing)),
            in_order: Mutex::new(None),
            socket: at.socket.clone(),
            programs: Mutex::default(),
            board: Arc::clone(&hooks.board),
            plane,
            moved,
            waiting: Mutex::new(Some(Vec::new())),
            pool: None,
            routes: Mutex::new(None),
        }));
        hooks
    }

    /// What hears the hook channel at `socket` for `plane`, wherever its lines come from, and
    /// the board and the slots it moves, not yet hearing anything.
    fn wired(
        plane: PlaneId,
        socket: PathBuf,
        moved: Teller,
        told_by_hand: ByHandTeller,
    ) -> (Self, Hearing) {
        let board = Arc::new(Mutex::new(Board::new()));
        let answering: Arc<Mutex<Option<Answering>>> = Arc::new(Mutex::new(None));
        let heard: Arc<Mutex<Option<Heard>>> = Arc::new(Mutex::new(None));
        let following: Arc<Mutex<Option<Following>>> = Arc::new(Mutex::new(None));
        let all_reports: Arc<Mutex<Option<Heard>>> = Arc::new(Mutex::new(None));
        let waits: Arc<Mutex<Option<WaitsOf>>> = Arc::new(Mutex::new(None));
        let saved: Arc<Mutex<Option<SavedHeard>>> = Arc::new(Mutex::new(None));
        let events: Arc<Mutex<Option<Events>>> = Arc::new(Mutex::new(None));
        let asks = Arc::new(HookAsks::new(Arc::new(Asks::new())));
        let asks_told: crate::asking::Telling = Arc::new(Mutex::new(None));
        let touching: Arc<Mutex<Option<Touches>>> = Arc::new(Mutex::new(None));
        let output: Arc<Mutex<Option<Outputs>>> = Arc::new(Mutex::new(None));
        let doing_heard: Arc<Mutex<Option<Doings>>> = Arc::new(Mutex::new(None));
        let doings = crate::doing::Doings::new(plane.clone());
        let skill_runs = crate::skillrun::SkillRuns::new(plane.clone());
        let blocked: Arc<Mutex<Option<HeardBlocks>>> = Arc::new(Mutex::new(None));
        let repeated: Arc<Mutex<Option<Repeats>>> = Arc::new(Mutex::new(None));
        let secret_exec: Arc<Mutex<Option<SecretExecs>>> = Arc::new(Mutex::new(None));
        // A sandbox block (#1338), a chat's hook's or a brokered run's: handed on for the
        // network record and the chat's Notice (#1662). Taken out of the lock before it runs,
        // as an answer is. One throttle for both roads.
        let throttle: Arc<Mutex<purlis_core::sandboxblock::Throttle>> = Arc::default();
        let hear: HeardBlocks = {
            let blocked = Arc::clone(&blocked);
            let repeated = Arc::clone(&repeated);
            let throttle = Arc::clone(&throttle);
            Arc::new(move |block| {
                heard_block(
                    &throttle,
                    &blocked,
                    &repeated,
                    block,
                    std::time::Instant::now(),
                );
            })
        };
        let hear_block: Blocks = {
            let hear = Arc::clone(&hear);
            Arc::new(move |line| hear(line.into()))
        };
        // A chat's spool drained while the host runs (#952), set off by its lines here, where
        // this process holds the event log (`Events::held`).
        let spooled = {
            let dir = purlis_core::hookwire::spool::dir_for(&socket);
            let root = plane.root().to_path_buf();
            let events = Arc::clone(&events);
            watching_spools(move |chat| {
                let held = events
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .as_ref()
                    .and_then(|events| events.held().cloned());
                if let Some(log) = held {
                    drain_while_running(&dir, &root, &log, chat);
                }
            })
        };
        let hearing = Hearing {
            // A brokered `secret exec` (#1407), held on its connection while the command runs.
            // Taken out of the lock before it runs, as an answer is. A host its sandbox refused
            // is the asking chat's block, told on the same road as one its hook found.
            secret_exec: {
                let secret_exec = Arc::clone(&secret_exec);
                let told = run_told(Arc::clone(&hear));
                Box::new(move |ask, reader, writer| {
                    let runs = secret_exec
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .clone();
                    match runs {
                        Some(runs) => runs(ask, reader, writer, Arc::clone(&told)),
                        None => purlis_core::secrets::brokered::not_answered(writer),
                    }
                })
            },
            blocked: {
                let hear_block = Arc::clone(&hear_block);
                Box::new(move |block| hear_block(block))
            },
            permission: crate::asking::permitting(
                plane.clone(),
                Arc::clone(&asks),
                Arc::clone(&asks_told),
            ),
            // What a chat's tool hook says the chat is doing (#1493): kept in memory for the
            // one line under its name, and never recorded. A helper's tool is not the chat's
            // own work, so its line is left as it was.
            doing: {
                let doings = Arc::clone(&doings);
                let board = Arc::clone(&board);
                let moved = Arc::clone(&moved);
                let plane = plane.clone();
                let doing_heard = Arc::clone(&doing_heard);
                Box::new(move |said| {
                    // Activity of the chat's own run, for a task's quiet stretch (#1981).
                    if said.agent.is_none()
                        && held_board(&board).spoken_by(said.chat, &said.speaker)
                    {
                        let listener = doing_heard
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .clone();
                        if let Some(listener) = listener {
                            listener(said.chat, said.doing.clone());
                        }
                    }
                    let agent = said.agent.as_deref();
                    if let Some(what) = got_past_its_prompt(
                        &board,
                        &plane,
                        ToolLine {
                            chat: said.chat,
                            agent,
                            speaker: &said.speaker,
                        },
                        &said.doing,
                    ) {
                        moved(what);
                    }
                    if agent.is_none() {
                        doings.heard(&board, said.chat, said.doing);
                    }
                })
            },
            // What a chat's turn ended saying (#1981): handed on only where it is the run the
            // chat adopted, and never recorded. Taken out of the lock before it runs.
            output: {
                let output = Arc::clone(&output);
                let board = Arc::clone(&board);
                Box::new(move |said| {
                    if !held_board(&board).spoken_by(said.chat, &said.speaker) {
                        return;
                    }
                    let listener = output
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .clone();
                    if let Some(listener) = listener {
                        listener(said);
                    }
                })
            },
            // A file a chat's tool touched (FM-6): handed on, never recorded (D-86a). Taken out
            // of the lock before it runs, as an answer is.
            touching: {
                let touching = Arc::clone(&touching);
                Box::new(move |touched| {
                    let listener = touching
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .clone();
                    if let Some(listener) = listener {
                        listener(touched);
                    }
                })
            },
            each: {
                let board = Arc::clone(&board);
                let plane = plane.clone();
                let heard = Arc::clone(&heard);
                let following = Arc::clone(&following);
                let all_reports = Arc::clone(&all_reports);
                let waits = Arc::clone(&waits);
                let moved = Arc::clone(&moved);
                let events = Arc::clone(&events);
                let doings = Arc::clone(&doings);
                let skill_runs = Arc::clone(&skill_runs);
                let throttle = Arc::clone(&throttle);
                let repeated = Arc::clone(&repeated);
                let spooled = spooled.clone();
                Box::new(move |report| {
                    // A chat whose turn ended has gone quiet: what its sandbox refused again
                    // within the minute is told now, not at the next block the project hears.
                    if report.agent.is_none() && report.event == purlis_core::state::Event::Stop {
                        tell_repeats(&throttle, &repeated, |t| t.repeats_of(report.chat));
                    }
                    let applied = apply(&board, &plane, &report, waits_at(&waits, &report));
                    // The chat's line follows the board (#1493): a turn that began starts
                    // it, and a chat that is no longer running loses it.
                    doings.reported(&board, report.chat);
                    // The skill run on its tab ends with the turn (SK-3).
                    skill_runs.reported(&report);
                    let followed = applied.followed();
                    // The run a `/clear` begins is the host's, minted here, so the record
                    // names it whether or not this machine keeps an event log (ADR 0066).
                    let begun = (followed == purlis_core::eventlog::Followed::Moved)
                        .then(purlis_core::reopen::mint);
                    let recorded = record(
                        &events,
                        Entry::Report {
                            plane: plane.root().to_path_buf(),
                            report: report.clone(),
                            followed,
                            begun: begun.clone(),
                        },
                    );
                    // The chats' record (`following`) is told before the window is, so it
                    // already names the conversation by the time anything the move prompts
                    // could ask for it. Whether or not a reader sees a difference: after
                    // `/clear` the chat may be in the state it was in, under a conversation it
                    // was not. Where the session host writes the event log (#2003), the event
                    // above is written there once this hearing returns, after the window is
                    // told and before the hook is told its line is taken: nothing the window
                    // is told reads the event log. It is not sent ahead as `record`, because
                    // only the host thread that heard the line tells it from its spooled copy
                    // (#952, D-2003-4).
                    if let Some(id) = applied.followed {
                        let listener = following
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .clone();
                        if let Some(listener) = listener {
                            listener(report.chat, &id, begun.as_deref());
                        }
                    }
                    if let Some(what) = applied.moved {
                        moved(what);
                        // Only a report that moved the board: one for a chat it does not have,
                        // or one ADR 0024's rules refused, is heard by nobody. A chat's first
                        // start always moves it (`unknown` to `waiting`). Taken out of the
                        // lock before it runs, as an answer is.
                        let listener = heard.lock().unwrap_or_else(PoisonError::into_inner).clone();
                        if let Some(listener) = listener {
                            listener(&report);
                        }
                    }
                    let listener = all_reports
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .clone();
                    if let Some(listener) = listener {
                        listener(&report);
                    }
                    spooled.heard_from(report.chat);
                    recorded
                })
            },
            answer: {
                let answering = Arc::clone(&answering);
                let doings = Arc::clone(&doings);
                let board = Arc::clone(&board);
                Box::new(move |connection, ask| {
                    // A dispatch, a question for its asker and a report are what the chat is
                    // doing (#1493): said by their kind alone, whatever comes of the ask.
                    if let Some((chat, kind)) = crate::doing::of_ask(&ask) {
                        doings.asked(&board, chat, kind);
                    }
                    // Taken out of the lock before it runs: an open starts a program, and a
                    // program that dies at once reaches back into this plane.
                    let answer = answering
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .clone();
                    match answer {
                        Some(answer) => answer(connection, ask),
                        None => Answer::No {
                            why: NOTHING_ANSWERS.to_owned(),
                        },
                    }
                })
            },
            noticed: {
                let plane = plane.clone();
                Box::new(move |notice| {
                    if let Some(told) = by_hand(&plane, notice) {
                        told_by_hand(told);
                    }
                })
            },
            saved: {
                let saved = Arc::clone(&saved);
                Box::new(move |record| {
                    let listener = saved.lock().unwrap_or_else(PoisonError::into_inner).clone();
                    match listener {
                        Some(listener) => listener(record),
                        None => tracing::warn!(
                            "purlis: chat {} saved a session record before this project could \
                             hear it, so nothing was closed",
                            record.chat
                        ),
                    }
                })
            },
            refused: {
                // A refused commit (SQ-16): an event in the log, and a needs-you item on the
                // chat, built under the same hold as the change, as every other move is. Taken
                // only once the event is durable (FD-30).
                let board = Arc::clone(&board);
                let plane = plane.clone();
                let events = Arc::clone(&events);
                let moved = Arc::clone(&moved);
                let spooled = spooled.clone();
                Box::new(move |refused| {
                    let recorded = record(
                        &events,
                        Entry::Refused {
                            plane: plane.root().to_path_buf(),
                            chat: refused.chat,
                        },
                    );
                    let what = {
                        moving(&mut held_board(&board), &plane, refused.chat, |board| {
                            board.commit_refused(refused.chat, &refused.commit_refused)
                        })
                    };
                    if let Some(what) = what {
                        moved(what);
                    }
                    spooled.heard_from(refused.chat);
                    recorded
                })
            },
            tool: {
                let events = Arc::clone(&events);
                let board = Arc::clone(&board);
                let plane = plane.clone();
                let skill_runs = Arc::clone(&skill_runs);
                Box::new(move |call| {
                    // The skill run on the chat's tab: opened by a call that starts a skill,
                    // and counted by every call inside it (SK-3).
                    skill_runs.heard(&call);
                    let recorded = record(
                        &events,
                        Entry::Tool {
                            plane: plane.root().to_path_buf(),
                            call: call.clone(),
                        },
                    );
                    // **A chat refused an update of the project's root asks the person for
                    // it** (qw348): the guard's own verdict (arm A3c) raises a needs-you item
                    // on that chat, in the app's words, which names the window's Update. No
                    // word of the chat's is in it, and it presses nothing.
                    if call.refused_a_root_update() {
                        let what = moving(&mut held_board(&board), &plane, call.chat, |board| {
                            board.needs(call.chat, purlis_core::state::Need::RootUpdateRefused)
                        });
                        if let Some(what) = what {
                            moved(what);
                        }
                    }
                    // A child agent's tool call shows it under its chat: a Codex child's
                    // first hook is one (FD-18). Built under the same hold as the change.
                    if let Some(agent) = &call.agent {
                        let what = {
                            let mut guard = held_board(&board);
                            guard
                                .child_heard(call.chat, agent)
                                .then(|| seen_by(&guard, &plane, call.chat))
                        };
                        if let Some(what) = what {
                            moved(what);
                        }
                    }
                    spooled.heard_from(call.chat);
                    recorded
                })
            },
        };
        let hooks = Self {
            plane,
            board,
            reading: Mutex::new(None),
            socket: Some(socket),
            tokens: None,
            answering,
            heard,
            following,
            all_reports,
            waits,
            saved,
            events,
            asks,
            asks_told,
            touching,
            output,
            doing_heard,
            doings,
            skill_runs,
            blocked,
            repeated,
            throttle,
            hear_block: Some(hear_block),
            secret_exec,
            relayed: None,
        };
        (hooks, hearing)
    }

    /// Where the session host owns this project's hook channel, what hears what it hands on.
    pub fn relayed_from_the_host(&self) -> Option<Arc<Relayed>> {
        self.relayed.clone()
    }

    /// The permission asks this project's chats hold open (HP-6).
    pub fn asks(&self) -> &HookAsks {
        &self.asks
    }

    /// Tells `teller` this project's asks each time they change (HP-6).
    pub fn tell_asks_to(&self, teller: crate::asking::Teller) {
        *self
            .asks_told
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(teller);
    }

    /// What tells the window this project's asks may have moved, from any thread: a profile's
    /// sign-in watch (#2035) calls it once the harness says the profile is signed in.
    pub fn asks_teller(&self) -> impl FnOnce() + Send + 'static {
        let (plane, asks, told) = (
            self.plane.clone(),
            Arc::clone(&self.asks),
            Arc::clone(&self.asks_told),
        );
        move || crate::asking::tell(&plane, &asks, &told)
    }

    /// Answers chat `session`'s ask `ask` with `option`, as the operator in the window, on the
    /// hook that waits on it; then tells the window the asks as they now are, answered or not.
    pub fn answer(&self, session: u32, ask: &str, option: &str) -> Result<(), String> {
        let id: purlis_core::harness::asks::AskId = ask.parse()?;
        let answered = self.asks.answer(
            &session.to_string(),
            &id,
            option,
            crate::asking::the_window(),
            std::time::Instant::now(),
        );
        crate::asking::tell(&self.plane, &self.asks, &self.asks_told);
        answered.map(|_| ()).map_err(|refused| refused.to_string())
    }

    /// Records every hook call this project's channel hears into `events` from now on (FD-9).
    pub fn record_into(&self, events: Events) {
        *self.events.lock().unwrap_or_else(PoisonError::into_inner) = Some(events);
    }

    /// One audited act (`kind` with `body`) in the event log, under chat `number` where it came
    /// from one, made durable before it answers. Refused where this machine has no event log
    /// open, saying that nothing was `done` and that every `what` is recorded.
    fn audit(
        &self,
        root: &Path,
        number: Option<u32>,
        kind: &str,
        body: serde_json::Value,
        (done, what): (&str, &str),
    ) -> Result<(), String> {
        let events = self
            .events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| {
                format!(
                    "purlis's event log is not open on this machine, so nothing was {done}: \
                     every {what} is recorded"
                )
            })?;
        events
            .record(Entry::Audited {
                plane: root.to_path_buf(),
                number,
                kind: kind.to_owned(),
                body,
            })
            .map_err(|why| format!("the event log refused it ({why}), so nothing was {done}"))
    }

    /// **Writes what the window says of a browser** (#2293): who drives it, that someone typed
    /// into it, or that the person handed it to a chat, under chat `number` where it is a chat's.
    /// Refused where this machine has no event log open, or the log refused it.
    pub fn record_browser(
        &self,
        root: &Path,
        number: Option<u32>,
        said: purlis_core::eventlog::browsing::Said,
    ) -> Result<(), String> {
        let events = self
            .events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| "purlis's event log is not open on this machine".to_owned())?;
        events
            .record(Entry::Browser {
                plane: root.to_path_buf(),
                number,
                said,
            })
            .map_err(|why| why.to_string())
    }

    /// **Use skill… typed a skill into chat `number`**, unsent (SK-12): `typed` is held for its
    /// run, which opens once a prompt shows it was sent, in the event log and on the chat's tab
    /// alike, started by Use skill…. Best effort: a log that is not open, or refused the line,
    /// starts no run there and the chip still shows it, as a run is only ever a record of what
    /// the chat did.
    pub fn skill_typed(&self, root: &Path, number: u32, typed: purlis_core::skillspan::Typed) {
        self.skill_runs.typed(number, typed.clone());
        let events = self
            .events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(events) = events
            && let Err(why) = events.record(Entry::SkillTyped {
                plane: root.to_path_buf(),
                number,
                typed,
            })
        {
            tracing::warn!(
                "purlis: the skill typed into chat {number} was not held for its run: {why}"
            );
        }
    }

    /// **A curation action put `prompt` in chat `number`**, unsent (SK-20), `by` saying so, its
    /// lead digested under the chat's `salt` (SK-38): the skill runs the turn it is sent in opens
    /// are the action's, in the event log and on the chat's tab alike. Best effort, as
    /// [`Self::skill_typed`] is: a run is only ever a record of what the chat did.
    pub fn skill_prompted(
        &self,
        root: &Path,
        number: u32,
        by: purlis_core::skills::runs::StartedBy,
        prompt: &str,
        salt: &purlis_core::skillspan::PromptSalt,
    ) {
        let prompted = purlis_core::skillspan::Prompted::of(by, prompt, salt);
        self.skill_runs.prompted(number, prompted);
        let events = self
            .events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(events) = events
            && let Err(why) = events.record(Entry::SkillPrompted {
                plane: root.to_path_buf(),
                number,
                prompted,
            })
        {
            tracing::warn!(
                "purlis: the curation prompt typed into chat {number} was not held for its runs: \
                 {why}"
            );
        }
    }

    /// **Audits a sandbox grant or revoke** (#1342) in the host's event log, under chat
    /// `number` where it came from one, and made durable before it answers. Refused where this
    /// machine has no event log open: a grant nobody recorded is never made.
    pub fn record_grant(
        &self,
        root: &Path,
        number: Option<u32>,
        audited: &purlis_core::sandbox::grant::Audited<'_>,
    ) -> Result<(), String> {
        self.audit(
            root,
            number,
            audited.kind(),
            audited.body(),
            ("changed", "sandbox grant"),
        )
    }

    /// **Records a proposed change the person applied** (#1975, PC-4), under chat `number`:
    /// before the file is written, so nothing is applied unrecorded.
    pub fn record_proposed(
        &self,
        root: &Path,
        number: u32,
        applied: &purlis_core::proposed::Applied<'_>,
    ) -> Result<(), String> {
        self.audit(
            root,
            Some(number),
            purlis_core::proposed::APPLIED_KIND,
            applied.body(),
            ("written", "applied change"),
        )
    }

    /// [`Hooks::record_dispatch_grant`] for one the person made on a refusal kept while nobody
    /// was there (#1507): the same event with no chat, whose body says `from: "away"`.
    pub fn record_dispatch_grant_from_away(
        &self,
        root: &Path,
        audited: &purlis_core::dispatchgrant::Audited<'_>,
    ) -> Result<(), String> {
        self.audit(
            root,
            None,
            audited.kind(),
            purlis_core::eventlog::dispatch_grant_body(
                audited,
                Some(purlis_core::dispatchaway::AUDITED_FROM),
            ),
            ("changed", "dispatch grant"),
        )
    }

    /// **Audits a dispatch grant or revoke** (#1437) in the host's event log, under chat
    /// `number` where it was allowed from one, and made durable before it answers. Refused
    /// where this machine has no event log open: a grant nobody recorded is never made.
    pub fn record_dispatch_grant(
        &self,
        root: &Path,
        number: Option<u32>,
        audited: &purlis_core::dispatchgrant::Audited<'_>,
    ) -> Result<(), String> {
        self.audit(
            root,
            number,
            audited.kind(),
            purlis_core::eventlog::dispatch_grant_body(audited, None),
            ("changed", "dispatch grant"),
        )
    }

    /// Drains this project's hook spool into the event log (FD-30, ADR 0068 §6): the lines its
    /// chats' hooks spooled while no host took them, each checked, and every gap and rejected
    /// line recorded as such.
    ///
    /// **Before any chat starts**, as a project is opened: a chat the reopen record names is
    /// told to the log first, so a line it spooled is recorded under the run it ran in. With
    /// no channel nothing is done. With no event log nothing is drained, and the spool waits
    /// for a host that has one.
    ///
    /// **A chat the reopen record does not bring back has ended** (V99i): once the drain has
    /// finished, its spool key is dropped. The chats it does bring back keep theirs, with what
    /// was drained under each, so a hook of theirs that is still about loses no line. With no
    /// event log an ended chat's key is dropped all the same, unless its spool still holds a
    /// line (`spool::forget_ended_at_open`, #983), so keys do not pile up launch after launch.
    ///
    /// **While chats run in the session host, the host drains it** (#2003): the host owns the
    /// socket and writes the log, so this does nothing.
    pub fn drain_spool(&self) {
        let Some(socket) = &self.socket else { return };
        let events = self
            .events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        match events.as_ref().map(Events::held) {
            Some(None) => {}
            held => drain_at_open(socket, self.plane.root(), held.flatten().map(Arc::as_ref)),
        }
    }

    /// Chat `chat` has ended, by its close or by another start taking its place: what its hooks
    /// spooled while this host was busy is drained into the event log, and then its spool keys
    /// are dropped, so no key outlives its chat (V99i, ADR 0068 §6).
    ///
    /// With no event log there is nowhere to record a line, and the keys are dropped all the
    /// same. A drain the log refuses leaves the keys and the lines for the next open of the
    /// project.
    ///
    /// **The event log is held for one item at a time**, as a live line holds it, and not at
    /// all for a chat that spooled nothing: every other chat's hooks go on being recorded
    /// while this one's spool is drained, where a hold across the drain would have each of
    /// them wait and, past 250 ms, spool.
    pub fn chat_ended(&self, chat: u32) {
        // What its sandbox refused again within the minute is told as it ends.
        tell_repeats(&self.throttle, &self.repeated, |t| t.repeats_of(chat));
        let Some(socket) = &self.socket else { return };
        let events = self
            .events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        match events.as_ref().map(Events::held) {
            // The session host drains it as the chat is closed there (#2003).
            Some(None) => {}
            held => spool_ended(
                socket,
                self.plane.root(),
                held.flatten().map(Arc::as_ref),
                chat,
            ),
        }
    }

    /// Stops listening and releases the socket. The plane on disk is untouched.
    ///
    /// Dropping the reader is what unlinks the socket file and joins the thread, and this is
    /// where a closed plane does it — so a plane that is opened again binds a socket of its
    /// own rather than inheriting a live one's path.
    pub fn stop(&self) {
        // Every repeat its chats' sandboxes were refused and the throttle held back is told
        // before the project is let go of, at a close or a quit.
        tell_repeats(&self.throttle, &self.repeated, |t| t.every_repeat());
        drop(
            self.reading
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take(),
        );
    }

    /// Whether this is still listening. Only a test asks.
    #[cfg(test)]
    pub fn listening(&self) -> bool {
        self.reading
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    /// Who answers asks on this socket from now on.
    pub fn answer_with(&self, answering: Answering) {
        *self
            .answering
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(answering);
    }

    /// Who is told each report the board takes from now on.
    pub fn when_heard(&self, heard: Heard) {
        *self.heard.lock().unwrap_or_else(PoisonError::into_inner) = Some(heard);
    }

    /// Who says, from now on, what a chat waits on as the end of its turn is applied (#1491).
    pub fn waits_by(&self, waits: WaitsOf) {
        *self.waits.lock().unwrap_or_else(PoisonError::into_inner) = Some(waits);
    }

    /// Applies `report` as the socket's listener does, with what its chat waits on read first,
    /// and answers what the window must now be told. For a test that stands in for a chat's
    /// hook without a socket: the listener's own path, less the event log and who listens.
    #[cfg(test)]
    pub(crate) fn hear(&self, report: &Report) -> Option<Moved> {
        apply(
            &self.board,
            &self.plane,
            report,
            waits_at(&self.waits, report),
        )
        .moved
    }

    /// Who is told every report on this socket from now on, after the board has had it, whether
    /// or not it moved anything.
    pub fn when_reported(&self, heard: Heard) {
        *self
            .all_reports
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(heard);
    }

    /// Who is told, from now on, each path a chat's file tool touched (FM-6), as the chat said
    /// it: the plane confines it to the chat's folder before the window hears of it.
    pub fn when_touching(&self, touches: Touches) {
        *self.touching.lock().unwrap_or_else(PoisonError::into_inner) = Some(touches);
    }

    /// Who is told what a chat's own tool hook says it is doing (#1981), once the board has
    /// judged the line its own run's.
    pub fn when_doing(&self, doings: Doings) {
        *self
            .doing_heard
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(doings);
    }

    /// Who is told what a chat's turn put out (#1981), once the board has judged the line its
    /// own run's.
    pub fn when_output(&self, outputs: Outputs) {
        *self.output.lock().unwrap_or_else(PoisonError::into_inner) = Some(outputs);
    }

    /// What this project's working chats are doing (#1493).
    pub fn doings(&self) -> &Arc<crate::doing::Doings> {
        &self.doings
    }

    /// What each chat the board has running is doing now, for a window that has just opened.
    pub fn doing_now(&self) -> Vec<crate::doing::ChatDoing> {
        self.doings.now(&self.board)
    }

    /// The skill run open in each chat (SK-3).
    pub fn skill_runs(&self) -> &Arc<crate::skillrun::SkillRuns> {
        &self.skill_runs
    }

    /// Who is told, from now on, the repeats of each block the throttle held back, once their
    /// minute is over (#1681): for the network record alone.
    pub fn when_repeated(&self, repeats: Repeats) {
        *self.repeated.lock().unwrap_or_else(PoisonError::into_inner) = Some(repeats);
    }

    /// Who is told, from now on, each sandbox block a chat's hook found (#1338), once it is kept.
    pub fn when_blocked(&self, blocks: HeardBlocks) {
        *self.blocked.lock().unwrap_or_else(PoisonError::into_inner) = Some(blocks);
    }

    /// What hears a sandbox block from outside a chat's own hook (#1663): purlis's own proxy
    /// beside a chat it wraps tells each host it refused here, and it takes the road a hook's
    /// block takes. None with nothing listening.
    pub fn block_hearer(&self) -> Option<Blocks> {
        self.hear_block.clone()
    }

    /// Who runs, from now on, each brokered `secret exec` a chat asks for (#1407).
    pub fn exec_secrets_with(&self, runs: SecretExecs) {
        *self
            .secret_exec
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(runs);
    }

    /// Who is told, from now on, each session record a chat says it saved (ADR 0064).
    pub fn when_saved(&self, saved: SavedHeard) {
        *self.saved.lock().unwrap_or_else(PoisonError::into_inner) = Some(saved);
    }

    /// Who is told, from now on, each time a chat's own harness moves it onto another
    /// conversation.
    ///
    /// **Only what the board took.** It is told the conversation [`Board::conversation`]
    /// answers after a report, and only when that changed — so a report ADR 0024's rules
    /// refused (a nested harness's, [`Conversation::Contradicted`], a
    /// [`Conversation::Foreign`] after adoption) is told to nobody, because it moved nothing.
    ///
    /// [`Conversation::Contradicted`]: purlis_core::hookwire::Conversation::Contradicted
    /// [`Conversation::Foreign`]: purlis_core::hookwire::Conversation::Foreign
    pub fn when_it_follows(&self, following: Following) {
        *self
            .following
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(following);
    }

    /// Where this listens. Only a test asks: a chat is told through [`Hooks::reporting`].
    #[cfg(test)]
    pub fn socket(&self) -> Option<&Path> {
        self.socket.as_deref()
    }

    /// Whether the asker on `connection` hung up while its ask was being answered
    /// (`ChatTokens::asker_gone`): what ends a wait nobody is waiting for any more (#1441).
    pub fn asker_gone(&self, connection: u64) -> bool {
        if let Some(relayed) = &self.relayed {
            return relayed.heeding.asker_gone(connection);
        }
        self.tokens
            .as_ref()
            .is_some_and(|tokens| tokens.asker_gone(connection))
    }

    /// The program chat `chat`'s lines are read from now, where one is recorded (qw381).
    pub fn program_of(&self, chat: u32) -> Option<purlis_core::hookwire::Program> {
        if let Some(relayed) = &self.relayed {
            return relayed
                .programs
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&chat)
                .cloned();
        }
        self.tokens.as_ref()?.program_of(chat)
    }

    /// What a chat started in this plane is told to report to: the socket and the tokens it
    /// checks, or nothing when this is not listening.
    pub fn reporting(&self) -> Option<crate::sessions::Reporting> {
        Some(crate::sessions::Reporting {
            socket: self.socket.clone()?,
            tokens: Arc::clone(self.tokens.as_ref()?),
        })
    }

    /// A fresh token for chat `chat`, as the app issues one when the chat starts: what a test
    /// standing in for that chat's hook sends with.
    #[cfg(test)]
    pub fn token_for(&self, chat: u32) -> purlis_core::hookwire::ChatToken {
        self.tokens
            .as_ref()
            .expect("the plane is listening")
            .issue_to_this_process(chat)
            .expect("a token")
    }

    pub fn board(&self) -> MutexGuard<'_, Board> {
        held_board(&self.board)
    }

    /// The board itself, for a caller that has to outlive this handle — the exit reporting,
    /// which is armed before the app manages anything.
    pub fn shared_board(&self) -> Arc<Mutex<Board>> {
        Arc::clone(&self.board)
    }
}

/// The in-process host's board. Each call is documented once, on [`ChatBoard`].
impl ChatBoard for Hooks {
    fn glance(&self, session: u32) -> Glance {
        let board = self.board();
        Glance {
            state: board.state(session),
            asking: board.asking(session),
            turns: board.turns(session),
            prompt: board.its_prompt(session),
        }
    }

    fn conversation(&self, session: u32) -> Option<String> {
        self.board().conversation(session).map(str::to_owned)
    }

    fn now(&self, session: u32) -> Moved {
        now(&self.board, self.plane.clone(), session)
    }

    fn closed(&self, session: u32) -> Moved {
        let moved = {
            let mut board = self.board();
            board.closed(session);
            seen_by(&board, &self.plane, session)
        };
        // And what it was doing goes with it (#1493): the window is told its line is gone,
        // once the board is let go, as every telling is.
        self.doings.closed(session);
        self.skill_runs.closed(session);
        moved
    }

    fn ignored(&self, session: u32) -> Moved {
        let mut board = self.board();
        board.ignored(session);
        seen_by(&board, &self.plane, session)
    }

    fn reported_back(&self, session: u32, from: &str) -> Option<Moved> {
        let mut board = self.board();
        board
            .reported_back(session, from)
            .then(|| seen_by(&board, &self.plane, session))
    }

    fn stopped_below(&self, session: u32, from: &str) -> Option<Moved> {
        let mut board = self.board();
        board
            .stopped_below(session, from)
            .then(|| seen_by(&board, &self.plane, session))
    }

    fn needs(&self, session: u32, need: purlis_core::state::Need) -> Option<Moved> {
        moving(&mut self.board(), &self.plane, session, |board| {
            board.needs(session, need)
        })
    }

    fn commit_refused(&self, session: u32, what: &str) -> Option<Moved> {
        // An event in the log, as a chat's own refused commit records one. The item is raised
        // whether or not the log could be written: the commit was refused either way.
        if let Err(e) = record(
            &self.events,
            Entry::Refused {
                plane: self.plane.root().to_path_buf(),
                chat: session,
            },
        ) {
            tracing::warn!("purlis: a refused commit of chat {session} was not logged: {e}");
        }
        moving(&mut self.board(), &self.plane, session, |board| {
            board.commit_refused(session, what)
        })
    }

    fn reported_to_its_asker(&self, session: u32) {
        self.board().reported_to_its_asker(session);
    }

    fn rested(&self, session: u32) -> Option<Moved> {
        moving(&mut self.board(), &self.plane, session, |board| {
            board.rested(session)
        })
    }

    fn task_failed(&self, session: u32, failed: purlis_core::state::FailedTask) -> Option<Moved> {
        moving(&mut self.board(), &self.plane, session, |board| {
            board.task_failed(session, failed)
        })
    }

    fn failure_cleared(&self, session: u32, id: &str) -> Option<Moved> {
        moving(&mut self.board(), &self.plane, session, |board| {
            board.failure_cleared(session, id)
        })
    }

    fn answered_by(
        &self,
        session: u32,
        asker: &purlis_core::harness::hooked::Asker,
    ) -> Option<Moved> {
        moving(&mut self.board(), &self.plane, session, |board| {
            board.answered_by(session, asker)
        })
    }
}

/// **A tool of chat `chat`'s own, heard while it was stopped on its prompt** (#1601): one that
/// came back after another began past the prompt says the person answered it in the chat's
/// pane, which no hook says, and its turn goes on, as for an answer in the window
/// (`Board::answered`). What the window is told, or nothing for a chat that waited on no prompt
/// or a line that does not say so (`purlis_core::state::Chat::tool_said`). Before the chat's
/// line is told, so the line is drawn for a chat the board has running again.
///
/// A tool of helper `agent` says the same of that helper's own prompt only (#1644,
/// `purlis_core::state::Chat::child_tool_said`). Either is believed only from the harness run
/// the chat adopted (#1601, `purlis_core::state::Board::tool_said_by`): a line from a harness
/// nested in the chat, or from a job it started, moves nothing.
fn got_past_its_prompt(
    board: &Mutex<Board>,
    plane: &PlaneId,
    tool: ToolLine<'_>,
    said: &purlis_core::doing::Said,
) -> Option<Moved> {
    if !said.goes_on_past_a_prompt() && !said.starts_a_tool_of_its_own() {
        return None;
    }
    let ToolLine {
        chat,
        agent,
        speaker,
    } = tool;
    let mut board = held_board(board);
    moving(&mut board, plane, chat, |board| match agent {
        Some(agent) => board.child_tool_said_by(chat, agent, speaker, said),
        None => board.tool_said_by(chat, speaker, said),
    })
}

/// Whose tool line it is: the chat's, the helper of it that ran the tool (`None` for the chat's
/// own), and the harness run the hook ran under.
#[derive(Clone, Copy)]
struct ToolLine<'a> {
    chat: u32,
    agent: Option<&'a str>,
    speaker: &'a purlis_core::hookwire::Speaker,
}

/// Makes one move of chat `session` on the board, and answers what the window must now be
/// told where a reader would see a difference: [`seen_by`], with whether the move raised
/// something for the person (`Moved::raised`), read off where the chat stood before and
/// after. One hold from the read before to the snapshot, as every move's is.
fn moving(
    board: &mut Board,
    plane: &PlaneId,
    session: u32,
    act: impl FnOnce(&mut Board) -> bool,
) -> Option<Moved> {
    let was = board.standing(session);
    if !act(board) {
        return None;
    }
    let mut moved = seen_by(board, plane, session);
    moved.raised = purlis_core::state::raised(was, board.standing(session));
    Some(moved)
}

/// What the chat `report` names waits on, asked only of a report that can end its turn or
/// nudge it after one: a `Stop` or a `Notification` of the chat's own. Every other report
/// raises no needs-you item this could hold back, so the project's records are not read for
/// it. **Asked before the board is taken**, never under it: the answer reads the chats, the
/// board and the tasks' ledger in turn.
fn waits_at(waits: &Mutex<Option<WaitsOf>>, report: &Report) -> purlis_core::state::Waits {
    use purlis_core::state::Event;
    if report.agent.is_some() || !matches!(report.event, Event::Stop | Event::Notification) {
        return purlis_core::state::Waits::default();
    }
    let asked = waits.lock().unwrap_or_else(PoisonError::into_inner).clone();
    asked.map_or_else(purlis_core::state::Waits::default, |asked| {
        asked(report.chat, report.event == Event::Stop)
    })
}

/// What a reader sees for this chat right now.
pub fn now(board: &Mutex<Board>, plane: PlaneId, session: u32) -> Moved {
    seen_by(&held_board(board), &plane, session)
}

/// The same, for a caller that is already holding the board.
///
/// **Only ever called with the board held**, which is what makes [`sequence`] an order over
/// the board's states: the number is taken inside the same hold as the read.
fn seen_by(board: &Board, plane: &PlaneId, session: u32) -> Moved {
    let queue = board.needs_you();
    Moved {
        sequence: sequence(),
        plane: plane.clone(),
        session,
        state: word(board.state(session)),
        needs_you: queue.contains(&session),
        queue,
        moved_at: board.moved_at(session),
        reports: board.reports(session),
        refusals: board.refusals(session),
        stopped: Some(board.stopped_of(session)).filter(|stopped| !stopped.is_empty()),
        // What the app found, then the tasks of its that came to nothing (#1491): the item
        // says the latest, and a failure is the one the person has not been told of.
        needs: Some(
            board
                .needs_of(session)
                .into_iter()
                .map(Need::from)
                .chain(board.failed_tasks(session).into_iter().map(Need::from))
                .collect::<Vec<Need>>(),
        )
        .filter(|needs| !needs.is_empty()),
        asking: board.waits_on_its_prompt(session).then_some(true),
        counts_only: false,
        raised: false,
        children: board
            .children(session)
            .into_iter()
            .map(|child| ChildAgent {
                agent: child.agent,
                state: word(child.state),
            })
            .collect(),
    }
}

/// The next snapshot's number. See [`Moved::sequence`].
///
/// **The process's and not one board's**, which is stronger than the window needs and costs
/// nothing: a project closed and opened again in one run gets a new board, and a count that
/// began again at one would have a window still holding the old board's numbers drop every
/// snapshot of the new one. Taken while a board is held, so for any one board the numbers
/// run in the order its states were read.
///
/// Saturating, and a `u32` for the reason [`Board::moved_at`] gives (`specta` cannot carry a
/// `u64`). At four billion snapshots every later one is numbered the same, and the window
/// keeps a snapshot numbered the same as the one it holds — so the app falls back to taking
/// events in the order they land, which is what it did before there was a number, rather than
/// stopping listening.
fn sequence() -> u32 {
    static TAKEN: AtomicU32 = AtomicU32::new(0);
    let was = TAKEN
        .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
            Some(n.saturating_add(1))
        })
        .unwrap_or_else(|n| n);
    was.saturating_add(1)
}

/// What one report did.
struct Applied {
    /// What a reader would now see differently, or nothing when no reader would.
    moved: Option<Moved>,
    /// The conversation the chat is in now, where the report moved it onto one: the first id
    /// a Codex or opencode chat names, or the one a Claude Code chat's own process moved to on
    /// `/clear` (C6). Nothing for a report that left the chat where it was.
    followed: Option<String>,
    /// The conversation the chat was in before the report, where it was in one: with
    /// `followed`, what tells a harness naming its conversation from `/clear` (ADR 0066).
    was: Option<String>,
}

impl Applied {
    /// What the board did with the chat's conversation, as the event log reads a new run.
    fn followed(&self) -> purlis_core::eventlog::Followed {
        use purlis_core::eventlog::Followed;
        match (&self.followed, &self.was) {
            (None, _) => Followed::No,
            (Some(_), None) => Followed::FirstNamed,
            (Some(_), Some(_)) => Followed::Moved,
        }
    }
}

/// Applies one report, answering with what a reader would now see differently and which
/// conversation, if a new one, the chat is now in.
///
/// The answer is built under the SAME hold as the change. Reports arrive on a thread each, so
/// dropping the lock in between let two of them interleave — mutate, mutate, read, read — and
/// the window could then be sent the older of the two snapshots last and keep it until the
/// next event. A review found it.
///
/// **Which conversation is the board's answer, read before and after**, and not the report's:
/// [`Board::reported`] is the one place that decides whether a report is the chat's own
/// harness speaking (ADR 0024), and a second reading of the report here would be a second
/// answer to that question.
///
/// `waits` is what the chat waits on that is not the person ([`waits_at`], #1491), read
/// before the board was taken.
fn apply(
    board: &Mutex<Board>,
    plane: &PlaneId,
    report: &Report,
    waits: purlis_core::state::Waits,
) -> Applied {
    let mut guard = held_board(board);
    let was = guard.conversation(report.chat).map(str::to_owned);
    let moved = moving(&mut guard, plane, report.chat, |board| {
        board.reported_while(report, waits)
    });
    let now = guard.conversation(report.chat);
    let followed = now
        .filter(|now| was.as_deref() != Some(*now))
        .map(str::to_owned);
    Applied {
        moved,
        followed,
        was,
    }
}

/// Writes one event into the host's log, when there is one, and answers once it is durable:
/// the hook is told its line is taken only on `Ok`, and answers its harness after that (ADR
/// 0075 §7, FD-30). The `fsync` is outside the log's lock, so hooks recorded at once share one.
///
/// A write or a sync that fails is said in the app's log and is the error: the hook is not
/// told its line was taken, so it spools the line and the next open records it. With no event
/// log there is nothing to wait for, and that is `Ok`. A line whose spooled copy the drain
/// recorded first is not written again (#952, [`purlis_core::hookwire::recorded_already`]):
/// that too is `Ok`.
///
/// **A line the session host handed on is recorded there** (#2003): `entry` goes back with the
/// line's reply ([`purlis_core::hookwire::relay::carry`]), and the host, which writes the log,
/// writes it on the thread that heard the line.
fn record(events: &Mutex<Option<Events>>, entry: Entry) -> std::io::Result<()> {
    if purlis_core::hookwire::recorded_already() {
        return Ok(());
    }
    let Some(entry) = purlis_core::hookwire::relay::carry(entry) else {
        return Ok(());
    };
    let Some(held) = events
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
    else {
        return Ok(());
    };
    held.record(entry).inspect_err(|why| {
        tracing::warn!("purlis: a hook call was not recorded durably ({why})");
    })
}

/// The drains of a hook spool, one for the window's own hearing and both binaries' hosts
/// (`purlis_core::hookchannel`, #2447).
pub(crate) use purlis_core::hookchannel::{
    SpoolWatch, drain_at_open, drain_while_running, spool_ended,
};

/// How long after one of a chat's lines is heard its spool is looked at (#952): the host's
/// ([`purlis_core::hookchannel::A_SPOOL_IS_LOOKED_AT_AFTER`]), short under test, so a test waits
/// on the look, not on the clock.
pub(crate) const A_SPOOL_IS_LOOKED_AT_AFTER: std::time::Duration = if cfg!(test) {
    std::time::Duration::from_millis(20)
} else {
    purlis_core::hookchannel::A_SPOOL_IS_LOOKED_AT_AFTER
};

/// **A chat's spool is drained while the window hears its hooks** (#952), looked at
/// [`A_SPOOL_IS_LOOKED_AT_AFTER`] after one of its lines is heard
/// ([`purlis_core::hookchannel::watching_spools`]).
pub(crate) fn watching_spools(drain: impl Fn(u32) + Send + 'static) -> SpoolWatch {
    purlis_core::hookchannel::watching_spools(A_SPOOL_IS_LOOKED_AT_AFTER, drain)
}

/// The board, whether or not a thread panicked while holding it.
///
/// A poisoned board is one whose last change may not have finished; the state it holds is
/// still the best answer there is, and refusing to draw anything at all would be worse.
pub fn held_board(board: &Mutex<Board>) -> MutexGuard<'_, Board> {
    board
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The word the window draws, which is the word the spec uses.
pub fn word(state: State) -> String {
    match state {
        State::Unknown => "unknown",
        State::Running => "running",
        State::Waiting => "waiting",
        State::Done => "done",
        State::Failed => "failed",
    }
    .to_owned()
}

/// What an exit says about a chat: the code, or none for a program killed by a signal.
pub fn code_of(exit: &Exit) -> Option<i32> {
    match exit {
        Exit::Code(code) => i32::try_from(*code).ok(),
        // A signal leaves no code behind, and it is not a clean end.
        Exit::Signal(_) => None,
    }
}

#[cfg(test)]
mod tests;
