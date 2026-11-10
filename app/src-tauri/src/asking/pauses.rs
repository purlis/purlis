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
    }
}

/// The ask's key: one per paused chat.
pub fn key(session: u32) -> String {
    format!("paused:{session}")
}

/// **Chat `session`, paused for `paused`, as a Blocking ask**, named by `chain`.
pub fn paused(session: u32, paused: &Paused, chain: Vec<String>) -> Shown {
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
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        secret: None,
        run: None,
        expires: None,
        blocks: None,
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
        let shown = paused(7, &at, vec!["steward 2".into(), "#7 drill".into()]);
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
        let shown = paused(3, &at, vec!["steward 3".into()]);
        assert_eq!(shown.source, AskSource::Looping);
        assert_eq!(ids(&shown), [FINE, STOP]);
        assert_eq!(labels(&shown), ["It's fine, continue", "Stop"]);
        assert_eq!(
            shown.says,
            "It may be looping: the same command failed 6 times in a row."
        );
    }
}
