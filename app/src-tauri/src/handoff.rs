// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The app's half of `charter handoff`: opening, in a window the operator is looking at, the
//! chat a chat of its own handed work to (charter-app#204).
//!
//! `charter handoff` does every check of the request in front of the open, and then asks the
//! app over the hook socket, in two lines on one connection: a ticket, and the open that
//! spends it. `purlis_core::hookwire::OpenChat` argues what that ticket is worth and what it
//! is not, and it is not repeated here.
//!
//! **A handoff is a dispatch in handoff mode, and this app decides it** (#1444, spec #1434
//! decisions 4 and 5). The harness's permission prompt consents to nothing. What does is the
//! dispatch grant: a handoff to the asking chat's own persona needs none, and one to another
//! persona is held with its brief until the person allows the pair on the asking chat's tab,
//! or is refused plainly for a chat nobody is at. Its limits, its depth, a policy's locks and
//! its profile are a task's, because one function decides both ([`dispatch_it`]). Nobody
//! approves the brief: it is the new chat's first message, it is in the dispatch's record,
//! and the Notice for a first grant shows it in full.
//!
//! What this module adds is **the questions only the app can answer**, and the one control
//! that module's argument rests on, which is visibility:
//!
//! - **The asking chat is one this app has open**, and the new chat starts on **its persona's
//!   own profile** where that persona's definition names one, else on the asking chat's, read
//!   from the app's own record of that chat (#1445). Either way it is a profile the project
//!   already offers on this machine and has approved
//!   (`purlis_core::personaprofile::for_dispatch`): a persona's file is one a chat can write,
//!   so a name it gives is only looked up, never run. One this machine does not offer falls
//!   back to the asking chat's profile, and the answer and the new chat's stamp say so
//!   (D-1445-8). The harness follows the profile, so a Claude Code chat can hand work to a
//!   persona that runs on Codex, and the first message reaches it by that harness's own route.
//! - **The first message carries the stamp** of a handoff from that chat
//!   (`purlis_core::handoff::is_stamped_from`), so the chat the operator finds on the strip
//!   always says where it came from.
//! - **It lands on a strip, and it does not take the operator's screen.** The window is told
//!   ([`ARRIVED`]) and draws a tab in the target workspace's strip without bringing it to the
//!   front and without raising the window. See [`Arrived`] for why.
//! - **It is named for its task** (charter-app#258): the `--name` the handoff carried, or the
//!   ordinary `<persona> <N>`. And it says where it came from by the parent's NAME — in its
//!   first message, and in the note its tab and header draw — never by the parent's number.
//!
//! And it answers the one ask a dispatched chat makes back: **a report** (charter-app#259,
//! #1436). The chat it goes to is the asker the app recorded when it started the chat, never
//! one the reporting chat names, and only for a task, or for a handed-off chat in the one turn
//! its stop gives it. **A handoff owes no report** (#1515, #1471): an open that still asks for
//! one is refused, naming `purlis dispatch`. See [`report_it`].
//!
//! # A dispatched task (#1436)
//!
//! A **task** is the other mode of the same act (ADR 0090, #1434): one chat starting another,
//! which runs as a persona and owes the chat that asked one report. It is answered here, on the
//! same road: the ticket, the stamped first message, the decision, the chat's start, the
//! lineage on its record and the report. What differs is what the request carries. A handoff's
//! carries a stamp and the workspace the work moves to, which this module checks ([`moving`]);
//! a task's carries a task's name. For both, **everything else is this app's own record of the
//! asking chat** ([`dispatch_it`]): who it is, where it works, what it runs as, and whether it
//! may. `purlis_core::dispatchdecision::decide` answers that last question, for every caller.
//!
//! # Where a task works (#1453)
//!
//! A dispatch may say one word about where its chat works: another workspace, or a worktree of
//! its own (`purlis_core::dispatchplace`). **The app cuts that worktree, never a chat**, by
//! the brokered route, under a folder and a branch purlis names for the dispatch; the persona
//! chat is started in it, on the sandbox the project compiles for its persona in that folder,
//! and purlis merges nothing for it by itself: only the person does, from the task's Changes
//! tab (#1511). Which branch that is, is written on the dispatch's record,
//! and the report names it from there.

use std::sync::Arc;
use std::time::Instant;

use purlis_core::active::Place;
use purlis_core::dispatchdecision::{self, Mode};
use purlis_core::engine::Size;
use purlis_core::hookwire::{Answer, Ask, DispatchAsk, OpenChat, TaskReport, Tickets};
use purlis_core::reopen::{Chat, HandedFrom, Owed};

use crate::planes::{Held, PlaneId};

/// The event the window is sent when a handoff has opened a chat.
pub const ARRIVED: &str = "handoff-arrived";

/// A chat a handoff opened, as the window needs it to draw a tab.
///
/// # Where it lands, and why not in front
///
/// **A tab in the target workspace's strip, in the same window, behind whatever is in front.**
/// The operator runs many chats at once, and a handoff is by construction work they sent
/// *away* from the chat they are reading. A tab that took the front would cut them off
/// mid-sentence, and one filed in another workspace that took the front would move them to a
/// workspace they did not ask to look at. Python's handoff opens its chat in a background
/// window for the same reason: the design is "opened without taking your screen"
/// (`docs show handoff`).
///
/// **The window is not raised either.** An operator who has switched to another app did so on
/// purpose; the chat is on the strip when they come back.
///
/// **The one exception is a window with no tab at all**, where the new one takes the front:
/// there is nothing to interrupt, and a strip with a tab and nothing in front of it is a blank
/// pane that looks broken.
///
/// That the chat is on a strip at all, visibly, is what makes a handoff from inside the app
/// acceptable: see the justification on `purlis_core::hookwire::OpenChat`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Arrived {
    pub plane: PlaneId,
    pub session: u32,
    /// The chat's own name — its number, which the tab's default puts after the persona.
    pub name: String,
    /// The task name the handoff gave it (`--name`), which the tab says instead of its default
    /// (charter-app#258).
    pub label: Option<String>,
    /// Where it came from, for the note its tab and header draw.
    pub from: Option<crate::HandedFromNote>,
    /// The workspace whose strip it is filed on, or none for a task dispatched by a chat at the
    /// project's root, which is filed where that chat is.
    pub workspace: Option<String>,
    pub persona: Option<String>,
    /// The harness it runs, by the word the plane calls it — what its tab's default name puts
    /// before the number when it adopted no persona (charter-app#254).
    pub harness: Option<String>,
}

/// Told when a handoff has opened a chat. The event carries its plane.
pub type Arrivals = Arc<dyn Fn(Arrived) + Send + Sync + 'static>;

/// A task a chat asked for, decided and started as every dispatch is: what the line is
/// answered.
fn task_dispatched(
    held: &Held,
    plane: &PlaneId,
    wanted: &Wanted,
    arrived: &(dyn Fn(Arrived) + Send + Sync),
) -> Answer {
    match dispatch_it(held, plane, wanted, STARTING) {
        Ok(Dispatched::Started {
            it, note, works, ..
        }) => {
            let answer = Answer::Dispatched {
                chat: it.session,
                name: it.label.clone().unwrap_or_else(|| it.name.clone()),
                persona: it.persona.clone(),
                note,
                works,
            };
            crate::dispatched::started(held, it.session);
            arrived(*it);
            answer
        }
        Ok(Dispatched::Held {
            from, to, waiting, ..
        }) => Answer::NeedsGrant { from, to, waiting },
        Ok(Dispatched::WaitsOnMemory { to }) => match waits_on_memory(held, wanted) {
            Ok(()) => Answer::WaitingOnMemory { to },
            Err(why) => no(why),
        },
        Err(why) => no(why),
    }
}

/// Answers one ask on the hook socket of `held`'s plane.
///
/// `connection` is the listener's number for the connection the ask came on, which is what a
/// ticket is bound to.
pub fn answer(
    held: &Held,
    plane: &PlaneId,
    tickets: &Tickets,
    connection: u64,
    ask: Ask,
    arrived: &(dyn Fn(Arrived) + Send + Sync),
) -> Answer {
    let now = Instant::now();
    match ask {
        Ask::Ticket { chat } => {
            // Only for a chat this app is running. The ticket's chat is the one whose profile
            // the new chat inherits, so a number the app has no chat under has nothing to
            // inherit and nothing to mint for.
            if !is_open(held, chat) {
                return no(format!("chat {chat} is not one this app has open"));
            }
            // **A chat the person is stopping is still given a ticket** (D-T59-j9). Its one
            // last report is sent on one, by either report command (D-1448-4). What it may
            // not do is start a chat, and that is refused where the ticket is spent on an
            // open or a dispatch, which is where it is known what the ticket is for.
            match tickets.mint(chat, connection, now) {
                Ok(ticket) => Answer::Ticket { ticket },
                Err(why) => no(why),
            }
        }
        Ask::Open(open) => {
            // Spent FIRST, before anything below can refuse, so a request that is refused for
            // any reason has used its ticket up.
            if let Err(why) = tickets.spend(open.chat, connection, &open.ticket, now) {
                return no(why);
            }
            // **A handoff that asks for a report is retired** (#1471): no command of purlis's
            // sends one since `purlis handoff --report` sends a dispatch (#1515), and a line
            // that still does is refused, its ticket spent, naming the one route for work that
            // reports back. Nothing opens a handoff that owes a report.
            if open.older_report {
                return no(an_open_asking_a_report(&open));
            }
            // Decided as every dispatch is (#1444): started, held for the person, or refused.
            let wanted = Wanted::moved(&open);
            match dispatch_it(held, plane, &wanted, STARTING) {
                Ok(Dispatched::Started { it, note, .. }) => {
                    let chat = it.session;
                    arrived(*it);
                    Answer::Opened { chat, note }
                }
                Ok(Dispatched::Held {
                    from, to, waiting, ..
                }) => Answer::NeedsGrant { from, to, waiting },
                // Waits on memory, as a task does (#1467): opened once memory frees.
                Ok(Dispatched::WaitsOnMemory { to }) => match waits_on_memory(held, &wanted) {
                    Ok(()) => Answer::WaitingOnMemory { to },
                    Err(why) => no(why),
                },
                Err(why) => no(why),
            }
        }
        Ask::Report(back) => {
            // Spent first, for the open's reason.
            if let Err(why) = tickets.spend(back.chat, connection, &back.ticket, now) {
                return no(why);
            }
            report_it(
                held,
                back.chat,
                &back.summary,
                back.task.as_ref(),
                Voice::Chat,
            )
            .unwrap_or_else(no)
        }
        Ask::Dispatch(dispatch) => {
            // Spent first, for the open's reason: one run of the command starts one chat.
            if let Err(why) = tickets.spend(dispatch.chat, connection, &dispatch.ticket, now) {
                return no(why);
            }
            task_dispatched(held, plane, &Wanted::of(&dispatch), arrived)
        }
        // A brokered write, not a handoff: no ticket, because a record is the chat's own to
        // write and the line can only name the chat whose token it carries (#1332).
        Ask::SessionRecord(record) => crate::smartclose::record(held, &record),
        // Another brokered write: the project's own files, for the chat that asks (#1333).
        Ask::Write(write) => crate::brokered::write(held, &write),
        // A workspace the chat asks the person to remove (qw356): no ticket, for the record's
        // reason. The line names a workspace; the app removes nothing for it, and holds the
        // question for the person on the tab of the chat whose token the line carries.
        Ask::Removal(removal) => crate::removalasks::asked(held, &removal),
        // A brokered git action (#1335): no ticket, for the record's reason — the line names
        // only the chat whose token it carries, and what it asks is checked against the app's
        // record of that chat.
        Ask::Git(git) => crate::gitbroker::answer(held, &git),
        // A commit in the branch folder the asking chat stands in (#1055): no ticket, for the
        // record's reason. The line names a message and paths; the folder and the branch are
        // this app's record of the chat whose token it carries.
        Ask::Commit(commit) => crate::gitbroker::commit(held, &commit),
        // `purlis vault list` from a sandboxed chat (#1430): no ticket, because it changes
        // nothing, and whose vaults it lists is the app's record of the chat the token is for.
        Ask::Vaults { chat } => crate::vaults::list_for_chat(held, chat),
        // `purlis secret list <vault>` from a sandboxed chat (qw385): no ticket, for the
        // listing's reason. The project's listener answers it before this, with the window's
        // teller so a refusal raises the vault's Notice (`planes.rs`); here it is answered the
        // same way with the refusal noted for the Notice and told to nobody.
        Ask::Keys { chat, vault } => {
            crate::vaults::keys_for_chat(held, plane, &|_| {}, chat, &vault)
        }
        // A proposed change (#1975): no ticket, for the removal's reason. The line names a file
        // and a change; the app writes nothing for it, and holds it for the person on the tab
        // of the chat whose token the line carries.
        Ask::Propose(proposal) => crate::proposals::asked(held, &proposal),
        // Where the asking chat is working (#1450): no ticket, for the record's reason. The
        // line names only the chat whose token it carries, and the picture is this app's own
        // record of its open chats.
        Ask::WhereWorking(asks) => held
            .chats()
            .working(
                asks.chat,
                asks.tell,
                &|chat| {
                    let glance = held.board().glance(chat);
                    (glance.state, glance.own_work)
                },
                &|chat| task_prompt(held, chat),
            )
            .map_or_else(
                || no(format!("chat {} is not one this app has open", asks.chat)),
                |working| {
                    // Whether each of its tasks is alive, and the ones orphaned by a restart
                    // (#3900): only a turn's answer carries a roster.
                    let working = match asks.tell {
                        purlis_core::awareness::Tell::Turn => working
                            .with_liveness(|task| Some(crate::dispatched::liveness(held, task)))
                            .with_orphans(crate::unstarted::orphans_of(held, asks.chat)),
                        _ => working,
                    };
                    Answer::Working(Box::new(working))
                },
            ),
        // An ask after a task this chat dispatched (#1441): no ticket, for the record's reason,
        // and whose task it names is the app's record of that chat.
        Ask::Task(asked) => crate::dispatched::answer(held, &asked, connection),
        // A browser tool's call (#2013): no ticket, for the record's reason. The line names a
        // browser of the chat whose token it carries, looked up among that chat's alone.
        Ask::Browse(browse) => crate::chatbrowsers::answered(held, &browse),
        // The theme specimen (#2330): no ticket, for the record's reason. The line names a theme
        // by its id, an appearance and option values; where the chat works, and whose drafts are
        // its own, are this app's record of the chat whose token the line carries (#2536). The
        // picture is of the specimen's own sample content.
        Ask::ThemePreview(asks) => crate::specimen::answered(
            &|chat| chat_at(held, chat),
            // This machine's store as the app resolved it at startup, as a theme draft's ask
            // reads it: never looked up again here.
            held.config().map(std::path::Path::to_path_buf),
            crate::specimen::painter().as_deref(),
            &asks,
        ),
        // A theme draft (#2332): no ticket, for the record's reason. The line names a theme
        // folder; whose draft it is, and where that chat works, are this app's record of the
        // chat whose token it carries.
        Ask::ThemeDraft(draft) => crate::themedrafts::asked(held, &draft),
    }
}

/// What chat `chat` is stopped on for the person, as the chat that asked for it is told: a
/// permission its harness asked on the hook purlis holds, or another prompt its harness said
/// it waits on (a question, or a permission on a harness that asks no hook); or a sandbox
/// block's Notice raised in the turn it ended on, which it was told to wait on (#1663).
fn task_prompt(held: &Held, chat: u32) -> Option<purlis_core::awareness::Prompt> {
    use purlis_core::awareness::Prompt;
    let glance = held.board().glance(chat);
    if held.asks_open_for(chat) {
        Some(Prompt::Permission)
    } else if glance.waits_on_its_prompt() {
        Some(Prompt::Other)
    } else if held.chats().blocks().waits_on_a_notice(
        chat,
        glance.turns,
        glance.state == purlis_core::state::State::Waiting,
    ) {
        Some(Prompt::SandboxBlock)
    } else {
        None
    }
}

/// Writes task `chat`'s report for it, with `outcome`, in the app's own voice: for a test that
/// stands in for a report, and that a stop refuses. Not in the app: a cancelled task's report
/// is written under its cancel's hold ([`report_for_held`]).
#[cfg(test)]
pub(crate) fn report_for(
    held: &Held,
    chat: u32,
    text: &str,
    outcome: purlis_core::handback::Outcome,
) -> Result<(), String> {
    let task = TaskReport {
        outcome,
        changed: None,
    };
    report_it(held, chat, text, Some(&task), Voice::Purlis).map(|_| ())
}

/// **Writes task `chat`'s report for it**, with `outcome`: the report of a cancelled task whose
/// chat sent none (#1441, #1459). The app's own sentence, delivered as any report is, under
/// the caller's hold of the lock a report is taken under: one the chat sent first is its one
/// report, and this is refused. Answers the asking chat to tell once that hold is let go
/// ([`after_a_report`]).
pub(crate) fn report_for_held(
    held: &Held,
    chat: u32,
    text: &str,
    outcome: purlis_core::handback::Outcome,
    _deciding: &Deciding<'_>,
) -> Result<Option<u32>, String> {
    let task = TaskReport {
        outcome,
        changed: None,
    };
    report_under(held, chat, text, Some(&task), false).map(|(_, tell)| tell)
}

/// What follows a report of task `chat` once the lock it was taken under is let go: the chat
/// that asked is woken, `tell` where it was told, and **the task's program is now to be
/// ended** (#1485).
pub(crate) fn after_a_report(held: &Held, chat: u32, tell: Option<u32>) {
    if let Some(asker) = tell {
        // Typed the line where it takes one. Where it does not, and this was the last task
        // it waited on, its held end of turn is the person's from here (#1491).
        crate::dispatched::told_or_settled(held, asker);
    } else if let Some(from) = held.chats().handed_from(chat) {
        // Its report reached no chat, and it owes none now: whoever waited on it waits no
        // more.
        crate::dispatched::settle_above(held, from.chat);
    }
    // The files it changed are read now, with the lock let go and its folder as it left it
    // (#1457): after the report is kept, so a slow read holds up nobody's report.
    crate::dispatches::files_changed(held, chat);
    // **Delivered, and only then is the task's program to be ended** (#1485): the report is
    // kept for the chat that asked and its record says sent, both under the lock just let go.
    // The end takes that lock again as a close does, so the exit it causes finds nothing
    // owed ([`its_program_ended`]) and marks nothing failed.
    crate::dispatched::delivered(held, chat);
}

/// What an open that still asks for a report (an older command line's `--report`) is refused
/// with (#1471): work that needs an answer is a task, and the command that sends one into the
/// workspace it named, **to the persona it named** (none for the chat's own). Where the older
/// line was to create that workspace, the refusal names the command that creates it first: a
/// dispatch into a workspace that is not there yet is refused before it would say so.
fn an_open_asking_a_report(open: &OpenChat) -> String {
    let one = purlis_core::personas::one_line;
    let workspace = one(&open.workspace);
    // A name the line sent that is no persona's name is not echoed as one: the placeholder
    // stands in for it, as it did before.
    let to = match open.persona.as_deref().map(str::trim) {
        None | Some("") => String::new(),
        Some(persona) if purlis_core::personas::valid_name(persona) => format!(" --to {persona}"),
        Some(_) => " --to <persona>".to_owned(),
    };
    let dispatch = format!("purlis dispatch --name \"<task>\"{to} --in workspace:{workspace}");
    match open.create_vision {
        Some(_) => format!(
            "a handoff asks for no report, so nothing was opened and workspace '{workspace}' \
             was not created. Work this chat needs an answer from is a task, sent into a \
             workspace that exists: create it first with purlis workspace create {workspace} \
             --vision \"<the goal>\", then {dispatch}"
        ),
        None => format!(
            "a handoff asks for no report, so nothing was opened. Work this chat needs an \
             answer from is a task: {dispatch}"
        ),
    }
}

/// Whose words a report is: the chat's own, or purlis's, written in its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Voice {
    Chat,
    /// **purlis writing in a task's place never takes a stop's last words** (#1488): those
    /// are the one report a stopped chat itself may send. And while the person is stopping
    /// the task nothing is written for it at all: the stop tells the chat that asked.
    Purlis,
}

