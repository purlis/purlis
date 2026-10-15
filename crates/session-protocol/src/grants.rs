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
//! | `hold_project`, `let_go_of_project` (1.2) | yes | | | | | |
//! | `stop_all` (1.5) | yes | yes | yes | yes | | yes |
//! | `kill_switch`, the switch's state (1.5) | yes | yes | yes | yes | | yes |
//! | `rearm` (1.5) | yes | | | | | |
//! | a start's `launch`, the window's own composed start (1.2) | yes | | | | | |
//! | `deal`, `dealt_already` (1.3) | yes | | | | | |
//! | `reply_heard`, and the `heard` lines pushed (1.3) | yes | | | | | |
//! | `confine`, `let_go_of_confinement`, `change_reach`, `reach_of`, and the pushed `sandbox` (1.4) | yes | | | | | |
//! | `start_program` of kind `browser`, `attach_view` of kind `browser` (1.6) | yes | | | | | |
//! | `record`, a write in the event log the host holds (1.8) | yes | | | | | |
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
//!   with the row it is like.
//! - **A chat's confinement is the window's alone** (1.4). The window plans a chat's proxy from
//!   the sandbox it compiled, answers its live asks and withdraws a port from it; only the
//!   window hears what a proxy refused, carried and asks about. Every answer to a sandbox ask
//!   is the window's, as it is where the proxy runs in the window.
//! - **Holding a project is the window's alone** (1.2): a hold says where the project is on the
//!   host's machine, and only the window, whose projects they are, says so or lets go.
//! - **A project's numbers are dealt for the window alone** (1.3), as its holds are: `deal`
//!   and `dealt_already` go with the `hold_project` row.
//! - **A held project's hook lines are heard and answered by the window alone** (1.3). The host
//!   pushes each line only to the links that hold the project, which only `local-ui` may, and
//!   only `local-ui` replies: a reply can carry an ask's answer, which no other scope gives here.
//! - **A browser is started and watched in the window alone** (1.6, #2012). A browser is a
//!   program started as the person, and what it shows is theirs: the pages of sites they are
//!   signed in to. Until runners run browsers (spec #2041), no other scope starts one or attaches
//!   a view of one, `remote-link` included; a stop of one, like every stop, only removes power.
//! - **Only a human scope answers an ask, and a secret's ask only in the window** (V16, V75):
//!   `local-ui` and `approval`. Whether an ask elicits a secret is the host's to say
//!   ([`crate::session::Host::elicits_a_secret`]), and a host that does not say is taken to
//!   hold one, so `approval` is refused rather than let through.
//! - **A stop needs no human** (ADR 0078 §4, ADR 0071): it only removes power. Every scope may
//!   send one but `editor`, which can make no agent act or stop (ADR 0081). `stop_all`, the
//!   kill switch, is a stop like any other, and its state a read like `list`.
//! - **Only the window writes in the event log through the host** (1.8). What it records is
//!   what the window decided (a run begun, a person's grant), and the log is the audit's source
//!   (ADR 0075): a scope that could write in it could record a person's act no person made.
//! - **Only the window re-arms** (ADR 0071, 1.5). Any agent can run a command, so a re-arm any
//!   other scope could send would put a person's power in every agent's hands. A `rearm` from
//!   another scope is refused in words saying where a re-arm is made.
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
    /// `hold_project` and `let_go_of_project` (1.2): where a project is on the host's machine.
    Hold,
    /// A start's `launch` (1.2): a program, its words and its environment as the window composed
    /// them.
    Launch,
    /// `reply_heard` (1.3): the window's reply to a line a held project's hooks wrote.
    Heard,
    /// A chat's confinement (1.4): `confine`, `let_go_of_confinement`, `change_reach` and
    /// `reach_of`, and hearing the pushed `sandbox`.
    Confine,
    /// `rearm` (1.5): lets programs start again after the kill switch.
    Rearm,
    /// `start_program` and `attach_view` of the kind `browser` (1.6): a browser started as the
    /// person, and what it shows.
    Browser,
    /// `record` (1.8): a write in the event log the host holds.
    Record,
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
    fn only_the_window_hands_the_host_a_composed_launch_or_holds_a_project() {
        for scope in Scope::ALL {
            assert_eq!(scope.may(Power::Rearm), scope == Scope::LocalUi, "{scope}");
            assert_eq!(scope.may(Power::Launch), scope == Scope::LocalUi, "{scope}");
            assert_eq!(scope.may(Power::Hold), scope == Scope::LocalUi, "{scope}");
        }
    }

    #[test]
    fn only_the_window_replies_to_what_a_projects_hooks_wrote() {
        for scope in Scope::ALL {
            assert_eq!(scope.may(Power::Heard), scope == Scope::LocalUi, "{scope}");
        }
    }

    #[test]
    fn only_the_window_confines_a_chat_and_answers_its_sandbox() {
        for scope in Scope::ALL {
            assert_eq!(
                scope.may(Power::Confine),
                scope == Scope::LocalUi,
                "{scope}"
            );
        }
    }

    #[test]
    fn only_the_window_starts_or_watches_a_browser() {
        for scope in Scope::ALL {
            assert_eq!(
                scope.may(Power::Browser),
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
        assert_eq!(answer, [Scope::LocalUi, Scope::Approval]);
    }
}
