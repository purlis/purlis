// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis dispatch` — start a persona chat on a task, and send a task's report back (#1436).
//!
//! ```text
//! purlis dispatch --name "<task>" [--to <persona>] [--profile <profile>]
//!                 [--in workspace:<name> | --in worktree] [--long] <<'BRIEF'
//! <the brief>
//! BRIEF
//!
//! purlis dispatch report --outcome done|blocked|failed [--changed "<what changed>"] "<text>"
//!
//! purlis dispatch --name "<task>" --wait [--timeout <seconds>] <<'BRIEF' …
//! purlis dispatch wait <chat> [--timeout <seconds>]
//! purlis dispatch list
//! purlis dispatch read <chat>
//! purlis dispatch close <chat>
//! purlis dispatch cancel <chat>
//!
//! purlis dispatch tell <chat> "<text>"        (the asking chat, to a task still working)
//! purlis dispatch tell --all "<text>"         (the asking chat, to every task still working)
//! purlis dispatch answer <chat> "<text>"      (the asking chat, to a task's question)
//! purlis dispatch note "<text>"               (a task, to its asking chat)
//! purlis dispatch note --ack <n> ["<text>"]   (a task, acknowledging instruction n)
//! purlis dispatch ask "<question>"            (a task, to its asking chat; it waits)
//! ```
//!
//! A dispatch is one chat starting another, which runs as a persona for its whole life
//! (ADR 0090, #1434). This command is the asking chat's half of a **task**: it hands the app the
//! persona, the task's name and the brief, and the app decides
//! ([`purlis_core::dispatchdecision`]) and starts the chat. The `dispatch` chat tool runs the
//! same two functions ([`send`], [`report`]), so the command and the tool cannot answer one
//! request two ways.
//!
//! **It rides the handoff's road**: a ticket for this chat, then the ask that spends it, on one
//! connection of the chat's hook socket ([`crate::handoff::ticketed`]); the stamp the new chat
//! opens with; and the report, which is the handoff's own report ask with a task's outcome on
//! it. What differs is who writes the facts. A handoff stamps its own message and the app
//! checks it; here the request says nothing about the chat that asks but its number, and the
//! app writes the stamp from its own record ([`purlis_core::hookwire::DispatchAsk`]).
//!
//! **Asking after a task** (#1441): `--wait` and `wait` hold the command until the task's report
//! lands, and print the report as the next turn would have been handed it; `list` prints the
//! tasks this chat dispatched; `read` prints what one said last and the commands it ran, as its
//! harness handed them to purlis (#1981); `cancel` ends one's turn and asks it for a short
//! report. Each is
//! one ask on the chat's hook socket with no ticket, which names a chat by the app's number
//! for it. Whether that chat is this chat's task is the app's record, never anything said here
//! ([`purlis_core::dispatched`]).
//!
//! **A task always reports back** (#1981): a task whose turn ends without its report is asked
//! for it once, in purlis's own line, and a turn that then ends without one too gets **a report
//! purlis made from its last message**, sent by the app in its place. A `tell` to a task at rest
//! starts its turn at once. Both are the app's ([`purlis_core::dispatched::Ledger`]).
//!
//! **Messages** (#1442): `tell` and `answer` go down, to a task this chat dispatched; `note`
//! and `ask` go up, and name nobody, because the chat they go to is the one the app recorded as
//! this chat's asker. Each is one ask with no ticket. Who may send to whom, the limit a minute
//! and the text's bounds are the app's to judge ([`purlis_core::dispatchtalk`]); the text is
//! checked here too, so an empty or oversized one is refused before anything is asked.
//!
//! **A message outlasts an app that does not answer** (#3902): it is queued first, durable,
//! where this process may write purlis's queue ([`purlis_core::dispatchqueue`]), and the app is
//! then asked to deliver it. An app that does not answer leaves it queued, and delivers it, in
//! order and once, when it next hears from this chat; the sender is told it is queued, not yet
//! delivered, and `list` shows where it stands. A sandbox that keeps this process from the
//! queue leaves the live ask alone, and a failure of it is said as nothing queued.
//!
//! **One instruction to every task, and who took it in** (#3904): `tell --all` sends one
//! follow-up to every task of this chat still working, numbered; a task acknowledges it with
//! `note --ack <n>`, and `list` says which tasks have.
//!
//! **Nothing here decides.** The checks in front of the ask are the ones that need no app: a
//! name purlis would draw, a brief that is there and carries no secret, a persona this project
//! defines. The persona is asked again by the app, with everything else.

use std::io::{IsTerminal, Read};
use std::process::ExitCode;

use purlis_core::dispatchdecision::{self, Persona, Refused};
use purlis_core::dispatched::{self, Answered, Asked, Reply, Waited, What};
use purlis_core::dispatchqueue::Queued;
use purlis_core::dispatchtalk::{self, Kind, Message};
use purlis_core::handback::Outcome;
use purlis_core::handoff;
use purlis_core::hookwire::{Answer, Ask, DispatchAsk, ReportBack, TaskReport};
use purlis_core::tui::Markup;

use crate::handoff::Ticketed;
use crate::voice;

/// What `purlis dispatch`'s own lines start with.
const SAYS: &str = "purlis dispatch:";

/// What `purlis dispatch report`'s own lines start with.
const REPORT_SAYS: &str = "purlis dispatch report:";

/// What every refusal ends with: a dispatch that did not start a chat says so.
const NOTHING: &str = "Nothing was started.";

/// How to pass a brief, said by every refusal about one.
const HEREDOC: &str = "Pass the brief as a quoted heredoc in the same call:\n  purlis dispatch \
                       --name \"<task>\" <<'BRIEF'\n  <the brief>\n  BRIEF";

#[derive(clap::Subcommand, Debug)]
pub enum DispatchCommand {
    /// Send this task's one report back to the chat that dispatched it: how it ended, what
    /// you did and found, and what changed. Write your session record first; purlis adds its
    /// path. The chat that asked reads the report on its next turn. A turn that ends without
    /// one is asked for it once; if the next turn ends without one too, purlis sends your last
    /// message in its place, marked as a report purlis made from its last message.
    Report {
        /// What you did and what you found, in a few lines.
        text: String,
        /// How the task ended: done, blocked or failed.
        #[arg(long)]
        outcome: String,
        /// What changed: files, commits, a branch. A few lines at most.
        #[arg(long)]
        changed: Option<String>,
    },
    /// Wait for the report of a task this chat dispatched, and print it. `purlis dispatch
    /// list` shows each task's chat number. A wait that runs out says the task is still
    /// running; its report reaches this chat when it lands all the same.
    Wait {
        /// The task's chat, by its number in `purlis dispatch list`.
        chat: u32,
        /// How long to wait, in seconds (default 100, at most 540).
        #[arg(long)]
        timeout: Option<u32>,
    },
    /// List the tasks this chat dispatched: each one's chat number, name, persona, where it
    /// works, its state and how long ago it started. A task ends at its report, and stays
    /// listed as finished until its row is cleared or this chat closes.
    List,
    /// Read what a task this chat dispatched said last and the commands it ran, with their
    /// exit codes, as its harness handed them to purlis: quoted as data, credential shapes
    /// masked. Only a task this chat dispatched can be read by it; the whole conversation is
    /// the person's, in the task's tab.
    Read {
        /// The task's chat, by its number in `purlis dispatch list`.
        chat: u32,
    },
    /// Close a finished task this chat dispatched: its finished row goes, and a task kept open
    /// after its report has its program ended. Refused for a task that has not reported, one
    /// in a turn, and one the person typed in. `purlis dispatch list` shows each task's state.
    Close {
        /// The task's chat, by its number in `purlis dispatch list`.
        chat: u32,
    },
    /// Cancel a task this chat dispatched: its turn is ended and it is asked for one short
    /// report of what it did, which arrives with the outcome `cancelled`. Only a task this
    /// chat dispatched can be cancelled by it, and not one the person typed in: that one is
    /// theirs. A task that reported while its program still runs can be cancelled too.
    Cancel {
        /// The task's chat, by its number in `purlis dispatch list`.
        chat: u32,
    },
    /// Send a follow-up to a task this chat dispatched that is still working, quoted as data
    /// from this chat: `tell <chat> "<text>"`, or `tell --all "<text>"` for every task still
    /// working. A task at rest is started on it at once; one mid-turn reads it when that turn
    /// ends. A task that reported while its program still runs is told too. Refused, with the
    /// task's state, once its program has ended. Each follow-up is an instruction with a
    /// number, which a task acknowledges with `note --ack <number>`; `list` shows which tasks
    /// have. Where the app does not answer, it is queued and delivered when it does.
    Tell {
        /// Send it to every task of this chat that is still working.
        #[arg(long)]
        all: bool,
        /// The task's chat, by its number in `purlis dispatch list` (not with --all), then what
        /// to add or correct, in a few lines.
        #[arg(num_args = 1..=2, required = true, value_name = "CHAT> <TEXT")]
        words: Vec<String>,
    },
    /// Answer the question a task this chat dispatched asked it. Only a question that task
    /// asked this chat can be answered: one it put to the person is the person's.
    Answer {
        /// The task's chat, by its number in `purlis dispatch list`.
        chat: u32,
        /// The answer, in a few lines.
        text: String,
    },
    /// From a dispatched task: send the chat that asked a progress note. It reads it on its
    /// next turn, and nothing waits on it. With `--ack <number>`, it acknowledges that
    /// instruction of the asking chat's, and the text may be left out.
    Note {
        /// The number of an instruction (a follow-up) of the asking chat's this task has taken in.
        #[arg(long)]
        ack: Option<u32>,
        /// Where the task has got to, in a few lines.
        text: Option<String>,
    },
    /// From a dispatched task: ask the chat that asked a question, and wait for its answer.
    /// A question only the person can answer is not asked this way: ask them in this chat.
    Ask {
        /// The question, in a few lines.
        question: String,
        /// How long to wait for the answer, in seconds (default 100, at most 540).
        #[arg(long)]
        timeout: Option<u32>,
    },
}

