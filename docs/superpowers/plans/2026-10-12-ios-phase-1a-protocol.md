# iOS Phase 1A: the protocol additions — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `session-protocol` 1.1: the `remote-device` scope and its three powers, `choice_lasts`
failing closed, `workspace` and `level` on a listed chat, the `prompt` command, and the
`timeline` stream kind, each recorded in the compatibility fixture, with no host behind them
yet.

**Architecture:** Everything lands inside `crates/session-protocol`, a minor version (1.0 →
1.1) that only adds. The scope goes into `auth::Scope` and `grants`, the check into
`session::serve` beside `elicits_a_secret`, the two commands and the answer into
`session::Command`/`Answer` through the `wire_words!` lists, and the timeline into a new
`timeline` module that mirrors `view` (a header byte, then length-delimited JSON items, bounded
by a queue; Yamux credit is the flow control). A new `stream` module tells a client which kind
of stream the host opened, so a view and a timeline can share one link.

**Tech Stack:** Rust 1.99 (workspace pin), tokio, tokio-util `LengthDelimitedCodec`, serde_json,
yamux (unchanged).

**Spec:** `docs/superpowers/specs/2026-10-11-ios-mobile-design.md` §4.3, §5 and §8
("Protocol"), with `docs/superpowers/specs/2026-10-11-ios-phase-0-findings.md` §2 and §4 for
what Phase 0 settled. Phase 1 is five plans; this is the first and the other four consume its
**Produces** blocks:

| Plan | Delivers |
|---|---|
| **1A (this)** | the protocol crate's additions |
| 1B | `purlis-core::remote`: Noise XX in the crate (`link::noise`, producing `ProvenDevice`), pairing, `peers.json` (its plane-format row first), the `Host` impl over `SessionHost`/`ChatBoard`, the timeline adapter, `choice_lasts`, audit metadata |
| 1C | desktop: Settings → Devices, "Keep this Mac awake", `machine.json` keys (row first), changelog fragments |
| 1D | `crates/purlis-mobile-core`: connect, reconnect from cursor, Keychain, Tauri commands |
| 1E | `mobile/`: Pair, Home, the level-2 chat screen, the ask sheet, the lock, the disconnected rules |

## Global Constraints

- Local branch `ios-mobile-design` of `~/purlis-research/purlis`. **Never push, never open a PR,
  never write to GitHub.** `gh` auth on this machine is invalid; that is fine.
- Commit messages: plain "what was done" only. **No `Co-Authored-By`, no AI trailers, no mention
  of AI.** purlis's `commit-msg` hook adds `Assisted-by:` when `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_0`/
  `GIT_CONFIG_VALUE_0` are in the environment: commit normally once (so its secret scan runs),
  then `env -u GIT_CONFIG_COUNT -u GIT_CONFIG_KEY_0 -u GIT_CONFIG_VALUE_0 git commit --amend -F <msg>`
  with the same message, and check `git log -1 --format=%B` shows no `Assisted-by:`.
- AGENTS.md: nothing parses harness output; no `unsafe`; a new store names its tier in
  `docs/plane-format.md` first (**this plan adds no store**); a change people would notice gets a
  `changes/<slug>.md` fragment (**this plan adds none: nothing here is reachable by a person
  until 1B serves it**).
- Spec §5: a minor only adds. A reader ignores fields it does not know; a host answers a command
  it does not know with `unknown_command`. N−1 holds for all four additions (§5.6).
- Spec §4.3: `AnswerAlways` is checked by `serve`, through `Host::choice_lasts`, **failing closed**:
  a host that does not implement it is taken to say the choice lasts, so `remote-device` is
  refused.
- Spec §4.4/D9: no message, prompt or answer text is ever logged by the crate. The crate logs
  nothing today; keep it so.
- Every word on the wire is snake_case (`the_session_protocol_keeps_every_frame_it_promised.rs`
  checks).
- Clippy runs with `-D warnings`. `cargo fmt` before every commit. Check command for the crate:
  `cargo test -p purlis-session-protocol` and `cargo clippy -p purlis-session-protocol --all-targets -- -D warnings`.
- The crate's `Scope::ALL` length is load-bearing: `Credentials` is an array of that length and
  `Credentials::mint` lists one `Credential::mint()?` per scope by hand.

## Review Focus

1. **A `remote-device` link answers an ask with a choice the host has not classified.** A host that
   leaves `choice_lasts` at its default must refuse every `answer` from `remote-device`, never let
   one through (Task 2: `a_host_that_says_nothing_of_a_choice_refuses_every_answer_from_remote_device`).
2. **An `answer` from `remote-device` to a secret's ask** must be refused by the secret check
   before the always check is consulted, so a host that answers `false` to `choice_lasts` cannot
   be used to leak a secret ask through (Task 2: `remote_device_never_answers_a_secrets_ask_whatever_the_choice_lasts`).
3. **A timeline whose client reads nothing** must not grow the host's memory past its bound: the
   feed closes `FellBehind` and the client learns it (Task 6:
   `a_timeline_whose_client_stops_reading_is_dropped_at_its_bound`).
4. **An older host meeting the new client**: `attach_timeline` and `prompt` refused
   `unknown_command` reach the client as `CallError::Refused` with that code, and a listed chat
   without `workspace`/`level` reads with both `None` (Task 7).
5. **A stream whose first byte is neither view nor timeline** is refused by the client's
   dispatcher with an `InvalidData` error naming the byte, never read as a view (Task 5:
   `a_stream_of_an_unknown_kind_is_refused_by_name`).

---

### Task 1: The `remote-device` scope

**Files:**
- Modify: `crates/session-protocol/src/auth.rs:76-137` (the `Scope` enum, `ALL`, `word`), `:236-255` (`Credentials::mint`)
- Modify: `crates/session-protocol/src/grants.rs` (the table, `Power`, `Scope::may`, the module doc)
- Modify: `crates/session-protocol/src/link.rs:295-330` (`ProvenDevice` carries its scope; `connect_as_a_device` takes one)
- Modify: `crates/session-protocol/tests/each_scope_may_call_only_what_its_row_grants.rs:78-90, 133-160`
- Test: `crates/session-protocol/src/grants.rs` (unit tests), `crates/session-protocol/tests/each_scope_may_call_only_what_its_row_grants.rs`

**Interfaces:**
- Consumes: nothing earlier.
- Produces: `Scope::RemoteDevice` (word `remote-device`, no credential file);
  `Power::{Timeline, Prompt, AnswerAlways}`; `ProvenDevice::scope(&self) -> Scope`;
  `ProvenDevice::stand_in_as(scope: Scope) -> ProvenDevice` (any-stream only);
  `link::connect_as_a_device(io, speaks, scope: Scope)` (any-stream only);
  `link::serve_as_a_device(io, speaks, device)` admits as `device.scope()`.

- [ ] **Step 1: Write the failing unit tests in `grants.rs`**

Replace the `tests` module at the bottom of `crates/session-protocol/src/grants.rs` with:

```rust
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
        assert_eq!(answer, [Scope::LocalUi, Scope::Approval, Scope::RemoteDevice]);
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
```

- [ ] **Step 2: Run them to see them fail to compile**

Run: `cd ~/purlis-research/purlis && cargo test -p purlis-session-protocol --lib grants 2>&1 | tail -20`
Expected: FAIL to compile: `no variant named RemoteDevice`, `no variant named AnswerAlways`.

- [ ] **Step 3: Add the scope to `auth.rs`**

In `crates/session-protocol/src/auth.rs`, change the `Scope` enum and its constants (lines 76–137) to:

```rust
/// A client scope (ADR 0068 §5, ADR 0081 for `editor`, ADR 0078 §3 for `remote-link`, the
/// iOS design §4.3 for `remote-device`): who a link is. What each may do is
/// [`crate::grants`]' table (FD-27).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// The app's window.
    LocalUi,
    /// `charter attach` and `charter ls` from the operator's shell.
    Terminal,
    /// The fleet MCP server.
    FleetMcp,
    /// `charter inbox` and approvals from a shell.
    Approval,
    /// The editor's charter extension.
    Editor,
    /// A peer device of the operator's, through a connector (V9): the desktop's `charterd` on a
    /// runner's. Proved by its link key, never by a credential file (ADR 0078 §3).
    RemoteLink,
    /// The operator's own phone, paired to this machine: a second client of its chats, proved by
    /// its link key inside a Noise handshake against the pinned record, never by a credential
    /// file (iOS design §4). It steers a chat but never resizes or starts one, and never makes
    /// a choice that lasts.
    RemoteDevice,
}

impl Scope {
    /// Every scope.
    pub const ALL: [Scope; 7] = [
        Scope::LocalUi,
        Scope::Terminal,
        Scope::FleetMcp,
        Scope::Approval,
        Scope::Editor,
        Scope::RemoteLink,
        Scope::RemoteDevice,
    ];

    /// The scopes on `charterd.sock`, each proved by a credential file this exchange checks:
    /// every one a person's, so none is ever admitted from inside a chat (V16a).
    pub const WITH_A_CREDENTIAL: [Scope; 5] = [
        Scope::LocalUi,
        Scope::Terminal,
        Scope::FleetMcp,
        Scope::Approval,
        Scope::Editor,
    ];

    /// Whether this scope is proved by a credential file ([`Scope::WITH_A_CREDENTIAL`]).
    pub fn has_a_credential(self) -> bool {
        Scope::WITH_A_CREDENTIAL.contains(&self)
    }

    /// The scope's word: on the wire, and as its credential file's name.
    pub fn word(self) -> &'static str {
        match self {
            Scope::LocalUi => "local-ui",
            Scope::Terminal => "terminal",
            Scope::FleetMcp => "fleet-mcp",
            Scope::Approval => "approval",
            Scope::Editor => "editor",
            Scope::RemoteLink => "remote-link",
            Scope::RemoteDevice => "remote-device",
        }
    }

    /// The scope a word names, where it names one.
    pub fn from_word(word: &str) -> Option<Scope> {
        Scope::ALL.into_iter().find(|scope| scope.word() == word)
    }
}
```

