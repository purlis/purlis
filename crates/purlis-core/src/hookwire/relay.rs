// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The hook channel, heard in the session host and answered in the window** (#2000).
//!
//! With chats in the session host, the host owns each held project's hook socket and its chats'
//! tokens: it binds the socket, issues each chat its token as it starts the chat, records the
//! chat's program, and admits every line exactly as the window did, with the same
//! [`super::Listener`] and the same checks (the token, the process that sent it, each framed ask
//! again). The host is the chat's ancestor, so a hook that asks for a permission admits it
//! (`purlis_same_user::admit_host`), as it admitted the window.
//!
//! What a line moves (the board, Needs you, the asks, a dispatch) is still the window's to
//! decide, so the host hands each admitted line on to the window ([`Relaying`]), and the window
//! hears it with the very [`Hearing`] it listened with before ([`Heeding`]); the reply comes
//! back the same way and is written on the hook's connection. A line carries no token past the
//! listener ([`super::read_delivered`] takes it off), so none crosses to the window.
//!
//! **The host writes the event log; the window decides what a line moves** (#2003). A report, a
//! tool call and a refused commit are recorded in the event log, which the host alone writes
//! while chats run in it. The window hears each with its own hearing, and what that hearing
//! would have written (with what the board made of the line) is carried back with its reply
//! ([`carry`], [`Replied::Record`]); the host writes it on the thread that heard the line, so a
//! line its hook also spooled, and the host's drain recorded first, is recorded once (#952).
//!
//! **Each kind keeps its shape.** A report, a tool call and a refused commit are answered once
//! the window has taken them and the host has recorded them durably, or not, within a hook's
//! wait ([`A_RELAYED_LINE_IS_TAKEN_WITHIN`]);
//! an ask waits for its answer as long as the window takes, and the window is told when its
//! asker hangs up ([`Heard::AskerGone`]); a permission ask is held in the window until it is
//! answered there, its hook goes away or its deadline passes; a brokered `secret exec` runs in
//! the window, its asker's input handed one way and its frames the other. A line nobody is there
//! to hear, because no window holds the project, is answered as an app with nothing to answer
//! it would answer.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::{Duration, Instant};

use super::permission::{Left, PermissionAsked, Waiting};
use super::{
    Answer, Ask, ChatTokens, CommitRefused, Doing, Hearing, Report, SandboxBlocked, SessionSaved,
    StartedByHand, ToolCall, Touching, TurnOutput,
};
use crate::eventlog::Entry;
use crate::harness::model::Choice;
use crate::secrets::brokered;

/// One line the host's hook channel admitted, as it is handed on to the window, or word of what
/// became of one handed on before.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Heard {
    /// A [`Report`]: replied [`Replied::Taken`] once the window has recorded it.
    Report(Report),
    /// An [`Ask`], on the hook connection numbered `connection`: replied [`Replied::Answer`].
    Ask { connection: u64, ask: Ask },
    /// A [`StartedByHand`]. Nothing is replied.
    StartedByHand(StartedByHand),
    /// A [`SessionSaved`]. Nothing is replied.
    Saved(SessionSaved),
    /// A [`CommitRefused`]: replied as a report is.
    Refused(CommitRefused),
    /// A [`ToolCall`]: replied as a report is.
    Tool(ToolCall),
    /// A [`Touching`]. Nothing is replied.
    Touching(Touching),
    /// A [`SandboxBlocked`]. Nothing is replied.
    Blocked(SandboxBlocked),
    /// A [`Doing`]. Nothing is replied.
    Doing(Doing),
    /// A [`TurnOutput`]. Nothing is replied.
    Output(TurnOutput),
    /// A [`PermissionAsked`]: replied [`Replied::Held`] or [`Replied::NotHeld`], and once
    /// held, [`Replied::Chosen`] or [`Replied::Ended`].
    Permission(PermissionAsked),
    /// The permission ask handed on as `of` ended in the host unanswered: its hook went away
    /// (`closed`) or its deadline passed.
    Left { of: u64, closed: bool },
    /// The asker on hook connection `connection` hung up while its ask was being answered.
    AskerGone { connection: u64 },
    /// A brokered `secret exec`: replied [`Replied::Wrote`] for what the command's frames say,
    /// then [`Replied::Ended`].
    SecretExec(brokered::Ask),
    /// One line of the asker's input to the `secret exec` handed on as `of`.
    Input { of: u64, line: String },
    /// The asker's input to the `secret exec` handed on as `of` has ended.
    InputEnded { of: u64 },
}

