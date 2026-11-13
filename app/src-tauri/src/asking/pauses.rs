// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Budgets and loops are Blocking asks** (#6015, ADR 0121 decision 20): a chat paused at its
//! budget, or found looping, is listed in Asks, counted on the ✋ and notified, with what it
//! was paused for and the answers that settle it (`crate::pausing`).
//!
//! - **Budget reached**: Continue, Raise the budget, Stop. Raise the budget opens the window's
//!   Settings where the limit is, and the chat goes on once a look finds it within the limit.
//! - **Looping**: It's fine, continue, or Stop.

use super::{AnswerPath, AskSource, Offered, Shown, said_inertly};
use crate::pausing::{Paused, Why};

/// The option id of a budget's Continue.
pub const CONTINUE: &str = "continue";
/// The option id of a budget's Raise the budget, which the window answers by opening Settings.
pub const RAISE: &str = "raise";
/// The option id of a loop's It's fine, continue.
pub const FINE: &str = "fine";
/// The option id of Stop, for either.
pub const STOP: &str = "stop";

fn offered(id: &str, label: &str, allows: bool) -> Offered {
    Offered {
        id: id.to_owned(),
        label: label.to_owned(),
        allows,
        preselected: None,
        // No answer to a pause is a grant (ADR 0121 decision 7): nothing for a rule to be for.
        scope: None,
    }
}

/// The ask's key: one per paused chat.
pub fn key(session: u32) -> String {
    format!("paused:{session}")
}