In `Credentials::mint` (line 246) add a seventh `Credential::mint()?,` line so the array has
`Scope::ALL.len()` entries. Update the doc comment above `Credentials` (line 236) to read
"`remote-link` and `remote-device` are held one each too, minted in memory and never written, …".
In the module doc (line 36–41), after "a peer device, [`Scope::RemoteLink`], by its link key
inside a Noise handshake" add ", and a paired phone, [`Scope::RemoteDevice`], the same way
(iOS design §4)", and change "An answer that names `remote-link` here" to "An answer that names
`remote-link` or `remote-device` here".

- [ ] **Step 4: Add the powers and the column to `grants.rs`**

Replace the table in the module doc with:

```rust
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
```

Add to the "Why each row is what it is" list:

```rust
//! - **`remote-device` is the operator's own phone** (iOS design §4.3): it continues the work
//!   (list, view, timeline, write, prompt, answer, stop) and nothing that shapes the desk. It
//!   never resizes (a chat's terminal is one terminal, §5.4), never starts a chat (phase 4), and
//!   never makes a choice that lasts (D8): whether an answer's choice lasts is the host's to say
//!   ([`crate::session::Host::choice_lasts`]), and a host that does not say is taken to mean it
//!   does, so the refusal fails closed, as a secret's does.
```

Change `Power`:

```rust
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
```

And `Scope::may`:

```rust
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
```

- [ ] **Step 5: Run the unit tests**

Run: `cargo test -p purlis-session-protocol --lib grants 2>&1 | tail -12`
Expected: PASS, 5 tests.

- [ ] **Step 6: Let a proven device carry its scope (`link.rs`)**

Replace lines 295–330 of `crates/session-protocol/src/link.rs` (`ProvenDevice` through
`serve_as_a_device`) with:

```rust
/// A peer device a link's Noise `XX` handshake proved against the keys pinned at pairing (ADR
/// 0078 §3, iOS design §4.2): what admits a link as [`Scope::RemoteLink`] or
/// [`Scope::RemoteDevice`], since no credential file does.
///
/// **Only that handshake makes one.** `link::noise` (Phase 1B) builds it, and its result is the
/// only thing that should. Until then neither scope is admitted anywhere: it fails closed. The
/// crate's own tests stand in for the handshake with [`ProvenDevice::stand_in`] and
/// [`ProvenDevice::stand_in_as`], which exist only with the `any-stream` feature.
#[derive(Debug)]
pub struct ProvenDevice {
    scope: Scope,
}

impl ProvenDevice {
    /// The scope the handshake proved the device to be: which pinned record its key matched.
    pub fn scope(&self) -> Scope {
        self.scope
    }
}

#[cfg(feature = "any-stream")]
impl ProvenDevice {
    /// A runner's `charterd` taken as proved, for the crate's tests only.
    pub fn stand_in() -> ProvenDevice {
        ProvenDevice::stand_in_as(Scope::RemoteLink)
    }

    /// A device taken as proved to be `scope`, for the crate's tests only. Only a device scope
    /// is one a handshake can prove.
    pub fn stand_in_as(scope: Scope) -> ProvenDevice {
        assert!(
            !scope.has_a_credential(),
            "{scope} is proved by a credential file, never by a handshake"
        );
        ProvenDevice { scope }
    }
}

/// The host end of a link from a peer device that `device` proved: answer with what `speaks`
/// names, start the multiplexer and take the control lane, all within one
/// [`version::HANDSHAKE_TIMEOUT`]. The link is the scope the device was proved as, whatever
/// the device says.
pub async fn serve_as_a_device<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    io: S,
    speaks: Speaks,
    device: ProvenDevice,
) -> Result<Link, LinkError> {
    let scope = device.scope();
    within_the_handshake(async {
        let (version, io) = version::answer_unbounded(io, &speaks).await?;
        take_the_lane(version, scope, io).await
    })
    .await
}
```

Then change `connect_as_a_device` to take the scope, replacing its signature and the `scope:`
line in its `Link`:

```rust
/// The device's end of [`serve_as_a_device`], for the crate's tests: negotiate and open the
/// control lane, with no credential, since the stream stands for one a handshake has already
/// proved to be `scope`.
#[cfg(feature = "any-stream")]
pub async fn connect_as_a_device<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    io: S,
    speaks: Speaks,
    scope: Scope,
) -> Result<Link, LinkError> {
```

and inside it `scope,` in place of `scope: Scope::RemoteLink,`.

- [ ] **Step 7: Fix the two callers and add the scope's row to the table test**

In `crates/session-protocol/tests/each_scope_may_call_only_what_its_row_grants.rs` replace the
`linked` function's first branch (lines 80–85) with:

```rust
    let (client, served) = if !scope.has_a_credential() {
        tokio::join!(
            link::connect_as_a_device(a, session::speaks(), scope),
            link::serve_as_a_device(
                b,
                session::speaks(),
                link::ProvenDevice::stand_in_as(scope)
            )
        )
    } else {
```

In `granted` add, before `Scope::Editor => &[],`:

```rust
        Scope::RemoteDevice => &[
            "list",
            "attach",
            "detach",
            "write",
            "answer",
            "stop",
            "subscribe",
            "unsubscribe",
        ],
```

(`attach_timeline`, `detach_timeline` and `prompt` join this row in Task 3 and Task 6, when
`every_command` has them.) And add the test:

```rust
#[tokio::test]
async fn remote_device_steers_a_chat_and_answers_but_never_resizes_or_starts_one() {
    check_the_row(Scope::RemoteDevice).await;
}
```

Also extend `an_ask_that_elicits_a_secret_is_answered_from_local_ui_only` to cover the new scope:
change its array to
`[(Scope::LocalUi, true), (Scope::Approval, false), (Scope::RemoteDevice, false)]`.

`a_link_carries_a_control_lane_and_streams.rs:172` and
`a_remote_link_client_in_another_process_lists_snapshots_and_stops.rs:97` call
`ProvenDevice::stand_in()` and need no change.

- [ ] **Step 8: Run the crate's suite**

Run: `cargo test -p purlis-session-protocol 2>&1 | grep -E "^test result|FAILED|panicked" | head -40`
Expected: every `test result: ok`. `remote_device_steers_a_chat_and_answers_but_never_resizes_or_starts_one` passes with the eight words above.

- [ ] **Step 9: Clippy, fmt, commit**

```bash
cd ~/purlis-research/purlis && cargo fmt -p purlis-session-protocol && cargo clippy -p purlis-session-protocol --all-targets -- -D warnings
git add crates/session-protocol && git commit -q -m "Session protocol: the remote-device scope, with timeline, prompt and a lasting choice as powers"
env -u GIT_CONFIG_COUNT -u GIT_CONFIG_KEY_0 -u GIT_CONFIG_VALUE_0 git commit -q --amend -m "Session protocol: the remote-device scope, with timeline, prompt and a lasting choice as powers"
git log -1 --format=%B | grep -c Assisted-by
```
Expected: clippy clean; the last line prints `0`.

---

### Task 2: `choice_lasts`, checked by `serve` and failing closed

**Files:**
- Modify: `crates/session-protocol/src/session.rs:363-394` (`Host`), `:464-488` (`refused`)
- Test: `crates/session-protocol/tests/each_scope_may_call_only_what_its_row_grants.rs`

**Interfaces:**
- Consumes: `Power::AnswerAlways`, `Scope::RemoteDevice` (Task 1).
- Produces: `Host::choice_lasts(&self, chat: &str, ask: &str, answer: &str) -> impl Future<Output = bool> + Send`, default `true`.

- [ ] **Step 1: Write the failing tests**

Add to `each_scope_may_call_only_what_its_row_grants.rs`. First give `Board` a third field and
make `choice_lasts` answer from it: change the struct and constructor to

```rust
#[derive(Clone)]
struct Board {
    heard: Arc<Mutex<Vec<String>>>,
    secret: bool,
    lasts: bool,
}

impl Board {
    fn new(secret: bool) -> Board {
        Board::with(secret, false)
    }

    fn with(secret: bool, lasts: bool) -> Board {
        Board {
            heard: Arc::default(),
            secret,
            lasts,
        }
    }
    …
}
```

and add to its `impl Host`:

```rust
    async fn choice_lasts(&self, _chat: &str, _ask: &str, _answer: &str) -> bool {
        self.lasts
    }
```

Then the tests:

```rust
#[tokio::test]
async fn a_choice_that_lasts_is_answered_from_local_ui_only() {
    for (scope, answers) in [
        (Scope::LocalUi, true),
        (Scope::Approval, false),
        (Scope::RemoteDevice, false),
    ] {
        let board = Board::with(false, true);
        let client = linked(scope, board.clone()).await;
        let answered = client.answer(CHAT, ASK, "always").await;
        assert_eq!(answered.is_ok(), answers, "{scope}: {answered:?}");
        if !answers {
            assert_eq!(
                answered.unwrap_err().refusal().map(|r| r.code.clone()),
                Some(session::NOT_ALLOWED.to_owned())
            );
            assert!(board.heard().is_empty(), "the host never heard it");
        }
    }
}

#[tokio::test]
async fn a_choice_for_this_session_is_answered_from_remote_device() {
    let board = Board::with(false, false);
    let client = linked(Scope::RemoteDevice, board.clone()).await;
    client.answer(CHAT, ASK, "session").await.unwrap();
    assert_eq!(board.heard(), ["answer"]);
}

/// Corrected after the final review: `Silent` also leaves `elicits_a_secret` at its default,
/// so the refusal below comes from the secret check and never reaches `choice_lasts`. The
/// test that pins the fail-closed default is `NoSecrets` (answers `false` to the secret
/// question, says nothing of the choice) with
/// `a_host_that_knows_no_secrets_but_says_nothing_of_a_choice_refuses_remote_device`,
/// proven red by flipping the default to `false` once.
#[tokio::test]
async fn a_host_that_says_nothing_of_a_choice_refuses_every_answer_from_remote_device() {
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect_as_a_device(a, session::speaks(), Scope::RemoteDevice),
        link::serve_as_a_device(
            b,
            session::speaks(),
            link::ProvenDevice::stand_in_as(Scope::RemoteDevice)
        )
    );
    // `Silent` says nothing of its asks: taken to hold a secret, and taken to let a choice last.
    tokio::spawn(session::serve(served.unwrap(), Silent, None));
    let (client, _views, _events) = Client::new(client.unwrap());
    let refused = client.answer(CHAT, ASK, "yes").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_ALLOWED)
    );
}

/// A host that classifies every ask as no secret and every choice as lasting: the secret check
/// is what refuses a secret's ask, before the choice is even asked about.
#[derive(Clone, Default)]
struct SecretButOnce;

impl Host for SecretButOnce {
    async fn call(&self, _command: Command, _peer: &Peer) -> Result<Answer, Refusal> {
        Ok(Answer::Done)
    }

    async fn elicits_a_secret(&self, _chat: &str, _ask: &str) -> bool {
        true
    }

    async fn choice_lasts(&self, _chat: &str, _ask: &str, _answer: &str) -> bool {
        false
    }
}

#[tokio::test]
async fn remote_device_never_answers_a_secrets_ask_whatever_the_choice_lasts() {
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect_as_a_device(a, session::speaks(), Scope::RemoteDevice),
        link::serve_as_a_device(
            b,
            session::speaks(),
            link::ProvenDevice::stand_in_as(Scope::RemoteDevice)
        )
    );
    tokio::spawn(session::serve(served.unwrap(), SecretButOnce, None));
    let (client, _views, _events) = Client::new(client.unwrap());
    let refused = client.answer(CHAT, ASK, "once").await.unwrap_err();
    let why = refused.refusal().unwrap();
    assert_eq!(why.code, session::NOT_ALLOWED);
    assert!(why.why.contains("secret"), "{}", why.why);
}
```

