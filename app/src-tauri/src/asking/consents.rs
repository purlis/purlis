// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Consents and command confirmations are Blocking asks** (#6000, ADR 0121 decision 2): what
//! stops a chat, or a `purlis` command, until the person answers is listed in Asks, counted on
//! the ✋ and notified, whatever drew it before.
//!
//! - **What a project expects and this machine has not allowed** ([`sandbox_expects`]): the
//!   sandbox consent ("Allow on this machine"), without which a sandboxed chat of the project is
//!   refused what it expects. While it can be answered it is in Asks alone (R-205): its Notice
//!   draws nothing beside it, and both read the one state (`sandbox::services::awaiting`), so
//!   its answer settles it everywhere. Where policy locks it, or nothing listed can be allowed
//!   here, there is nothing to decide: it is no ask, and its Notice alone says so.
//! - **A `purlis` command's confirmation in the window** ([`confirmation`]): a pool sign-in, a
//!   profile taken off a pool, a signing check. It is answered on its row alone, with the code
//!   its command's terminal shows, and no dialog over the window (#6136); its two minutes still
//!   run in the core (`poolsignin_confirm`). It is listed under the one project it was filed
//!   under, so one confirmation is counted once.

use super::{AnswerPath, AskSource, Offered, Shown, said_inertly};
use crate::poolsignin_confirm::{Asks, PoolSignInAsk};
use crate::sandboxing::{ExpectedItem, GrantWhat, ProjectExpects};

/// What the expects consent's Allow says, here and on Settings › Sandbox
/// (`ProjectExpectsNotice.tsx`).
pub const ALLOW_EXPECTS: &str = "Allow on this machine";
/// The option id of the expects consent's Allow on this machine.
pub const ALLOW: &str = "allow";
/// The option id of its Not now.
pub const NOT_NOW: &str = "not-now";
/// The option id of a command confirmation's yes.
pub const YES: &str = "yes";
/// The option id of a command confirmation's no.
pub const NO: &str = "no";

fn offered(id: &str, label: &str, allows: bool) -> Offered {
    Offered {
        id: id.to_owned(),
        label: label.to_owned(),
        allows,
        preselected: None,
        scope: None,
    }
}

/// A row of no chat's, from `chain`, saying `says`, offering `options` on `answer`.
fn of_no_chat(
    ask: String,
    chain: &str,
    says: &str,
    options: Vec<Offered>,
    source: AskSource,
    answer: AnswerPath,
) -> Shown {
    Shown {
        session: 0,
        ask,
        says: said_inertly(says),
        options,
        source,
        chain: vec![said_inertly(chain)],
        answer,
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    }
}

/// How an expected item is named on the row: as an Allow would keep it.
fn named(one: &ExpectedItem) -> String {
    let kind = if one.what == GrantWhat::Socket {
        "the local socket"
    } else {
        "the system service"
    };
    format!("{kind} {}", one.named)
}

/// **The expects consent, as a Blocking ask** (#6000): what the project at hand expects and this
/// machine has not allowed, while the person is still to be asked and something listed can be
/// allowed here: Allow on this machine, Not now, answered on its row alone (R-205). None otherwise.
pub fn sandbox_expects(expects: Option<&ProjectExpects>) -> Option<Shown> {
    let expects = expects?;
    let allowable = expects.items.iter().any(|one| one.refused.is_none());
    if !expects.asked || expects.locked.is_some() || !allowable {
        return None;
    }
    let listed: Vec<String> = expects.items.iter().map(named).collect();
    let says = format!(
        "This project expects more than this machine allows: {}",
        listed.join(", ")
    );
    Some(of_no_chat(
        "sandbox-expects".to_owned(),
        "purlis",
        &says,
        vec![
            offered(ALLOW, ALLOW_EXPECTS, true),
            offered(NOT_NOW, "Not now", false),
        ],
        AskSource::SandboxExpects,
        AnswerPath::SandboxExpects {
            items: expects.items.clone(),
        },
    ))
}