/// What a dispatch may say besides whom it is to, its name and its brief.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options<'a> {
    /// `--profile`: one of the project's harness profiles, and one of those the project
    /// lists for the persona where it lists any (#1509).
    pub profile: Option<&'a str>,
    /// `--in`: `worktree`, or `workspace:<name>` (#1453).
    pub place: Option<&'a str>,
    /// `--long`: a long-running task (#3905), whose notes are its heartbeat and which purlis
    /// never asks for a report.
    pub long: bool,
}

/// `purlis dispatch --name <task> [--to <persona>] [--profile <profile>] [--in <where>]
/// [--wait]`, with the brief on stdin. `waits` is how long to wait for the report, in seconds,
/// where the command was told to wait.
pub fn dispatch(
    here: &crate::Here,
    to: Option<&str>,
    name: Option<&str>,
    options: &Options<'_>,
    waits: Option<u32>,
) -> ExitCode {
    let Some(name) = name else {
        voice::err(format!(
            "{SAYS} a task needs a name, which the new chat is called and listed under: add \
             --name \"<task>\". {NOTHING}"
        ));
        return ExitCode::FAILURE;
    };
    // The name and the persona are asked before the read: neither needs the brief, and a
    // refusal that waited for stdin would wait on a terminal.
    if let Err(why) = checked(here, to, name).and_then(|_| placed(options.place)) {
        voice::err(refusal(why));
        return ExitCode::FAILURE;
    }
    let brief = match read_brief() {
        Ok(brief) => brief,
        Err(why) => {
            voice::err(refusal(why));
            return ExitCode::FAILURE;
        }
    };
    said(send(here, to, name, &brief, options, waits))
}

/// A refusal as said: the sentence escaped as one line, and [`HEREDOC`], purlis's own lines,
/// as written after it (SK-54).
fn refusal(why: String) -> Markup {
    match why.strip_suffix(HEREDOC) {
        Some(head) => Markup::new().text(head).own(HEREDOC),
        None => why.into(),
    }
}

/// `--in` as the app will read it, or the refusal: one of its three words, said before a brief
/// is read. Whether the workspace is there, and whether this chat works in a repo a worktree
/// can be cut from, are the app's to say, from its own record of this chat.
fn placed(place: Option<&str>) -> Result<Option<String>, String> {
    let asked = purlis_core::dispatchplace::asked(place)
        .map_err(|refused| format!("{SAYS} {} {NOTHING}", refused.say()))?;
    Ok(asked.map(|_| place.unwrap_or_default().trim().to_owned()))
}

/// Prints what a dispatch or a report answered: `Ok` on stdout, `Err` on stderr as a refusal.
fn said(answered: Result<String, String>) -> ExitCode {
    match answered {
        Ok(said) => {
            println!("{said}");
            ExitCode::SUCCESS
        }
        Err(why) => {
            voice::err(refusal(why));
            ExitCode::FAILURE
        }
    }
}

/// The task's name as purlis will draw it, and the persona as this project defines it: the
/// two questions answered without the brief.
fn checked(here: &crate::Here, to: Option<&str>, name: &str) -> Result<String, String> {
    // A task's own rule: a chat's name, not empty, and none of the marks purlis's own lines
    // are written with. The app holds it to the same rule again.
    let name = dispatchdecision::task_name(name)
        .map_err(|why| format!("{SAYS} --name: {why} {NOTHING}"))?;
    // The app asks this again, with everything else it decides. Asked here too so a persona
    // that is not there is said before a brief is read, in the decision's own sentence.
    if let Some(to) = to {
        let refused = match dispatchdecision::persona_in(here.plane.root(), to) {
            Persona::Unknown(name) => Some(Refused::NoPersona(name.to_owned())),
            Persona::Draft(name) => Some(Refused::Draft(name.to_owned())),
            Persona::Defined(_) | Persona::None => None,
        };
        if let Some(refused) = refused {
            return Err(format!("{SAYS} {} {NOTHING}", refused.say()));
        }
    }
    Ok(name)
}

