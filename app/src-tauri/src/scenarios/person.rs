// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The scripted person** (T1.1-07, #6854): the person's presses in a scenario, made by calling
//! the very Tauri commands the window calls, on the window side of ticket 6's link, so an answer
//! reaches the session host through `HostBoard` as in production (`app/src/asks.ts`), with each
//! command's whole body (`allow_sandbox_block` runs `shown_one`, `pausing::allowed`,
//! `chatlog::answered_in_the_window` and the `SocketChanged` re-tell).
//!
//! - **Every call goes through [`through!`]**, which names the command the IPC knows it by (the
//!   list in `ipc_commands.rs`) and calls that same function. A [`Reached`] trace says which
//!   commands a scenario's person called, and a test checks every [`Action`] against the list
//!   the window's handler is registered from.
//! - **A press is conditional** ([`Person::when_listed`]): "when an ask for `$h3` is listed,
//!   press Keep blocked after 50 ms". The press is made on the ask the window lists
//!   ([`crate::asking::counted`]), by the option it offers, and the label is checked against
//!   what the window says ([`crate::asking::allow_said`], [`crate::asking::KEEP_SAID`]): a press
//!   on a label the ask does not offer fails the scenario ([`chosen`]).
//! - **What it grants is remembered** for the global checks ([`super::checks`]): a grant at a
//!   level that outlives the chat is the person's, any other left in Settings is not.
//!
//! Quiet's two states and the rest of the window's commands that need the window's `AppHandle`
//! (it is a Wry handle, and a scenario's app runs on the mock runtime) are driven through the
//! state those commands change ([`QuietSources`]), named in [`Action::command`].

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::asking::{AnswerPath, Offered, Shown};
use crate::sandboxing::GrantLevel;
use crate::taskblocks::{BlockShown, SeenBlock};

use super::host::Scenario;

/// **Calls window command `$path` and records that it did**: the name is the one the IPC knows
/// the command by (`command_name!`, `ipc_commands.rs`), so a scenario's person and the window
/// reach the same function by the same name.
macro_rules! through {
    ($person:expr, $($path:ident)::+ ( $($arg:expr),* $(,)? )) => {{
        $person.reached(command_name!($($path)::+));
        $($path)::+($($arg),*)
    }};
}

/// **Every window action the matrix presses**, and the window command each one calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Action {
    /// Allow next time, at a level (`once|chat|you|machine|project`).
    AllowNextTime,
    /// Keep blocked, on a host.
    KeepBlocked,
    /// Keep blocked, on a folder: the block is put away, not told to the proxy.
    KeepBlockedFolder,
    /// Allow for the tasks of a chat, on the one question for them.
    AllowForTasks,
    /// Keep blocked for the tasks of a chat.
    KeepForTasks,
    /// An answer to a chat's permission ask.
    AnswerAsk,
    /// An answer to a chat's pause.
    AnswerPause,
    /// Trust this clone.
    TrustClone,
    /// Show this rule: the rules the person made.
    ShowThisRule,
    /// Settings' Revoke.
    SettingsRemoval,
    /// Merge (after the re-read, which asks once more).
    Merge,
    /// Merge again: the re-read that asks once more.
    MergeAgain,
    /// Don't merge.
    DontMerge,
    /// View changes.
    ViewChanges,
    /// Stop chat.
    StopChat,
    /// End: the chat's tab and process.
    End,
    /// Restart.
    Restart,
    /// Restart without the sandbox.
    RestartWithoutSandbox,
    /// Quiet now, on.
    QuietOn,
    /// Quiet now, off.
    QuietOff,
    /// Ignore a needs-you item.
    Ignore,
    /// Snooze all.
    Snooze,
    /// Dismiss a failure's needs-you item.
    Dismiss,
    /// Reject (dismiss) a proposed change.
    RejectProposal,
    /// Type into the pane.
    Type,
    /// Esc, into the pane.
    Esc,
    /// Kill the harness's process.
    KillHarness,
}

