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
//! - **A persona whose hosts or tools wait for an Allow on this machine** ([`persona_allows`]):
//!   its chats reach none of its hosts and run none of its tools until the person allows them,
//!   as they were shown, and a list that changed since an Allow waits again. One ask per persona,
//!   however many of its chats are refused. Its Notice (`PersonaHostsNotice.tsx`) stays and is
//!   drawn from this same ask in the window, so one answer settles both. Not now puts it away
//!   until the project is opened again or the list changes ([`Later`]); nothing is kept.

use super::{AnswerPath, AskSource, Offered, Shown, said_inertly};
use crate::poolsignin_confirm::{Asks, PoolSignInAsk};
use crate::sandboxing::{ExpectedItem, GrantWhat, PersonaHosts, ProjectExpects};
use std::collections::HashMap;
use std::sync::Mutex;

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

/// **The personas the person said Not now to, in a project while it is open** (#1362's Not now,
/// as an ask): each by the digest of the list it was said to, so a list that changes asks again.
/// Held in memory with the project, as the Notice's Not now always was: nothing is kept, and
/// opening the project again asks again.
#[derive(Debug, Default)]
pub struct Later {
    said: Mutex<HashMap<String, String>>,
}

impl Later {
    /// Not now, to `persona`'s list as it was shown, `digest`.
    pub fn say(&self, persona: &str, digest: &str) {
        self.said
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(persona.to_owned(), digest.to_owned());
    }

    /// Whether Not now was said to `persona`'s list as it stands, `digest`.
    pub fn holds(&self, persona: &str, digest: &str) -> bool {
        self.said
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(persona)
            .is_some_and(|said| said == digest)
    }
}

/// The option id of a persona's Allow.
pub const ALLOW_PERSONA: &str = "allow";

/// "a", "a and b", "a, b and c".
fn listed(items: &[String], and: &str) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{}{and}{last}", rest.join(", ")),
    }
}

/// **A persona whose hosts or tools wait for an Allow, as a Blocking ask** (ADR 0121 decision 2,
/// amended 2026-11-12): what `waitingPersonas` in `PersonaHostsNotice.tsx` draws a Notice for,
/// the same rule. Not allowed as it stands, and with hosts a chat would reach (none where policy
/// forbids persona hosts) or tools. Its Allow is the Notice's (`allow_persona_hosts`, sent the
/// digest of what the row showed); Not now is [`Later`]. Its risk is that of the first of its
/// hosts that looks like production (decision 8), so its Allow asks once more.
pub fn persona_allows(
    personas: &[PersonaHosts],
    hosts_locked: bool,
    later: &Later,
    risk: &purlis_core::askrisk::Facts,
) -> Vec<Shown> {
    personas
        .iter()
        .filter(|one| {
            !one.allowed
                && ((!hosts_locked && !one.reached.is_empty()) || !one.tools.is_empty())
                && !later.holds(&one.persona, &one.digest)
        })
        .map(|one| persona_allow(one, hosts_locked, risk))
        .collect()
}

