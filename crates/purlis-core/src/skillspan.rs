// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How the hook lines move a chat's skill runs** (SK-3, #2766): the one reading of them, which
//! the event log's writer ([`crate::eventlog::Recorder`]) records runs from and the window draws
//! a chat's open run from, so the two can never disagree.
//!
//! A tool call whose hook found a skill starting ([`crate::hookwire::ToolCall::skill`]) opens a
//! run of it, once per turn, where the claim checks; every tool call is counted onto the runs
//! open; a question put to the person is counted as an ask; and the turn's end, a new prompt or
//! the session's end closes them ([`purlis_skills::runs::Spans`]). A sub-agent's own stop closes
//! nothing. Runs are kept by a key the caller chooses: the event log's writer keys them by the
//! chat's run, the window by the chat's number.
//!
//! **A skill Use skill… typed** (SK-12, #2775) is held for its chat ([`Tracker::typed`]), each
//! pick its own, as each pick is its own paste, and its run opens, started by
//! [`StartedBy::UseSkill`], only once the chat shows that the prompt sent was the one it was
//! typed into ([`Confirmed`]):
//!
//! - **by name**, where the chat's harness says which skill a prompt the person sent started
//!   (Claude Code's `UserPromptExpansion`, read by its adapter): the run opens when the skill
//!   named is one held and its prompt was sent, whichever of the two is heard first;
//! - **by the prompt**, everywhere else: the run opens with the next prompt where a line of that
//!   prompt holds the typed text ([`PromptMarks`]), and the next prompt drops it either way.
//!
//! One held longer than [`HELD_FOR_MS`] is dropped unopened, and so is every one held for a chat
//! whose session ends.
//!
//! **A prompt a curation action typed, or a trigger sent** (SK-20, #2783; decision 60), is held
//! the same way for its chat ([`Tracker::prompted`]), by its lead ([`PromptMarks::lead`]). The
//! next prompt decides: where it holds that lead, the skill runs its turn opens at the top, by
//! whatever route the harness took, are started by the action ([`StartedBy::Curation`]) or the
//! trigger ([`StartedBy::Trigger`]), as that prompt is what asked for them; a run opened inside
//! one is the harness's own pick. It is held until a prompt is sent, however long the person
//! reads it first, and dropped with the session.

use std::collections::HashMap;

use purlis_skills::runs::{Call, Closed, Ended, Heard, Open, Spans, Start, StartedBy, Whose};

/// The hook words of a call that failed: `posttoolusefailure` and its kin.
const FAILED: &str = "posttoolusefailure";

/// How long a typed skill is held for its prompt: ten minutes.
pub const HELD_FOR_MS: u64 = 10 * 60 * 1000;

/// **How a chat shows that the prompt sent was the one a skill was typed into**: its harness
/// adapter says ([`crate::harness::HarnessAdapter::typed_start_named`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confirmed {
    /// A hook names the skill a prompt the person sent started.
    ByName,
    /// The next prompt holds the typed text ([`PromptMarks`]).
    ByPrompt,
}

/// Every skill run open, by the caller's key for the chat it is in.
#[derive(Debug, Default)]
pub struct Tracker {
    spans: Spans,
    /// The skills Use skill… typed into each chat, by the same key, in the order typed.
    typed: HashMap<String, Vec<Held>>,
    /// The prompt a curation action or a trigger put in each chat, unsent, by the same key.
    prompted: HashMap<String, Prompted>,
    /// What started the turn under way in each chat, by the same key, where it was a curation
    /// action or a trigger: the start of every run it opens at the top.
    turn_by: HashMap<String, StartedBy>,
}

/// **A prompt a curation action typed into a chat, or a trigger sent it** (SK-20): the runs the
/// turn it is sent in opens are theirs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Prompted {
    /// [`StartedBy::Curation`] or [`StartedBy::Trigger`]; any other start is held for nothing.
    pub by: StartedBy,
    /// The prompt's lead, as [`PromptMarks::lead`] digests it. Never the text itself.
    pub lead: u64,
}

impl Prompted {
    /// The prompt `prompt`, put in a chat by `by`.
    pub fn of(by: StartedBy, prompt: &str) -> Self {
        Self {
            by,
            lead: PromptMarks::lead(prompt),
        }
    }
}