/// **Chat `session`, paused for `paused`, as a Blocking ask**, named by `chain`.
///
/// Where it stands in Asks (#6006): it never expires, as a pause waits on the person for as long
/// as that takes, but `blocks` chats wait on its report, as on any chat's.
pub fn paused(session: u32, paused: &Paused, chain: Vec<String>, blocks: Option<u32>) -> Shown {
    let (says, options, source) = match paused.why {
        Why::Budget(reached) => (
            format!("Budget reached: {}", reached.say()),
            vec![
                offered(CONTINUE, "Continue", true),
                offered(RAISE, "Raise the budget", true),
                offered(STOP, "Stop", false),
            ],
            AskSource::Budget,
        ),
        Why::Looping(why) => (
            format!("It may be looping: {}.", why.said()),
            vec![
                offered(FINE, "It's fine, continue", true),
                offered(STOP, "Stop", false),
            ],
            AskSource::Looping,
        ),
    };
    Shown {
        session,
        ask: key(session),
        says: said_inertly(&says),
        options,
        source,
        chain,
        answer: AnswerPath::Paused,
        since: Some(paused.since),
        held_until: None,
        expires: None,
        // Nor does it time out (#6007): it waits for the person's answer.
        timed_out: None,
        blocks,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        secret: None,
        run: None,
        // A pause is never offered a rule nor cooled off (ADR 0121 decision 7, #6010): rules
        // are for hosts, reviewed programs and permission rules, and each pause waits on the
        // person's own answer.
        rule: None,
        // Nor does it carry a risk (ADR 0121 decision 8, #6011): going on is not a push, a
        // host or a write outside the project.
        risk: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use purlis_core::dispatchlimits::Reached;

    fn ids(shown: &Shown) -> Vec<&str> {
        shown.options.iter().map(|one| one.id.as_str()).collect()
    }

    fn labels(shown: &Shown) -> Vec<&str> {
        shown.options.iter().map(|one| one.label.as_str()).collect()
    }

    /// **#6015: a chat at its budget asks Continue, Raise the budget, Stop**, saying the figure
    /// and the limit it reached.
    #[test]
    fn a_chat_at_its_budget_asks_continue_raise_the_budget_or_stop() {
        let at = Paused {
            why: Why::Budget(Reached::Time {
                limit: 30,
                worked: 30,
            }),
            since: 1_000,
            held: vec![7],
        };
        let shown = paused(7, &at, vec!["steward 2".into(), "#7 drill".into()], None);
        assert_eq!(shown.session, 7);
        assert_eq!(shown.ask, "paused:7");
        assert_eq!(shown.source, AskSource::Budget);
        assert_eq!(shown.answer, AnswerPath::Paused);
        assert_eq!(ids(&shown), [CONTINUE, RAISE, STOP]);
        assert_eq!(labels(&shown), ["Continue", "Raise the budget", "Stop"]);
        assert!(!shown.options[2].allows);
        assert!(shown.says.starts_with("Budget reached: "), "{}", shown.says);
        assert!(shown.says.contains("30 minutes"), "{}", shown.says);
        assert_eq!(shown.since, Some(1_000));
        assert_eq!(shown.chain, ["steward 2", "#7 drill"]);
    }

    /// **#6015: a chat found looping asks It's fine, continue, or Stop**, saying why.
    #[test]
    fn a_chat_found_looping_asks_its_fine_continue_or_stop() {
        let at = Paused {
            why: Why::Looping(purlis_core::looping::Why::Failing { times: 6 }),
            since: 1_000,
            held: vec![3],
        };
        let shown = paused(3, &at, vec!["steward 3".into()], None);
        assert_eq!(shown.source, AskSource::Looping);
        assert_eq!(ids(&shown), [FINE, STOP]);
        assert_eq!(labels(&shown), ["It's fine, continue", "Stop"]);
        assert_eq!(
            shown.says,
            "It may be looping: the same command failed 6 times in a row."
        );
    }

    fn blocked_loop() -> Paused {
        Paused {
            why: Why::Looping(purlis_core::looping::Why::Blocked { times: 3 }),
            since: 1_000,
            held: vec![3],
        }
    }

    /// **R-216 (T1-36): the block loop's pause names the chat and the count**, the chat being
    /// the last of its chain, the one paused.
    #[test]
    fn the_loop_pause_ask_names_the_chat_and_the_count() {
        let shown = paused(
            3,
            &blocked_loop(),
            vec!["steward 2".into(), "steward 3".into()],
            None,
        );
        assert_eq!(shown.source, AskSource::Looping);
        assert_eq!(shown.answer, AnswerPath::Paused);
        assert_eq!(
            shown.says,
            "steward 3 was blocked 3 times on the same thing. Let it keep trying?"
        );
    }

    /// **R-217 (T1-36): its answers are Tell it to stop trying, the lead, then Let it keep
    /// trying and Stop chat.** Only Let it keep trying lets the chat go on as it was.
    #[test]
    fn the_loop_pause_ask_offers_tell_it_to_stop_trying_first_then_let_it_keep_trying_and_stop_chat()
     {
        let shown = paused(3, &blocked_loop(), vec!["steward 3".into()], None);
        assert_eq!(ids(&shown), [STOP_TRYING, KEEP_TRYING, STOP]);
        assert_eq!(
            labels(&shown),
            ["Tell it to stop trying", "Let it keep trying", "Stop chat"]
        );
        assert_eq!(shown.options[0].preselected, Some(true), "the lead");
        assert!(
            shown.options[1..]
                .iter()
                .all(|one| one.preselected.is_none())
        );
        let allows: Vec<bool> = shown.options.iter().map(|one| one.allows).collect();
        assert_eq!(allows, [false, true, false]);
    }

    /// **#6006: a paused task never expires, and blocks the chats that wait on its report**: it
    /// stands in Asks by how many do, before the asks that only wait.
    #[test]
    fn a_paused_task_never_expires_and_blocks_the_chats_waiting_on_its_report() {
        let at = Paused {
            why: Why::Budget(Reached::Time {
                limit: 30,
                worked: 30,
            }),
            since: 1_000,
            held: vec![7],
        };
        let waited_on = paused(7, &at, vec!["#7 drill".into()], Some(2));
        assert_eq!(waited_on.expires, None);
        assert_eq!(waited_on.blocks, Some(2));
        let alone = paused(7, &at, vec!["#7 drill".into()], None);
        assert_eq!(alone.blocks, None);
    }

    /// **#6011: a pause carries no risk and is offered no rule**: going on past a budget or a
    /// loop is not a push, a host or a write outside the project, so its row never asks twice.
    #[test]
    fn a_pause_carries_no_risk_and_is_offered_no_rule() {
        let at = Paused {
            why: Why::Budget(Reached::Time {
                limit: 30,
                worked: 30,
            }),
            since: 1_000,
            held: vec![7],
        };
        let shown = paused(7, &at, vec!["#7 drill".into()], None);
        assert!(shown.risk.is_none());
        assert!(shown.rule.is_none());
    }
}