/// Dispatches the task `name` with `brief` to `to` (this chat's own persona when none is
/// named): what the command prints and the `dispatch` tool answers, or the refusal.
///
/// Every check a caller could skip is made here, so the tool, which reads no stdin, is held to
/// exactly what the command is.
///
/// With `waits`, the answer goes on to the task's report, or to where the task stands once
/// that many seconds have passed ([`wait`]): a dispatch and its result in one call (#1441).
pub fn send(
    here: &crate::Here,
    to: Option<&str>,
    name: &str,
    brief: &str,
    options: &Options<'_>,
    waits: Option<u32>,
) -> Result<String, String> {
    let name = checked(here, to, name)?;
    let place = placed(options.place)?;
    if !brief
        .split(purlis_core::memstore::is_python_space)
        .any(|word| !word.is_empty())
    {
        return Err(format!("{SAYS} the brief is empty. {NOTHING} {HEREDOC}"));
    }
    // The leak guard's own classifier, as a handoff's brief is held to (#1315). The KIND,
    // never the matched text.
    if let Some(kind) = purlis_core::secretshape::kind_as_read(None, brief) {
        return Err(format!(
            "{SAYS} the brief looks like it carries a secret ({kind}). {NOTHING} A brief \
             travels to the new chat as a command-line argument any local process can read \
             while the harness starts, so it never carries a secret. Name where the credential \
             lives instead of pasting it, as the whole value on its line: `vault:<vault>/<key>`."
        ));
    }
    // Measured as the new chat is sent it: under the app's two lines, with the longest name a
    // chat can have standing in for the one the app will write. The app measures again.
    let longest = "x".repeat(purlis_core::reopen::MOST_LABEL);
    let sent = handoff::task_message(
        &longest,
        &here.place(None),
        chrono::Local::now().naive_local(),
        brief,
    );
    if let Some(bad) = handoff::bad_message(&sent) {
        return Err(format!(
            "{SAYS} {} The brief is {} bytes.",
            bad.say(),
            brief.len()
        ));
    }

    let (mut asking, chat, ticket) = match crate::handoff::ticketed() {
        Ticketed::Yes(asking, chat, ticket) => (asking, chat, ticket),
        Ticketed::Refused(why) => return Err(app_refused(&why)),
        Ticketed::NoApp => return Err(no_app()),
    };
    let ask = Ask::Dispatch(Box::new(DispatchAsk {
        chat,
        to: to.map(str::to_owned),
        name,
        brief: brief.to_owned(),
        profile: options
            .profile
            .map(str::trim)
            .filter(|profile| !profile.is_empty())
            .map(str::to_owned),
        place,
        ticket,
        long: options.long,
    }));
    // A worktree is cut before the chat starts: a checkout, which on a large repo takes far
    // longer than a start alone, and from a workspace's folder a question to the remote too.
    let persona = to.map(str::to_owned).or_else(|| here.active_persona(None));
    let cuts_a_worktree = may_cut_a_worktree(here.plane.root(), options.place, persona.as_deref());
    let within = if cuts_a_worktree {
        A_WORKTREE_TAKES_AT_MOST
    } else {
        crate::handoff::AN_OPEN_TAKES_AT_MOST
    };
    match asking.ask(&ask, within) {
        Ok(Answer::Dispatched {
            chat,
            name,
            persona,
            note,
            works,
        }) => {
            let who = match persona {
                Some(persona) => format!(" as {}", purlis_core::personas::one_line(&persona)),
                None => String::new(),
            };
            // What the app has to say about how it was started: today, that it runs on this
            // chat's profile because its persona's own is not offered here (D-1445-8).
            // Whole, never cut ([`whole`]): since #1453 it can say more than one thing (a
            // default worktree that gave way, a clone whose uncommitted changes stayed behind).
            let note = match note {
                Some(note) => format!(" Note: {}.", whole(&note)),
                None => String::new(),
            };
            // Where it works, in the app's words: another workspace, or a worktree of its own
            // with the branch purlis cut (#1453). The app says nothing where it is this
            // chat's folder.
            let works = match works {
                Some(works) => whole(&works),
                None => "in this chat's folder".to_owned(),
            };
            let name = purlis_core::personas::one_line(&name);
            let Some(within) = waits else {
                return Ok(format!(
                    "{SAYS} started '{name}'{who} (chat {chat}). It works {works}, and its \
                     report reaches this chat as context on its next turn.{note}"
                ));
            };
            // The chat is running either way: what the wait says is added to that, never
            // instead of it, so a wait that fails still leaves the chat's number said.
            let waited = match wait(chat, Some(within)) {
                Ok(said) | Err(said) => said,
            };
            Ok(format!(
                "{SAYS} started '{name}'{who} (chat {chat}), {works}.{note}\n{waited}"
            ))
        }
        // Held, not refused: the dispatch is accepted and waits on the person, so this is not
        // a failure and is not said as one.
        // Held because this chat runs on a pool, whatever the persona (#4148): no grant is asked.
        Ok(Answer::NeedsGrant {
            waiting: None,
            held_by: Some(pool),
            ..
        }) => Ok(held_on_a_pool(&pool, &ask_name(&ask))),
        Ok(Answer::NeedsGrant {
            from,
            to,
            waiting: None,
            held_by: None,
        }) => Ok(held_for_the_person(from.as_deref(), &to, &ask_name(&ask))),
        // Not held: the person is already being asked about this pair for another task, and
        // this one was dropped. Nothing of it will start, so it is a refusal.
        Ok(Answer::NeedsGrant {
            from,
            to,
            waiting: Some(first),
            ..
        }) => Err(not_held(from.as_deref(), &to, &ask_name(&ask), &first)),
        // Held, not refused: every check let it through, and it waits on this machine's memory
        // (#1467). Not a failure either.
        Ok(Answer::WaitingOnMemory { to }) => Ok(waiting_on_memory(to.as_deref(), &ask_name(&ask))),
        Ok(Answer::No { why }) => Err(app_refused(&why)),
        Ok(
            Answer::Ticket { .. }
            | Answer::Opened { .. }
            | Answer::Reported { .. }
            | Answer::Finished { .. }
            | Answer::Browsed { .. }
            | Answer::Previewed { .. }
            | Answer::Recorded { .. }
            | Answer::Written { .. }
            | Answer::Said { .. }
            | Answer::Alive {}
            | Answer::Drafted { .. }
            | Answer::Vaults { .. }
            | Answer::Keys { .. }
            | Answer::Working(_)
            | Answer::Pointed { .. }
            | Answer::Task(_),
        )
        | Err(_) => Err(no_answer(&ask_name(&ask), cuts_a_worktree)),
    }
}

/// Whether the app may cut a worktree for this dispatch, and so is given the longer wait
/// ([`A_WORKTREE_TAKES_AT_MOST`]): `--in worktree` or `--in worktree:<repo>`, or no `--in` to
/// a persona whose definition says `dispatch-isolation: worktree` (#3903), whose cut asks the
/// remote for its default branch where the asking chat stands in a workspace's own folder.
fn may_cut_a_worktree(root: &std::path::Path, place: Option<&str>, persona: Option<&str>) -> bool {
    use purlis_core::dispatchplace::{Where, asked, isolates};
    match asked(place) {
        Ok(Some(Where::Worktree(_))) => true,
        Ok(None) => persona.is_some_and(|persona| isolates(root, persona)),
        Ok(Some(Where::Workspace(_))) | Err(_) => false,
    }
}

/// How long the app has to answer a dispatch that cuts a worktree: a checkout of the repo,
/// then the chat's start. Still a bound, for the reason every wait here is one.
const A_WORKTREE_TAKES_AT_MOST: std::time::Duration = std::time::Duration::from_secs(120);

/// What a chat is told where the app did not answer in time. **It is not told nothing
/// started**: the app may still be cutting the worktree or starting the chat, and a second
/// dispatch of the same task would make a second chat.
fn no_answer(task: &str, cuts_a_worktree: bool) -> String {
    let task = purlis_core::personas::one_line(task);
    if cuts_a_worktree {
        return format!(
            "{SAYS} the purlis app did not answer in time, so purlis cannot say whether the \
             chat started. Cutting a worktree of a large repo can take longer than this \
             command waits, and the chat may still start. Look for '{task}' under this chat \
             in the explorer before you dispatch it again."
        );
    }
    format!(
        "{SAYS} the purlis app did not answer, so purlis cannot say whether the chat started. \
         Look for '{task}' under this chat in the explorer before you dispatch it again."
    )
}

/// The asking side of a pair, as a sentence names it.
fn who_asks(from: Option<&str>) -> String {
    match from {
        Some(from) => format!("'{}' chats", purlis_core::personas::one_line(from)),
        None => "this chat".to_owned(),
    }
}

/// What a chat is told where its dispatch waits on the person for a dispatch grant: nothing
/// has started, the person is being asked on this chat's tab, and what happens next.
fn held_for_the_person(from: Option<&str>, to: &str, task: &str) -> String {
    let one = purlis_core::personas::one_line;
    format!(
        "{SAYS} held for the person. {} may not dispatch to '{}' yet, so the person is \
         being asked on this chat's tab, with this brief in front of them. Nothing has started. \
         If they allow it, '{}' starts then and its report reaches this chat as context on a \
         later turn; if they keep it blocked, this chat is told on its next turn. Carry on \
         with other work, and do not dispatch it again.",
        who_asks(from),
        one(to),
        one(task)
    )
}

/// What a chat on pool `pool` is told where its task waits for the person's Approve (#4148).
fn held_on_a_pool(pool: &str, task: &str) -> String {
    let one = purlis_core::personas::one_line;
    format!(
        "{SAYS} held for the person. This chat runs on pool '{}', so the person approves what it \
         dispatches in the window, with this brief in front of them. Nothing has started. If \
         they approve it, '{}' starts then and its report reaches this chat as context on a \
         later turn; if they refuse it, this chat is told on its next turn. Carry on with other \
         work, and do not dispatch it again.",
        one(pool),
        one(task)
    )
}