/// A skill Use skill… typed into a chat, unsent: the run its prompt opens.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Typed {
    /// The skill, by the name the chat's harness knows it by.
    pub skill: String,
    /// The run's id, minted by the window as it typed it, so the event log and the window name
    /// one run.
    pub run: ulid::Ulid,
    /// Where the chat works, `None` at the plane root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// Which copy of the skill it is.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "purlis_skills::runs::level_or_none"
    )]
    pub level: Option<purlis_skills::runs::Level>,
    /// The text typed, as [`PromptMarks`] digests it: what a prompt that holds it holds. Never
    /// the text itself.
    pub text: u64,
    pub confirmed: Confirmed,
}

/// One typed skill held for a chat.
#[derive(Debug, Clone)]
struct Held {
    typed: Typed,
    /// When it was typed, in milliseconds since 1970.
    at_ms: u64,
    /// Its prompt was sent: the turn it opens in is under way.
    sent: bool,
    /// Its harness named it as started by a prompt the person sent.
    named: bool,
}

/// What a state hook did to a chat's skill runs ([`Tracker::report`]).
#[derive(Debug, Default)]
pub struct Reported {
    /// Every run it closed, innermost first.
    pub closed: Vec<Closed>,
    /// The runs a prompt opened, for the skills Use skill… typed into it.
    pub opened: Vec<Open>,
}

/// **A prompt's marks** (SK-12): which stretches of its lines a prompt holds, as a 256-bit Bloom
/// filter of the digests of each line's word-by-word stretches, so a typed skill's text can be
/// checked against the prompt it was sent in without the prompt leaving its hook. A stretch
/// starts at any word of a line and runs on for at most [`PromptMarks::SPAN`] words, so a skill
/// typed after another on the same line is found too; at most [`PromptMarks::MOST`] are marked,
/// in the order the prompt reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PromptMarks([u64; 4]);

impl PromptMarks {
    /// The most stretches one prompt marks.
    pub const MOST: usize = 36;
    /// The most words one stretch runs to.
    pub const SPAN: usize = 6;

    /// The marks of `prompt`: every stretch of each of its lines, white space collapsed.
    pub fn of(prompt: &str) -> Self {
        let mut marks = Self::default();
        let mut marked = 0;
        for line in prompt.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            for start in 0..words.len() {
                let mut stretch = String::new();
                for word in words[start..].iter().take(Self::SPAN) {
                    if marked == Self::MOST {
                        return marks;
                    }
                    if !stretch.is_empty() {
                        stretch.push(' ');
                    }
                    stretch.push_str(word);
                    marks.mark(Self::digest(&stretch));
                    marked += 1;
                }
            }
        }
        marks
    }

    /// The digest of `prompt`'s lead: the first [`Self::SPAN`] words of its first line, the
    /// first stretch [`Self::of`] marks of a prompt that begins with it, however long the rest.
    pub fn lead(prompt: &str) -> u64 {
        let first = prompt.lines().next().unwrap_or_default();
        let lead: Vec<&str> = first.split_whitespace().take(Self::SPAN).collect();
        Self::digest(&lead.join(" "))
    }

    /// Whether nothing is marked: no prompt, or one of no words.
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|bits| *bits == 0)
    }

    /// The digest of `text` a prompt that holds it as a stretch of a line is marked with: its
    /// words, white space collapsed.
    pub fn digest(text: &str) -> u64 {
        use sha2::Digest;
        let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let hash = sha2::Sha256::digest(words.as_bytes());
        let mut first = [0u8; 8];
        first.copy_from_slice(&hash[..8]);
        u64::from_le_bytes(first)
    }

    /// Whether a line of the prompt holds the text `digest` is of. A Bloom filter's answer:
    /// never no for one marked, and rarely yes for one not.
    pub fn holds(&self, digest: u64) -> bool {
        Self::bits(digest).all(|(word, bit)| self.0[word] & bit != 0)
    }

    fn mark(&mut self, digest: u64) {
        for (word, bit) in Self::bits(digest) {
            self.0[word] |= bit;
        }
    }

    /// The three bits of 256 a digest sets, as a word and a bit in it.
    fn bits(digest: u64) -> impl Iterator<Item = (usize, u64)> {
        (0..3).map(move |n| {
            let at = usize::try_from((digest >> (n * 8)) & 255).unwrap_or(0);
            (at / 64, 1u64 << (at % 64))
        })
    }
}

