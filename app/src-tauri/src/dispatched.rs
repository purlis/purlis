// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The app's half of what a chat may do with a task it dispatched (#1441): wait for its report,
//! list its tasks, cancel one, and be told when a report lands.
//!
//! The rules are `purlis_core::dispatched`'s, which is plain Rust with tests of its own. This
//! module feeds them the two things only the app has, and does what they answer:
//!
//! - **the app's own record of each chat** ([`crate::chats::Chats::handed_from`]), which is what
//!   says whose task a chat is. An ask names a chat by number, and nothing else it says is read;
//! - **what the app knows of a chat right now** ([`seen`]): its harness, whether that harness
//!   has reported at all, whether a turn is under way or has ended, and whether it has shown
//!   the person a prompt.
//!
//! # What is typed into a chat, and when (D-1441-13)
//!
//! Two lines, both purlis's own sentences and never a chat's words: the one that tells a chat
//! something was left for its next turn (`dispatched::nudge`), and the one that asks a
//! cancelled chat for a short report (`dispatched::CANCEL_PROMPT`). And one key: a cancel's
//! interrupt, which is the key the person would press to stop a turn.
//!
//! **Each goes to a pane only where purlis knows what the pane is showing.** All of:
//!
//! - the chat's harness is one purlis has measured its line in (Claude Code; Codex once
//!   purlis's hooks are trusted there, which is when it reports at all);
//! - the board has heard from the chat since it started. A chat nothing has reported for may
//!   be showing a start-up dialog, a login, anything, and an Enter of purlis's would answer it;
//! - it has shown the person no prompt this turn;
//! - no key of the person's has gone to its pane since its harness last said a turn began or
//!   ended ([`person_typed`]). A harness's own picker, opened by a local command, is reported
//!   by no hook, and the person's key is the only sign of it.
//!
//! A line further waits for the chat's turn to have ended, and the interrupt goes only into a
//! turn purlis heard begin. Anywhere else nothing is typed: a report or a message waits for the
//! chat's next turn, as it always did, and a cancel is recorded and takes effect when the
//! chat's turn or program ends, within the bound a stop has (`crate::stopping`, #1459). A chat
//! that is not ready is looked at again when it next moves ([`moved`]), which the hook channel
//! says.
//!
//! No write to a pane is made under `Chats::deciding()`: a program that has stopped reading
//! its terminal must not hold up every dispatch and report in the project.
//!
//! # Messages (#1442)
//!
//! A follow-up, a progress note, a question and an answer are `purlis_core::dispatchtalk`'s:
//! who may send to whom is the app's record of each chat, the text is held to a report's rule,
//! and each pair is limited to the project's messages a minute. A message is left in the
//! project for the chat it is for and handed to its next turn by its own hook, quoted as data.
//! Nothing a chat wrote is ever typed.
//!
//! **A task question goes to its asking chat first** (ADR 0107), and `dispatchtalk` holds it
//! until it is answered, across a restart too. An answer from the asking chat is refused unless
//! this module holds a question that task asked it: a harness's prompt with no adapter stays a
//! prompt in the task's own tab, which only the person's keys answer.
//!
//! **The person's words in a task's pane are remembered as one bit** ([`person_typed`]), which
//! its report carries as "the operator stepped in". Nothing they typed is kept.
//!
//! # A task ends at its report (#1485)
//!
//! Once a task's report is delivered, purlis ends its program ([`delivered`], [`end_look`]):
//! by the chat's own close, so everything a close settles is settled the one way. **The
//! report first, the end after**: a task becomes one to end only where its report was kept for
//! the chat that asked (`Ledger::reported`), and the end is made on a thread of its own, which
//! takes `Chats::deciding()` as any close does. So a report in flight and the end are ordered
//! by the lock that already orders a report against an exit, and the exit that follows finds
//! nothing owed and says nothing.
//!
//! **When** is the core's rule (`Ledger::end_step`): after the turn that reported has ended,
//! by the harness's `Stop`, and had a moment for the harness to finish writing its
//! conversation down; and after a bounded wait for a turn that does not end, or a harness
//! purlis hears nothing from. Never while a task of its own is at work, never while the person
//! is stopping it (the stop ends it), and never as the app quits.
//!
//! **And never a chat that is working again, mid-turn.** A key of the person's in its pane
//! after the report ([`person_typed`]), or a Smart close of it beginning ([`stand_down`]),
//! stands the end down for good, and its record says so for the next launch. A line purlis
//! types into it starts a turn that is waited for. While it is the chat in front, the end is
//! held until the person moves away ([`front_moved`]). A blocked task and one the person
//! started from a tab are never ones to end. A task that had reported when the app quit is
//! left out of the next launch's put-back and is a finished row
//! ([`crate::finished::put_back_without_the_finished`]).
//!
//! What is left is its dispatch record, which its finished row is read from
//! ([`crate::finished`]), and the ledger's memory of how it ended, which answers a wait.
//! # The person answers a task's question (#1496, V100-46)
//!
//! A question a task put to its asking chat may be answered by the person, in the window
//! ([`answer_task_question`], [`person_answers`]). It is taken under the hold the asking chat's
//! own answer is taken under, so the question is answered once and whoever comes second is
//! told who answered. The task is handed the answer as the person's; the asking chat is told
//! the person answered, with the question and the answer, and its own answer is refused from
//! then on.
//!
//! **Only the window can say the person answered.** [`answer_task_question`] is a command of
//! the window's and of nothing else: it is in no ask a chat's command or hook can send
//! ([`answer`] has no arm that reaches [`person_answers`]), and no link serves it
//! (`purlis_session_protocol::ui::WINDOW_ONLY`). What the person said is kept in this
//! module's memory and handed to each chat on the app's own channel, to the command of the
//! task's that waits or to the chat's next turn when its hook asks ([`What::FromThePerson`]):
//! it is left in no file, because the folder a chat's messages wait in is one a chat may be
//! able to write. Nothing of it is typed into a pane: each chat is typed purlis's one line,
//! where it may be typed one.
//!
//! # A wait holds a thread, never the listener (D-1441-14)
//!
//! [`wait`] runs on the thread the hook channel gives each connection, so the listener and
//! every other chat's lines are untouched by it. It holds the project only weakly and for a
//! bounded time (`dispatched::WAITS_AT_MOST`). One chat may have [`MOST_WAITS_A_CHAT`] waits
//! parked at once, and a wait whose command has hung up ends at its next look, so a process
//! inside a chat cannot spend the app's threads.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant, SystemTime};

use purlis_core::active::Place;
use purlis_core::dispatched::{
    self, Answered, Asked, Ends, Landed, Ledger, Looked, Reply, Row, Seen, Waited, What,
};
use purlis_core::dispatchtalk::{self, Kind, Message};
use purlis_core::hookwire::{Answer, Report};
use purlis_core::reopen::{HandedFrom, Mode, Owed};
use purlis_core::state::{Event, State};

use crate::planes::Held;

/// What one project's app remembers of its dispatched tasks, and who is waiting on them.
#[derive(Debug, Default)]
pub struct Tasks {
    ledger: Mutex<Ledger>,
    /// Told whenever something a wait could end on has happened.
    moved: Condvar,
    /// By chat: how many waits of its are parked on a thread now.
    parked: Mutex<HashMap<u32, usize>>,
    /// The finished tasks being reopened, by their dispatch record's id ([`Tasks::reopen`]).
    reopening: Mutex<HashMap<String, Reopening>>,
    /// Why the last Reopen of a finished task did not hold, by its record's id, for its row.
    not_reopened: Mutex<HashMap<String, String>>,
    /// What is heard of each task, for its quiet notes (#1981).
    quiet: crate::silence::Quiet,
    /// The project's running dispatch records, read again only once the store has changed
    /// (#3932): what a turn's roster finds tasks orphaned by a restart in.
    running: Mutex<purlis_core::dispatchrecord::Running>,
    /// Whether a reported task's end runs on the clock in a test ([`Tasks::on_the_clock`]).
    #[cfg(test)]
    clocked: std::sync::atomic::AtomicBool,
}

/// One finished task being reopened: the chat that was started for it, once one was, and when.
#[derive(Debug, Clone, Copy)]
struct Reopening {
    /// The new chat's number; none while it is being started.
    session: Option<u32>,
    started: Instant,
}

/// How long a reopened chat that purlis never heard from must have lived for its resume to
/// count as having worked. One that ends sooner is a harness that could not bring the
/// conversation back, and the task's row is put back.
pub const A_RESUME_HOLDS_AFTER: Duration = Duration::from_secs(15);

/// What a row says when the chat a Reopen started ended at once.
pub const NOT_RESUMED: &str = "It could not be reopened: its harness ended at once, without \
    bringing the conversation back (it may have been removed). Its report is still here.";

/// The most waits one chat may have parked at once, as it may hold that many live tickets
/// (`hookwire::MOST_LIVE_TICKETS_A_CHAT`): room for a wait on every task it can have running.
pub const MOST_WAITS_A_CHAT: usize = 16;

impl Tasks {
    pub fn ledger(&self) -> MutexGuard<'_, Ledger> {
        self.ledger.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn changed(&self) {
        self.moved.notify_all();
    }

    /// What is heard of each task, for its quiet notes (#1981).
    pub(crate) fn quiet(&self) -> &crate::silence::Quiet {
        &self.quiet
    }

    /// What `each` makes of the running dispatch records of the project at `root` asked for by
    /// `asker` whose chat is nowhere `here` (#3900, #3932): read from this project's copy of its
    /// running records ([`purlis_core::dispatchrecord::Running`]), which reads the store again
    /// only once it has changed, and kept slim. Under a lock of its own, taken and let go here,
    /// and never while another is wanted.
    pub(crate) fn orphaned<T>(
        &self,
        root: &std::path::Path,
        asker: &purlis_core::dispatchrecord::ChatRef,
        here: impl Fn(&purlis_core::dispatchrecord::ChatRef) -> bool,
        each: impl Fn(&purlis_core::dispatchrecord::Record) -> T,
    ) -> Vec<T> {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        running
            .orphaned(root, asker, here)
            .into_iter()
            .map(each)
            .collect()
    }

    fn reopens(&self) -> MutexGuard<'_, HashMap<String, Reopening>> {
        self.reopening
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// **Takes the one Reopen of finished task `id`** (#1485): `false` where one is already
    /// under way, so two presses start one chat. Tested and taken under one lock. Let go by
    /// [`Self::reopen_failed`], or settled when the chat it started is heard from or ends.
    pub(crate) fn reopen_begins(&self, id: &str) -> bool {
        let mut reopening = self.reopens();
        if reopening.contains_key(id) {
            return false;
        }
        reopening.insert(
            id.to_owned(),
            Reopening {
                session: None,
                started: Instant::now(),
            },
        );
        self.not_reopened
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(id);
        true
    }

    /// Whether any Reopen is under way: asked first, so a chat's move costs nothing more.
    fn any_reopening(&self) -> bool {
        !self.reopens().is_empty()
    }

    /// The Reopen of `id` started chat `session`.
    pub(crate) fn reopen_started(&self, id: &str, session: u32) {
        if let Some(entry) = self.reopens().get_mut(id) {
            entry.session = Some(session);
            entry.started = Instant::now();
        }
    }

    /// The Reopen of `id` started nothing: the row is as it was.
    pub(crate) fn reopen_failed(&self, id: &str) {
        self.reopens().remove(id);
    }

    /// Whether a Reopen of `id` is under way: its row is not drawn while one is.
    pub(crate) fn reopening(&self, id: &str) -> bool {
        self.reopens().contains_key(id)
    }

    /// Why the last Reopen of `id` did not hold, where it did not.
    pub(crate) fn not_reopened(&self, id: &str) -> Option<String> {
        self.not_reopened
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .cloned()
    }

    /// Chat `session` was heard from (`ended` false) or has gone (`ended` true): where a
    /// Reopen started it, how that Reopen came out. `Some((id, true))` where the resume
    /// worked, and the task's row is to be cleared; `Some((id, false))` where the chat ended
    /// at once unheard, and the row stays with the reason.
    fn reopen_settles(&self, session: u32, ended: bool) -> Option<(String, bool)> {
        let mut reopening = self.reopens();
        let (id, entry) = reopening
            .iter()
            .find(|(_, entry)| entry.session == Some(session))
            .map(|(id, entry)| (id.clone(), *entry))?;
        reopening.remove(&id);
        drop(reopening);
        let worked = !ended || entry.started.elapsed() >= A_RESUME_HOLDS_AFTER;
        if !worked {
            self.not_reopened
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(id.clone(), NOT_RESUMED.to_owned());
        }
        Some((id, worked))
    }

    /// In a test, a reported task's end waits on no clock unless the test says so here: the
    /// test looks for it ([`end_look`]) when it means the time to have passed. The hundreds of
    /// tests that go on using a task's chat after its report are then not in a race with a
    /// thread that would close it two seconds later.
    #[cfg(test)]
    pub(crate) fn on_the_clock(&self) {
        self.clocked
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// How many waits of chat `chat`'s are parked now, for a test that has to know.
    #[cfg(test)]
    pub(crate) fn parked_now(&self, chat: u32) -> usize {
        self.parked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&chat)
            .copied()
            .unwrap_or_default()
    }

    /// A place among chat `chat`'s parked waits, let go when what this answers is dropped; or
    /// the refusal where it has [`MOST_WAITS_A_CHAT`] already.
    fn park(self: &Arc<Self>, chat: u32) -> Result<Parked, String> {
        let mut parked = self.parked.lock().unwrap_or_else(PoisonError::into_inner);
        let count = parked.entry(chat).or_default();
        if *count >= MOST_WAITS_A_CHAT {
            return Err(format!(
                "chat {chat} has {MOST_WAITS_A_CHAT} waits under way already, and that is the \
                 most one chat may have. Let one end first; `purlis dispatch list` shows where \
                 each task stands without waiting."
            ));
        }
        *count += 1;
        Ok(Parked {
            tasks: Arc::clone(self),
            chat,
        })
    }
}