/// Hands `summary` back from `chat` to the chat whose handoff opened it, or says why not
/// (charter-app#259).
///
/// **The recipient is the app's record, never the request.** `chat` is the one this ticket was
/// minted for, and the chat its report goes to is the parent the app wrote into that chat's own
/// record when it opened it ([`HandedFrom`]). Nothing the reporting chat says can point it
/// anywhere else.
///
/// **A report goes to the chat that asked, and not to the person** (#1448), a handoff's and a
/// task's alike. While that chat is open the report is left for its next turn ([`deliver`])
/// and raises no needs-you item, on either chat: the asking chat reads it, and the chat that
/// wrote it now waits on that chat. When the asking chat has gone the report has nowhere to
/// go, and that is a needs-you item on the chat that wrote it; the report is kept for the
/// workspace, where the next chat to start reads it.
///
/// **A chat the person is stopping may send one last report whatever it owed** (#1448): its
/// stop asked for it ([`crate::stopping`]). It is taken in one call, and given back if the
/// report is then refused or cannot be kept, so one last turn is one report. That turn is the
/// only report a handed-off chat sends: a handoff owes none (#1471), and its stop's report is
/// delivered as its words alone, with no outcome of a task's.
///
/// **Every report is sent by `purlis dispatch report`**, with how the work ended (#1471). A
/// line without that part, which only an older `purlis handoff report` wrote, is refused with
/// the command that sends it.
///
/// **A task's report** (#1436) is the same report with three more parts: its outcome and what
/// changed, which the persona chat says, and its session record's path, which is this app's
/// own record of what it wrote for that chat and never a path the chat named.
fn report_it(
    held: &Held,
    chat: u32,
    summary: &str,
    task: Option<&TaskReport>,
    voice: Voice,
) -> Result<Answer, String> {
    // From "is one owed" to "one was sent" under one lock, so two reports in flight are one
    // report and one refusal, and a restart of the chat that asked lands on one side of it.
    let deciding = held.chats().deciding();
    // Asked under the lock a stop is recorded under, so it is one or the other.
    if voice == Voice::Purlis && held.stopping().is_stopping(chat) {
        let how = match held.stopping().limit_of(chat) {
            // purlis's stop at a limit says so, never "by the person" (#1512).
            Some(reached) => purlis_core::dispatched::being_stopped_as(reached),
            None => "being stopped by the person".to_owned(),
        };
        return Err(format!(
            "chat {chat} is {how}, and its stop tells the chat that asked"
        ));
    }
    // The one report a stop asks for, tested and taken in one call.
    let last_words = voice == Voice::Chat && held.stopping().take_last_report(chat);
    // **A cancel's one report is an ordinary report** (#1459): its outcome is `cancelled`, the
    // app's record, and it carries no word of a stop. Only the person's stop, or purlis's at a
    // limit they set, makes a report the stop's last words.
    let a_stop_s = last_words && !held.stopping().is_cancelling(chat);
    let said = report_under(held, chat, summary, task, a_stop_s);
    if said.is_err() && last_words {
        held.stopping().give_back_last_report(chat);
    }
    // The lock is let go before anything is typed: a program that has stopped reading its
    // terminal must not hold up every dispatch and report in the project (#1441).
    drop(deciding);
    let (answer, tell) = said?;
    after_a_report(held, chat, tell);
    Ok(answer)
}

/// [`report_it`], under its lock. `last_words` says the chat is being stopped and this is the
/// one report its stop asked for. Answers, beside the answer, the asking chat to tell that a
/// task's report landed, once the lock is let go.
fn report_under(
    held: &Held,
    chat: u32,
    summary: &str,
    task: Option<&TaskReport>,
    last_words: bool,
) -> Result<(Answer, Option<u32>), String> {
    use purlis_core::handback;

    let from = held.chats().handed_from(chat).ok_or_else(|| {
        // A chat reopened from a finished task is an ordinary chat (#1485, V100-8): the task
        // it was has reported, and nothing it says now is a report to anyone.
        if let Some(was) = crate::finished::reopened_from(held, chat) {
            return purlis_core::dispatched::reopened_reports_nothing(&was);
        }
        format!(
            "chat {chat} was not started by a dispatch, so there is no chat waiting on a report \
             from it"
        )
    })?;
    // **A task's stop takes a report only while one is owed** (#1488): one that reported
    // before the person's stop was recorded ended by itself, and has no second report. A chat
    // handed its work may send its stop's one report whatever it owed, as before.
    let last_words = last_words && (from.mode == Mode::Handoff || from.report == Owed::Due);
    match (from.report, from.mode) {
        (Owed::Due, Mode::Task) => {}
        _ if last_words => {}
        // **A task kept open reports once more** (#1768): its report came out blocked, or the
        // person took it up again after it. Not once the person's stop is recorded: a task
        // that had reported has no report of its stop's (#1488).
        (Owed::Again, Mode::Task) if !held.stopping().is_stopping(chat) => {}
        // **A handoff owes no report** (#1515, #1471): the work moved to a chat the person
        // reads themselves. Whatever its record says it owes, outside its stop's one turn.
        (_, Mode::Handoff) => {
            return Err(format!(
                "this chat was handed its work, and a handoff owes no report, so '{}' is not \
                 waiting on one. Say what you found to the person instead",
                from.name
            ));
        }
        (Owed::Nothing, Mode::Task) => {
            return Err(format!(
                "'{}' is not waiting on a report from this chat",
                from.name
            ));
        }
        (Owed::Sent | Owed::Again, Mode::Task) => {
            return Err(format!(
                "this chat has already reported to '{}', and a dispatched task gets one \
                 report — already reported",
                from.name
            ));
        }
        // purlis reported for it (#1443): its program had ended, or its tab was closed and
        // reopened, before it said a word. That was the task's one report.
        (Owed::Failed, _) => {
            return Err(format!(
                "purlis already told '{}' that this chat ended without a report, and a task \
                 gets one report — already reported. Say what you found to the person instead",
                from.name
            ));
        }
    }
    // **Every report says how the work ended** (#1471): a line without it was written by an
    // older `purlis handoff report`, and is answered with the command that sends one. That
    // holds for a chat an older build opened by a handoff that asked for a report, too, which
    // is read as a task (#1519).
    let Some(said) = task else {
        return Err(format!(
            "this chat owes '{}' a report, and a report says how the work ended. Send it with \
             `purlis dispatch report --outcome done \"<what you did and found>\"`, or \
             --outcome blocked or failed",
            from.name
        ));
    };
    // **Which kind of report it is, is this app's record of how the chat was started**, never
    // what the line says. A handed-off chat's stop report is delivered as its words alone: a
    // handoff has no outcome of a task's (#1471). What it said of how it ended is still what
    // its dispatch's record keeps, so the log says what the command echoed to it.
    let said_outcome = said.outcome;
    let task = match from.mode {
        Mode::Handoff => None,
        Mode::Task => Some(said),
    };
    let summary = purlis_core::handoff::report_summary(summary).map_err(|bad| bad.say())?;
    // **The branch a worktree task worked on is this app's record of what it cut** (#1453),
    // read from the dispatch's own record and never from the line: a chat cannot report another
    // branch as its own. What it says of branches rides `changed`, quoted as its words.
    let cut_for_it = crate::dispatches::branch_of(held, chat);
    // What changed is the chat's words too, and held to the rule its report is.
    let task = match task {
        None => None,
        Some(said) => Some(handback::Task {
            // A cancelled task's outcome is the app's record, whatever its chat says (#1441).
            outcome: crate::dispatched::outcome_for(held, chat, said.outcome)?,
            changed: match said.changed.as_deref() {
                None => None,
                Some(changed) => {
                    Some(purlis_core::handoff::report_summary(changed).map_err(|bad| bad.say())?)
                }
            },
            record: crate::restored::its_session_record(held, chat)
                .and_then(|path| handback::record_path(&path)),
            by_person: from.by_person,
            unreported: false,
            // The app's own record of the person's keys in that pane, and only the fact
            // (#1442).
            stepped_in: crate::dispatched::stepped_in(held, chat),
            branch: cut_for_it,
        }),
    };
    // The dispatch's record ends with the report (#1452), saying how it ended: a task's
    // outcome as the app holds it, and a handed-off chat's as it said it (#1471). Nobody
    // cancels a handoff, so a handoff that says so is recorded as done.
    let outcome = match task.as_ref().map_or(said_outcome, |task| task.outcome) {
        handback::Outcome::Blocked => purlis_core::dispatchrecord::Outcome::Blocked,
        handback::Outcome::Failed => purlis_core::dispatchrecord::Outcome::Failed,
        // A task its asking chat cancelled is recorded as that, and not as a failure of the
        // work (D-1441-15, D-T59-j5).
        handback::Outcome::Cancelled if task.is_some() => {
            purlis_core::dispatchrecord::Outcome::Cancelled
        }
        handback::Outcome::Cancelled | handback::Outcome::Done => {
            purlis_core::dispatchrecord::Outcome::Done
        }
        // purlis's word alone (#1981), refused from a chat by `outcome_for` before here.
        handback::Outcome::LastMessage => purlis_core::dispatchrecord::Outcome::LastMessage,
    };
    // What it says changed is kept on the record with the report's text (#1452).
    let changed = task.as_ref().and_then(|task| task.changed.clone());
    // **A report a task sends once the person's stop is recorded is the stop's report**
    // (#1488, V100-6), whatever it says of itself and whether or not it had been asked yet:
    // it is delivered as purlis's own word that the person stopped the task, which carries it
    // quoted as data and names what was stopped below. The stop was recorded under the lock
    // this is taken under, so the two are in one order: a report that landed first was an
    // ordinary report, and the task had ended by itself.
    let the_stop_s = (last_words && task.is_some()).then(|| handback::Stopped {
        wrote: true,
        task: true,
        by_person: from.by_person,
        // Its session record is named by the report's own part.
        record: None,
        below: held.stopping().ended_below(chat),
        // purlis's stop at a limit the person set, where it was that (#1512).
        limit: held.stopping().limit_of(chat),
        // Named by the report's own part.
        branch: None,
    });
    let delivered = deliver(held, chat, &from, summary.clone(), task, the_stop_s)?;
    // A task's report that came out blocked leaves it one more (#1768), unless it is its
    // stop's: the stop ends it.
    let blocked = from.mode == Mode::Task
        && !last_words
        && outcome == purlis_core::dispatchrecord::Outcome::Blocked;
    held.chats()
        .owes(chat, if blocked { Owed::Again } else { Owed::Sent });
    // A report a chat sends in the one turn its stop gave it is still a stop's: the person
    // ended it, whatever it says of itself, and its row does not fold (#1485).
    let by = last_words.then_some(match held.stopping().limit_of(chat) {
        Some(_) => purlis_core::dispatchrecord::EndedBy::Limit,
        None => purlis_core::dispatchrecord::EndedBy::Person,
    });
    let way = last_words.then_some(purlis_core::dispatchrecord::EndedWay::Stopped);
    if from.report == Owed::Again {
        // The one more report (#1768): its record ends with it, and keeps the one before.
        let taken_over =
            crate::dispatches::reported_again(held, chat, outcome, &summary, changed.as_deref());
        // **A task the person took up again is still not ended by purlis** (#1485): their
        // key stood its end down for good, and this report does not bring one back.
        if taken_over {
            held.tasks().ledger().end_stood_down(chat);
        }
    } else {
        crate::dispatches::reported(held, chat, outcome, &summary, changed.as_deref(), by, way);
    }
    // **Whether its program is now to be ended is the ledger's own word**, set as the report
    // was delivered (#1485): from what this app holds of that delivery, never from a record on
    // disk. A task whose asking chat has gone ends at its report too (#1510, V100-64).
    let ends = held.tasks().ledger().ending(chat);
    if delivered.kept_for.is_none() || ends {
        held.board().reported_to_its_asker(chat);
    } else if !last_words {
        // Nowhere to go, and the chat that wrote it stays open: the person is told there. So
        // is the report of a task the person started whose tab chat is gone (D-1443-9), and a
        // blocked one whose asking chat has gone.
        held.needs_the_person(
            chat,
            purlis_core::state::Need::ReportUndelivered {
                asker: delivered.to.clone(),
            },
        );
    }
    let tell = (delivered.reached_the_chat && from.mode == Mode::Task).then_some(from.chat);
    // **The chat is told its program will be ended only where it will be** (#1485). Never a
    // handoff's chat, a blocked task or one the person started.
    let answer = if ends {
        Answer::Finished {
            to: delivered.to,
            kept_for: delivered.kept_for,
        }
    } else {
        Answer::Reported {
            to: delivered.to,
            kept_for: delivered.kept_for,
        }
    };
    Ok((answer, tell))
}

/// Where [`deliver`] left what a chat said.
pub(crate) struct Delivered {
    /// The chat that asked, as the person sees it.
    pub to: String,
    /// The place it was kept for, by `Place::word`, when that chat has gone: a workspace's
    /// name or the plane root's word (SI-1b).
    pub kept_for: Option<String>,
    /// Whether it waits for the asking chat's own next turn.
    pub reached_the_chat: bool,
}

/// **The one delivery from a chat to the chat that started it** (charter-app#259, #1448):
/// `summary` from `chat`, with a task's `task` part where it is a task's report, to the chat
/// `from` names. A report travels by it, whoever wrote it — the chat itself ([`report_it`]) or
/// the app in its place ([`unreported`]) — and so does purlis's own word that the person
/// stopped the chat (`stopped`, [`crate::stopping`]), which carries no words of the chat's and
/// is marked so nothing a chat reports can pass for it.
///
/// Nothing is typed into the asking chat. While it is open and its program runs, what is
/// delivered is left in the plane for its next `UserPromptSubmit` hook to hand its turn as
/// context (`purlis_core::handback`), and its row says who reported back, or who was stopped.
/// **A chat is reachable when its tab is open AND its program is still running**: one that has
/// ended will never fire the prompt its report waits for. When it has gone, it is kept for the
/// workspace it asked from, and the next chat to start there reads it. No needs-you item is
/// raised here, for a handoff or a task (#1448).
pub(crate) fn deliver(
    held: &Held,
    chat: u32,
    from: &HandedFrom,
    summary: String,
    task: Option<purlis_core::handback::Task>,
    stopped: Option<purlis_core::handback::Stopped>,
) -> Result<Delivered, String> {
    use purlis_core::handback::{self, For, Handback};

    let was_stopped = stopped.is_some();
    let chats = held.chats().open_now();
    let child = chats
        .iter()
        .find(|open| open.session == chat)
        .ok_or_else(|| format!("chat {chat} is not one this app has open"))?;
    let child_name = held
        .chats()
        .shown_name(chat)
        .unwrap_or_else(|| child.name.clone());
    let parent_open = chats.iter().any(|open| open.session == from.chat)
        && !matches!(
            held.board().glance(from.chat).state,
            purlis_core::state::State::Done | purlis_core::state::State::Failed
        );
    let to = if parent_open {
        held.chats()
            .shown_name(from.chat)
            .unwrap_or_else(|| from.name.clone())
    } else {
        from.name.clone()
    };
    let report = Handback {
        // The stop's word is purlis's own line, so the name is written as that line can
        // carry it; a report's name is a chat's, as it stands (D-T59-j11).
        from: if was_stopped {
            handback::in_purlis_s_line(&child_name)
        } else {
            child_name.clone()
        },
        from_workspace: child
            .cwd
            .as_deref()
            .and_then(|cwd| workspace_of(held.root(), cwd))
            .map_or_else(|| from.workspace.clone(), Place::Workspace),
        to: to.clone(),
        to_workspace: from.workspace.clone(),
        summary,
        task,
        answered: None,
        stopped,
    };
    // **A task the person started, whose tab chat is gone, is the person's and nobody
    // else's** (D-1443-9): its report is not handed to whichever chat starts next in that
    // workspace, which never asked for it and would read it as its own business. It stays
    // with the persona chat, and that chat is marked as needing the person.
    if !parent_open && from.by_person {
        return Ok(Delivered {
            to,
            kept_for: Some(handback::FOR_THE_PERSON.to_owned()),
            reached_the_chat: false,
        });
    }
    let whose = if parent_open {
        For::Chat(from.chat)
    } else {
        For::Place(&from.workspace)
    };
    let kept = handback::leave_at(held.root(), whose, &report)
        .map_err(|why| format!("the report could not be kept ({why})"))?;
    // **Kept for a workspace because the chat that asked has gone**: a task's record says so,
    // and the report is handed to that chat if the person reopens it (#1513, V100-64).
    if !parent_open && from.mode == Mode::Task {
        crate::restored::kept_for_its_asker(held, chat, &kept, from.report == Owed::Again);
    }
    // A command waiting on this task has its report now (#1441); the asking chat is told it
    // landed once the lock is let go, by whoever holds it ([`report_it`], and the end of a
    // program). Only a file left for the chat itself is one a
    // wait may take back: one kept for a workspace is the next chat's there.
    if from.mode == Mode::Task {
        if parent_open {
            crate::dispatched::reported(held, chat, from.chat, report.clone(), Some(kept));
        } else {
            // **And it ends at its report, as every other task does** (#1510, V100-64): the
            // report is on its dispatch record, where the person reads it.
            crate::dispatched::reported_with_its_asker_gone(held, chat, from.chat, report.clone());
        }
    }
    if parent_open {
        // **A task that came to nothing is a needs-you item on the chat that asked** (#1491,
        // V100-15): it failed, was blocked, or ended without a report. One that finished as
        // done or cancelled changes that chat's count and no more. **And so does one the
        // person stopped, whatever its one short report says of itself** (#1488): the stop's
        // own line asks it to say what is left undone, so "blocked" is the ordinary answer,
        // and the person who stopped it is not then flagged for a failure.
        // A task's later report (#1768) takes the item its blocked report raised: what it says
        // now is the word on it, and one blocked again raises it afresh.
        if from.report == Owed::Again
            && let Some((id, _)) = crate::finished::task_record(held, chat)
        {
            held.task_failure_cleared(from.chat, &id);
        }
        if !was_stopped
            && let Some(failed) = came_to_nothing(held, chat, report.task.as_ref(), &report.summary)
        {
            held.task_failed(from.chat, failed);
        }
        if was_stopped {
            held.stopped_below(from.chat, &child_name);
        } else {
            held.reported_back(from.chat, &child_name);
        }
    }
    Ok(Delivered {
        to,
        // By `Place::word`: a workspace's name, or the plane root's word (SI-1b).
        kept_for: (!parent_open).then(|| from.workspace.word().to_owned()),
        reached_the_chat: parent_open,
    })
}

/// What the chat that asked is flagged for when task `chat`'s report is `task`, saying
/// `summary`: nothing for a task that finished as done or cancelled, and nothing for a
/// handoff's report, which has no outcome.
///
/// The task is named as its finished row will name it (`crate::finished::task_name`), so the
/// item leads to that row and is cleared with it. Why is the report's own first words for a
/// task that said it failed or was blocked, and nothing for one purlis reported for: "ended
/// without a report" is the whole of what is known.
fn came_to_nothing(
    held: &Held,
    chat: u32,
    task: Option<&purlis_core::handback::Task>,
    summary: &str,
) -> Option<purlis_core::state::FailedTask> {
    use purlis_core::handback::Outcome;
    use purlis_core::state::{FailedTask, HowFailed};

    let task = task?;
    let (how, why) = if task.unreported {
        (HowFailed::Unreported, "")
    } else if task.outcome == Outcome::LastMessage {
        // The person reads what it said: purlis sent it, and the task did not (#1981).
        (HowFailed::LastMessage, "")
    } else if matches!(task.outcome, Outcome::Failed | Outcome::Blocked) {
        (HowFailed::Failed, summary)
    } else {
        return None;
    };
    // Named, and told apart from every other failure, by its dispatch record: its finished
    // row carries the same id. A task with no record is named by its chat.
    let (id, name) = crate::finished::task_record(held, chat).unwrap_or_else(|| {
        (
            format!("chat-{chat}"),
            held.chats()
                .shown_name(chat)
                .unwrap_or_else(|| chat.to_string()),
        )
    });
    let failed = FailedTask::new(&id, &name, how, why);
    // One that reported is still open as a chat, and a blocked one stays so: going to the
    // failure shows that chat. One whose program died has only its finished row.
    Some(if task.unreported {
        failed
    } else {
        failed.of_open_chat(chat)
    })
}

/// The hold [`crate::chats::Chats::deciding`] gives: proof, to the functions below, that the
/// caller is the one deciding. They read and then write the chats' records, and none of them
/// takes the lock itself, so one hold covers a whole close.
pub type Deciding<'a> = std::sync::MutexGuard<'a, ()>;

/// **A persona chat whose program ended without a report is reported for** (#1443): chat
/// `chat` still owed its asking chat a task's report, and its program ended on its own. The
/// app says `failed: ended without a report` in its place, with the path of the session record
/// it wrote for that chat where it wrote one.
///
/// **Once, by nobody's word but the app's, and final** (D-1443-10). Under the caller's hold
/// of the lock a report is taken under, so the program's end, a close of its tab and the
/// chat's own report are one report between them. The record is marked only once the report
/// is kept: one that could not be written is still owed. It goes where that chat's own report
/// would have gone ([`deliver`]).
///
/// **Only for a program that ended on its own.** Not at a quit, not as the project is let go
/// of, not when every agent is stopped and not for a chat started again in its own place:
/// those chats are kept, and report when they run again. And not for a chat the person closed
/// or stopped: that is [`operator_stopped`]'s to say.
pub fn unreported(held: &Held, chat: u32, _deciding: &Deciding<'_>) {
    use purlis_core::handback;

    let Some(from) = held.chats().owed_task_report(chat) else {
        return;
    };
    let record = crate::restored::its_session_record(held, chat)
        .and_then(|path| handback::record_path(&path));
    let text = handback::UNREPORTED;
    // What the app says in a chat's place names the branch it cut for it, as that chat's own
    // report would have (#1453): the work on it is still there to find.
    let task = handback::Task::unreported(record, from.by_person)
        .on_branch(crate::dispatches::branch_of(held, chat));
    match deliver(held, chat, &from, text.to_owned(), Some(task), None) {
        Ok(delivered) => {
            held.chats().owes(chat, Owed::Failed);
            // Its dispatch's record ends here too, in the app's own words (#1452).
            crate::dispatches::reported(
                held,
                chat,
                purlis_core::dispatchrecord::Outcome::Failed,
                text,
                None,
                Some(purlis_core::dispatchrecord::EndedBy::Unreported),
                None,
            );
            tracing::info!(
                "purlis: chat {chat} {text}, so '{}' is told{}",
                delivered.to,
                delivered
                    .kept_for
                    .map(|place| format!(" (kept for {place}: that chat is gone)"))
                    .unwrap_or_default()
            );
        }
        Err(why) => tracing::warn!(
            "purlis: chat {chat} {text}, and its asking chat could not be told yet ({why})"
        ),
    }
}

