// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Whether a chat is attended: one fact purlis sets** (T1-17, #6408; Q142, Q152, Q105). The
//! expand step: the fact stands beside the readers of today, which keep reading what they read
//! ([`crate::dispatchunattended::Mark`], [`crate::floorguard::unattended`]), until each moves to
//! it. A4 has (#6409). Dispatch (#6410), the briefing (#6411) and the commit gate (#6412) read
//! it through a seam each until T1-22 (#6413) hands them the fact A4 holds: dispatch from the
//! app's record of the chat ([`crate::dispatchunattended::fact_of`]), the briefing from
//! [`crate::briefing::Ask::attendance`], the commit gate as an argument
//! ([`crate::commitgate::nudge`]), each lowered by the old one-way mark heard from the report.
//!
//! A chat is **unattended** if purlis started it unattended, or if it reports a mode purlis
//! didn't set:
//!
//! - **How it started** ([`Begun`]): a task or a runner has nobody at it (R-106), and
//!   nor has a chat whose profile's command switches its prompts off
//!   ([`crate::dispatchunattended::bypass_in`]), read at the start as part of its kind. The
//!   app builds three kinds: [`Begun::Person`], [`Begun::Task`] and [`Begun::Runner`]. A loop's
//!   start ([`Begun::Loop`]) is owed: no start path names it yet (#6409).
//! - **A mode purlis didn't set** (R-107): only Claude Code's, and only as its armed hook
//!   reports it (R-095, [`crate::hookwire::Report::mode_unproven`]), never from the start
//!   arguments. A string nobody measured reads as [`ClaudeCodeMode::Unknown`], and that is never
//!   a mode purlis set (Q198, Q270).
//! - **Codex**: its constant bypass report is no such mode, since purlis sets its approval
//!   policy itself. Its attendance comes only from how purlis started it (R-289), and so does
//!   every other harness's: no harness is read by Claude Code's strings.
//!
//! **It moves one way** (R-318): nothing returns an unattended chat to attended. Each report is
//! the harness's, relayed beside the chat's own commands, so a later one that says otherwise is
//! no evidence that anything changed. A line that cannot prove it came from the armed hook is
//! not read for its mode, but on Claude Code its bypass bit still lowers the fact, so the fact is
//! never weaker than the old mark (Q271). A resume or a relaunch carries the fact forward
//! ([`Fact::resumed`]), never rebuilding it as attended.
//!
//! **Who holds it** (Q271, T1-18 #6409): the app, per chat, beside the chat's other records,
//! from the start that opens the chat ([`Fact::resumed`] of the fact its record kept, which for
//! a chat never started before is [`Fact::at_start`]), held from the moment its number is
//! announced, before its program exists, and fed by each report the board takes from the chat's
//! own harness. Once unattended, the chat's in-memory record says so too, so a restart and a
//! resume under a new number carry it (Q275). A report a spool drain hands the event log is never heard: its
//! mode is not proven. The reopen record keeps whether it is unattended (`chats[].unattended`,
//! `docs/plane-format.md`), so a relaunch after the app's restart carries it forward.
//!
//! **Who reads it**: A4, the release floor ([`crate::toolgate`]), which the hook asks of a line
//! it would refuse unattended, with what that line's own report says ([`Fact::hearing`]).
//! Dispatch (#6410), the briefing (#6411) and the commit gate (#6412) read it through their seams
//! until T1-22 (#6413).

use crate::dispatchunattended::Attendance;
use crate::harness::Harness;

/// **How purlis started a chat**, as the start path names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Begun {
    /// A person started it fresh, or will read it: the window, a handoff. A resume or a
    /// relaunch is a start of this kind too, but its fact comes from [`Fact::resumed`], which
    /// carries the prior fact forward, never from [`Fact::at_start`] alone.
    Person,
    /// Another chat dispatched it.
    Task,
    /// A loop started it. **Owed** (#6409): no start path the app has names it yet, so no
    /// chat is begun this way today.
    Loop,
    /// A runner runs it.
    Runner,
}

impl Begun {
    /// Whether nobody is at a chat started this way.
    fn unattended(self) -> bool {
        !matches!(self, Self::Person)
    }
}

/// **A Claude Code permission mode**, as its hooks name it in `permission_mode`: measured on
/// 2.1.325, headless `claude -p`, in `UserPromptSubmit`, `PreToolUse` and `Stop` (T1-16, #3825,
/// Q270; `tests/fixtures/harness-modes/claude-code-permission-modes.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClaudeCodeMode {
    /// `default`: it asks before each tool its rules do not allow.
    Default,
    /// `acceptEdits`: it edits files without asking, and asks for the rest.
    AcceptEdits,
    /// `plan`: it plans and asks before it acts.
    Plan,
    /// `auto`: its classifier judges each tool call in a person's place.
    Auto,
    /// `bypassPermissions`: its prompts are off, so nobody is asked.
    BypassPermissions,
    /// A string nobody measured: never read as a mode purlis set, so never as attended.
    #[serde(other)]
    Unknown,
}

impl ClaudeCodeMode {
    /// Each measured string and the mode it names.
    pub const MEASURED: [(&'static str, Self); 5] = [
        ("default", Self::Default),
        ("acceptEdits", Self::AcceptEdits),
        ("plan", Self::Plan),
        ("auto", Self::Auto),
        ("bypassPermissions", Self::BypassPermissions),
    ];

    /// The mode `permission_mode` names: only the exact measured strings count.
    pub fn of(permission_mode: &str) -> Self {
        Self::MEASURED
            .iter()
            .find(|(said, _)| *said == permission_mode)
            .map_or(Self::Unknown, |(_, mode)| *mode)
    }

    /// The string Claude Code names this mode by, or `unknown`.
    pub fn as_str(self) -> &'static str {
        Self::MEASURED
            .iter()
            .find(|(_, mode)| *mode == self)
            .map_or("unknown", |(said, _)| *said)
    }