/// One parked wait of a chat's.
struct Parked {
    tasks: Arc<Tasks>,
    chat: u32,
}

impl Drop for Parked {
    fn drop(&mut self) {
        let mut parked = self
            .tasks
            .parked
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = parked.get_mut(&self.chat) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                parked.remove(&self.chat);
            }
        }
    }
}

/// How often a wait looks again at what no notice tells it of: a chat that closed, a program
/// that ended, a command that hung up, a project that was let go.
const A_WAIT_LOOKS_EVERY: Duration = Duration::from_millis(500);

/// Chat `chat` as the app knows it now, for a module beside this one (#1981).
pub(crate) fn seen_now(held: &Held, chat: u32) -> Seen {
    seen(held, chat)
}

/// Chat `chat` as the app knows it now.
fn seen(held: &Held, chat: u32) -> Seen {
    let open = held
        .chats()
        .open_now()
        .iter()
        .any(|open| open.session == chat);
    let glance = held.board().glance(chat);
    Seen {
        ended: !open || matches!(glance.state, State::Done | State::Failed),
        heard: glance.state != State::Unknown,
        // A turn the board heard begin, which has shown the person no prompt: a prompt mid-turn
        // moves the board to waiting, with `asking` set until the turn ends.
        running: glance.state == State::Running,
        waiting: glance.state == State::Waiting,
        own_work: glance.own_work,
        asking: glance.asking,
        measured: dispatched::told_by_a_line(held.chats().harness(chat)),
        interrupts: dispatched::interrupted_by_escape(held.chats().harness(chat)),
        // A permission ask its hook holds open in the window's needs-you list (HP-6): a prompt
        // in front of the person whether or not the harness said it asked.
        ask_open: held.asks_open_for(chat),
    }
}

fn no(why: String) -> Answer {
    Answer::No { why }
}

fn task(answered: Answered) -> Answer {
    Answer::Task(Box::new(answered))
}

/// The app's record of task `of`, where the chat that asks dispatched it, and its name.
fn owned(held: &Held, asker: u32, of: u32) -> Result<(HandedFrom, String), String> {
    let record = held.chats().handed_from(of);
    let from = dispatched::owned(asker, of, record.as_ref())?.clone();
    let name = held
        .chats()
        .shown_name(of)
        .ok_or_else(|| dispatched::not_yours(of))?;
    Ok((from, name))
}

/// `why`, the refusal for a chat that is not the asker's own open task; or, where `of` is a
/// task of its own that has finished and whose program purlis ended (#1485), the sentence that
/// says so: `asked` is what it wanted of it. Only of its own: the memory asked is keyed by the
/// asking chat, so the refusal still says nothing of any other chat.
fn or_finished(held: &Held, asker: u32, of: u32, why: String, asked: &str) -> String {
    match held.tasks().ledger().gone(asker, of) {
        Some(gone) => dispatched::finished_already(&gone.name, of, gone.report.as_ref(), asked),
        None => why,
    }
}

/// Answers one ask after a dispatched task. `connection` is the listener's number for the
/// connection it came on, which a wait asks after.
pub fn answer(held: &Held, asked: &Asked, connection: u64) -> Answer {
    let asker = asked.chat;
    // What this chat queued while the app did not answer goes first (#3902): a message sent
    // now never overtakes one it sent before, and a list says where each stands.
    if matches!(
        asked.what,
        What::Tell { .. }
            | What::TellAll { .. }
            | What::Note { .. }
            | What::Question { .. }
            | What::Answer { .. }
            | What::List
            | What::Output { .. }
    ) {
        deliver_queued(held, asker, None);
    }
    // **A chat that waits on another does not wait on that one's queue** (#3902): a wait
    // delivers what the task it waits on queued, and a task's wait for its answer what its
    // asking chat queued, each time it looks again ([`wait`], [`await_answer`]).

    match &asked.what {
        What::Wait { .. } => wait(&held.weak(), asked, connection),
        What::AwaitAnswer { .. } => await_answer(&held.weak(), asked, connection),
        What::Read { of } => {
            // A closed task's report is read from the app's memory, and waits in no file.
            // And a task the person asked for from this chat's tab is refused as theirs.
            if let Err(why) = owned(held, asker, *of)
                && held.tasks().ledger().gone(asker, *of).is_none()
            {
                return no(why);
            }
            let files = held.tasks().ledger().read(*of);
            for file in files {
                purlis_core::handback::took(held.root(), &file);
            }
            task(Answered::Noted)
        }
        What::List => task(Answered::Listed {
            rows: list(held, asker),
            messages: records(held, asker),
        }),
        What::Cancel { of } => cancel(held, asker, *of).unwrap_or_else(no),
        What::Output { of } => output(held, asker, *of).unwrap_or_else(no),
        What::Close { of } => close(held, asker, *of).unwrap_or_else(no),
        What::Tell { to, text } => tell(held, asker, *to, text).unwrap_or_else(no),
        What::TellAll { text } => tell_all(held, asker, text).unwrap_or_else(no),
        What::Note { text, ack } => note(held, asker, text, *ack).unwrap_or_else(no),
        What::Deliver { id } => {
            deliver_queued(held, asker, Some(id)).unwrap_or_else(|| what_became_of(held, asker, id))
        }
        What::Question { text } => send_up(held, asker, Kind::Question, text).unwrap_or_else(no),
        What::Answer { to, text } => answer_it(held, asker, *to, text).unwrap_or_else(no),
        What::GotAnswer => {
            let file = held.tasks().ledger().got_answer(asker);
            if let Some(file) = file {
                purlis_core::handback::took(held.root(), &file);
            }
            task(Answered::Noted)
        }
        // What the person said to the chat that asks (#1496), and to no other: the chat is
        // the one whose token the line carried. Read, and kept until it says it has it.
        What::FromThePerson => task(Answered::FromThePerson {
            said: held.tasks().ledger().talk.from_person(asker),
        }),
        // Its turn has those: by the number of the question each is about, never by how
        // many, so what the person said since it read its list stays for the next turn.
        What::HasFromThePerson { numbers } => {
            held.tasks().ledger().handed_from_person(asker, numbers);
            task(Answered::Noted)
        }
        // The reports this chat's turn read, in its own folder or its place's, and no other:
        // taken off disk here, where the folder is the app's to write (#3718's first slice).
        What::TookHandbacks { folder, names } => {
            let place = held
                .chats()
                .chat_at(asker)
                .and_then(|at| at.cwd)
                .map(
                    |cwd| match purlis_core::active::workspace_of_tree(held.root(), &cwd) {
                        Some(ws) => purlis_core::active::Place::Workspace(ws),
                        None => purlis_core::active::Place::PlaneRoot,
                    },
                );
            let its_own = [
                purlis_core::handback::folder_of(purlis_core::handback::For::Chat(asker)),
                place.as_ref().and_then(|place| {
                    purlis_core::handback::folder_of(purlis_core::handback::For::Place(place))
                }),
            ];
            if !its_own.iter().flatten().any(|own| own == folder) {
                return no("those reports are not this chat's to take".to_owned());
            }
            task(Answered::Took {
                taken: purlis_core::handback::took_waiting(held.root(), folder, names),
            })
        }
        // The messages waiting for this chat that its turn read, and no other chat's.
        What::TookMessages { names } => task(Answered::TookMessages {
            taken: purlis_core::dispatchtalk::took(held.root(), asker, names),
        }),
    }
}

/// The name the person sees chat `chat` under, as a message says it is from.
fn shown(held: &Held, chat: u32) -> Result<String, String> {
    held.chats()
        .shown_name(chat)
        .map(|name| dispatchtalk::sender_of(&name))
        .ok_or_else(|| format!("chat {chat} is not one this app has open"))
}

fn kept(why: std::io::Error) -> String {
    format!("the message could not be kept ({why})")
}

/// **The limits in force between asking chat `asker` and its task `task`** (#1439): the
/// project's files as they stand now, for the workspace the asking chat works in and the two
/// personas, under this machine's policy. The one place a message's limit is read.
pub(crate) fn limits_between(
    held: &Held,
    asker: u32,
    task: u32,
) -> purlis_core::dispatchlimits::Limits {
    let open = held.chats().open_now();
    let of = |chat: u32| open.iter().find(|open| open.session == chat);
    purlis_core::dispatchlimits::of(
        held.root(),
        of(asker).and_then(|open| open.workspace.as_deref()),
        of(asker).and_then(|open| open.persona.as_deref()),
        of(task).and_then(|open| open.persona.as_deref()),
    )
}

/// **How many tasks each asking chat of one list of rows may have running at once** (#1491,
/// V100-26): `running-per-chat`, as in force for the workspace the chat works in and the
/// persona it runs as, by the limits function a dispatch is decided by
/// (`purlis_core::dispatchlimits::of`). What a chat's row says it is at: `6 of 6 tasks`.
///
/// Asked only for a chat that has a task open, and the files are read once for each place and
/// persona in the list: a sidebar with no task reads nothing.
pub(crate) struct RunningLimits<'a> {
    held: &'a Held,
    /// The chats that have a task open.
    askers: std::collections::BTreeSet<u32>,
    read: std::collections::HashMap<(Option<String>, Option<String>), u32>,
}

impl<'a> RunningLimits<'a> {
    pub(crate) fn of(held: &'a Held, open: &[crate::chats::Open]) -> Self {
        Self {
            held,
            askers: open
                .iter()
                .filter_map(|one| one.from.as_ref())
                .filter(|from| from.mode == Mode::Task)
                .map(|from| from.chat)
                .collect(),
            read: std::collections::HashMap::new(),
        }
    }

    /// How many tasks `chat` has running against that limit, where it has a task open: the
    /// count a dispatch from it is decided over (`dispatchdecision::lineage_of`), a chat
    /// still starting included, so `6 of 6 tasks` is said exactly when a seventh is refused.
    pub(crate) fn running(&self, chat: &crate::chats::Open) -> Option<u32> {
        use purlis_core::dispatchdecision::{lineage_of, pair_of};
        if !self.askers.contains(&chat.session) {
            return None;
        }
        let held = self.held;
        let default = purlis_core::start::persona_for_a_new_chat(held.root());
        let session = chat.session;
        Some(held.chats().deciding_over(|open, starting| {
            let pair = open
                .iter()
                .find(|(number, _)| *number == session)
                .map(|(_, chat)| pair_of(chat, None, default.as_deref()))
                .unwrap_or_default();
            lineage_of(
                session,
                open,
                default.as_deref(),
                &|n| starting(n) || crate::handoff::still_working(held, n),
                &pair,
            )
            .running
        }))
    }

    /// The limit in force for `chat`, where it has a task open.
    pub(crate) fn of_chat(&mut self, chat: &crate::chats::Open) -> Option<u32> {
        if !self.askers.contains(&chat.session) {
            return None;
        }
        let held = self.held;
        let key = (chat.workspace.clone(), chat.persona.clone());
        Some(
            *self
                .read
                .entry(key)
                .or_insert_with_key(|(workspace, persona)| {
                    purlis_core::dispatchlimits::of(
                        held.root(),
                        workspace.as_deref(),
                        persona.as_deref(),
                        None,
                    )
                    .running
                }),
        )
    }
}

/// **Chat `chat`'s key in the record of messages** (#3902, #3904): its id, which it keeps across
/// a restart of the app. A chat with none is keyed for this run of the app alone, never by its
/// number alone: a later run reuses numbers, and an acknowledgement must never match an
/// instruction another chat was sent under the same number.
fn key(held: &Held, chat: u32) -> String {
    static RUN: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    crate::dispatches::chat_ref(held, chat)
        .and_then(|chat| chat.id)
        .unwrap_or_else(|| {
            let run = RUN.get_or_init(|| {
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_or(0, |since| since.as_nanos())
            });
            format!("run-{run}-chat-{chat}")
        })
}

/// The ledger, with its record of messages read from where it is kept beside the queues
/// ([`purlis_core::dispatchqueue::records_for`]) the first time it is needed.
fn ledger(held: &Held) -> MutexGuard<'_, Ledger> {
    let mut ledger = held.tasks().ledger();
    if let Some(socket) = held.hooks().socket() {
        ledger
            .talk
            .keep_at(&purlis_core::dispatchqueue::records_for(socket));
    }
    ledger
}

/// Keeps the record of a message chat `sender` sent: of `kind`, instruction `instruction`, and
/// for each chat it was for, what became of it there.
fn kept_record(
    held: &Held,
    sender: u32,
    kind: Kind,
    instruction: Option<u32>,
    to: Vec<(u32, String, dispatchtalk::Delivered)>,
) {
    let to_all = to.len() > 1;
    record_with(held, sender, kind, instruction, to_all, to);
}

fn record_with(
    held: &Held,
    sender: u32,
    kind: Kind,
    instruction: Option<u32>,
    to_all: bool,
    to: Vec<(u32, String, dispatchtalk::Delivered)>,
) {
    let to = to
        .into_iter()
        .map(|(chat, name, state)| dispatchtalk::Delivery {
            chat,
            key: if chat == 0 {
                String::new()
            } else {
                key(held, chat)
            },
            name,
            state,
        })
        .collect();
    let sender = key(held, sender);
    ledger(held).talk.record(
        &sender,
        dispatchtalk::Record {
            id: None,
            kind,
            instruction,
            to_all,
            to,
            queued: false,
        },
    );
}