/// **A report purlis made from its last message** (#1981): task `chat` ended a turn without its
/// report, was asked for it once in purlis's line, and ended the turn that line started without
/// one too. What it last said, `last` (masked and made a report's text already,
/// `handback::last_message_summary`), goes to the chat that asked in its place, under purlis's
/// heading that names it as that and quotes it as data, and the person is shown it on the chat
/// that asked.
///
/// **It settles nothing for good.** The task is left owing one more report (`Owed::Again`,
/// #1768): a report of its own later reaches the chat that asked too, and takes this one's
/// place on its record. Nor is its program ended: a task purlis reported for in its place is
/// not one to end (`dispatched::ends_its_task`).
///
/// Under the lock a report is taken under, so the task's own report and this are one or the
/// other: one that landed first leaves nothing owed here. Not while the person is stopping it
/// or its asking chat is cancelling it: each asks for its report itself.
pub fn made_from_last_message(held: &Held, chat: u32, last: Option<&str>) {
    use purlis_core::handback;

    let deciding = held.chats().deciding();
    let Some(from) = held.chats().owed_task_report(chat) else {
        return;
    };
    if held.stopping().is_stopping(chat) || held.stopping().is_cancelling(chat) {
        return;
    }
    let text = last.map_or_else(|| handback::NOTHING_SAID.to_owned(), str::to_owned);
    let record = crate::restored::its_session_record(held, chat)
        .and_then(|path| handback::record_path(&path));
    let task = handback::Task::from_last_message(
        record,
        from.by_person,
        crate::dispatched::stepped_in(held, chat),
    )
    .on_branch(crate::dispatches::branch_of(held, chat));
    match deliver(held, chat, &from, text.clone(), Some(task), None) {
        Ok(delivered) => {
            held.chats().owes(chat, Owed::Again);
            crate::dispatches::reported(
                held,
                chat,
                purlis_core::dispatchrecord::Outcome::LastMessage,
                &text,
                None,
                None,
                None,
            );
            if delivered.kept_for.is_none() {
                held.board().reported_to_its_asker(chat);
            } else {
                held.needs_the_person(
                    chat,
                    purlis_core::state::Need::ReportUndelivered {
                        asker: delivered.to.clone(),
                    },
                );
            }
            tracing::info!(
                "purlis: chat {chat} ended its turns without a report, so '{}' is sent {}",
                delivered.to,
                handback::LAST_MESSAGE
            );
        }
        Err(why) => tracing::warn!(
            "purlis: chat {chat} ended its turns without a report, and {} could not be sent \
             yet ({why})",
            handback::LAST_MESSAGE
        ),
    }
    // Typed only once the lock is let go, as a chat's own report is (#1441).
    drop(deciding);
    crate::dispatched::told_or_settled(held, from.chat);
}

/// **The operator stopped chat `chat`**: the one word for it, to the chat that asked, and the
/// one place it is written (D-T59-j3). Stop on the chat or on one above it
/// ([`crate::stopping`]), "Stop them" as the person closes the chat that asked, and the Close
/// of a task that had not reported all end here.
///
/// The chat that asked is told when `tell`, in purlis's own marked word
/// (`purlis_core::handback::Stopped`), which says whether the chat `wrote` a last report,
/// whose task it was and where its session record is. It travels the road a report does
/// ([`deliver`]) and is none: nothing in it is a chat's words. **A task that still owed its
/// report is settled by it**, for good, as one that ended on its own is ([`unreported`]): the
/// close that follows says nothing a second time, and its dispatch's record ends as stopped.
/// Not told (`tell` is false: the chat that asked is ending in the same stop), it is settled
/// all the same, so no word goes on to a workspace for nobody.
///
/// **A task is told of in one of two words** (#1488, V100-6). One that sent the report its
/// stop took (`wrote`) was told of as it reported, as **stopped by the person**, in a word
/// that carries that report ([`report_under`]): nothing is left to say here. One that sent
/// none is **closed by the person**, and that word is written here: Close now, the tab's
/// Close on a task that had not reported, and a stop that got no report in the time it had.
/// `below` names the tasks ended with it, below it. **A task whose report was settled before
/// the person ended it is told of no more**: it had ended by itself, and the chat that asked
/// already knows how.
///
/// Under the caller's hold of the lock a report is taken under.
pub(crate) fn operator_stopped(
    held: &Held,
    chat: u32,
    wrote: bool,
    tell: bool,
    below: Vec<String>,
    _deciding: &Deciding<'_>,
) {
    use purlis_core::handback;

    let Some(from) = held.chats().handed_from(chat) else {
        return;
    };
    let task = from.mode == Mode::Task;
    // Settled already: it reported, by itself or as its stop's one report, or purlis said in
    // its place that it went without one.
    if task && from.report != Owed::Due {
        return;
    }
    if tell {
        let stopped = handback::Stopped {
            wrote: wrote && !task,
            task,
            by_person: task && from.by_person,
            record: crate::restored::its_session_record(held, chat)
                .and_then(|path| handback::record_path(&path)),
            below: if task { below } else { Vec::new() },
            limit: held.stopping().limit_of(chat).filter(|_| task),
            // Where the stopped task's work is, by the app's record of what it cut (#1472).
            branch: crate::dispatches::branch_of(held, chat).filter(|_| task),
        };
        if let Err(why) = deliver(held, chat, &from, String::new(), None, Some(stopped)) {
            // Still owed where it was: the chat's close tries once more.
            tracing::warn!(
                "purlis: chat {chat} was stopped, and the chat that asked could not be told \
                 yet ({why})"
            );
            return;
        }
    }
    if task && from.report == Owed::Due {
        held.chats().owes(chat, Owed::Failed);
        // Its dispatch's record ends here too, in the app's own words and under a word of
        // its own (#1452, D-T59-j10): the person closed it, or purlis did at a limit the
        // person set (#1512), and it did not fail by itself.
        let (by, text) = match held.stopping().limit_of(chat) {
            Some(_) => (
                purlis_core::dispatchrecord::EndedBy::Limit,
                handback::CLOSED_AT_A_LIMIT,
            ),
            None => (
                purlis_core::dispatchrecord::EndedBy::Person,
                handback::CLOSED,
            ),
        };
        crate::dispatches::reported(
            held,
            chat,
            purlis_core::dispatchrecord::Outcome::Stopped,
            text,
            None,
            Some(by),
            Some(purlis_core::dispatchrecord::EndedWay::Closed),
        );
    }
    // Stopped, it owes nothing more: the chat that asked and every chat above it may have
    // held an end of turn for it (#1491). Read only: this is under the deciding lock.
    crate::dispatched::settle_above(held, from.chat);
}

/// **A persona chat's program ended on its own** (#1443): [`unreported`], called as the
/// operating system says the process is gone. That is
/// the bound on how long an asking chat can wait on a task that died.
///
/// It waits for the deciding lock only while the chat still owes a report. A close holds that
/// lock while it ends the chat's program, and has settled what the chat owed before it does;
/// so the end of a program that a close is ending finds nothing owed and returns at once,
/// and never waits on the close that is waiting on it.
pub fn its_program_ended(held: &Held, chat: u32) {
    // **A task being cancelled is reported for by its cancel** (#1459): its program's end ends
    // the cancel's turn, which writes its report as `cancelled` (`stopping::exited`).
    if held.stopping().is_cancelling(chat) {
        return;
    }
    loop {
        if held.chats().owed_task_report(chat).is_none() {
            return;
        }
        if let Some(deciding) = held.chats().try_deciding() {
            let asker = held.chats().owed_task_report(chat).map(|from| from.chat);
            unreported(held, chat, &deciding);
            // Typed only once the lock is let go, as a chat's own report is (#1441).
            drop(deciding);
            if let Some(asker) = asker {
                crate::dispatched::told_or_settled(held, asker);
            }
            // Its asking chat is told it failed, and its row is a finished one from here
            // (#1485): the chat whose program is gone is closed, off this thread.
            crate::dispatched::delivered(held, chat);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

// ----------------------------------------------------------------------------------------
// a chat's persona chats: where each stands, and what closing their asking chat does (#1443)
// ----------------------------------------------------------------------------------------

/// Where a persona chat stands (`purlis_core::dispatchdecision::Standing`), as the window and
/// a chat's own list of its dispatches are told it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PersonaChatState {
    /// At work, and its report is still to come.
    Running,
    /// Stopped on something only you can answer, in its own tab: a permission prompt or a
    /// question.
    WaitingOnOperator,
    /// It sent its report. Its program is ended once the turn that sent it is over (#1485),
    /// and it is listed from then as a finished task.
    Reported,
    /// Its program ended before it reported, and the chat that asked was told it failed.
    Ended,
}

impl From<dispatchdecision::Standing> for PersonaChatState {
    fn from(standing: dispatchdecision::Standing) -> Self {
        use dispatchdecision::Standing;
        match standing {
            Standing::Running => Self::Running,
            Standing::WaitingOnOperator => Self::WaitingOnOperator,
            Standing::Reported => Self::Reported,
            Standing::Ended => Self::Ended,
        }
    }
}

/// One persona chat a chat dispatched as a task, as that chat sees it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PersonaChat {
    pub session: u32,
    /// The name the person sees it under: its task's name.
    pub name: String,
    /// The persona it runs as.
    pub persona: Option<String>,
    pub state: PersonaChatState,
    /// [`PersonaChatState`] in the words a chat's list of its dispatches says: `running`,
    /// `waiting on the person`, `reported`, `ended without a report`.
    pub said: String,
    /// Whether closing the chat that asked for it closes it too: it has reported, it is not
    /// at work, and its session record is written or can be asked for ([`close_reported`]).
    pub closes_with_its_asker: bool,
}

/// Why chat `asker`'s dispatch named `name` is refused as a task it already has open (#3788,
/// [`dispatchdecision::same_name_open`]), from this app's record of its tasks; or `None`.
///
/// Each is named with its liveness (#3900): a task that came back after a restart is named
/// live, never left to be taken for a dead one.
fn same_name_open(held: &Held, asker: u32, name: &str) -> Option<String> {
    type Listed = (
        u32,
        String,
        dispatchdecision::Standing,
        purlis_core::dispatchliveness::Liveness,
    );
    let tasks: Vec<Listed> = held
        .chats()
        .tasks_of(asker)
        .into_iter()
        .filter_map(|(session, from)| {
            let named = held.chats().shown_name(session)?;
            Some((
                session,
                named,
                standing_of(held, session, &from),
                crate::dispatched::liveness(held, session),
            ))
        })
        .collect();
    dispatchdecision::same_name_open(
        name,
        tasks.iter().map(|(session, named, standing, live)| {
            (*session, named.as_str(), *standing, Some(live))
        }),
    )
}

/// Where chat `chat`, a persona chat with lineage record `from`, stands: read from the app's
/// records of it and nothing it says.
///
/// **Waiting on the operator is the board's own word that it asks mid-turn** (a permission
/// prompt, a question), or a permission ask this app holds open for it. Its asking chat cannot
/// answer either, and reads this to know the wait is not its own to end.
fn standing_of(held: &Held, chat: u32, from: &HandedFrom) -> dispatchdecision::Standing {
    dispatchdecision::standing(from.report, !still_working(held, chat), asks(held, chat))
}

/// Whether chat `chat` is asking the person something mid-turn, or holds a permission ask
/// open: it cannot go on until they answer in its own tab.
fn asks(held: &Held, chat: u32) -> bool {
    held.board().glance(chat).asking || held.asks().asks.iter().any(|ask| ask.session == chat)
}

/// **The persona chats chat `asker` dispatched as tasks, each with where it stands** (#1443):
/// what that chat's own list of its dispatches reads, and what closing it says.
///
/// `asker` is the app's number for a chat it has open. The list is of the chats still open: a
/// persona chat that was closed is not one anybody waits on. A task the person started from
/// that chat's tab is listed too, by name and state, and that is all the list gives a chat of
/// it: who may steer a listed chat is not this function's to say.
pub fn persona_chats(held: &Held, asker: u32) -> Vec<PersonaChat> {
    held.chats()
        .tasks_of(asker)
        .into_iter()
        .map(|(session, from)| {
            let standing = standing_of(held, session, &from);
            PersonaChat {
                session,
                name: held
                    .chats()
                    .shown_name(session)
                    .unwrap_or_else(|| session.to_string()),
                persona: held
                    .chats()
                    .recorded_chat(session)
                    .and_then(|chat| chat.persona),
                state: standing.into(),
                said: standing.word().to_owned(),
                closes_with_its_asker: closes_with(held, session, &from) != ClosesWith::Stays,
            }
        })
        .collect()
}

/// Whether chat `chat`, started by another, is **at work**: what "Stop them" stops, and what
/// closing a chat above it asks about. Its program runs, and either it is a task that has not
/// reported, or it is mid-turn or asking the person something. A chat that reported and sits
/// idle, and a handoff's chat waiting for its next prompt, are not at work: each is a tab the
/// person can see, with nothing running unseen in it.
fn at_work(held: &Held, chat: u32, from: &HandedFrom) -> bool {
    if !still_working(held, chat) {
        return false;
    }
    let owes_a_report = from.mode == Mode::Task && from.report == Owed::Due;
    owes_a_report
        || held.board().glance(chat).state == purlis_core::state::State::Running
        || asks(held, chat)
}

/// Every chat below chat `asker` that is at work ([`at_work`]), deepest first: the tasks it
/// asked for, and the tasks they asked for, through every task on the way whether or not that
/// one is still at work. A chat that reported may have dispatched before it did, and what it
/// started is still below `asker`.
///
/// **Task links only** (#1492, V100-69), as [`crate::stopping`]'s lineage and the window's
/// tree are: a handoff moved the work to a session of its own, which is below no chat. So
/// closing `asker` does not ask about it, "Stop them" does not stop it, and a task that
/// handed off is not held from ending by it.
pub(crate) fn at_work_below(held: &Held, asker: u32) -> Vec<u32> {
    let mut found = Vec::new();
    tasks_below(held, asker, &|_| true, &mut |_, chat, from| {
        if at_work(held, chat, from) {
            found.push(chat);
        }
    });
    found
}

/// **The one walk down task links** (#1491, #1492): every task below chat `top`, at any
/// depth, handed to `each` deepest first with the chat that asked for it. What closing a chat
/// asks about ([`at_work_below`]) and what a chat's end of turn waits on ([`owing_below`])
/// are both this walk, so they cannot come to mean different chats by "below".
///
/// - **Task links only** ([`crate::chats::Chats::tasks_of`]): a handoff's chat is below no
///   chat (V100-69).
/// - **Through every task on the way**, whatever it is doing: a task that reported may have
///   dispatched before it did, and what it started is still below `top`.
/// - `enters` says whether a task and everything below it are walked at all.
/// - Bounded by the ceiling no chain passes, and each chat once: a record that names a loop
///   ends.
fn tasks_below(
    held: &Held,
    top: u32,
    enters: &dyn Fn(&purlis_core::reopen::HandedFrom) -> bool,
    each: &mut dyn FnMut(u32, u32, &purlis_core::reopen::HandedFrom),
) {
    fn below(
        held: &Held,
        asker: u32,
        deeper: u32,
        seen: &mut Vec<u32>,
        enters: &dyn Fn(&purlis_core::reopen::HandedFrom) -> bool,
        each: &mut dyn FnMut(u32, u32, &purlis_core::reopen::HandedFrom),
    ) {
        if deeper == 0 {
            return;
        }
        for (chat, from) in held.chats().tasks_of(asker) {
            if seen.contains(&chat) || !enters(&from) {
                continue;
            }
            seen.push(chat);
            below(held, chat, deeper - 1, seen, enters, each);
            each(asker, chat, &from);
        }
    }
    below(
        held,
        top,
        dispatchdecision::DEEPEST,
        &mut vec![top],
        enters,
        each,
    );
}

/// **The tasks below chat `asker`, at any depth, that still owe their report** (#1491):
/// working, asking, or idle with the report still to come, and their program running. What
/// `asker` waits on when its turn ends, and why that end is no needs-you item
/// ([`crate::dispatched::waits`]).
///
/// **Down task links only**, through every task on the way whether or not that one has
/// reported: a task that reported may have dispatched before it did, and what it started is
/// still below `asker`. A handoff moved the work to a session of its own (V100-69), so what is
/// below a handoff's chat is that chat's and not `asker`'s. A task whose program has ended
/// owes nothing more: purlis reports for it.
///
/// **Not a task the person asked for** from `asker`'s tab, nor anything below one (M4):
/// `asker` asked for nothing there, and a long task of the person's would otherwise hide every
/// end of `asker`'s own turns. It is still counted on the row. **And not a task of `asker`'s
/// own that has a question open with `asker`** (M3): that task waits on `asker`, so `asker`
/// stopping without answering is the person's to see, and neither waits on the other.
pub fn owing_below(held: &Held, asker: u32) -> Vec<u32> {
    let mut found = Vec::new();
    tasks_below(
        held,
        asker,
        // A task the person asked for from this chat's tab is theirs: the chat did not ask
        // for it and waits on nothing of it or below it (M4).
        &|from| !from.by_person,
        &mut |asked_by, chat, from| {
            // A task of the chat's own that has a question open with it is waiting on the
            // chat, not the other way round (M3): the next move is the chat's.
            let asks_it = asked_by == asker && crate::dispatched::asks_its_asker(held, chat);
            if from.report == Owed::Due && still_working(held, chat) && !asks_it {
                found.push(chat);
            }
        },
    );
    found
}

/// The chats at work below chat `asker`, by the names the person sees them under, deepest
/// first: what closing `asker` asks about, keep them running or stop them.
pub fn running_below(held: &Held, asker: u32) -> Vec<String> {
    at_work_below(held, asker)
        .into_iter()
        .map(|chat| {
            held.chats()
                .shown_name(chat)
                .unwrap_or_else(|| chat.to_string())
        })
        .collect()
}

/// **Stops every chat at work below chat `asker`** (#1443): the person's "Stop them", asked
/// once as they close the asking chat.
///
/// **By the stop every chat is stopped by** (`crate::stopping::press_below`, D-T59-j3), and
/// not by a second mechanism: a chat another chat started gets its one short turn, the chat
/// that asked for it is told the operator stopped it, in the one word for that, and from the
/// moment the stop is recorded none of them starts a chat. Under the caller's hold of the
/// deciding lock, which is also what a dispatch is decided under: no chat below can start
/// another between the answer and the stop.
pub fn stop_below(held: &Held, asker: u32, deciding: &Deciding<'_>) {
    crate::stopping::press_below(held, &at_work_below(held, asker), deciding);
}

/// What closing its asking chat does with a persona chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosesWith {
    /// It stays open: it has not reported, it is at work or asking the person something, or
    /// its session record is gone.
    Stays,
    /// It closes now, on the session record the app wrote for it.
    OnItsRecord(crate::smartclose::SavedRecord),
    /// It has no session record yet: it is asked for one, and closes when that is saved.
    OnceRecorded,
}

/// What closing its asking chat does with chat `chat`, a persona chat with lineage `from`.
///
/// **Only a chat that reported and is at rest closes with its asker.** One that reported and
/// was then given more to do (the person typed in it, a follow-up arrived) is working, and
/// its work is not ended because the chat that once asked it something has closed. And **one
/// whose saved record is gone is not closed**: the record is what a close leaves of it.
fn closes_with(held: &Held, chat: u32, from: &HandedFrom) -> ClosesWith {
    use purlis_core::state::State;

    if from.mode != Mode::Task || !(from.report.reported() || from.report == Owed::Failed) {
        return ClosesWith::Stays;
    }
    let glance = held.board().glance(chat);
    // Waiting on work of its own is at work through it (#3878).
    if glance.state == State::Running || glance.own_work || asks(held, chat) {
        return ClosesWith::Stays;
    }
    match held.chats().last_record(chat) {
        Some(path) => {
            match purlis_core::sessionrecord::saved(held.root(), &held.root().join(&path)) {
                Some(listed) => ClosesWith::OnItsRecord(crate::smartclose::SavedRecord {
                    path: listed.shown,
                    title: listed.title,
                }),
                None => ClosesWith::Stays,
            }
        }
        // Asked for its record, which only a chat whose program still runs can write.
        None if matches!(glance.state, State::Done | State::Failed) => ClosesWith::Stays,
        None => ClosesWith::OnceRecorded,
    }
}

/// The persona chats chat `asker` dispatched that close with it, each with how
/// ([`closes_with`]): read before `asker` is closed, and carried out after ([`close_reported`]).
pub fn reported_by(held: &Held, asker: u32) -> Vec<(u32, ClosesWith)> {
    held.chats()
        .tasks_of(asker)
        .into_iter()
        .map(|(chat, from)| (chat, closes_with(held, chat, &from)))
        .filter(|(_, how)| *how != ClosesWith::Stays)
        .collect()
}

/// **Closes the persona chats in `reported`, whose asking chat has just closed, each once its
/// session record is written** (#1443).
///
/// One whose record the app has written closes now, and the window is told as it is told of a
/// Smart close that ended on its record. One with no record yet is smart-closed: it is asked
/// to write its record, and closes when that is saved. **None is closed without its record.**
pub fn close_reported(held: &Arc<Held>, reported: Vec<(u32, ClosesWith)>, deciding: &Deciding<'_>) {
    for (chat, how) in reported {
        match how {
            ClosesWith::Stays => {}
            ClosesWith::OnItsRecord(record) => {
                if let Err(why) = held.close_chat_held(chat, deciding) {
                    tracing::warn!(
                        "purlis: chat {chat}, closed with its asking chat, did not end cleanly \
                         ({why})"
                    );
                }
                held.tell_smart_closed(chat, Some(record));
            }
            ClosesWith::OnceRecorded => match crate::smartclose::begin(held, chat) {
                Ok(_) => tracing::info!(
                    "purlis: chat {chat} reported and its asking chat closed, so it is asked \
                     for its session record and closes on it"
                ),
                Err(why) => tracing::info!(
                    "purlis: chat {chat} reported and its asking chat closed; it stays open \
                     with no session record ({why})"
                ),
            },
        }
    }
}