/// What a chat is told where its task waits on this machine's memory (#1467): nothing has
/// started, it starts by itself once memory frees, and what it hears if it never does.
fn waiting_on_memory(to: Option<&str>, task: &str) -> String {
    let one = purlis_core::personas::one_line;
    let to = to.map_or_else(String::new, |to| format!(" as '{}'", one(to)));
    format!(
        "{SAYS} waiting on memory. This machine is short on memory, so '{}'{to} has not \
         started yet. It starts by itself once memory frees, held to the limits as they are \
         then, and its report reaches this chat as context on a later turn. If memory is still \
         short after {} minutes, nothing starts and this chat is told on its next turn. Carry \
         on with other work, and do not dispatch it again.",
        one(task),
        purlis_core::dispatchdecision::MEMORY_WAIT_MINUTES
    )
}

/// What a chat is told where the person is already being asked about this pair for the task
/// `first`: this ask was not held beside it, because the person was shown one brief.
fn not_held(from: Option<&str>, to: &str, task: &str, first: &str) -> String {
    let one = purlis_core::personas::one_line;
    format!(
        "{SAYS} not held. The person is already being asked whether {} may dispatch to \
         '{}', for the task '{}', and they were shown that task's brief, so only it \
         starts when they allow it. {NOTHING} Dispatch '{}' again once they have answered.",
        who_asks(from),
        one(to),
        one(first),
        one(task)
    )
}

/// What the app said, as one line and **whole**: escaped as anything a chat may have had a
/// hand in is, and never cut. A refusal says what to do in its last sentence, and the budget
/// a name is drawn within (160 characters) would cut most of them off before it.
fn whole(said: &str) -> String {
    purlis_core::shown::one_line(said, purlis_core::shown::NO_CLIP)
}

/// The name a dispatch ask carries.
fn ask_name(ask: &Ask) -> String {
    match ask {
        Ask::Dispatch(dispatch) => dispatch.name.clone(),
        _ => String::new(),
    }
}

fn app_refused(why: &str) -> String {
    let why = whole(why);
    // A refusal that already says nothing was started is not told so twice.
    if why.to_lowercase().contains("nothing was started") {
        format!("{SAYS} {why}")
    } else {
        format!("{SAYS} {why} {NOTHING}")
    }
}

fn no_app() -> String {
    format!(
        "{SAYS} no purlis app answered this call, so nothing was started. A dispatch starts a \
         chat in the app: run it from a chat the purlis app started."
    )
}

/// `purlis dispatch report`: the one report a task owes, sent to the chat that dispatched it.
///
/// **It names no recipient**, as a handoff's report names none: the app sends it to the chat
/// it recorded as this one's asker, and adds the path of the session record it wrote for this
/// chat. What this hands over is what only the chat can say: how it ended, its words, and what
/// it says changed.
pub fn report(outcome: &str, text: &str, changed: Option<&str>) -> Result<String, String> {
    let Some(outcome) = Outcome::of(outcome) else {
        return Err(format!(
            "{REPORT_SAYS} --outcome is one of done, blocked or failed, not '{}'. Nothing was \
             sent.",
            purlis_core::personas::one_line(outcome)
        ));
    };
    let summary =
        handoff::report_summary(text).map_err(|bad| format!("{REPORT_SAYS} {}", words(&bad)))?;
    let changed = match changed {
        None => None,
        Some(changed) => Some(
            handoff::report_summary(changed)
                .map_err(|bad| format!("{REPORT_SAYS} --changed: {}", words(&bad)))?,
        ),
    };
    let (mut asking, chat, ticket) = match crate::handoff::ticketed() {
        Ticketed::Yes(asking, chat, ticket) => (asking, chat, ticket),
        Ticketed::Refused(why) => return Err(not_sent(&why)),
        Ticketed::NoApp => {
            return Err(format!(
                "{REPORT_SAYS} no purlis app answered this call, so nothing was sent. A report \
                 goes back only from a chat a dispatch started."
            ));
        }
    };
    let back = ReportBack {
        chat,
        summary,
        ticket,
        task: Some(TaskReport { outcome, changed }),
    };
    match asking.ask(
        &Ask::Report(Box::new(back)),
        crate::handoff::A_TICKET_TAKES_AT_MOST,
    ) {
        // Delivered, and the app says the task is over (#1485): the chat is told its program
        // ends with this turn, so it starts nothing it would lose. Only where the app says
        // so: a handoff's chat, a blocked task and one the person started are not ended.
        Ok(Answer::Finished { to, kept_for: None }) => Ok(format!(
            "{REPORT_SAYS} sent to '{}' ({}). It reaches that chat as context on its next turn. \
             This task is finished: this chat's program is ended once this turn is over, so \
             start nothing more.",
            purlis_core::personas::one_line(&to),
            outcome.word()
        )),
        // The chat that asked has gone, and the task still ends at its report (#1510): the
        // person reads it on the task's record.
        Ok(Answer::Finished {
            to,
            kept_for: Some(kept),
        }) => Ok(format!(
            "{REPORT_SAYS} '{}' has closed, so the report is kept for {} ({}), and the person \
             reads it with this task. This task is finished: this chat's program is ended once \
             this turn is over, so start nothing more.",
            purlis_core::personas::one_line(&to),
            kept_place(&kept),
            outcome.word()
        )),
        Ok(Answer::Reported { to, kept_for: None }) => Ok(format!(
            "{REPORT_SAYS} sent to '{}' ({}). It reaches that chat as context on its next turn.",
            purlis_core::personas::one_line(&to),
            outcome.word()
        )),
        // The person started this task from that chat's tab, and that chat is gone: the
        // report is the person's, and stays with this chat for them (D-1443-9).
        Ok(Answer::Reported {
            to,
            kept_for: Some(kept),
        }) if kept == purlis_core::handback::FOR_THE_PERSON => Ok(format!(
            "{REPORT_SAYS} '{}' has closed, and the person started this task from its tab, so \
             the report is theirs: this chat is marked as needing them, and they read it here.",
            purlis_core::personas::one_line(&to),
        )),
        Ok(Answer::Reported {
            to,
            kept_for: Some(kept),
        }) => Ok(format!(
            "{REPORT_SAYS} '{}' has closed, so the report is kept for {}. The next chat that \
             starts there reads it.",
            purlis_core::personas::one_line(&to),
            kept_place(&kept),
        )),
        Ok(Answer::No { why }) => Err(not_sent(&why)),
        Ok(
            Answer::Ticket { .. }
            | Answer::Opened { .. }
            | Answer::Recorded { .. }
            | Answer::Written { .. }
            | Answer::Said { .. }
            | Answer::Alive {}
            | Answer::Drafted { .. }
            | Answer::Vaults { .. }
            | Answer::Keys { .. }
            | Answer::Working(_)
            | Answer::Pointed { .. }
            | Answer::Dispatched { .. }
            | Answer::NeedsGrant { .. }
            | Answer::WaitingOnMemory { .. }
            | Answer::Task(_)
            | Answer::Browsed { .. }
            | Answer::Previewed { .. },
        )
        | Err(_) => Err(format!(
            "{REPORT_SAYS} the purlis app did not answer, so nothing was sent."
        )),
    }
}

/// A report refusal in this command's words, as its recorded answers have it.
/// Where a report with nowhere to go was kept, as a sentence names it: a workspace or the
/// project root, by `Place::word`.
fn kept_place(kept: &str) -> String {
    match purlis_core::active::Place::read(kept) {
        Some(place) => place.said(),
        None => format!("'{}'", purlis_core::personas::one_line(kept)),
    }
}

fn words(bad: &handoff::BadReport) -> String {
    match bad {
        handoff::BadReport::Empty => "the report is empty, so nothing was sent. Say what was \
                                      done in a few lines."
            .to_owned(),
        handoff::BadReport::TooLong(_) | handoff::BadReport::Undrawable => bad.say(),
    }
}

fn not_sent(why: &str) -> String {
    format!("{REPORT_SAYS} {} — nothing was sent.", whole(why))
}