/// A follow-up from chat `asker` to task `to`: refused unless `asker` dispatched it and it is
/// still working and not being cancelled, then left for its next turn.
fn tell(held: &Held, asker: u32, to: u32, said: &str) -> Result<Answer, String> {
    let (name, number) = tell_numbered(held, asker, to, said, None)?;
    record_with(
        held,
        asker,
        Kind::FollowUp,
        Some(number),
        false,
        vec![(to, name.clone(), dispatchtalk::Delivered::Delivered)],
    );
    Ok(task(Answered::Sent {
        kind: Kind::FollowUp,
        to: name,
        instruction: Some(number),
    }))
}

/// The tasks of chat `asker` a `tell --all` is for: listed under it, not finished, and not one
/// the person started from its tab.
fn live_tasks(held: &Held, asker: u32) -> Vec<(u32, String)> {
    list(held, asker)
        .into_iter()
        .filter(|row| !row.finished && !row.by_person && row.chat != 0)
        .map(|row| (row.chat, row.name))
        .collect()
}

/// **One follow-up from chat `asker` to every task of its that is still working** (#3904):
/// one instruction, numbered once, delivered to each as [`tell`] delivers it. A task it cannot
/// reach (finished, being cancelled, at the pair's limit) is said with why, beside those it
/// reached. Refused only where it reached none.
fn tell_all(held: &Held, asker: u32, said: &str) -> Result<Answer, String> {
    let text = dispatchtalk::text(said)?;
    let tasks = live_tasks(held, asker);
    if tasks.is_empty() {
        return Err(NO_LIVE_TASKS.to_owned());
    }
    let asker_key = key(held, asker);
    let number = ledger(held).talk.instruction(&asker_key);
    let to: Vec<(u32, String, dispatchtalk::Delivered)> = tasks
        .into_iter()
        .map(|(chat, name)| {
            let state = match tell_numbered(held, asker, chat, &text, Some(number)) {
                Ok(_) => dispatchtalk::Delivered::Delivered,
                Err(why) => dispatchtalk::Delivered::Undeliverable { why },
            };
            (chat, name, state)
        })
        .collect();
    if to
        .iter()
        .all(|(_, _, state)| matches!(state, dispatchtalk::Delivered::Undeliverable { .. }))
    {
        let why: Vec<String> = to.iter().map(|(_, _, state)| state.say()).collect();
        return Err(format!(
            "no task of this chat's could be sent it. Nothing was sent: {}",
            why.join("; ")
        ));
    }
    record_with(held, asker, Kind::FollowUp, Some(number), true, to.clone());
    Ok(task(Answered::SentAll {
        instruction: number,
        to: to
            .into_iter()
            .map(|(chat, name, state)| dispatchtalk::Delivery {
                chat,
                key: key(held, chat),
                name,
                state,
            })
            .collect(),
    }))
}

/// What `tell --all` is told where the chat has no task still working.
const NO_LIVE_TASKS: &str = "this chat has no task still working to tell: `purlis dispatch \
    list` shows its tasks. Nothing was sent.";

/// A progress note from task `sender` to its asking chat, acknowledging instruction `ack` of
/// that chat's where it names one (#3904). With an acknowledgement, the note may be empty: the
/// acknowledgement is recorded, and nothing more is sent.
fn note(held: &Held, sender: u32, said: &str, ack: Option<u32>) -> Result<Answer, String> {
    let Some(number) = ack else {
        return send_up(held, sender, Kind::Note, said);
    };
    let record = held.chats().handed_from(sender);
    let from = dispatchtalk::up(record.as_ref())?.clone();
    let (asker, task_key) = (key(held, from.chat), key(held, sender));
    ledger(held).talk.acknowledge(&asker, &task_key, number)?;
    held.tasks().changed();
    if said.trim().is_empty() {
        return Ok(task(Answered::Acknowledged {
            instruction: number,
            to: held
                .chats()
                .shown_name(from.chat)
                .unwrap_or_else(|| from.name.clone()),
        }));
    }
    send_up(held, sender, Kind::Note, said)
}

/// **The lock one drain of chat `chat`'s queue holds** (#3902): one drain of a chat at a time,
/// so no message is delivered twice; drains of different chats run side by side.
///
/// **Lock order**: a drain's lock is taken before the ledger's and never while the ledger is
/// held, and a drain never takes another chat's drain lock (nothing a delivery runs drains).
fn draining(chat: u32) -> Arc<Mutex<()>> {
    static DRAINS: Mutex<std::collections::BTreeMap<u32, Arc<Mutex<()>>>> =
        Mutex::new(std::collections::BTreeMap::new());
    let mut drains = DRAINS.lock().unwrap_or_else(PoisonError::into_inner);
    // A drain lock nobody holds is let go of, so the map holds only those in use.
    drains.retain(|_, lock| Arc::strong_count(lock) > 1);
    Arc::clone(drains.entry(chat).or_default())
}

/// How a drain treats what a limit holds back.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Drain {
    /// It waits, with everything after it, for the next drain.
    Waits,
    /// The sender is closing: nothing is held back, and what cannot be delivered is reported
    /// where the person reads it.
    Closing,
}

/// **Delivers what chat `sender` queued while the app did not answer** (#3902,
/// [`purlis_core::dispatchqueue`]), oldest first, each once: removed as it is delivered or found
/// undeliverable ([`draining`]).
///
/// A message a limit holds back stays, with every one after it, for the next drain: order is
/// kept. One that cannot be delivered (its task has finished, it is not the sender's task, it
/// does not check) is removed and recorded undeliverable, with why, for `purlis dispatch list`;
/// but for `fresh`, the one the asking command has just queued, whose answer is its own, as a
/// live send's is.
///
/// Answers `fresh`'s fate where this drain found it: `None` where it did not (another drain
/// took it first), for [`what_became_of`] to answer.
fn deliver_queued(held: &Held, sender: u32, fresh: Option<&str>) -> Option<Answer> {
    drain(held, sender, fresh, Drain::Waits)
}

fn drain(held: &Held, sender: u32, fresh: Option<&str>, how: Drain) -> Option<Answer> {
    let socket = held.hooks().socket()?.to_path_buf();
    if !purlis_core::dispatchqueue::holds(&socket, sender) {
        return None;
    }
    // **Nothing is typed into a pane while the drain lock is held** (the rule at the top of this
    // module): the chats each delivery wakes are collected, and woken once it is let go. A
    // closing chat's drain collects them already, for after the close's hold ([`sender_ending`]).
    let collects = WAKES.with(|wakes| {
        let mut wakes = wakes.borrow_mut();
        let outer = wakes.is_none();
        if outer {
            *wakes = Some(Vec::new());
        }
        outer
    });
    let answered = drain_locked(held, &socket, sender, fresh, how);
    if collects {
        let wakes = WAKES
            .with(|wakes| wakes.borrow_mut().take())
            .unwrap_or_default();
        for wake in wakes {
            wake_now(held, wake);
        }
    }
    answered
}

/// [`drain`], under chat `sender`'s drain lock ([`draining`]), which is let go as it returns.
fn drain_locked(
    held: &Held,
    socket: &std::path::Path,
    sender: u32,
    fresh: Option<&str>,
    how: Drain,
) -> Option<Answer> {
    use purlis_core::dispatchqueue::{self, Found};
    let socket = socket.to_path_buf();
    let lock = draining(sender);
    let _one = lock.lock().unwrap_or_else(PoisonError::into_inner);
    let sender_key = key(held, sender);
    let mut answered = None;
    for found in dispatchqueue::waiting(&socket, sender) {
        let message = match found {
            Found::Message(message) => message,
            // A copy of one delivered before: the floor was raised before it was, so an app
            // that stopped between the two leaves it behind. It is not delivered again.
            Found::Delivered(path) => {
                dispatchqueue::drop_unchecked(&path);
                continue;
            }
            Found::Unchecked(path) => {
                tracing::warn!(
                    "purlis: a message in chat {sender}'s queue does not check, so it is removed \
                     and not delivered ({})",
                    path.display()
                );
                dispatchqueue::drop_unchecked(&path);
                record_with(
                    held,
                    sender,
                    Kind::FollowUp,
                    None,
                    false,
                    vec![(
                        0,
                        "a message in this chat's queue".to_owned(),
                        dispatchtalk::Delivered::Undeliverable {
                            why: DID_NOT_CHECK.to_owned(),
                        },
                    )],
                );
                held.tasks().changed();
                continue;
            }
        };
        let is_fresh = fresh == Some(message.id.as_str());
        if how == Drain::Waits
            && let Some(limit) = held_back(held, sender, &message.what)
        {
            if !is_fresh {
                // It waits, and so does every message after it.
                break;
            }
            dispatchqueue::delivered(&socket, sender, &message);
            answered = Some(no(limit));
            break;
        }
        let before = ledger(held).talk.recorded(&sender_key);
        // **At most once**: the floor is raised and the file removed before the message is
        // delivered, so an app that stops in between never delivers it a second time.
        dispatchqueue::delivered(&socket, sender, &message);
        let outcome = delivered_now(held, sender, &message.what);
        match outcome {
            Ok(answer) => {
                let mut ledger = ledger(held);
                // Only where this delivery kept a record: an acknowledgement alone keeps none.
                if ledger.talk.recorded(&sender_key) > before {
                    ledger.talk.mark_last_queued(&sender_key, &message.id);
                }
                drop(ledger);
                if is_fresh {
                    answered = Some(answer);
                }
            }
            Err(why) if is_fresh => answered = Some(no(why)),
            Err(why) if how == Drain::Closing => not_delivered(held, sender, &message, why),
            Err(why) => undeliverable(held, sender, &message, why),
        }
    }
    answered
}

/// What a message that did not check is said to be, in its sender's list.
const DID_NOT_CHECK: &str = "it did not check under a key the app issued this chat: it was \
    changed, written by something else, or is a copy of one already delivered, so it was \
    removed";

/// The ask a queued message is, made now.
fn delivered_now(
    held: &Held,
    sender: u32,
    what: &purlis_core::dispatchqueue::Queued,
) -> Result<Answer, String> {
    use purlis_core::dispatchqueue::Queued;
    match what {
        Queued::Tell { to, text } => tell(held, sender, *to, text),
        Queued::TellAll { text } => tell_all(held, sender, text),
        Queued::Note { text, ack } => note(held, sender, text.as_deref().unwrap_or_default(), *ack),
        Queued::Question { text } => send_up(held, sender, Kind::Question, text),
        Queued::Answer { to, text } => answer_it(held, sender, *to, text),
    }
}

/// **What became of queued message `id` of chat `sender`'s**, where no drain of this ask found
/// it (#3902): still queued, delivered, or undeliverable with why, by the queue and the record.
fn what_became_of(held: &Held, sender: u32, id: &str) -> Answer {
    use purlis_core::dispatchqueue::{self, Found};
    let waits = held.hooks().socket().is_some_and(|socket| {
        dispatchqueue::waiting(socket, sender)
            .iter()
            .any(|found| matches!(found, Found::Message(message) if message.id == id))
    });
    if waits {
        return task(Answered::Queued {
            id: id.to_owned(),
            delivered: false,
        });
    }
    let sender_key = key(held, sender);
    let records = ledger(held).talk.records(&sender_key);
    let Some(record) = records
        .into_iter()
        .rev()
        .find(|record| record.id.as_deref() == Some(id))
    else {
        return no(format!(
            "the app holds no record of the message this chat queued as {id}: `purlis \
             dispatch list` shows where this chat's messages stand"
        ));
    };
    match record.to.iter().find_map(|to| match &to.state {
        dispatchtalk::Delivered::Undeliverable { why } => Some(why.clone()),
        _ => None,
    }) {
        Some(why) if record.to.len() == 1 => no(why),
        _ => task(Answered::Queued {
            id: id.to_owned(),
            delivered: true,
        }),
    }
}

/// The kind of a queued message, and the chat it names, where it names one.
fn queued_to(what: &purlis_core::dispatchqueue::Queued) -> (Kind, Option<u32>) {
    use purlis_core::dispatchqueue::For;
    let to = match what.to() {
        For::Task(to) => Some(to),
        For::Tasks | For::Asker => None,
    };
    (what.kind(), to)
}

/// A queued message the app could not deliver, recorded for its sender's list (#3902).
fn undeliverable(
    held: &Held,
    sender: u32,
    message: &purlis_core::dispatchqueue::Waiting,
    why: String,
) {
    let (kind, to) = queued_to(&message.what);
    let name = to
        .and_then(|to| held.chats().shown_name(to))
        .or_else(|| {
            held.chats()
                .handed_from(sender)
                .map(|from| from.name.clone())
        })
        .unwrap_or_else(|| "its asking chat".to_owned());
    let sender_key = key(held, sender);
    record_with(
        held,
        sender,
        kind,
        None,
        matches!(
            message.what,
            purlis_core::dispatchqueue::Queued::TellAll { .. }
        ),
        vec![(
            to.unwrap_or(0),
            name,
            dispatchtalk::Delivered::Undeliverable { why },
        )],
    );
    ledger(held).talk.mark_last_queued(&sender_key, &message.id);
    held.tasks().changed();
}

