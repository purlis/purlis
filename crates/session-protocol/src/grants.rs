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
//! | `subscribe`, `unsubscribe`, and the pushed `link_down` and `link_back` a relaying host says to every subscriber (1.17) | yes | yes | yes | yes | | yes |
//! | `close` (1.2) | yes | yes | yes | yes | | yes |
//! | `readiness` (1.2) | yes | yes | yes | yes | | yes |
//! | `hold_project`, `let_go_of_project` (1.2) | yes | | | | | relayed |
//! | `stop_all` (1.5) | yes | yes | yes | yes | | yes |
//! | `kill_switch`, the switch's state (1.5) | yes | yes | yes | yes | | yes |
//! | `rearm` (1.5) | yes | | | | | |
//! | a start's `launch`, the window's own composed start (1.2) | yes | | | | | relayed |
//! | `deal`, `dealt_already` (1.3) | yes | | | | | relayed |
//! | `reply_heard`, and the `heard` lines pushed (1.3) | yes | | | | | relayed |
//! | a start's sandbox opt-out (`without_sandbox`) | yes | | | | | |
//! | `confine`, `let_go_of_confinement`, `change_reach`, `reach_of`, and the pushed `sandbox` (1.4); a start's `confinement`, the composed confinement named to its link (1.16) | yes | | | | | relayed |
//! | a start's `launch` that names a harness, the window's composed chat | yes | | | | | |
//! | `start_program` of kind `browser`, `attach_view` of kind `browser` (1.6) | yes | | | | | |
//! | `start_saving`, `stop_saving`, `fetch_project`, `mid_turn`, and the pushed `saving` and `who_is_mid_turn` (1.7) | yes | | | | | |
//! | `record`, a write in the event log the host holds (1.8) | yes | | | | | |
//! | `board_moved`, `board` (1.10) | yes | | | | | |
//! | `browse`, an act of a chat's browser, and a browser's `reach` (1.11) | yes | | | | | |
//! | `programs_of` (1.13) | yes | | | | | |
//! | `quit` (1.13) | yes | | | | | |
//! | `take_over`, `hand_back`, `browser_input`, `size_browser`: the person driving a browser (1.14) | yes | | | | | |
//! | `log_in`, a login purlis performs with a vault's values (1.18) | yes | | | | | |
//! | the UI RPC: vault values, settings writes, the app's own commands | yes | | | | | |
//!
//! **Why each row is what it is.**
//!
//! - **Vault values and settings writes are `local-ui`'s alone** (V7). The session protocol has
//!   no command that reads a value or writes a setting; both are the app's commands, which ride
//!   only the UI RPC, and the UI RPC opens for `local-ui` only.
//! - **A start's `launch` is the window's alone** (1.2). It is what the window composed above
//!   its seam, the program, its words and its environment. No other scope hands the host a
//!   program and an environment to run; each starts by persona, profile and harness, which the
//!   host composes from the project's own files and this machine's profiles, with the code the
//!   window's start runs (#2171).
//! - **`close` is a stop that also forgets**, and `readiness` a read like `list`: each goes
//!   with the row it is like.
//! - **A chat's confinement is the window's alone** (1.4). The window plans a chat's proxy from
//!   the sandbox it compiled, answers its live asks and withdraws a port from it; only the
//!   window hears what a proxy refused, carried and asks about. Every answer to a sandbox ask
//!   is the window's, as it is where the proxy runs in the window. **On a runner it is the
//!   window's as its host relays it** (#2217): the runner's host starts the confinement with
//!   its own binary's proxy, pushes what that proxy does to its peer, and takes its peer's
//!   answers and withdrawn ports, which the desktop's relay sends on for its own `local-ui`
//!   client alone, and it pushes nothing of a sandbox on to any other client.
//!   **This row says which links may confine; each confinement is the links' it is named to**
//!   (#2444): a host acts on a confinement, and pushes what its proxy did, only for a link it
//!   named that confinement to, and every other is one it does not hold (ADR 0068, amended for
//!   #2444). A start's `confinement` (1.16) is in this row: it names the confinement the host
//!   composes for a chat to the link the start came on, so only a link that may confine sends
//!   one, and the desktop's relay sends it on for its window alone (ADR 0068, amended for
//!   #2407).
//! - **A launch that names a harness is the window's for its own host alone** (#2217). It is a
//!   chat the window composed with the sandbox it compiled for its own machine. A runner
//!   composes every harness chat it runs by its names (#2171), with the sandbox it compiles
//!   for itself, so it refuses one from `remote-link`; a relayed launch starts a program that
//!   names no harness, such as a shell.
//! - **Holding a project is the window's alone** (1.2): a hold says where the project is on the
//!   host's machine, and only the window, whose projects they are, says so or lets go.
//! - **A project's numbers are dealt for the window alone** (1.3), as its holds are: `deal`
//!   and `dealt_already` go with the `hold_project` row.
//! - **"Relayed": a runner's host takes the window's own commands from its desktop's host**
//!   (ruled 2026-10-15, #2042; ADR 0078 and ADR 0068 amended). Over the link between hosts,
//!   `remote-link` may hold a project, deal its numbers, start with the window's `launch`,
//!   reply to a held project's hook lines, whose lines it is pushed, and confine a chat (#2217),
//!   whose proxy's notices it is pushed. **"Relayed" is enforced by
//!   the desktop's relay alone** (`purlis_host::Relay`), which sends these on for its own
//!   `local-ui` client only. The runner's host cannot tell a relayed command from one its peer
//!   composed, so a runner trusts its peer as it trusts the window: to compose the programs it
//!   starts and their environment, and to answer its chats' asks. That is why a runner admits
//!   one peer, the desktop that relays to it (`crate::hostlink::answer`). A runner's harness
//!   chats are started by their names, which its host composes (#2171, #2217); the relayed
//!   launch stays for a program that names no harness.
//! - **A held project's hook lines are heard and answered by the window** (1.3), on its own
//!   host or relayed. The host pushes each line to the links that hold the project, which only
//!   `local-ui` and a relayed `remote-link` may, and only they reply: a reply can carry an ask's
//!   answer.
//! - **A start marked as opting out of the sandbox is the window's alone, on every host** (ADR
//!   0078 §4, ADR 0068 amended 2026-10-15). It is refused from every other scope before the
//!   host hears of it, `remote-link` included: an opt-out is asked again on the device a chat
//!   moves to. What a relayed start runs is what the runner's trusted peer composed.
//! - **A held project's kept board is read and moved by the window's own link alone** (1.10,
//!   #2194). The kept board is what the window draws Needs you from, and its moves are the
//!   window's decisions on the lines it heard and the person's acts (an Ignore, an answer, a
//!   close). Only `local-ui` sends either; a relayed `remote-link` does not.
//! - **Taking a project's programs back, and quitting the host, are the window's alone** (1.13,
//!   #2006). What `programs_of` answers is how the window takes each chat back, the place of
//!   its confinement among it, which only the window confines; and `quit` ends every program
//!   and the host with them, which is the person's act of quitting purlis. A stop needs no
//!   human, so every other scope still has `stop` and `stop_all`.
//! - **A browser is started and watched in the window alone** (1.6, #2012). A browser is a
//!   program started as the person, and what it shows is theirs: the pages of sites they are
//!   signed in to. Until runners run browsers (spec #2041), no other scope starts one or attaches
//!   a view of one, `remote-link` included; a stop of one, like every stop, only removes power.
//! - **A chat's browser is driven through the window alone** (1.11, #2013). The window asks the
//!   person what a chat's browser may reach, starts it with that reach, and hands on each of
//!   the chat's acts with `browse`. No other scope sends an act, `remote-link` included: a
//!   relayed hold does not carry a chat's browser.
//! - **A browser is taken over, driven and handed back in the window alone** (1.14, #2014). What
//!   the person types into a page is theirs, and only the window, where they see the page, sends
//!   it. No other scope takes a browser over or hands it back, `remote-link` included.
//! - **Saving a held project is the window's alone** (1.7): the project's commits and pushes
//!   are the person's, and only the link that may save a project hears how saving it goes: what
//!   each look did and the host's question of who is mid-turn go only to the window's links that
//!   hold it, which alone answer the question. A hold relayed over a link does not carry it.
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
//! - **`terminal` and `remote-link` send no `answer`, pending a ruling.** ADR 0078 lets
//!   `terminal` answer and a runner's `remote-link` carry an answer a desktop human scope sent;
//!   V75, ruled later, names only `local-ui` and `approval` as answerers. Both are refused the
//!   `answer` command until the operator rules, which fails closed; `remote-link`'s relayed
//!   `reply_heard` is the one way a hook's ask's answer reaches a runner (ruled 2026-10-15), and
//!   its relayed `change_reach` the one way a sandbox ask's answer does (#2217).

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
    /// `start_saving`, `stop_saving`, `fetch_project` and `mid_turn` (1.7): whether and how the
    /// host saves a held project, and hearing the pushed `saving` and `who_is_mid_turn`. The
    /// window's alone: a relayed hold does not carry it.
    Save,
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
    /// person, and what it shows; `take_over`, `hand_back`, `browser_input` and `size_browser`
    /// (1.14), the person driving it; and `browse` (1.11), an act of a chat's browser, which the
    /// window hands on once it has decided the chat may have one.
    Browser,
    /// `record` (1.8): a write in the event log the host holds.
    Record,
    /// `log_in` (1.18, #2016): a login purlis performs in a browser, carrying the values from a
    /// vault. The window's alone: a relayed link never carries a vault's value to another host.
    LogIn,
    /// A start that opts its chat out of the sandbox (`without_sandbox`): the window's alone.
    OptOut,
    /// A start's `launch` that names a harness (#2217): a chat the window composed with the
    /// sandbox it compiled for its own machine, taken by the window's own host alone.
    LaunchAHarness,
    /// `board_moved` and `board` (1.10): what a held project's kept board holds, and the
    /// window's moves of it.
    Board,
    /// `programs_of` (1.13): a held project's programs, with what a window takes each back by,
    /// its confinement's place among them.
    Reattach,
    /// `quit` (1.13): the host ends every program and stops.
    Quit,
}