impl Tracker {
    /// **Tool call `call` was heard** in the chat keyed `key`, at `at_ms`. A call that starts a
    /// skill, and that no guard refused, opens its run where the claim checks and the turn has
    /// not started that skill already, and answers it; `tokens` is asked only then. The call is
    /// then counted onto every run open, the one it opened included.
    ///
    /// A line that says a prompt the person sent started a skill ([`StartedBy::UseSkill`]) is
    /// no tool call: it opens the run of the skill of that name Use skill… typed, where one is
    /// held and its prompt was sent, or marks it to open when it is, and is counted nowhere.
    pub fn call(
        &mut self,
        key: &str,
        call: &crate::hookwire::ToolCall,
        at_ms: u64,
        tokens: impl FnOnce() -> Option<u64>,
    ) -> Option<Open> {
        use crate::hookwire::Decision;
        if let Some(started) = &call.skill
            && started.started_by == StartedBy::UseSkill
        {
            return self.named(key, &started.skill, at_ms, tokens);
        }
        let opened = match &call.skill {
            Some(started) if call.decision != Decision::Deny => {
                self.open(key, started, at_ms, tokens)
            }
            _ => None,
        };
        self.spans.heard(
            key,
            Heard {
                call: Some(call.call.as_deref().map_or(Call::Unnamed, Call::Id)),
                failed: call.tool_hook.starts_with(FAILED),
                refused: call.decision == Decision::Deny,
                asked: call.decision == Decision::Ask,
            },
        );
        opened
    }

    /// **Use skill… typed `typed` into the chat keyed `key`** at `at_ms`, unsent: held beside
    /// any typed before it, as a second paste sits beside the first, until a prompt shows it was
    /// sent ([`Confirmed`]). One with no name holds nothing.
    pub fn typed(&mut self, key: &str, typed: Typed, at_ms: u64) {
        let named = typed.skill.trim();
        if named.is_empty() {
            return;
        }
        let skill = crate::shown::readable(named, crate::shown::DISPLAY_LIMIT);
        self.typed.entry(key.to_owned()).or_default().push(Held {
            typed: Typed { skill, ..typed },
            at_ms,
            sent: false,
            named: false,
        });
    }

    /// **A curation action or a trigger put prompt `prompted` in the chat keyed `key`**, unsent:
    /// the next prompt that holds its lead is theirs, and so are the runs its turn opens at the
    /// top. The one put there last stands. Any other start is held for nothing.
    pub fn prompted(&mut self, key: &str, prompted: Prompted) {
        if matches!(prompted.by, StartedBy::Curation | StartedBy::Trigger) {
            self.prompted.insert(key.to_owned(), prompted);
        }
    }

    /// **State hook `report` was heard** in the chat keyed `key`, at `at_ms`: the turn's end, a
    /// new prompt and the session's end close every run open there, innermost first, and are
    /// answered; a question the chat put to the person is counted. A new prompt then opens the
    /// runs of the skills typed into it ([`Confirmed`]), with `tokens` asked only then; the end
    /// of a turn drops a skill its harness did not name in it, and the session's end every one.
    /// A sub-agent's report moves nothing.
    pub fn report(
        &mut self,
        key: &str,
        report: &crate::hookwire::Report,
        at_ms: u64,
        tokens: impl FnOnce() -> Option<u64>,
    ) -> Reported {
        use crate::harness::model::Said;
        use crate::state::Event;
        if report.agent.is_some() {
            return Reported::default();
        }
        self.expire(key, at_ms);
        let ended = match report.event {
            Event::Stop => {
                // A turn its prompt was sent in and that never named it: not started.
                if let Some(held) = self.typed.get_mut(key) {
                    held.retain(|one| !one.sent);
                }
                self.turn_by.remove(key);
                Ended::Turn
            }
            Event::UserPromptSubmit => Ended::Interrupted,
            Event::SessionEnd => {
                self.typed.remove(key);
                self.prompted.remove(key);
                self.turn_by.remove(key);
                Ended::Session
            }
            _ => {
                if matches!(report.event.said(report.detail), Said::Ask(_)) {
                    self.spans.heard(
                        key,
                        Heard {
                            asked: true,
                            ..Heard::default()
                        },
                    );
                }
                return Reported::default();
            }
        };
        let closed = self.spans.close(key, ended, at_ms);
        if report.event != Event::UserPromptSubmit {
            return Reported {
                closed,
                opened: Vec::new(),
            };
        }
        let marks = report.detail.prompt_marks;
        // The prompt put there decides at the next prompt, sent as it was or not.
        self.turn_by.remove(key);
        if let Some(prompted) = self.prompted.remove(key)
            && marks.holds(prompted.lead)
        {
            self.turn_by.insert(key.to_owned(), prompted.by);
        }
        let mut starting = Vec::new();
        if let Some(held) = self.typed.get_mut(key) {
            held.retain_mut(|one| match one.typed.confirmed {
                // The next prompt decides: it holds the text, or the skill was not sent.
                Confirmed::ByPrompt => {
                    if marks.holds(one.typed.text) {
                        starting.push(one.typed.clone());
                    }
                    false
                }
                Confirmed::ByName if one.named => {
                    starting.push(one.typed.clone());
                    false
                }
                Confirmed::ByName => {
                    one.sent = true;
                    true
                }
            });
        }
        let tokens = (!starting.is_empty()).then(tokens).flatten();
        let opened = starting
            .iter()
            .filter_map(|typed| self.open_typed(key, typed, at_ms, tokens))
            .collect();
        Reported { closed, opened }
    }