/// The workspace a chat standing in `cwd` works in: the directory under the plane's
/// `workspaces/` it is in, where it is in one. The ladder's cwd rung, so this and `charter`
/// cannot answer one directory two ways (SI-1).
pub(crate) fn workspace_of(root: &std::path::Path, cwd: &std::path::Path) -> Option<String> {
    purlis_core::active::workspace_of_tree(root, cwd)
}

fn no(why: String) -> Answer {
    Answer::No { why }
}

/// **Where the open chat `chat` works**, by this app's record of it (#2536): `None` for a chat
/// it does not have open.
fn chat_at(held: &Held, chat: u32) -> Option<crate::specimen::ChatAt> {
    if !is_open(held, chat) {
        return None;
    }
    let record = held
        .chats()
        .drafting()
        .into_iter()
        .find(|one| one.session == chat);
    Some(crate::specimen::ChatAt {
        plane: held.root().to_path_buf(),
        workspace: record.as_ref().and_then(|one| one.workspace.clone()),
        id: record.map(|one| one.id),
    })
}

fn is_open(held: &Held, chat: u32) -> bool {
    held.chats()
        .open_now()
        .iter()
        .any(|open| open.session == chat)
}

/// The size a handed-off chat starts at, the same one a relaunch uses: it has no pane yet to
/// ask, and the pane it lands in tells it the real one when it is first shown.
const STARTING: Size = Size {
    columns: 80,
    rows: 24,
};

/// What a handoff's request says, read and held to its rules before anything is decided.
struct Moving {
    /// The folder the new chat stands in: the workspace's, which exists or is about to.
    dir: std::path::PathBuf,
    /// Where the stamp says the handoff left from, which the new chat's record keeps.
    left_from: Place,
}

/// What the handoff `moved` asks of the project at `root`, for the chat `from`, or why it
/// cannot be opened in a sentence the asker prints.
///
/// The order is `charter/commands_handoff.py`'s after its frame check, and every question
/// here is asked before anything is decided or written: the stamp, the place it left from,
/// the workspace it names. **Nothing is created here**: a handoff that waits on the person
/// creates its workspace when they allow it, and never before ([`dispatch_it`]).
fn moving(root: &std::path::Path, from: u32, moved: &Moved) -> Result<Moving, String> {
    use purlis_core::{handoff, wscmd};

    let Some(stamp) = handoff::stamped(&moved.message).filter(|read| read.chat == from.to_string())
    else {
        return Err(format!(
            "the first message does not open with the stamp of a handoff from chat {from}, \
             and a chat the app opens on request always says where it came from"
        ));
    };
    // A workspace's name, or the plane root (SI-1b): a chat at the root is in no workspace,
    // and its stamp says so rather than naming the one the ladder would have picked.
    let Some(left_from) = stamp.place() else {
        return Err(format!(
            "the stamp names '{}' as the workspace the handoff left from, which cannot be one",
            purlis_core::shown::short(stamp.workspace)
        ));
    };
    let ws = moved.workspace.as_str();
    let dir = wscmd::workspace_dir(root, ws).ok_or_else(|| {
        format!(
            "'{}' cannot name a workspace",
            purlis_core::shown::short(ws)
        )
    })?;
    match moved.create_vision {
        Some(_) if wscmd::workspace_dir_exists(root, ws) => {
            return Err(format!("workspace '{ws}' already exists"));
        }
        None if !dir.is_dir() => {
            return Err(format!(
                "workspace '{ws}' has no directory to open a chat in"
            ));
        }
        _ => {}
    }
    Ok(Moving { dir, left_from })
}

/// Creates the workspace a handoff with `--create` names, with its vision: `create`'s own two
/// calls, in its order. Only once the handoff is allowed to start.
fn create_it(root: &std::path::Path, ws: &str, vision: &str) -> Result<(), String> {
    use purlis_core::wscmd;

    if wscmd::workspace_dir_exists(root, ws) {
        return Err(format!("workspace '{ws}' already exists"));
    }
    wscmd::ensure::ensure(root, ws, chrono::Utc::now(), &wscmd::ensure::author())?;
    // The vision is written into the workspace `ensure` just scaffolded.
    let plane_on_disk = purlis_core::workspaces::Plane::open(root);
    if let Ok(workspace) = plane_on_disk.workspace(ws) {
        let _ = workspace.set_vision(vision);
    }
    Ok(())
}

/// What a chat is opened on and filed as, whichever mode opened it.
struct Opening<'a> {
    /// Its first message, as it is sent: the stamp, then the brief.
    message: &'a str,
    /// The brief alone, as the asking chat wrote it: what the dispatch's record keeps.
    brief: &'a str,
    /// The task's name, where it was given one.
    label: Option<String>,
    /// Its lineage, which rides its record.
    from: HandedFrom,
    /// [`Arrived::workspace`].
    workspace: Option<String>,
    /// The number it starts under, where one was dealt already: a task's, dealt as its slot
    /// was reserved. `None` deals one now.
    number: Option<u32>,
    /// Whether the person asked for it, from the asking chat's tab (#1438): what its dispatch
    /// record says of who asked.
    by_person: bool,
    /// The dispatch record's id, where it was minted before the chat started: a worktree
    /// task's folder and branch are named for it (#1453). `None` mints one as the record opens.
    id: Option<String>,
    /// The worktree the app cut for it, where the dispatch gave it one.
    worktree: Option<purlis_core::dispatchrecord::Worktree>,
    /// The level its harness starts at (#3787, [`purlis_core::reopen::Chat::task_level`]).
    task_level: purlis_core::dispatchmode::Level,
}

/// The project's profiles as one read of them: what a profile is chosen from and the chat is
/// then started from ([`purlis_core::start::ready_read`]).
type LaunchRead<'a> = (
    &'a purlis_core::harness_declaration::Declarations,
    &'a (
        purlis_core::profiles::ProfileSet,
        purlis_core::profiles::IgnoreCheck,
    ),
);

/// Starts a chat on `opening`'s first message and records it, for a handoff and a task alike:
/// a name no chat here has had, the start `start_of` makes under that name, the harness's own
/// way of taking a first message ([`told_first`]), and the record that keeps the lineage.
///
/// **The record keeps the profile's own words and not the brief**: a relaunch resumes the
/// conversation, and sending the brief a second time would be a message nobody sent twice.
fn start_on(
    held: &Held,
    plane: &PlaneId,
    start_of: impl FnOnce(String) -> Result<purlis_core::start::Start, String>,
    opening: &Opening<'_>,
    (declared, launch): LaunchRead<'_>,
    size: Size,
) -> Result<Arrived, String> {
    use purlis_core::start;

    // Its own name is a number no chat in this plane has had, dealt now so the chat can be
    // started under it: with no task name its tab says `<persona> <N>`, the ordinary default,
    // and four handoffs from one chat are four different tabs (charter-app#258).
    let number = opening
        .number
        .unwrap_or_else(|| held.chats().sessions().deal());
    let name = number.to_string();
    let start = start_of(name.clone())?;
    let ready = start::ready_read(&start, held.root(), declared, launch)
        .map_err(|why| crate::asking::start_refused(held, why))
        .and_then(|ready| {
            told_first(
                ready,
                start.profile.as_deref().unwrap_or_default(),
                opening.message,
            )
        })?;
    let chat = Chat {
        program: ready.program.clone(),
        args: Vec::new(),
        cwd: ready.cwd.clone(),
        name: name.clone(),
        resume: ready.session.clone(),
        active: false,
        profile: start.profile.clone(),
        persona: start.persona.clone(),
        show_footer: false,
        pinned: false,
        number: Some(number),
        label: opening.label.clone(),
        from: Some(opening.from.clone()),
        held: start.held.clone(),
        renamed_from: None,
        task_level: opening.task_level,
        ..Default::default()
    };
    // **A dispatch decided and reserved before a stop was recorded does not start**
    // (D-T59-j4, amending D-1443-14): the stop and the decision took the deciding lock one
    // after the other, and the one that came second is asked here, before any program runs.
    let own = !opening.by_person;
    if own && crate::stopping::refuses_a_start(held, opening.from.chat) {
        return Err(crate::stopping::STARTS_NOTHING.to_owned());
    }
    let session = held.chats().start_ready(&chat, &ready, size)?;
    // **The stop may have been pressed while this chat was starting** (#1448). The press reads
    // the lineage and records the stop in one hold, and this asks in the same hold: either the
    // press saw this chat and stops it with the rest, or this sees the stop and ends the chat
    // here, before anything is told of it.
    if own && crate::stopping::sweeps(held, session, opening.from.chat) {
        return Err(crate::stopping::STARTS_NOTHING.to_owned());
    }
    // And the dispatch's own record, in the app's state (#1452): who asked and where from are
    // this app's record of the asking chat, never the stamp or the request.
    record_it(held, session, opening, &chat, &ready);
    Ok(Arrived {
        plane: plane.clone(),
        session,
        name,
        label: opening.label.clone(),
        from: chat
            .from
            .as_ref()
            .map(|from| crate::HandedFromNote::of(from, chat.has_tab())),
        workspace: opening.workspace.clone(),
        persona: start.persona,
        harness: ready.harness.map(|harness| harness.name().to_owned()),
    })
}

/// What became of a dispatch the app was asked for.
enum Dispatched {
    /// The persona chat is running, with what the asking chat is told about how it was
    /// started and where it works ([`Answer::Dispatched`]'s `note` and `works`). Boxed: it is
    /// the whole arrival.
    Started {
        it: Box<Arrived>,
        note: Option<String>,
        works: Option<String>,
    },
    /// Nothing has started yet: the person is being asked for a dispatch grant for this pair,
    /// on the asking chat's tab, and the dispatch is held for their answer
    /// ([`Answer::NeedsGrant`]).
    Held {
        from: Option<String>,
        to: String,
        waiting: Option<String>,
        /// The grants store's number for the held dispatch.
        pending: u32,
    },
    /// Nothing has started yet: every check let it through and its grant stands, and this
    /// machine is short on memory (#1467, [`dispatchdecision::waits`]). The caller holds it
    /// ([`HeldDispatches::wait_on_memory`]) and the asking chat is told it waits
    /// ([`Answer::WaitingOnMemory`]).
    WaitsOnMemory {
        /// The persona it is for, none for the asking chat's own.
        to: Option<String>,
    },
}

/// What a handoff's request says beyond a task's ([`OpenChat`]): where the work moves to, and
/// the message the command stamped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    /// The workspace the work moves to, where the new chat stands for life.
    pub workspace: String,
    /// The vision of that workspace, where this handoff creates it (`--create --vision`).
    pub create_vision: Option<String>,
    /// The whole first message as the command stamped it: the stamp line, a blank line, the
    /// brief. The app checks the stamp and writes the asking chat's shown name into it.
    pub message: String,
}

/// A dispatch the app is asked for, a task ([`DispatchAsk`]) or a handoff ([`OpenChat`]): what
/// the request says, and who says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// The asking chat: the chat whose token the line carried, or whose tab the person is at.
    pub chat: u32,
    /// The persona named, or none for the asking chat's own.
    pub to: Option<String>,
    /// The task's name. A handoff may give none, which is the empty string here.
    pub name: String,
    /// The brief, as it was written: what the Notice for a first grant shows in full, and
    /// what the dispatch's record keeps.
    pub brief: String,
    /// The profile asked for, or none.
    pub profile: Option<String>,
    /// Where the new chat is to work, as it was asked (`--in`, #1453): `worktree`, or
    /// `workspace:<name>`, or none for the persona's own default, else the asking chat's
    /// folder. A word [`purlis_core::dispatchplace::asked`] reads, never a folder or a branch.
    ///
    /// **The seam for "Ask <persona>…"** (#1438): the dialog on a chat's tab offers the same
    /// three places and passes its choice here, with `by` set to the person.
    pub place: Option<String>,
    /// Whether the chat asks, or the person does from its tab (#1438). The limits hold either
    /// way; only a chat's ask needs a grant.
    pub by: dispatchdecision::By,
    /// What a handoff adds, or none for a task: the mode is which of the two this is.
    pub moved: Option<Moved>,
}

impl Wanted {
    /// What a chat's own ask wants.
    fn of(ask: &DispatchAsk) -> Self {
        Self {
            chat: ask.chat,
            to: ask.to.clone(),
            name: ask.name.clone(),
            brief: ask.brief.clone(),
            profile: ask.profile.clone(),
            place: ask.place.clone(),
            by: dispatchdecision::By::Chat,
            moved: None,
        }
    }

    /// What a chat's handoff wants (#1444): the same dispatch, in handoff mode. The persona is
    /// the one it names, or the asking chat's own, as for a task. The brief is read off the
    /// stamped message; one that is not stamped is refused where it is read ([`moving`]).
    fn moved(open: &OpenChat) -> Self {
        Self {
            chat: open.chat,
            to: open.persona.clone(),
            name: open.name.clone().unwrap_or_default(),
            brief: purlis_core::handoff::stamped(&open.message)
                .map_or(open.message.as_str(), |read| read.brief)
                .to_owned(),
            profile: None,
            // Where a handoff's chat works is the workspace it names ([`Moved`]), never `--in`.
            place: None,
            by: dispatchdecision::By::Chat,
            moved: Some(Moved {
                workspace: open.workspace.clone(),
                create_vision: open.create_vision.clone(),
                message: open.message.clone(),
            }),
        }
    }

    /// Which mode this dispatch is.
    fn mode(&self) -> Mode {
        match self.moved {
            Some(_) => Mode::Handoff,
            None => Mode::Task,
        }
    }

    /// What this dispatch is called where purlis names it to its asking chat: the task's
    /// name, or for a handoff that gave none, where the work moved to. `None` for a name
    /// purlis would not draw.
    pub(crate) fn shown(&self) -> Option<String> {
        match &self.moved {
            Some(moved) if self.name.trim().is_empty() => Some(
                purlis_core::handoff::task_name_of_a_handoff(&moved.workspace),
            ),
            _ => dispatchdecision::task_name(&self.name).ok(),
        }
    }
}

/// **The dispatches waiting on the person, as they were asked** (#1437): the grants store
/// holds each as the Notice shows it, with its brief escaped and cut, and that is not what a
/// chat is started on. This keeps the ask itself, by the store's number for it, so an Allow
/// starts exactly the dispatch whose brief the person read.
///
/// In memory only, as the store's own list is: both end with the app.
///
/// **And what the Dispatches tab lists of them** (#1456): each one held, from when it was
/// held, and the last [`KEPT_BLOCKED_LISTED`] the person kept blocked. Neither ever had a
/// dispatch record, and nothing of them is written: they end with the app too.
pub struct HeldDispatches {
    held: std::sync::Mutex<std::collections::HashMap<u32, Wanted>>,
    /// When each held dispatch was held, by the store's number.
    since: std::sync::Mutex<std::collections::HashMap<u32, String>>,
    /// The dispatches the person kept blocked, oldest first, with when.
    kept_blocked: std::sync::Mutex<std::collections::VecDeque<(Wanted, String)>>,
    /// **The dispatches waiting on this machine's memory** (#1467), oldest first. Every check
    /// let each through and its grant stands; [`memory_freed`] decides each again once memory
    /// frees. In memory only, as the rest: a launch starts with none.
    on_memory: std::sync::Mutex<Vec<OnMemory>>,
    /// The numbers [`Self::on_memory`] gives, so a dispatch put back keeps its own.
    on_memory_dealt: std::sync::atomic::AtomicU64,
    /// The dispatches that waited [`dispatchdecision::MEMORY_WAIT`] on memory and started
    /// nothing, oldest first, with when: listed beside the ones the person kept blocked.
    gave_up: std::sync::Mutex<std::collections::VecDeque<(Wanted, String)>>,
    /// This project's reader of the machine's memory, with the seam a test sets
    /// ([`purlis_core::memorypressure::Gauge::stand_in`]).
    memory: purlis_core::memorypressure::Gauge,
}

impl Default for HeldDispatches {
    fn default() -> Self {
        let held = Self {
            held: std::sync::Mutex::default(),
            since: std::sync::Mutex::default(),
            kept_blocked: std::sync::Mutex::default(),
            on_memory: std::sync::Mutex::default(),
            on_memory_dealt: std::sync::atomic::AtomicU64::default(),
            gave_up: std::sync::Mutex::default(),
            memory: purlis_core::memorypressure::Gauge::default(),
        };
        // **A test's project reads no machine** (#1467): memory is enough until the test says
        // otherwise, so no test's dispatch waits on whatever the machine running it is short
        // of, and the project's own timer leaves the looking to the test.
        #[cfg(test)]
        held.memory
            .stand_in(Some(purlis_core::memorypressure::Memory::Enough));
        held
    }
}

/// A dispatch waiting on this machine's memory (#1467).
#[derive(Clone)]
struct OnMemory {
    /// Its number among the ones waiting.
    id: u64,
    wanted: Wanted,
    /// When it began to wait, which the bound is counted from: kept when it is put back.
    since: Instant,
    /// The same, as the Dispatches tab says it.
    at: String,
}

/// How often the dispatches waiting on memory are looked at again ([`memory_freed`]): soon
/// enough that one starts within seconds of memory freeing, and only a read of the machine's
/// memory while any waits.
pub const MEMORY_LOOKED_AT_EVERY: std::time::Duration = std::time::Duration::from_secs(5);

/// **The most dispatches one chat may have waiting on memory at once** (#1467): none of them
/// holds a slot of its limits while it waits, so a chat that asks in a loop while memory is
/// short is refused, and told so, before it fills the list. One chat's running limit by
/// default.
pub const MOST_ON_MEMORY_PER_CHAT: usize = 6;

/// What a chat is told where it already has [`MOST_ON_MEMORY_PER_CHAT`] dispatches waiting on
/// memory.
fn too_many_on_memory() -> String {
    format!(
        "this machine is short on memory, and this chat already has {MOST_ON_MEMORY_PER_CHAT} \
         dispatches waiting for it to free, so nothing was started. Wait until they start, then \
         dispatch again"
    )
}

/// **Holds `wanted` until memory frees** ([`HeldDispatches::wait_on_memory`]), and tells the
/// window the asking chat's row moved: it says how many of its dispatches wait (#1617).
fn waits_on_memory(held: &Held, wanted: &Wanted) -> Result<(), String> {
    held.held_dispatches().wait_on_memory(wanted)?;
    held.rows_changed();
    Ok(())
}

/// What the asking chat is told beside [`purlis_core::handback::Answered::WaitingOnMemory`]
/// (#1617): how long it may wait, and what then.
fn still_short_after_the_wait() -> String {
    format!(
        "if this machine is still short on memory after {} minutes, nothing starts and this \
         chat is told",
        dispatchdecision::MEMORY_WAIT_MINUTES
    )
}

/// How many dispatches the person kept blocked the Dispatches tab lists, newest kept.
pub const KEPT_BLOCKED_LISTED: usize = 50;

/// A time as the dispatch records write one: RFC 3339, in UTC, to the second.
fn now_stamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