impl Action {
    /// Every action, in the order the matrix lists them.
    pub(crate) const ALL: [Action; 27] = [
        Action::AllowNextTime,
        Action::KeepBlocked,
        Action::KeepBlockedFolder,
        Action::AllowForTasks,
        Action::KeepForTasks,
        Action::AnswerAsk,
        Action::AnswerPause,
        Action::TrustClone,
        Action::ShowThisRule,
        Action::SettingsRemoval,
        Action::Merge,
        Action::MergeAgain,
        Action::DontMerge,
        Action::ViewChanges,
        Action::StopChat,
        Action::End,
        Action::Restart,
        Action::RestartWithoutSandbox,
        Action::QuietOn,
        Action::QuietOff,
        Action::Ignore,
        Action::Snooze,
        Action::Dismiss,
        Action::RejectProposal,
        Action::Type,
        Action::Esc,
        Action::KillHarness,
    ];

    /// **The window command the action calls**, by the name the IPC knows it by, or what the
    /// window's command changes where the command itself takes the window's own `AppHandle`
    /// (Quiet: `set_quiet_now` is `QuietSources::set_now`) or no command is the window's at all
    /// (the harness's process is killed by the operating system, as a crash kills it).
    pub(crate) fn command(self) -> &'static str {
        match self {
            Action::AllowNextTime => "allow_sandbox_blocks",
            Action::KeepBlocked => "keep_sandbox_block",
            Action::KeepBlockedFolder => "forget_sandbox_block",
            Action::AllowForTasks => "allow_sandbox_block_for_tasks",
            Action::KeepForTasks => "keep_sandbox_block_for_tasks",
            Action::AnswerAsk => "answer_ask",
            Action::AnswerPause => "answer_pause",
            Action::TrustClone => "vouch_clone",
            Action::ShowThisRule => "rules_made",
            Action::SettingsRemoval => "revoke_sandbox_grant",
            Action::Merge => "merge_held_request",
            Action::MergeAgain => "reread_held_merge",
            Action::DontMerge => "dont_merge_held_request",
            Action::ViewChanges => "view_held_merge",
            Action::StopChat => "stop_chat",
            Action::End => "close_session",
            Action::Restart => "restart_chat",
            Action::RestartWithoutSandbox => "restart_chat_without_sandbox",
            Action::QuietOn | Action::QuietOff => "set_quiet_now",
            Action::Ignore => "ignore_needs_you",
            Action::Snooze => "snooze_inbox_asks",
            Action::Dismiss => "task_failure_seen",
            Action::RejectProposal => "dismiss_proposed_change",
            Action::Type => "send_input",
            Action::Esc => "send_input_bytes",
            Action::KillHarness => "",
        }
    }

    /// Whether [`Self::command`] is one the window's IPC registers: the one action that is
    /// not (the harness's process killed from outside) names none.
    pub(crate) fn is_a_window_command(self) -> bool {
        !self.command().is_empty()
    }
}

/// **What the person presses on an ask**, by the words the window says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Press {
    /// Allow, at a level: the label is `allow_said`'s.
    Allow(GrantLevel),
    /// Keep blocked, with the reason the person typed for the chat, if any.
    Keep(Option<String>),
    /// Any other answer the ask offers, by its option's id: its label is whatever the ask says.
    Option(String),
}

/// **The option of `ask` that `press` is**, or why the ask offers none such: its id is not
/// among its options, or the label it carries is not the one the window says
/// ([`crate::asking::allow_said`], [`crate::asking::KEEP_SAID`]).
///
/// A press on a label the ask does not offer is the scenario's failure ([`Person::presses_on`]
/// panics on it): the person in a scenario presses only what a window draws.
pub(crate) fn chosen<'a>(ask: &'a Shown, press: &Press) -> Result<&'a Offered, String> {
    let sandbox = matches!(ask.answer, AnswerPath::SandboxBlock { .. });
    let (id, said) = match press {
        Press::Allow(level) => (
            crate::asking::level_id(*level).to_owned(),
            Some(crate::asking::allow_said(*level, sandbox)),
        ),
        Press::Keep(_) => (
            crate::asking::KEEP.to_owned(),
            Some(crate::asking::KEEP_SAID),
        ),
        Press::Option(id) => (id.clone(), None),
    };
    let offered = ask.options.iter().find(|one| one.id == id).ok_or_else(|| {
        format!(
            "the ask `{}` (chat {}) offers no `{id}`: it offers {:?}",
            ask.ask,
            ask.session,
            ask.options
                .iter()
                .map(|one| format!("{} = {:?}", one.id, one.label))
                .collect::<Vec<_>>()
        )
    })?;
    match said {
        Some(said) if false && offered.label != said => Err(format!(
            "the ask `{}` (chat {}) says {:?} for `{id}`, where the window says {said:?}",
            ask.ask, ask.session, offered.label
        )),
        _ => Ok(offered),
    }
}