/// What the window replies to a line handed on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Replied {
    /// A report, a tool call or a refused commit is taken, with nothing for the host to record.
    Taken,
    /// A report, a tool call or a refused commit is taken, and this is what the host records of
    /// it in the event log (#2003): what the window's hearing would have written itself.
    Record(Entry),
    /// It is not, for `why`: its hook spools it where it may.
    NotTaken { why: String },
    /// An ask's answer.
    Answer(Answer),
    /// A permission ask is held, for at most `until_ms`.
    Held { until_ms: u64 },
    /// A permission ask is not held: the harness's own prompt decides.
    NotHeld,
    /// A held permission ask was answered with this option.
    Chosen(Choice),
    /// What a `secret exec`'s command wrote to its asker, as it wrote it.
    Wrote { text: String },
    /// A held permission ask will not be answered, or a `secret exec` has ended.
    Ended,
}

/// How long the host waits for the window to take a report, a tool call or a refused commit,
/// and to say whether it holds a permission ask: inside a hook's own wait for its answer, so a
/// window that does not answer in time leaves the hook time to spool its line.
pub const A_RELAYED_LINE_IS_TAKEN_WITHIN: Duration = Duration::from_millis(1500);

/// How often an ask's asker is looked at for having hung up while the window answers it.
const AN_ASKER_IS_LOOKED_AT_EVERY: Duration = Duration::from_millis(100);

/// What a line nobody is there to hear is answered with.
pub const NOBODY_HEARS: &str = "purlis's window is not there to take this";

/// What hands a line on to the window, numbered: `false` when nothing holds the project to hear
/// it.
pub type HandOn = Box<dyn Fn(u64, &Heard) -> bool + Send + Sync + 'static>;

/// **The host's half**: hands every line its hook channel admits on to the window, and gives
/// the hook what the window replied ([`Relaying::hearing`]).
pub struct Relaying {
    hand_on: HandOn,
    next: AtomicU64,
    waiting: Mutex<HashMap<u64, Awaiting>>,
    /// Where what the window carries back is recorded, where this host keeps the event log.
    keeping: Option<Keeping>,
}

/// **How the host keeps what its hook channel hears** (#2003): it writes the event log, and
/// looks at a chat's hook spool soon after one of its lines comes, as the window did.
pub struct Keeping {
    /// Writes one entry in the event log, durably: the hook is told its line is taken only once
    /// this answers `Ok`.
    pub record: Box<dyn Fn(Entry) -> io::Result<()> + Send + Sync + 'static>,
    /// Chat `chat` was heard from: its spool is looked at soon after.
    pub heard_from: Box<dyn Fn(u32) + Send + Sync + 'static>,
}

/// What a line handed on waits for.
enum Awaiting {
    /// Its replies.
    Replies(mpsc::Sender<Replied>),
    /// A held permission ask's option.
    Choice(mpsc::Sender<Choice>),
}

impl Relaying {
    /// Hands each line on through `hand_on`.
    pub fn new(hand_on: HandOn) -> Arc<Relaying> {
        Self::built(hand_on, None)
    }

    /// Hands each line on through `hand_on`, and records what the window carries back of each
    /// in the event log through `keeping` (#2003).
    pub fn keeping(hand_on: HandOn, keeping: Keeping) -> Arc<Relaying> {
        Self::built(hand_on, Some(keeping))
    }

    fn built(hand_on: HandOn, keeping: Option<Keeping>) -> Arc<Relaying> {
        Arc::new(Relaying {
            hand_on,
            next: AtomicU64::new(1),
            waiting: Mutex::default(),
            keeping,
        })
    }

    /// The window replied `replied` to the line handed on as `id`. A reply to a line nothing
    /// waits for any more is dropped.
    pub fn replied(&self, id: u64, replied: Replied) {
        let mut waiting = self.waiting();
        match waiting.get(&id) {
            Some(Awaiting::Replies(replies)) => {
                if replies.send(replied).is_err() {
                    waiting.remove(&id);
                }
            }
            // A held ask has one more reply: its option, or that none will come.
            Some(Awaiting::Choice(chosen)) => {
                if let Replied::Chosen(choice) = replied {
                    let _ = chosen.send(choice);
                }
                waiting.remove(&id);
            }
            None => {}
        }
    }