impl Scope {
    /// Whether this scope is granted `power`: the table above, and nothing else.
    pub fn may(self, power: Power) -> bool {
        use Power as P;
        use Scope as S;
        match self {
            S::LocalUi => true,
            S::Terminal => matches!(
                power,
                P::List | P::View | P::Write | P::Resize | P::Stop | P::Start | P::Subscribe
            ),
            S::RemoteLink => matches!(
                power,
                P::List
                    | P::View
                    | P::Write
                    | P::Resize
                    | P::Stop
                    | P::Start
                    | P::Subscribe
                    | P::Hold
                    | P::Launch
                    | P::Heard
                    | P::Confine
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
    fn only_the_window_or_its_hosts_relay_hands_a_host_a_composed_launch_or_holds_a_project() {
        for scope in Scope::ALL {
            assert_eq!(scope.may(Power::Rearm), scope == Scope::LocalUi, "{scope}");
            let windows = matches!(scope, Scope::LocalUi | Scope::RemoteLink);
            assert_eq!(scope.may(Power::Launch), windows, "{scope}");
            assert_eq!(scope.may(Power::Hold), windows, "{scope}");
        }
    }

    #[test]
    fn only_the_window_or_its_hosts_relay_replies_to_what_a_projects_hooks_wrote() {
        for scope in Scope::ALL {
            let windows = matches!(scope, Scope::LocalUi | Scope::RemoteLink);
            assert_eq!(scope.may(Power::Heard), windows, "{scope}");
        }
    }

    #[test]
    fn only_the_window_or_its_hosts_relay_confines_a_chat_and_answers_its_sandbox() {
        for scope in Scope::ALL {
            let windows = matches!(scope, Scope::LocalUi | Scope::RemoteLink);
            assert_eq!(scope.may(Power::Confine), windows, "{scope}");
        }
    }

    #[test]
    fn only_the_window_hands_its_own_host_a_chat_it_composed_for_a_harness() {
        for scope in Scope::ALL {
            assert_eq!(
                scope.may(Power::LaunchAHarness),
                scope == Scope::LocalUi,
                "{scope}"
            );
        }
    }

    #[test]
    fn only_the_window_reads_or_moves_a_projects_board() {
        for scope in Scope::ALL {
            assert_eq!(scope.may(Power::Board), scope == Scope::LocalUi, "{scope}");
        }
    }

    #[test]
    fn only_the_window_takes_back_a_projects_programs_or_quits_its_host() {
        for scope in Scope::ALL {
            assert_eq!(
                scope.may(Power::Reattach),
                scope == Scope::LocalUi,
                "{scope}"
            );
            assert_eq!(scope.may(Power::Quit), scope == Scope::LocalUi, "{scope}");
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
    fn only_the_window_writes_in_the_event_log_the_host_holds() {
        for scope in Scope::ALL {
            assert_eq!(scope.may(Power::Record), scope == Scope::LocalUi, "{scope}");
        }
    }

    #[test]
    fn only_the_window_saves_a_held_project() {
        for scope in Scope::ALL {
            assert_eq!(scope.may(Power::Save), scope == Scope::LocalUi, "{scope}");
        }
    }

    #[test]
    fn only_the_window_opts_a_chat_out_of_the_sandbox() {
        for scope in Scope::ALL {
            assert_eq!(scope.may(Power::OptOut), scope == Scope::LocalUi, "{scope}");
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