/// **A command's confirmation in the window, as a Blocking ask** (#6000): its question, with the
/// code its command's terminal shows, its yes and its no, answered on its row
/// (`answer_pool_sign_in`, #6136).
/// `until` is when it is taken back as a no: the window reads the list again then.
pub fn confirmation(asked: &PoolSignInAsk, until: u32) -> Shown {
    let (command, says, yes) = match asked.asks {
        Asks::SignIn => (
            "purlis profile sign-in",
            format!(
                "Asks to sign {} in on pool {}. Its terminal shows the code {}.",
                asked.profile, asked.pool, asked.code
            ),
            "Sign in",
        ),
        Asks::TakeOff => (
            "purlis pool remove-profile",
            format!(
                "Asks to take {} off pool {}. Its terminal shows the code {}.",
                asked.profile, asked.pool, asked.code
            ),
            "Take it off",
        ),
        Asks::SigningCheck => (
            "purlis signing check",
            format!(
                "Asks to check {} in {} on {}. Its terminal shows the code {}.",
                asked.pool,
                asked.profile,
                asked.on.as_deref().unwrap_or("GitHub"),
                asked.code
            ),
            "Check it",
        ),
    };
    let mut shown = of_no_chat(
        format!("confirmation:{}", asked.id),
        command,
        &says,
        vec![offered(YES, yes, true), offered(NO, "Cancel", false)],
        AskSource::Confirmation,
        AnswerPath::Confirmation { id: asked.id },
    );
    shown.held_until = Some(until);
    // It is taken back as a no at `until`: an Expiring ask (ADR 0121 decision 5, #6006).
    shown.expires = Some(until);
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(refused: Option<&str>) -> ExpectedItem {
        ExpectedItem {
            what: GrantWhat::Service,
            target: "com.example.helper".to_owned(),
            named: "com.example.helper".to_owned(),
            says: "It is a service on this machine.".to_owned(),
            refused: refused.map(str::to_owned),
        }
    }

    fn expects(asked: bool, locked: Option<&str>, items: Vec<ExpectedItem>) -> ProjectExpects {
        ProjectExpects {
            items,
            locked: locked.map(str::to_owned),
            asked,
        }
    }

    fn ids(shown: &Shown) -> Vec<&str> {
        shown.options.iter().map(|one| one.id.as_str()).collect()
    }

    /// **#6000: a consent the person is still to give is a Blocking ask of no chat's**, answered
    /// with its Notice's own words, and sent back the items it drew.
    #[test]
    fn a_consent_still_to_give_is_an_ask_of_no_chat_with_its_notices_answers() {
        let waiting = expects(true, None, vec![item(None)]);
        let shown = sandbox_expects(Some(&waiting)).expect("it is an ask");
        assert_eq!(shown.session, 0);
        assert_eq!(shown.source, AskSource::SandboxExpects);
        assert_eq!(shown.chain, ["purlis"]);
        assert_eq!(ids(&shown), [ALLOW, NOT_NOW]);
        assert_eq!(shown.options[0].label, "Allow on this machine");
        assert!(shown.options[0].allows);
        assert!(
            shown.says.contains("the system service com.example.helper"),
            "{}",
            shown.says
        );
        assert_eq!(
            shown.answer,
            AnswerPath::SandboxExpects {
                items: waiting.items.clone()
            }
        );
    }

    /// **A decision with nothing to decide stays a Notice** (ADR 0121 decision 2): answered
    /// already, locked by policy, nothing that can be allowed here, or nothing declared.
    #[test]
    fn a_consent_answered_locked_or_with_nothing_to_allow_is_no_ask() {
        assert_eq!(sandbox_expects(None), None);
        assert_eq!(
            sandbox_expects(Some(&expects(false, None, vec![item(None)]))),
            None
        );
        assert_eq!(
            sandbox_expects(Some(&expects(true, Some("Locked by IT"), vec![item(None)]))),
            None
        );
        assert_eq!(
            sandbox_expects(Some(&expects(true, None, vec![item(Some("not here"))]))),
            None
        );
        assert_eq!(sandbox_expects(Some(&expects(true, None, vec![]))), None);
    }

    fn asked(asks: Asks) -> PoolSignInAsk {
        PoolSignInAsk {
            id: 7,
            asks,
            profile: "work".to_owned(),
            pool: "team".to_owned(),
            code: "A1B2C3".to_owned(),
            on: None,
        }
    }

    /// **#6000: a command's confirmation is a Blocking ask**, named by its command, showing its
    /// code, answered on its dialog's path, and read again when its two minutes end.
    #[test]
    fn a_commands_confirmation_is_an_ask_named_by_its_command_until_it_is_taken_back() {
        for (asks, command, yes) in [
            (Asks::SignIn, "purlis profile sign-in", "Sign in"),
            (Asks::TakeOff, "purlis pool remove-profile", "Take it off"),
            (Asks::SigningCheck, "purlis signing check", "Check it"),
        ] {
            let shown = confirmation(&asked(asks), 1_000);
            assert_eq!(shown.session, 0);
            assert_eq!(shown.source, AskSource::Confirmation);
            assert_eq!(shown.chain, [command]);
            assert_eq!(ids(&shown), [YES, NO]);
            assert_eq!(shown.options[0].label, yes);
            assert!(shown.says.contains("A1B2C3"), "{}", shown.says);
            assert_eq!(shown.answer, AnswerPath::Confirmation { id: 7 });
            assert_eq!(shown.held_until, Some(1_000));
            assert_eq!(shown.ask, "confirmation:7");
        }
    }

    /// **#6006: a confirmation is an Expiring ask**, ending when its two minutes do, while the
    /// expects consent waits for its answer and ends at no time of its own.
    #[test]
    fn a_confirmation_expires_when_it_is_taken_back_and_the_expects_consent_never_does() {
        assert_eq!(
            confirmation(&asked(Asks::SignIn), 1_000).expires,
            Some(1_000)
        );
        let consent = sandbox_expects(Some(&expects(true, None, vec![item(None)])))
            .expect("a consent to give");
        assert_eq!(consent.expires, None);
        assert_eq!(consent.blocks, None);
    }
}