/// **A message chat `sender` queued and closed before it could be delivered** (#3902): kept on
/// the dispatch record of the task it is between, with why, where the person reads it in the
/// Dispatches tab. A message between no task and its asking chat is said in the log.
fn not_delivered(
    held: &Held,
    sender: u32,
    message: &purlis_core::dispatchqueue::Waiting,
    why: String,
) {
    let (kind, to) = queued_to(&message.what);
    let text = message.what.text().map_or_else(
        || {
            message
                .what
                .acknowledges()
                .map(|n| format!("(acknowledges instruction {n})"))
                .unwrap_or_default()
        },
        str::to_owned,
    );
    // The task the message is between: the one it names, or the sender itself going up.
    let task = to.or_else(|| {
        matches!(kind, Kind::Note | Kind::Question)
            .then_some(sender)
            .filter(|_| held.chats().handed_from(sender).is_some())
    });
    match task {
        Some(task) => crate::dispatches::not_delivered(held, task, kind, &text, &why),
        None => tracing::warn!(
            "purlis: chat {sender} closed with a {} queued that could not be delivered ({why})",
            kind.word()
        ),
    }
}

/// **The limit's refusal, where a pair a queued message is between has reached it now**
/// (#3902): the message waits for the next drain rather than being dropped. A `tell --all`
/// waits while any of the sender's live tasks' pairs is at its limit.
fn held_back(
    held: &Held,
    sender: u32,
    what: &purlis_core::dispatchqueue::Queued,
) -> Option<String> {
    use purlis_core::dispatchqueue::Queued;
    let pairs: Vec<(u32, u32)> = match what {
        Queued::Tell { to, .. } | Queued::Answer { to, .. } => vec![(sender, *to)],
        Queued::Note { text, .. } if text.as_deref().is_none_or(|t| t.trim().is_empty()) => {
            return None;
        }
        Queued::Note { .. } | Queued::Question { .. } => {
            vec![(held.chats().handed_from(sender)?.chat, sender)]
        }
        Queued::TellAll { .. } => live_tasks(held, sender)
            .into_iter()
            .map(|(task, _)| (sender, task))
            .collect(),
    };
    pairs.into_iter().find_map(|(asker, task)| {
        let limits = limits_between(held, asker, task);
        let sent = held
            .tasks()
            .ledger()
            .talk
            .in_the_last_minute(asker, task, Instant::now());
        dispatchtalk::may_send(&limits, sent).err()
    })
}

/// What chat `sender` sent lately, and what it still has queued (#3902, #3904).
fn records(held: &Held, sender: u32) -> Vec<dispatchtalk::Record> {
    use purlis_core::dispatchqueue::{self, Found};
    // The chats are asked before the ledger is taken.
    let sender_key = key(held, sender);
    let mut records = ledger(held).talk.records(&sender_key);
    let Some(socket) = held.hooks().socket() else {
        return records;
    };
    for found in dispatchqueue::waiting(socket, sender) {
        let Found::Message(message) = found else {
            continue;
        };
        let (kind, to) = queued_to(&message.what);
        let to_all = matches!(
            message.what,
            purlis_core::dispatchqueue::Queued::TellAll { .. }
        );
        let to = if to_all { None } else { Some(to.unwrap_or(0)) };
        records.push(dispatchtalk::Record {
            id: Some(message.id.clone()),
            kind,
            instruction: None,
            to_all,
            to: to
                .map(|chat| dispatchtalk::Delivery {
                    chat,
                    key: String::new(),
                    name: if chat == 0 {
                        "its asking chat".to_owned()
                    } else {
                        held.chats()
                            .shown_name(chat)
                            .unwrap_or_else(|| format!("chat {chat}"))
                    },
                    state: dispatchtalk::Delivered::Queued,
                })
                .into_iter()
                .collect(),
            queued: true,
        });
    }
    records
}

/// **Delivers what every chat queued**, once the project's chats and their tasks are back
/// (#3902): what a chat queued while the app was away reaches its task with nobody acting.
///
/// **Off the launch path**: a thread of its own, started as the launch finishes, so nothing is
/// added to the time the window takes to come up; [`deliver_all_queued_now`] is the drain.
pub(crate) fn deliver_all_queued(held: &Held) {
    let Some(socket) = held.hooks().socket() else {
        return;
    };
    // Nothing queued and nothing left, nothing started: a look at two folders.
    if purlis_core::dispatchqueue::chats(socket).is_empty()
        && purlis_core::dispatchqueue::undelivered::to_place(socket).is_empty()
    {
        return;
    }
    let weak = held.weak();
    let spawned = std::thread::Builder::new()
        .name("purlis-queue-at-open".into())
        .spawn(move || {
            if let Some(held) = weak.upgrade() {
                deliver_all_queued_now(&held);
            }
        });
    if let Err(why) = spawned {
        tracing::warn!(
            "purlis: what chats queued while the app was away is delivered when each is next \
             heard from ({why})"
        );
    }
}

/// [`deliver_all_queued`], on this thread.
pub(crate) fn deliver_all_queued_now(held: &Held) {
    let Some(socket) = held.hooks().socket() else {
        return;
    };
    for chat in purlis_core::dispatchqueue::chats(socket) {
        if held.chats().shown_name(chat).is_some() {
            deliver_queued(held, chat, None);
        }
    }
    place_undelivered(held);
}

/// **What chats that did not come back left queued is recorded where the person reads it**
/// (#3958, [`purlis_core::dispatchqueue::undelivered`]): the spool set it aside as it cleared
/// their queues, when the project opened without them.
///
/// Each message is kept, marked not delivered, on the running dispatch record it was between
/// ([`kept_on_its_dispatch`]). What finds no running record is kept as one notice in the Inbox
/// a sender, naming it and how many
/// ([`purlis_core::dispatchqueue::undelivered::keep_as_notice`]), so nothing is cleared with no
/// record the person can see. A message is placed once: a report an app that stopped left
/// behind places nothing a record or a notice holds already, by id.
pub(crate) fn place_undelivered(held: &Held) {
    use purlis_core::dispatchqueue::undelivered::{self, Message};
    let Some(socket) = held.hooks().socket().map(std::path::Path::to_path_buf) else {
        return;
    };
    let reports = undelivered::to_place(&socket);
    if reports.is_empty() {
        return;
    }
    let records = purlis_core::dispatchrecord::list(held.root());
    let mut seen = std::collections::HashSet::new();
    for (path, report) in reports {
        let unplaced: Vec<Message> = report
            .messages
            .iter()
            // One message in two reports is placed from the first.
            .filter(|message| seen.insert(message.id.clone()))
            .filter(|message| !kept_on_its_dispatch(held, &records, report.sender, message))
            .cloned()
            .collect();
        if unplaced.is_empty() {
            undelivered::placed(&path);
            continue;
        }
        // The sender's name as the newest record of it says it, where one does.
        let name = records
            .iter()
            .flat_map(|record| [&record.worker.chat, &record.asker.chat])
            .find(|chat| chat.chat == report.sender)
            .map(|chat| chat.name.clone());
        undelivered::keep_as_notice(&path, &report, unplaced, name);
    }
    held.tasks().changed();
}

/// **Keeps chat `sender`'s undelivered `message` on the running dispatch records it was
/// between** (#3958), found in `records` by the numbers the chats had when it was queued, and
/// started no later than it was: a follow-up or an answer on the record of the task it names
/// that its sender asked for, a note or a question on the sender's own record as a task, a
/// follow-up to every task on each running record its sender asked for. Whether any record
/// holds it now, kept by this look or by one before it.
fn kept_on_its_dispatch(
    held: &Held,
    records: &[purlis_core::dispatchrecord::Record],
    sender: u32,
    message: &purlis_core::dispatchqueue::undelivered::Message,
) -> bool {
    use purlis_core::dispatchqueue::For;
    let queued_at = chrono::DateTime::from_timestamp(message.at, 0);
    let between = |record: &&purlis_core::dispatchrecord::Record| {
        record.running()
            && chrono::DateTime::parse_from_rfc3339(&record.started)
                .ok()
                .zip(queued_at)
                .is_some_and(|(started, queued)| started <= queued)
    };
    let on: Vec<&purlis_core::dispatchrecord::Record> = match message.what.to() {
        For::Task(to) => records
            .iter()
            .filter(between)
            .find(|record| record.worker.chat.chat == to && record.asker.chat.chat == sender)
            .into_iter()
            .collect(),
        For::Tasks => records
            .iter()
            .filter(between)
            .filter(|record| record.asker.chat.chat == sender)
            .collect(),
        For::Asker => records
            .iter()
            .filter(between)
            .find(|record| record.worker.chat.chat == sender)
            .into_iter()
            .collect(),
    };
    let mut kept = false;
    for record in on {
        kept |= crate::dispatches::undelivered_on(held, record, message);
    }
    kept
}

/// **Chat `chat` is closing** (#3902): what it still has queued is delivered now, held back by
/// no limit, and what cannot be is kept on the dispatch record it is about, where the person
/// reads it ([`not_delivered`]). The spool clears its folder as the chat ends. Then the records
/// of what it sent go with it.
///
/// **Nothing is typed into a pane here**: a close runs under `Chats::deciding()`. Each chat a
/// delivery would wake ([`woken`]) is woken by a thread of its own once that hold is let go.
pub(crate) fn sender_ending(held: &Held, chat: u32) {
    WAKES.with(|wakes| *wakes.borrow_mut() = Some(Vec::new()));
    drain(held, chat, None, Drain::Closing);
    let wakes = WAKES
        .with(|wakes| wakes.borrow_mut().take())
        .unwrap_or_default();
    let key = key(held, chat);
    ledger(held).talk.forget_records(&key);
    if wakes.is_empty() {
        return;
    }
    let weak = held.weak();
    let spawned = std::thread::Builder::new()
        .name("purlis-queue-wake".into())
        .spawn(move || {
            let Some(held) = weak.upgrade() else { return };
            // Once the close that drained them has let go of the hold.
            drop(held.chats().deciding());
            for wake in wakes {
                wake_now(&held, wake);
            }
        });
    if let Err(why) = spawned {
        tracing::warn!(
            "purlis: the chats a closing chat's messages were left for are not woken ({why}); \
             each reads them on its next turn"
        );
    }
}

/// A chat a delivery wakes: typed the line, where it takes one now.
enum Wake {
    Told(u32),
    ToldOrSettled(u32),
}

thread_local! {
    /// Where a drain on this thread collects the chats it would wake, so none is typed into
    /// under its drain lock ([`drain`]) or a close's `Chats::deciding()` ([`sender_ending`]);
    /// `None` everywhere else.
    static WAKES: std::cell::RefCell<Option<Vec<Wake>>> = const { std::cell::RefCell::new(None) };
}

/// **`wake`, now, or once the drain or the close it is under has let go** ([`drain`],
/// [`sender_ending`]): what every delivery wakes its reader by.
fn woken(held: &Held, wake: Wake) {
    let wake = WAKES.with(|wakes| match wakes.borrow_mut().as_mut() {
        Some(later) => {
            later.push(wake);
            None
        }
        None => Some(wake),
    });
    if let Some(wake) = wake {
        wake_now(held, wake);
    }
}

/// Wakes the chat `wake` names, now.
fn wake_now(held: &Held, wake: Wake) {
    match wake {
        Wake::Told(chat) => {
            tell_the_chat(held, chat);
        }
        Wake::ToldOrSettled(chat) => told_or_settled(held, chat),
    }
}

/// [`tell`], as instruction `number`, or one numbered here once every check has let it
/// through: the task's name and the number, once the follow-up is left for it.
fn tell_numbered(
    held: &Held,
    asker: u32,
    to: u32,
    said: &str,
    number: Option<u32>,
) -> Result<(String, u32), String> {
    let (from, name) = owned(held, asker, to)
        .map_err(|why| or_finished(held, asker, to, why, "no turn left to read a message in"))?;
    let seen = seen(held, to);
    let state = held.tasks().ledger().state(to, &from, seen);
    if seen.ended || state == dispatched::State::Cancelling {
        return Err(format!(
            "'{name}' (chat {to}) has finished: it is {}. A message would reach no turn of its \
             work. Dispatch a new task for more.",
            state.say()
        ));
    }
    // A task that reported while its program still runs is told too (#3899).
    dispatchtalk::still_working(&name, to, &from, seen, &state.say())?;
    let text = dispatchtalk::text(said)?;
    let from_name = shown(held, asker)?;
    let limits = limits_between(held, asker, to);
    let asker_key = key(held, asker);
    let number = {
        let mut ledger = ledger(held);
        ledger.talk.count(asker, to, &limits, Instant::now())?;
        number.unwrap_or_else(|| ledger.talk.instruction(&asker_key))
    };
    let message = Message {
        kind: Kind::FollowUp,
        from: from_name,
        chat: asker,
        text,
        instruction: Some(number),
    };
    dispatchtalk::leave(held.root(), to, &message).map_err(kept)?;
    held.tasks().ledger().landed(to, Landed::FollowUp);
    // One more message on the task's dispatch record (#1452), now that it is kept.
    crate::dispatches::message(held, to, &message);
    woken(held, Wake::Told(to));
    Ok((name, number))
}

/// **Whether task `task`, with record `from`, may still talk with the chat that asked for it**:
/// send it a progress note or a question, and be answered. While it owes its report, and
/// while it may send one more (`Owed::Again`, #1768): its report came out blocked, or the
/// person took it up again after it, and it may need to ask before it can finish (#1786).
/// Not once the person's stop is recorded on a task owed one more, which takes no report
/// then either (`handoff::report_under`). Its later report ends this, as its first would have.
fn talks(held: &Held, task: u32, from: &HandedFrom) -> bool {
    match from.report {
        Owed::Due => true,
        Owed::Again => !held.stopping().is_stopping(task),
        Owed::Nothing | Owed::Sent | Owed::Failed => false,
    }
}