- [ ] **Step 2: Run them**

Run: `cargo test -p purlis-session-protocol --test each_scope_may_call_only_what_its_row_grants 2>&1 | tail -20`
Expected: FAIL to compile: `choice_lasts` is not a member of trait `Host`.

- [ ] **Step 3: Add the trait method and the check**

In `session.rs`, after `elicits_a_secret` in `trait Host` (line 378) add:

```rust
    /// Whether answering `chat`'s ask `ask` with `answer` makes a choice that lasts beyond this
    /// session: one that writes a rule, which only `local-ui` may make (iOS design D8, §4.3).
    /// A host that does not say is taken to say yes, so the check fails closed.
    fn choice_lasts(
        &self,
        chat: &str,
        ask: &str,
        answer: &str,
    ) -> impl Future<Output = bool> + Send {
        let _ = (chat, ask, answer);
        async { true }
    }
```

In `refused`, after the secret check's `if let … { return Some(…) }` block and before `None`, add:

```rust
    if let Command::Answer { chat, ask, answer } = command
        && !scope.may(Power::AnswerAlways)
        && host.choice_lasts(chat, ask, answer).await
    {
        return Some(Refusal {
            code: NOT_ALLOWED.into(),
            why: format!(
                "this choice would last beyond the session, which is made in purlis's window \
                 only, and this link is `{scope}`"
            ),
        });
    }
```

Update the doc on `Command::Answer` (lines 120–122) to: "Only a human scope answers one
(`local-ui`, `approval` and `remote-device`), an ask that elicits a secret only `local-ui`, and
a choice that lasts beyond the session only `local-ui` ([`crate::grants`])."

- [ ] **Step 4: Run the test file**

Run: `cargo test -p purlis-session-protocol --test each_scope_may_call_only_what_its_row_grants 2>&1 | tail -20`
Expected: PASS, all tests in the file (the four new ones included). Note the secret test's
`why` names the secret, which proves the secret check ran first.

- [ ] **Step 5: Clippy, fmt, commit**

```bash
cargo fmt -p purlis-session-protocol && cargo clippy -p purlis-session-protocol --all-targets -- -D warnings
git add crates/session-protocol && git commit -q -m "Session protocol: a choice that lasts is refused before the host hears it, failing closed"
env -u GIT_CONFIG_COUNT -u GIT_CONFIG_KEY_0 -u GIT_CONFIG_VALUE_0 git commit -q --amend -m "Session protocol: a choice that lasts is refused before the host hears it, failing closed"
```
Expected: clippy clean, no `Assisted-by` trailer.

---

### Task 3: `prompt`, `workspace` and `level`, and version 1.1

**Files:**
- Modify: `crates/session-protocol/src/session.rs:56-57` (`VERSION`), `:99-137` (`Command`), `:173-191` (`Chat`), `:230-259` (words, `power`), `:608-692` (`Client`), after line 97 (two refusal codes)
- Modify: `crates/session-protocol/tests/fixtures/session-protocol.jsonl`
- Modify: `crates/session-protocol/tests/each_scope_may_call_only_what_its_row_grants.rs` (`every_command`, `granted`)
- Modify: `crates/session-protocol/tests/the_session_protocol_keeps_every_frame_it_promised.rs:150-160` (the refusal-code list)
- Test: `crates/session-protocol/tests/the_session_protocol_keeps_every_frame_it_promised.rs`, `each_scope_may_call_only_what_its_row_grants.rs`

**Interfaces:**
- Consumes: `Power::Prompt` (Task 1).
- Produces: `Command::Prompt { chat: String, text: String }` (word `prompt`, power `Prompt`,
  answer `Done`); `session::TURN_RUNNING: &str = "turn_running"`;
  `session::NOT_AN_AGENT: &str = "not_an_agent"`; `Chat { …, workspace: Option<String>, level: Option<u8> }`;
  `Client::prompt(&self, chat: &str, text: &str) -> Result<(), CallError>`;
  `session::VERSION == Version { major: 1, minor: 1 }`.

- [ ] **Step 1: Record the new frames, so the promise test fails first**

Append to `crates/session-protocol/tests/fixtures/session-protocol.jsonl`:

```
{"call":{"id":12,"command":"prompt","chat":"01J9ZQ3V7K8M2N4P6R8T0V2X4Z","text":"also add a test for expiry"}}
{"reply":{"re":12,"refused":{"code":"turn_running","why":"the agent is mid-turn; stop it, or wait"}}}
{"reply":{"re":1,"ok":{"chats":[{"chat":"01J9ZQ3V7K8M2N4P6R8T0V2X4Z","project":"01J9PRJCT00000000000000000","project_path":"/home/me/project","title":"fix the login","state":"working","workspace":"login","level":2},{"chat":"01J9ZQ5000000000000000000A","project":"01J9PRJCT00000000000000000","state":"idle","level":1}]}}}
```

- [ ] **Step 2: Run the promise test to see it fail**

Run: `cargo test -p purlis-session-protocol --test the_session_protocol_keeps_every_frame_it_promised 2>&1 | grep -E "panicked|no longer|not recorded|test result" | head`
Expected: `every_recorded_frame_reads_back_as_it_was_recorded` FAILS on the `prompt` line
("no longer reads": unknown variant `prompt`) and on the `chats` line (`workspace` is dropped
on re-serialisation, so it "no longer writes as it was recorded").

- [ ] **Step 3: Add the command, the fields, the codes and the client method**

In `session.rs`:

Change `VERSION` to `Version { major: 1, minor: 1 }` and its doc to "The session protocol's
current version. 1.1 added `prompt`, `attach_timeline`/`detach_timeline`, and `workspace` and
`level` on a listed chat (iOS design §5)."

After `NOT_ALLOWED` (line 97) add:

```rust
/// A `prompt` to an agent whose turn is still running: stop it, or wait (iOS design §5.2).
pub const TURN_RUNNING: &str = "turn_running";
/// A `prompt` to a chat that is not an agent's: a shell tab takes `write` only (iOS design
/// §5.2).
pub const NOT_AN_AGENT: &str = "not_an_agent";
```

In `enum Command`, after `Write { … }` add:

```rust
    /// A turn's worth of text for the chat's agent: handed to it whole, the way a person's
    /// message is, never typed byte by byte. A level-3 chat takes it as its next prompt and
    /// refuses with [`TURN_RUNNING`] while a turn runs; a level-2 chat takes it as one
    /// bracketed paste and Enter; a shell tab refuses with [`NOT_AN_AGENT`] (iOS design §5.2).
    Prompt { chat: String, text: String },
```

In `struct Chat`, after `state` add:

```rust
    /// Its workspace's name, or absent for the project's root: what [`Start::workspace`]
    /// carries (iOS design §5.5). A host from before 1.1 omits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// How much the host knows of it (ADR 0073): 1 a shell tab, 2 a chat its hooks report on,
    /// 3 a chat that speaks ACP. A host from before 1.1 omits it, and a client draws such a
    /// chat as a terminal (iOS design §5.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
```

In `wire_words!(Command { … })` add `Prompt => "prompt",` after `Write => "write",`. In
`Command::power` add `Command::Prompt { .. } => Power::Prompt,` after the `Write` arm.

In `impl Client`, after `write` add:

```rust
    /// Hand `text` to `chat`'s agent as its next prompt ([`Command::Prompt`]).
    pub async fn prompt(&self, chat: &str, text: &str) -> Result<(), CallError> {
        self.done(Command::Prompt {
            chat: chat.into(),
            text: text.into(),
        })
        .await
    }
```

Every `Chat { … }` literal in the crate's tests and bins now needs `workspace: None, level: None`:
`each_scope_may_call_only_what_its_row_grants.rs:46`, and whatever
`grep -rn "state: " crates/session-protocol --include=*.rs | grep -v "src/session.rs"` lists
(`a_client_speaks_the_session_protocol_to_the_host.rs`, `src/bin/charter-session-peer.rs`,
`src/bin/charter-session-bench.rs` among them). Add the two fields to each.

- [ ] **Step 4: Add the words to the table test and the code list**

In `each_scope_may_call_only_what_its_row_grants.rs`: in `every_command` add, after the `Write`
entry,

```rust
        Command::Prompt {
            chat: CHAT.into(),
            text: "also add a test for expiry".into(),
        },
```

and in `granted` add `"prompt"` after `"write"` in the `LocalUi` and `RemoteDevice` rows only.

