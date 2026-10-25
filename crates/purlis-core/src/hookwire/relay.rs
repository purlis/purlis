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
//!
//! **A browser login asked while nobody is there waits for the window** (#2050, spec #2041
//! story 14): on a runner whose desktop is away, `browser_login` is answered at once in words
//! that say it waits for the person as an Ask and the chat should carry on, and the ask is
//! kept, in memory, at most [`MOST_LOGINS_WAITING`] and each chat's login once. As soon as a
//! window hears this host's lines again, each is handed on to it as it was asked, so the window
//! decides it as it decides any login (the persona's opt-in for the pool included), and what it
//! came to is said to that chat at the start of its next browser call. No value is kept here:
//! what waits is the login's name.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::{Duration, Instant};

use super::permission::{Left, PermissionAsked, Waiting};
use super::{
    Answer, Ask, BrowseAsk, Browsing, ChatTokens, CommitRefused, Doing, Hearing, Report,
    SandboxBlocked, SessionSaved, StartedByHand, ToolCall, Touching, TurnOutput,
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

impl Heard {
    /// The chat the line says it comes from, where it names one: a line about a line handed on
    /// before (`Left`, `Input`, `InputEnded`) or a hook connection (`AskerGone`) names none.
    pub fn chat(&self) -> Option<u32> {
        match self {
            Heard::Report(report) => Some(report.chat),
            Heard::Ask { ask, .. } => Some(ask.chat()),
            Heard::StartedByHand(notice) => Some(notice.chat),
            Heard::Saved(saved) => Some(saved.chat),
            Heard::Refused(refused) => Some(refused.chat),
            Heard::Tool(call) => Some(call.chat),
            Heard::Touching(touching) => Some(touching.chat),
            Heard::Blocked(blocked) => Some(blocked.chat),
            Heard::Doing(doing) => Some(doing.chat),
            Heard::Output(output) => Some(output.chat),
            Heard::Permission(asked) => Some(asked.chat),
            Heard::SecretExec(ask) => Some(ask.chat),
            Heard::Left { .. }
            | Heard::AskerGone { .. }
            | Heard::Input { .. }
            | Heard::InputEnded { .. } => None,
        }
    }

    /// **Refuses the line, replying through `reply`** as each kind is refused, for `why`: a
    /// report, a tool call or a refused commit is not taken, an ask is answered no, a permission
    /// ask is not held, and a `secret exec` ends; nothing is replied to the rest.
    pub fn refuse(&self, reply: &Reply, why: &str) {
        match self {
            Heard::Report(_) | Heard::Refused(_) | Heard::Tool(_) => {
                reply(Replied::NotTaken {
                    why: why.to_owned(),
                });
            }
            Heard::Ask { .. } => reply(Replied::Answer(Answer::No {
                why: why.to_owned(),
            })),
            Heard::Permission(_) => reply(Replied::NotHeld),
            Heard::SecretExec(_) => reply(Replied::Ended),
            _ => {}
        }
    }
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

/// The most browser logins that wait for the window at once (#2050).
pub const MOST_LOGINS_WAITING: usize = 16;

/// The most things one chat's next browser call is told of logins that waited.
const MOST_TOLD: usize = 8;

/// What a chat is told of its browser login `login`, asked while nobody is there (#2050).
pub fn login_waits_said(login: &str) -> String {
    format!(
        "the person's desktop is not connected, so purlis did not log in with {login}: the login \
         waits for them as an Ask, and purlis performs it once they are back if they allow it. Go \
         on with what needs no login; a later browser call says how it went"
    )
}

/// What a chat's next browser call says first of its login `login` that waited, which came to
/// `outcome` once a window heard it.
fn waited_said(login: &str, outcome: &str) -> String {
    format!(
        "while the person was away, this chat asked purlis to log in with {login}; now they are \
         back: {outcome}"
    )
}

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
    /// The browser logins asked while nobody was there, each on the hook connection it came on,
    /// oldest first (#2050).
    logins_waiting: Mutex<Vec<(u64, BrowseAsk)>>,
    /// What each chat's next browser call is told first: what its logins that waited came to.
    told: Mutex<HashMap<u32, Vec<String>>>,
}

/// **How the host keeps what its hook channel hears** (#2003): it writes the event log, and
/// looks at a chat's hook spool soon after one of its lines comes, as the window did.
pub struct Keeping {
    /// Writes one entry in the event log, durably: the hook is told its line is taken only once
    /// this answers `Ok`.
    pub record: Box<dyn Fn(Entry) -> io::Result<()> + Send + Sync + 'static>,
    /// Chat `chat` was heard from: its spool is looked at soon after.
    pub heard_from: Box<dyn Fn(u32) + Send + Sync + 'static>,
    /// A report, a tool call or a refused commit was handed on, and the window it went to went
    /// before it replied ([`Relaying::let_go`]): the host's own board hears it, as it hears a
    /// line no window took (#2006). A window that is only slow is not this: it still moves its
    /// own board, and tells the host so.
    pub unheard: Box<dyn Fn(&Heard) + Send + Sync + 'static>,
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
            logins_waiting: Mutex::default(),
            told: Mutex::default(),
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
    /// nobody heard. A report, a tool call or a refused commit among them is told to the host's
    /// keeping as unheard ([`Keeping::unheard`]). What the host does when the window's link
    /// ends, and as the channel stops.
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
    fn taken(self: &Arc<Self>, chat: u32, heard: &Heard) -> io::Result<()> {
        if let Some(keeping) = &self.keeping {
            (keeping.heard_from)(chat);
        }
        let Some((id, replied)) = self.exchange(heard) else {
            return Err(io::Error::other(NOBODY_HEARS));
        };
        self.hand_on_the_logins_that_waited();
        let reply = replied.recv_timeout(A_RELAYED_LINE_IS_TAKEN_WITHIN);
        self.done(id);
        if let (Err(mpsc::RecvTimeoutError::Disconnected), Some(keeping)) = (&reply, &self.keeping)
        {
            (keeping.unheard)(heard);
        }
        match reply {
            Ok(Replied::Taken) => Ok(()),
            Ok(Replied::Record(entry)) if of_the_line(&entry, heard) => self.record(entry),
            Ok(Replied::Record(_)) => Err(io::Error::other(
                "what the window sent back to record is not of this line, so it was not recorded",
            )),
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
    fn answer(self: &Arc<Self>, tokens: &ChatTokens, connection: u64, ask: Ask) -> Answer {
        let browsing = match &ask {
            Ask::Browse(browse) => Some((**browse).clone()),
            _ => None,
        };
        let Some((id, replied)) = self.exchange(&Heard::Ask { connection, ask }) else {
            return match browsing {
                Some(browse) => self.wait_for_the_window(connection, browse),
                None => nobody(),
            };
        };
        self.hand_on_the_logins_that_waited();
        let answer = self.answered(tokens, connection, id, &replied);
        match browsing {
            Some(browse) => self.told_first(browse.chat, answer),
            None => answer,
        }
    }

    /// The answer to the ask handed on as `id` on hook connection `connection`, waited for as
    /// long as the window takes, the window told once its asker hangs up.
    fn answered(
        &self,
        tokens: &ChatTokens,
        connection: u64,
        id: u64,
        replied: &mpsc::Receiver<Replied>,
    ) -> Answer {
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

    /// **Browser call `browse`, on hook connection `connection`, that nobody is there to hear**
    /// (#2050): a login waits for the window, and is said to; any other is answered as a line
    /// nobody heard.
    fn wait_for_the_window(&self, connection: u64, browse: BrowseAsk) -> Answer {
        let Browsing::LogIn { login } = &browse.asked else {
            return nobody();
        };
        let login = login.clone();
        let mut waiting = lock(&self.logins_waiting);
        let already = waiting.iter().any(|(_, one)| {
            one.chat == browse.chat
                && matches!(&one.asked, Browsing::LogIn { login: theirs } if *theirs == login)
        });
        if !already {
            if waiting.len() >= MOST_LOGINS_WAITING {
                return Answer::No {
                    why: format!(
                        "the person's desktop is not connected, and too many logins wait for \
                         them already, so purlis did not log in with {login} and keeps nothing \
                         of it: go on with what needs no login, and ask again once they are back"
                    ),
                };
            }
            waiting.push((connection, browse));
        }
        Answer::No {
            why: login_waits_said(&login),
        }
    }

    /// **Hands every login that waited on to the window that hears this host again** (#2050),
    /// each on a thread of its own, as it was asked: what each came to is told to its chat's
    /// next browser call. One the window did not take waits on.
    fn hand_on_the_logins_that_waited(self: &Arc<Self>) {
        let waited = std::mem::take(&mut *lock(&self.logins_waiting));
        for (connection, browse) in waited {
            let relaying = Arc::clone(self);
            let spawned = std::thread::Builder::new()
                .name("purlisd-login-waited".into())
                .spawn(move || relaying.hand_on_one_that_waited(connection, browse));
            if let Err(why) = spawned {
                tracing::warn!("purlisd: a login that waited was not handed on: {why}");
            }
        }
    }

    fn hand_on_one_that_waited(&self, connection: u64, browse: BrowseAsk) {
        let ask = Ask::Browse(Box::new(browse.clone()));
        let Some((id, replied)) = self.exchange(&Heard::Ask { connection, ask }) else {
            self.waits_again(connection, browse);
            return;
        };
        let reply = replied.recv();
        self.done(id);
        let Browsing::LogIn { login } = &browse.asked else {
            return;
        };
        match reply {
            Ok(Replied::Answer(answer)) => {
                let outcome = match &answer {
                    Answer::No { why } => why.clone(),
                    Answer::Browsed { browsed, .. } => browsed
                        .note
                        .clone()
                        .unwrap_or_else(|| "purlis performed it".to_owned()),
                    other => format!("the window answered {other:?}"),
                };
                let mut told = lock(&self.told);
                let chat = told.entry(browse.chat).or_default();
                if chat.len() < MOST_TOLD {
                    chat.push(waited_said(login, &outcome));
                }
            }
            // The window went before it answered: it waits for the next one.
            Err(_) => self.waits_again(connection, browse),
            Ok(_) => {}
        }
    }

    fn waits_again(&self, connection: u64, browse: BrowseAsk) {
        let mut waiting = lock(&self.logins_waiting);
        if waiting.len() < MOST_LOGINS_WAITING {
            waiting.push((connection, browse));
        }
    }

    /// `answer` to a browser call of chat `chat`'s, after what its logins that waited came to,
    /// which it is told once.
    fn told_first(&self, chat: u32, mut answer: Answer) -> Answer {
        let Some(told) = lock(&self.told).remove(&chat) else {
            return answer;
        };
        let said = told.join(". ");
        match &mut answer {
            Answer::No { why } => *why = format!("{said}. {why}"),
            Answer::Browsed { browsed, .. } => {
                browsed.note = Some(match browsed.note.take() {
                    Some(note) => format!("{said}. {note}"),
                    None => said,
                });
            }
            // Nothing else answers a browser call: what it would say is kept for the next.
            _ => {
                lock(&self.told).insert(chat, told);
            }
        }
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
    /// Shared with every heeding made [`Heeding::beside`] it.
    hearing: Arc<Hearing>,
    /// The hook connections whose askers hung up while their asks were answered.
    gone: Mutex<HashSet<u64>>,
    /// What each permission ask held here is told if the host lets it go first.
    held: Mutex<HashMap<u64, LetGo>>,
    /// The input of each `secret exec` handed on, by the number it was handed on as.
    inputs: Mutex<Inputs>,
    /// What tells each `secret exec` run here from one run before it under the same number.
    execs: AtomicU64,
    /// The link to the host the lines come over now, by its number: `None` before the first
    /// and after it ended.
    link: Mutex<Option<u64>>,
}

/// **The input of one `secret exec`** (#2271, #2571). The window's link hears the exec on a
/// thread of its own and its input, line by line, in order on another, so a line, and the
/// input's end, can be heard before the exec has begun. The exec is therefore tracked from the
/// moment its line is taken, in order, before its thread is started ([`Heeding::admit`]): what
/// comes for it before it begins is kept for it, and what comes for a number not tracked is of
/// no exec here and is dropped. Every entry is an exec its chat has in flight, so how many a
/// chat and a link track is bounded ([`EXECS_A_CHAT_HAS_TRACKED_AT_MOST`],
/// [`EXECS_A_LINK_TRACKS_AT_MOST`]), as is what waits for those not begun
/// ([`AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST`], [`EXECS_NOT_BEGUN_ARE_KEPT_AT_MOST`]). None outlives
/// its exec or the link it came over ([`Heeding::linked`], [`Heeding::unlinked`]).
enum Input {
    /// The exec, of chat `chat`, has not begun: the lines that came for it, their bytes, and
    /// whether its input has ended, and why.
    Early {
        chat: u32,
        lines: Vec<String>,
        bytes: usize,
        ending: Ending,
    },
    /// The exec, of chat `chat` and numbered `exec`, runs and reads what is sent through `to`:
    /// `None` once its input has ended, and what still comes for it is dropped.
    Running {
        chat: u32,
        to: Option<mpsc::Sender<String>>,
        exec: u64,
    },
}

impl Input {
    fn chat(&self) -> u32 {
        match self {
            Input::Early { chat, .. } | Input::Running { chat, .. } => *chat,
        }
    }

    fn bytes(&self) -> usize {
        match self {
            Input::Early { bytes, .. } => *bytes,
            Input::Running { .. } => 0,
        }
    }
}

/// The `secret exec` inputs, held with the link the lines come over now.
type InputsOn<'a> = (MutexGuard<'a, Option<u64>>, MutexGuard<'a, Inputs>);

/// **The `secret exec`s the current link tracks** (#2571): at most
/// [`EXECS_A_LINK_TRACKS_AT_MOST`] of them, and [`EXECS_A_CHAT_HAS_TRACKED_AT_MOST`] of one
/// chat's, with the bytes kept for those not begun and the execs of each chat counted as they
/// come and go, and whether this link was warned that a cap was passed. Every entry goes in
/// through [`Inputs::track`] and out through [`Inputs::untrack`].
#[derive(Default)]
struct Inputs {
    by_exec: HashMap<u64, Input>,
    /// How many each chat has tracked.
    of_chat: HashMap<u32, usize>,
    /// What every [`Input::Early`] holds together.
    kept: usize,
    /// Whether this link was warned of input past what is kept for execs not begun.
    warned_of_bytes: bool,
    /// Whether this link was warned of execs past what it, or a chat, may have tracked.
    warned_of_count: bool,
}

impl Inputs {
    /// Tracks `input` for `exec`, a number not tracked, where its chat and the link have room
    /// for one more: false where not, warned of once on the link, and nothing is tracked.
    fn track(&mut self, exec: u64, input: Input) -> bool {
        debug_assert!(!self.by_exec.contains_key(&exec), "a number tracked twice");
        let chat = input.chat();
        let of_chat = self.of_chat.get(&chat).copied().unwrap_or(0);
        if self.by_exec.len() >= EXECS_A_LINK_TRACKS_AT_MOST
            || of_chat >= EXECS_A_CHAT_HAS_TRACKED_AT_MOST
        {
            if !self.warned_of_count {
                self.warned_of_count = true;
                tracing::warn!(
                    "purlis: a secret exec was not run: its chat, or the link to the session \
                     host, has as many in flight as are tracked \
                     ({EXECS_A_CHAT_HAS_TRACKED_AT_MOST} for a chat, \
                     {EXECS_A_LINK_TRACKS_AT_MOST} for a link)"
                );
            }
            return false;
        }
        self.kept += input.bytes();
        *self.of_chat.entry(chat).or_default() += 1;
        self.by_exec.insert(exec, input);
        true
    }

    /// What is tracked for `exec`, no longer tracked.
    fn untrack(&mut self, exec: u64) -> Option<Input> {
        let input = self.by_exec.remove(&exec)?;
        let bytes = input.bytes();
        debug_assert!(self.kept >= bytes, "more bytes given back than were kept");
        self.kept -= bytes;
        let chat = input.chat();
        if let Some(count) = self.of_chat.get_mut(&chat) {
            *count -= 1;
            if *count == 0 {
                self.of_chat.remove(&chat);
            }
        }
        Some(input)
    }

    /// One line of the input to `of`: kept for it where it has not begun, handed to it where it
    /// runs, and dropped where its input ended or it is not tracked.
    fn input(&mut self, of: u64, line: String) {
        let Inputs {
            by_exec,
            kept,
            warned_of_bytes,
            ..
        } = self;
        match by_exec.get_mut(&of) {
            Some(Input::Early {
                lines,
                bytes,
                ending: ending @ Ending::Open,
                ..
            }) => {
                let more = line.len() + 1;
                if *bytes + more > AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST
                    || *kept + more > EXECS_NOT_BEGUN_ARE_KEPT_AT_MOST
                {
                    if !*warned_of_bytes {
                        *warned_of_bytes = true;
                        tracing::warn!(
                            "purlis: a secret exec that has not begun was sent more input than \
                             is kept for it ({AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST} bytes, \
                             {EXECS_NOT_BEGUN_ARE_KEPT_AT_MOST} for all on a link); its input \
                             ends there, and so does any other's on this link that passes it"
                        );
                    }
                    *ending = Ending::PastTheCap;
                } else {
                    *bytes += more;
                    *kept += more;
                    lines.push(line);
                }
            }
            Some(Input::Running { to: Some(to), .. }) => {
                let _ = to.send(line);
            }
            Some(Input::Early { .. } | Input::Running { to: None, .. }) | None => {}
        }
    }

    /// The host said `of`'s input ended: one not begun reads what came, then the end; one that
    /// runs reads to the end.
    fn input_ended(&mut self, of: u64) {
        match self.by_exec.get_mut(&of) {
            Some(Input::Early { ending, .. }) => *ending = Ending::Told,
            Some(Input::Running { to, .. }) => *to = None,
            None => {}
        }
    }

    /// Exec `exec`, handed on as `id`, of chat `chat`, begins, reading through `to`: what was
    /// kept for it is sent, and where its input ended, the end. False where it may not begin:
    /// its chat or the link has no room for it, or another runs under its number.
    fn begin(&mut self, id: u64, chat: u32, exec: u64, to: mpsc::Sender<String>) -> bool {
        let (chat, lines, ending) = match self.untrack(id) {
            Some(Input::Early {
                chat,
                lines,
                ending,
                ..
            }) => (chat, lines, ending),
            Some(running @ Input::Running { .. }) => {
                self.track(id, running);
                return false;
            }
            None => (chat, Vec::new(), Ending::Open),
        };
        for line in lines {
            let _ = to.send(line);
        }
        let to = (ending == Ending::Open).then_some(to);
        self.track(id, Input::Running { chat, to, exec })
    }

    /// Exec `exec`, handed on as `id`, ended: it is no longer tracked, and what still comes for
    /// it is dropped. An exec under its number from a link before is not this one.
    fn finished(&mut self, id: u64, exec: u64) {
        if matches!(
            self.by_exec.get(&id),
            Some(Input::Running { exec: running, .. }) if *running == exec
        ) {
            self.untrack(id);
        }
    }
}

/// Whether, and why, the input kept for a `secret exec` not begun has ended.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// More may come.
    Open,
    /// More came than is kept: the exec reads what was, then the end, and what still comes is
    /// dropped.
    PastTheCap,
    /// The host said its input ended: nothing more comes.
    Told,
}

/// The most bytes of input, newlines counted, kept for one `secret exec` that has not begun: a
/// line past it is dropped, and the exec's input ends there.
pub const AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST: usize = 64 * 1024;

/// The most bytes of input kept for every `secret exec` not begun on a link together: a line
/// past it is dropped, and its exec's input ends there.
pub const EXECS_NOT_BEGUN_ARE_KEPT_AT_MOST: usize = 4 * AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST;

/// The most `secret exec`s a link tracks at once, every chat's together: from when each is
/// taken until it ends. One past it is not run, and its host is told it ended.
pub const EXECS_A_LINK_TRACKS_AT_MOST: usize = 256;

/// The most `secret exec`s one chat has tracked on a link at once, so one chat's cannot crowd
/// out another's: one past it is not run, and its host is told it ended.
pub const EXECS_A_CHAT_HAS_TRACKED_AT_MOST: usize = 16;

/// What the asker of a `secret exec` is told where it was not run for want of room.
pub const TOO_MANY_SECRET_EXECS: &str =
    "too many secret commands are running at once; run it again once one has ended";

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
        Heeding::hearing_with(Arc::new(hearing))
    }

    /// **Hears another host's lines with this one's hearing** (#2046): the lines of a pool's
    /// runner, beside this machine's host. Its links, its secret execs' input, its held asks
    /// and its askers that hung up are its own, since each host numbers its own lines and its
    /// own hook connections.
    pub fn beside(&self) -> Heeding {
        Heeding::hearing_with(Arc::clone(&self.hearing))
    }

    fn hearing_with(hearing: Arc<Hearing>) -> Heeding {
        Heeding {
            hearing,
            gone: Mutex::default(),
            held: Mutex::default(),
            inputs: Mutex::default(),
            execs: AtomicU64::new(1),
            link: Mutex::default(),
        }
    }

    /// The lines come over link `link` to the host from now on. A link other than the one
    /// they came over before is a host that may have started again, numbering its lines from
    /// the start: nothing kept of a `secret exec`'s input from before is kept on.
    pub fn linked(&self, link: u64) {
        let mut on = lock(&self.link);
        if *on != Some(link) {
            *on = Some(link);
            *lock(&self.inputs) = Inputs::default();
        }
    }

    /// Link `link` to the host ended. Where the lines came over it, every `secret exec`'s
    /// input ends, and nothing kept of one is kept on. A link the lines no longer come over
    /// changes nothing.
    pub fn unlinked(&self, link: u64) {
        let mut on = lock(&self.link);
        if *on == Some(link) {
            *on = None;
            *lock(&self.inputs) = Inputs::default();
        }
    }

    /// **Takes line `id`, heard over link `link`, in the order the lines came**, before it is
    /// heard ([`Heeding::hear`]), which for a line that waits is on a thread of its own: a
    /// `secret exec` is tracked from here, so input that comes for it before it begins is kept
    /// for it (#2571). False where it is not to be heard, and its host was told through `reply`
    /// that it ended: an exec from a link other than the current one, one under a number taken
    /// already, or one its chat or the link has no room for, whose asker is told why. Every
    /// other line is taken.
    pub fn admit(&self, link: u64, id: u64, heard: &Heard, reply: &Reply) -> bool {
        let Heard::SecretExec(ask) = heard else {
            return true;
        };
        let tracked = self.inputs_on(link).and_then(|(_on, mut inputs)| {
            // The host numbers each line once on a link.
            (!inputs.by_exec.contains_key(&id)).then(|| {
                inputs.track(
                    id,
                    Input::Early {
                        chat: ask.chat,
                        lines: Vec::new(),
                        bytes: 0,
                        ending: Ending::Open,
                    },
                )
            })
        });
        match tracked {
            Some(true) => return true,
            Some(false) => brokered::refused(
                Box::new(Frames {
                    reply: Arc::clone(reply),
                    pending: Vec::new(),
                }),
                TOO_MANY_SECRET_EXECS.to_owned(),
            ),
            None => {}
        }
        reply(Replied::Ended);
        false
    }

    /// The `secret exec` handed on as `id` over link `link` will not run here (the thread it was
    /// to be heard on could not be had): what still comes for it is dropped, and the host is
    /// told it ended.
    pub fn not_begun(&self, link: u64, id: u64, reply: &Reply) {
        if let Some((_on, mut inputs)) = self.inputs_on(link) {
            inputs.untrack(id);
        }
        reply(Replied::Ended);
    }

    /// The `secret exec` inputs, where `link` is the link the lines come over now: held with
    /// the link, so the link cannot change while they are changed.
    fn inputs_on(&self, link: u64) -> Option<InputsOn<'_>> {
        let on = lock(&self.link);
        if *on != Some(link) {
            return None;
        }
        let inputs = lock(&self.inputs);
        Some((on, inputs))
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

    /// Hears the line handed on as `id` over link `link`, and replies through `reply`: returns
    /// once it is heard, which for an ask, a permission ask or a `secret exec` is once it is
    /// answered or ended. A `secret exec`, or its input, from a link other than the one the lines
    /// come over now is of a host that went or started again: it is dropped, and the exec told
    /// to its host as ended.
    pub fn hear(&self, link: u64, id: u64, heard: Heard, reply: Reply) {
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
            Heard::SecretExec(ask) => self.secret_exec(link, id, ask, reply),
            Heard::Input { of, line } => self.input(link, of, line),
            Heard::InputEnded { of } => {
                if let Some((_on, mut inputs)) = self.inputs_on(link) {
                    inputs.input_ended(of);
                }
            }
        }
    }

    /// One line of the input to the `secret exec` handed on as `of`, over link `link`.
    fn input(&self, link: u64, of: u64, line: String) {
        if let Some((_on, mut inputs)) = self.inputs_on(link) {
            inputs.input(of, line);
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
    fn secret_exec(&self, link: u64, id: u64, ask: brokered::Ask, reply: Reply) {
        let exec = self.execs.fetch_add(1, Ordering::Relaxed);
        let (to, lines) = mpsc::channel();
        let begun = self
            .inputs_on(link)
            .is_some_and(|(_on, mut inputs)| inputs.begin(id, ask.chat, exec, to));
        if !begun {
            reply(Replied::Ended);
            return;
        }
        let reader = Box::new(std::io::BufReader::new(Lines {
            lines,
            pending: Vec::new(),
            at: 0,
        }));
        let writer = Box::new(Frames {
            reply: Arc::clone(&reply),
            pending: Vec::new(),
        });
        // Untracked and told ended however the command's run ends, a panic too.
        let _finishing = Finishing {
            heeding: self,
            id,
            exec,
            reply,
        };
        (self.hearing.secret_exec)(ask, reader, writer);
    }
}

/// **What ends a `secret exec` run here** as it goes, however its run ends: it is no longer
/// tracked, and its host is told it ended.
struct Finishing<'a> {
    heeding: &'a Heeding,
    id: u64,
    exec: u64,
    reply: Reply,
}

impl Drop for Finishing<'_> {
    fn drop(&mut self) {
        // Only this exec's own: a link to a host started again may have handed on another
        // under the same number since.
        lock(&self.heeding.inputs).finished(self.id, self.exec);
        (self.reply)(Replied::Ended);
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

/// Whether `entry`, sent back with the reply to `heard`, is the record of that line: the same
/// kind of line, saying what it said, of the same chat. Nothing else is written through a
/// line's reply.
fn of_the_line(entry: &Entry, heard: &Heard) -> bool {
    match (entry, heard) {
        (Entry::Report { report, .. }, Heard::Report(line)) => report == line,
        (Entry::Tool { call, .. }, Heard::Tool(line)) => call == line,
        (Entry::Refused { chat, .. }, Heard::Refused(line)) => *chat == line.chat,
        _ => false,
    }
}

thread_local! {
    /// What the line this thread's hearing hears has asked to be recorded, while the window
    /// hears one the host handed on ([`carrying`]): `None` when no such line is being heard.
    static CARRIED: RefCell<Option<Option<Entry>>> = const { RefCell::new(None) };
}

/// **Carries `entry` back to the host with the reply to the line this thread is hearing**
/// (#2003): the window's hearing asks the event log for `entry` where it hears a line the host
/// handed on, and the host, which writes the log, records it. `entry` is handed back where no
/// such line is being heard on this thread, or one was carried for it already: the window then
/// writes it as it writes anything else. `None` once it is carried.
pub fn carry(entry: Entry) -> Option<Entry> {
    CARRIED.with(|carried| match &mut *carried.borrow_mut() {
        Some(slot @ None) => {
            *slot = Some(entry);
            None
        }
        _ => Some(entry),
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