/// One persona's ask ([`persona_allows`]).
fn persona_allow(
    one: &PersonaHosts,
    hosts_locked: bool,
    risk: &purlis_core::askrisk::Facts,
) -> Shown {
    let hosts: &[String] = if hosts_locked { &[] } else { &one.reached };
    let what = match (hosts.is_empty(), one.tools.is_empty()) {
        (false, false) => "hosts and tools",
        (true, false) => "tools",
        _ => "hosts",
    };
    let who = if one.default {
        format!("chats as {}, and chats that name no persona,", one.persona)
    } else {
        format!("chats as {}", one.persona)
    };
    let lets = [
        (!hosts.is_empty()).then(|| format!("reach {}", listed(hosts, " and "))),
        (!one.tools.is_empty()).then(|| format!("run {}", listed(&one.tools, "; and "))),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", and ");
    let said = if one.waiting {
        format!(
            "You allowed {}'s {what} on this machine and they have changed since, so no chat gets \
             them until you allow them again. Allowed, {who} {lets}.",
            one.persona
        )
    } else {
        format!(
            "{}'s {what} wait for your Allow on this machine: until then {who} get none of them. \
             Allowed, they {lets}.",
            one.persona
        )
    };
    let allow = if one.default {
        format!(
            "Allow for {} chats and chats that name no persona",
            one.persona
        )
    } else {
        format!("Allow for {} chats", one.persona)
    };
    let mut shown = of_no_chat(
        format!("persona-allow:{}", one.persona),
        &one.persona,
        &said,
        vec![
            offered(ALLOW_PERSONA, &allow, true),
            offered(NOT_NOW, "Not now", false),
        ],
        AskSource::PersonaAllow,
        AnswerPath::PersonaAllow {
            persona: one.persona.clone(),
            digest: one.digest.clone(),
        },
    );
    shown.risk = hosts
        .iter()
        .find_map(|host| risk.of_host(host))
        .map(Into::into);
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

    fn persona(name: &str, hosts: &[&str], tools: &[&str]) -> PersonaHosts {
        PersonaHosts {
            persona: name.to_owned(),
            hosts: hosts.iter().map(|one| (*one).to_owned()).collect(),
            reached: hosts.iter().map(|one| (*one).to_owned()).collect(),
            tools: tools.iter().map(|one| (*one).to_owned()).collect(),
            digest: format!("{name}-digest"),
            allowed: false,
            default: false,
            waiting: false,
            cautions: None,
            refused: None,
        }
    }

    /// **A persona whose list changed since its Allow is a Blocking ask** (ADR 0121, amended
    /// 2026-11-12): the person's report of 2026-11-12, steward's tools `changed`, waited as a
    /// Notice no one found while steward's chats were refused their tool. One ask for the
    /// persona, of no chat's, answered with its Notice's Allow and the digest it showed.
    #[test]
    fn a_persona_whose_tools_changed_since_its_allow_is_one_ask_with_its_notices_allow() {
        let mut steward = persona(
            "steward",
            &[],
            &["gh with token from vault devops as GH_TOKEN"],
        );
        steward.waiting = true;
        let asks = persona_allows(
            &[steward],
            false,
            &Later::default(),
            &purlis_core::askrisk::Facts::default(),
        );
        let [shown] = asks.as_slice() else {
            panic!("one ask for the persona: {asks:?}");
        };
        assert_eq!(shown.session, 0);
        assert_eq!(shown.ask, "persona-allow:steward");
        assert_eq!(shown.source, AskSource::PersonaAllow);
        assert_eq!(shown.chain, ["steward"]);
        assert_eq!(ids(shown), [ALLOW_PERSONA, NOT_NOW]);
        assert_eq!(shown.options[0].label, "Allow for steward chats");
        assert!(shown.options[0].allows);
        assert!(shown.says.contains("changed since"), "{}", shown.says);
        assert!(shown.says.contains("run gh with token"), "{}", shown.says);
        assert_eq!(
            shown.answer,
            AnswerPath::PersonaAllow {
                persona: "steward".to_owned(),
                digest: "steward-digest".to_owned(),
            }
        );
        assert_eq!(shown.risk, None);
    }

    /// **Allowed, nothing to allow, or put away with Not now, it is no ask**; a list that
    /// changed after its Not now asks again, and hosts a policy forbids are not asked about.
    #[test]
    fn a_persona_allowed_with_nothing_waiting_or_said_not_now_to_is_no_ask() {
        let facts = purlis_core::askrisk::Facts::default();
        let later = Later::default();
        let mut allowed = persona("scribe", &["api.example.com"], &[]);
        allowed.allowed = true;
        let empty = persona("quiet", &[], &[]);
        let hosts_only = persona("ops", &["api.example.com"], &[]);
        assert!(persona_allows(&[allowed, empty], false, &later, &facts).is_empty());
        assert!(
            persona_allows(std::slice::from_ref(&hosts_only), true, &later, &facts).is_empty(),
            "hosts a policy forbids are not asked about"
        );

        later.say("ops", "ops-digest");
        assert!(
            persona_allows(std::slice::from_ref(&hosts_only), false, &later, &facts).is_empty()
        );
        let mut changed = hosts_only.clone();
        changed.digest = "ops-digest-2".to_owned();
        assert_eq!(persona_allows(&[changed], false, &later, &facts).len(), 1);
    }

    /// **One ask per persona, however many chats wait on it**, and a production host is its
    /// risk (decision 8), so its Allow asks once more.
    #[test]
    fn each_persona_is_one_ask_and_a_production_host_is_its_risk() {
        let asks = persona_allows(
            &[
                persona("ops", &["api.example.com", "db.prod.example.com"], &[]),
                persona("steward", &[], &["kubectl"]),
            ],
            false,
            &Later::default(),
            &purlis_core::askrisk::Facts::default(),
        );
        let keys: Vec<&str> = asks.iter().map(|one| one.ask.as_str()).collect();
        assert_eq!(keys, ["persona-allow:ops", "persona-allow:steward"]);
        let risk = asks[0].risk.as_ref().expect("a production host");
        assert_eq!(risk.class, crate::asking::RiskClass::ProductionHost);
        assert!(risk.why.contains("db.prod.example.com"), "{}", risk.why);
        assert_eq!(asks[1].risk, None);
    }
}