In `the_session_protocol_keeps_every_frame_it_promised.rs`, the snake_case test's code list
(line ~150) gains `session::TURN_RUNNING, session::NOT_AN_AGENT,`.

- [ ] **Step 5: Run both test files**

Run: `cargo test -p purlis-session-protocol --test the_session_protocol_keeps_every_frame_it_promised --test each_scope_may_call_only_what_its_row_grants 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: both `test result: ok`.

- [ ] **Step 6: The whole crate, clippy, fmt, commit**

Run: `cargo test -p purlis-session-protocol 2>&1 | grep -E "^test result|FAILED" | sort | uniq -c`
Expected: only `ok` lines. If `the_version_is_negotiated_and_an_unknown_major_is_refused` or
another test asserts `session::VERSION`'s minor, it is wrong by the spec and moves to
`session::VERSION`; those that build their own `Speaks::new([Version { major: 1, minor: 0 }])`
are unaffected.

```bash
cargo fmt -p purlis-session-protocol && cargo clippy -p purlis-session-protocol --all-targets -- -D warnings
git add crates/session-protocol && git commit -q -m "Session protocol 1.1: prompt, and a listed chat's workspace and level"
env -u GIT_CONFIG_COUNT -u GIT_CONFIG_KEY_0 -u GIT_CONFIG_VALUE_0 git commit -q --amend -m "Session protocol 1.1: prompt, and a listed chat's workspace and level"
```

---

### Task 4: The timeline's items on the wire

**Files:**
- Create: `crates/session-protocol/src/timeline.rs` (the items only, in this task)
- Modify: `crates/session-protocol/src/lib.rs` (`pub mod timeline;`, and a paragraph in the layer list)
- Test: `crates/session-protocol/src/timeline.rs` (unit tests)

**Interfaces:**
- Consumes: nothing earlier.
- Produces: `timeline::Item { level: u8, what: What }` with `What` an `#[serde(tag = "item", rename_all = "snake_case")]` enum:
  `Turn { turn: String, began: bool, stop_reason: Option<String> }`,
  `Message { turn: String, role: Role, text: String, done: bool }`,
  `Tool { tool: String, title: String, kind: String, status: String, duration_ms: Option<u64> }`,
  `Plan { steps: Vec<Step> }`, `Ask { ask: String, summary: String, choices: Vec<Choice>, elicits_secret: bool }`,
  `AskClosed { ask: String, by: String }`, `Usage { input_tokens: u64, output_tokens: u64 }`,
  `CaughtUp`; `Role { User, Agent }`; `Step { text: String, status: String }`;
  `Choice { id: String, label: String, lasts: bool }`; `timeline::TIMELINE: u8 = 0x02`.

- [ ] **Step 1: Write the failing unit tests**

Create `crates/session-protocol/src/timeline.rs` with the module doc and the tests first:

```rust
//! A timeline: one chat's story to one client, on a stream of its own (iOS design §5.1).
//!
//! What a view is for a terminal's bytes, a timeline is for what the host knows of a chat
//! without reading its output (AGENTS.md: nothing parses harness output): a turn began or ended,
//! a tool ran, the agent asked something and who answered, and, for a chat that speaks ACP, its
//! messages and plan. Every item carries the chat's `level` (ADR 0073), so a client knows how
//! much to expect.
//!
//! **On the wire,** the host opens the stream and writes a header ([`TIMELINE`], then the
//! timeline's number and its epoch, each a big-endian `u32`), then one length-delimited JSON
//! object per item: first the bounded snapshot of what it still holds, ended by
//! [`What::CaughtUp`], then each item as it happens. The client writes nothing back: Yamux's
//! per-stream credit is the flow control, so a slow phone throttles only its own stream, and a
//! client that stops reading past the host's queue bound is dropped ([`Feed::push`]) and
//! attaches again, as a view is (V76).
//!
//! **Message text lives only here and in the host's memory** (D9): it is never written to the
//! event log or to disk for this feature, and this crate logs nothing.

use serde::{Deserialize, Serialize};

/// What opens a timeline's stream.
pub const TIMELINE: u8 = 0x02;

/// One item of a chat's timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// How much the host knows of the chat: 2 from its hooks, 3 from ACP (ADR 0073).
    pub level: u8,
    #[serde(flatten)]
    pub what: What,
}

/// Which kind of item, each with what it carries. Unknown fields are ignored, so a later minor
/// may add some; an unknown `item` word fails to read, and a client passes such an item over
/// ([`crate::timeline::Reader::next`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "item", rename_all = "snake_case")]
pub enum What {
    /// A turn began (`began`), or ended with `stop_reason` (`end_turn`, `cancelled`, …, the
    /// harness's own word).
    Turn {
        turn: String,
        began: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stop_reason: Option<String>,
    },
    /// A chunk of a message's text; `done` on its last chunk. Level 3 only.
    Message {
        turn: String,
        role: Role,
        text: String,
        done: bool,
    },
    /// A tool call: at level 3 with the agent's title, at level 2 its name, verdict and timing
    /// only (ruling O3: a digest, never the arguments).
    Tool {
        tool: String,
        title: String,
        kind: String,
        status: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    /// The agent's plan, whole, each time it changes. Level 3 only.
    Plan { steps: Vec<Step> },
    /// The agent asked something: the source's own summary and choices, in its order and
    /// words. A choice that lasts beyond the session is marked, so a phone can leave it out
    /// (D8); a secret's ask has no answerable choice from a phone (FD-27).
    Ask {
        ask: String,
        summary: String,
        choices: Vec<Choice>,
        elicits_secret: bool,
    },
    /// The ask was answered, by `by`: `desktop`, `device`, or the harness's own word when it
    /// closed the ask itself.
    AskClosed { ask: String, by: String },
    /// Tokens so far. Level 3 only.
    Usage {
        input_tokens: u64,
        output_tokens: u64,
    },
    /// The snapshot is over: everything after this happened after the attach.
    CaughtUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub text: String,
    /// `pending`, `in_progress`, `completed`: ACP's words.
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub id: String,
    pub label: String,
    /// Whether choosing it writes a rule that outlasts the session.
    pub lasts: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_item_is_one_flat_object_tagged_by_its_kind() {
        let item = Item {
            level: 2,
            what: What::Tool {
                tool: "t1".into(),
                title: "cargo test".into(),
                kind: "execute".into(),
                status: "completed".into(),
                duration_ms: Some(1200),
            },
        };
        assert_eq!(
            serde_json::to_value(&item).unwrap(),
            json!({"level": 2, "item": "tool", "tool": "t1", "title": "cargo test",
                   "kind": "execute", "status": "completed", "duration_ms": 1200})
        );
    }

    #[test]
    fn caught_up_carries_only_its_level() {
        let item = Item { level: 3, what: What::CaughtUp };
        assert_eq!(
            serde_json::to_value(&item).unwrap(),
            json!({"level": 3, "item": "caught_up"})
        );
    }

    #[test]
    fn a_field_from_a_later_minor_is_ignored_and_an_unknown_item_does_not_read() {
        let read: Item = serde_json::from_value(
            json!({"level": 2, "item": "turn", "turn": "u1", "began": true, "mood": "calm"}),
        )
        .unwrap();
        assert_eq!(
            read.what,
            What::Turn { turn: "u1".into(), began: true, stop_reason: None }
        );
        assert!(serde_json::from_value::<Item>(json!({"level": 2, "item": "mood"})).is_err());
    }

    #[test]
    fn every_word_is_snake_case() {
        let items = [
            What::Turn { turn: "u".into(), began: false, stop_reason: Some("end_turn".into()) },
            What::Message { turn: "u".into(), role: Role::Agent, text: "hi".into(), done: true },
            What::Plan { steps: vec![Step { text: "a".into(), status: "pending".into() }] },
            What::Ask {
                ask: "a".into(),
                summary: "Allow cargo test?".into(),
                choices: vec![Choice { id: "allow".into(), label: "Allow".into(), lasts: false }],
                elicits_secret: false,
            },
            What::AskClosed { ask: "a".into(), by: "desktop".into() },
            What::Usage { input_tokens: 1, output_tokens: 2 },
            What::CaughtUp,
        ];
        for what in items {
            let value = serde_json::to_value(Item { level: 3, what }).unwrap();
            let mut keys = vec![];
            collect_keys(&value, &mut keys);
            for key in keys {
                assert!(
                    key.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                    "{key} in {value}"
                );
            }
            let word = value["item"].as_str().unwrap();
            assert!(word.chars().all(|c| c.is_ascii_lowercase() || c == '_'), "{word}");
        }
    }

    fn collect_keys(value: &serde_json::Value, into: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, inner) in fields {
                    into.push(key.clone());
                    collect_keys(inner, into);
                }
            }
            serde_json::Value::Array(items) => items.iter().for_each(|i| collect_keys(i, into)),
            _ => {}
        }
    }
}
```

- [ ] **Step 2: Register the module and run the tests**

In `lib.rs` add `pub mod timeline;` after `pub mod session;` (alphabetical among the `pub mod`
lines), and in the layer list after item 4 (`view`) add:

```rust
//! 4b. [`timeline`]: a chat's story to one client, items the host knows without reading the
//!    chat's output, on a stream of its own (iOS design §5.1). Yamux's credit is its flow
//!    control; a client that stops reading past the queue bound is dropped and attaches again.
```

Run: `cargo test -p purlis-session-protocol --lib timeline 2>&1 | tail -12`
Expected: PASS, 4 tests. (They are written together with the type here because the type *is*
the wire format; the first run is the one that shows them green. If any fails, the type is
wrong, not the test.)

- [ ] **Step 3: Clippy, fmt, commit**

```bash
cargo fmt -p purlis-session-protocol && cargo clippy -p purlis-session-protocol --all-targets -- -D warnings
git add crates/session-protocol && git commit -q -m "Session protocol: what a timeline's items say on the wire"
env -u GIT_CONFIG_COUNT -u GIT_CONFIG_KEY_0 -u GIT_CONFIG_VALUE_0 git commit -q --amend -m "Session protocol: what a timeline's items say on the wire"
```

---

### Task 5: Which kind of stream the host opened

