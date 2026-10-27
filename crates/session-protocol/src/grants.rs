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
//! | `hold_project`, `let_go_of_project` (1.2), and the pushed `not_held` a relaying host says to the client that held the project (1.30) | yes | | | | | relayed |
//! | `stop_all` (1.5) | yes | yes | yes | yes | | yes |
//! | `kill_switch`, the switch's state (1.5) | yes | yes | yes | yes | | yes |
//! | `rearm` (1.5) | yes | | | | | relayed (#2051) |
//! | a start's `launch`, the window's own composed start (1.2) | yes | | | | | relayed |
//! | `deal`, `dealt_already` (1.3) | yes | | | | | relayed |
//! | `reply_heard`, and the `heard` lines pushed (1.3) | yes | | | | | relayed |
//! | a start's sandbox opt-out (`without_sandbox`) | yes | | | | | |
//! | `confine`, `let_go_of_confinement`, `change_reach`, `reach_of`, and the pushed `sandbox` (1.4); a start's `confinement`, the composed confinement named to its link (1.16), and its `alone` (1.26) | yes | | | | | relayed |
//! | a start's `launch` that names a harness, the window's composed chat | yes | | | | | |
//! | `start_program` of kind `browser`, `attach_view` of kind `browser` (1.6) | yes | | | | | relayed (1.38) |
//! | `start_saving`, `stop_saving`, `fetch_project`, `mid_turn`, and the pushed `saving` and `who_is_mid_turn` (1.7) | yes | | | | | |
//! | `record`, a write in the event log the host holds (1.8) | yes | | | | | |
//! | `board_moved`, `board` (1.10) | yes | | | | | |
//! | `browse`, an act of a chat's browser, and a browser's `reach` (1.11) | yes | | | | | relayed (1.38) |
//! | `programs_of` (1.13), a project's browsers among them, `note_program` and a start's `note` (1.39) | yes | | | | | |
//! | `quit` (1.13) | yes | | | | | |
//! | `take_back`, a relaying host naming the window a link is for, named again the confinements of the chats that window started (1.22) | | | | | | yes |
//! | `take_over`, `hand_back`, `browser_input`, `size_browser`: the person driving a browser (1.14) | yes | | | | | relayed (1.38) |
//! | `log_in`, a login purlis performs with a vault's values (1.18) | yes | | | | | relayed (1.38) |
//! | a kept login: a browser's start from one, and a `log_in` that keeps one (1.27) | yes | | | | | |
//! | `unpair`, the desktop taken out of a runner's peers (1.25) | | | | | | yes |
//! | `keep_profile`, a desktop's profile kept on its runner (1.42) | | | | | | yes |
//! | `name_window`, the window naming itself on a link to its own host (1.31) | yes | | | | | |
//! | `relay_to`, the window's link relayed by its host to a runner it keeps (1.33) | yes | | | | | |
//! | `take_code`, `give_code`, a repository's history moved over the link as a git bundle (1.34) | yes | | | | | relayed |
//! | `cancel_engine_download`, and the pushed `engine_download`: a browser engine's first download (1.37) | yes | | | | | |
//! | `log_end`, and the host's own event log pushed from a cursor for its device (1.41) | yes | | | | | relayed |
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
//!   human, so every other scope still has `stop` and `stop_all`. A program's note (1.39,
//!   #2025) is the window's own record of it, which it reads back as it takes the program back
//!   (a chat's browser, by its chat), so only the window keeps one.
//! - **A window takes back its own runner chats' sandboxes after a re-link, and only its own**
//!   (1.22, #2511, ruled 2026-10-17). `take_back` is the desktop's relay's own word over the
//!   link between hosts, never a client's: on each link it makes for one window it names that
//!   window, and the runner names that link the confinements of the chats that window started
//!   there over the links before, made or composed for them. It names it no confinement the
//!   runner's own window made, and none made or composed for a link of another window. The
//!   window itself sends none: on its own host it takes its chats back with `programs_of`, and
//!   the desktop's relay refuses one from any client.
//! - **A browser is started and watched in the window alone** (1.6, #2012). A browser is a
//!   program started as the person, and what it shows is theirs: the pages of sites they are
//!   signed in to. No other scope starts one or attaches a view of one; a stop of one, like
//!   every stop, only removes power. **On a runner it is the window's as its host relays it**
//!   (1.38, #2050, spec #2041): a runner chat's browser runs in the runner's host, and the
//!   desktop's relay sends its start and its views on for its own `local-ui` client alone.
//! - **A chat's browser is driven through the window alone** (1.11, #2013). The window asks the
//!   person what a chat's browser may reach, starts it with that reach, and hands on each of
//!   the chat's acts with `browse`. No other scope sends an act; `remote-link` sends one only as
//!   the desktop's host relaying its window (1.38).
//! - **A browser is taken over, driven and handed back in the window alone** (1.14, #2014). What
//!   the person types into a page is theirs, and only the window, where they see the page, sends
//!   it, to its own host or relayed by it to a runner (1.38).
//! - **A login purlis performs is the window's alone** (1.18, #2016): it carries a vault's
//!   values, which only the window reads. Relayed to a runner (1.38, #2050) only as the window
//!   decided it (a runner chat's persona opted in for that pool, the desktop connected), and the
//!   runner holds the values only while the browser fills them, never storing them. **A kept
//!   login is this device's alone** (1.27, #2021): a browser opened from one, and a login that
//!   keeps one, are refused from `remote-link`, so a kept login never leaves the device it was
//!   made on.
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
//!   another scope is refused in words saying where a re-arm is made. **A runner is re-armed by
//!   its desktop's window, through its desktop's host** (#2051, ADR 0078 §4 and ADR 0071 as
//!   amended, ruled V29b): a runner has no window of its own, so its peer's `remote-link` may
//!   re-arm it, and the desktop's host sends one only as its own window re-arms it, to the
//!   runners its stop reached, or on as it relays its window's link. No other scope of the
//!   desktop's host reaches a link.
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
//! - **`unpair` is the paired desktop's, on its own link to the runner** (1.25, #2525). It takes
//!   that desktop out of the runner's peers as the person removes the runner (ADR 0078 §3), so
//!   only `remote-link` sends it where it is carried out, and a runner's host carries it out
//!   for the desktop the link's handshake proved, never another. No other scope is granted it,
//!   the window's included, whose own host holds no peers; and a relaying host refuses it, so it
//!   never reaches a runner but from the desktop that removes it.
//! - **`keep_profile` is the paired desktop's, on its own link to the runner** (1.42, #2049),
//!   as `unpair` is. It names a profile of the desktop's, which the runner keeps in a folder of
//!   its own with nothing of the desktop's in it, so a runner's chats can start on it. No other
//!   scope is granted it, the window's included, whose own host has its profiles already; and a
//!   relaying host refuses it, so it reaches a runner only from the desktop it is paired with.
//! - **`name_window` is the window's, on a link to its own host** (1.31, #2623). It names the
//!   window by the name it keeps for that link across its restarts, so its host's relay names it
//!   to a runner the same way after a restart, and the window takes back its runner chats. Only
//!   `local-ui` sends it: a name another scope gave would take the window's chats. A relaying host
//!   never sends it on, and says only a digest of it to the other host, so `remote-link` is never
//!   granted it, and a runner never learns the name.
//! - **`relay_to` is the window's alone** (1.33, #2679). It has the desktop's host link to a
//!   runner with this device's key and run the runner's connector, which is a human act (ADR
//!   0078 §2 and §4): no agent's scope reaches a link. A relaying host refuses it, so a window
//!   whose link is relayed never asks the runner to relay again.
//! - **A host's own event log is read by the window, on its own host or relayed** (1.41,
//!   #3393, ADR 0078 §7). It is the audit's source and names what each chat ran, so only the
//!   window reads it: on the desktop it reads this machine's from its own disk, and a runner's
//!   through its host's relay, from a cursor of its own for that runner's device. Nothing of a
//!   runner's log is copied into the desktop's. No other scope is pushed it.
//! - **`take_code` and `give_code` are the window's, relayed for it** (1.34, #2046). They write
//!   into a repository of a project the host holds, and read its branches back: what the window
//!   decides moves between the desktop and a runner, as holding the project is. A relaying host
//!   sends them on for a `local-ui` client alone, so `remote-link` is granted them only as the
//!   desktop's host relaying its window, never as any other client of it.

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
    /// vault. The window's, relayed by its host to a runner's browser as the window decided it
    /// (1.38, #2050).
    LogIn,
    /// A kept login (1.27, #2021): a browser's start from one, and a `log_in` that keeps one.
    /// This device's alone: never relayed, so a kept login stays where it was made.
    KeptLogin,
    /// A start that opts its chat out of the sandbox (`without_sandbox`): the window's alone.
    OptOut,
    /// A start's `launch` that names a harness (#2217): a chat the window composed with the
    /// sandbox it compiled for its own machine, taken by the window's own host alone.
    LaunchAHarness,
    /// `board_moved` and `board` (1.10): what a held project's kept board holds, and the
    /// window's moves of it.
    Board,
    /// `programs_of` (1.13): a held project's programs, with what a window takes each back by,
    /// its confinement's place among them; and the window's note of each, kept in the host by
    /// `note_program` and a start's `note` (1.39, #2025).
    Reattach,
    /// `quit` (1.13): the host ends every program and stops.
    Quit,
    /// `take_back` (1.22): a relaying host names the window a link is for, and is named again
    /// the confinements of the chats that window started. Never the window's own.
    TakeBack,
    /// `unpair` (1.25, #2525): the desktop on a link between hosts taken out of the runner's
    /// peers, as it removes the runner.
    Unpair,
    /// `keep_profile` (1.42, #2049): a desktop's profile kept on its runner.
    KeepProfile,
    /// `name_window` (1.31, #2623): the window names itself on a link to its own host, by the
    /// name it keeps for that link. The window's alone.
    NameWindow,
    /// `relay_to` (1.33, #2679): the window's link relayed by its own host to a runner it keeps.
    RelayTo,
    /// `take_code` and `give_code` (1.34, #2046): a repository's history moved between the
    /// desktop and a runner as a git bundle.
    Code,
    /// `log_end`, and the host's own event log pushed from a cursor for its device (1.41,
    /// #3393): the window's, on its own host or relayed to a runner's.
    EventLog,
}

impl Scope {
    /// Whether this scope is granted `power`: the table above, and nothing else.
    pub fn may(self, power: Power) -> bool {
        use Power as P;
        use Scope as S;
        match self {
            S::LocalUi => !matches!(power, P::TakeBack | P::Unpair | P::KeepProfile),
            S::Terminal => matches!(
                power,
                P::List | P::View | P::Write | P::Resize | P::Stop | P::Start | P::Subscribe
            ),
            // Only the window names itself: every other scope is refused it below, as none of
            // their rows lists it.
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
                    | P::Browser
                    | P::LogIn
                    | P::TakeBack
                    | P::Unpair
                    | P::KeepProfile
                    | P::Code
                    | P::Rearm
                    | P::EventLog
            ),
            S::FleetMcp => matches!(power, P::List | P::Stop | P::Subscribe),
            S::Approval => matches!(power, P::List | P::Answer | P::Stop | P::Subscribe),
            S::Editor => false,
        }
    }
}

#[cfg(test)]
mod tests;