/// A progress note or a question from task `sender` to the chat that dispatched it. The
/// recipient is the app's record of `sender`, never anything the message says.
fn send_up(held: &Held, sender: u32, kind: Kind, said: &str) -> Result<Answer, String> {
    let record = held.chats().handed_from(sender);
    let from = dispatchtalk::up(record.as_ref())?;
    if !talks(held, sender, from) {
        return Err(dispatchtalk::ALREADY_REPORTED.to_owned());
    }
    let asker = from.chat;
    if seen(held, asker).ended {
        return Err(format!(
            "'{}', the chat that asked for this task, has closed, so nobody is left to read a \
             {}. Finish what you can and send the task's report, which is kept for its \
             workspace.",
            from.name,
            kind.word()
        ));
    }
    let text = dispatchtalk::text(said)?;
    let message = Message {
        kind,
        from: shown(held, sender)?,
        chat: sender,
        text: text.clone(),
        instruction: None,
    };
    let limits = limits_between(held, asker, sender);
    let sender_key = key(held, sender);
    {
        // One hold from "has it a question open" to "this is its question", so of two
        // questions sent at once one is asked and the other refused with nothing left behind.
        let mut ledger = ledger(held);
        if kind == Kind::Question && ledger.talk.asks(sender).is_some() {
            return Err(
                "this chat already has a question waiting for its asking chat's answer, and is \
                 paused on it. Nothing was sent."
                    .to_owned(),
            );
        }
        ledger.talk.count(asker, sender, &limits, Instant::now())?;
        let file = dispatchtalk::leave(held.root(), asker, &message).map_err(kept)?;
        if kind == Kind::Question {
            ledger.talk.known_as(sender, &sender_key, asker);
            ledger.talk.ask(sender, &text, Some(file))?;
            ledger.landed(asker, Landed::Question(sender));
        }
        // Its last note is what its list row shows (#3905).
        if kind == Kind::Note {
            ledger.noted(sender, &text, SystemTime::now());
        }
    }
    // Kept, so it counts on this task's dispatch record (#1452): a note as a question does.
    crate::dispatches::message(held, sender, &message);
    // When it last sent a note, for its liveness (#3900).
    if kind == Kind::Note {
        held.tasks().ledger().heard_doing(
            sender,
            purlis_core::dispatchliveness::Did::SentANote,
            SystemTime::now(),
        );
    }
    if kind == Kind::Question {
        // Its row says whom it is asking from now (#1484): no hook reports a question.
        held.rows_changed();
        // A command of the asking chat's that waits on this task is answered with it; a chat
        // that is waiting for the person is typed the line. A note is neither: it is read on
        // the asking chat's next turn, and starts none.
        held.tasks().changed();
        // The asking chat is typed the line where it takes one. Where it does not, and its
        // turn's end was held for this task, it is the one who must act and the person the
        // one who can see that (#1491): this task no longer counts toward its wait.
        woken(held, Wake::ToldOrSettled(asker));
    }
    let to = held
        .chats()
        .shown_name(asker)
        .unwrap_or_else(|| from.name.clone());
    kept_record(
        held,
        sender,
        kind,
        None,
        vec![(asker, to.clone(), dispatchtalk::Delivered::Delivered)],
    );
    Ok(task(Answered::Sent {
        kind,
        to,
        instruction: None,
    }))
}

/// Chat `asker` answers the question task `to` asked it. Refused unless `asker` dispatched
/// `to`, `to` is still working, and the app holds a question `to` asked that is still open.
///
/// **Nothing an answer says is typed into `to`**, whatever its tab is showing: the answer is
/// left for its next turn, and handed to the command of its that waits. A prompt the task is
/// showing the person is not a question held here, so it cannot be answered this way.
fn answer_it(held: &Held, asker: u32, to: u32, said: &str) -> Result<Answer, String> {
    let (from, name) = owned(held, asker, to)
        .map_err(|why| or_finished(held, asker, to, why, "no question left to answer"))?;
    {
        // Where the person answered it first (#1496), the refusal says so; and where they
        // answered an earlier question this chat has not been told of, it waits a turn.
        let ledger = held.tasks().ledger();
        if !talks(held, to, &from) {
            return Err(ledger.talk.nothing_to_answer(&name, to));
        }
        ledger.talk.may_answer(to, asker, &name)?;
    }
    let text = dispatchtalk::text(said)?;
    let asker_name = shown(held, asker)?;
    let message = Message {
        kind: Kind::Answer,
        from: asker_name.clone(),
        chat: asker,
        text: text.clone(),
        instruction: None,
    };
    let limits = limits_between(held, asker, to);
    let number = {
        // The hold the person's own answer is taken under ([`person_answers`]): of the two,
        // one is the answer and the other is told who gave it.
        let mut ledger = held.tasks().ledger();
        // Asked again under the hold, before anything is counted or left.
        ledger.talk.may_answer(to, asker, &name)?;
        ledger.talk.count(asker, to, &limits, Instant::now())?;
        let file = dispatchtalk::leave(held.root(), to, &message).map_err(kept)?;
        let given = dispatchtalk::Given {
            from: asker_name,
            text,
            file: Some(file),
            by_person: false,
        };
        let number = ledger.talk.answer(to, asker, &name, given)?;
        ledger.landed(to, Landed::Answer);
        number
    };
    crate::dispatches::answered(held, to, &message, number);
    // Answered: its row stops saying it is asking (#1484).
    held.rows_changed();
    held.tasks().changed();
    // Typed the line where it takes one; where it does not, its held end of turn is the
    // person's from here (#1491).
    woken(held, Wake::ToldOrSettled(to));
    kept_record(
        held,
        asker,
        Kind::Answer,
        None,
        vec![(to, name.clone(), dispatchtalk::Delivered::Delivered)],
    );
    Ok(task(Answered::Sent {
        kind: Kind::Answer,
        to: name,
        instruction: None,
    }))
}

/// Waits for the answer to the question task `asked.chat` asked its asking chat, for as long
/// as the ask says and no longer than a wait may be. Held and bounded as [`wait`] is.
pub fn await_answer(held: &Weak<Held>, asked: &Asked, connection: u64) -> Answer {
    let What::AwaitAnswer { within_secs } = asked.what else {
        return no("not a wait".to_owned());
    };
    let sender = asked.chat;
    let until = Instant::now() + Duration::from_secs(u64::from(dispatched::wait_secs(within_secs)));
    let mut parked = None;
    loop {
        let Some(held) = held.upgrade() else {
            return no("this project has been closed".to_owned());
        };
        let record = held.chats().handed_from(sender);
        let from = match dispatchtalk::up(record.as_ref()) {
            Ok(from) => from.clone(),
            Err(why) => return no(why),
        };
        // Each time the wait looks again (#3902): an answer its asking chat queued since is
        // delivered now.
        deliver_queued(&held, from.chat, None);
        let tasks = Arc::clone(held.tasks_shared());
        if parked.is_none() {
            match tasks.park(sender) {
                Ok(place) => parked = Some(place),
                Err(why) => return no(why),
            }
        }
        let asker_gone = seen(&held, from.chat).ended;
        let hung_up = held.hooks().asker_gone(connection);
        let ledger = tasks.ledger();
        if let Some(given) = ledger.talk.answered(sender) {
            return task(Answered::Replied {
                what: Reply::Answered {
                    from: given.from.clone(),
                    text: given.text.clone(),
                    by_person: given.by_person,
                },
            });
        }
        if ledger.talk.asks(sender).is_none() {
            return no("this chat has no question waiting for an answer".to_owned());
        }
        if asker_gone {
            return task(Answered::Replied {
                what: Reply::AskerGone,
            });
        }
        if hung_up || Instant::now() >= until {
            return task(Answered::Replied {
                what: Reply::NotYet { from: from.name },
            });
        }
        drop(held);
        let left = until.saturating_duration_since(Instant::now());
        let _ = tasks
            .moved
            .wait_timeout(ledger, left.min(A_WAIT_LOOKS_EVERY))
            .unwrap_or_else(PoisonError::into_inner);
    }
}

/// A question a task is paused on, as the window shows it to be answered (#1496).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct TaskQuestion {
    /// The task, by the name the person sees it under.
    pub task: String,
    /// The chat it asked, by its name.
    pub asked: String,
    /// **The question's number**: what an answer is for (`answer_task_question`). The app
    /// numbers each question as it is asked, so a later question in the same words is
    /// another question.
    pub number: u32,
    /// What it asked, **as text**: a chat's own words, never drawn as markup.
    pub question: String,
}

/// The question task `task` has put to its asking chat and is paused on, where the person may
/// answer it now: `None` for a chat that is no task, one that may no longer talk with its
/// asking chat ([`talks`]) or has ended, and one that asks nothing.
pub fn question_of(held: &Held, task: u32) -> Option<TaskQuestion> {
    let from = held.chats().handed_from(task)?;
    if from.mode != Mode::Task || !talks(held, task, &from) || seen(held, task).ended {
        return None;
    }
    let (number, question) = held
        .tasks()
        .ledger()
        .talk
        .open(task)
        .map(|(number, text)| (number, text.to_owned()))?;
    Some(TaskQuestion {
        task: dispatchtalk::chat_shown(&held.chats().shown_name(task)?),
        asked: dispatchtalk::chat_shown(
            &held
                .chats()
                .shown_name(from.chat)
                .unwrap_or_else(|| from.name.clone()),
        ),
        number,
        question,
    })
}

/// **The person answers question `number`, which task `task` put to its asking chat** (#1496,
/// V100-46). `number` and `question` are the question as the window showed it, its number
/// and its words, and `said` is what they typed.
///
/// Refused, with a sentence for the person and nothing sent, where the chat is not a task
/// that is still working, where it is paused on no question or on another one (**another
/// number, though its words be the same**), where the asking chat answered first, and where
/// the text is not one purlis hands a chat (`dispatchtalk::person_text`). **The text is never
/// cut**: it is sent whole or not at all.
///
/// Taken, the question is closed under the hold the asking chat's answer is taken under, and:
///
/// - the task has the answer **as the person's**, from its waiting command or on its next
///   turn, quoted as data under purlis's sentence;
/// - the asking chat is told the person answered, with the question and the answer, on its
///   next turn, and the question it had not read yet is no longer left for it;
/// - the dispatch's record keeps the answer with who said it, for the Activity view;
/// - each of the two is typed purlis's one line where it may be typed one, and never while
///   the person is typing in it ([`tell_the_chat`]).
///
/// **The pair's messages-a-minute limit is not asked.** It is there so two chats cannot drive
/// each other in a loop, and the person is not a chat: their answer is not counted against
/// the pair, and a limit of 0 does not stop it.
pub fn person_answers(
    held: &Held,
    task: u32,
    number: u32,
    question: &str,
    said: &str,
) -> Result<(), String> {
    let name = held
        .chats()
        .shown_name(task)
        .ok_or_else(|| format!("Chat {task} is not open, so your answer was not sent."))?;
    let from = held
        .chats()
        .handed_from(task)
        .filter(|from| from.mode == Mode::Task)
        .ok_or_else(|| dispatchtalk::not_the_person_s_to_answer(&name, None))?;
    let seen_now = seen(held, task);
    if !talks(held, task, &from) || seen_now.ended {
        let state = held.tasks().ledger().state(task, &from, seen_now).say();
        return Err(dispatchtalk::finished_for_the_person(&name, &state));
    }
    let text = dispatchtalk::person_text(said)?;
    // The asking chat, while it is open: one that has closed is told nothing, and its number
    // may be another chat's by now.
    let asker = Some(from.chat).filter(|asker| !seen(held, *asker).ended);
    let unread = held
        .tasks()
        .ledger()
        .person_answers(task, &name, asker, number, question, &text)?;
    // The question the asking chat had not read: it is told it was answered, and not asked it.
    if let Some(file) = unread {
        purlis_core::handback::took(held.root(), &file);
    }
    // One more message on the task's dispatch record, kept as the person's (#1495).
    crate::dispatches::person_answered(held, task, &text, number);
    held.tasks().changed();
    tell_the_chat(held, task);
    if let Some(asker) = asker {
        tell_the_chat(held, asker);
    }
    Ok(())
}

/// **The question a task is paused on, for the window to show before the person answers it**
/// (#1496): chat `session`'s question to its asking chat. `null` where there is none to
/// answer: the chat is no task, has reported or ended, or asks nothing now.
#[tauri::command]
#[specta::specta]
pub(crate) fn task_question(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
) -> Result<Option<TaskQuestion>, String> {
    let held = planes.held(&plane)?;
    Ok(question_of(&held, session))
}

/// **The person answers the question a task put to its asking chat** (#1496, V100-46): chat
/// `session`'s question `number` (`task_question`, or a line's `asks`), which read `question`
/// as the window showed it, with `text`. It answers that question and no other: a question
/// the task asked later is refused, though its words be the same.
///
/// The task is handed the answer as the person's and carries on; the asking chat is told the
/// person answered and does not answer again. An error is a sentence for the person, and
/// nothing was sent: the question was answered first by the asking chat, the task has moved
/// on, or the text is empty, too long or holds a character purlis hands to no chat.
///
/// **The window's alone.** No chat's command, hook or tool reaches it, and no link serves it
/// (`purlis_session_protocol::ui::WINDOW_ONLY`): what it sends reaches a chat marked as the
/// person's. On a blocking thread, as it writes the dispatch's record.
#[tauri::command]
#[specta::specta]
pub(crate) async fn answer_task_question(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
    number: u32,
    question: String,
    text: String,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("answering a task's question", move || {
        person_answers(&held, session, number, &question, &text)
    })
    .await
}

