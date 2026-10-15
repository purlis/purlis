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
//! | Power | `local-ui` | `terminal` | `fleet-mcp` | `approval` | `editor` | `remote-link` |
//! |---|---|---|---|---|---|---|
//! | `list` | yes | yes | yes | yes | | yes |
//! | `attach_view`, `detach_view` (a view of a hosted program), and 1.0's `attach`, `detach` | yes | yes | | | | yes |
//! | `write` | yes | yes | | | | yes |
//! | `resize` | yes | yes | | | | yes |
//! | `answer` an ask | yes | | | yes | | |
//! | `answer` an ask that elicits a secret | yes | | | | | |
//! | `stop` | yes | yes | yes | yes | | yes |
//! | `start_program`, and 1.0's `start` | yes | yes | | | | yes |
//! | `subscribe`, `unsubscribe` | yes | yes | yes | yes | | yes |
//! | `close` (1.2) | yes | yes | yes | yes | | yes |
//! | `readiness` (1.2) | yes | yes | yes | yes | | yes |
//! | `hold_project`, `let_go_of_project` (1.2) | yes | yes | | | | yes |
//! | a start's `launch`, the window's own composed start (1.2) | yes | | | | | |
//! | the UI RPC: vault values, settings writes, the app's own commands | yes | | | | | |
//!
//! **Why each row is what it is.**
//!
//! - **Vault values and settings writes are `local-ui`'s alone** (V7). The session protocol has
//!   no command that reads a value or writes a setting; both are the app's commands, which ride
//!   only the UI RPC, and the UI RPC opens for `local-ui` only.
//! - **A start's `launch` is the window's alone** (1.2). It is what the window composed above
//!   its seam, the program, its words and its environment, while what composes a chat's start
//!   still lives in the window (#1998). No other scope hands the host a program and an
//!   environment to run; each starts by persona, profile and harness, which the host composes.
//! - **`close` is a stop that also forgets**, and `readiness` a read like `list`: each goes
//!   with the row it is like. Holding a project goes with starting in it.
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

use crate::auth::Scope;

/// One thing a scope may be granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Power {
    /// `list`.
    List,
    /// `attach_view` and `detach_view` (and 1.0's `attach` and `detach`): a view of a hosted
    /// program, of any kind.
    View,
    /// `write`: bytes typed into a chat.
    Write,
    /// `resize`.
    Resize,
    /// `answer` an ask that elicits no secret.
    Answer,
    /// `answer` an ask that elicits a secret.
    AnswerASecret,
    /// `stop`.
    Stop,
    /// `start_program` (and 1.0's `start`): a hosted program of any kind.
    Start,
    /// `subscribe` and `unsubscribe`.
    Subscribe,
    /// The UI RPC: the app's own commands, vault values and settings writes among them.
    UiRpc,
    /// A start's `launch` (1.2): a program, its words and its environment as the window composed
    /// them.
    Launch,
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
    fn only_the_window_hands_the_host_a_composed_launch() {
        for scope in Scope::ALL {
            assert_eq!(scope.may(Power::Launch), scope == Scope::LocalUi, "{scope}");
        }
    }

    #[test]
    fn only_a_human_scope_answers() {
        let answer: Vec<Scope> = Scope::ALL
            .into_iter()
            .filter(|scope| scope.may(Power::Answer))
            .collect();
        assert_eq!(answer, [Scope::LocalUi, Scope::Approval]);
    }
}