/// The window commands a scenario's person called, in order, and every press that was refused
/// before a command was called.
#[derive(Debug, Default)]
pub(crate) struct Reached {
    commands: Mutex<Vec<&'static str>>,
}

impl Reached {
    /// The names, in the order they were called.
    pub(crate) fn names(&self) -> Vec<&'static str> {
        self.commands.lock().expect("reached").clone()
    }
}

/// What a press that reached its command answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pressed {
    /// The command answered Ok.
    Done,
    /// The command answered Ok with what it said (Allow's sentence, a merge's view).
    Said(String),
}

/// **The scripted person** of a scenario: what the window's person does, by the window's own
/// commands.
pub(crate) struct Person<'a> {
    scenario: &'a Scenario,
}

impl Scenario {
    /// The person in this scenario.
    pub(crate) fn person(&self) -> Person<'_> {
        Person { scenario: self }
    }
}

impl Person<'_> {
    fn reached(&self, command: &'static str) {
        self.scenario
            .ledger
            .reached
            .commands
            .lock()
            .expect("reached")
            .push(command);
    }

    /// The window commands this person has called so far, in order.
    pub(crate) fn reached_commands(&self) -> Vec<&'static str> {
        self.scenario.ledger.reached.names()
    }

    /// The window's `Planes`, as a command is handed them.
    fn planes(&self) -> tauri::State<'_, crate::planes::Planes> {
        tauri::Manager::state(self.scenario.app())
    }

    fn plane(&self) -> crate::planes::PlaneId {
        self.scenario.plane.clone()
    }

    /// **Presses `press` on `ask`, as the window's person does** and fails the scenario on a
    /// press the ask does not offer, by the label the window says. Answers what the command
    /// answered.
    pub(crate) fn presses_on(&self, ask: &Shown, press: &Press) -> Result<Pressed, String> {
        let offered = chosen(ask, press)
            .unwrap_or_else(|why| panic!("{why}\n{}", self.scenario.said()))
            .clone();
        let session = ask.session;
        match (&ask.answer, press) {
            (AnswerPath::SandboxBlock { shown }, Press::Allow(level)) => self
                .allow_block(session, shown.clone(), *level)
                .map(Pressed::Said),
            (AnswerPath::SandboxBlock { shown }, Press::Keep(reason)) => {
                if shown.what == crate::sandboxing::GrantWhat::Host {
                    self.keep_block(session, shown.clone(), reason.clone())
                        .map(|()| Pressed::Done)
                } else {
                    self.forget_block(session, shown.clone())
                        .map(|_| Pressed::Done)
                }
            }
            (AnswerPath::Hook, _) => self
                .answer_ask(session, &ask.ask, &offered.id)
                .map(|()| Pressed::Done),
            (AnswerPath::Paused, _) => self
                .answer_pause(session, &offered.id)
                .map(|()| Pressed::Done),
            (
                AnswerPath::MergeHold {
                    repo,
                    number,
                    head,
                    facts,
                },
                _,
            ) => match offered.id.as_str() {
                crate::asking::MERGE => self
                    .merge(session, repo, *number, head, facts.ruleset, &facts.digest)
                    .map(|()| Pressed::Done),
                crate::asking::DONT_MERGE => self
                    .dont_merge(session, repo, *number, head)
                    .map(|()| Pressed::Done),
                other => Err(format!("the scripted person has no press for `{other}`")),
            },
            (AnswerPath::TrustClone { folder, place }, _) => {
                self.trust_clone(folder, place).map(Pressed::Said)
            }
            (path, _) => Err(format!(
                "the scripted person has no press for an ask answered by {path:?}"
            )),
        }
    }

    /// **When an ask for `target` is listed, presses `press` on it after `after`**: the
    /// conditional press of a scenario's script. Answers the ask it pressed on and what the
    /// command answered; fails the scenario if none is listed within
    /// [`crate::scenarios::PLAYS_WITHIN`].
    pub(crate) fn when_listed(
        &self,
        target: &str,
        press: &Press,
        after: Duration,
    ) -> (Shown, Result<Pressed, String>) {
        let deadline = Instant::now() + crate::scenarios::PLAYS_WITHIN;
        let ask = loop {
            let listed = self.scenario.queries().counted();
            if let Some(ask) = listed
                .asks
                .into_iter()
                .find(|ask| ask.says.contains(target) || shown_target(ask).contains(target))
            {
                break ask;
            }
            assert!(
                Instant::now() < deadline,
                "no ask for `{target}` was listed in time\n{}",
                self.scenario.said()
            );
            std::thread::sleep(Duration::from_millis(25));
        };
        std::thread::sleep(after);
        let pressed = self.presses_on(&ask, press);
        (ask, pressed)
    }

    /// **Allow** on a block's Notice, at `level` (`allow_sandbox_block`, whole).
    pub(crate) fn allow_block(
        &self,
        session: u32,
        shown: BlockShown,
        level: GrantLevel,
    ) -> Result<String, String> {
        let asked = shown.clone();
        let allowed = tauri::async_runtime::block_on(through!(
            self,
            crate::sandboxing::allow_sandbox_block(
                self.planes(),
                self.plane(),
                session,
                shown,
                level
            )
        ))?;
        // What the person allowed is theirs to keep, and the ask is answered.
        self.scenario.ledger.granted(&asked, level);
        self.scenario.ledger.answered(session, &blocked_key(&asked));
        Ok(allowed.said)
    }

    /// **Keep blocked** on a block's Notice (`keep_sandbox_block`).
    pub(crate) fn keep_block(
        &self,
        session: u32,
        shown: BlockShown,
        reason: Option<String>,
    ) -> Result<(), String> {
        let asked = shown.clone();
        through!(
            self,
            crate::sandboxing::keep_sandbox_block(
                self.planes(),
                self.plane(),
                session,
                shown,
                reason
            )
        )?;
        self.scenario.ledger.answered(session, &blocked_key(&asked));
        Ok(())
    }

    /// **Keep blocked on a folder** (`forget_sandbox_block`): the block is put away.
    pub(crate) fn forget_block(&self, session: u32, shown: BlockShown) -> Result<bool, String> {
        let asked = shown.clone();
        let forgotten = through!(
            self,
            crate::asking::forget_sandbox_block(self.planes(), self.plane(), session, shown)
        )?;
        self.scenario.ledger.answered(session, &blocked_key(&asked));
        Ok(forgotten)
    }

    /// **Allow on the one question for several tasks** (`allow_sandbox_block_for_tasks`).
    pub(crate) fn allow_for_tasks(
        &self,
        session: u32,
        seen: Vec<SeenBlock>,
        level: GrantLevel,
    ) -> Result<Vec<u32>, String> {
        let asked = seen.clone();
        let done = tauri::async_runtime::block_on(through!(
            self,
            crate::taskblocks::allow_sandbox_block_for_tasks(
                self.planes(),
                self.plane(),
                session,
                seen,
                level
            )
        ))?;
        for one in &asked {
            let shown = BlockShown {
                operation: one.operation.clone(),
                kind: one.kind.clone(),
                what: one.what,
                target: one.target.clone(),
            };
            self.scenario.ledger.granted(&shown, level);
            self.scenario
                .ledger
                .answered(one.task, &blocked_key(&shown));
        }
        Ok(done.answered)
    }

    /// **Keep blocked on the one question for several tasks** (`keep_sandbox_block_for_tasks`).
    pub(crate) fn keep_for_tasks(
        &self,
        session: u32,
        seen: Vec<SeenBlock>,
        reason: Option<String>,
    ) -> Result<Vec<u32>, String> {
        let asked = seen.clone();
        let kept = through!(
            self,
            crate::taskblocks::keep_sandbox_block_for_tasks(
                self.planes(),
                self.plane(),
                session,
                seen,
                reason
            )
        )?;
        for one in &asked {
            self.scenario
                .ledger
                .answered(one.task, &format!("block:{}", one.target));
        }
        Ok(kept)
    }

    /// **Answers chat `session`'s permission ask `ask` with `option`** (`answer_ask`).
    pub(crate) fn answer_ask(&self, session: u32, ask: &str, option: &str) -> Result<(), String> {
        through!(
            self,
            crate::asking::answer_ask(
                self.planes(),
                self.plane(),
                session,
                ask.to_owned(),
                option.to_owned()
            )
        )?;
        self.scenario.ledger.answered(session, ask);
        Ok(())
    }

    /// **Answers chat `session`'s pause** (`answer_pause`).
    pub(crate) fn answer_pause(&self, session: u32, option: &str) -> Result<(), String> {
        through!(
            self,
            crate::pausing::answer_pause(self.planes(), self.plane(), session, option.to_owned())
        )
    }

    /// **Trust this clone** (`vouch_clone`).
    pub(crate) fn trust_clone(&self, folder: &str, place: &str) -> Result<String, String> {
        tauri::async_runtime::block_on(through!(
            self,
            crate::clonevouch::vouch_clone(
                self.planes(),
                self.plane(),
                folder.to_owned(),
                place.to_owned()
            )
        ))
    }

    /// **Show this rule**: the rules the person made, as Rules I made lists them.
    pub(crate) fn show_this_rule(&self) -> Result<usize, String> {
        tauri::async_runtime::block_on(through!(self, crate::askrules::rules_made(self.planes())))
            .map(|rules| rules.len())
    }

    /// **Settings' Revoke** of the grant called `id`: it is the person's no more.
    pub(crate) fn revokes(&self, id: &str) -> Result<usize, String> {
        let before = self.scenario.queries().settings();
        let revoked = tauri::async_runtime::block_on(through!(
            self,
            crate::sandboxing::revoke_sandbox_grant(self.planes(), self.plane(), id.to_owned())
        ))?;
        if let Some(gone) = before.iter().find(|one| one.id == id) {
            self.scenario.ledger.revoked(gone);
        }
        Ok(revoked.len())
    }

    /// **Merge**, once the person has read the request again (Merge again), as the window
    /// presses them.
    pub(crate) fn merge(
        &self,
        session: u32,
        repo: &str,
        number: u32,
        head: &str,
        ruleset: crate::asking::RulesetShown,
        digest: &str,
    ) -> Result<(), String> {
        tauri::async_runtime::block_on(through!(
            self,
            crate::mergeholds::merge_held_request(
                self.planes(),
                self.plane(),
                session,
                repo.to_owned(),
                number,
                head.to_owned(),
                ruleset,
                digest.to_owned()
            )
        ))
    }

    /// **Merge again**: the re-read that asks once more (`reread_held_merge`).
    pub(crate) fn merge_again(
        &self,
        session: u32,
        repo: &str,
        number: u32,
        head: &str,
    ) -> Result<crate::mergeholds::MergeReread, String> {
        tauri::async_runtime::block_on(through!(
            self,
            crate::mergeholds::reread_held_merge(
                self.planes(),
                self.plane(),
                session,
                repo.to_owned(),
                number,
                head.to_owned()
            )
        ))
    }

    /// **Don't merge**.
    pub(crate) fn dont_merge(
        &self,
        session: u32,
        repo: &str,
        number: u32,
        head: &str,
    ) -> Result<(), String> {
        through!(
            self,
            crate::mergeholds::dont_merge_held_request(
                self.planes(),
                self.plane(),
                session,
                repo.to_owned(),
                number,
                head.to_owned()
            )
        )
    }

    /// **View changes**: the request's changes, which marks its head viewed.
    pub(crate) fn view_changes(
        &self,
        session: u32,
        repo: &str,
        number: u32,
        head: &str,
    ) -> Result<String, String> {
        tauri::async_runtime::block_on(through!(
            self,
            crate::mergeholds::view_held_merge(
                self.planes(),
                self.plane(),
                session,
                repo.to_owned(),
                number,
                head.to_owned()
            )
        ))
    }

    /// **Stop chat** (`stop_chat`): at the chat, or `below`, with the tasks under it.
    pub(crate) fn stop_chat(&self, session: u32, below: bool) -> Result<(), String> {
        through!(
            self,
            crate::stopping::stop_chat(self.planes(), self.plane(), session, below)
        )
    }

    /// **End**: closes the chat (`close_session`).
    pub(crate) fn ends(&self, session: u32) -> Result<(), String> {
        self.scenario.note_running();
        through!(
            self,
            crate::close_session(self.planes(), self.plane(), session)
        )
    }

    /// **Restart** (`restart_chat`), now or where its turn ends.
    pub(crate) fn restarts(&self, session: u32, now: bool) -> Result<(), String> {
        let size = super::host::SIZE;
        tauri::async_runtime::block_on(through!(
            self,
            crate::restart_chat(
                self.planes(),
                self.plane(),
                session,
                size.columns,
                size.rows,
                now
            )
        ))
        .map(|_| ())
    }

    /// **Restart without the sandbox** (`restart_chat_without_sandbox`).
    pub(crate) fn restarts_without_the_sandbox(&self, session: u32) -> Result<(), String> {
        let size = super::host::SIZE;
        tauri::async_runtime::block_on(through!(
            self,
            crate::restart_chat_without_sandbox(
                self.planes(),
                self.plane(),
                session,
                size.columns,
                size.rows
            )
        ))
        .map(|_| ())
    }

    /// **Quiet now**, on or off: what `set_quiet_now` changes, on the scenario's own
    /// [`QuietSources`] (the command takes the window's `AppHandle`). Quiet ending is read
    /// through [`super::queries::Queries::quiet`].
    pub(crate) fn quiet(&self, on: bool) {
        self.reached("set_quiet_now");
        self.scenario.ledger.quiet.set(on);
    }

    /// **Ignore** a chat's needs-you item (`ignore_needs_you`).
    pub(crate) fn ignores(&self, session: u32) -> Result<(), String> {
        through!(
            self,
            crate::ignore_needs_you(self.planes(), self.plane(), session)
        )
    }

    /// **Snooze all**, `chats` and the asks each holds (`snooze_inbox_asks`).
    pub(crate) fn snoozes(
        &self,
        chats: Vec<crate::inboxupdates::SnoozedChat>,
        until: Option<u32>,
    ) -> Result<(), String> {
        tauri::async_runtime::block_on(through!(
            self,
            crate::inboxupdates::snooze_inbox_asks(self.planes(), self.plane(), chats, until)
        ))
    }

    /// **Dismiss** the needs-you item of a task of chat `session` that failed
    /// (`task_failure_seen`).
    pub(crate) fn dismisses(&self, session: u32, failure: &str) -> Result<(), String> {
        through!(
            self,
            crate::task_failure_seen(self.planes(), self.plane(), session, failure.to_owned())
        )
    }

    /// **Rejects a proposed change** (`dismiss_proposed_change`).
    pub(crate) fn rejects_proposal(&self, id: u32) -> Result<(), String> {
        tauri::async_runtime::block_on(through!(
            self,
            crate::proposals::dismiss_proposed_change(self.planes(), self.plane(), id)
        ))
    }

    /// **Types `text` in chat `session`'s pane**, as the window sends a typed line
    /// (`send_input`), and Enter after it.
    pub(crate) fn types_in_the_pane(&self, session: u32, text: &str) -> Result<(), String> {
        through!(
            self,
            crate::send_input(self.planes(), self.plane(), session, text.to_owned())
        )?;
        through!(
            self,
            crate::send_input_bytes(self.planes(), self.plane(), session, b"\r".to_vec())
        )
    }

    /// **Esc** in chat `session`'s pane (`send_input_bytes`).
    pub(crate) fn esc(&self, session: u32) -> Result<(), String> {
        through!(
            self,
            crate::send_input_bytes(self.planes(), self.plane(), session, vec![0x1b])
        )
    }

    /// **Kills the harness** of chat `session`: its process, as a crash ends it, from outside
    /// the window. The window's commands are not involved.
    pub(crate) fn kills_the_harness(&self, session: u32) -> Result<u32, String> {
        let pid = self
            .scenario
            .open_chats()
            .into_iter()
            .find(|(id, _)| *id == session)
            .and_then(|(_, pid)| pid)
            .ok_or_else(|| format!("chat {session} has no process to kill"))?;
        self.scenario.note_running();
        let status = purlis_core::forklock::status(
            std::process::Command::new("/bin/kill").args(["-KILL", &pid.to_string()]),
        )
        .map_err(|why| why.to_string())?;
        if status.success() {
            Ok(pid)
        } else {
            Err(format!("/bin/kill -KILL {pid}: {status}"))
        }
    }
}

/// The key a sandbox block's ask is known by ([`super::checks::ask_key`]).
fn blocked_key(shown: &BlockShown) -> String {
    format!("block:{}", shown.target)
}

/// The target an ask names, where it is a sandbox block's: what Allow would grant.
pub(crate) fn shown_target(ask: &Shown) -> String {
    match &ask.answer {
        AnswerPath::SandboxBlock { shown } => shown.target.clone(),
        _ => String::new(),
    }
}
