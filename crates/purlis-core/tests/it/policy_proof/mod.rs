// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Proof infrastructure, first half** (M75 slice 09, #7018; spec §14): a cell's policy as an
//! input, the oracle, and what "equal" means.
//!
//! - [`PolicyInput`] is `policy {template, rules?}` on a [`Batch`] or a [`Cell`]; a cell that
//!   names none runs under `strict` ([`Batch::policy_of`], Q317.1).
//! - [`oracle`] is the expectation of an **equality cell**: the written action goes straight to
//!   Cedar's authorizer ([`policy::Policy::decide`]) over the shipped default and the cell's
//!   rules, never through `decide`'s request builder, the host or [`policy::levels`]
//!   (Q317.4). `the_oracle_never_calls_the_hook_request_builder` reads this file to keep it so.
//! - [`engine`] is what is under test on the host side: [`policy::decide::decide`] over the same
//!   input.
//! - [`compare`] says whether an [`Observed`] answer is **equal** to what the oracle expected
//!   (Q317.3): the same outcome, the same deciding `@id`s and the same outcome after the mode.
//!   A looser mismatch is an escape, a stricter one an ordinary bug (Q317.7).
//!
//! Not here yet: the fake leg and the two S-B3 cells (a scrubbed environment, a fake socket),
//! which need M74.1's scenario host on `main` (P3).

use std::path::Path;

mod oracle;
pub use oracle::oracle;

use purlis_core::enforcement::Template;
use purlis_core::policy::World;
use purlis_core::policy::decide::{self, Outcome};
use purlis_core::policy::levels::{self, Chat, Held, Sources};

/// The first line a project's rules file needs (`policy::VERSION_LINE`, schema version 1).
pub const RULES_HEADER: &str = "// purlis-policy 1\n";

/// **`policy {template, rules?}`**: the policy a cell runs under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyInput {
    /// The project's template.
    pub template: Template,
    /// The project's `policy.cedar`, with its `// purlis-policy 1` first line, if it has one.
    pub rules: Option<String>,
}

impl Default for PolicyInput {
    /// `strict`, with no rules of the project's own.
    fn default() -> Self {
        Self::template(Template::Strict)
    }
}

impl PolicyInput {
    /// A template and no rules.
    pub fn template(template: Template) -> Self {
        Self {
            template,
            rules: None,
        }
    }

    /// A template with the project's rules (the header is added).
    pub fn with_rules(template: Template, rules: &str) -> Self {
        Self {
            template,
            rules: Some(format!("{RULES_HEADER}{rules}")),
        }
    }

    /// The policy as the host loads it: the shipped default with the project's files on top.
    pub fn loaded(&self) -> levels::Loaded {
        let settings = format!("[policy]\ntemplate = \"{}\"\n", self.template.word());
        let loaded = levels::load_held(
            &Sources {
                root: Path::new("/nonexistent-plane"),
                workspaces: &[],
                personas: &[],
                organisation: None,
                accepted: None,
            },
            &Held {
                settings: Some(("purlis.toml", &settings)),
                rules: self.rules.as_deref(),
                local: None,
            },
        );
        assert!(
            loaded.notices.is_empty(),
            "a cell's policy must load clean: {:?}",
            loaded.notices
        );
        loaded
    }
}

/// **One thing asked of the policy**, written as the action a chat would take.
#[derive(Debug, Clone)]
pub struct Cell {
    /// Its own policy; the batch's, and else `strict`, when none.
    pub policy: Option<PolicyInput>,
    /// The action, a verb of the schema.
    pub action: &'static str,
    /// The resource: its entity type and id.
    pub resource: (&'static str, &'static str),
    /// The context, every field the action declares.
    pub context: serde_json::Value,
    /// A known gap this cell declares, where the harness has no rule to name (Q319 C1).
    pub gap: Option<&'static str>,
}

/// **A set of cells** sharing a policy.
#[derive(Debug, Clone, Default)]
pub struct Batch {
    /// The policy of every cell that names none.
    pub policy: Option<PolicyInput>,
    /// The cells.
    pub cells: Vec<Cell>,
}

impl Batch {
    /// **The policy `cell` runs under**: its own, else the batch's, else `strict`.
    pub fn policy_of(&self, cell: &Cell) -> PolicyInput {
        cell.policy
            .clone()
            .or_else(|| self.policy.clone())
            .unwrap_or_default()
    }
}

/// What a policy says of an action: the same three things whoever answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// Cedar's outcome.
    pub outcome: Outcome,
    /// The `@id`s of the rules that decided it.
    pub deciding: Vec<String>,
    /// The outcome once the mode is applied.
    pub after_mode: Outcome,
}

/// What an enforcement point did, for [`compare`]: an [`Answer`] as it was seen.
pub type Observed = Answer;

/// The chat id every cell is asked as.
const CHAT: &str = "cell";

fn no_chat() -> Chat<'static> {
    Chat {
        workspace: None,
        persona: None,
    }
}

fn empty_world() -> World {
    World::from_json(serde_json::json!([])).expect("an empty world fits any schema")
}

/// **The engine's answer** to `cell` under `input`: [`decide::decide`] over the loaded policy.
pub fn engine(input: &PolicyInput, cell: &Cell) -> Answer {
    let decision = decide::decide(
        &input.loaded(),
        &decide::Request {
            chat: CHAT,
            of: no_chat(),
            action: cell.action,
            resource: cell.resource,
            context: cell.context.clone(),
        },
        &empty_world(),
    );
    Answer {
        outcome: decision.outcome,
        deciding: decision.deciding.into_iter().map(|d| d.id).collect(),
        after_mode: decision.after_mode,
    }
}

/// **How an observed answer differs from what the oracle expected.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// The harness blocked, with no rule to name, and the cell declares no gap.
    BlockWithNoRule,
    /// Enforcement is looser than the policy: an escape, a public p0 (Q317.7).
    Looser,
    /// Enforcement is stricter than the policy: an ordinary bug.
    Stricter,
    /// The same outcome decided by other rules.
    OtherRule,
}

/// **Equal, or why not** (Q317.3): the same outcome, the same deciding `@id`s and the same
/// outcome after the mode. A block with no rule is a mismatch unless the cell declares its gap.
pub fn compare(expected: &Answer, observed: &Observed, gap: Option<&str>) -> Result<(), Mismatch> {
    if expected == observed {
        return Ok(());
    }
    if observed.deciding.is_empty() && observed.after_mode == Outcome::Deny {
        return match gap {
            Some(_) => Ok(()),
            None => Err(Mismatch::BlockWithNoRule),
        };
    }
    // Outcome orders allow < ask < deny.
    match observed.after_mode.cmp(&expected.after_mode) {
        std::cmp::Ordering::Less => Err(Mismatch::Looser),
        std::cmp::Ordering::Greater => Err(Mismatch::Stricter),
        std::cmp::Ordering::Equal => Err(Mismatch::OtherRule),
    }
}

/// **An equality cell**: the oracle's expectation against the engine's answer.
pub fn equality(batch: &Batch, cell: &Cell) -> Result<(), Mismatch> {
    let input = batch.policy_of(cell);
    compare(&oracle(&input, cell), &engine(&input, cell), cell.gap)
}