    /// Nothing replies from now on: every line that waits is let go of, and answered as a line
    /// nobody heard.
    pub fn let_go(&self) {
        self.waiting().clear();
    }

    fn waiting(&self) -> MutexGuard<'_, HashMap<u64, Awaiting>> {
        self.waiting.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn fresh(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }

    /// Hands `heard` on, expecting no reply.
    fn tell(&self, heard: &Heard) {
        let id = self.fresh();
        (self.hand_on)(id, heard);
    }

    /// Hands `heard` on, and where something heard it, its number and where its replies come.
    fn exchange(&self, heard: &Heard) -> Option<(u64, mpsc::Receiver<Replied>)> {
        let id = self.fresh();
        let (replies, replied) = mpsc::channel();
        self.waiting().insert(id, Awaiting::Replies(replies));
        if (self.hand_on)(id, heard) {
            Some((id, replied))
        } else {
            self.done(id);
            None
        }
    }

    /// Line `id` waits for nothing more.
    fn done(&self, id: u64) {
        self.waiting().remove(&id);
    }

    /// Hands chat `chat`'s line `heard` on and waits for the window to say it took it, and
    /// records what the window carried back of it.
    fn taken(&self, chat: u32, heard: &Heard) -> io::Result<()> {
        if let Some(keeping) = &self.keeping {
            (keeping.heard_from)(chat);
        }
        let Some((id, replied)) = self.exchange(heard) else {
            return Err(io::Error::other(NOBODY_HEARS));
        };
        let reply = replied.recv_timeout(A_RELAYED_LINE_IS_TAKEN_WITHIN);
        self.done(id);
        match reply {
            Ok(Replied::Taken) => Ok(()),
            Ok(Replied::Record(entry)) => self.record(entry),
            Ok(Replied::NotTaken { why }) => Err(io::Error::other(why)),
            Ok(other) => Err(io::Error::other(format!(
                "the window replied {other:?} to a line it was to take"
            ))),
            Err(_) => Err(io::Error::other("the window did not take it in time")),
        }
    }

    /// Records `entry`, what the window carried back of the line this thread is hearing: not
    /// where its spooled copy was recorded first (#952), and as nothing where this host keeps
    /// no event log, which a hook is told is taken as an app with no log tells it.
    fn record(&self, entry: Entry) -> io::Result<()> {
        if super::recorded_already() {
            return Ok(());
        }
        match &self.keeping {
            Some(keeping) => (keeping.record)(entry),
            None => Ok(()),
        }
    }

    /// Hands an ask on, and waits as long as the window takes to answer it. While it waits it
    /// looks at the asker, and tells the window once it has hung up.
    fn answer(&self, tokens: &ChatTokens, connection: u64, ask: Ask) -> Answer {
        let Some((id, replied)) = self.exchange(&Heard::Ask { connection, ask }) else {
            return nobody();
        };
        let mut told_gone = false;
        let answer = loop {
            match replied.recv_timeout(AN_ASKER_IS_LOOKED_AT_EVERY) {
                Ok(Replied::Answer(answer)) => break answer,
                Ok(other) => {
                    break Answer::No {
                        why: format!("the window replied {other:?} to an ask"),
                    };
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break nobody(),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !told_gone && tokens.asker_gone(connection) {
                        told_gone = true;
                        self.tell(&Heard::AskerGone { connection });
                    }
                }
            }
        };
        self.done(id);
        answer
    }

    /// Hands a permission ask on: held where the window holds it, its option coming when it is
    /// chosen there, and the window told if the hook leaves first.
    fn permission(self: &Arc<Self>, asked: PermissionAsked) -> Option<Waiting> {
        let (id, replied) = self.exchange(&Heard::Permission(asked))?;
        let until = match replied.recv_timeout(A_RELAYED_LINE_IS_TAKEN_WITHIN) {
            Ok(Replied::Held { until_ms }) => Duration::from_millis(until_ms),
            _ => {
                self.done(id);
                return None;
            }
        };
        let (chosen, answered) = mpsc::channel();
        {
            let mut waiting = self.waiting();
            // A reply that came between the hold and now is the option, or that none comes.
            let chosen_already = replied.try_iter().find_map(|reply| match reply {
                Replied::Chosen(choice) => Some(Some(choice)),
                Replied::Ended => Some(None),
                _ => None,
            });
            match chosen_already {
                Some(Some(choice)) => {
                    let _ = chosen.send(choice);
                    waiting.remove(&id);
                }
                Some(None) => {
                    waiting.remove(&id);
                }
                None => {
                    waiting.insert(id, Awaiting::Choice(chosen));
                }
            }
        }
        let relaying = Arc::clone(self);
        Some(Waiting {
            answered,
            until,
            left: Box::new(move |left| {
                relaying.done(id);
                relaying.tell(&Heard::Left {
                    of: id,
                    closed: left == Left::Closed,
                });
            }),
        })
    }