impl HeldDispatches {
    fn lock(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<u32, Wanted>> {
        self.held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn since(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<u32, String>> {
        self.since
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Every dispatch held on the person now, in the order they were asked, with when.
    pub(crate) fn listed(&self) -> Vec<(Wanted, Option<String>)> {
        let held = self.lock();
        let since = self.since();
        let mut listed: Vec<(u32, Wanted, Option<String>)> = held
            .iter()
            .map(|(id, wanted)| (*id, wanted.clone(), since.get(id).cloned()))
            .collect();
        listed.sort_by_key(|(id, ..)| *id);
        listed
            .into_iter()
            .map(|(_, wanted, at)| (wanted, at))
            .collect()
    }

    /// The person kept `wanted` blocked: listed, newest last, until the app ends or
    /// [`KEPT_BLOCKED_LISTED`] newer ones push it out.
    pub(crate) fn kept_blocked(&self, wanted: Wanted) {
        let mut kept = self
            .kept_blocked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if kept.len() == KEPT_BLOCKED_LISTED {
            kept.pop_front();
        }
        kept.push_back((wanted, now_stamp()));
    }

    /// The dispatches the person kept blocked, oldest first, with when.
    pub(crate) fn kept_blocked_listed(&self) -> Vec<(Wanted, String)> {
        self.kept_blocked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }

    /// Keeps `wanted` as held dispatch `pending`, unless one is kept under that number
    /// already: the store answers a second ask across the same pair with the first's number,
    /// and the person saw the first's brief. Answers what the dispatch already kept is called
    /// ([`Wanted::shown`]), where there was one.
    fn keep(&self, pending: u32, wanted: &Wanted) -> Option<String> {
        let mut held = self.lock();
        if let Some(first) = held.get(&pending) {
            return Some(first.shown().unwrap_or_else(|| first.name.clone()));
        }
        held.insert(pending, wanted.clone());
        self.since().insert(pending, now_stamp());
        None
    }

    fn take(&self, pending: u32) -> Option<Wanted> {
        let taken = self.lock().remove(&pending);
        self.since().remove(&pending);
        taken
    }

    /// Chat `session` closed: what it asked for goes with it, as the store's own entry does.
    pub fn forget(&self, session: u32) {
        let mut held = self.lock();
        held.retain(|_, wanted| wanted.chat != session);
        self.since().retain(|id, _| held.contains_key(id));
        drop(held);
        // And what it waits on memory for: nothing would tell it.
        self.on_memory().retain(|one| one.wanted.chat != session);
    }

    /// Takes out everything chat `session` asked for that still waits on the person.
    fn taken_from(&self, session: u32) -> Vec<Wanted> {
        let mut held = self.lock();
        let ids: Vec<u32> = held
            .iter()
            .filter(|(_, wanted)| wanted.chat == session)
            .map(|(id, _)| *id)
            .collect();
        let mut taken: Vec<(u32, Wanted)> = ids
            .into_iter()
            .filter_map(|id| held.remove(&id).map(|wanted| (id, wanted)))
            .collect();
        self.since().retain(|id, _| held.contains_key(id));
        // In the order they were asked.
        taken.sort_by_key(|(id, _)| *id);
        taken.into_iter().map(|(_, wanted)| wanted).collect()
    }

    fn on_memory(&self) -> std::sync::MutexGuard<'_, Vec<OnMemory>> {
        self.on_memory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// This project's reader of the machine's memory: what a dispatch's last step asks, and
    /// where a test puts its stand-in.
    pub fn memory(&self) -> &purlis_core::memorypressure::Gauge {
        &self.memory
    }

    /// Holds `wanted` until memory frees (#1467), counted from now, or answers why not: its
    /// chat already has [`MOST_ON_MEMORY_PER_CHAT`] waiting.
    fn wait_on_memory(&self, wanted: &Wanted) -> Result<(), String> {
        let mut waiting = self.on_memory();
        let theirs = waiting
            .iter()
            .filter(|one| one.wanted.chat == wanted.chat)
            .count();
        if theirs >= MOST_ON_MEMORY_PER_CHAT {
            return Err(too_many_on_memory());
        }
        let id = self
            .on_memory_dealt
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        waiting.push(OnMemory {
            id,
            wanted: wanted.clone(),
            since: Instant::now(),
            at: now_stamp(),
        });
        Ok(())
    }

    /// Puts `one` back, still waiting, where it was among the others.
    fn put_back(&self, one: OnMemory) {
        let mut waiting = self.on_memory();
        let at = waiting.partition_point(|other| other.id < one.id);
        waiting.insert(at, one);
    }

    /// Every dispatch waiting on memory now, by its number and since when, oldest first.
    fn waiting_on_memory(&self) -> Vec<(u64, Instant)> {
        self.on_memory()
            .iter()
            .map(|one| (one.id, one.since))
            .collect()
    }

    /// Takes out the dispatch waiting on memory as number `id`, where it still waits: whoever
    /// takes it is the one that starts it or gives it up.
    fn take_on_memory(&self, id: u64) -> Option<OnMemory> {
        let mut waiting = self.on_memory();
        let at = waiting.iter().position(|one| one.id == id)?;
        Some(waiting.remove(at))
    }

    /// Takes out everything chat `session` asked for that still waits on memory, oldest first.
    fn on_memory_taken_from(&self, session: u32) -> Vec<Wanted> {
        let mut waiting = self.on_memory();
        let (theirs, others): (Vec<OnMemory>, Vec<OnMemory>) = waiting
            .drain(..)
            .partition(|one| one.wanted.chat == session);
        *waiting = others;
        theirs.into_iter().map(|one| one.wanted).collect()
    }

    /// Every dispatch waiting on memory now, oldest first, with when it began to wait.
    pub(crate) fn on_memory_listed(&self) -> Vec<(Wanted, String)> {
        self.on_memory()
            .iter()
            .map(|one| (one.wanted.clone(), one.at.clone()))
            .collect()
    }

    /// How many dispatches wait on memory now, by the chat that asked for each (#1617): what
    /// that chat's row says. Counted, and nothing of any brief copied.
    pub(crate) fn on_memory_by_chat(&self) -> std::collections::HashMap<u32, u32> {
        let mut by_chat = std::collections::HashMap::new();
        for one in self.on_memory().iter() {
            *by_chat.entry(one.wanted.chat).or_insert(0) += 1;
        }
        by_chat
    }

    /// `wanted` waited on memory past the bound and started nothing: listed, newest last, as
    /// the ones the person kept blocked are.
    fn gave_up(&self, wanted: Wanted) {
        let mut gave_up = self
            .gave_up
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if gave_up.len() == KEPT_BLOCKED_LISTED {
            gave_up.pop_front();
        }
        gave_up.push_back((wanted, now_stamp()));
    }

    /// The dispatches that waited on memory past the bound, oldest first, with when.
    pub(crate) fn gave_up_listed(&self) -> Vec<(Wanted, String)> {
        self.gave_up
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }
}

/// **What each chat's own harness said of how it runs**: whether its permission prompts are
/// off (#1446) and the permission level it reported (#3787), per chat, as [`HeardOfAChat`].
/// Fed by the hook reports the board takes from the chat, never by a line the chat sends. Its
/// name is the first of the two it held.
#[derive(Default)]
pub struct Unattended(std::sync::Mutex<std::collections::HashMap<u32, HeardOfAChat>>);

/// What the app heard of one chat's harness.
#[derive(Debug, Clone, Copy, Default)]
struct HeardOfAChat {
    /// Whether it ever ran with its prompts off: one way, for the chat's life
    /// ([`purlis_core::dispatchunattended::Mark`]).
    mark: purlis_core::dispatchunattended::Mark,
    /// The permission mode it last said it runs in ([`purlis_core::dispatchmode::Heard`]),
    /// which a task it dispatches starts from.
    mode: purlis_core::dispatchmode::Heard,
}

impl Unattended {
    fn lock(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<u32, HeardOfAChat>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A hook of chat `session` reported, saying whether its harness runs with its prompts
    /// off.
    pub fn heard(&self, session: u32, unattended: bool) {
        if unattended {
            self.lock()
                .entry(session)
                .or_default()
                .mark
                .heard(Some(purlis_core::floorguard::UNATTENDED_MODE));
        }
    }

    /// A hook of chat `session` reported, saying which mode its own harness runs in, where it
    /// said one its adapter reads ([`heard_mode`] asks whether it was the chat's own).
    pub fn heard_mode(&self, session: u32, mode: Option<purlis_core::dispatchmode::Reported>) {
        if mode.is_some() {
            self.lock().entry(session).or_default().mode.heard(mode);
        }
    }

    fn mark(&self, session: u32) -> purlis_core::dispatchunattended::Mark {
        self.lock()
            .get(&session)
            .map(|heard| heard.mark)
            .unwrap_or_default()
    }

    /// **The level a task chat `session` dispatches starts at** (#3787): what its harness
    /// last said, capped at auto. A chat whose harness never said starts its tasks asking.
    pub fn task_level(&self, session: u32) -> purlis_core::dispatchmode::Level {
        self.lock()
            .get(&session)
            .map(|heard| heard.mode)
            .unwrap_or_default()
            .for_a_task()
    }

    /// Chat `session` closed. A chat started again in its place is marked afresh.
    pub fn forget(&self, session: u32) {
        self.lock().remove(&session);
    }
}

/// **A hook report of chat `report.chat`, after the board took it** (#3787): the permission
/// mode it names, kept as that chat's only where it is its own harness speaking
/// ([`purlis_core::dispatchmode::of_its_own_harness`]): not a sub-agent's, and about the
/// conversation the board holds for the chat now. A nested harness's report, or one the board
/// refused, leaves the record as it was.
pub fn heard_mode(held: &Held, report: &purlis_core::hookwire::Report) {
    let now = held.board().conversation(report.chat);
    held.unattended().heard_mode(
        report.chat,
        purlis_core::dispatchmode::of_its_own_harness(report, now.as_deref()),
    );
}

/// **How chat `session`, recorded as `asking`, is taken to run**: unattended where its harness
/// ever reported its prompts off, or where the command it was started with switches them off
/// (its own words on its record, or its profile's as this machine declares it, which is what
/// "set at start" comes to). Never a word of the request.
fn attendance(
    held: &Held,
    session: u32,
    asking: &Chat,
    profiles: &purlis_core::profiles::ProfileSet,
) -> purlis_core::dispatchunattended::Attendance {
    use purlis_core::dispatchunattended::bypass_in;
    let mut mark = held.unattended().mark(session);
    let own: Vec<String> = std::iter::once(asking.program.clone())
        .chain(asking.args.iter().cloned())
        .collect();
    let declared = asking
        .profile
        .as_deref()
        .and_then(|name| profiles.get(name))
        .is_some_and(|profile| bypass_in(&profile.command).is_some());
    if declared || bypass_in(&own).is_some() {
        mark.heard(Some(purlis_core::floorguard::UNATTENDED_MODE));
    }
    mark.attendance()
}

/// **Whether a person is at chat `session`** (#1501): judged as a dispatch from it is, by
/// [`attendance`] on the profiles a launch offers ([`launch_profiles`], which [`profile_for`]
/// reads too), so the window says nothing on the tab of a chat that runs with its prompts off
/// and the two can never disagree. A chat this app does not have open is nobody's.
pub fn attended(held: &Held, session: u32) -> bool {
    let Some(asking) = held.chats().recorded_chat(session) else {
        return false;
    };
    let root = held.root();
    let launch = launch_profiles(root, &purlis_core::harness_declaration::read(root));
    attendance(held, session, &asking, &launch.0)
        == purlis_core::dispatchunattended::Attendance::Attended
}

/// The profiles a launch in `root` offers, with `declared` already read: what a dispatch
/// chooses its profile from and judges its asker's attendance by, and what [`attended`] asks.
fn launch_profiles(
    root: &std::path::Path,
    declared: &purlis_core::harness_declaration::Declarations,
) -> (
    purlis_core::profiles::ProfileSet,
    purlis_core::profiles::IgnoreCheck,
) {
    purlis_core::profiles::for_launch_in(root, declared)
}

/// Dispatches what `wanted` describes, a task or a handoff, or says why not in a sentence the
/// asking chat reads (#1436, #1444).
///
/// **The asker is this app's record of the chat whose token the line carried, never the
/// request.** Its persona, its profile, its folder, the name the new chat is told it came
/// from, where it stands in its lineage, whether anybody answers its prompts: each is read
/// here. The request says which persona, what the task is called, which of the project's
/// profiles and the brief; a handoff's says the workspace the work moves to as well
/// ([`Moved`]). Nothing else it says is read.
///
/// **This is where the parts meet**, in the decision's order
/// ([`dispatchdecision::asked_by_a_chat`]): the persona; this machine's policy; the profile
/// ([`purlis_core::personaprofile::for_dispatch`]); the limits in force for the asking chat's
/// workspace and persona, against the lineage as it stands
/// ([`purlis_core::dispatchlimits`]); then the grant. A limit is said before the person is
/// asked for anything.
///
/// **The grant is asked of the one place that gives one**
/// ([`crate::dispatchunattended::request_dispatch`]): covered, and the chat starts holding
/// its own persona's grants and nothing of the asking chat's; not covered, and a chat a person
/// is at has the dispatch held and a Notice raised on its tab, while a chat nobody is at is
/// refused. The person dispatching from a tab needs none.
///
/// **A handoff is the same dispatch** (#1444): one decision, one grant, one start. What it
/// adds is where the chat stands (the workspace it names, created here when the handoff says
/// so and only once it may start), whether a report is owed, and its row in the project's
/// dispatch log. It holds none of the asking chat's grants: the hold a handoff used to carry
/// (D-1362-5) is retired with the prompt it stood beside, and a chat that still holds
/// another's is refused a handoff as it is refused a task.
///
/// **Decided and reserved under one lock** ([`crate::chats::Chats::deciding`]), so asks in
/// flight on other threads cannot each be let past a limit the other is about to fill. The
/// lock is not held while the chat starts.
///
/// **Where it works is one word of the request, resolved here** (#1453,
/// [`dispatchplace::ground`]): another workspace is looked up among the project's own and
/// reached through no link, and a worktree is cut from the repo this app records the asking
/// chat as standing in. Both are asked before the decision, which then holds the dispatch to
/// the limits of the workspace the chat will work in as well as the asking chat's. The
/// worktree itself is cut only once the dispatch is let through and its grant stands, by the
/// brokered route ([`dispatchplace::cut`]), and is taken back where the chat does not start.
fn dispatch_it(
    held: &Held,
    plane: &PlaneId,
    wanted: &Wanted,
    size: Size,
) -> Result<Dispatched, String> {
    dispatch_noting(held, plane, wanted, size).map_err(|not| not.why)
}

/// **Whether the crossing rule stands aside for a chat nobody is at** (#1543): it was started
/// with no sandbox and asks for a persona other than the one it runs with (`runs_with`, as
/// `dispatchgrants::asking_from` read it), so the no-sandbox refusal answers instead
/// ([`purlis_core::dispatchunattended::answer_of`]). The same comparison that refusal makes,
/// so the rule never stands aside for a dispatch the refusal would let through.
fn crossing_stands_aside(to: Option<&str>, runs_with: Option<&str>, sandboxed: bool) -> bool {
    !sandboxed && to.is_some_and(|to| runs_with != Some(to))
}

/// Why a dispatch started nothing ([`dispatch_noting`]).
struct NotDispatched {
    /// The sentence for whoever is on the line for it.
    why: String,
    /// The number of a task that was let through and then did not start (#1497), where its
    /// failed row is written (`crate::unstarted::recorded`). None for every refusal before a
    /// start was tried, for a handoff, and for a start the person's stop refused.
    row: Option<u32>,
}

impl From<String> for NotDispatched {
    /// A refusal before a start was tried: a sentence, and no row.
    fn from(why: String) -> Self {
        Self { why, row: None }
    }
}

/// [`dispatch_it`], saying beside a refusal whether a task that was let through did not start
/// and has its failed row (#1497, [`NotDispatched::row`]). The answer is the same either
/// way: whoever is on the line for it is told why, once.
fn dispatch_noting(
    held: &Held,
    plane: &PlaneId,
    wanted: &Wanted,
    size: Size,
) -> Result<Dispatched, NotDispatched> {
    use crate::dispatchgrants::Requested;
    use purlis_core::dispatchdecision::{By, Decision, Moment};
    use purlis_core::dispatchplace::{self, Ground};
    use purlis_core::dispatchunattended::{self, Attendance, Inherited};
    use purlis_core::{dispatchgrant, dispatchrecord, handoff, start};

    let root = held.root();
    let from = wanted.chat;
    let not_open = || format!("chat {from} is not one this app has open");
    let asking = held.chats().recorded_chat(from).ok_or_else(not_open)?;
    // What a handoff's request says, held to its rules before anything else is asked.
    let moving = wanted
        .moved
        .as_ref()
        .map(|moved| moving(root, from, moved))
        .transpose()?;
    // The asking chat as the person sees it, which is what the new chat is told (never its
    // number), and a copy, so the note still reads once that chat is closed.
    let asker = held.chats().shown_name(from).ok_or_else(not_open)?;
    // Held to its own rule. The command asked already; asked again because the request is
    // what arrived here, and this name is drawn in a tree and on purlis's own lines. A task
    // needs one; a handoff may go without, and its chat is then called by its number.
    let label = match &wanted.moved {
        None => Some(dispatchdecision::task_name(&wanted.name)?),
        Some(_) => purlis_core::reopen::label(&wanted.name)?,
    };
    // Where the asking chat works, from this app's record of it: what the stamp says, and
    // where the report goes when that chat is gone.
    let workspace = asking
        .cwd
        .as_deref()
        .and_then(|cwd| workspace_of(root, cwd));
    let place = workspace.clone().map_or(Place::PlaneRoot, Place::Workspace);
    // The persona a new chat adopts by default, which a chat that names none runs as.
    let default = start::persona_for_a_new_chat(root);
    let pair = dispatchdecision::pair_of(&asking, wanted.to.as_deref(), default.as_deref());
    // **Where it works** (#1453): the word the dispatch gave, else the persona's own default,
    // against this app's record of where the asking chat stands. Every question that needs no
    // git, asked before anything is decided.
    // A refused place is said to whoever asked, in their words: a chat reads of `--in`, the
    // person of the choice they made in the dialog.
    let said_of_place = |refused: dispatchplace::Refused| match wanted.by {
        By::Chat => refused.say(),
        By::Person => refused.in_window(),
    };
    //
    // **A handoff's place is the workspace it names, and nothing here** (D-T61-2): where its
    // work moves is its own word, checked by [`moving`], so it is given no `--in` and its
    // persona's default worktree is not cut for it. The rule for a chat nobody is at that
    // crosses workspaces is a task's (D-1453-16): a handoff's own is ruling V99f's.
    let ground = match &wanted.moved {
        Some(_) => Ground::Asker { fell_back: false },
        None => dispatchplace::asked(wanted.place.as_deref())
            .and_then(|asked| {
                dispatchplace::ground(root, asked, pair.to.as_deref(), asking.cwd.as_deref())
            })
            .map_err(said_of_place)?,
    };
    // A worktree's folder and branch are named for the dispatch, so its record's id is minted
    // now and the record is opened under it once the chat has started.
    // Only a task is given one, and a task has a name.
    let named_for = match (&ground, &label) {
        (Ground::Worktree(repo), Some(task)) => {
            let id = dispatchrecord::mint();
            let piece = dispatchplace::piece_name(task, &id);
            Some((id, repo.clone(), piece, task.clone()))
        }
        _ => None,
    };
    // **A handoff is held to the limits of the workspace it moves into, as a task sent into
    // one is** (D-T61-7, D-1453-17): they are the project's own configuration, and a handoff
    // has always named a workspace. By the name the folder itself has, where it is there, so
    // a spelling that differs only in case reads the same limits; one a handoff is about to
    // create is read by the name it gives.
    let moves_into = wanted.moved.as_ref().map(|moved| {
        dispatchplace::workspace_folder(root, &moved.workspace)
            .map_or_else(|_| moved.workspace.clone(), |(name, _)| name)
    });
    // **The profile**: the one the dispatch names, else the persona's own, else the asking
    // chat's (#1445). Chosen before the decision, which refuses where there is none, and kept
    // with the read it was chosen from, which is the read the chat is then started from.
    let on = profile_for(
        root,
        asking.profile.as_deref(),
        pair.to.as_deref(),
        wanted.profile.as_deref(),
    );
    // Where the persona's own profile is not offered on this machine, the chat runs on the
    // asking chat's: it is told so under its stamp, and the asking chat in the answer.
    let note = on.chosen.as_ref().ok().and_then(|chosen| chosen.note());
    // And a chat given a worktree is told the branch it is on and that only the person merges
    // it, in purlis's own line under its stamp. **Where it starts sandboxed it is told it cannot
    // commit there** (#1055): a worktree's git data is outside the folder it may write. Read
    // only for a worktree task, which is the one it changes anything for.
    let sandboxed = named_for.is_some() && dispatchplace::starts_sandboxed(root);
    let mut on_a_branch: Vec<String> = named_for
        .iter()
        .map(|(_, repo, piece, _)| dispatchplace::told_the_chat(&repo.repo, piece, sandboxed))
        .collect();
    // And one started in another workspace than its asker's is told which is its own: the
    // stamp names the asking chat's (D-1453-28).
    if let Ground::Workspace { name, .. } = &ground
        && workspace.as_deref() != Some(name.as_str())
    {
        on_a_branch.push(dispatchplace::told_of_its_workspace(
            name,
            workspace.as_deref(),
        ));
    }
    // Who asked is the one thing the first message says differently on the roads: a chat's
    // brief is a request from that chat, and the person's is what they typed (#1438). A
    // handoff's is the command's own stamp, with the asking chat named the way the person
    // sees it; where it works is the workspace that stamp's command named, so it has no line
    // of `on_a_branch` (D-T61-2).
    let when = chrono::Local::now().naive_local();
    let message = match (&wanted.moved, wanted.by) {
        (Some(moved), _) => handoff::delivered_noting(&moved.message, &asker, note.as_deref())
            .expect("the stamp was read a moment ago"),
        (None, By::Chat) => handoff::task_message_telling(
            &asker,
            &place,
            when,
            &wanted.brief,
            note.as_deref(),
            &on_a_branch,
        ),
        (None, By::Person) => handoff::person_task_message_telling(
            &asker,
            &place,
            when,
            &wanted.brief,
            note.as_deref(),
            &on_a_branch,
        ),
    };
    // The command measured this already, with a name standing in for the one written here;
    // measured again because these bytes are about to become a harness's argv.
    if let Some(bad) = handoff::bad_message(&message) {
        return Err(bad.say().into());
    }
    // **A brief is not dispatched a second time across a restart** (#1513): a chat started
    // again since it dispatched a task, which sends the same brief to the same persona while
    // that task still works, is told it is running.
    if wanted.by == By::Chat && wanted.moved.is_none() {
        crate::restored::refuse_twice(held, from, &asking, pair.to.as_deref(), &wanted.brief)?;
    }
    // **Nor under the name of a task of this chat's still open** (#3788): a parent that lost
    // track of a task it unblocked is told the task's number and state, and what to do with it.
    if let (By::Chat, None, Some(name)) = (wanted.by, &wanted.moved, label.as_deref())
        && let Some(why) = same_name_open(held, from, name)
    {
        return Err(why.into());
    }
    let attended = attendance(held, from, &asking, &on.launch.0);
    // **A chat nobody is at makes no workspace** (D-1444-13): its handoff goes into one that
    // exists. Said before anything is decided, and nothing is created.
    if matches!(attended, Attendance::Unattended)
        && wanted
            .moved
            .as_ref()
            .is_some_and(|moved| moved.create_vision.is_some())
    {
        return Err(dispatchunattended::NO_WORKSPACE_IS_MADE.to_owned().into());
    }
    let asking_as = crate::dispatchgrants::asking_from(&asking, from, asker.clone(), root);
    // **Where the task works** (#1505): the workspace a handoff moves into, else the one the
    // dispatch names or cuts its worktree in, else the asking chat's own; none at the
    // project's root. Each from this app's own record, and it is where the chat is then
    // started, so a grant limited to one workspace is judged against where the work runs.
    let works_in = purlis_core::dispatchwithin::works_in(
        moves_into.as_deref(),
        ground.workspace(),
        workspace.as_deref(),
    )
    .map(str::to_owned);
    // **Before the lock the dispatch is decided under** (#1506): this machine's acceptances
    // of the project's grants are settled against git's history here, where a slow history
    // holds nothing else. Under the lock the grants in force are read by this settling's
    // verdict, and no git runs. One settling per project at a time: asks that arrive
    // together share one.
    purlis_core::dispatcharrival::settle(root);
    // **A chat nobody is at crosses into another workspace only under a grant that already
    // stands** (D-1453-16): the person's on this machine or the project's acknowledged one,
    // read here with no grant of one chat. Its own persona's rule does not carry it across:
    // with nobody to see it, a chat confined to one workspace is not let into another on that
    // rule alone. The person dispatching from its tab is at it.
    //
    // **A handoff that moves the work into another workspace as another persona crosses
    // too**, under the same rule (ADR 0090 items 4 and 13): "any persona" carries a handoff no
    // further than it carries a task. A handoff to the chat's own persona keeps its own rule
    // (it goes into a workspace that exists). Refused, the crossing is kept for the person to
    // read afterwards, as a refusal for lack of a grant is.
    let to_another = pair.to.is_some() && pair.to != pair.asking;
    let crosses_into = match (&wanted.moved, &ground) {
        (None, Ground::Workspace { name, .. }) => Some(name.as_str()),
        (Some(_), _) if to_another => moves_into.as_deref(),
        _ => None,
    };
    // **A chat this app started with no sandbox is told that first** (#1543): it dispatches
    // to no other persona whatever the grants say (`dispatchunattended`, said in the decision
    // below), so the crossing's sentence, which says a grant naming the pair would carry it,
    // would send it after a grant that carries nothing. The crossing rule still answers for
    // its own persona, which needs no sandbox.
    //
    // **"Another persona" is what that refusal itself compares** (`dispatchunattended`'s
    // `answer_of`): the target against the persona the asking chat runs with, as
    // `asking_as` read it. Never a second read of the project's default persona, which
    // could differ from the one the refusal goes by and let a dispatch the refusal reads as
    // to its own persona past the crossing rule.
    let unsandboxed_to_another = crossing_stands_aside(
        pair.to.as_deref(),
        asking_as.persona.as_deref(),
        held.chats().runs_sandboxed(from),
    );
    if let (Attendance::Unattended, By::Chat, Some(name)) = (attended, wanted.by, crosses_into)
        && workspace.as_deref() != Some(name)
        && !unsandboxed_to_another
    {
        let standing = dispatchgrant::InForce::read(root, Vec::new());
        if let Some(refused) = dispatchplace::nobody_to_ask(
            pair.asking.as_deref(),
            pair.to.as_deref(),
            &standing,
            name,
        ) {
            let road = match wanted.mode() {
                Mode::Task => purlis_core::dispatchaway::Road::Task,
                Mode::Handoff => purlis_core::dispatchaway::Road::Handoff,
            };
            return Err(crate::dispatchaway::refused_crossing(
                held,
                &asking_as,
                pair.to.as_deref(),
                refused.say(),
                (name, road),
            )
            .into());
        }
    }

    // **Decided, and its slot reserved, under one lock.** Asks arrive a thread each, and a
    // start takes seconds: two dispatches that each read the counts before either chat was
    // open would both be let past a limit. So the decision and the slot that makes it count
    // are one step, and the lock is let go before anything starts.
    let (to, profile, its, lineage_of_it, number, _slot) = {
        let _deciding = held.chats().deciding();
        // A chat the person is stopping, or one below it, starts no chat (#1448). Under the
        // lock the decision is made under, so it is part of that decision.
        // **A chat's own ask.** The person asking from that chat's tab is not that chat
        // starting one, and may still ask (D-T59-j4).
        if wanted.by == By::Chat && crate::stopping::refuses_a_start(held, from) {
            return Err(crate::stopping::STARTS_NOTHING.to_owned().into());
        }
        // What the decision reads of grants only orders its answer: a limit before a question
        // to the person. A chat nobody is at has no grant of one chat read for it.
        let grants = match attended {
            Attendance::Attended => {
                held.dispatch_grants()
                    .in_force_for(root, &asking_as, works_in.as_deref())
            }
            Attendance::Unattended => {
                dispatchgrant::InForce::read(root, Vec::new()).for_task_in(works_in.as_deref())
            }
        };
        let asked = held.chats().deciding_over(|open, starting| {
            dispatchdecision::asked_by_a_chat(
                root,
                from,
                wanted.to.as_deref(),
                &Moment {
                    open,
                    working: &|chat| starting(chat) || still_working(held, chat),
                    default: default.as_deref(),
                    grants: &grants,
                    profile: on.chosen.as_ref().err(),
                    by: wanted.by,
                    mode: wanted.mode(),
                    // For a chat nobody is at, a handoff counts toward its running-per-chat
                    // limit as a task does (D-1444-13): nothing else bounds what it opens.
                    counted: match attended {
                        Attendance::Attended => dispatchdecision::Counted::Tasks,
                        Attendance::Unattended => dispatchdecision::Counted::HandoffsToo,
                    },
                    works_in: moves_into.as_deref().or(ground.workspace()),
                    // Read only where a token limit is set, from where no sandboxed chat can
                    // write (#1457).
                    session_tokens: &|| crate::overlimit::session_tokens_now(root, open, from),
                },
            )
        });
        if let Decision::Refused(why) = &asked.decision {
            // **No sandbox is said before a limit** (#1543 l3): a chat nobody is at, started
            // with no sandbox, that asks for another persona is refused that whatever the
            // limits say, so a limit's sentence (wait for a slot) would send it after a slot
            // that carries nothing. Only where the grants' own answer is that refusal: a lock,
            // the person's never and an unread list of nevers keep their places, and a
            // refusal of the policy file is policy's. It refuses the same calls; only the
            // sentence moves.
            if let (dispatchdecision::Refused::Limit(limit), Attendance::Unattended, By::Chat) =
                (why, attended, wanted.by)
                && unsandboxed_to_another
                && !matches!(
                    limit,
                    purlis_core::dispatchlimits::Refused::Off {
                        by: purlis_core::dispatchlimits::Source::PolicyRefused,
                        ..
                    }
                )
                && let Some(to) = asked.to.as_deref()
                && let Some(unsandboxed @ dispatchunattended::Refusal::Unsandboxed(_)) =
                    crate::dispatchunattended::refusal_of(
                        root,
                        &purlis_core::sandbox::policy::Locks::of(root),
                        &asking_as,
                        crate::dispatchunattended::Runs {
                            holds_anothers: asking_as.held,
                            sandboxed: false,
                        },
                        to,
                        works_in.as_deref(),
                    )
            {
                return Err(unsandboxed.say().into());
            }
            // A limit a slot frees is said on the asking chat's row too, until one does
            // (#1498): the person sees why a chat they are not reading dispatched nothing.
            let counted = match attended {
                Attendance::Attended => dispatchdecision::Counted::Tasks,
                Attendance::Unattended => dispatchdecision::Counted::HandoffsToo,
            };
            let works_in = moves_into.as_deref().or(ground.workspace());
            if held.at_limits().refused(
                from,
                (asked.to.clone(), works_in.map(str::to_owned)),
                counted,
                why,
            ) {
                held.rows_changed();
            }
            // Said to whoever asked: a chat reads what to do instead, and the person
            // reading the dialog is not that chat.
            return Err(match (wanted.by, why, &wanted.moved) {
                (By::Person, _, _) => said_to_the_person(why),
                // A handoff names no profile, so a profile the project does not list for
                // the persona is said in a handoff's own words (#1509).
                (By::Chat, dispatchdecision::Refused::Profile(profile), Some(_)) => {
                    profile.said_on(purlis_core::personaprofile::Road::Handoff)
                }
                (By::Chat, _, _) => why.say(),
            }
            .into());
        }
        // Let past every limit: a slot is free for it, and a line its last refusal put on its
        // row goes (#1498).
        held.at_limits().clear(from);
        // The decision refused where no profile was chosen.
        let chosen = on.chosen.as_ref().map_err(|refused| refused.say())?;
        // **No chat is started for another chat on a profile whose own command switches the
        // harness's prompts off**, whoever named it: the dispatch, the persona's definition
        // (which a chat can write), or nobody, where it is the asking chat's own. Said now,
        // before the person is asked for anything.
        if let Some(refused) = asks_nobody(&on, chosen, asked.to.as_deref()) {
            return Err(refused.into());
        }
        // **The grant**, where a chat asks for a persona. The person needs none, and a chat
        // on no persona dispatching to none has no pair to grant.
        let its = match (wanted.by, asked.to.as_deref()) {
            (By::Chat, Some(to)) => {
                // As `asking_as` read the chat: the crossing rule above was decided by the
                // same read (#1543).
                match crate::dispatchunattended::request_dispatch_as(
                    held,
                    asking_as.clone(),
                    attended,
                    (to, &wanted.brief),
                    works_in.as_deref(),
                    // What the grant Notice names besides the pair (#1456): the task's name
                    // as the chat gave it, and the profile chosen for it here.
                    // The name is the one checked above, never the request's own.
                    crate::dispatchgrants::Task {
                        name: label.clone(),
                        profile: Some(chosen.profile.clone()),
                        asking_profile: asking.profile.clone(),
                        named_profile: wanted.profile.clone(),
                        handoff: wanted.mode() == Mode::Handoff,
                    },
                ) {
                    Requested::Covered(its) => Some(*its),
                    Requested::NeedsGrant { pending } => {
                        return Ok(Dispatched::Held {
                            from: pair.asking.clone(),
                            to: to.to_owned(),
                            waiting: held.held_dispatches().keep(pending, wanted),
                            pending,
                        });
                    }
                    Requested::Locked(why) => {
                        return Err(dispatchdecision::Refused::Locked(why).say().into());
                    }
                    // Refused for lack of a grant with nobody there: kept for the person
                    // to read afterwards, and the chat is told they will (#1507). **Kept
                    // with the workspace the task would have worked in** (#1505), which is
                    // what the refusal was judged for and what an Allow on the item is then
                    // limited to; never the asking chat's own, where the two differ.
                    Requested::Refused(why) => {
                        return Err(crate::dispatchaway::refused(
                            held,
                            attended,
                            &asking_as,
                            to,
                            why,
                            works_in.as_deref(),
                        )
                        .into());
                    }
                }
            }
            (By::Person, Some(to)) => Some(dispatchgrant::grants_for_a_dispatched_chat(to)),
            (_, None) => None,
        };
        // **The last step: memory** (#1467, [`dispatchdecision::waits`]). Every check let it
        // through and its grant stands; while this machine is short on memory it waits, and is
        // decided again once memory frees. Nothing is reserved or created for it meanwhile.
        if let Some(dispatchdecision::Waits::Memory) =
            dispatchdecision::waits(wanted.by, held.held_dispatches().memory().read())
        {
            return Ok(Dispatched::WaitsOnMemory { to: asked.to });
        }
        let its_lineage = HandedFrom {
            // The asking chat's id, from this app's record of it (#872): the link that holds
            // once its number is dealt again or the record is read elsewhere.
            id: asked.asker,
            chat: from,
            name: asker,
            // A task is filed where the asking chat works, by this app's record of it. A
            // handoff says where it left from in its stamp, which was read and held above.
            workspace: moving
                .as_ref()
                .map_or(place, |moving| moving.left_from.clone()),
            // A handoff owes nothing: one that asked for a report was dispatched as a task
            // (#1519).
            report: match &wanted.moved {
                Some(_) => Owed::Nothing,
                None => Owed::Due,
            },
            mode: wanted.mode(),
            depth: asked.depth,
            root: asked.root,
            // Who is above it, from this app's record of the asking chat (#1521).
            above: asked.above,
            by_person: wanted.by == By::Person,
        };
        // Its number is dealt here, so the slot is the chat it is about to be.
        let number = held.chats().sessions().deal();
        let slot = held.chats().reserve(
            number,
            Chat {
                name: number.to_string(),
                profile: Some(chosen.profile.clone()),
                persona: asked.to.clone(),
                from: Some(its_lineage.clone()),
                ..Default::default()
            },
        );
        (
            asked.to,
            chosen.profile.clone(),
            its,
            its_lineage,
            number,
            slot,
        )
    };
    // **Now, and only now, a handoff's workspace is made** where the handoff says to make it:
    // it may start, so a handoff that was refused or waits on the person has created nothing.
    let created = match &wanted.moved {
        Some(Moved {
            workspace: ws,
            create_vision: Some(vision),
            ..
        }) => {
            create_it(root, ws, vision)?;
            true
        }
        _ => false,
    };
    // **From here the dispatch was let through, and a task that starts no chat is a failed
    // row under the chat that asked** (#1497): what it was, kept for the two places below
    // where a start can still be refused. A chat's own task only. The person's is answered in
    // the dialog they asked from, with what they typed still in it, and a handoff is not a
    // task.
    let unstarted = (wanted.moved.is_none() && wanted.by == By::Chat)
        .then(|| label.clone())
        .flatten()
        .map(|name| crate::unstarted::Unstarted {
            asker: from,
            number,
            name,
            persona: to.clone(),
            profile: Some(profile.clone()),
            by_person: false,
            brief: wanted.brief.clone(),
            workspace: ground.workspace().map(str::to_owned).or(workspace.clone()),
            folder: None,
            id: named_for.as_ref().map(|(id, _, _, _)| id.clone()),
            worktree: None,
            was: None,
        });
    // **Its record, before the cut** (#1472): opened now, under the id its worktree is named
    // for, so an app that quits between the cut and the start leaves a worktree a dispatch
    // lists. The chat it will be has its number and no id yet. Written over once it starts,
    // or by the record of a start that did not happen; taken back where neither is written.
    if let Some((id, repo, piece, task)) = &named_for {
        early_record(
            held,
            (id, from, number),
            to.as_ref(),
            (task, repo, piece),
            &EarlyFacts {
                profile: &profile,
                workspace: ground.workspace().map(str::to_owned).or(workspace.clone()),
                brief: &wanted.brief,
                by_person: wanted.by == By::Person,
            },
        );
    }
    // **The worktree, cut now and by the app** (#1453): the dispatch is let through and its
    // grant stands, so this is the first moment anything is written for it. By the brokered
    // route, under purlis's own name for it, used exactly or refused.
    let (cut, isolation) = match &named_for {
        Some((id, repo, _, task)) => {
            // Read only where git is about to run: it asks git for the operator's identity.
            let isolation = crate::gitbroker::isolation();
            let cut = dispatchplace::cut(root, repo, task, id, &isolation)
                .map_err(said_of_place)
                // A worktree that could not be cut is a start that did not happen.
                .map_err(|why| {
                    let row = unstarted
                        .as_ref()
                        .and_then(|task| crate::unstarted::recorded(held, task, &why))
                        .map(|_| number);
                    if row.is_none() {
                        crate::dispatches::withdrawn(held, id);
                    }
                    NotDispatched { row, why }
                })?;
            (Some(cut), Some(isolation))
        }
        None => (None, None),
    };
    // Where the chat starts: its worktree, the workspace named, or the asking chat's folder.
    // A handoff's chat stands in the workspace the work moved to, not beside the asking chat.
    let cwd = match (&cut, &ground, &moving) {
        (Some(cut), _, _) => Some(cut.path.clone()),
        (None, Ground::Workspace { folder, .. }, _) => Some(folder.clone()),
        (None, Ground::Asker { .. } | Ground::Worktree(_), Some(moving)) => {
            Some(moving.dir.clone())
        }
        (None, Ground::Asker { .. } | Ground::Worktree(_), None) => asking.cwd.clone(),
    };
    // The folder is settled now: a row for a start refused from here on says where.
    let unstarted = unstarted.map(|task| crate::unstarted::Unstarted {
        folder: cwd.clone(),
        ..task
    });
    // What the chat starts on, for the one function every persona chat's start goes through.
    let its_profile = on.launch.0.get(&profile).cloned();
    // The slot is let go when this returns: the chat is open by then and counts for itself,
    // or its start was refused and nothing does.
    // The chain above the new chat as its record keeps it: what its first start reads, as
    // every later one does (#2071, `chats::made_of`).
    let chain_above = lineage_of_it.above.clone();
    let started = start_on(
        held,
        plane,
        |name| {
            // **Its sandbox is what the project compiles for its own persona in that folder**
            // (`start::ready`), as for any chat of that persona started there: nothing of the
            // asking chat's is carried, and no folder but its own is added for it (#1453).
            // How it is made (#2071), as its record will say at every later start: the
            // person's, asked for from a tab; else asked for by `asking`, as a handoff or a
            // dispatch, through the chain above it.
            let made = if wanted.by == By::Person {
                purlis_core::sandbox::Made::Window
            } else {
                purlis_core::sandbox::Made::Asked {
                    how: if wanted.mode() == Mode::Handoff {
                        purlis_core::sandbox::Asking::Handoff
                    } else {
                        purlis_core::sandbox::Asking::Dispatch
                    },
                    above: chain_above,
                }
            };
            let start = start::Start {
                cwd,
                ..dispatchdecision::start_for(&asking, its, profile, name, made)
            };
            match to.as_deref() {
                // **The joined start** (#1446): whatever the start held of the asking chat's
                // is dropped here, and a profile that asks nobody is refused here too.
                Some(target) => dispatchunattended::start_of_a_persona_chat(
                    start,
                    target,
                    its_profile.as_ref().map(|profile| Inherited {
                        profile: &profile.name,
                        command: &profile.command,
                    }),
                ),
                // A chat on no persona dispatching to none: nothing of a persona to hold.
                None => Ok(start),
            }
        },
        &Opening {
            message: &message,
            brief: &wanted.brief,
            label,
            from: lineage_of_it,
            // The workspace it works in, which is where it is filed and what its record says:
            // the one a handoff moved the work to, or the one a task's place names.
            workspace: match &wanted.moved {
                Some(moved) => Some(moved.workspace.clone()),
                None => ground.workspace().map(str::to_owned).or(workspace.clone()),
            },
            number: Some(number),
            by_person: wanted.by == By::Person,
            id: named_for.as_ref().map(|(id, _, _, _)| id.clone()),
            worktree: cut.as_ref().map(|cut| dispatchrecord::Worktree {
                repo: cut.repo.clone(),
                piece: cut.piece.clone(),
                branch: Some(cut.branch.clone()),
                removed: None,
            }),
            // **A task starts in its asking chat's mode, capped at auto** (#3787): as the
            // app heard it from that chat's own harness, never as the dispatch says. A
            // handoff's chat is the person's to read, and starts in its harness's default.
            task_level: match wanted.mode() {
                Mode::Task => held.unattended().task_level(from),
                Mode::Handoff => purlis_core::dispatchmode::Level::Asks,
            },
        },
        (&on.declared, &on.launch),
        size,
    )
    .map_err(|why| match &wanted.moved {
        // A handoff that half-happened says so, the way Python's `NOTHING_ELSE` does.
        Some(moved) if created => format!(
            "{why} The workspace '{}' was created and stays; no chat was opened.",
            moved.workspace
        ),
        _ => why,
    });
    // What is still on the disk of a worktree that could not be taken back whole (#1497): the
    // record of a task that did not start names it, so the person can find it.
    let mut left_behind = None;
    let still_there = |cut: &purlis_core::chatpiece::Cut| dispatchrecord::Worktree {
        repo: cut.repo.clone(),
        piece: cut.piece.clone(),
        branch: Some(cut.branch.clone()),
        removed: None,
    };
    let started = match (started, &cut, &isolation) {
        (Ok(arrived), _, _) => Ok(arrived),
        // **A start that did not happen takes its worktree back**: nothing has written to it,
        // so git's safe removal takes the folder and the branch, and what could not be taken
        // back is said after the start's own sentence.
        (Err(refused), Some(cut), Some(isolation)) => {
            use purlis_core::chatpiece::Undone;
            Err(match dispatchplace::take_back(root, cut, isolation) {
                Ok(Undone::Gone) => refused,
                Ok(Undone::BranchKept) => {
                    left_behind = Some(still_there(cut));
                    format!(
                        "{refused} Its folder was taken back, and git kept the branch {} in {}.",
                        cut.branch, cut.repo
                    )
                }
                Err(kept) => {
                    left_behind = Some(still_there(cut));
                    format!(
                        "{refused} The branch {} cut for it in {} could not be taken back: {kept}",
                        cut.branch, cut.repo
                    )
                }
            })
        }
        (Err(refused), _, _) => Err(refused),
    };
    let arrived = match started {
        Ok(arrived) => arrived,
        // **And a task that did not start is a failed row under the chat that asked**
        // (#1497), with the whole of why. The deciding lock was let go before the start.
        //
        // **A start the person's stop refused is not a failed start** (D-T59-j4, #1448): the
        // stop says what happened to the chats below it, and one of the two places that
        // answer it had started a program and ended it. No row is written for it.
        Err(why) => {
            let stopped = why.starts_with(crate::stopping::STARTS_NOTHING);
            let row = unstarted
                .filter(|_| !stopped)
                .map(|task| crate::unstarted::Unstarted {
                    worktree: left_behind,
                    ..task
                })
                .and_then(|task| crate::unstarted::recorded(held, &task, &why))
                .map(|_| number);
            // No record of a start that did not happen was written over the one before the
            // cut: it is taken back, as its worktree was (#1472).
            if row.is_none()
                && let Some((id, ..)) = &named_for
            {
                crate::dispatches::withdrawn(held, id);
            }
            return Err(NotDispatched { why, row });
        }
    };
    debug_assert_eq!(arrived.persona, to);
    if let Some(cut) = &cut {
        // Logged `claimed` once its chat has started, as a writing chat's branch is, and
        // credited to that chat by this app's number for it.
        let who = purlis_core::pieces::Who::here(
            held.config(),
            Some(arrived.session.to_string()),
            arrived.persona.clone(),
        )
        // By its stable id too: what its branch folder's one live writer is named by (#3901).
        .for_chat(held.chats().id_of(arrived.session));
        if purlis_core::chatpiece::claim(root, cut, &who, chrono::Utc::now()).is_none() {
            tracing::warn!("purlis: a dispatch's worktree was not logged as claimed");
        }
    }
    // What the asking chat is told beside the start: a profile that fell back, a persona's
    // default worktree that gave way, and what the cut found (a dirty clone's changes stay
    // behind).
    let fell_back = match (&ground, to.as_deref()) {
        (Ground::Asker { fell_back: true }, Some(persona)) => {
            Some(dispatchplace::fell_back_note(persona))
        }
        _ => None,
    };
    // A second task in the same folder, with no branch of its own, is named beside the first
    // (#1511, V100-68), whichever chat asked for it (#1534): there are no file locks. The
    // person who chose the place in the window read it in the dialog, and reads it on the
    // asking chat's tab.
    let sharing = match (&wanted.moved, wanted.by) {
        (None, By::Chat) => crate::taskchanges::shares_a_folder(held, arrived.session, false),
        _ => None,
    };
    // A branch folder another live chat holds (#3901): the task is told who, before its first
    // commit there is refused.
    let claimed = match (&wanted.moved, &cut) {
        (None, None) => held_by_another(held, arrived.session),
        _ => None,
    };
    let noted: Vec<String> = note
        .into_iter()
        .chain(fell_back)
        .chain(sharing)
        .chain(claimed)
        .chain(cut.iter().flat_map(|cut| {
            cut.notes
                .iter()
                .map(purlis_core::worktree::Note::in_window)
                .map(|note| note.trim_end_matches('.').to_owned())
        }))
        .collect();
    // Said only where it is not simply the asking chat's folder.
    let works = match (&ground, &cut) {
        (Ground::Asker { .. }, None) => None,
        _ => Some(dispatchplace::said_to_the_asker(
            &ground,
            cut.as_ref(),
            sandboxed,
        )),
    };
    // No row in the project's dispatch log (#1457): the handoff's record is this app's.
    Ok(Dispatched::Started {
        it: Box::new(arrived),
        note: (!noted.is_empty()).then(|| noted.join("; ")),
        works,
    })
}

/// **Where chat `task` was started in a branch folder another live chat holds** (#3901), what
/// the dispatch warns of, naming the holder: purlis will refuse the task's commit, update and
/// push there until that chat hands it over. `None` anywhere else.
fn held_by_another(held: &Held, task: u32) -> Option<String> {
    let at = held.chats().chat_at(task)?;
    let cwd = at.cwd?;
    let piece = purlis_core::worktree::locate(held.root(), &cwd)?;
    let live = crate::gitbroker::live_chats(held);
    match purlis_core::piececlaim::writer(
        held.root(),
        &piece,
        at.id.as_deref(),
        Some(&live),
        purlis_core::piececlaim::Act::Write,
    ) {
        purlis_core::piececlaim::Writer::Refused(why) => Some(format!(
            "the folder it works in is held by another chat, so its writes there will be \
             refused: {}",
            why.trim_end_matches(". Nothing was done")
        )),
        _ => None,
    }
}

/// The todo a handoff the person allowed records in the workspace it moved to (#1471), as the
/// command records one for a handoff that opened at once (`purlis_core::handoff::todo_text`):
/// the brief's title and where it came from, read off the stamp this app checked, never the
/// brief itself. One already open about the same work is not written twice. Answers what went
/// wrong, in a sentence for the asking chat, or nothing.
fn held_handoff_todo(held: &Held, moved: &Moved) -> Option<String> {
    let read = purlis_core::handoff::stamped(&moved.message)?;
    let source = read.place()?;
    let text = purlis_core::handoff::todo_text(read.brief, read.chat, &source);
    let target = match purlis_core::workspaces::Plane::open(held.root()).workspace(&moved.workspace)
    {
        Ok(target) => target,
        Err(why) => {
            return Some(format!(
                "its todo could not be recorded ({})",
                purlis_core::personas::one_line(&why.to_string())
            ));
        }
    };
    if target.todo_for_the_same_work(&text).is_some() {
        return None;
    }
    target
        .add_todo(&text, chrono::Local::now().naive_local())
        .err()
        .map(|why| {
            format!(
                "its todo could not be recorded in '{}' ({})",
                moved.workspace,
                purlis_core::rewrite::os_words(&why)
            )
        })
}

/// **The person answered a dispatch that waited on them** (#1437): the grants store hands
/// each answer here ([`crate::dispatchgrants::Store::answers_with`]).
///
/// Allowed: the dispatch that was held is asked for again, as it was first asked, and is
/// decided again at this moment, under the lock, against the limits and the chats as they
/// stand now. Kept blocked: nothing starts. Either way the asking chat's command returned
/// long ago, so it is told on its next turn, as it is told a report
/// ([`purlis_core::handback::Answered`]), and nothing is typed into it. **But a task that was
/// allowed and did not start** is told as a task's failed report is (#1497,
/// `crate::unstarted::allowed`): that one may be typed the line a landed report types.
///
/// **Only what was held starts.** The brief is the one this app kept under the store's own
/// number when the Notice was raised, never the store's shown copy and never a later ask's.
pub fn answered(
    held: &Held,
    plane: &PlaneId,
    answer: &crate::dispatchgrants::Answered,
    arrived: &(dyn Fn(Arrived) + Send + Sync),
) {
    use purlis_core::handback::Answered;

    let Some(wanted) = held.held_dispatches().take(answer.pending.id) else {
        return;
    };
    if answer.asker_closed {
        went_with_its_asker(held, &wanted, &answer.pending);
        return;
    }
    let pair = format!(
        "{} to {}",
        answer
            .pending
            .asking
            .persona
            .as_deref()
            .unwrap_or("this chat"),
        answer.pending.target
    );
    // Where the start itself was refused, its failed row is written already (#1497).
    let mut row = None;
    let (how, detail) = if answer.allowed.is_none() {
        // Listed on the Dispatches tab, which has no record of it (#1456).
        held.held_dispatches().kept_blocked(wanted.clone());
        (Answered::KeptBlocked, pair)
    } else {
        match dispatch_noting(held, plane, &wanted, STARTING) {
            Ok(Dispatched::Started { it, note, works }) => {
                let detail = started_after_holding(held, &wanted, &it, (note, works));
                crate::dispatched::started(held, it.session);
                arrived(*it);
                (Answered::Started, detail)
            }
            // Allowed a moment ago and not covered now: the pair changed in between (the grant
            // was taken back, or the chat no longer asks as the persona it was allowed for).
            // Deciding it again raised a new question for the person; that one is taken back
            // out, so the chat is not told "not started" while a Notice that would start it
            // waits on its tab.
            Ok(Dispatched::Held { pending, .. }) => {
                held.held_dispatches().take(pending);
                held.dispatch_grants().withdraw(pending);
                (
                    Answered::NotStarted,
                    format!(
                        "the grant for {pair} no longer covers this dispatch. Dispatch it \
                         again, and the person is asked"
                    ),
                )
            }
            // **Allowed, and the machine is short on memory** (#1467): it waits, as one let
            // through at once does, and is decided again once memory frees, its grant asked
            // again with the rest (D-1467-6): a grant the person kept still covers it, and
            // one they took back meanwhile is not gone round. The chat is told now that it
            // waits (#1617), and again when it starts or gives up, as it would have been.
            Ok(Dispatched::WaitsOnMemory { .. }) => match waits_on_memory(held, &wanted) {
                Ok(()) => {
                    held.dispatch_grants().end_once(answer.pending.id);
                    tell_the_asker(
                        held,
                        &wanted,
                        Answered::WaitingOnMemory,
                        &still_short_after_the_wait(),
                    );
                    return;
                }
                Err(why) => (Answered::NotStarted, why),
            },
            Err(not) => {
                row = not.row;
                (Answered::NotStarted, not.why)
            }
        }
    };
    // **Whatever one start the person's answer carried ends here**, on every arm: started,
    // held again, or refused before it reached the grant store. It was for this dispatch as
    // they read it, so nothing of it is left for a later ask with another brief.
    held.dispatch_grants().end_once(answer.pending.id);
    if how == Answered::NotStarted {
        not_started_after_holding(
            held,
            &wanted,
            Some(answer.pending.target.clone()),
            row,
            &detail,
        );
        return;
    }
    // A handoff is told in the app's word on a held dispatch, and that is all: it is no task,
    // so nothing here says a task failed (a task took the road above, `unstarted::allowed`,
    // which flags the chat that asked by the task's own record).
    tell_the_asker(held, &wanted, how, &detail);
}

/// Why a held dispatch whose asking chat closed did not start (#1464): the reason its record
/// gives, after "it did not start: ".
pub(crate) const ASKER_CLOSED: &str =
    "the chat that asked for it closed before the person answered whether it could start";

/// **A held dispatch whose asking chat closed before the person answered** (#1464): nothing
/// starts and there is no chat left to tell, so it is not dropped in silence. Its dispatch
/// record is written as one that did not start ([`crate::unstarted::recorded_as`]): ended,
/// failed, marked `did_not_start`, saying its chat closed ([`ASKER_CLOSED`]). That is the
/// vocabulary every other dispatch that never ran is recorded in (#1497); no new state.
///
/// Called on the thread closing the chat, before it is taken off the app's record, so the
/// record names the chat that asked; the close then clears that chat's finished rows, this one
/// with them (V100-10). Past tasks still reads it.
fn went_with_its_asker(held: &Held, wanted: &Wanted, pending: &crate::dispatchgrants::Pending) {
    let (name, mode) = match wanted.mode() {
        Mode::Task => (
            dispatchdecision::task_name(&wanted.name).ok(),
            purlis_core::dispatchrecord::Mode::Task,
        ),
        Mode::Handoff => (wanted.shown(), purlis_core::dispatchrecord::Mode::Handoff),
    };
    let Some(name) = name else {
        tracing::info!("purlis: a held dispatch went with its chat ({ASKER_CLOSED})");
        return;
    };
    let unstarted = crate::unstarted::Unstarted {
        asker: wanted.chat,
        number: held.chats().sessions().deal(),
        name,
        persona: Some(pending.target.clone()),
        profile: wanted.profile.clone(),
        by_person: false,
        brief: wanted.brief.clone(),
        workspace: pending.works_in.clone(),
        folder: None,
        id: None,
        worktree: None,
        was: None,
    };
    crate::unstarted::recorded_as(held, &unstarted, mode, ASKER_CLOSED);
}

/// **What the asking chat is told of a dispatch that was held and has now started**: on the
/// person's Allow ([`answered`]) or once memory freed ([`memory_freed`]). Its command returned
/// long ago, so this is said on its next turn.
///
/// **A held handoff leaves what one that opened at once leaves** (#1471): its todo in the
/// workspace it moved to, which the command writes only after an open it saw. What could not
/// be written is said here, to the asking chat.
fn started_after_holding(
    held: &Held,
    wanted: &Wanted,
    it: &Arrived,
    (note, works): (Option<String>, Option<String>),
) -> String {
    let running = match &it.persona {
        Some(persona) => format!("running as {persona}"),
        None => "running".to_owned(),
    };
    let todo = wanted
        .moved
        .as_ref()
        .and_then(|moved| held_handoff_todo(held, moved));
    // Where it works, where that is not the asking chat's folder (#1453): the branch purlis
    // cut is in it.
    [
        Some(running),
        works.map(|works| format!("it works {works}")),
        note,
        todo,
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("; ")
}

/// **A dispatch that was held, and then did not start**, for `why`: the person allowed it and
/// its start was refused ([`answered`]), or it was decided again once memory freed and refused,
/// or it waited on memory past the bound ([`memory_freed`]). `row` is the number its failed row
/// was written under, where its start itself was refused.
///
/// **A task is a failed row under the chat that asked, and that chat's report** (#1497): told
/// once, as a task's report is, and not also as the app's word on a held dispatch. A handoff
/// is not a task, and keeps that word.
fn not_started_after_holding(
    held: &Held,
    wanted: &Wanted,
    persona: Option<String>,
    row: Option<u32>,
    why: &str,
) {
    if wanted.moved.is_none()
        && let Ok(name) = dispatchdecision::task_name(&wanted.name)
    {
        let task = crate::unstarted::Unstarted {
            asker: wanted.chat,
            // The number its start was dealt; one dealt now for a dispatch refused before a
            // start was tried, so the chat that asked has a number to wait on and list it by.
            number: row.unwrap_or_else(|| held.chats().sessions().deal()),
            name,
            persona,
            profile: wanted.profile.clone(),
            by_person: false,
            brief: wanted.brief.clone(),
            workspace: workspace_of_chat(held, wanted.chat),
            folder: None,
            id: None,
            worktree: None,
            was: None,
        };
        crate::unstarted::allowed(held, &task, row.is_some(), why);
        return;
    }
    tell_the_asker(
        held,
        wanted,
        purlis_core::handback::Answered::NotStarted,
        why,
    );
}

/// **The dispatches waiting on this machine's memory are looked at again** (#1467), every
/// [`MEMORY_LOOKED_AT_EVERY`] from the project's own thread (`planes.rs`). Nothing is read
/// while none waits.
///
/// - **Still short**: each keeps waiting; one that has waited [`dispatchdecision::MEMORY_WAIT`]
///   starts nothing, is listed as given up, and the asking chat is told on its next turn
///   ([`dispatchdecision::gave_up_on_memory`]).
/// - **Memory has freed**: each is decided again, oldest first, **as it was first asked and
///   against the limits, grants and chats as they stand now** ([`dispatch_noting`]). It starts,
///   or is refused and the asking chat is told why, or waits on the person for a grant that no
///   longer stands, or, where memory is short again by the time it is decided, waits on.
///
/// `now` is the moment the bound is counted to.
pub fn memory_freed(
    held: &Held,
    plane: &PlaneId,
    now: Instant,
    arrived: &(dyn Fn(Arrived) + Send + Sync),
) {
    let waiting = held.held_dispatches().waiting_on_memory();
    if waiting.is_empty() {
        return;
    }
    let short = held.held_dispatches().memory().read().is_short();
    let before = held.held_dispatches().on_memory_by_chat();
    for (id, since) in waiting {
        if short && now.saturating_duration_since(since) < dispatchdecision::MEMORY_WAIT {
            continue;
        }
        // Taken out by whoever starts it or gives it up, so it is never both.
        let Some(one) = held.held_dispatches().take_on_memory(id) else {
            continue;
        };
        let persona = one
            .wanted
            .to
            .clone()
            .or_else(|| runs_as(held, one.wanted.chat));
        if short {
            held.held_dispatches().gave_up(one.wanted.clone());
            not_started_after_holding(
                held,
                &one.wanted,
                persona,
                None,
                &dispatchdecision::gave_up_on_memory(),
            );
            continue;
        }
        match dispatch_noting(held, plane, &one.wanted, STARTING) {
            Ok(Dispatched::Started { it, note, works }) => {
                let detail = started_after_holding(held, &one.wanted, &it, (note, works));
                crate::dispatched::started(held, it.session);
                arrived(*it);
                tell_the_asker(
                    held,
                    &one.wanted,
                    purlis_core::handback::Answered::Started,
                    &detail,
                );
            }
            // Short again by the moment it was decided: it waits on, from when it first did.
            Ok(Dispatched::WaitsOnMemory { .. }) => held.held_dispatches().put_back(one),
            // The grant it was let through on no longer stands: the person is asked now, on
            // the asking chat's tab, and their answer starts it or tells the chat ([`answered`]).
            Ok(Dispatched::Held { .. }) => {}
            Err(not) => not_started_after_holding(held, &one.wanted, persona, not.row, &not.why),
        }
    }
    // What the asking chats' rows say of what waits (#1617) moved: the window reads them again.
    if held.held_dispatches().on_memory_by_chat() != before {
        held.rows_changed();
    }
}

/// The workspace chat `chat` works in, by this app's record of it; none at the project's root.
fn workspace_of_chat(held: &Held, chat: u32) -> Option<String> {
    held.chats()
        .recorded_chat(chat)
        .and_then(|asking| asking.cwd)
        .and_then(|cwd| workspace_of(held.root(), &cwd))
}

/// **Chat `session` was started again as `started` while dispatches of its own waited on the
/// person** (a restart for a grant, Restart chat, Start fresh). The Notice was the old
/// session's and goes with it, so nothing would ever start them: the chat, which was told it
/// would hear, is told on its next turn that each was not started, and to dispatch it again.
pub fn started_again(held: &Held, session: u32, started: u32) {
    for wanted in held.held_dispatches().taken_from(session) {
        tell_the_asker(
            held,
            &Wanted {
                chat: started,
                ..wanted
            },
            purlis_core::handback::Answered::NotStarted,
            "this chat was started again before the person answered, and the question went \
             with the old run. Dispatch it again, and the person is asked",
        );
    }
    // And what waited on memory (#1467): it was the old run's to start.
    for wanted in held.held_dispatches().on_memory_taken_from(session) {
        tell_the_asker(
            held,
            &Wanted {
                chat: started,
                ..wanted
            },
            purlis_core::handback::Answered::NotStarted,
            "this chat was started again while it waited on memory, and what it asked for went \
             with the old run. Dispatch it again",
        );
    }
}

/// Leaves the asking chat of `wanted` the app's word on its held dispatch, for its next turn.
/// A chat that has closed is told nothing: nothing will prompt it again.
fn tell_the_asker(
    held: &Held,
    wanted: &Wanted,
    how: purlis_core::handback::Answered,
    detail: &str,
) {
    use purlis_core::handback::{self, For, Handback};

    let root = held.root();
    let Some(asking) = held.chats().recorded_chat(wanted.chat) else {
        return;
    };
    let Some(task) = wanted.shown() else {
        return;
    };
    let place = asking
        .cwd
        .as_deref()
        .and_then(|cwd| workspace_of(root, cwd))
        .map_or(Place::PlaneRoot, Place::Workspace);
    let word = Handback {
        from: task,
        from_workspace: place.clone(),
        to: held
            .chats()
            .shown_name(wanted.chat)
            .unwrap_or_else(|| asking.name.clone()),
        to_workspace: place,
        // One line, within a report's bound: it is read back by the rule a report is.
        summary: purlis_core::shown::one_line(detail, purlis_core::handoff::MOST_REPORT_BYTES / 2),
        task: None,
        answered: Some(how),
        stopped: None,
    };
    if let Err(why) = handback::leave(root, For::Chat(wanted.chat), &word) {
        tracing::warn!("purlis: a chat was not told what became of its dispatch ({why})");
    }
}

/// **Why no chat is started on the profile `chosen`**, where its own command switches the
/// harness's permission prompts off (`purlis_core::dispatchunattended::bypass_refusal`), in
/// the words for whoever named it. `persona` is the persona the new chat runs as. A handoff
/// and a task are held to it alike.
fn asks_nobody(
    on: &On,
    chosen: &purlis_core::personaprofile::Chosen,
    persona: Option<&str>,
) -> Option<String> {
    use purlis_core::dispatchunattended::{NamedBy, bypass_refusal};
    use purlis_core::personaprofile::Who;
    let profile = on.launch.0.get(&chosen.profile)?;
    let by = match chosen.by {
        Who::Asker => NamedBy::TheDispatch,
        Who::Persona | Who::PersonaModel => NamedBy::ThePersona(persona.unwrap_or_default()),
        Who::AskingChat => NamedBy::TheAskingChat,
    };
    bypass_refusal(&profile.name, &profile.command, by)
}

/// Whether chat `chat`'s program still runs, by the board: one that has ended, with or
/// without a report, owes no more work and is not counted against a limit (D-1436-18).
pub(crate) fn still_working(held: &Held, chat: u32) -> bool {
    !matches!(
        held.board().glance(chat).state,
        purlis_core::state::State::Done | purlis_core::state::State::Failed
    )
}

/// The profile a dispatched chat starts on, or why none, with the one read of the project's
/// profiles it was chosen from, which is the read the chat is then started from.
struct On {
    chosen: Result<purlis_core::personaprofile::Chosen, purlis_core::personaprofile::Refused>,
    declared: purlis_core::harness_declaration::Declarations,
    launch: (
        purlis_core::profiles::ProfileSet,
        purlis_core::profiles::IgnoreCheck,
    ),
}

/// **The profile a chat dispatched to `persona` starts on**, a handoff or a task (#1445): the
/// one the asking chat `named` in its dispatch, else the persona's own where its definition
/// names one, else `asking`, the profile of the chat that asked as this app recorded it. Held
/// to the profiles the project offers on this machine, approved
/// (`purlis_core::personaprofile::for_dispatch`). A persona's own profile this machine does
/// not offer falls back to `asking`, and the answer says so (D-1445-8).
fn profile_for(
    root: &std::path::Path,
    asking: Option<&str>,
    persona: Option<&str>,
    named: Option<&str>,
) -> On {
    use purlis_core::personaprofile;
    let declared = purlis_core::harness_declaration::read(root);
    let launch = launch_profiles(root, &declared);
    let chosen = personaprofile::for_dispatch(
        &persona
            .map(|who| personaprofile::named_by(root, who))
            .unwrap_or_default(),
        asking,
        named.map(str::trim).filter(|named| !named.is_empty()),
        &personaprofile::offers_of(root, &launch.0, &declared),
    );
    On {
        chosen,
        declared,
        launch,
    }
}

/// **The profile a dispatch to `target` would start its chat on now** ([`profile_for`]), or
/// none where none would be chosen: what an Allow holds the profile its Notice named to
/// (#1456).
pub(crate) fn profile_now(
    root: &std::path::Path,
    asking: Option<&str>,
    target: &str,
    named: Option<&str>,
) -> Option<String> {
    profile_for(root, asking, Some(target), named)
        .chosen
        .ok()
        .map(|chosen| chosen.profile)
}

/// `ready` with `message` as the chat's first message, **by the route the harness it runs
/// takes one** (`purlis_core::handoff::first_message_argv`): the harness is the one `profile`
/// names, whatever the asking chat runs, so nothing here knows which harness asked.
///
/// Last on the line: a positional prompt is what nothing may come after, and the app's own
/// hook arguments go in FRONT of these (`Chats::open_it`).
pub(crate) fn told_first(
    mut ready: purlis_core::start::Ready,
    profile: &str,
    message: &str,
) -> Result<purlis_core::start::Ready, String> {
    let Some(first) = ready
        .harness
        .and_then(|harness| purlis_core::handoff::first_message_argv(harness.name(), message))
    else {
        return Err(format!(
            "profile '{profile}' runs a harness purlis has not measured the first message of, \
             so it cannot be started on the brief."
        ));
    };
    ready.args.extend(first);
    ready.first_message = Some(message.to_owned());
    Ok(ready)
}

/// What a worktree task's record before its cut says besides who and where (#1472).
struct EarlyFacts<'a> {
    profile: &'a str,
    /// The workspace it works in.
    workspace: Option<String>,
    brief: &'a str,
    by_person: bool,
}

/// **Opens the record of a worktree task before its worktree is cut** (#1472): `ids` are the
/// dispatch's id, the asking chat's number and the number the new chat was dealt; `cut` is
/// the task's name, the repo and the worktree it is about to be given. The asking chat is
/// this app's record of it, and the chat it will be has its number and no id. Every fact is
/// the app's, as [`record_it`]'s are.
fn early_record(
    held: &Held,
    ids: (&str, u32, u32),
    to: Option<&String>,
    cut: (&str, &purlis_core::dispatchplace::Repo, &str),
    facts: &EarlyFacts<'_>,
) {
    use purlis_core::dispatchrecord::{self, Asker, ChatRef, Place as Worked, Worker};

    let (id, from, number) = ids;
    let (task, repo, piece) = cut;
    let Some(asker) = crate::dispatches::chat_ref(held, from) else {
        return;
    };
    let asked_from = held
        .chats()
        .recorded_chat(from)
        .and_then(|asking| asking.cwd)
        .and_then(|cwd| workspace_of(held.root(), &cwd));
    crate::dispatches::before_the_cut(
        held,
        id,
        dispatchrecord::Opening {
            mode: dispatchrecord::Mode::Task,
            asker: Asker {
                chat: asker,
                workspace: asked_from,
                by_person: facts.by_person,
                session_record: None,
            },
            persona: to.cloned(),
            worker: Worker {
                chat: ChatRef {
                    chat: number,
                    id: None,
                    name: task.to_owned(),
                    persona: to.cloned(),
                },
                harness: None,
                profile: Some(facts.profile.to_owned()),
                session_record: None,
            },
            task: Some(task.to_owned()),
            place: Worked {
                workspace: facts.workspace.clone(),
                folder: None,
                worktree: Some(dispatchrecord::Worktree {
                    repo: repo.repo.clone(),
                    piece: piece.to_owned(),
                    branch: Some(piece.to_owned()),
                    removed: None,
                }),
            },
            brief: facts.brief.to_owned(),
            report_owed: true,
        },
    );
}

/// Opens the dispatch record of the dispatch that started `session`, a handoff or a task
/// (#1452).
///
/// **Every fact but the brief is the app's.** The asking chat, its persona and its workspace
/// are the app's record of chat `from`; the persona, the profile, the folder and the worktree
/// are what the app started the new chat with, and cut for it. The brief is the one the asking
/// chat wrote, as the new chat was given it; the task name is the one the app held to a tab
/// name's rule.
fn record_it(
    held: &Held,
    session: u32,
    opening: &Opening<'_>,
    chat: &Chat,
    ready: &purlis_core::start::Ready,
) {
    use purlis_core::dispatchrecord::{self, Asker, Place as Worked, Worker};

    let root = held.root();
    let from = opening.from.chat;
    let (Some(asker), Some(worker)) = (
        crate::dispatches::chat_ref(held, from),
        crate::dispatches::chat_ref(held, session),
    ) else {
        return;
    };
    let asked_from = held
        .chats()
        .recorded_chat(from)
        .and_then(|asking| asking.cwd)
        .and_then(|cwd| workspace_of(root, &cwd));
    crate::dispatches::opened(
        held,
        opening.id.clone(),
        dispatchrecord::Opening {
            mode: match opening.from.mode {
                Mode::Handoff => dispatchrecord::Mode::Handoff,
                Mode::Task => dispatchrecord::Mode::Task,
            },
            asker: Asker {
                chat: asker,
                workspace: asked_from,
                by_person: opening.by_person,
                session_record: None,
            },
            persona: chat.persona.clone(),
            worker: Worker {
                chat: worker,
                harness: ready.harness.map(|harness| harness.name().to_owned()),
                profile: chat.profile.clone(),
                session_record: None,
            },
            task: chat.label.clone(),
            place: Worked {
                workspace: opening.workspace.clone(),
                folder: ready
                    .cwd
                    .as_deref()
                    .map(|cwd| crate::dispatches::folder(root, cwd)),
                worktree: opening.worktree.clone(),
            },
            brief: opening.brief.to_owned(),
            report_owed: opening.from.report == Owed::Due,
        },
    );
}

// ----------------------------------------------------------------------------------------
// the person's own dispatch, from a chat's tab (#1438)
// ----------------------------------------------------------------------------------------

/// What "Ask <persona>…" offers on the chats of one project (#1438): the personas it can ask,
/// or why it offers none, and the asks policy takes off one chat's tab.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct AskOffer {
    /// Every persona the project defines that is finished, by name, in order. A draft runs no
    /// chat (`purlis_core::dispatchdecision::Refused::Draft`), so it is not offered one.
    pub personas: Vec<String>,
    /// Why nothing is offered, where an administrator's policy locks all dispatch: what is
    /// locked, and who set it. The action is then absent, and the palette says this.
    pub locked: Option<String>,
    /// The asks policy locks for one chat: a pair of personas no chat dispatches across,
    /// where that chat runs as the first and the ask is to the second. That ask is not on
    /// that chat's tab, and the palette's row says why.
    pub locked_for: Vec<AskLocked>,
}

/// One ask policy takes off one chat's tab ([`AskOffer::locked_for`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct AskLocked {
    /// The chat whose tab it is.
    pub session: u32,
    /// The persona that chat's tab does not ask.
    pub persona: String,
    /// What policy forbids, and who set it.
    pub why: String,
}

/// What "Ask <persona>…" offers in the project at `root`, under `locks`, on `chats`: each
/// open chat by number, with the persona it runs as.
///
/// **Policy binds the person here as it does everywhere else** (D-1438-9). A lock on all
/// dispatch takes the action away. A locked pair takes "Ask B…" off the tabs of chats running
/// as A: what the ask does that a chat opened from the picker does not is send B's report into
/// A's next turn, and that pairing is what the lock is for. The person still starts a chat as
/// B from the picker, where nothing flows to A.
pub fn ask_offer(
    root: &std::path::Path,
    locks: &purlis_core::sandbox::policy::Locks,
    chats: &[(u32, Option<String>)],
) -> AskOffer {
    if locks.forbids_dispatch() {
        return AskOffer {
            personas: Vec::new(),
            locked: locks.dispatch_refused(None, ""),
            locked_for: Vec::new(),
        };
    }
    let personas: Vec<String> = purlis_core::workspaces::Plane::open(root)
        .personas()
        .unwrap_or_default()
        .into_iter()
        .filter(|name| !purlis_core::personaverbs::is_draft(root, name))
        .collect();
    let locked_for = chats
        .iter()
        .flat_map(|(session, runs_as)| {
            personas.iter().filter_map(|persona| {
                Some(AskLocked {
                    session: *session,
                    persona: persona.clone(),
                    why: locks.dispatch_refused(runs_as.as_deref(), persona)?,
                })
            })
        })
        .collect();
    AskOffer {
        personas,
        locked: None,
        locked_for,
    }
}

/// What the person typed into "Ask <persona>…" on a chat's tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonAsk {
    /// The chat whose tab it was: the new chat starts under it, and reports to it.
    pub chat: u32,
    /// The persona to ask.
    pub persona: String,
    /// What the task is called.
    pub name: String,
    /// What to ask.
    pub ask: String,
    /// Where the new chat works, as the dialog's choice spells it (#1453): `worktree`,
    /// `workspace:<name>`, or none for that chat's folder (or the persona's own default).
    pub place: Option<String>,
}

/// **The person dispatches to a persona from a chat's tab** (#1438): a persona chat starts
/// under chat `asked.chat`, running as `asked.persona`, on what the person typed.
///
/// **A window command, and only that.** [`answer`], which is everything a hook line can reach,
/// has no arm that comes here: [`Ask`] has no variant for the person's dispatch, so a chat
/// cannot name the person as its asker however it writes its line. That is the whole of why no
/// grant is needed. The person is the one a grant is asked of, and the press is theirs.
///
/// **It starts as that persona and holds nothing of the chat it was launched from**
/// ([`dispatchdecision::start_for`]): the project's sandbox for that persona, its hosts and
/// its vaults. So "Ask devops…" on a steward chat's tab starts a chat a brokered `secret exec`
/// answers as devops, with no Notice and no handoff brief.
///
/// The limits in force and every other check of [`dispatchdecision::decide`] hold for the
/// person too, and so does policy: where `locks` forbid all dispatch, or the pair from the
/// persona that chat runs as to the one asked ([`ask_offer`]), nothing starts, whatever the
/// window drew. A refusal is said to the person, in the window's words ([`said_to_the_person`]).
pub fn ask_persona(
    held: &Held,
    plane: &PlaneId,
    asked: &PersonAsk,
    locks: &purlis_core::sandbox::policy::Locks,
    size: Size,
) -> Result<Arrived, String> {
    let persona = asked.persona.trim();
    if persona.is_empty() {
        return Err("Choose the persona to ask.".to_owned());
    }
    let runs_as = runs_as(held, asked.chat);
    if let Some(why) = locks.dispatch_refused(runs_as.as_deref(), persona) {
        return Err(why);
    }
    if asked.ask.trim().is_empty() {
        return Err("Write what to ask, so the new chat has something to start on.".to_owned());
    }
    let wanted = Wanted {
        chat: asked.chat,
        to: Some(persona.to_owned()),
        name: asked.name.clone(),
        brief: asked.ask.clone(),
        // The persona's own profile, else that chat's: the person's ask names none.
        profile: None,
        // Where the dialog said, held to the same two words a chat's ask is.
        place: asked.place.clone(),
        by: dispatchdecision::By::Person,
        // The person asks for a task; a handoff is a chat's own to make.
        moved: None,
    };
    match dispatch_it(held, plane, &wanted, size)? {
        Dispatched::Started { it, .. } => Ok(*it),
        // The decision never asks the person for a grant of their own dispatch.
        Dispatched::Held { .. } => {
            Err("purlis did not start the chat: it asked for a grant you do not need.".to_owned())
        }
        // Nor waits theirs on memory (D-1467-4).
        Dispatched::WaitsOnMemory { .. } => Err(
            "purlis did not start the chat: it waited on memory, which your own ask does not."
                .to_owned(),
        ),
    }
}

/// The persona chat `chat` runs as, by the app's record of it: its own, else the one a new
/// chat adopts by default. `None` for a chat on none, or one not open.
fn runs_as(held: &Held, chat: u32) -> Option<String> {
    held.chats()
        .recorded_chat(chat)?
        .persona
        .or_else(|| purlis_core::start::persona_for_a_new_chat(held.root()))
}

/// Why the person's ask starts nothing, **said to the person**. [`Refused::say`] is written
/// for the chat that asked ("say in your report what is left", "wait for one to report, then
/// dispatch again"), and the person reading the dialog is not that chat: each refusal says
/// what is true of the chat whose tab they asked from, and what they can do.
///
/// [`Refused::say`]: dispatchdecision::Refused::say
pub(crate) fn said_to_the_person(why: &dispatchdecision::Refused) -> String {
    use dispatchdecision::Refused;
    use purlis_core::dispatchlimits::{ASK_THE_PERSON, Refused as Limit};
    use purlis_core::personaprofile::Refused as Profile;
    use purlis_core::shown::short;
    const IN_SETTINGS: &str = "in Settings › Project › Dispatch";
    match why {
        Refused::Profile(Profile::NoProfile) => "This tab is not on a harness profile, so there                                                  is no harness to start the new chat on. Ask                                                  from a chat that was started on a profile."
            .to_owned(),
        // The person's ask names no profile, so what they can do about one the project does
        // not list for the persona is theirs: ask from another chat, or change the list or
        // the persona (#1509).
        Refused::Profile(unlisted @ Profile::NotListed { .. }) => {
            sentence(&unlisted.said_on(purlis_core::personaprofile::Road::Person))
        }
        // Written for a chat or a person alike (`personaprofile::Refused::say`).
        Refused::Profile(other) => sentence(&other.say()),
        Refused::NoPersona(name) => {
            format!("This project has no persona '{}' that loads.", short(name))
        }
        Refused::Draft(name) => format!(
            "Persona '{}' is still a draft, and a draft persona runs no chat. Finish its \
             definition first.",
            short(name)
        ),
        // The policy's own sentence, which names who set it, without what a chat is told to
        // do about it.
        Refused::Locked(why) => why.clone(),
        Refused::Limit(Limit::Loop(name)) => format!(
            "Persona '{}' is already above this chat in its own chain of chats, and a chain \
             never goes back to a persona above it. Ask from a chat that persona did not start.",
            short(name)
        ),
        // This chat's chain began under an older version, which kept no record of it, and a
        // chat above it has closed (#1521): which personas are above it is not known.
        Refused::Limit(Limit::ChainUnread(name)) => format!(
            "This chat's chain began under an older version of purlis, which kept no record \
             of the chats above it, and one of them has closed, so persona '{}' may already be \
             above it. \
             A chain never goes back to a persona above it. Ask from a chat you started.",
            short(name)
        ),
        // What is off and which level set it are the core's words; what to do about it is
        // the person's own to do.
        Refused::Limit(off @ Limit::Off { .. }) => sentence(
            &off.say()
                .replace(ASK_THE_PERSON, &format!("You can change it {IN_SETTINGS}.")),
        ),
        Refused::Limit(Limit::TooDeep { limit, .. }) => format!(
            "This chat is {limit} chats below the one you started, which is as deep as a \
             chain goes here. Ask from a chat higher up, or raise the depth {IN_SETTINGS}."
        ),
        Refused::Limit(Limit::TooManyRunning { limit, .. }) => format!(
            "This chat already has {limit} tasks that have not reported, which is as many as it \
             may have at once. Close one or wait for one to report, or raise the \
             limit {IN_SETTINGS}."
        ),
        Refused::Limit(Limit::LineageFull { limit, .. }) => format!(
            "This chat's chain already holds {limit} chats that have not reported, which is \
             as many as it may hold. Close one or wait for one to report, or raise the limit \
             {IN_SETTINGS}."
        ),
        Refused::Limit(Limit::PersonaDispatches { persona, limit, .. }) => format!(
            "Chats running as {} already have as many tasks running between them as they may, \
             which is {limit}. Wait for one to finish, or raise the limit \
             {IN_SETTINGS}.",
            short(persona)
        ),
        Refused::Limit(Limit::SessionTokens { limit, used, by }) => format!(
            "This chat's session has used {} tokens, its own chat and its tasks together as \
             their harnesses reported them, which is as many as a session may use here ({}). \
             {}",
            purlis_core::dispatchlimits::spelled(*used),
            purlis_core::dispatchlimits::spelled(u64::from(*limit)),
            if matches!(
                by,
                purlis_core::dispatchlimits::Source::Policy
                    | purlis_core::dispatchlimits::Source::PolicyRefused
            ) {
                purlis_core::dispatchlimits::ASK_AN_ADMINISTRATOR.to_owned()
            } else {
                format!("Raise the limit {IN_SETTINGS}, or start a new chat.")
            },
        ),
        Refused::Limit(Limit::PersonaFull { persona, limit, .. }) => format!(
            "As many chats already run as {} as may at once, which is {limit}. Wait for one to \
             finish, or raise the limit {IN_SETTINGS}.",
            short(persona)
        ),
        // The chat whose tab it was closed while the ask was decided (#1521).
        Refused::NotOpen(_) => format!("{}.", sentence(&why.say())),
        // Never the person's: a helper is not a tab, a held chat's tab may ask, the person's
        // ask always names a persona, it sends no message between chats, and their own
        // dispatch is not held to their never. Said as the chat is told, should one arise.
        Refused::Helper
        | Refused::Held
        | Refused::NoPersonaNamed
        | Refused::Never(_)
        | Refused::Limit(Limit::TooManyMessages { .. }) => why.say(),
    }
}

/// `said`, opening with a capital: a refusal the core writes to follow "nothing was
/// dispatched:", read on its own in the window.
fn sentence(said: &str) -> String {
    let mut letters = said.chars();
    letters
        .next()
        .map(|first| first.to_uppercase().chain(letters).collect())
        .unwrap_or_default()
}

/// What "Ask <persona>…" offers on this project's chats: its finished personas, or why it
/// offers none, and the asks policy takes off one chat's tab.
// Its plane is a `PlaneId` the registry vouches for. On a blocking thread: it reads each
// persona's definition (SC-2).
#[tauri::command]
#[specta::specta]
pub async fn ask_persona_offer(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
) -> Result<AskOffer, String> {
    let held = planes.held(&plane)?;
    tauri::async_runtime::spawn_blocking(move || {
        let default = purlis_core::start::persona_for_a_new_chat(held.root());
        let chats: Vec<(u32, Option<String>)> = held
            .chats()
            .open_now()
            .into_iter()
            .map(|open| (open.session, open.persona.or_else(|| default.clone())))
            .collect();
        ask_offer(
            held.root(),
            &purlis_core::sandbox::policy::Locks::of(held.root()),
            &chats,
        )
    })
    .await
    .map_err(|err| format!("purlis could not read this project's personas: {err}"))
}

/// Ask `persona` from chat `session`'s tab: starts a chat as that persona, under that chat, on
/// what you typed. Its report goes to that chat, marked as started by you. Answers the new
/// chat's number; the window is told of it as it is told of any chat another chat started.
///
/// `place` is where it works (#1453): `null` for that chat's folder, `worktree` for a branch of
/// its own cut from the repo that chat works in, or `workspace:<name>` for another workspace.
// On a blocking thread, as `start_chat` is: a chat on a profile resolves its launch.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn ask_persona_chat(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    session: u32,
    persona: String,
    name: String,
    ask: String,
    place: Option<String>,
    columns: u16,
    rows: u16,
) -> Result<u32, String> {
    let held = planes.held(&plane)?;
    let asked = PersonAsk {
        chat: session,
        persona,
        name,
        ask,
        place,
    };
    let arrived = tauri::async_runtime::spawn_blocking(move || {
        let locks = purlis_core::sandbox::policy::Locks::of(held.root());
        ask_persona(&held, &plane, &asked, &locks, Size { columns, rows })
    })
    .await
    .map_err(|err| format!("purlis could not start the chat: {err}"))??;
    let started = arrived.session;
    planes.arrived(arrived);
    Ok(started)
}

/// What closing chat `session` would do with the chats below it: what the close dialog says
/// before it is answered.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ClosingChat {
    /// The persona chats it dispatched as tasks, each with where it stands.
    pub tasks: Vec<PersonaChat>,
    /// Every chat below it that is at work, by name, deepest first: its tasks that have not
    /// reported, the chats it handed work to that are mid-turn, and the same below those. With
    /// any, the close asks once: keep them running, or stop them.
    pub running: Vec<String>,
}