    /// Closes every run open in the chat keyed `key`, as `ended` at `at_ms`: its run or its
    /// window went away under them, and the skills typed into it with them.
    pub fn close(&mut self, key: &str, ended: Ended, at_ms: u64) -> Vec<Closed> {
        self.typed.remove(key);
        self.prompted.remove(key);
        self.turn_by.remove(key);
        self.spans.close(key, ended, at_ms)
    }

    /// The innermost run open in the chat keyed `key`: the skill an entry there is inside.
    pub fn inside(&self, key: &str) -> Option<&Open> {
        self.spans.inside(key)
    }

    /// The chat's harness named `skill` as started by a prompt the person sent: the held skill
    /// of that name opens now if its prompt was sent, or is marked to open when it is.
    fn named(
        &mut self,
        key: &str,
        skill: &str,
        at_ms: u64,
        tokens: impl FnOnce() -> Option<u64>,
    ) -> Option<Open> {
        self.expire(key, at_ms);
        let held = self.typed.get_mut(key)?;
        let at = held.iter().position(|one| {
            one.typed.confirmed == Confirmed::ByName && !one.named && one.typed.skill == skill
        })?;
        if !held[at].sent {
            held[at].named = true;
            return None;
        }
        let one = held.remove(at);
        self.open_typed(key, &one.typed, at_ms, tokens())
    }

    /// Drops every skill held for the chat keyed `key` longer than [`HELD_FOR_MS`].
    fn expire(&mut self, key: &str, at_ms: u64) {
        if let Some(held) = self.typed.get_mut(key) {
            held.retain(|one| at_ms.saturating_sub(one.at_ms) <= HELD_FOR_MS);
        }
    }

    /// Opens the run of typed skill `typed`, started by Use skill….
    fn open_typed(
        &mut self,
        key: &str,
        typed: &Typed,
        at_ms: u64,
        tokens: Option<u64>,
    ) -> Option<Open> {
        let whose = Whose {
            workspace: typed.workspace.clone(),
            level: typed.level,
        };
        let id = typed.run.to_string();
        self.spans
            .open(
                key,
                Start {
                    skill: &typed.skill,
                    id: &id,
                    started_by: StartedBy::UseSkill,
                    whose: &whose,
                    at_ms,
                    tokens,
                },
            )
            .cloned()
    }

    /// Opens the run `started` claims, where it checks: a start this purlis knows, a ULID for
    /// its id and a name, contained as every value a chat sends is.
    fn open(
        &mut self,
        key: &str,
        started: &crate::hookwire::SkillStarted,
        at_ms: u64,
        tokens: impl FnOnce() -> Option<u64>,
    ) -> Option<Open> {
        let skill = crate::shown::readable(started.skill.trim(), crate::shown::DISPLAY_LIMIT);
        if skill.is_empty()
            || started.started_by == StartedBy::Unknown
            || ulid::Ulid::from_string(&started.run).is_err()
            || self.spans.holds(key, &skill)
        {
            return None;
        }
        // The workspace is a claim too: one plain, readable name, or the copy is not known.
        let workspace = started.workspace.as_deref().map(str::trim);
        let whose = match workspace {
            Some(ws)
                if ws.is_empty()
                    || ws.contains(['/', '\\'])
                    || crate::shown::readable(ws, crate::shown::DISPLAY_LIMIT) != ws =>
            {
                Whose::default()
            }
            _ => Whose {
                workspace: workspace.map(str::to_owned),
                level: started.level,
            },
        };
        // At the top of a turn a curation action or a trigger began, the run is theirs.
        let started_by = match self.turn_by.get(key) {
            Some(by) if self.spans.inside(key).is_none() => *by,
            _ => started.started_by,
        };
        self.spans
            .open(
                key,
                Start {
                    skill: &skill,
                    id: &started.run,
                    started_by,
                    whose: &whose,
                    at_ms,
                    tokens: tokens(),
                },
            )
            .cloned()
    }
}

#[cfg(test)]
mod tests;