/// The person sent `bytes` from chat `chat`'s pane.
///
/// **Any key of theirs holds back what purlis would type into that chat**, until its harness
/// next says a turn began or ended: they may have opened a picker of the harness's own, which
/// no hook reports.
///
/// **And in a task still working, a prompt they send is them stepping in**
/// (`dispatched::steps_in`): its report will say so. The fact is kept, and nothing of the
/// bytes. `submitted` is the line an Enter in `bytes` sent, by how many bytes they had typed on
/// it where that could be followed, and `None` where they sent nothing (#1463).
pub fn person_typed(held: &Held, chat: u32, bytes: &[u8], submitted: Option<Option<usize>>) {
    if !crate::smartclose::typed_by_the_operator(bytes) {
        return;
    }
    let (took_over, first_key, stepped_in) = {
        let mut ledger = held.tasks().ledger();
        let first_key = !ledger.keyed(chat);
        (
            ledger.person_keyed(chat),
            first_key,
            ledger.stepped_in(chat),
        )
    };
    let from = held.chats().handed_from(chat);
    let reported = from
        .as_ref()
        .is_some_and(|from| from.mode == Mode::Task && from.report.reported());
    // **A key of theirs in a task that had reported is the person taking it over** (#1485):
    // its end stood down, for good, and its record says so for the next launch. A blocked
    // task had no end to stand down, and its record says so all the same (#1768): its later
    // report does not end it either. Written once a turn, not at every key.
    if took_over || (reported && first_key) {
        crate::finished::taken_over(held, chat);
    }
    // **And it may report once more** (#1768): what it finds now reaches the chat that asked.
    // Its record holds that across a relaunch. Decided under the lock a report is taken under,
    // as everything a chat owes is.
    if from.as_ref().is_some_and(|from| from.report == Owed::Sent) && reported {
        let _deciding = held.chats().deciding();
        if held
            .chats()
            .handed_from(chat)
            .is_some_and(|now| now.report == Owed::Sent)
        {
            held.chats().owes(chat, Owed::Again);
        }
    }
    if stepped_in {
        return;
    }
    let owes = from.is_some_and(|from| from.mode == Mode::Task && from.report == Owed::Due);
    if owes && dispatched::steps_in(held.board().glance(chat).asking, submitted) {
        held.tasks().ledger().person_typed(chat);
    }
}

/// Whether the person typed in task `task`'s pane while it worked.
pub fn stepped_in(held: &Held, task: u32) -> bool {
    held.tasks().ledger().stepped_in(task)
}

/// Waits for task `of`'s report on behalf of the chat that dispatched it, for as long as the
/// ask says and no longer than a wait may be.
///
/// Every look is a fresh judgement from the app's records, so a task that is closed, or a
/// project that is let go, ends the wait at once; and so does a command that hung up
/// (`connection`'s asker gone). Nothing is held across a look but the ledger's own lock, and
/// that only while it sleeps on its condition.
pub fn wait(held: &Weak<Held>, asked: &Asked, connection: u64) -> Answer {
    let What::Wait { of, within_secs } = asked.what else {
        return no("not a wait".to_owned());
    };
    let until = Instant::now() + Duration::from_secs(u64::from(dispatched::wait_secs(within_secs)));
    let mut parked = None;
    loop {
        let Some(held) = held.upgrade() else {
            return no("this project has been closed".to_owned());
        };
        // Each time the wait looks again (#3902): what the two chats queued since is
        // delivered, so a message queued during a long wait arrives in it.
        deliver_queued(&held, asked.chat, None);
        deliver_queued(&held, of, None);
        let tasks = Arc::clone(held.tasks_shared());
        let (from, name) = match owned(&held, asked.chat, of) {
            Ok(owned) => owned,
            // Its tab was closed: the chat that asked is told how it ended, not that the
            // task was never its own.
            Err(why) => {
                let ledger = tasks.ledger();
                let Some(gone) = ledger.gone(asked.chat, of) else {
                    drop(ledger);
                    // One a launch could not start again is still this chat's task (#1497):
                    // it is told where it stands, at once, and not that the task is not its
                    // own. Its report comes if the person ends it.
                    return match crate::unstarted::standing(&held, asked.chat, of) {
                        Some((name, state)) => task(Answered::Waited {
                            of,
                            name,
                            what: Waited::Running { state },
                        }),
                        None => no(why),
                    };
                };
                return task(Answered::Waited {
                    of,
                    name: gone.name.clone(),
                    what: match &gone.report {
                        Some(report) => Waited::Reported {
                            report: Box::new(report.clone()),
                        },
                        None => Waited::Ended,
                    },
                });
            }
        };
        if parked.is_none() {
            match tasks.park(asked.chat) {
                Ok(place) => parked = Some(place),
                Err(why) => return no(why),
            }
        }
        let seen = seen(&held, of);
        let hung_up = held.hooks().asker_gone(connection);
        let mut ledger = tasks.ledger();
        let ended = ledger.waited(of, &from, seen).or_else(|| {
            (hung_up || Instant::now() >= until).then(|| Waited::Running {
                state: ledger.state(of, &from, seen).say(),
            })
        });
        if let Some(what) = ended {
            return task(Answered::Waited { of, name, what });
        }
        // The project is let go of while this sleeps: only the ledger is kept.
        drop(held);
        let left = until.saturating_duration_since(Instant::now());
        let _ = tasks
            .moved
            .wait_timeout(ledger, left.min(A_WAIT_LOOKS_EVERY))
            .unwrap_or_else(PoisonError::into_inner);
    }
}

/// The tasks under chat `asker`: the ones still open, in the order they were started, then
/// the ones that have finished and are still listed.
///
/// **One list, read from the one place that says which chats those are and where each stands**
/// ([`crate::handoff::persona_chats`], #1443, D-T59-j2): what closing the chat asks about, what
/// the window marks a task with and what `purlis dispatch list` prints are the same chats in
/// the same words. A task the chat dispatched itself says more where the app knows more: that
/// it is idle, cancelling, asking this chat a question, or how it reported. A task the person
/// started from this chat's tab is listed too, by name and standing, and marked as theirs:
/// the list gives the chat nothing over it ([`dispatched::owned`]).
fn list(held: &Held, asker: u32) -> Vec<Row> {
    use crate::handoff::PersonaChatState;

    let now = SystemTime::now();
    let open = held.chats().open_now();
    let mut rows: Vec<Row> = crate::handoff::persona_chats(held, asker)
        .into_iter()
        .filter_map(|listed| {
            let open = open.iter().find(|open| open.session == listed.session)?;
            let from = open.from.as_ref()?;
            // Where it works on a branch of its own, by the dispatch's record (#1453). Read
            // before the ledger is taken: it asks the chats, and reads a file.
            let branch = crate::dispatches::branch_listed(held, listed.session);
            // And asked before it too: a stop is decided under a lock of its own.
            let being_stopped = held.stopping().is_stopping(listed.session);
            // purlis's stop at a limit says so, never "by the person" (#1512).
            let at_a_limit = held.stopping().limit_of(listed.session);
            // Whether its program runs, by the board (#3900). Asked before the ledger is taken.
            let running = !seen(held, listed.session).ended;
            let ledger = held.tasks().ledger();
            let own = dispatched::owned(asker, listed.session, Some(from)).is_ok();
            let state = match (own, listed.state) {
                // The person is stopping it (#1488): said first, whoever's task it is.
                _ if being_stopped => match at_a_limit {
                    Some(reached) => dispatched::being_stopped_as(reached),
                    None => dispatched::BEING_STOPPED.to_owned(),
                },
                // Waiting on the person, by the board or by an ask the app holds open: the
                // standing knows both, and is the word for it. A cancel under way is said
                // first: that is what this chat asked for.
                // Asked of the hold this row is built under: the ledger's lock is not one a
                // thread may take twice.
                (true, PersonaChatState::WaitingOnOperator)
                    if !ledger.cancelling(listed.session) =>
                {
                    listed.said.clone()
                }
                (true, _) => ledger
                    .state(listed.session, from, seen(held, listed.session))
                    .say(),
                (false, _) => listed.said.clone(),
            };
            let last_note = ledger.last_note(listed.session);
            Some(Row {
                chat: listed.session,
                name: listed.name,
                persona: listed.persona,
                place: open
                    .workspace
                    .clone()
                    .map_or(Place::PlaneRoot, Place::Workspace)
                    .word()
                    .to_owned(),
                state,
                age_secs: ledger.age(listed.session, now).map(|age| age.as_secs()),
                by_person: !own,
                branch: branch.as_ref().map(|(branch, _)| branch.clone()),
                branch_stands: branch.map(|(_, stands)| stands),
                finished: false,
                // A long-running task, and its last note (#3905).
                long: from.long,
                last_note: last_note.and_then(|note| note.text.clone()),
                noted_secs: last_note.map(|note| note.ago(now).as_secs()),
                liveness: Some(ledger.liveness(listed.session, running, now)),
            })
        })
        .collect();
    // Then the ones that have finished, whose programs purlis ended (#1485): from their
    // dispatch records, until their rows are cleared or this chat closes. With them the ones
    // orphaned by a restart (#3900), marked so.
    rows.extend(crate::finished::listed_for(held, asker));
    rows
}

/// Task `task`'s liveness now (#3900), as its asking chat's list and roster say it.
pub(crate) fn liveness(held: &Held, task: u32) -> purlis_core::dispatchliveness::Liveness {
    let running = !seen(held, task).ended;
    held.tasks()
        .ledger()
        .liveness(task, running, SystemTime::now())
}

/// Cancels task `of` for the chat that dispatched it: the cancel is recorded, and its turn is
/// ended and it is asked for a short report as soon as purlis may send its pane anything.
///
/// **By the stop machine** (#1459, `crate::stopping::cancel`): one bounded turn to report, then
/// its end, as the person's stop gives a task, but for who may ask and what the asking chat is
/// told: its report, as `cancelled`.
fn cancel(held: &Held, asker: u32, of: u32) -> Result<Answer, String> {
    let name = {
        // Under the lock a report is taken under, so a cancel and a report in flight land in
        // one order: either the task has reported and the cancel is refused, or the cancel is
        // recorded first and the report is delivered as cancelled.
        let deciding = held.chats().deciding();
        let (from, name) = owned(held, asker, of)
            .map_err(|why| or_finished(held, asker, of, why, "nothing to cancel"))?;
        // The person is stopping it already: that ends it and tells this chat, and a chat in
        // a stop is not cancelled as well (D-T59-j3).
        if held.stopping().is_stopping(of) {
            return Err(match held.stopping().limit_of(of) {
                Some(reached) => dispatched::being_stopped_at_a_limit(&name, of, reached),
                None => dispatched::being_stopped(&name, of),
            });
        }
        // The person has typed in it: it is theirs (#3785, ruled 2026-10-29), as a close is
        // refused for it.
        if stepped_in(held, of) || held.tasks().ledger().keyed(of) {
            return Err(dispatched::the_person_typed_in(&name, of, "cancel"));
        }
        // A task that reported while its program still runs is cancelled too (#3899).
        let seen = seen(held, of);
        let began = held.tasks().ledger().cancel(of, &name, &from, seen)?;
        if began {
            crate::stopping::cancel(held, of, &deciding);
        }
        name
    };
    // What the cancel asks for, carried out with the lock let go.
    held.stops_carry_on();
    Ok(task(Answered::Cancelling { of, name }))
}

/// **Cancelled task `task` ended its cancel's turn without a report, having reported before
/// the cancel** (#3899): its report stands, and its program is ended as a task's is at its
/// report, once that turn has settled, on a thread of this module's own ([`end_look_after`]).
/// Nothing for a task that is not such a one.
pub(crate) fn cancelled_without_a_report(held: &Held, task: u32) {
    // Only while it is still running after a report, by the one rule (#3899): one that
    // reported in the cancel's turn is ending at that report already, and one whose program
    // has ended has nothing left to end.
    let ended = seen(held, task).ended;
    let reported_before = held
        .chats()
        .handed_from(task)
        .is_some_and(|from| dispatched::still_running_after_report(from.report, ended));
    if reported_before && held.tasks().ledger().cancelled_without_a_report(task) {
        end_look_after(
            held,
            task,
            dispatched::A_TURN_SETTLES_WITHIN,
            Looked::WaitedOut,
        );
    }
}

/// The app started the persona chat `task`.
pub fn started(held: &Held, task: u32) {
    held.tasks().ledger().started(task, SystemTime::now());
}

/// Chat `task` was put back by a launch (#1463): where it is a dispatch's running task among
/// `records`, the ledger keeps the start its record says, so its row says its age again. And
/// where it is a task, how it came back (#3900): `kept` that its program ran on in the session
/// host and was reattached, else that the restart ended it and it was started again.
pub fn brought_back(
    held: &Held,
    task: u32,
    records: &[purlis_core::dispatchrecord::Record],
    kept: bool,
) {
    use purlis_core::dispatchliveness::Restart;
    let is_a_task = held
        .chats()
        .handed_from(task)
        .is_some_and(|from| from.mode == Mode::Task);
    let Some(me) = crate::dispatches::chat_ref(held, task) else {
        return;
    };
    let record = records.iter().find(|record| {
        record.running() && purlis_core::dispatchrecord::same_chat(&record.worker.chat, &me)
    });
    if record.is_none() && !is_a_task {
        return;
    }
    let restart = if kept {
        Restart::KeptRunning
    } else {
        Restart::StartedAgain
    };
    let key = key(held, task);
    let mut ledger = ledger(held);
    // What it had open is open again, where it stood (#4553): its asking chat is told of it
    // again if it has not had it, since a line it was due died with the last run.
    let back = ledger
        .talk
        .restored(task, &key, held.root(), now_secs(SystemTime::now()));
    if back == dispatchtalk::Restored::Open
        && ledger.talk.handed_at(task).is_none()
        && let Some(asker) = ledger.talk.asked_of(task)
    {
        ledger.landed(asker, Landed::Question(task));
    }
    ledger.brought_back(task, record.map(|record| record.started.as_str()), restart);
    // The last note it sent is on its record, which a restart keeps (#3900): it was heard from
    // then, though not by this app. What it ran is kept in memory only. And its list row says
    // that note again, with its words where the record kept them (#3956).
    if let Some(note) = record.and_then(purlis_core::dispatchrecord::last_note_of) {
        let at = SystemTime::from(note.at);
        ledger.heard_doing(task, purlis_core::dispatchliveness::Did::SentANote, at);
        ledger.noted_before(task, note);
    }
}