/// `purlis dispatch report …`, `wait`, `list` and `cancel`, printed.
pub fn run(command: &DispatchCommand) -> ExitCode {
    match command {
        DispatchCommand::Report {
            text,
            outcome,
            changed,
        } => said(report(outcome, text, changed.as_deref())),
        DispatchCommand::Wait { chat, timeout } => said(wait(*chat, *timeout)),
        DispatchCommand::List => said(list()),
        DispatchCommand::Read { chat } => said(read(*chat)),
        DispatchCommand::Close { chat } => said(close(*chat)),
        DispatchCommand::Cancel { chat } => said(cancel(*chat)),
        DispatchCommand::Tell { all, words } => said(tell_words(*all, words)),
        DispatchCommand::Answer { chat, text } => said(answer(Via::Command, *chat, text)),
        DispatchCommand::Note { ack, text } => said(note(Via::Command, text.as_deref(), *ack)),
        DispatchCommand::Ask { question, timeout } => said(ask(Via::Command, question, *timeout)),
    }
}

/// A message's text as the app will take it, or the refusal, said before anything is asked.
fn message(text: &str) -> Result<String, String> {
    dispatchtalk::text(text).map_err(|why| format!("{SAYS} {why}"))
}

/// What became of one message: the app's answer, or that it waits in this chat's queue.
enum Fate {
    Answered(Answered),
    /// Queued, and not delivered yet: the app did not answer, or a limit holds it back.
    Queued,
    /// Delivered by another look of the app's at this chat's queue.
    DeliveredEarlier,
}

/// **What a message was sent with** (#3914): the command, or purlis's tool for it, which this
/// process serves ([`crate::mcp::serve`]). A sender a queue refused is never pointed to the
/// tool it used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Command,
    Tool,
}

/// **One message handed to the app**: queued first where this process may write the queue,
/// then delivered by the app (#3902); asked live where it may not.
fn sent(via: Via, what: What, queued: Queued) -> Result<Fate, String> {
    sent_on(via, &mut None, what, queued)
}

/// [`sent`], asked on `on` where the caller holds a connection to the app already, and on a
/// new one held there otherwise: a question and the wait for its answer are asked on one.
fn sent_on(
    via: Via,
    on: &mut Option<(purlis_core::hookwire::Asking, u32)>,
    what: What,
    queued: Queued,
) -> Result<Fate, String> {
    use purlis_core::hookwire::{CHAT_ENV, ChatToken, SOCKET_ENV};
    const WITHIN: std::time::Duration = crate::handoff::A_TICKET_TAKES_AT_MOST;
    let socket = purlis_core::envvar::var_os(SOCKET_ENV).filter(|s| !s.is_empty());
    let chat = purlis_core::envvar::var(CHAT_ENV).and_then(|chat| chat.parse::<u32>().ok());
    let (Some(socket), Some(chat), Some(token)) = (socket, chat, ChatToken::from_env()) else {
        return match on.as_mut() {
            Some((asking, chat)) => answered(asking, *chat, what, WITHIN).map(Fate::Answered),
            None => asked(what, WITHIN).map(Fate::Answered),
        };
    };
    let socket = std::path::PathBuf::from(socket);
    let now = chrono::Utc::now().timestamp();
    let queued = purlis_core::dispatchqueue::queue(&socket, chat, &token, queued, now);
    if on.is_none() {
        *on = the_app();
    }
    let unqueued = match queued {
        Ok(id) => {
            let Some((asking, chat)) = on.as_mut() else {
                return Ok(Fate::Queued);
            };
            return match ask_app(asking, *chat, What::Deliver { id }, WITHIN) {
                Ok(Answered::Queued {
                    delivered: false, ..
                })
                | Err(Failed::NoAnswer) => Ok(Fate::Queued),
                Ok(Answered::Queued {
                    delivered: true, ..
                }) => Ok(Fate::DeliveredEarlier),
                Ok(answered) => Ok(Fate::Answered(answered)),
                Err(Failed::Refused(why)) => Err(why),
            };
        }
        Err(purlis_core::dispatchqueue::Unqueued::Full) => {
            return Err(format!(
                "{SAYS} {}. The app has not answered for a while: `purlis dispatch list` shows \
                 where this chat's messages stand.",
                purlis_core::dispatchqueue::Unqueued::Full.say()
            ));
        }
        Err(unqueued) => unqueued,
    };
    match on.as_mut() {
        Some((asking, chat)) => match ask_app(asking, *chat, what, WITHIN) {
            Ok(answered) => Ok(Fate::Answered(answered)),
            Err(Failed::Refused(why)) => Err(why),
            Err(Failed::NoAnswer) => Err(not_queued(via, &unqueued)),
        },
        None => Err(not_queued(via, &unqueued)),
    }
}

/// What a sender is told where the app did not answer and the message could not be queued.
fn not_queued(via: Via, unqueued: &purlis_core::dispatchqueue::Unqueued) -> String {
    let harness = purlis_core::hookwire::chat_harness(&purlis_core::envvar::var);
    not_queued_for(
        unqueued,
        via,
        purlis_core::dispatchqueue::tool_path(harness.as_deref()),
    )
}

/// [`not_queued`], for a message sent `via` the command or a tool, in a chat whose harness's
/// tools reach the queue or not (`tools`, [`purlis_core::dispatchqueue::tool_path`]): only the
/// command, where the tools queue, is pointed to the tool (#3914).
fn not_queued_for(
    unqueued: &purlis_core::dispatchqueue::Unqueued,
    via: Via,
    tools: purlis_core::dispatchqueue::ToolPath,
) -> String {
    use purlis_core::dispatchqueue::ToolPath;
    let instead = match (via, tools) {
        (Via::Command, ToolPath::Queues) => {
            ", or with purlis's dispatch tool for it, which queues it"
        }
        (Via::Tool, _) | (Via::Command, ToolPath::KeptOut) => "",
    };
    format!(
        "{SAYS} the purlis app did not answer, and {}. Nothing was sent. Send it again once the \
         app answers{instead}.",
        unqueued.say()
    )
}

/// What a sender is told of a message that waits in its queue (#3902).
fn queued_said(what: &str) -> String {
    format!(
        "{SAYS} {what} queued, not yet delivered: the purlis app did not take it yet. It waits \
         in this chat's queue, and purlis delivers it, in order and once, when the app next \
         hears from this chat. `purlis dispatch list` shows where it stands."
    )
}

/// What a sender is told of a queued message another look of the app's delivered.
fn delivered_earlier(what: &str) -> String {
    format!(
        "{SAYS} {what} delivered, with what this chat queued before it. `purlis dispatch list` \
         shows where it stands."
    )
}

/// The app's answer to a message it delivered, as `(kind, to, instruction)`.
fn sent_to(answered: Answered) -> Result<(String, Option<u32>), String> {
    match answered {
        Answered::Sent {
            to, instruction, ..
        } => Ok((purlis_core::personas::one_line(&to), instruction)),
        _ => Err(format!(
            "{SAYS} the purlis app did not answer, so nothing was sent."
        )),
    }
}

/// `purlis dispatch tell`'s words, as clap hands them: `<chat> <text>`, or `--all <text>`.
fn tell_words(all: bool, words: &[String]) -> Result<String, String> {
    match (all, words) {
        (true, [text]) => tell_all(Via::Command, text),
        (true, _) => Err(format!(
            "{SAYS} `tell --all` takes the text alone: `purlis dispatch tell --all \"<text>\"`. \
             Nothing was sent."
        )),
        (false, [chat, text]) => match chat.parse::<u32>() {
            Ok(chat) => tell(Via::Command, chat, text),
            Err(_) => Err(format!(
                "{SAYS} '{}' is not a task's chat number: `purlis dispatch list` shows them. \
                 Nothing was sent.",
                purlis_core::personas::one_line(chat)
            )),
        },
        (false, _) => Err(format!(
            "{SAYS} `tell` takes a task's chat number and the text: `purlis dispatch tell \
             <chat> \"<text>\"`, or `--all \"<text>\"` for every task still working. Nothing \
             was sent."
        )),
    }
}