    /// Hands a `secret exec` on: the asker's input one way, line by line, and what the command
    /// writes the other, until the window says it has ended.
    fn secret_exec(
        self: &Arc<Self>,
        ask: brokered::Ask,
        reader: Box<dyn BufRead + Send>,
        mut writer: Box<dyn Write + Send>,
    ) {
        let Some((id, replied)) = self.exchange(&Heard::SecretExec(ask)) else {
            brokered::not_answered(writer);
            return;
        };
        let relaying = Arc::clone(self);
        let input = std::thread::Builder::new()
            .name("purlisd-secret-input".into())
            .spawn(move || {
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    relaying.tell(&Heard::Input { of: id, line });
                }
                relaying.tell(&Heard::InputEnded { of: id });
            });
        if input.is_err() {
            self.done(id);
            brokered::not_answered(writer);
            return;
        }
        while let Ok(reply) = replied.recv() {
            match reply {
                Replied::Wrote { text } => {
                    if writer
                        .write_all(text.as_bytes())
                        .and_then(|()| writer.flush())
                        .is_err()
                    {
                        break;
                    }
                }
                Replied::Ended => break,
                _ => {}
            }
        }
        self.done(id);
    }

    /// The [`Hearing`] the host's hook channel listens with: every line handed on, and the
    /// window's reply given back. `tokens` are the channel's own, which say whether an asker
    /// hung up.
    pub fn hearing(self: &Arc<Self>, tokens: Arc<ChatTokens>) -> Hearing {
        let to = |relaying: &Arc<Relaying>| Arc::clone(relaying);
        Hearing {
            each: {
                let relaying = to(self);
                Box::new(move |report| relaying.taken(report.chat, &Heard::Report(report)))
            },
            answer: {
                let relaying = to(self);
                Box::new(move |connection, ask| relaying.answer(&tokens, connection, ask))
            },
            noticed: {
                let relaying = to(self);
                Box::new(move |notice| relaying.tell(&Heard::StartedByHand(notice)))
            },
            saved: {
                let relaying = to(self);
                Box::new(move |saved| relaying.tell(&Heard::Saved(saved)))
            },
            refused: {
                let relaying = to(self);
                Box::new(move |refused| relaying.taken(refused.chat, &Heard::Refused(refused)))
            },
            tool: {
                let relaying = to(self);
                Box::new(move |call| relaying.taken(call.chat, &Heard::Tool(call)))
            },
            permission: {
                let relaying = to(self);
                Box::new(move |asked| relaying.permission(asked))
            },
            touching: {
                let relaying = to(self);
                Box::new(move |touching| relaying.tell(&Heard::Touching(touching)))
            },
            blocked: {
                let relaying = to(self);
                Box::new(move |blocked| relaying.tell(&Heard::Blocked(blocked)))
            },
            doing: {
                let relaying = to(self);
                Box::new(move |doing| relaying.tell(&Heard::Doing(doing)))
            },
            output: {
                let relaying = to(self);
                Box::new(move |output| relaying.tell(&Heard::Output(output)))
            },
            secret_exec: {
                let relaying = to(self);
                Box::new(move |ask, reader, writer| relaying.secret_exec(ask, reader, writer))
            },
        }
    }
}

fn nobody() -> Answer {
    Answer::No {
        why: NOBODY_HEARS.to_owned(),
    }
}

/// What the window replies through, from any thread, in the order it is called.
pub type Reply = Arc<dyn Fn(Replied) + Send + Sync + 'static>;

