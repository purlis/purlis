// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **This chat's `[policy]`, as the app answers it** (rulings Q308, Q308a; the review of #6992).
//!
//! Only the app can verify what the person accepted, so a hook, a git hook or `purlis secret
//! exec` asks it over the hook socket ([`Ask::Policy`]) the first time a family's mode is
//! needed, and runs every family enforced where the app does not answer
//! ([`purlis_core::enforcement::in_force`]). A log line goes to the app too ([`Ask::PolicyLog`]):
//! a process inside the chat's wrap cannot write the log, and one outside it is not trusted to.

use std::time::Duration;

use purlis_core::enforcement::{self, Family, Resolved};
use purlis_core::hookwire::{Answer, Ask, PolicyAsk, PolicyLogAsk};

/// How long a process waits for the app's answer: the app answers from what it holds.
const ANSWERED_WITHIN: Duration = Duration::from_millis(500);

/// The app's answer about this chat's policy, or none: then every family is enforced.
fn asked() -> Option<Resolved> {
    match crate::brokered::asked(|chat| Ask::Policy(PolicyAsk { chat }), ANSWERED_WITHIN) {
        // The cause of an enforced family (ADR 0130 §7) changes only the words of its refusal.
        Ok(Ok(Answer::Policy {
            spelled,
            unaccepted,
        })) => Some(Resolved::parse(&spelled).with_unaccepted(&unaccepted)),
        _ => None,
    }
}

/// One log line handed to the app; dropped where nobody takes it.
fn logged(family: Family, rule: &str, host: Option<&str>) {
    let _ = crate::brokered::asked(
        |chat| {
            Ask::PolicyLog(PolicyLogAsk {
                chat,
                family: family.key().to_owned(),
                rule: rule.to_owned(),
                host: host.map(str::to_owned),
            })
        },
        ANSWERED_WITHIN,
    );
}

/// **Asks the app, and logs through it**, for every command of this process. First thing in
/// `main`: nothing is asked until a family's mode is needed.
pub fn from_the_app() {
    enforcement::ask_with(asked);
    enforcement::log_to(Box::new(logged));
}