/// How a task acknowledges instruction `number`, as a sender is told it.
fn acknowledged_by(number: u32) -> String {
    format!(
        "It is instruction {number}: the task acknowledges it with `purlis dispatch note --ack \
         {number}`, and `purlis dispatch list` shows which tasks have."
    )
}

/// `purlis dispatch tell <chat> "<text>"`: a follow-up to a task this chat dispatched.
pub fn tell(via: Via, to: u32, text: &str) -> Result<String, String> {
    let text = message(text)?;
    let queued = Queued::Tell {
        to,
        text: text.clone(),
    };
    let answered = match sent(via, What::Tell { to, text }, queued)? {
        Fate::Answered(answered) => answered,
        Fate::Queued => return Ok(queued_said(&format!("follow-up to chat {to}"))),
        Fate::DeliveredEarlier => return Ok(delivered_earlier(&format!("follow-up to chat {to}"))),
    };
    let (name, instruction) = sent_to(answered)?;
    let number = instruction
        .map(|number| format!(" {}", acknowledged_by(number)))
        .unwrap_or_default();
    Ok(format!(
        "{SAYS} follow-up sent to '{name}' (chat {to}), quoted as data from this chat. If it is \
         at rest, purlis starts its turn on it now; if it is mid-turn, it reads it when that \
         turn ends.{number}"
    ))
}

/// `purlis dispatch tell --all "<text>"`: one follow-up to every task of this chat still
/// working (#3904).
pub fn tell_all(via: Via, text: &str) -> Result<String, String> {
    let text = message(text)?;
    let queued = Queued::TellAll { text: text.clone() };
    let answered = match sent(via, What::TellAll { text }, queued)? {
        Fate::Answered(answered) => answered,
        Fate::Queued => return Ok(queued_said("follow-up to every task still working")),
        Fate::DeliveredEarlier => {
            return Ok(delivered_earlier("follow-up to every task still working"));
        }
    };
    let Answered::SentAll { instruction, to } = answered else {
        return Err(format!(
            "{SAYS} the purlis app did not answer, so nothing was sent."
        ));
    };
    let each: Vec<String> = to
        .iter()
        .map(|to| {
            format!(
                "'{}' (chat {}) {}",
                purlis_core::personas::one_line(&to.name),
                to.chat,
                to.state.say()
            )
        })
        .collect();
    Ok(format!(
        "{SAYS} instruction {instruction} sent to every task of this chat still working, quoted \
         as data: {}. Each task acknowledges it with `purlis dispatch note --ack \
         {instruction}`, and `purlis dispatch list` shows which have.",
        each.join("; ")
    ))
}

/// `purlis dispatch answer <chat> "<text>"`: the answer to a question a task asked this chat.
pub fn answer(via: Via, to: u32, text: &str) -> Result<String, String> {
    let text = message(text)?;
    let queued = Queued::Answer {
        to,
        text: text.clone(),
    };
    let answered = match sent(via, What::Answer { to, text }, queued)? {
        Fate::Answered(answered) => answered,
        Fate::Queued => return Ok(queued_said(&format!("answer to chat {to}"))),
        Fate::DeliveredEarlier => return Ok(delivered_earlier(&format!("answer to chat {to}"))),
    };
    let (name, _) = sent_to(answered)?;
    Ok(format!(
        "{SAYS} answer sent to '{name}' (chat {to}), which carries on with it."
    ))
}

/// `purlis dispatch note "<text>"`: a progress note to the chat that dispatched this one; with
/// `ack`, the acknowledgement of that instruction of its (#3904), the text then optional.
pub fn note(via: Via, text: Option<&str>, ack: Option<u32>) -> Result<String, String> {
    // With an acknowledgement, a note left out or empty sends nothing more.
    let text = match text {
        Some(text) if ack.is_none() || !text.trim().is_empty() => message(text)?,
        _ if ack.is_some() => String::new(),
        _ => message("")?,
    };
    let queued = Queued::Note {
        text: (!text.is_empty()).then(|| text.clone()),
        ack,
    };
    let answered = match sent(via, What::Note { text, ack }, queued)? {
        Fate::Answered(answered) => answered,
        Fate::Queued => return Ok(queued_said("progress note")),
        Fate::DeliveredEarlier => return Ok(delivered_earlier("progress note")),
    };
    if let Answered::Acknowledged { instruction, to } = answered {
        return Ok(format!(
            "{SAYS} instruction {instruction} acknowledged to '{}'.",
            purlis_core::personas::one_line(&to)
        ));
    }
    let (name, _) = sent_to(answered)?;
    let acked = ack
        .map(|number| format!(" It acknowledges instruction {number}."))
        .unwrap_or_default();
    Ok(format!(
        "{SAYS} progress note sent to '{name}'.{acked} It reads it on its next turn; nothing \
         waits on it, so carry on."
    ))
}

/// `purlis dispatch ask "<question>"`: a question to the chat that dispatched this one, and
/// the wait for its answer.
///
/// **This chat pauses on it.** The command holds until the answer comes and prints it, quoted
/// as data. If the wait runs out first, it says to end the turn: the answer is then handed to
/// this chat's next turn, which purlis starts when the answer lands.
pub fn ask(via: Via, question: &str, within: Option<u32>) -> Result<String, String> {
    let text = message(question)?;
    let within = dispatched::wait_secs(within.unwrap_or(dispatched::WAITS_BY_DEFAULT));
    let mut on = the_app();
    if on.is_none() && purlis_core::hookwire::ChatToken::from_env().is_none() {
        return Err(format!(
            "{SAYS} no purlis app answered this call, so nothing was sent. A question goes \
             back only from a chat a dispatch started."
        ));
    }
    let asked_text = text.clone();
    let queued = Queued::Question { text: text.clone() };
    let asker = match sent_on(via, &mut on, What::Question { text }, queued)? {
        Fate::Answered(Answered::Sent { to, .. }) => purlis_core::personas::one_line(&to),
        // Queued (#3902): the question is asked when the app takes it, and this chat is paused
        // on it from now, as on one asked.
        Fate::Queued | Fate::DeliveredEarlier => {
            return Ok(format!(
                "{} This chat is paused on that question: do not guess, and end this turn now. \
                 The answer is handed to a later turn as context, quoted as data.",
                queued_said("question")
            ));
        }
        Fate::Answered(_) => {
            return Err(format!(
                "{SAYS} the purlis app did not answer, so nothing was sent."
            ));
        }
    };
    let held = std::time::Duration::from_secs(u64::from(within)) + A_WAIT_IS_ANSWERED_WITHIN;
    let paused = format!(
        "{SAYS} question sent to '{asker}', which has not answered after {within} seconds. \
         This chat is paused on that question: do not guess, and end this turn now. The answer \
         is handed to this chat's next turn as context, quoted as data."
    );
    // The question is asked, and the wait is on the connection it was asked on.
    let Some((mut asking, chat)) = on else {
        return Ok(paused);
    };
    let replied = answered(
        &mut asking,
        chat,
        What::AwaitAnswer {
            within_secs: within,
        },
        held,
    );
    match replied {
        Ok(Answered::Replied {
            what:
                Reply::Answered {
                    from,
                    text,
                    by_person,
                },
        }) => {
            // On the same connection, so the next turn is not handed the same answer again.
            let _ = answered(
                &mut asking,
                chat,
                What::GotAnswer,
                crate::handoff::A_TICKET_TAKES_AT_MOST,
            );
            // The person answered it, in the purlis window (#1496): the app says so, on the
            // connection this command asked on, and the sentence is the person's own.
            if by_person {
                return Ok(dispatchtalk::person_said(
                    &dispatchtalk::PersonSaid::Answered {
                        // The sentence for a task names no number: the question is its own, quoted.
                        number: 0,
                        question: asked_text,
                        text,
                    },
                ));
            }
            Ok(dispatchtalk::said(&Message {
                kind: Kind::Answer,
                from,
                chat: 0,
                text,
                instruction: None,
            }))
        }
        Ok(Answered::Replied {
            what: Reply::AskerGone,
        }) => Ok(format!(
            "{SAYS} '{asker}', the chat that asked for this task, has closed, so the question \
             has nobody to answer it. Finish what you can and send the task's report, saying \
             what the question was."
        )),
        // The question is asked either way: an app that stopped answering leaves this chat
        // where a wait that ran out does.
        Ok(_) | Err(_) => Ok(paused),
    }
}