    /// **The mode `command` starts a chat in**: its last `--permission-mode`, attached or the
    /// next word, quoted or spaced; the harness's default where it names none.
    pub fn set_in(command: &[String]) -> Self {
        use crate::sandbox::codex::{is_flag, split};
        let named = command
            .iter()
            .enumerate()
            .filter(|(_, word)| is_flag(word))
            .filter_map(|(at, word)| {
                let (flag, attached) = split(word);
                (flag == PERMISSION_MODE_FLAG).then(|| {
                    attached
                        .or_else(|| command.get(at + 1).map(String::as_str))
                        .map(|value| value.replace(['"', '\'', ' '], ""))
                })
            })
            .next_back();
        match named {
            None => Self::Default,
            Some(None) => Self::Unknown,
            Some(Some(mode)) => Self::of(&mode),
        }
    }
}

/// The flag Claude Code starts a chat in a permission mode by.
const PERMISSION_MODE_FLAG: &str = "--permission-mode";

/// **Whether a chat is attended**, for the chat's life: set at its start, moved only toward
/// unattended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fact {
    attendance: Attendance,
    /// The mode purlis started a Claude Code chat in; `None` on any other harness, whose
    /// reports are not read.
    set: Option<ClaudeCodeMode>,
    /// The last mode the chat's armed hook reported.
    reported: Option<ClaudeCodeMode>,
}

impl Fact {
    /// **The fact at a chat's fresh start**: `begun` as the start path names it, on `harness`,
    /// whose `command` is the profile's own words and those purlis adds for a task's mode,
    /// never a line the chat sends. A resume or a relaunch starts with [`Self::resumed`].
    pub fn at_start(begun: Begun, harness: Option<Harness>, command: &[String]) -> Self {
        let unattended =
            begun.unattended() || crate::dispatchunattended::bypass_in(command).is_some();
        Self {
            attendance: if unattended {
                Attendance::Unattended
            } else {
                Attendance::Attended
            },
            set: (harness == Some(Harness::ClaudeCode)).then(|| ClaudeCodeMode::set_in(command)),
            reported: None,
        }
    }

    /// **The fact at a resume or a relaunch** of a chat whose fact was `prior` (R-318, Q271):
    /// the new start's, as [`Self::at_start`] reads it, but never more attended than `prior`.
    /// It keeps the prior fact or lowers it, and never rebuilds an unattended chat as attended.
    /// The new start's mode is the one set; the new hook has reported nothing yet.
    pub fn resumed(
        prior: Self,
        begun: Begun,
        harness: Option<Harness>,
        command: &[String],
    ) -> Self {
        let mut fact = Self::at_start(begun, harness, command);
        if prior.attendance == Attendance::Unattended {
            fact.attendance = Attendance::Unattended;
        }
        fact
    }

    /// Hears one of the chat's hook reports. Its Claude Code mode the hook channel has already
    /// cleared where the line could not prove it came from the armed hook, in the window's
    /// listener and in the session host's alike, before the host relays it
    /// ([`crate::hookwire::Report::mode_unproven`]). On Claude Code its bypass bit
    /// (`detail.unattended`) lowers the fact whether or not the line is proven, as the old
    /// mark's did (Q271); on any other harness neither is read (R-289).
    pub fn heard(&mut self, report: &crate::hookwire::Report) {
        self.lowered_by(report.detail.unattended, report.detail.claude_code_mode);
    }

    /// **The fact as it would stand once it heard a line saying `unattended` and `said`**,
    /// without hearing it: what the hook asks for a tool call (T1-18, R-109), whose own report
    /// the app may not have heard yet. Read as [`Self::heard`] reads a report, so it can only
    /// keep the fact or lower it, and whoever sends the line can never raise it.
    pub fn hearing(self, unattended: bool, said: Option<ClaudeCodeMode>) -> Attendance {
        let mut fact = self;
        fact.lowered_by(unattended, said);
        fact.attendance
    }

    /// **A fact a record kept** (T1-18): only whether the chat was attended, which is all a
    /// relaunch carries forward ([`Self::resumed`]). It reads no mode, as no start set one.
    pub fn kept(attendance: Attendance) -> Self {
        Self {
            attendance,
            set: None,
            reported: None,
        }
    }

    /// Whether the chat is unattended.
    pub fn unattended(self) -> bool {
        self.attendance == Attendance::Unattended
    }

    /// Lowers the fact by a line's bypass bit and the mode it said, as [`Self::heard`] does.
    fn lowered_by(&mut self, unattended: bool, said: Option<ClaudeCodeMode>) {
        if self.set.is_some() && unattended {
            self.attendance = Attendance::Unattended;
        }
        self.hear(said);
    }

    /// Hears a mode the chat's armed hook reported, or none.
    pub fn hear(&mut self, said: Option<ClaudeCodeMode>) {
        let (Some(set), Some(said)) = (self.set, said) else {
            return;
        };
        self.reported = Some(said);
        if said != set || said == ClaudeCodeMode::Unknown {
            self.attendance = Attendance::Unattended;
        }
    }

    /// Whether the chat is attended.
    pub fn attendance(self) -> Attendance {
        self.attendance
    }

    /// The last mode the chat's armed hook reported, where its harness's is read.
    pub fn reported(self) -> Option<ClaudeCodeMode> {
        self.reported
    }
}
