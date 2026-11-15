// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The event kinds a cell may name, read from purlis-core.** A `require` or `forbid` on a
//! kind purlis never writes would pass or fail for ever without testing anything.
//!
//! - **Hook kinds** are `hook.<word>` for every word in `hookreg::HANDLERS` (never `NO_OPS`,
//!   which purlis answers with nothing and never writes). Which a dialect produces is the words it is armed
//!   with: every handler for Claude Code, `plugin::CODEX` for Codex, and for OpenCode the words
//!   its shim forwards (`opencode::TOOLS`, `DEFAULT_PRE`, and the state words below).
//! - **Kinds purlis writes itself** (stops, asks, grants, the spool, a run's start and end) are
//!   the same in every dialect. Those purlis-core names as constants are read from them; the
//!   few it writes as plain text are listed in [`WRITTEN_AS_TEXT`], and
//!   `every_kind_written_as_text_is_still_written_by_purlis_core` fails when one is no longer
//!   in purlis-core's source.

use std::collections::BTreeSet;

use purlis_core::eventlog::{self, stops};
use purlis_core::{hookreg, opencode, plugin};

use crate::model::Dialect;

/// Kinds purlis-core writes from a string literal rather than a public constant.
pub const WRITTEN_AS_TEXT: [&str; 6] = [
    "run.started",
    "run.ended",
    "trust.sandbox.grant",
    "trust.sandbox.revoke",
    "trust.sandbox.on",
    "trust.sandbox.off",
];

/// The state words OpenCode's shim sends from its own events (`opencode.rs`, the shim's
/// `run("sessionstart")`, `run("userpromptsubmit")`, `run("stop")` and `run("notification")`).
const OPENCODE_STATE_WORDS: [&str; 4] =
    ["sessionstart", "userpromptsubmit", "stop", "notification"];

/// **Every event kind purlis-core writes that a cell may name.**
#[derive(Debug, Clone)]
pub struct Kinds {
    hook_words: BTreeSet<&'static str>,
    own: BTreeSet<&'static str>,
}

impl Kinds {
    /// The kinds as purlis-core has them now.
    pub fn from_core() -> Self {
        // `hookreg::NO_OPS` are answered with exit 0 and never written, so they are not kinds.
        let hook_words = hookreg::HANDLERS.iter().map(|h| h.name).collect();
        let own = [
            stops::BLOCK,
            stops::HOLD,
            stops::LOOP_PAUSE,
            stops::ASK_ANSWERED,
            stops::ASK_RAISED,
            eventlog::SPOOL_GAP,
            eventlog::SPOOL_REJECTED,
            eventlog::SPOOL_DRAINED,
            eventlog::COMMIT_REFUSED,
            eventlog::SKILL_LOADED,
            purlis_core::sandbox::arrived::AUDITED,
        ]
        .into_iter()
        .chain(WRITTEN_AS_TEXT)
        .collect();
        Kinds { hook_words, own }
    }

    /// Whether purlis writes `kind` at all.
    pub fn knows(&self, kind: &str) -> bool {
        self.own.contains(kind) || hook_word(kind).is_some_and(|w| self.hook_words.contains(w))
    }

    /// Whether `dialect` can produce `kind`: purlis's own kinds always, a hook kind only when
    /// the dialect is armed with its word.
    pub fn produced_by(&self, kind: &str, dialect: Dialect) -> bool {
        if self.own.contains(kind) {
            return true;
        }
        let Some(word) = hook_word(kind) else {
            return false;
        };
        self.hook_words.contains(word) && dialect_words(dialect).contains(word)
    }

    /// Whether `kind` shows that the step's call ran: a tool call's own hook event, or a
    /// refusal or ask raised on it.
    pub fn proves_a_call_ran(&self, kind: &str) -> bool {
        [stops::BLOCK, stops::HOLD, stops::ASK_RAISED].contains(&kind)
            || hook_word(kind)
                .is_some_and(|w| eventlog::phase(w).is_some() || w == hookreg::HARNESS_DENIED)
    }
}

fn hook_word(kind: &str) -> Option<&str> {
    kind.strip_prefix("hook.")
}

/// The hook words `dialect` is armed with.
pub fn dialect_words(dialect: Dialect) -> BTreeSet<&'static str> {
    match dialect {
        Dialect::Claude => hookreg::HANDLERS.iter().map(|h| h.name).collect(),
        Dialect::Codex => plugin::CODEX.into_iter().collect(),
        Dialect::Opencode => opencode::TOOLS
            .iter()
            .flat_map(|t| [t.pre, t.post])
            .flatten()
            .chain([opencode::DEFAULT_PRE])
            .chain(OPENCODE_STATE_WORDS)
            .collect(),
    }
}