/// How long a turn's hook waits on the app for what the person said, for each of the ask and
/// the answer: it holds the turn, so an app that is slow costs the turn nothing, and what the
/// person said is handed to the next one.
const A_HOOK_WAITS: std::time::Duration = std::time::Duration::from_millis(750);

/// What the person said to this chat, as its turn is told it, and the means of saying the
/// turn has it ([`from_the_person`]).
pub struct FromThePerson {
    /// purlis's sentences with the person's words quoted under each, as data.
    pub told: String,
    /// The question each is about, by number: what the turn says it has.
    numbers: Vec<u32>,
    chat: u32,
    /// The connection they were read on, which the acknowledgement goes back on.
    asking: purlis_core::hookwire::Asking,
}

impl FromThePerson {
    /// **The turn has them**: said to the app, by each one's number, so they are handed to no
    /// later turn. **Called once the hook has printed its context, and not before**: a hook
    /// that is killed before it prints has said nothing here, and the next turn is handed
    /// them again. Briefly: an acknowledgement the app is slow to take costs only that.
    pub fn handed_over(mut self) {
        let has = Ask::Task(Box::new(Asked {
            chat: self.chat,
            what: What::HasFromThePerson {
                numbers: self.numbers,
            },
        }));
        let _ = self.asking.ask(&has, A_HOOK_WAITS / 3);
    }
}

/// **What the person said to this chat in the purlis window** since it last said it had any
/// (#1496): their answer to a question this task asked its asking chat, or word that they
/// answered a question one of this chat's tasks asked it. `None` for nothing, outside a chat
/// the app started, and where the app does not answer.
///
/// **Asked of the app and read from no file.** That the person said it is the app's to say,
/// on the connection it answers this chat on: a file is something another chat can write.
///
/// **Reading is not having.** The app keeps each one until the turn says it has it
/// ([`FromThePerson::handed_over`]), which the hook does after it has printed.
pub fn from_the_person() -> Option<FromThePerson> {
    let (mut asking, chat) = the_app()?;
    let asked = Ask::Task(Box::new(Asked {
        chat,
        what: What::FromThePerson,
    }));
    let Ok(Answer::Task(answered)) = asking.ask(&asked, A_HOOK_WAITS) else {
        return None;
    };
    let Answered::FromThePerson { said } = *answered else {
        return None;
    };
    let told = dispatchtalk::person_context(&said)?;
    Some(FromThePerson {
        told,
        numbers: said.iter().map(dispatchtalk::PersonSaid::number).collect(),
        chat,
        asking,
    })
}

/// **The reports waiting for `whose` that this turn is handed**, read and then taken.
///
/// **The app takes them.** In a chat the app started, the hook reads the reports and the app
/// takes them off disk and says which it wrote there (`What::TookHandbacks`): the folder is the
/// app's alone to write, and a report the app did not write there is handed as a chat's words
/// alone (`handback::handed`). Where no app answers, they are taken here, as a hook outside any
/// sandbox may, and only those the plane's key shows the app wrote are handed whole.
pub fn handbacks(
    root: &std::path::Path,
    whose: purlis_core::handback::For<'_>,
) -> Vec<purlis_core::handback::Handback> {
    let waiting = purlis_core::handback::waiting(root, whose);
    let Some(folder) = waiting.first().map(|one| one.folder.clone()) else {
        return Vec::new();
    };
    let names: Vec<String> = waiting.iter().map(|one| one.name.clone()).collect();
    let by_the_app = the_app().and_then(|(mut asking, chat)| {
        let asked = Ask::Task(Box::new(Asked {
            chat,
            what: What::TookHandbacks {
                folder: folder.clone(),
                names: names.clone(),
            },
        }));
        match asking.ask(&asked, A_HOOK_WAITS) {
            Ok(Answer::Task(answered)) => match *answered {
                Answered::Took { taken } => Some(taken),
                _ => None,
            },
            _ => None,
        }
    });
    let taken =
        by_the_app.unwrap_or_else(|| purlis_core::handback::took_waiting(root, &folder, &names));
    taken.handed(&waiting)
}

/// **The messages waiting for chat `chat` that this turn is handed**, read and then taken: by the
/// app where it answers (`What::TookMessages`), as the folder is the app's alone to write, and
/// here otherwise.
pub fn messages(root: &std::path::Path, chat: u32) -> Vec<dispatchtalk::Message> {
    let waiting = dispatchtalk::waiting(root, chat);
    if waiting.is_empty() {
        return Vec::new();
    }
    let names: Vec<String> = waiting.iter().map(|(name, _)| name.clone()).collect();
    let by_the_app = the_app().and_then(|(mut asking, asker)| {
        let asked = Ask::Task(Box::new(Asked {
            chat: asker,
            what: What::TookMessages {
                names: names.clone(),
            },
        }));
        match asking.ask(&asked, A_HOOK_WAITS) {
            Ok(Answer::Task(answered)) => match *answered {
                Answered::TookMessages { taken } => Some(taken),
                _ => None,
            },
            _ => None,
        }
    });
    let taken = by_the_app.unwrap_or_else(|| dispatchtalk::took(root, chat, &names));
    waiting
        .into_iter()
        .filter(|(name, _)| taken.contains(name))
        .map(|(_, message)| message)
        .collect()
}

/// **What this chat's router points `prompt` at** (SK-15c): asked of the app that started the
/// chat, which asks the router within its 100 ms. `None` where there is no app, it answers
/// nothing in time, or the router points at nothing.
pub fn pointed(prompt: &str) -> Option<String> {
    use purlis_core::hookwire::PromptAsk;
    let (mut asking, chat) = the_app()?;
    // Cut here, so no more of it crosses to the app than the router is handed.
    let asked = Ask::Prompt(Box::new(PromptAsk {
        chat,
        prompt: purlis_core::extension::studio::prompt_handed(prompt).to_owned(),
    }));
    match asking.ask(&asked, A_ROUTER_IS_ANSWERED_WITHIN) {
        Ok(Answer::Pointed { pointer }) => pointer,
        _ => None,
    }
}

/// How long a prompt hook waits for the app's answer about its router: the router's own 100 ms
/// and the way to the app and back. Past it the prompt goes on with no pointer.
const A_ROUTER_IS_ANSWERED_WITHIN: std::time::Duration = std::time::Duration::from_millis(400);

/// How much longer than the wait itself the app has to answer one: it answers when the wait
/// ends, and the answer is one line.
const A_WAIT_IS_ANSWERED_WITHIN: std::time::Duration = std::time::Duration::from_secs(10);

/// A conversation with the app that started this chat, and the chat's number: what an ask
/// that needs no ticket is made on.
fn the_app() -> Option<(purlis_core::hookwire::Asking, u32)> {
    use purlis_core::hookwire::{Asking, CHAT_ENV, ChatToken, SOCKET_ENV};

    let socket = purlis_core::envvar::var_os(SOCKET_ENV).filter(|s| !s.is_empty())?;
    let chat = purlis_core::envvar::var(CHAT_ENV).and_then(|chat| chat.parse::<u32>().ok())?;
    let asking = Asking::on(std::path::Path::new(&socket), ChatToken::from_env()).ok()?;
    Some((asking, chat))
}

/// One ask after a dispatched task, answered: what the app said of it, or the refusal.
fn asked(what: What, within: std::time::Duration) -> Result<Answered, String> {
    let Some((mut asking, chat)) = the_app() else {
        return Err(format!(
            "{SAYS} no purlis app answered this call. A chat's dispatched tasks are the app's \
             to say: run it from a chat the purlis app started."
        ));
    };
    answered(&mut asking, chat, what, within)
}