/// **The window's half**: hears each line the host handed on with the window's own
/// [`Hearing`], and replies what it answered.
pub struct Heeding {
    hearing: Hearing,
    /// The hook connections whose askers hung up while their asks were answered.
    gone: Mutex<HashSet<u64>>,
    /// What each permission ask held here is told if the host lets it go first.
    held: Mutex<HashMap<u64, LetGo>>,
    /// Where each running `secret exec`'s input goes.
    inputs: Mutex<HashMap<u64, mpsc::Sender<String>>>,
}

/// What a permission ask held in the window is told as it ends unanswered.
type LetGo = Box<dyn FnOnce(Left) + Send + 'static>;

/// How much longer than its deadline the window holds a permission ask the host has not let go
/// of: the host's word that it has is on its way.
const A_HELD_ASK_OUTLIVES_ITS_DEADLINE_BY: Duration = Duration::from_secs(5);

/// How often a held permission ask is looked at for having been let go of by the host.
const A_HELD_ASK_IS_LOOKED_AT_EVERY: Duration = Duration::from_millis(100);

impl Heeding {
    /// Hears every line with `hearing`.
    pub fn new(hearing: Hearing) -> Heeding {
        Heeding {
            hearing,
            gone: Mutex::default(),
            held: Mutex::default(),
            inputs: Mutex::default(),
        }
    }

    /// Whether hearing `heard` may wait as long as a person takes, or a command runs: such a
    /// line is heard on a thread of its own. Every other is heard at once and in order.
    pub fn waits(heard: &Heard) -> bool {
        matches!(
            heard,
            Heard::Ask { .. } | Heard::Permission(_) | Heard::SecretExec(_)
        )
    }

    /// Whether the asker on hook connection `connection` hung up while its ask was answered.
    pub fn asker_gone(&self, connection: u64) -> bool {
        lock(&self.gone).contains(&connection)
    }

    /// Hears the line handed on as `id`, and replies through `reply`: returns once it is heard,
    /// which for an ask, a permission ask or a `secret exec` is once it is answered or ended.
    pub fn hear(&self, id: u64, heard: Heard, reply: Reply) {
        // What the hearing took, with what it would have recorded carried back to the host.
        let taken = |heard: (io::Result<()>, Option<Entry>)| match heard {
            (Ok(()), Some(entry)) => Replied::Record(entry),
            (Ok(()), None) => Replied::Taken,
            (Err(why), _) => Replied::NotTaken {
                why: why.to_string(),
            },
        };
        match heard {
            Heard::Report(report) => reply(taken(carrying(|| (self.hearing.each)(report)))),
            Heard::Ask { connection, ask } => {
                let answer = (self.hearing.answer)(connection, ask);
                lock(&self.gone).remove(&connection);
                reply(Replied::Answer(answer));
            }
            Heard::StartedByHand(notice) => (self.hearing.noticed)(notice),
            Heard::Saved(saved) => (self.hearing.saved)(saved),
            Heard::Refused(refused) => {
                reply(taken(carrying(|| (self.hearing.refused)(refused))));
            }
            Heard::Tool(call) => reply(taken(carrying(|| (self.hearing.tool)(call)))),
            Heard::Touching(touching) => (self.hearing.touching)(touching),
            Heard::Blocked(blocked) => (self.hearing.blocked)(blocked),
            Heard::Doing(doing) => (self.hearing.doing)(doing),
            Heard::Output(output) => (self.hearing.output)(output),
            Heard::Permission(asked) => self.hold(id, asked, &reply),
            Heard::Left { of, closed } => {
                let left = lock(&self.held).remove(&of);
                if let Some(left) = left {
                    left(if closed { Left::Closed } else { Left::TimedOut });
                }
            }
            Heard::AskerGone { connection } => {
                lock(&self.gone).insert(connection);
            }
            Heard::SecretExec(ask) => self.secret_exec(id, ask, reply),
            Heard::Input { of, line } => {
                if let Some(input) = lock(&self.inputs).get(&of) {
                    let _ = input.send(line);
                }
            }
            Heard::InputEnded { of } => {
                lock(&self.inputs).remove(&of);
            }
        }
    }