/// What closing chat `session` would do with the chats below it: its persona chats, each with
/// where it stands and whether it closes too, and every chat below it that is at work.
// Its plane is a `PlaneId` the registry vouches for.
#[tauri::command]
#[specta::specta]
pub fn persona_chats_of(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<ClosingChat, String> {
    let held = planes.held(&plane)?;
    Ok(ClosingChat {
        tasks: persona_chats(&held, session),
        running: running_below(&held, session),
    })
}

/// What a close that stops the chats below does next with the chat itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ThenClose {
    /// Close it now.
    Close,
    /// Smart close it: it is asked for its session record, and closes when that is saved.
    SmartClose,
}

/// **Stop them**, your answer to what closing chat `session` asks: every chat at work below
/// it is stopped, and then it is closed (`then: close`), in one step, so it
/// cannot start another in between. For a Smart close (`then: smart_close`) the chats below
/// are stopped now, and any it starts while it writes its record are stopped as it closes.
/// Each chat below gets one short turn to write what it did, as any stopped chat does, and
/// the chat that asked for it is told the operator stopped it. Answers the chats this closed
/// at once: the chat itself, where it was closed.
// On a blocking thread: ending a program waits for it to go.
#[tauri::command]
#[specta::specta]
pub async fn close_chat_stopping(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    session: u32,
    then: ThenClose,
) -> Result<Vec<u32>, String> {
    let held = planes.held(&plane)?;
    tauri::async_runtime::spawn_blocking(move || {
        held.close_chat_stopping(session, then == ThenClose::Close)
    })
    .await
    .map_err(|err| format!("purlis could not stop those chats: {err}"))
}

#[cfg(test)]
mod tests;