/// The app's refusal of an ask after a task or of a message, as it is printed: one line and
/// **whole** ([`whole`]). These sentences end with what to do, and most are longer than the
/// budget a name is drawn within.
fn task_refused(why: &str) -> String {
    format!("{SAYS} {}", whole(why))
}

/// Why an ask after a task came back with no answer to use.
enum Failed {
    /// The app refused it, in this sentence.
    Refused(String),
    /// The app did not answer.
    NoAnswer,
}

/// One ask after a task, answered: the app's answer, its refusal, or no answer.
fn ask_app(
    asking: &mut purlis_core::hookwire::Asking,
    chat: u32,
    what: What,
    within: std::time::Duration,
) -> Result<Answered, Failed> {
    match asking.ask(&Ask::Task(Box::new(Asked { chat, what })), within) {
        Ok(Answer::Task(answered)) => Ok(*answered),
        Ok(Answer::No { why }) => Err(Failed::Refused(task_refused(&why))),
        Ok(_) | Err(_) => Err(Failed::NoAnswer),
    }
}

fn answered(
    asking: &mut purlis_core::hookwire::Asking,
    chat: u32,
    what: What,
    within: std::time::Duration,
) -> Result<Answered, String> {
    ask_app(asking, chat, what, within).map_err(|failed| match failed {
        Failed::Refused(why) => why,
        Failed::NoAnswer => format!(
            "{SAYS} the purlis app did not answer. `purlis dispatch list` shows where this \
             chat's tasks stand."
        ),
    })
}

/// `purlis dispatch wait <chat>`: the report of task `of`, as this chat's next turn would have
/// been handed it, or where the task stands once the wait has run out.
///
/// **A wait that runs out is an answer, not a failure**: the task is still running, its report
/// still reaches this chat when it lands, and the sentence says how to look again.
pub fn wait(of: u32, within: Option<u32>) -> Result<String, String> {
    let within = dispatched::wait_secs(within.unwrap_or(dispatched::WAITS_BY_DEFAULT));
    let Some((mut asking, chat)) = the_app() else {
        return Err(format!(
            "{SAYS} no purlis app answered this call, so there is no report to wait for here. \
             Run it from a chat the purlis app started."
        ));
    };
    let held = std::time::Duration::from_secs(u64::from(within)) + A_WAIT_IS_ANSWERED_WITHIN;
    let waited = answered(
        &mut asking,
        chat,
        What::Wait {
            of,
            within_secs: within,
        },
        held,
    )?;
    let Answered::Waited { of, name, what } = waited else {
        return Err(format!("{SAYS} the purlis app did not answer the wait."));
    };
    let name = purlis_core::personas::one_line(&name);
    Ok(match what {
        Waited::Reported { report } => {
            // Said on the same connection, so the turn after this one is not handed the same
            // report again. Unsaid, by a command that was killed here, the report is still
            // waiting for that turn: nothing is lost either way.
            let _ = answered(
                &mut asking,
                chat,
                What::Read { of },
                crate::handoff::A_TICKET_TAKES_AT_MOST,
            );
            purlis_core::handback::context(&[*report], false).unwrap_or_default()
        }
        Waited::Running { state } => format!(
            "{SAYS} '{name}' (chat {of}) has not reported after {within} seconds: it is {}. \
             It is still running, and its report reaches this chat as context when it lands. \
             `purlis dispatch wait {of}` waits again, and `purlis dispatch list` shows where \
             it stands.",
            purlis_core::personas::one_line(&state)
        ),
        Waited::Asks { question } => {
            // Read here, so the turn after this one is not handed the question again.
            let _ = answered(
                &mut asking,
                chat,
                What::Read { of },
                crate::handoff::A_TICKET_TAKES_AT_MOST,
            );
            format!(
                "{}\nThen `purlis dispatch wait {of}` waits on for its report.",
                dispatchtalk::said(&Message {
                    kind: Kind::Question,
                    from: name,
                    chat: of,
                    text: question,
                    instruction: None,
                })
            )
        }
        Waited::AlreadyRead => format!(
            "{SAYS} '{name}' (chat {of}) reported earlier, and its report was handed to this \
             chat then. The app no longer holds its text."
        ),
        Waited::Ended => {
            format!("{SAYS} '{name}' (chat {of}) failed: its chat ended without a report.")
        }
    })
}

/// `purlis dispatch list`: the tasks this chat dispatched.
pub fn list() -> Result<String, String> {
    match asked(What::List, crate::handoff::A_TICKET_TAKES_AT_MOST)? {
        Answered::Listed { rows, messages } => Ok(match dispatchtalk::records_text(&messages) {
            Some(messages) => format!("{}\n\n{messages}", dispatched::list_text(&rows)),
            None => dispatched::list_text(&rows),
        }),
        _ => Err(format!("{SAYS} the purlis app did not answer the list.")),
    }
}

/// `purlis dispatch close <chat>`: closes finished task `of` (#1981 D-4).
pub fn close(of: u32) -> Result<String, String> {
    match asked(What::Close { of }, crate::handoff::A_TICKET_TAKES_AT_MOST)? {
        Answered::Closed { of, name } => Ok(format!(
            "{SAYS} closed '{name}' (chat {of}). Its report stays on its dispatch record; the \
             Dispatches tab shows it."
        )),
        _ => Err(format!("{SAYS} the purlis app did not answer the close.")),
    }
}

/// `purlis dispatch read <chat>`: what task `of` said last and the commands it ran (#1981).
pub fn read(of: u32) -> Result<String, String> {
    match asked(What::Output { of }, crate::handoff::A_TICKET_TAKES_AT_MOST)? {
        Answered::Output {
            of,
            name,
            finals,
            ran,
        } => {
            let output = dispatched::output_text(&name, of, &finals, &ran);
            // And what became of this chat's messages to it (#3902, #3904): one more ask, and
            // nothing said where the app does not answer it.
            let messages = match asked(What::List, crate::handoff::A_TICKET_TAKES_AT_MOST) {
                Ok(Answered::Listed { messages, .. }) => messages
                    .into_iter()
                    .filter(|record| record.to_all || record.to.iter().any(|to| to.chat == of))
                    .collect(),
                _ => Vec::new(),
            };
            Ok(match dispatchtalk::records_text(&messages) {
                Some(messages) => format!("{output}\n\n{messages}"),
                None => output,
            })
        }
        _ => Err(format!("{SAYS} the purlis app did not answer the read.")),
    }
}

/// `purlis dispatch cancel <chat>`: ends task `of`'s turn and asks it for a short report.
pub fn cancel(of: u32) -> Result<String, String> {
    match asked(What::Cancel { of }, crate::handoff::A_TICKET_TAKES_AT_MOST)? {
        Answered::Cancelling { of, name } => Ok(format!(
            "{SAYS} cancelling '{}' (chat {of}). Its report reaches this chat with the \
             outcome `cancelled`. Where purlis may send its chat keys, its turn is ended now \
             and it is asked for one short report of what it did. Where it may not (the chat \
             has shown the person a prompt this turn, the person has typed in it, or its \
             harness is one purlis does not type into), the cancel takes effect when that \
             turn ends. `purlis dispatch wait {of}` waits for the report here.",
            purlis_core::personas::one_line(&name)
        )),
        _ => Err(format!("{SAYS} the purlis app did not answer the cancel.")),
    }
}

/// The brief on stdin, or the sentence saying why there is none.
///
/// Asked of the terminal before any read, as a handoff's is: a read on a terminal blocks for
/// ever with nothing on screen to say why.
fn read_brief() -> Result<String, String> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Err(format!(
            "{SAYS} reads its brief from stdin, and stdin here is a terminal. {NOTHING} \
             {HEREDOC}"
        ));
    }
    let mut bytes = Vec::new();
    if stdin.lock().read_to_end(&mut bytes).is_err() {
        return Err(format!(
            "{SAYS} this shell has no stdin to read a brief from. {NOTHING} {HEREDOC}"
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| format!("{SAYS} the brief on stdin is not UTF-8 text. {NOTHING}"))
}

#[cfg(test)]
mod tests;
