// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What each client scope may do: one table, checked once (FD-27, ADR 0068 §5 as amended by
//! FD-27).
//!
//! [`crate::session::serve`] asks [`Scope::may`] about every command before any
//! [`crate::session::Host`] hears of it, and the UI RPC's hello asks it about [`Power::UiRpc`],
//! so no host can forget the check, and a host never sees a command its link may not send.
//! A command a scope is not granted is refused `not_allowed`, and the link carries on.
//!
//! | Power | `local-ui` | `terminal` | `fleet-mcp` | `approval` | `editor` | `remote-link` | `remote-device` |
//! |---|---|---|---|---|---|---|---|
//! | `list` | yes | yes | yes | yes | | yes | yes |
//! | `attach`, `detach` (a view of a terminal) | yes | yes | | | | yes | yes |
//! | `attach_timeline`, `detach_timeline` | yes | | | | | | yes |
//! | `write` | yes | yes | | | | yes | yes |
//! | `prompt` | yes | | | | | | yes |
//! | `resize` | yes | yes | | | | yes | |
//! | `answer` an ask, with a choice for once or this session | yes | | | yes | | | yes |
//! | `answer` with a choice that lasts (writes a rule) | yes | | | | | | |
//! | `answer` an ask that elicits a secret | yes | | | | | | |
//! | `stop` | yes | yes | yes | yes | | yes | yes |
//! | `start` | yes | yes | | | | yes | |
//! | `subscribe`, `unsubscribe` | yes | yes | yes | yes | | yes | yes |
//! | the UI RPC: vault values, settings writes, the app's own commands | yes | | | | | | |
//!
//! **Why each row is what it is.**
//!
//! - **Vault values and settings writes are `local-ui`'s alone** (V7). The session protocol has
//!   no command that reads a value or writes a setting; both are the app's commands, which ride
//!   only the UI RPC, and the UI RPC opens for `local-ui` only.
//! - **Only a human scope answers an ask, and a secret's ask only in the window** (V16, V75):
//!   `local-ui` and `approval`. Whether an ask elicits a secret is the host's to say
//!   ([`crate::session::Host::elicits_a_secret`]), and a host that does not say is taken to
//!   hold one, so `approval` is refused rather than let through.
//! - **A stop needs no human** (ADR 0078 §4, ADR 0071): it only removes power. Every scope may
//!   send one but `editor`, which can make no agent act or stop (ADR 0081).
//! - **`fleet-mcp` lists, watches events and stops.** Its clients are outside agents (HP-20),
//!   and an agent never holds a human power (V16): it does not type into, start, resize, view
//!   or answer a chat. A view of a terminal is left out until HP-20 asks for it.
//! - **`approval` answers, lists, watches and stops.** It is `charter inbox`: it never drives a
//!   chat.
//! - **`editor` has none of these commands.** Its four (`resolve`, `reveal`, `place`,
//!   `changes`) are ED-2's, and are its alone (ADR 0081).
//! - **`terminal` and `remote-link` are the session protocol without answers, pending a
//!   ruling.** ADR 0078 lets `terminal` answer and a runner's `remote-link` carry an answer a
//!   desktop human scope sent; V75, ruled later, names only `local-ui` and `approval` as
//!   answerers. Both are refused until the operator rules, which fails closed.
//! - **`remote-device` is the operator's own phone** (iOS design §4.3): it continues the work
//!   (list, view, timeline, write, prompt, answer, stop) and nothing that shapes the desk. It
//!   never resizes (a chat's terminal is one terminal, §5.4), never starts a chat (phase 4), and
//!   never makes a choice that lasts (D8): whether an answer's choice lasts is the host's to say
//!   ([`crate::session::Host::choice_lasts`]), and a host that does not say is taken to mean it
//!   does, so the refusal fails closed, as a secret's does.

use crate::auth::Scope;

/// One thing a scope may be granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Power {
    /// `list`.
    List,
    /// `attach` and `detach`: a view of a chat's terminal.
    View,
    /// `attach_timeline` and `detach_timeline`: a chat's timeline ([`crate::timeline`]).
    Timeline,
    /// `write`: bytes typed into a chat.
    Write,
    /// `prompt`: a turn's worth of text handed to the agent, not typed byte by byte.
    Prompt,
    /// `resize`.
    Resize,
    /// `answer` an ask with a choice for once or this session, eliciting no secret.
    Answer,
    /// `answer` with a choice that lasts beyond the session: one that writes a rule.
    AnswerAlways,
    /// `answer` an ask that elicits a secret.
    AnswerASecret,
    /// `stop`.
    Stop,
    /// `start`.
    Start,
    /// `subscribe` and `unsubscribe`.
    Subscribe,
    /// The UI RPC: the app's own commands, vault values and settings writes among them.
    UiRpc,
}

impl Scope {
    /// Whether this scope is granted `power`: the table above, and nothing else.
    pub fn may(self, power: Power) -> bool {
        use Power as P;
        use Scope as S;
        match self {
            S::LocalUi => true,
            S::Terminal | S::RemoteLink => matches!(
                power,
                P::List | P::View | P::Write | P::Resize | P::Stop | P::Start | P::Subscribe
            ),
            S::FleetMcp => matches!(power, P::List | P::Stop | P::Subscribe),
            S::Approval => matches!(power, P::List | P::Answer | P::Stop | P::Subscribe),
            S::Editor => false,
            S::RemoteDevice => matches!(
                power,
                P::List
                    | P::View
                    | P::Timeline
                    | P::Write
                    | P::Prompt
                    | P::Answer
                    | P::Stop
                    | P::Subscribe
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_ui_reaches_the_ui_rpc_or_answers_a_secrets_ask() {
        for scope in Scope::ALL {
            assert_eq!(scope.may(Power::UiRpc), scope == Scope::LocalUi, "{scope}");
            assert_eq!(
                scope.may(Power::AnswerASecret),
                scope == Scope::LocalUi,
                "{scope}"
            );
        }
    }

    #[test]
    fn only_a_human_scope_answers() {
        let answer: Vec<Scope> = Scope::ALL
            .into_iter()
            .filter(|scope| scope.may(Power::Answer))
            .collect();
        assert_eq!(
            answer,
            [Scope::LocalUi, Scope::Approval, Scope::RemoteDevice]
        );
    }

    #[test]
    fn a_choice_that_lasts_is_the_windows_alone() {
        for scope in Scope::ALL {
            assert_eq!(
                scope.may(Power::AnswerAlways),
                scope == Scope::LocalUi,
                "{scope}"
            );
        }
    }

    #[test]
    fn remote_device_steers_a_chat_but_never_resizes_or_starts_one() {
        let s = Scope::RemoteDevice;
        for granted in [
            Power::List,
            Power::View,
            Power::Timeline,
            Power::Write,
            Power::Prompt,
            Power::Answer,
            Power::Stop,
            Power::Subscribe,
        ] {
            assert!(s.may(granted), "{granted:?}");
        }
        for refused in [
            Power::Resize,
            Power::Start,
            Power::AnswerAlways,
            Power::AnswerASecret,
            Power::UiRpc,
        ] {
            assert!(!s.may(refused), "{refused:?}");
        }
    }

    #[test]
    fn the_new_powers_are_the_windows_and_remote_devices_only() {
        for scope in Scope::ALL {
            let new = matches!(scope, Scope::LocalUi | Scope::RemoteDevice);
            assert_eq!(scope.may(Power::Timeline), new, "{scope} timeline");
            assert_eq!(scope.may(Power::Prompt), new, "{scope} prompt");
        }
    }
}