/// A launch has put back the tasks with the keys `back`: what was kept of a question of any
/// other is let go, its task having ended (#4553).
pub fn only_these_came_back(held: &Held, back: &[String]) {
    ledger(held).talk.only_these_came_back(back);
}

/// Seconds since 1970 at `at`.
fn now_secs(at: SystemTime) -> u64 {
    at.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// The outcome `task`'s report is delivered with, given the one its chat said.
pub fn outcome_for(
    held: &Held,
    task: u32,
    said: purlis_core::handback::Outcome,
) -> Result<purlis_core::handback::Outcome, String> {
    held.tasks().ledger().outcome_for(task, said)
}

/// Task `task`'s report was delivered for the chat `asker`: left in `file` for its next turn
/// where that chat is open. A command waiting on it has it now. **Nothing is typed here**: it
/// is called under the lock a report is taken under, and [`told_or_settled`] follows once that is let
/// go.
pub fn reported(
    held: &Held,
    task: u32,
    asker: u32,
    report: purlis_core::handback::Handback,
    file: Option<std::path::PathBuf>,
) {
    held.tasks().ledger().reported(task, asker, report, file);
    held.tasks().changed();
}

/// Task `task`'s report was kept for its workspace, because the chat that asked, `asker`, has
/// gone (#1510, V100-64): a command waiting on it has it now, and **its program is to be
/// ended, as a task's whose report reached its chat** (`Ledger::reported_with_its_asker_gone`).
/// Decided from what this app holds of the delivery it just made, never from a record on
/// disk. Under the lock a report is taken under, as [`reported`] is.
pub fn reported_with_its_asker_gone(
    held: &Held,
    task: u32,
    asker: u32,
    report: purlis_core::handback::Handback,
) {
    held.tasks()
        .ledger()
        .reported_with_its_asker_gone(task, asker, report);
    held.tasks().changed();
}

/// Whether task `task` has a question open with the chat that dispatched it: asked, and not
/// answered yet. What its row in the window says it is waiting on (#1484).
pub fn asks_its_asker(held: &Held, task: u32) -> bool {
    held.tasks().ledger().talk.asks(task).is_some()
}

/// **What chat `chat` waits on that is not the person** (#1491), by this project's own
/// records at this moment: how many tasks below it, at any depth, still owe their report
/// ([`crate::handoff::owing_below`]); whether it is a task paused on a question to the chat
/// that dispatched it, while that chat is open and its program runs; and whether a line of
/// purlis's own is about to start a turn of it (`Ledger::line_coming`).
///
/// Takes the chats, the board and the ledger in turn and holds none across another: never
/// called with the board held.
pub fn waits(held: &Held, chat: u32) -> purlis_core::state::Waits {
    waits_as(held, chat, false)
}

/// [`waits`], asked as chat `chat`'s own turn ends where `turn_ends`: what the board is told
/// as that end is applied (`purlis_core::state::Board::reported_while`), so it raises no
/// needs-you item while the next move is a task's, the asking chat's, or a turn purlis is
/// about to start.
pub fn waits_as(held: &Held, chat: u32, turn_ends: bool) -> purlis_core::state::Waits {
    let tasks = crate::handoff::owing_below(held, chat).len();
    // Only while it still owes its report, and only while the chat it asked can answer: a
    // question to a chat that has closed, or whose program has ended, is nobody's.
    let its_asker = held
        .chats()
        .owed_task_report(chat)
        .is_some_and(|from| asks_its_asker(held, chat) && !seen(held, from.chat).ended);
    // A chat the person is stopping, or its own chat cancelling, is typed that stop's line and
    // no other.
    let line_coming = !held.stopping().in_its_last_turn(chat)
        && held
            .tasks()
            .ledger()
            .line_coming(chat, seen(held, chat), turn_ends);
    // A sandbox block's Notice raised in the turn that ends asks the person (#1663, #3878).
    let on_a_notice = turn_ends && {
        let glance = held.board().glance(chat);
        held.chats()
            .blocks()
            .waits_on_a_notice(chat, glance.turns, true)
    };
    purlis_core::state::Waits {
        tasks: u32::try_from(tasks).unwrap_or(u32::MAX),
        its_asker,
        line_coming,
        on_a_notice,
        // Stamped as the report is applied (`hooks::waits_at`).
        now: 0,
    }
}

/// **Whether the end of turn held for chat `chat` is the person's now, and if it is, makes
/// it the needs-you item** (#1491). The one place a held end is released: the board holds
/// one for `chat`, nothing is left for it to wait on by the project's records ([`waits`]),
/// and no line of purlis's own is about to start a turn of it.
///
/// **Asked wherever what a chat waits on may have gone**, for that chat and every chat above
/// it ([`settle_above`]): a task reported, was stopped, was closed, died or was ended; a
/// question was answered or the chat it was put to went; the chat's own turn ended; and the
/// line typed into it was never taken. It reads and never types, so it may be called under
/// the lock a report is taken under.
///
/// The rule it keeps: a chat that needs the person is never left idle with no hand.
pub fn settle(held: &Held, chat: u32) {
    // Two statements: the board is let go before the records are read, which read it too.
    let holds = held.hooks().board().held(chat);
    if !holds || waits(held, chat).on_something() {
        return;
    }
    held.rested(chat);
}

/// [`settle`], for chat `chat` and every chat above it by task links: a task far below may
/// have been the last thing each of them waited on. Each chat once, and no deeper than a
/// chain may go.
pub fn settle_above(held: &Held, chat: u32) {
    let mut at = Some(chat);
    let mut seen_already = Vec::new();
    while let Some(one) = at {
        if seen_already.contains(&one)
            || seen_already.len() > purlis_core::dispatchdecision::DEEPEST as usize
        {
            break;
        }
        seen_already.push(one);
        settle(held, one);
        at = held
            .chats()
            .handed_from(one)
            .filter(|from| from.mode == Mode::Task)
            .map(|from| from.chat);
    }
}

/// **Chat `chat` may now be told what was left for its next turn**: typed purlis's one line,
/// where it may be sent one. Never called under `Chats::deciding()`.
///
/// Then it and every chat above it are looked at as chats that may have nothing left to wait
/// on ([`settle_above`]): where no line was typed, a held end of turn is the person's once
/// nothing will prompt the chat. Where one was, the chat is about to work, and a clock looks
/// again if its harness never takes the line ([`line_typed_into`]).
pub fn told_or_settled(held: &Held, chat: u32) {
    tell_the_chat(held, chat);
    settle_above(held, chat);
}

/// The line typed into chat `chat` was given its time and no turn began on it: its harness
/// had something open that no hook reports. The chat is no longer about to work, and is
/// looked at as one nothing will prompt ([`settle`]). Nothing where a turn did begin.
pub fn line_not_taken(held: &Held, chat: u32) {
    if held.tasks().ledger().line_not_taken(chat) {
        settle_above(held, chat);
    }
}

/// purlis typed its line into chat `chat`: in the time a typed line is taken within, the chat
/// is looked at again ([`line_not_taken`]). On a thread of its own, holding the project
/// weakly. A test turns the clock on where it means the time to pass (`Tasks::on_the_clock`),
/// and calls [`line_not_taken`] itself otherwise.
fn line_typed_into(held: &Held, chat: u32) {
    #[cfg(test)]
    if !held
        .tasks()
        .clocked
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return;
    }
    let held = held.weak();
    let _ = std::thread::Builder::new()
        .name("purlis-line-taken".into())
        .spawn(move || {
            std::thread::sleep(dispatched::A_LINE_IS_TAKEN_WITHIN);
            if let Some(held) = held.upgrade() {
                line_not_taken(&held, chat);
            }
        });
}

/// **Task `task`'s report is delivered: its program is now to be ended** (#1485). Called once
/// the lock the report was taken under is let go, by whoever took it. It ends nothing itself:
/// it looks once, for a task whose turn is already over, and sets the bound on how long the
/// rest may take ([`dispatched::ends_within`]).
pub fn delivered(held: &Held, task: u32) {
    if !held.tasks().ledger().ending(task) {
        return;
    }
    end_look_after(
        held,
        task,
        dispatched::ends_within(seen(held, task)),
        Looked::WaitedOut,
    );
    end_look(held, task, Looked::Moved);
}

/// **The end of task `task` stands down, for good**: a Smart close of it is beginning, which is
/// a turn of its own that the pending end must not cut. As for a key of the person's
/// ([`person_typed`]). Nothing where it was not one to end.
pub fn stand_down(held: &Held, task: u32) {
    if held.tasks().ledger().end_stood_down(task) {
        crate::finished::taken_over(held, task);
    }
}

/// What the person is looking at changed (another chat was brought in front, a tab was
/// switched to another chat of its session, or a pane was added to or taken out of a tab):
/// every task whose end was held is looked at again, since one of them may have been held
/// because the person had it in front of them.
pub fn front_moved(held: &Held) {
    let held_back = held.tasks().ledger().held_back();
    for task in held_back {
        end_look(held, task, Looked::Moved);
    }
}

/// Looks at task `task`, which has reported, and does the next thing toward ending its
/// program (`Ledger::end_step`). `looked` says why it is looked at.
///
/// **The end is only ever made here on a thread this module started**
/// ([`end_look_after`]): a look for [`Looked::Moved`] answers at most "settle", so the thread
/// that heard a hook or a program's end never closes a chat.
pub(crate) fn end_look(held: &Held, task: u32, looked: Looked) {
    end_look_reading(held, task, looked, &is_held_back);
}

/// [`end_look`], reading whether the task is held back with `held_back`: [`is_held_back`] in
/// the app, and a read a test can make change between the look's two reads of it.
pub(crate) fn end_look_reading(
    held: &Held,
    task: u32,
    looked: Looked,
    held_back: &dyn Fn(&Held, u32) -> bool,
) {
    if !held.tasks().ledger().ending(task) {
        return;
    }
    // The person's stop ends it, and says so in its own word; and a quit ends nothing here:
    // the chats it leaves are the ones the next launch reads.
    if held.stopping().is_stopping(task) || held.chats().ending() {
        return;
    }
    let seen = seen(held, task);
    // Read before the ledger is taken: it asks the chats and the board. **Held back** while a
    // chat it started is at work, and while the person is looking at it: it is on screen in
    // the tab in front, as a pane's own chat or as the task a pane shows (`Chats::looked_at`,
    // #1486, #1489). It is not ended under them, and is ended when they move away from it
    // ([`front_moved`]).
    //
    // **The window's two words do not arrive together.** A task moved from a tab of its own
    // to a pane beside its session is said as "the session is in front" and then "the task is
    // beside it", and between the two nobody is looking at it here. That is safe only because
    // a look for [`Looked::Moved`] answers at most "settle", and the look after the settle
    // reads this again: by then the second word has landed.
    let read = held_back;
    let held_back = read(held, task);
    let step = held
        .tasks()
        .ledger()
        .end_step(task, seen, held_back, looked);
    match step {
        Ends::Nothing => {}
        // Held, and the person may have looked away between the read above and the answer:
        // the look their move made found nothing held yet. Read once more, and look as that
        // move would have.
        Ends::Hold => {
            if held_back && held.tasks().ledger().held_back().contains(&task) && !read(held, task) {
                end_look(held, task, Looked::Moved);
            }
        }
        // What held it has gone and its bound passed meanwhile: the bound is set again.
        Ends::Bound => end_look_after(held, task, dispatched::ends_within(seen), Looked::WaitedOut),
        Ends::Settle => end_look_after(
            held,
            task,
            dispatched::A_TURN_SETTLES_WITHIN,
            Looked::Settled,
        ),
        Ends::End => end_it(held, task),
    }
}

/// Whether something that is not task `task`'s own turn holds its end back: a chat it started
/// is at work, or it is the chat the person is looking at.
fn is_held_back(held: &Held, task: u32) -> bool {
    held.chats().looks_at(task) || !crate::handoff::running_below(held, task).is_empty()
}

/// Ends task `task`'s program, its report delivered: the chat is closed as its tab's Close
/// closes it, under the lock a report is taken under, and the window takes its tab away where
/// it had one. Its finished row is its dispatch record's ([`crate::finished`]).
fn end_it(held: &Held, task: u32) {
    if let Err(why) = held.close_chat(task) {
        tracing::warn!("purlis: task chat {task}, reported, did not end cleanly ({why})");
    }
    held.tell_stop(task, crate::stopping::StopPhase::Stopped);
}

/// Looks at task `task`'s end again in `after`, as `looked`. On a thread of its own, holding
/// the project weakly: a closed project is not kept open by it.
fn end_look_after(held: &Held, task: u32, after: Duration, looked: Looked) {
    #[cfg(test)]
    if !held
        .tasks()
        .clocked
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return;
    }
    // A bound set anew since this one, as the task's own work ended, makes this one stale
    // (#3921): it looks at nothing.
    let epoch = held.tasks().ledger().bound_epoch(task);
    let held = held.weak();
    let _ = std::thread::Builder::new()
        .name("purlis-task-end".into())
        .spawn(move || {
            std::thread::sleep(after);
            if let Some(held) = held.upgrade() {
                let stale =
                    looked == Looked::WaitedOut && held.tasks().ledger().bound_epoch(task) != epoch;
                if !stale {
                    end_look(&held, task, looked);
                }
            }
        });
}