**Files:**
- Create: `crates/session-protocol/src/stream.rs`
- Modify: `crates/session-protocol/src/lib.rs` (`pub mod stream;`)
- Modify: `crates/session-protocol/src/view.rs:384-404` (`accept` reads an `Opened`), `:515-533` (`Viewer::accept_opened`)
- Test: `crates/session-protocol/tests/a_client_tells_a_view_from_a_timeline_by_the_streams_first_byte.rs`

**Interfaces:**
- Consumes: `view::VIEW`, `timeline::TIMELINE`.
- Produces: `stream::Kind { View, Timeline }`; `stream::Opened { pub fn kind(&self) -> Kind }`;
  `stream::take(stream: link::Stream) -> Result<Opened, LinkError>`;
  `view::Viewer::accept_opened(&mut self, opened: Opened) -> Result<Reader, LinkError>`;
  `view::Viewer::accept(&mut self, stream)` unchanged in signature (it now calls the two).

- [ ] **Step 1: Write the failing test**

Create `crates/session-protocol/tests/a_client_tells_a_view_from_a_timeline_by_the_streams_first_byte.rs`:

```rust
//! A host opens a view or a timeline on a stream of its own, and the client learns which from
//! the stream's first byte, before anything else of it is read.

use bytes::Bytes;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link::{self, Link};
use purlis_session_protocol::stream::{self, Kind};
use purlis_session_protocol::view::{Attacher, Limits, ViewId, Viewer};
use tokio::io::{AsyncWriteExt, duplex};

mod common;
use common::HELD;

async fn linked() -> (Link, Link) {
    let (a, b) = duplex(256 * 1024);
    let (client, host) = tokio::join!(
        link::connect(
            a,
            purlis_session_protocol::session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, purlis_session_protocol::session::speaks(), &HELD)
    );
    (client.unwrap(), host.unwrap())
}

#[tokio::test]
async fn a_view_is_told_by_its_first_byte_and_still_reads_whole() {
    let (mut client, host) = linked().await;
    let _feed = Attacher::new(host.opener(), Limits::default())
        .attach(ViewId(4), Bytes::from_static(b"snap"))
        .await
        .unwrap();
    let opened = stream::take(client.accept().await.unwrap()).await.unwrap();
    assert_eq!(opened.kind(), Kind::View);
    let mut reader = Viewer::default().accept_opened(opened).await.unwrap();
    assert_eq!(reader.view(), ViewId(4));
    let chunk = reader.next().await.unwrap().unwrap();
    assert_eq!(chunk, purlis_session_protocol::view::Chunk::Snapshot(Bytes::from_static(b"snap")));
}

#[tokio::test]
async fn a_stream_of_an_unknown_kind_is_refused_by_name() {
    let (mut client, host) = linked().await;
    let opener = host.opener();
    tokio::spawn(async move {
        let mut s = opener.open().await.unwrap();
        s.write_all(&[0x7f, 0, 0, 0, 1]).await.unwrap();
        s.flush().await.unwrap();
        // Hold it open: the client refuses on the byte, not on the close.
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        drop(s);
    });
    let refused = stream::take(client.accept().await.unwrap()).await.unwrap_err();
    let text = refused.to_string();
    assert!(text.contains("0x7f"), "{text}");
}

#[tokio::test]
async fn a_timeline_byte_is_told_apart_from_a_view() {
    let (mut client, host) = linked().await;
    let opener = host.opener();
    tokio::spawn(async move {
        let mut s = opener.open().await.unwrap();
        s.write_all(&[purlis_session_protocol::timeline::TIMELINE])
            .await
            .unwrap();
        s.flush().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        drop(s);
    });
    let opened = stream::take(client.accept().await.unwrap()).await.unwrap();
    assert_eq!(opened.kind(), Kind::Timeline);
    // Handed to the viewer anyway, it is refused rather than read as a view.
    let refused = Viewer::default().accept_opened(opened).await.unwrap_err();
    assert!(refused.to_string().contains("not a view"), "{refused}");
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p purlis-session-protocol --test a_client_tells_a_view_from_a_timeline_by_the_streams_first_byte 2>&1 | tail -8`
Expected: FAIL to compile: `could not find stream in purlis_session_protocol`.

- [ ] **Step 3: Write `stream.rs`**

```rust
//! Which kind of stream the host opened: the first byte says, and the client reads it before
//! anything else of the stream.
//!
//! A view ([`crate::view`]) and a timeline ([`crate::timeline`]) ride the same link, each on a
//! stream of its own, and the host opens them in any order. The client takes each stream the
//! link accepts, reads its kind here, and hands it on to the module that reads the rest.

use tokio::io::AsyncReadExt;

use crate::link::{LinkError, Stream};
use crate::timeline::TIMELINE;
use crate::view::VIEW;

/// What a stream carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A terminal's bytes ([`crate::view`]).
    View,
    /// A chat's story ([`crate::timeline`]).
    Timeline,
}

/// A stream whose kind has been read, and nothing else.
#[derive(Debug)]
pub struct Opened {
    kind: Kind,
    stream: Stream,
}

impl Opened {
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The stream, with its first byte gone: for the module its kind names.
    pub(crate) fn into_stream(self) -> Stream {
        self.stream
    }
}

/// Read a stream's kind. A first byte this version does not know is refused with its value,
/// and the stream is dropped: it is from a later version, and nothing here can read it.
pub async fn take(mut stream: Stream) -> Result<Opened, LinkError> {
    let kind = match stream.read_u8().await? {
        VIEW => Kind::View,
        TIMELINE => Kind::Timeline,
        other => {
            return Err(LinkError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("a stream that opens with {other:#04x} is neither a view nor a timeline"),
            )));
        }
    };
    Ok(Opened { kind, stream })
}
```

Add `pub mod stream;` to `lib.rs` (after `pub mod session;`, before `pub mod timeline;`).

- [ ] **Step 4: Let `view` read an `Opened`**

In `view.rs`, replace `async fn accept(stream: Stream)` (lines 384–404) with:

```rust
/// Take a view the host opened: read its header past the kind byte.
async fn accept(opened: crate::stream::Opened) -> Result<Reader, LinkError> {
    if opened.kind() != crate::stream::Kind::View {
        return Err(LinkError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("a {:?} stream is not a view", opened.kind()),
        )));
    }
    let (mut input, acks) = tokio::io::split(opened.into_stream());
    let view = ViewId(input.read_u32().await?);
    let epoch = Epoch(input.read_u32().await?);
    let snapshot_left = input.read_u32().await? as usize;
    Ok(Reader {
        view,
        epoch,
        snapshot_left,
        input,
        acks,
    })
}
```

And in `impl Viewer` replace `accept` with:

```rust
    /// Take a view the host opened, reading its kind first ([`crate::stream::take`]).
    pub async fn accept(&mut self, stream: Stream) -> Result<Reader, LinkError> {
        self.accept_opened(crate::stream::take(stream).await?).await
    }

    /// Take a view whose kind the client already read.
    pub async fn accept_opened(
        &mut self,
        opened: crate::stream::Opened,
    ) -> Result<Reader, LinkError> {
        let reader = accept(opened).await?;
        if let Some(last) = self.seen.get(&reader.view)
            && reader.epoch <= *last
        {
            return Err(LinkError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "view {} at epoch {}, which is not newer than {}",
                    reader.view.0, reader.epoch.0, last.0
                ),
            )));
        }
        self.seen.insert(reader.view, reader.epoch);
        Ok(reader)
    }
```

The module doc's "On the wire" sentence stays true (the header still opens with `0x01`); add
"The kind byte is what [`crate::stream::take`] reads." after it.

- [ ] **Step 5: Run the new test and every view test**

Run: `cargo test -p purlis-session-protocol --test a_client_tells_a_view_from_a_timeline_by_the_streams_first_byte 2>&1 | tail -8 && cargo test -p purlis-session-protocol 2>&1 | grep -E "^test result|FAILED" | sort | uniq -c`
Expected: the three new tests PASS; every other file still `ok` (the twelve `Viewer::default().accept(...)` call sites compile unchanged).

- [ ] **Step 6: Clippy, fmt, commit**

```bash
cargo fmt -p purlis-session-protocol && cargo clippy -p purlis-session-protocol --all-targets -- -D warnings
git add crates/session-protocol && git commit -q -m "Session protocol: a client tells a view from a timeline by the stream's first byte"
env -u GIT_CONFIG_COUNT -u GIT_CONFIG_KEY_0 -u GIT_CONFIG_VALUE_0 git commit -q --amend -m "Session protocol: a client tells a view from a timeline by the stream's first byte"
```

---

### Task 6: The timeline stream, and `attach_timeline`/`detach_timeline`

**Files:**
- Modify: `crates/session-protocol/src/timeline.rs` (the stream: `Feed`, `Attacher`, `Reader`, `Timelines`)
- Modify: `crates/session-protocol/src/session.rs` (`Command::{AttachTimeline, DetachTimeline}`, `Answer::TimelineAttached`, words, powers, `Client::{attach_timeline, detach_timeline}`)
- Modify: `crates/session-protocol/tests/fixtures/session-protocol.jsonl`
- Modify: `crates/session-protocol/tests/each_scope_may_call_only_what_its_row_grants.rs` (`every_command`, `granted`, `Board::call`)
- Test: `crates/session-protocol/tests/a_timeline_is_a_snapshot_then_each_item_as_it_happens.rs`