    /// Holds a permission ask here until it is answered, the host lets it go, or it is past
    /// its deadline.
    fn hold(&self, id: u64, asked: PermissionAsked, reply: &Reply) {
        let Some(Waiting {
            answered,
            until,
            left,
        }) = (self.hearing.permission)(asked)
        else {
            reply(Replied::NotHeld);
            return;
        };
        lock(&self.held).insert(id, left);
        reply(Replied::Held {
            until_ms: u64::try_from(until.as_millis()).unwrap_or(u64::MAX),
        });
        let began = Instant::now();
        loop {
            match answered.recv_timeout(A_HELD_ASK_IS_LOOKED_AT_EVERY) {
                Ok(choice) => {
                    lock(&self.held).remove(&id);
                    reply(Replied::Chosen(choice));
                    return;
                }
                // Nothing here will answer it.
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    lock(&self.held).remove(&id);
                    reply(Replied::Ended);
                    return;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // The host let it go, and said so.
                    if !lock(&self.held).contains_key(&id) {
                        return;
                    }
                    if began.elapsed() >= until + A_HELD_ASK_OUTLIVES_ITS_DEADLINE_BY {
                        let left = lock(&self.held).remove(&id);
                        if let Some(left) = left {
                            left(Left::TimedOut);
                        }
                        reply(Replied::Ended);
                        return;
                    }
                }
            }
        }
    }

    /// Runs a `secret exec` here, its input the lines the host hands on, its frames replied.
    fn secret_exec(&self, id: u64, ask: brokered::Ask, reply: Reply) {
        let (input, lines) = mpsc::channel();
        lock(&self.inputs).insert(id, input);
        let reader = Box::new(std::io::BufReader::new(Lines {
            lines,
            pending: Vec::new(),
            at: 0,
        }));
        let writer = Box::new(Frames {
            reply: Arc::clone(&reply),
            pending: Vec::new(),
        });
        (self.hearing.secret_exec)(ask, reader, writer);
        lock(&self.inputs).remove(&id);
        reply(Replied::Ended);
    }
}

/// A `secret exec`'s input, as the lines the host hands on, each ended again by its newline.
struct Lines {
    lines: mpsc::Receiver<String>,
    pending: Vec<u8>,
    at: usize,
}

impl Read for Lines {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.at >= self.pending.len() {
            let Ok(line) = self.lines.recv() else {
                return Ok(0);
            };
            self.pending = line.into_bytes();
            self.pending.push(b'\n');
            self.at = 0;
        }
        let read = buf.len().min(self.pending.len() - self.at);
        buf[..read].copy_from_slice(&self.pending[self.at..self.at + read]);
        self.at += read;
        Ok(read)
    }
}

/// What a `secret exec`'s command writes to its asker, replied a whole line at a time, so no
/// character is split between two replies.
struct Frames {
    reply: Reply,
    pending: Vec<u8>,
}

impl Frames {
    fn send(&mut self, upto: usize) {
        let text: Vec<u8> = self.pending.drain(..upto).collect();
        (self.reply)(Replied::Wrote {
            text: String::from_utf8_lossy(&text).into_owned(),
        });
    }
}

impl Write for Frames {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);
        if let Some(last) = self.pending.iter().rposition(|byte| *byte == b'\n') {
            self.send(last + 1);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.pending.is_empty() {
            let all = self.pending.len();
            self.send(all);
        }
        Ok(())
    }
}

impl Drop for Frames {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

thread_local! {
    /// What the line this thread's hearing hears has asked to be recorded, while the window
    /// hears one the host handed on ([`carrying`]): `None` when no such line is being heard.
    static CARRIED: RefCell<Option<Option<Entry>>> = const { RefCell::new(None) };
}

/// **Carries `entry` back to the host with the reply to the line this thread is hearing**
/// (#2003): the window's hearing asks the event log for `entry` where it hears a line the host
/// handed on, and the host, which writes the log, records it. `Err` hands `entry` back where no
/// such line is being heard on this thread, or one was carried for it already: the window then
/// writes it as it writes anything else.
pub fn carry(entry: Entry) -> Result<(), Entry> {
    CARRIED.with(|carried| match &mut *carried.borrow_mut() {
        Some(slot @ None) => {
            *slot = Some(entry);
            Ok(())
        }
        _ => Err(entry),
    })
}

/// Runs `hear` with a line being heard on this thread, and answers what it answered and the
/// entry it asked to be recorded ([`carry`]).
fn carrying<T>(hear: impl FnOnce() -> T) -> (T, Option<Entry>) {
    CARRIED.with(|carried| *carried.borrow_mut() = Some(None));
    let heard = hear();
    let entry = CARRIED.with(|carried| carried.borrow_mut().take().flatten());
    (heard, entry)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