/// Types the line that says something was left for its next turn into chat `chat`, where
/// there is something it was not told of and it may be sent a line now: the reports and
/// questions of tasks it dispatched, and a message from the chat that dispatched it. Answers
/// whether a line was typed.
fn tell_the_chat(held: &Held, chat: u32) -> bool {
    // A chat the person is stopping, or its own chat cancelling, is typed that stop's one line
    // and no other.
    if held.stopping().in_its_last_turn(chat) {
        return false;
    }
    // A session whose tasks the person is stopping all at once is typed one line for all of
    // their words, once the last has ended (#1498).
    if held.stopping().holds_word_for(chat) {
        return false;
    }
    let seen = seen(held, chat);
    let landed = held.tasks().ledger().nudge_step(chat, seen);
    if landed.is_empty() {
        return false;
    }
    if let Err(why) = type_line(held, chat, &dispatched::nudge(&landed)) {
        tracing::warn!(
            "purlis: chat {chat} was not told what was left for it ({why}); it reaches its \
             next turn"
        );
        return false;
    }
    // The line starts a turn: a task that had reported is working again from now (#1485),
    // and a chat whose end was held is about to work (#1491).
    held.tasks().ledger().line_typed(chat);
    line_typed_into(held, chat);
    true
}

/// One line and Enter, as a smart close's prompt is sent: a bracketed paste, then a carriage
/// return, in one write.
fn type_line(held: &Held, chat: u32, line: &str) -> Result<(), String> {
    let bytes = format!("{}\r", crate::curation::bracketed(line));
    held.chats().sessions().input(chat, bytes.as_bytes())
}

/// A report reached the project from chat `report.chat`'s harness: whatever was waiting for
/// that chat to move is looked at again.
pub fn heard(held: &Held, report: &Report) {
    // A chat a Reopen started was heard from: its harness brought the conversation back.
    reopen_settled(held, report.chat, false);
    // The app hears from it, so what it queued while the app did not answer is delivered
    // now (#3902). One look at a folder where it queued nothing.
    deliver_queued(held, report.chat, None);
    // Activity of its own run's, which a task's quiet stretch is measured from (#1981). A
    // helper's report is the chat's work too.
    crate::silence::heard(
        held,
        report.chat,
        report.event == Event::Stop && report.agent.is_none(),
    );
    match report.event {
        Event::UserPromptSubmit => held.tasks().ledger().turn_began(report.chat),
        Event::Stop => {
            held.tasks().ledger().turn_ended(report.chat);
            // The chat's own turn, and not one that waits on work of its own (#1981, #3878).
            if report.agent.is_none()
                && !report.detail.helpers_at_work
                && report.detail.own_work.is_nothing()
            {
                owing(held, report.chat);
            }
        }
        _ => {}
    }
    moved(held, report.chat);
}

/// **What chat `output.chat`'s turn put out** (#1981), heard on its own line before the turn's
/// end: kept, masked and made a report's text (`handback::last_message_summary`), for a task
/// alone, in memory, until its next turn begins. The chat is the one the line's token is
/// for, and the board has judged the line its adopted run's.
pub fn said(held: &Held, output: purlis_core::hookwire::TurnOutput) {
    let task = held
        .chats()
        .handed_from(output.chat)
        .is_some_and(|from| from.mode == Mode::Task);
    if !task {
        return;
    }
    // What a turn put out is activity (#1981).
    crate::silence::heard(held, output.chat, false);
    match output.turn_output {
        purlis_core::hookwire::Output::Final { text } => {
            let kept = purlis_core::handback::last_message_summary(Some(&text));
            held.tasks().ledger().said_last(output.chat, kept);
        }
        purlis_core::hookwire::Output::Ran {
            command,
            exit,
            failed,
        } => {
            let ran = dispatched::Ran::shown(&command, exit, failed);
            let mut ledger = held.tasks().ledger();
            ledger.ran(output.chat, ran);
            // When it last ran a command, for its liveness (#3900).
            ledger.heard_doing(
                output.chat,
                purlis_core::dispatchliveness::Did::RanACommand,
                SystemTime::now(),
            );
        }
    }
}

/// **`purlis dispatch close <chat>`** (#1981 D-4, ruled 2026-10-15): closes a finished task
/// the asking chat dispatched itself, and no other chat's. #1485 stands: a task's program ends
/// at its report, and what stays is its finished row, a readable record with Reopen and Clear.
/// Close takes that row away. A task kept open after its report (it came out blocked, or
/// purlis made its report from its last message) has its program ended first, by the chat's
/// own close, as a reported task's always is.
///
/// **Refused where the task is not finished, or not the asking chat's to end**: one still owing
/// its report (cancel asks it for one), one in a turn or showing a prompt, one the person is
/// stopping, one the person typed in or is looking at, and one with a task of its own still at
/// work: the holds the #1485 end keeps ([`is_held_back`]).
///
/// **Under the lock a report is taken under**, as a cancel is: every refusal is read again
/// under it, and the chat is closed in the same hold, so a report, a stop and this close land
/// in one order.
fn close(held: &Held, asker: u32, of: u32) -> Result<Answer, String> {
    let (name, above) = {
        let deciding = held.chats().deciding();
        let (from, name) = match owned(held, asker, of) {
            Ok(owned) => owned,
            Err(why) => {
                let Some(name) = held
                    .tasks()
                    .ledger()
                    .gone(asker, of)
                    .map(|gone| gone.name.clone())
                else {
                    return Err(why);
                };
                drop(deciding);
                clear_row(held, asker, of);
                return Ok(task(Answered::Closed {
                    of,
                    name: purlis_core::personas::one_line(&name),
                }));
            }
        };
        let name = purlis_core::personas::one_line(&name);
        close_refused(held, of, &from, &name)?;
        if let Err(why) = held.close_chat_held(of, &deciding) {
            tracing::warn!(
                "purlis: task chat {of}, closed by its asking chat, did not end cleanly ({why})"
            );
        }
        (name, from.chat)
    };
    held.stops_carry_on();
    held.told_of_a_close(Some(above));
    held.tell_stop(of, crate::stopping::StopPhase::Stopped);
    clear_row(held, asker, of);
    Ok(task(Answered::Closed { of, name }))
}

/// Why open task `of`, called `name`, with record `from`, is not one its asking chat may close
/// now (#1981 D-4), or nothing. Asked under the lock a report is taken under.
fn close_refused(held: &Held, of: u32, from: &HandedFrom, name: &str) -> Result<(), String> {
    if from.report == Owed::Due {
        return Err(format!(
            "'{name}' (chat {of}) has not reported yet, so it is not finished. `purlis \
             dispatch cancel {of}` asks it for its report; close it once it has."
        ));
    }
    if held.stopping().is_stopping(of) {
        return Err(format!(
            "'{name}' (chat {of}) is being stopped by the person, and its stop ends it."
        ));
    }
    if stepped_in(held, of) || held.tasks().ledger().keyed(of) {
        return Err(dispatched::the_person_typed_in(name, of, "close"));
    }
    if held.chats().looks_at(of) {
        return Err(format!(
            "the person is looking at '{name}' (chat {of}); close it once they have moved on."
        ));
    }
    if !crate::handoff::running_below(held, of).is_empty() {
        return Err(format!(
            "a task '{name}' (chat {of}) dispatched is still at work; close it once that one \
             has reported."
        ));
    }
    let seen = seen(held, of);
    if seen.running || seen.prompt_showing() {
        return Err(format!(
            "'{name}' (chat {of}) is in a turn now; close it once that turn ends."
        ));
    }
    Ok(())
}

/// Clears task `of`'s finished row under the chat `asker`, where it has one.
fn clear_row(held: &Held, asker: u32, of: u32) {
    let ids: Vec<String> = crate::finished::listed(held)
        .into_iter()
        .filter(|row| row.asker == asker && row.chat == Some(of))
        .map(|row| row.id)
        .collect();
    if !ids.is_empty() {
        crate::finished::clear(held, &ids);
    }
}

/// **`purlis dispatch read <chat>`** (#1981): what task `of` said last and the commands it ran,
/// for the chat that dispatched it and no other. The app's record of that chat decides whose
/// task it is ([`owned`]), and a task that has closed is read from the app's memory of it,
/// keyed by the chat that asked: every other chat is refused in the one sentence a wait is.
fn output(held: &Held, asker: u32, of: u32) -> Result<Answer, String> {
    let name = match owned(held, asker, of) {
        Ok((_, name)) => name,
        Err(why) => match held.tasks().ledger().gone(asker, of) {
            Some(gone) => gone.name.clone(),
            None => return Err(why),
        },
    };
    let heard = held.tasks().ledger().heard(asker, of).unwrap_or_default();
    Ok(task(Answered::Output {
        of,
        name,
        finals: heard.finals.into(),
        ran: heard.ran.into(),
    }))
}

/// **Task `task`'s turn ended** (#1981): where it still owes its report, it is asked for it
/// once in purlis's line, which [`moved`] types where it may; where the turn that line started
/// ended owing too, purlis sends **a report purlis made from its last message**
/// (`crate::handoff::made_from_last_message`). Not while its turn's end is not the end of its
/// work: a task of its own still owes a report, it is paused on a question to the chat that
/// asked, or a stop or a cancel of it is under way, which asks for its report itself.
fn owing(held: &Held, task: u32) {
    let Some(from) = held.chats().handed_from(task) else {
        return;
    };
    if from.mode != Mode::Task {
        return;
    }
    // A long-running task is never asked, and has none made (#3905).
    let owes = dispatched::owes_at_turn_end(&from);
    let waits = owes
        && (held.stopping().is_stopping(task)
            || held.stopping().is_cancelling(task)
            || asks_its_asker(held, task)
            || !crate::handoff::owing_below(held, task).is_empty());
    let owing = held.tasks().ledger().turn_ended_owing(task, owes, waits);
    if let dispatched::Owing::Make { last } = owing {
        crate::handoff::made_from_last_message(held, task, last.as_deref());
    }
}

/// Chat `chat` moved: its turn ended, a prompt of its was answered, or its program ended.
pub fn moved(held: &Held, chat: u32) {
    // Most lines are from chats nothing here waits on: asked first, so they cost one look.
    if held.tasks().ledger().waits_on(chat) {
        tell_the_chat(held, chat);
        // A task that has reported: its turn's end is when its program is ended (#1485).
        end_look(held, chat, Looked::Moved);
    }
    // And the chat that started this one may be a reported task held back while this one was
    // at work: a turn ending here, as a close does, is when that hold may have gone.
    if let Some(from) = held.chats().handed_from(chat) {
        end_look(held, from.chat, Looked::Moved);
    }
    // **Its own end of turn, where it was held** (#1491): the last report may have landed
    // while that end was on its way, or its harness may take no line. Whatever it waited on
    // is asked again now that the end is applied; a line just typed keeps it held.
    settle(held, chat);
    // And where its program has ended: a task of its that was paused on a question to it has
    // nobody left to answer, and the chats above it may have waited on it.
    if seen(held, chat).ended {
        for (task, _) in held.chats().tasks_of(chat) {
            settle(held, task);
        }
        if let Some(from) = held.chats().handed_from(chat) {
            settle_above(held, from.chat);
        }
    }
    // A chat a Reopen started that has ended: how that Reopen came out.
    if held.tasks().any_reopening() && seen(held, chat).ended {
        reopen_settled(held, chat, true);
    }
    held.tasks().changed();
}

/// Chat `session`, which a Reopen of a finished task may have started, was heard from or has
/// gone: the task's row is cleared where the resume worked, and stays, saying why, where the
/// chat ended at once ([`Tasks::reopen_settles`]).
fn reopen_settled(held: &Held, session: u32, ended: bool) {
    if let Some((id, true)) = held.tasks().reopen_settles(session, ended)
        && let Err(why) = purlis_core::dispatchrecord::clear(held.root(), &id)
    {
        tracing::warn!("purlis: a reopened task's row was not cleared ({why})");
    }
}

/// Chat `chat` was closed. `task_of` is its asking chat and its name, where it was a task,
/// read before its record went: that much is kept, so a wait on it says how it ended. Whoever
/// waits on it looks again.
pub fn closed(held: &Held, chat: u32, task_of: Option<(u32, String)>) {
    held.tasks().quiet().forget(chat);
    held.tasks().ledger().forget(
        chat,
        task_of
            .as_ref()
            .map(|(asker, name)| (*asker, name.as_str())),
    );
    // A message is for one chat's turn, and this chat will have no more.
    dispatchtalk::forget(held.root(), chat);
    reopen_settled(held, chat, true);
    // The chat that asked for it may have been waiting for it to go before its own program
    // is ended (#1485). A look only: called under a close's lock, it ends nothing here.
    if let Some((asker, _)) = task_of {
        end_look(held, asker, Looked::Moved);
    }
    held.tasks().changed();
}

/// Chat `old` is now `new`: the same chat, started again under a new number. What the app
/// remembers of it, and the messages waiting for its next turn, follow it.
pub fn followed(held: &Held, old: u32, new: u32) {
    // What it queued while the app did not answer goes on with it (#3902): moved to the new
    // number, signed under the new run's key, before the old run ends and its folder is cleared.
    if let Some(socket) = held.hooks().socket() {
        purlis_core::dispatchqueue::moved(socket, old, new);
    }
    held.tasks().ledger().followed(old, new);
    // A cancel under way goes on, as a cancel of the new run (#1459).
    crate::stopping::cancel_followed(held, old, new);
    dispatchtalk::moved(held.root(), old, new);
    // A task that had reported is still to be ended, under its new number (#1485).
    delivered(held, new);
    held.tasks().changed();
}