**Interfaces:**
- Consumes: `timeline::{Item, What, TIMELINE}` (Task 4), `stream::{Opened, Kind, take}` (Task 5), `Power::Timeline` (Task 1), `link::{Opener, Stream}`.
- Produces:
  - `timeline::TimelineId(pub u32)`, `timeline::Epoch(pub u32)` (its own, not the view's);
  - `timeline::Limits { most_queued_bytes: usize }` (`Default` 256 KiB);
  - `timeline::Feed::push(&self, item: &Item) -> Result<(), Closed>`, `Feed::queued_bytes`, `Feed::closed`; `timeline::Closed::FellBehind`;
  - `timeline::Attacher::new(opener: Opener, limits: Limits)`, `Attacher::attach(&self, timeline: TimelineId, snapshot: Vec<Item>) -> Result<Feed, LinkError>` (writes the header, the snapshot items, then `CaughtUp` at the chat's level, then what is pushed);
  - `timeline::Reader::{timeline(), epoch(), next(&mut self) -> Option<Result<Item, LinkError>>}`;
  - `timeline::Timelines::accept_opened(&mut self, opened: Opened) -> Result<Reader, LinkError>` and `Timelines::accept(&mut self, stream)`;
  - `Command::AttachTimeline { chat }` (word `attach_timeline`, power `Timeline`), `Command::DetachTimeline { chat, timeline: u32 }` (word `detach_timeline`, power `Timeline`), `Answer::TimelineAttached(TimelineAttached { timeline: u32, level: u8 })` (word `timeline_attached`);
  - `Client::attach_timeline(&self, chat) -> Result<TimelineAttached, CallError>`, `Client::detach_timeline(&self, chat, timeline: u32) -> Result<(), CallError>`.

- [ ] **Step 1: Record the frames first**

Append to `fixtures/session-protocol.jsonl`:

```
{"call":{"id":13,"command":"attach_timeline","chat":"01J9ZQ3V7K8M2N4P6R8T0V2X4Z"}}
{"reply":{"re":13,"ok":{"timeline_attached":{"timeline":2,"level":2}}}}
{"call":{"id":14,"command":"detach_timeline","chat":"01J9ZQ3V7K8M2N4P6R8T0V2X4Z","timeline":2}}
```

Run: `cargo test -p purlis-session-protocol --test the_session_protocol_keeps_every_frame_it_promised 2>&1 | grep -E "panicked|no longer|test result" | head -5`
Expected: FAIL: the `attach_timeline` call "no longer reads" (unknown variant).

- [ ] **Step 2: Add the commands and the answer to `session.rs`**

In `enum Command`, after `Detach { … }`:

```rust
    /// Open a timeline of a chat: the host opens a stream for it ([`crate::timeline`]), whose
    /// snapshot then live items follow, and answers [`Answer::TimelineAttached`].
    AttachTimeline { chat: String },
    /// Close a timeline the client attached. The chat keeps running.
    DetachTimeline { chat: String, timeline: u32 },
```

In `enum Answer`, after `Attached(Attached)`:

```rust
    /// The timeline it opened: [`Command::AttachTimeline`].
    TimelineAttached(TimelineAttached),
```

After `struct Attached`:

```rust
/// The timeline an attach opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineAttached {
    /// The timeline's number, which its stream's header carries ([`crate::timeline::TimelineId`]).
    pub timeline: u32,
    /// The chat's level (ADR 0073): what kinds of item to expect.
    pub level: u8,
}
```

Words: `AttachTimeline => "attach_timeline", DetachTimeline => "detach_timeline",` after
`Detach => "detach",`; `TimelineAttached => "timeline_attached",` after `Attached => "attached",`.
Power: `Command::AttachTimeline { .. } | Command::DetachTimeline { .. } => Power::Timeline,`.

Client methods, after `detach`:

```rust
    pub async fn attach_timeline(&self, chat: &str) -> Result<TimelineAttached, CallError> {
        match self.call(Command::AttachTimeline { chat: chat.into() }).await? {
            Answer::TimelineAttached(attached) => Ok(attached),
            other => Err(unexpected(&other)),
        }
    }

    pub async fn detach_timeline(&self, chat: &str, timeline: u32) -> Result<(), CallError> {
        self.done(Command::DetachTimeline {
            chat: chat.into(),
            timeline,
        })
        .await
    }
```

In `each_scope_may_call_only_what_its_row_grants.rs`: `every_command` gains, after `Detach`,

```rust
        Command::AttachTimeline { chat: CHAT.into() },
        Command::DetachTimeline {
            chat: CHAT.into(),
            timeline: 2,
        },
```

`Board::call` gains `Command::AttachTimeline { .. } => Answer::TimelineAttached(session::TimelineAttached { timeline: 2, level: 2 }),`
and `granted` gains `"attach_timeline", "detach_timeline",` after `"detach",` in the `LocalUi`
and `RemoteDevice` rows only.

Run: `cargo test -p purlis-session-protocol --test the_session_protocol_keeps_every_frame_it_promised --test each_scope_may_call_only_what_its_row_grants 2>&1 | grep -E "test result|FAILED"`
Expected: both `ok`.

- [ ] **Step 3: Write the failing stream test**

Create `crates/session-protocol/tests/a_timeline_is_a_snapshot_then_each_item_as_it_happens.rs`:

```rust
//! A timeline is one stream: a header, the items the host still holds, `caught_up`, then each
//! item as it happens (iOS design §5.1). Yamux's credit paces it; past the host's queue bound
//! the client is dropped and attaches again, as a view is.

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link::{self, Link};
use purlis_session_protocol::session;
use purlis_session_protocol::stream;
use purlis_session_protocol::timeline::{
    Attacher, Closed, Epoch, Item, Limits, Role, TimelineId, Timelines, What,
};
use tokio::io::duplex;

mod common;
use common::HELD;

async fn linked() -> (Link, Link) {
    let (a, b) = duplex(256 * 1024);
    let (client, host) = tokio::join!(
        link::connect(a, session::speaks(), Scope::LocalUi, HELD.of(Scope::LocalUi)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    (client.unwrap(), host.unwrap())
}

fn turn(level: u8, id: &str, began: bool) -> Item {
    Item {
        level,
        what: What::Turn {
            turn: id.into(),
            began,
            stop_reason: (!began).then(|| "end_turn".to_owned()),
        },
    }
}

#[tokio::test]
async fn the_snapshot_comes_first_then_caught_up_then_the_live_items_in_order() {
    let (mut client, host) = linked().await;
    let held = vec![turn(2, "t1", true), turn(2, "t1", false)];
    let feed = Attacher::new(host.opener(), Limits::default())
        .attach(TimelineId(9), held.clone())
        .await
        .unwrap();
    feed.push(&turn(2, "t2", true)).unwrap();

    let mut reader = Timelines::default()
        .accept_opened(stream::take(client.accept().await.unwrap()).await.unwrap())
        .await
        .unwrap();
    assert_eq!((reader.timeline(), reader.epoch()), (TimelineId(9), Epoch(1)));
    let mut got = Vec::new();
    for _ in 0..4 {
        got.push(reader.next().await.unwrap().unwrap());
    }
    assert_eq!(
        got,
        [
            held[0].clone(),
            held[1].clone(),
            Item { level: 2, what: What::CaughtUp },
            turn(2, "t2", true)
        ]
    );
}

#[tokio::test]
async fn an_empty_snapshot_is_caught_up_at_once() {
    let (mut client, host) = linked().await;
    let _feed = Attacher::new(host.opener(), Limits::default())
        .attach(TimelineId(1), vec![])
        .await
        .unwrap();
    let mut reader = Timelines::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    // With nothing held, the level is not known from the items; the host says 0.
    assert_eq!(reader.next().await.unwrap().unwrap(), Item { level: 0, what: What::CaughtUp });
}

#[tokio::test]
async fn a_dropped_feed_ends_the_timeline_after_what_was_queued() {
    let (mut client, host) = linked().await;
    let feed = Attacher::new(host.opener(), Limits::default())
        .attach(TimelineId(1), vec![])
        .await
        .unwrap();
    feed.push(&turn(3, "t", true)).unwrap();
    drop(feed);
    let mut reader = Timelines::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    assert_eq!(reader.next().await.unwrap().unwrap().what, What::CaughtUp);
    assert_eq!(reader.next().await.unwrap().unwrap(), turn(3, "t", true));
    assert!(reader.next().await.is_none(), "the stream ended");
}

// Corrected after the final review: as written below, the push loop never awaits, so on the
// test runtime's single thread the writer never drains and the bound trips without the
// stream's credit ever being exercised; nothing is asserted on the client's side either. As
// built, `a_timeline_whose_client_stops_reading_is_dropped_at_its_bound_and_learns_it` keeps a
// `Reader`, yields between pushes, asserts `queued_bytes() <= most_queued_bytes` throughout,
// asserts far more pushes than the queue holds (the credit took them), and then reads the
// client to the end within 5 s; it was proven red by removing the cancel in `Shared::close`.
#[tokio::test]
async fn a_timeline_whose_client_stops_reading_is_dropped_at_its_bound() {
    let (mut client, host) = linked().await;
    let limits = Limits { most_queued_bytes: 4096 };
    let feed = Attacher::new(host.opener(), limits)
        .attach(TimelineId(1), vec![])
        .await
        .unwrap();
    // The client takes the stream and reads nothing of it.
    let _opened = stream::take(client.accept().await.unwrap()).await.unwrap();
    let chunk = Item {
        level: 3,
        what: What::Message {
            turn: "t".into(),
            role: Role::Agent,
            text: "x".repeat(1000),
            done: false,
        },
    };
    let mut pushed = 0;
    let dropped = loop {
        match feed.push(&chunk) {
            Ok(()) => pushed += 1,
            Err(why) => break why,
        }
        assert!(pushed < 10_000, "the queue never filled");
    };
    assert_eq!(dropped, Closed::FellBehind);
    assert_eq!(feed.closed(), Some(Closed::FellBehind));
    assert_eq!(feed.queued_bytes(), 0, "the queue was let go");
}

#[tokio::test]
async fn an_item_from_a_later_minor_is_passed_over() {
    use tokio::io::AsyncWriteExt;
    let (mut client, host) = linked().await;
    let opener = host.opener();
    tokio::spawn(async move {
        let mut s = opener.open().await.unwrap();
        let mut bytes = vec![purlis_session_protocol::timeline::TIMELINE];
        bytes.extend_from_slice(&5u32.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        for item in [
            r#"{"level":2,"item":"mood","mood":"calm"}"#,
            r#"{"level":2,"item":"caught_up"}"#,
        ] {
            bytes.extend_from_slice(&(item.len() as u32).to_be_bytes());
            bytes.extend_from_slice(item.as_bytes());
        }
        s.write_all(&bytes).await.unwrap();
        s.flush().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        drop(s);
    });
    let mut reader = Timelines::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    assert_eq!(reader.timeline(), TimelineId(5));
    assert_eq!(reader.next().await.unwrap().unwrap().what, What::CaughtUp);
}

#[tokio::test]
async fn a_timeline_attached_again_comes_back_at_the_next_epoch() {
    let (mut client, host) = linked().await;
    let attacher = Attacher::new(host.opener(), Limits::default());
    let first = attacher.attach(TimelineId(3), vec![]).await.unwrap();
    drop(first);
    let _second = attacher.attach(TimelineId(3), vec![]).await.unwrap();
    let mut timelines = Timelines::default();
    let a = timelines.accept(client.accept().await.unwrap()).await.unwrap();
    let b = timelines.accept(client.accept().await.unwrap()).await.unwrap();
    assert_eq!((a.epoch(), b.epoch()), (Epoch(1), Epoch(2)));
}
```

- [ ] **Step 4: Run it to see it fail**

Run: `cargo test -p purlis-session-protocol --test a_timeline_is_a_snapshot_then_each_item_as_it_happens 2>&1 | tail -8`
Expected: FAIL to compile: no `Attacher`, `Timelines`, … in `timeline`.

- [ ] **Step 5: Write the stream half of `timeline.rs`**

Append to `crates/session-protocol/src/timeline.rs` (above its `tests` module), with these
imports added at the top: `use std::collections::{HashMap, VecDeque}; use std::sync::{Arc, Mutex, MutexGuard, OnceLock}; use bytes::Bytes; use futures::{SinkExt, StreamExt}; use tokio::io::AsyncReadExt; use tokio::sync::Notify; use tokio_util::codec::{Framed, LengthDelimitedCodec}; use tokio_util::sync::CancellationToken; use crate::link::{LinkError, Opener, Stream}; use crate::stream::{Kind, Opened};`

```rust
/// The most one item may be on the wire. An ask's choices or a plan fit many times over; a
/// message chunk is cut by the host well below it.
pub const MOST_ITEM_BYTES: usize = 64 * 1024;

/// How much one timeline may hold queued before its client is dropped.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Bytes of items queued for the stream and not yet written. Past it, the client has fallen
    /// behind. Yamux's window is the in-flight bound; this is the host's memory bound.
    pub most_queued_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            most_queued_bytes: 256 << 10,
        }
    }
}

/// A timeline's number on a link: which chat's story it is, as the layer above says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimelineId(pub u32);

/// Which attachment of a timeline this is: every re-attach is the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Epoch(pub u32);

/// Why the host closed a timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Closed {
    /// The client could not keep up and was dropped ([`Feed::push`]).
    #[error("the timeline fell behind and was dropped; attach it again")]
    FellBehind,
}

#[derive(Default)]
struct Queue {
    items: VecDeque<Bytes>,
    bytes: usize,
    /// The [`Feed`] was dropped: write what is queued, then end the stream.
    gone: bool,
}

struct Shared {
    queue: Mutex<Queue>,
    more: Notify,
    /// Cancelled when the host closes the timeline: a writer blocked on the stream's credit
    /// stops where it is, as a view's does.
    dropped: CancellationToken,
    why: OnceLock<Closed>,
    most_queued: usize,
}

impl Shared {
    fn queue(&self) -> MutexGuard<'_, Queue> {
        self.queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The host's end of a timeline: where a chat's items go.
pub struct Feed {
    shared: Arc<Shared>,
}

impl Feed {
    /// Queue `item` for the stream, without waiting. Past [`Limits::most_queued_bytes`] the
    /// client has fallen behind: the queue is let go, the stream ends, and this and every later
    /// push answer [`Closed::FellBehind`]. The host then attaches the timeline again with a
    /// fresh snapshot at the next epoch.
    pub fn push(&self, item: &Item) -> Result<(), Closed> {
        let bytes = Bytes::from(serde_json::to_vec(item).expect("an item is always JSON"));
        let mut queue = self.shared.queue();
        if let Some(why) = self.shared.why.get() {
            return Err(*why);
        }
        if queue.bytes + bytes.len() > self.shared.most_queued {
            queue.items.clear();
            queue.bytes = 0;
            let why = *self.shared.why.get_or_init(|| Closed::FellBehind);
            drop(queue);
            self.shared.dropped.cancel();
            return Err(why);
        }
        queue.bytes += bytes.len();
        queue.items.push_back(bytes);
        drop(queue);
        self.shared.more.notify_one();
        Ok(())
    }

    /// Bytes queued and not yet written.
    pub fn queued_bytes(&self) -> usize {
        self.shared.queue().bytes
    }

    /// Why the host closed the timeline, once it has.
    pub fn closed(&self) -> Option<Closed> {
        self.shared.why.get().copied()
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.shared.queue().gone = true;
        self.shared.more.notify_one();
    }
}

fn framed(stream: Stream) -> Framed<Stream, LengthDelimitedCodec> {
    LengthDelimitedCodec::builder()
        .max_frame_length(MOST_ITEM_BYTES)
        .new_framed(stream)
}

/// Write the header, the snapshot, `caught_up`, then whatever is pushed, until the feed is
/// dropped or the client falls behind.
async fn write(
    stream: Stream,
    timeline: TimelineId,
    epoch: Epoch,
    snapshot: Vec<Item>,
    shared: Arc<Shared>,
) -> Result<(), LinkError> {
    use tokio::io::AsyncWriteExt;
    let mut stream = stream;
    let mut header = Vec::with_capacity(9);
    header.push(TIMELINE);
    header.extend_from_slice(&timeline.0.to_be_bytes());
    header.extend_from_slice(&epoch.0.to_be_bytes());
    stream.write_all(&header).await?;
    let level = snapshot.first().map(|item| item.level).unwrap_or(0);
    let mut out = framed(stream);
    let writing = async {
        for item in snapshot.iter().chain(std::iter::once(&Item {
            level,
            what: What::CaughtUp,
        })) {
            out.send(Bytes::from(serde_json::to_vec(item).expect("an item is always JSON")))
                .await?;
        }
        loop {
            let next = {
                let mut queue = shared.queue();
                match queue.items.pop_front() {
                    Some(item) => {
                        queue.bytes -= item.len();
                        Some(item)
                    }
                    None if queue.gone => return Ok::<(), LinkError>(()),
                    None => None,
                }
            };
            match next {
                Some(item) => out.send(item).await?,
                None => shared.more.notified().await,
            }
        }
    };
    tokio::select! {
        done = writing => done,
        () = shared.dropped.cancelled() => Ok(()),
    }
}

/// The host's timelines on one link: it numbers each timeline's attachments.
pub struct Attacher {
    opener: Opener,
    limits: Limits,
    epochs: Mutex<HashMap<TimelineId, Epoch>>,
}

impl Attacher {
    pub fn new(opener: Opener, limits: Limits) -> Self {
        Attacher {
            opener,
            limits,
            epochs: Mutex::new(HashMap::new()),
        }
    }

    /// Attach `timeline`, new or again, with `snapshot`, the items the host still holds in
    /// order: open its stream at the next epoch. The snapshot's first item's level is
    /// `caught_up`'s; an empty snapshot says level 0, and the layer above tells the client the
    /// level in its answer ([`crate::session::TimelineAttached`]).
    pub async fn attach(&self, timeline: TimelineId, snapshot: Vec<Item>) -> Result<Feed, LinkError> {
        let epoch = {
            let mut epochs = self
                .epochs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let next = match epochs.get(&timeline) {
                None => Epoch(1),
                Some(before) => before.0.checked_add(1).map(Epoch).ok_or_else(|| {
                    LinkError::Io(std::io::Error::other("a timeline attached 4 billion times"))
                })?,
            };
            epochs.insert(timeline, next);
            next
        };
        let stream = self.opener.open().await?;
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            more: Notify::new(),
            dropped: CancellationToken::new(),
            why: OnceLock::new(),
            most_queued: self.limits.most_queued_bytes,
        });
        tokio::spawn(write(stream, timeline, epoch, snapshot, Arc::clone(&shared)));
        Ok(Feed { shared })
    }
}

/// The client's end of a timeline.
pub struct Reader {
    timeline: TimelineId,
    epoch: Epoch,
    input: Framed<Stream, LengthDelimitedCodec>,
}

impl Reader {
    pub fn timeline(&self) -> TimelineId {
        self.timeline
    }

    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// The next item, or `None` when the host has ended the timeline. An item this version
    /// cannot read (a kind from a later minor) is passed over, as the session protocol passes
    /// over a frame kind it does not know.
    pub async fn next(&mut self) -> Option<Result<Item, LinkError>> {
        loop {
            let frame = match self.input.next().await? {
                Ok(frame) => frame,
                Err(e) => return Some(Err(e.into())),
            };
            if let Ok(item) = serde_json::from_slice::<Item>(&frame) {
                return Some(Ok(item));
            }
        }
    }
}

/// The client's timelines on one link: it takes each the host opens, and refuses an attachment
/// that is not newer than the last it saw of that timeline.
#[derive(Default)]
pub struct Timelines {
    seen: HashMap<TimelineId, Epoch>,
}

impl Timelines {
    /// Take a timeline the host opened, reading its kind first ([`crate::stream::take`]).
    pub async fn accept(&mut self, stream: Stream) -> Result<Reader, LinkError> {
        self.accept_opened(crate::stream::take(stream).await?).await
    }

    /// Take a timeline whose kind the client already read.
    pub async fn accept_opened(&mut self, opened: Opened) -> Result<Reader, LinkError> {
        if opened.kind() != Kind::Timeline {
            return Err(LinkError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("a {:?} stream is not a timeline", opened.kind()),
            )));
        }
        let mut stream = opened.into_stream();
        let timeline = TimelineId(stream.read_u32().await?);
        let epoch = Epoch(stream.read_u32().await?);
        if let Some(last) = self.seen.get(&timeline)
            && epoch <= *last
        {
            return Err(LinkError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "timeline {} at epoch {}, which is not newer than {}",
                    timeline.0, epoch.0, last.0
                ),
            )));
        }
        self.seen.insert(timeline, epoch);
        Ok(Reader {
            timeline,
            epoch,
            input: framed(stream),
        })
    }
}
```

`LinkError` must convert from `std::io::Error` (it does: `LinkError::Io`, used as `e.into()` in
`view.rs:366`). `futures` is already a dependency.

- [ ] **Step 6: Run the stream test and the whole crate**

Run: `cargo test -p purlis-session-protocol --test a_timeline_is_a_snapshot_then_each_item_as_it_happens 2>&1 | tail -12 && cargo test -p purlis-session-protocol 2>&1 | grep -E "^test result|FAILED" | sort | uniq -c`
Expected: 6 PASS; whole crate `ok`. If `a_timeline_whose_client_stops_reading_is_dropped_at_its_bound` hangs rather than fails, the writer task is blocked in `out.send` on Yamux credit with the queue still draining: that is the test's point, and the push loop must see `FellBehind` once the queue, not the window, fills (4096 bytes is four items; the window holds more, so the writer drains a few, then blocks, then the queue fills). If it fails, use superpowers:systematic-debugging; do not raise the bound to pass.

- [ ] **Step 7: Clippy, fmt, commit**

```bash
cargo fmt -p purlis-session-protocol && cargo clippy -p purlis-session-protocol --all-targets -- -D warnings
git add crates/session-protocol && git commit -q -m "Session protocol: a timeline stream beside the view, with attach_timeline and detach_timeline"
env -u GIT_CONFIG_COUNT -u GIT_CONFIG_KEY_0 -u GIT_CONFIG_VALUE_0 git commit -q --amend -m "Session protocol: a timeline stream beside the view, with attach_timeline and detach_timeline"
```

---

### Task 7: N−1: the new client against an older host

**Files:**
- Test: `crates/session-protocol/tests/a_1_1_client_does_without_what_a_1_0_host_lacks.rs`

**Interfaces:**
- Consumes: `Client::{prompt, attach_timeline}` (Tasks 3, 6), `Chat::{workspace, level}` (Task 3), `session::UNKNOWN_COMMAND`.
- Produces: nothing; it pins §5.6.

- [ ] **Step 1: Write the test**

```rust
//! N−1 for 1.1's additions (iOS design §5.6): a host from before 1.1 refuses `prompt` and
//! `attach_timeline` as unknown, lists its chats without `workspace` and `level`, and the
//! client carries on with the terminal view and raw `write`.
//!
//! The 1.0 host is played by hand on the control lane, from the frames
//! `fixtures/session-protocol.jsonl` recorded for 1.0, so this is what such a host really says.

use bytes::Bytes;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client};
use serde_json::{Value, json};
use tokio::io::duplex;

mod common;
use common::HELD;

/// A 1.0 host: it answers `list` with a 1.0 `chats` reply and every other word 1.0 knows with
/// `done`, and refuses what it does not know as 1.0's `serve` would.
async fn a_1_0_host(mut link: link::Link) {
    const KNOWN: &[&str] = &[
        "list", "attach", "detach", "write", "resize", "answer", "stop", "start", "subscribe",
        "unsubscribe",
    ];
    while let Some(Ok(frame)) = link.control().next().await {
        let call: Value = serde_json::from_slice(&frame).unwrap();
        let id = call["call"]["id"].as_u64().unwrap();
        let word = call["call"]["command"].as_str().unwrap();
        let reply = match word {
            "list" => json!({"reply": {"re": id, "ok": {"chats": [
                {"chat": "01J9ZQ3V7K8M2N4P6R8T0V2X4Z", "project": "01J9PRJCT00000000000000000",
                 "title": "fix the login", "state": "working"}
            ]}}}),
            known if KNOWN.contains(&known) => json!({"reply": {"re": id, "ok": "done"}}),
            unknown => json!({"reply": {"re": id, "refused": {
                "code": session::UNKNOWN_COMMAND,
                "why": format!("this host does not know the command `{unknown}`")}}}),
        };
        link.control()
            .send(Bytes::from(reply.to_string()))
            .await
            .unwrap();
    }
}

async fn client_of_a_1_0_host() -> Client {
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), Scope::Terminal, HELD.of(Scope::Terminal)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(a_1_0_host(served.unwrap()));
    let (client, _views, _events) = Client::new(client.unwrap());
    client
}

#[tokio::test]
async fn a_chat_listed_without_workspace_and_level_reads_with_neither() {
    let client = client_of_a_1_0_host().await;
    let chats = client.list().await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!((chats[0].workspace.as_deref(), chats[0].level), (None, None));
    assert_eq!(chats[0].title.as_deref(), Some("fix the login"));
}

#[tokio::test]
async fn prompt_is_refused_as_unknown_and_write_still_works() {
    let client = client_of_a_1_0_host().await;
    let refused = client.prompt("c", "hi").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::UNKNOWN_COMMAND)
    );
    client.write("c", b"hi\r").await.unwrap();
}

#[tokio::test]
async fn attach_timeline_is_refused_as_unknown_and_attach_still_works() {
    let client = client_of_a_1_0_host().await;
    let refused = client.attach_timeline("c").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::UNKNOWN_COMMAND)
    );
    // `attach` answers `done` from this stand-in, which the client reports as unexpected: the
    // call reached the host and came back, which is what N−1 promises.
    let answer = client.attach("c").await.unwrap_err();
    assert!(matches!(answer, session::CallError::Unexpected(_)), "{answer}");
}

#[tokio::test]
async fn the_1_0_frames_still_read_under_1_1() {
    // Every 1.0 frame in the fixture is also a 1.1 frame: the promise test holds them, and this
    // names the version so the two cannot drift apart.
    assert_eq!(session::VERSION.major, 1);
    assert!(session::VERSION.minor >= 1);
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p purlis-session-protocol --test a_1_1_client_does_without_what_a_1_0_host_lacks 2>&1 | tail -10`
Expected: PASS, 4 tests. (This is a characterisation test of Tasks 3 and 6; it was not
expected to fail first. If any fails, Tasks 3 or 6 are wrong.)

- [ ] **Step 3: Commit**

```bash
cargo fmt -p purlis-session-protocol && cargo clippy -p purlis-session-protocol --all-targets -- -D warnings
git add crates/session-protocol && git commit -q -m "Session protocol: a 1.1 client does without what a 1.0 host lacks"
env -u GIT_CONFIG_COUNT -u GIT_CONFIG_KEY_0 -u GIT_CONFIG_VALUE_0 git commit -q --amend -m "Session protocol: a 1.1 client does without what a 1.0 host lacks"
```

---

### Task 8: The whole workspace still builds, and the docs say what changed

**Files:**
- Modify: `docs/superpowers/specs/2026-10-11-ios-mobile-design.md` §7 (the Phase 1 row gains a "1A done" note), §5.1 (the item words as built, if they differ)
- Modify: `~/purlis-research/README.md` §10

**Interfaces:** none.

- [ ] **Step 1: Build and test what depends on the crate**

Run: `cd ~/purlis-research/purlis && cargo build --workspace 2>&1 | tail -3 && cargo test -p purlis-core --no-run 2>&1 | tail -2 && grep -rn "session_protocol::session::Chat\b\|session::Chat {" --include='*.rs' crates app | grep -v "crates/session-protocol" | head`
Expected: the workspace builds; the grep shows every `Chat { … }` literal outside the crate
(expected none today; any shown needs `workspace: None, level: None` and a commit "Session
protocol 1.1: the two new fields on a listed chat, where the app builds one").

Run: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3`
Expected: clean.

- [ ] **Step 2: Note the state in the spec and the dossier**

In the spec's §7 Phase 1 row, prepend to "Delivers": **"1A, the protocol additions, is built:
`docs/superpowers/plans/2026-10-12-ios-phase-1a-protocol.md`."** In §5.1, if the item words
built differ from the table (they add `ask_closed`'s `by`, `caught_up`, and `message`'s `done`),
add one sentence after the table: "As built: each item is one flat object tagged `item`, with
`caught_up` ending the snapshot; `crates/session-protocol/src/timeline.rs` is the record."

In `~/purlis-research/README.md` §10 add a row "Phase 1A" with the plan path, "done" and the
commit list (`git log --oneline main..ios-mobile-design | head`), and say the next plan is 1B.

- [ ] **Step 3: Commit the clone's doc change (the dossier is not a git repo)**

```bash
cd ~/purlis-research/purlis && git add docs/superpowers/specs/2026-10-11-ios-mobile-design.md && git commit -q -m "Design: Phase 1A built; the timeline's items as built"
env -u GIT_CONFIG_COUNT -u GIT_CONFIG_KEY_0 -u GIT_CONFIG_VALUE_0 git commit -q --amend -m "Design: Phase 1A built; the timeline's items as built"
git log --oneline main..ios-mobile-design | head -12
```
Expected: the eight commits of this plan on top of the design commits, none with an
`Assisted-by` trailer (`git log main..ios-mobile-design --format=%B | grep -c Assisted-by` prints `0`).

---

## Self-review (done while writing)

- **Spec coverage (§4.3, §5, §8 Protocol):** scope and powers → Task 1; `choice_lasts` failing
  closed → Task 2; `prompt`, `turn_running`, `not_an_agent` → Task 3 (the routing by level is the
  host's, 1B); `workspace`/`level` → Task 3; `timeline` with every item kind and `level` on each
  → Tasks 4 and 6; per-stream credit and re-attach on a gap → Task 6; N−1 for all four → Task 7
  plus the fixture. Not here, on purpose: a `remote-device` link's Noise admission (1B), the UI
  RPC refusal for `remote-device` (`Power::UiRpc` is already refused for every scope but
  `local-ui`, and `the_ui_rpc_rides_beside_the_session_protocol_for_one_build.rs` covers it).
- **Placeholders:** none; every step has its code.
- **Type consistency:** `TimelineAttached { timeline: u32, level: u8 }` in Task 6 is what Task 6's
  `Board::call` returns and what 1B's host will fill; `Opened::into_stream` is `pub(crate)` and
  used only by `view` and `timeline`; `Scope::has_a_credential` gates `stand_in_as` and the test
  `linked`'s branch alike.
- **Review Focus:** each of the five names its test and task above.
