# iOS Phase 1C: the desktop's wiring — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The desktop serves a paired phone. In `purlis-core`: the gateway's and keep-awake's
keys in `machine.json` (their row first) and a `remote-device` answerer for asks. In the app:
the adapters that put 1B's gateway over 1B's desk and peers, a `session::Host` over the app's
`SessionHost`/`ChatBoard` seam (list, view, write, prompt, answer, stop, timeline, subscribe),
the audit of a device's acts, the listener's lifecycle, keep-awake, and the commands the
window will call in 1D. No window pixel changes here.

**Architecture:** Core stays plain: two keys on `machine::Store` and one variant on the asks'
`Admitted`. The app gains one module, `remote`, in four files: `adapters.rs` (the crate's
`Pins`, `PairingDesk`, revocation and `Heard` over core's `peers`, `Desk` and `audit`),
`board.rs` (the `session::Host`: it holds the app's `Planes` through a small trait so tests
build it on `Planes::telling` and the pretend host, as `lib.rs`'s tests do), `timeline.rs`
(level-2 items from the event log's subscription and the hooks' asks; never from terminal
bytes), `gateway.rs` (the managed state: identity, desk, listener task, keep-awake, and the
Tauri commands), `awake.rs` (a `caffeinate`/`systemd-inhibit` child while a chat works). The
protocol crate becomes an ordinary dependency of the app with its `noise` feature; tokio gains
`net`.

**Tech Stack:** Rust 1.99, tauri 2 (`tauri::async_runtime`), tokio (`net` added), the
protocol crate's `noise`/`pairing`/`gateway`, `purlis-core::remote`, `tauri-specta` commands
(`ipc_commands.rs`).

**Spec:** `docs/superpowers/specs/2026-10-11-ios-mobile-design.md` §3, §4.1 (the keys), §4.3
(what the host answers), §4.4 (audit), §5.1 (level-2 items), §5.2 (`prompt` by level), §5.5
(`workspace`, `level`), §6.4 (keep-awake), §8 "Gateway" (prompt routing, `choice_lasts`,
timeline from recorded hook reports, no text in the log). Plans 1A and 1B produced what this
consumes: `docs/superpowers/plans/2026-10-12-ios-phase-1a-protocol.md`,
`docs/superpowers/plans/2026-10-12-ios-phase-1b-link-and-pairing.md`.

**Rulings made here (the later plans inherit them):**

1. **Where the listener binds.** The spec says LAN and Tailscale interfaces, never `0.0.0.0`
   without asking. No interface-enumeration crate is in the lockfile and the sandbox cannot add
   one, so: the listener binds the machine's primary IPv4 (found with std alone: a UDP socket
   "connected" to a public address, which sends nothing, and its local address) plus every
   address the operator adds in `machine.json`'s `gateway.addresses` (a Tailscale address is
   typed or pasted there in 1D), each on `gateway.port`; never `0.0.0.0`. Cost if wrong: a
   Tailscale link needs one paste until an enumeration lands.
2. **Keep-awake is a child process, not an FFI call.** `IOPMAssertionCreateWithName` needs
   `unsafe`, which AGENTS.md forbids outside two audited blocks; macOS ships `caffeinate`
   (already a word purlis's shell wrapper knows) and Linux `systemd-inhibit`. The child is held
   while any chat is `working` or `needs_you` and killed when none is, when the setting turns
   off, and when the app quits (it is also bound to this process by `-w <pid>`, so a crash
   releases it). Cost if wrong: one process in `ps` while chats run.
3. **The `Host` adapter talks to `Planes` through a trait**, `remote::board::Opened`, so the
   board's tests run on `Planes::telling(...).running_sessions_on(...)` with the pretend host,
   as `lib.rs`'s own tests do, and the app implements it on `tauri::AppHandle`. Cost if wrong:
   one trait with two impls.
4. **The timeline's level-2 source is the event log**, read through `eventlog::subscribe`
   (`hook.<word>` events carry the tool name, decision and timings; `run.started` the turns),
   and the hooks' pending asks; both are named causes (ADR 0076), nothing parses output. The
   adapter polls the subscription (it is a file reader) every 250 ms per link, which is inside
   R3's idle budget only while a timeline is attached. Cost if wrong: a 250 ms lag a person
   does not notice, or a poll interval to tune.

## Global Constraints

- Local branch `ios-mobile-design` of `~/purlis-research/purlis`. **Never push, never open a PR,
  never write to GitHub.**
- Commit messages: plain "what was done" only; **no AI trailers, no mention of AI**; commit
  from a Terminal without purlis's `GIT_CONFIG_*` env, or record each task's message in the
  ledger when the sandbox cannot write `.git` (1A's Setup ruling).
- AGENTS.md: **the `machine.json` row widens in `docs/plane-format.md` before any code writes
  the keys** (Task 0); nothing parses harness output; no `unsafe`; no colour; a change people
  would notice gets a `changes/<slug>.md` fragment: **1D adds it**, when the switch and the
  Devices page appear.
- The core never depends on Tauri or the UI; the app reaches core only through
  `purlis_core::remote` and `machine`.
- D9/§4.4: the host holds no transcript for this feature; the audit carries ids, never a
  prompt's, message's or answer's text. `remote.prompted` carries the prompt's byte length.
- §4.3: `resize` and `start` are refused by the grants table already (1A); `answer` with a
  lasting choice and a secret's ask are refused by `serve` through `choice_lasts` and
  `elicits_a_secret`, which this host **implements from the ask itself** (no default).
- §5.2: `prompt` to a level-2 chat is one bracketed paste and Enter in one `input`
  (`purlis_core::curation::bracketed(text) + "\r"`); to a shell tab `not_an_agent`; level 3 is
  Phase 2 (`turn_running` is not yet sent: an ACP chat lists as level 3 only when the app runs
  one, and this host answers its prompt as level 2 until Phase 2).
- Clippy `-D warnings`; `cargo fmt`. Check commands: `cargo test -p purlis-core --features fenced
  <test>`, `cargo test -p purlis-app remote` (the app's tests; `--exclude purlis-app` builds
  cannot run them in the chat's sandbox because the Tauri build script writes inside the clone:
  **the app's tests are run by the user**, and the plan says so where it matters), `cargo clippy
  --workspace --all-targets -- -D warnings`.

## Review Focus

1. **A chat with no identity yet** (never started, `Identity.id == None`) must not be listed
   with an empty id, and an attach to it must be `no_such_chat` (Task 3:
   `a_chat_without_an_id_is_not_listed`).
2. **A prompt to a chat that ended between list and prompt** must be refused `no_such_chat`,
   not written into a dead session (Task 3: `a_prompt_to_a_chat_that_ended_is_refused`).
3. **An answer from the phone to an ask the window answered first** must come back as a refusal
   the phone can show ("answered on the desktop"), and nothing is recorded as the device's
   answer (Task 3: `an_ask_the_window_answered_first_is_refused_to_the_device`).
4. **The listener must never bind `0.0.0.0`**, and a configured address that does not bind must
   not stop the others (Task 5: `the_listener_binds_each_address_it_can_and_never_everything`).
5. **Keep-awake must end when the last working chat ends**, and must not start for an idle chat
   (Task 6: `the_assertion_follows_the_working_chats`).

---

### Task 0: The `machine.json` row, first

**Files:**
- Modify: `docs/plane-format.md` line 6502 (`machine.json`)

- [ ] **Step 1: Widen the row**

In the `machine.json` row, after "`weekly`, `2026-W41`, ADR 0083 as amended for OB-17): never sent, and the same value on every machine that checked that week." insert:

```markdown
 And two keys of the iOS design (§4.1, §6.4; **written from Phase 1C on**): `gateway`, whether this machine serves paired phones and where (`{"on":false,"port":7777,"addresses":[]}`: `on` off by default; `addresses` the operator's own additions, each `host` or `host:port`, beside the primary address purlis finds itself; never `0.0.0.0`), and `keep_awake`, "Keep this Mac awake while chats are running" (`false` by default; while on and a chat is `working` or `needs_you` the app holds a `caffeinate`, or `systemd-inhibit`, child). Both are facts about this Mac, so they stay in this device-bound file.
```

- [ ] **Step 2: Run the tier test and commit**

Run: `cargo test -p purlis-core --test every_store_the_plane_format_names_has_a_tier 2>&1 | grep "test result"`
Expected: ok (the socket test aside in the sandbox).

```bash
git add docs/plane-format.md && git commit -q -m "Plane format: the gateway and keep-awake keys in machine.json"
```

---

### Task 1: The two keys on `machine::Store`

**Files:**
- Modify: `crates/purlis-core/src/machine.rs` (`Store`, `OnDisk`, `load`)
- Test: `crates/purlis-core/tests/the_machine_store_keeps_the_gateway_and_keep_awake_keys.rs`

**Interfaces:**
- Produces: `machine::Gateway { pub on: bool, pub port: u16, pub addresses: Vec<String> }` (`Default`: off, 7777, empty), `Store::gateway: Gateway`, `Store::keep_awake: bool`.

- [ ] **Step 1: Write the failing test**

```rust
//! `machine.json` keeps the gateway's switch, port and addresses and the keep-awake setting
//! (iOS design §4.1, §6.4): off by default, written through `update`, read back the same, and a
//! file from before them reads as the defaults.

use purlis_core::machine::{self, Gateway};

#[test]
fn the_defaults_are_off_7777_and_no_addresses() {
    let dir = tempfile::tempdir().unwrap();
    let loaded = machine::read(dir.path());
    assert_eq!(loaded.store.gateway, Gateway::default());
    assert!(!loaded.store.gateway.on);
    assert_eq!(loaded.store.gateway.port, 7777);
    assert!(!loaded.store.keep_awake);
}

#[test]
fn the_keys_round_trip_through_update() {
    let dir = tempfile::tempdir().unwrap();
    machine::update(dir.path(), |store| {
        store.gateway = Gateway {
            on: true,
            port: 7800,
            addresses: vec!["100.64.0.9".into()],
        };
        store.keep_awake = true;
    })
    .unwrap();
    let loaded = machine::read(dir.path());
    assert!(loaded.store.gateway.on);
    assert_eq!(loaded.store.gateway.port, 7800);
    assert_eq!(loaded.store.gateway.addresses, vec!["100.64.0.9"]);
    assert!(loaded.store.keep_awake);
    let text = std::fs::read_to_string(machine::file(dir.path())).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["gateway"]["on"], true);
    assert_eq!(json["gateway"]["port"], 7800);
    assert_eq!(json["keep_awake"], true);
}

#[test]
fn a_file_without_the_keys_reads_as_the_defaults_and_keeps_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    machine::update(dir.path(), |store| store.keep_awake = false).unwrap();
    let path = machine::file(dir.path());
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    json.as_object_mut().unwrap().remove("gateway");
    json.as_object_mut().unwrap().remove("keep_awake");
    std::fs::write(&path, json.to_string()).unwrap();
    let loaded = machine::read(dir.path());
    assert_eq!(loaded.store.gateway, Gateway::default());
    assert!(!loaded.store.keep_awake);
    assert!(loaded.dropped.is_empty(), "{:?}", loaded.dropped);
}

#[test]
fn a_gateway_key_of_the_wrong_shape_is_dropped_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    machine::update(dir.path(), |_| {}).unwrap();
    let path = machine::file(dir.path());
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    json["gateway"] = serde_json::json!("yes please");
    std::fs::write(&path, json.to_string()).unwrap();
    let loaded = machine::read(dir.path());
    assert_eq!(loaded.store.gateway, Gateway::default());
    assert!(!loaded.dropped.is_empty(), "the bad key is reported");
}
```

Check `Loaded`'s field names (`store`, `dropped`, `unreadable`) at `machine.rs:1593` and the
default-key behaviour: the defaults are **omitted on disk** when they are the defaults
(`skip_serializing_if`), as `channel` and `weekly` are.

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p purlis-core --features fenced --test the_machine_store_keeps_the_gateway_and_keep_awake_keys 2>&1 | grep -E "^error" | head -3`
Expected: FAIL to compile: no `Gateway`, no `gateway` field.

- [ ] **Step 3: Add the keys**

In `machine.rs`:

```rust
/// Whether this machine serves paired phones, and where (iOS design §4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gateway {
    /// Off by default: the listener runs only when the person turned it on.
    pub on: bool,
    pub port: u16,
    /// The operator's own addresses to bind beside the primary one purlis finds (a Tailscale
    /// address, typically), each `host` or `host:port`. Never `0.0.0.0`.
    pub addresses: Vec<String>,
}

impl Default for Gateway {
    fn default() -> Self {
        Gateway {
            on: false,
            port: 7777,
            addresses: Vec::new(),
        }
    }
}
```

`Store` gains `pub gateway: Gateway` and `pub keep_awake: bool` (its `Default` derive covers
both). `OnDisk` gains

```rust
    #[serde(skip_serializing_if = "Gateway::is_default")]
    gateway: GatewayOnDisk,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    keep_awake: bool,
```

with `GatewayOnDisk { on: bool, port: u16, addresses: Vec<String> }` (`serde::Serialize`),
`impl Gateway { fn is_default(on_disk: &GatewayOnDisk) -> bool { … == default } }`, and
`From<&Store> for OnDisk` filling both. In `load`, after `weekly:`:

```rust
        gateway: gateway_of(doc.get("gateway"), dropped),
        keep_awake: doc
            .get("keep_awake")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
```

```rust
/// The `gateway` key, or the default; a key that is not an object is reported and ignored.
fn gateway_of(raw: Option<&serde_json::Value>, dropped: &mut Vec<Dropped>) -> Gateway {
    let Some(raw) = raw else {
        return Gateway::default();
    };
    let Some(object) = raw.as_object() else {
        dropped.push(Dropped::Key("gateway".into()));
        return Gateway::default();
    };
    let defaults = Gateway::default();
    Gateway {
        on: object.get("on").and_then(serde_json::Value::as_bool).unwrap_or(defaults.on),
        port: object
            .get("port")
            .and_then(serde_json::Value::as_u64)
            .and_then(|p| u16::try_from(p).ok())
            .filter(|p| *p != 0)
            .unwrap_or(defaults.port),
        addresses: object
            .get("addresses")
            .and_then(serde_json::Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
    }
}
```

`Dropped` is the enum `read` reports with; use its existing variant for a bad key (read the
enum at the top of `load`'s neighbourhood and pick the one `channel_of` uses for a bad
channel, or add `Key(String)` if none fits).

- [ ] **Step 4: Run, clippy, commit**

Run: `cargo test -p purlis-core --features fenced --test the_machine_store_keeps_the_gateway_and_keep_awake_keys 2>&1 | grep "test result"` and `cargo test -p purlis-core --features fenced --lib machine:: 2>&1 | grep "test result"`
Expected: 4/4; the machine module's own tests still pass.

```bash
cargo clippy -p purlis-core --all-targets --features fenced -- -D warnings && cargo fmt -p purlis-core
git add crates/purlis-core && git commit -q -m "Core: the gateway and keep-awake keys in the machine store"
```

---

### Task 2: A `remote-device` answerer

**Files:**
- Modify: `crates/purlis-core/src/harness/asks.rs` (`Admitted::RemoteDevice`, `HumanScope::RemoteDevice`)
- Test: `crates/purlis-core/tests/an_answer_from_a_paired_phone_is_a_human_answer.rs`

**Interfaces:**
- Produces: `Admitted::RemoteDevice`, `HumanScope::RemoteDevice` (serialised `remote-device`), `Answerer::admitted(Admitted::RemoteDevice) == Some(_)`.

- [ ] **Step 1: Write the failing test**

```rust
//! A paired phone answers an ask as a human scope, `remote-device` (iOS design §4.3), so the
//! ask closes with "who answered" naming it, as the desktop's and `charter inbox`'s do.

use purlis_core::harness::asks::{Admitted, Answerer, HumanScope};

#[test]
fn remote_device_is_a_human_answerer_and_says_so() {
    let by = Answerer::admitted(Admitted::RemoteDevice).expect("a human scope");
    assert_eq!(by.scope(), HumanScope::RemoteDevice);
    assert_eq!(serde_json::to_value(by.scope()).unwrap(), "remote-device");
}

#[test]
fn the_other_scopes_are_as_they_were() {
    assert!(Answerer::admitted(Admitted::LocalUi).is_some());
    assert!(Answerer::admitted(Admitted::Approval).is_some());
    for not in [Admitted::Terminal, Admitted::FleetMcp, Admitted::Editor, Admitted::Chat] {
        assert!(Answerer::admitted(not).is_none(), "{not:?}");
    }
}
```

- [ ] **Step 2: Run to see it fail; Step 3: add the variants**

Run: `cargo test -p purlis-core --features fenced --test an_answer_from_a_paired_phone_is_a_human_answer 2>&1 | grep -E "^error" | head -2`
Expected: FAIL: no variant `RemoteDevice`.

In `asks.rs`: `Admitted` gains `RemoteDevice` (doc: "A paired phone, admitted by its link
key (iOS design §4.3)."), `HumanScope` gains `RemoteDevice`, and `Answerer::admitted` maps
`Admitted::RemoteDevice => Some(Self(HumanScope::RemoteDevice))`. Every exhaustive `match` on
`Admitted` or `HumanScope` in the crate gains the arm (`cargo build` lists them; the ask's
"answered by" wording, where one exists, says "your phone").

- [ ] **Step 4: Run, clippy, commit**

Run: `cargo test -p purlis-core --features fenced --test an_answer_from_a_paired_phone_is_a_human_answer 2>&1 | grep "test result"` → 2/2; `cargo test -p purlis-core --features fenced --lib asks 2>&1 | grep "test result"` → ok.

```bash
cargo clippy -p purlis-core --all-targets --features fenced -- -D warnings && cargo fmt -p purlis-core
git add crates/purlis-core && git commit -q -m "Core: a paired phone answers an ask as remote-device"
```

---

### Task 3: The app's `session::Host` over the planes

**Files:**
- Modify: `app/src-tauri/Cargo.toml` (`purlis-session-protocol = { path = "../../crates/session-protocol", features = ["noise"] }` under `[dependencies]`; `tokio` gains `"net"`, `"sync"`; dev-dependency adds `features = ["any-stream", "noise"]`)
- Create: `app/src-tauri/src/remote/mod.rs`, `app/src-tauri/src/remote/board.rs`, `app/src-tauri/src/remote/board_tests.rs`
- Modify: `app/src-tauri/src/lib.rs` (`mod remote;`), `app/src-tauri/src/hooks.rs` (`answer_as`), `app/src-tauri/src/planes.rs` (`Held::project_id()`)

**Interfaces:**
- Consumes: `Planes::{open_now, held}`, `Held::{root, hooks, chats, board}`, `Chats::{open_now, sessions, close, harness, chat_at}`, `SessionHost::{watch, unwatch, input}`, `Hooks::{asks, answer}`, `Asks::pending`, `session::{Host, Command, Answer, Refusal, Chat, Attached, TimelineAttached, Peer, NO_SUCH_CHAT, NOT_AN_AGENT, NOT_ALLOWED}`, `view::{Attacher, Limits, ViewId}`, `timeline::{Attacher as Timelines…}` (Task 4), `curation::bracketed`.
- Produces:
  - `remote::board::Opened: Send + Sync + 'static` trait: `fn with_planes<R>(&self, f: &mut dyn FnMut(&Planes) -> R) -> Option<R>` (None when the app has no planes yet); impls on `tauri::AppHandle` (`self.try_state::<Planes>()`) and on `Arc<Planes>`;
  - `remote::board::Board { opened: Arc<dyn Opened>, device: DeviceOf, views: Mutex<HashMap<(String /*chat*/, u32 /*view*/), (PlaneId, u32 /*session*/, u32 /*app view*/)>>, timelines: … }` with `Board::new(opened, device: DeviceOf)` where `DeviceOf { key: [u8; 32], device: Option<String> }` names the link's phone for the audit;
  - `impl session::Host for Board` (one per link; `Clone` through an `Arc` inside);
  - `Hooks::answer_as(&self, session, ask, option, by: Answerer) -> Result<(), String>`;
  - `Held::project_id(&self) -> Option<String>` (`scaffold::planefile::ensure_project_id(root)`, cached).

- [ ] **Step 1: Write the failing tests** (`remote/board_tests.rs`, `#[cfg(test)] mod board_tests;` in `remote/mod.rs`)

```rust
//! The host a paired phone talks to (iOS design §4.3, §5): what it lists, what it refuses, and
//! what reaches the app's own seam. Over `Planes::telling` and the pretend session host, the
//! way `lib.rs`'s tests build a plane, with the protocol's own client on a duplex link.

use std::sync::Arc;

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, NO_SUCH_CHAT, NOT_AN_AGENT};
use tokio::io::duplex;

use super::board::{Board, DeviceOf, Opened};
use crate::host::pretend::Pretend;
use crate::planes::Planes;

const KEPT: &str = "01J9ZQ3V7K8M2N4P6R8T0V2X4Z";

/// A plane with one chat open on the pretend host, named `one`, in workspace `login`.
fn a_plane_with_one_chat(pretend: &Pretend) -> (Arc<Planes>, u32) {
    let root = tempfile::tempdir().unwrap().keep();
    purlis_core::scaffold::new_plane(&root).unwrap();
    let host = pretend.clone();
    let planes = Planes::telling(
        Arc::new(|_: crate::hooks::Moved| {}),
        crate::Shipped::default(),
        None,
    )
    .running_sessions_on(Arc::new(move |_| Box::new(host.clone())));
    let plane = planes.open(&root);
    let held = planes.held(&plane).unwrap();
    let chat = purlis_core::reopen::Chat {
        program: "/nowhere/claude".to_owned(),
        name: "one".to_owned(),
        workspace: Some("login".to_owned()),
        identity: purlis_core::reopen::Identity {
            id: Some(KEPT.to_owned()),
            ..Default::default()
        },
        ..Default::default()
    };
    let record = purlis_core::reopen::Record {
        chats: vec![chat],
        ..Default::default()
    };
    let opened = held
        .chats()
        .put_back(&record, purlis_core::engine::Size { cols: 80, rows: 24 });
    (Arc::new(planes), opened[0].session)
}

async fn linked(board: Board) -> Client {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect_as_a_device(a, session::speaks(), Scope::RemoteDevice),
        link::serve_as_a_device(
            b,
            session::speaks(),
            link::ProvenDevice::stand_in_as(Scope::RemoteDevice)
        )
    );
    tokio::spawn(session::serve(served.unwrap(), board, None));
    let (client, _views, _events) = Client::new(client.unwrap());
    client
}

fn board_over(planes: Arc<Planes>) -> Board {
    Board::new(
        Arc::new(planes) as Arc<dyn Opened>,
        DeviceOf {
            key: [7u8; 32],
            device: Some("01J9PHONE00000000000000000".into()),
        },
    )
}

#[tokio::test]
async fn list_names_each_open_chat_by_its_id_project_workspace_and_level() {
    let pretend = Pretend::default();
    let (planes, _session) = a_plane_with_one_chat(&pretend);
    let client = linked(board_over(planes.clone())).await;
    let chats = client.list().await.unwrap();
    assert_eq!(chats.len(), 1);
    let chat = &chats[0];
    assert_eq!(chat.chat, KEPT);
    assert_eq!(chat.title.as_deref(), Some("one"));
    assert_eq!(chat.workspace.as_deref(), Some("login"));
    assert_eq!(chat.level, Some(2), "a hooks harness is level 2");
    assert_eq!(chat.project.len(), 26, "the project's stable id, a ULID");
    assert!(chat.project_path.is_some());
    assert!(["working", "needs_you", "idle", "ended"].contains(&chat.state.as_str()));
}

#[tokio::test]
async fn a_chat_without_an_id_is_not_listed() {
    let pretend = Pretend::default();
    let root = tempfile::tempdir().unwrap().keep();
    purlis_core::scaffold::new_plane(&root).unwrap();
    let host = pretend.clone();
    let planes = Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None)
        .running_sessions_on(Arc::new(move |_| Box::new(host.clone())));
    let plane = planes.open(&root);
    let held = planes.held(&plane).unwrap();
    let record = purlis_core::reopen::Record {
        chats: vec![purlis_core::reopen::Chat {
            program: "/nowhere/claude".to_owned(),
            name: "nameless".to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    };
    held.chats()
        .put_back(&record, purlis_core::engine::Size { cols: 80, rows: 24 });
    let client = linked(board_over(Arc::new(planes))).await;
    assert!(client.list().await.unwrap().is_empty());
    let refused = client.attach("not-a-chat").await.unwrap_err();
    assert_eq!(refused.refusal().map(|r| r.code.as_str()), Some(NO_SUCH_CHAT));
}

#[tokio::test]
async fn write_reaches_the_session_and_prompt_is_one_bracketed_paste_then_enter() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let client = linked(board_over(planes)).await;
    client.write(KEPT, b"y").await.unwrap();
    client.prompt(KEPT, "also add a test for expiry").await.unwrap();
    let typed = pretend.typed(session);
    assert_eq!(typed[0], b"y");
    assert_eq!(
        typed[1],
        format!(
            "{}\r",
            purlis_core::curation::bracketed("also add a test for expiry")
        )
        .into_bytes(),
        "one write: the paste and Enter together"
    );
}

#[tokio::test]
async fn a_prompt_to_a_shell_tab_is_not_an_agent() {
    let pretend = Pretend::default();
    let root = tempfile::tempdir().unwrap().keep();
    purlis_core::scaffold::new_plane(&root).unwrap();
    let host = pretend.clone();
    let planes = Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None)
        .running_sessions_on(Arc::new(move |_| Box::new(host.clone())));
    let plane = planes.open(&root);
    let held = planes.held(&plane).unwrap();
    let record = purlis_core::reopen::Record {
        chats: vec![purlis_core::reopen::Chat {
            program: "/bin/zsh".to_owned(),
            name: "shell".to_owned(),
            identity: purlis_core::reopen::Identity {
                id: Some("01J9SHELL00000000000000000".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        }],
        ..Default::default()
    };
    held.chats()
        .put_back(&record, purlis_core::engine::Size { cols: 80, rows: 24 });
    let client = linked(board_over(Arc::new(planes))).await;
    let listed = client.list().await.unwrap();
    assert_eq!(listed[0].level, Some(1));
    let refused = client
        .prompt("01J9SHELL00000000000000000", "ls")
        .await
        .unwrap_err();
    assert_eq!(refused.refusal().map(|r| r.code.as_str()), Some(NOT_AN_AGENT));
    client.write("01J9SHELL00000000000000000", b"ls\r").await.unwrap();
}

#[tokio::test]
async fn a_prompt_to_a_chat_that_ended_is_refused() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let client = linked(board_over(planes.clone())).await;
    assert_eq!(client.list().await.unwrap().len(), 1);
    pretend.program_ends(session, purlis_core::session::Exit::Code(0));
    // The chat left the board between list and prompt.
    let plane = planes.open_now()[0].clone();
    planes.held(&plane).unwrap().chats().close(session).ok();
    let refused = client.prompt(KEPT, "hello?").await.unwrap_err();
    assert_eq!(refused.refusal().map(|r| r.code.as_str()), Some(NO_SUCH_CHAT));
}

#[tokio::test]
async fn attach_opens_a_view_fed_by_the_sessions_output_and_detach_closes_it() {
    use purlis_session_protocol::view::{Chunk, Viewer};
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect_as_a_device(a, session::speaks(), Scope::RemoteDevice),
        link::serve_as_a_device(b, session::speaks(), link::ProvenDevice::stand_in_as(Scope::RemoteDevice))
    );
    tokio::spawn(session::serve(served.unwrap(), board_over(planes), None));
    let (client, mut views, _events) = Client::new(client.unwrap());
    let attached = client.attach(KEPT).await.unwrap();
    assert_eq!((attached.cols, attached.rows), (80, 24));
    pretend.emit(session, "hello from the pane");
    let mut reader = Viewer::default().accept(views.accept().await.unwrap()).await.unwrap();
    let mut seen = Vec::new();
    while !String::from_utf8_lossy(&seen).contains("hello from the pane") {
        match reader.next().await.unwrap().unwrap() {
            Chunk::Snapshot(b) | Chunk::Live(b) => {
                reader.ack(b.len()).await.unwrap();
                seen.extend_from_slice(&b);
            }
        }
    }
    client.detach(KEPT, attached.view).await.unwrap();
    assert!(pretend.watching(session).is_empty(), "the app's view was closed");
}

#[tokio::test]
async fn an_ask_the_window_answered_first_is_refused_to_the_device() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let plane = planes.open_now()[0].clone();
    let held = planes.held(&plane).unwrap();
    let raised = held.hooks().asks().raise(
        &session.to_string(),
        purlis_core::harness::asks::test_support::an_allow_ask(),
        std::time::Instant::now(),
    );
    let ask = raised.id().to_string();
    held.hooks().answer(session, &ask, "allow").unwrap();
    let client = linked(board_over(planes.clone())).await;
    let refused = client.answer(KEPT, &ask, "allow").await.unwrap_err();
    let why = refused.refusal().unwrap();
    assert_ne!(why.code, NOT_ALLOWED_WORD, "not a scope refusal: an answered ask");
    assert!(why.why.contains("desktop"), "{}", why.why);
}

const NOT_ALLOWED_WORD: &str = session::NOT_ALLOWED;

#[tokio::test]
async fn an_open_ask_answered_from_the_device_closes_as_remote_device() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let plane = planes.open_now()[0].clone();
    let held = planes.held(&plane).unwrap();
    let raised = held.hooks().asks().raise(
        &session.to_string(),
        purlis_core::harness::asks::test_support::an_allow_ask(),
        std::time::Instant::now(),
    );
    let ask = raised.id().to_string();
    let client = linked(board_over(planes.clone())).await;
    client.answer(KEPT, &ask, "allow").await.unwrap();
    assert!(held.hooks().asks().pending(std::time::Instant::now()).is_empty());
}

#[tokio::test]
async fn a_lasting_choice_and_a_secrets_ask_are_refused_before_the_board_hears_them() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let plane = planes.open_now()[0].clone();
    let held = planes.held(&plane).unwrap();
    let lasting = held.hooks().asks().raise(
        &session.to_string(),
        purlis_core::harness::asks::test_support::an_ask_with_an_always_choice(),
        std::time::Instant::now(),
    );
    let secret = held.hooks().asks().raise(
        &session.to_string(),
        purlis_core::harness::asks::test_support::an_ask_that_elicits_a_secret(),
        std::time::Instant::now(),
    );
    let client = linked(board_over(planes.clone())).await;
    let refused = client.answer(KEPT, &lasting.id().to_string(), "always").await.unwrap_err();
    assert_eq!(refused.refusal().map(|r| r.code.as_str()), Some(session::NOT_ALLOWED));
    client.answer(KEPT, &lasting.id().to_string(), "once").await.unwrap();
    let refused = client.answer(KEPT, &secret.id().to_string(), "yes").await.unwrap_err();
    assert_eq!(refused.refusal().map(|r| r.code.as_str()), Some(session::NOT_ALLOWED));
    assert_eq!(held.hooks().asks().pending(std::time::Instant::now()).len(), 1, "the secret's ask stays open");
}

#[tokio::test]
async fn stop_ends_the_chat() {
    let pretend = Pretend::default();
    let (planes, session) = a_plane_with_one_chat(&pretend);
    let client = linked(board_over(planes.clone())).await;
    client.stop(KEPT).await.unwrap();
    assert!(pretend.closed().contains(&session));
    assert!(client.list().await.unwrap().is_empty());
}
```

Facts to check while writing it (each is one grep): `Pretend`'s helpers (`typed`, `program_ends`,
`openings`; add `emit(session, text)`, `watching(session)` and `closed()` to the pretend host
if it lacks them, in `host.rs`'s `pretend` module: `emit` pushes text to every sink `watch`
registered for the session); `reopen::Chat`/`Record`/`Identity` `Default` impls (they derive it
where `lib.rs`'s tests use `..Default::default()`); `scaffold::new_plane` (the function
`lib.rs`'s tests use to make a plane root; use that one); `Asks::raise`'s return
(`Raising` has an `id()` or holds `Raised`; adjust); and the three `asks::test_support`
constructors (add them under `#[cfg(any(test, feature = "test-support"))]` in
`crates/purlis-core/src/harness/asks.rs`: an allow/deny ask with `Once` choices, one with an
`Always` choice, one with `elicits_secret: true`; the app's dev-dependency already turns
`test-support` on).

- [ ] **Step 2: Run to see it fail**

Run (the user, outside the sandbox): `cd ~/purlis-research/purlis && cargo test -p purlis-app remote::board_tests 2>&1 | grep -E "^error" | head -3`
Expected: FAIL to compile: no `remote` module.

- [ ] **Step 3: Write `remote/mod.rs` and `remote/board.rs`**

`remote/mod.rs`:

```rust
//! A paired phone's side of this desktop (iOS design §3): the app's wiring of
//! `purlis-session-protocol`'s gateway over `purlis_core::remote`'s state.
//!
//! - [`board`]: the `session::Host` a phone talks to, over the app's own seam.
//! - [`timeline`]: a chat's level-2 story, from the event log and the hooks' asks.
//! - [`adapters`]: the gateway's `Pins`, `PairingDesk` and revocation over core's peers and
//!   desk, and its `Heard` into the audit.
//! - [`gateway`]: the listener's lifecycle, keep-awake, and the commands the window calls.
//! - [`awake`]: the keep-awake child.

pub mod adapters;
pub mod awake;
pub mod board;
pub mod gateway;
pub mod timeline;

#[cfg(test)]
mod board_tests;
```

(Tasks 4–6 fill `timeline`, `adapters`, `gateway`, `awake`; create each as a one-line doc stub
now.)

`remote/board.rs`:

```rust
//! The host a paired phone talks to (iOS design §4.3, §5.2, §5.5): the protocol crate's
//! `session::Host`, answered from the app's own seam, [`crate::host::SessionHost`] and
//! [`crate::host::ChatBoard`], one `Board` per link.
//!
//! A chat is named on the wire by its id (`reopen::Identity::id`), a project by its stable id
//! (`[project].id`, minted on first ask), and a chat without an id yet is not listed: it has
//! nothing a phone could name it by. Every answer or prompt from the phone is recorded as
//! metadata through [`crate::remote::adapters::audit`], never with its text.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use bytes::Bytes;
use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::harness::model::ChoiceScope;
use purlis_session_protocol::session::{
    self, Answer, Attached, Chat, Command, Host, NO_SUCH_CHAT, NOT_AN_AGENT, Peer, Refusal,
    TimelineAttached,
};
use purlis_session_protocol::view::{Attacher, Limits, ViewId};

use crate::planes::{PlaneId, Planes};

/// Where the planes are: the app's managed state, or a test's own.
pub trait Opened: Send + Sync + 'static {
    fn with_planes<R>(&self, f: &mut dyn FnMut(&Planes) -> R) -> Option<R>;
}

impl Opened for Arc<Planes> {
    fn with_planes<R>(&self, f: &mut dyn FnMut(&Planes) -> R) -> Option<R> {
        Some(f(self))
    }
}

impl Opened for tauri::AppHandle {
    fn with_planes<R>(&self, f: &mut dyn FnMut(&Planes) -> R) -> Option<R> {
        use tauri::Manager;
        self.try_state::<Planes>().map(|planes| f(&planes))
    }
}

/// Which phone this board serves, for the audit.
#[derive(Debug, Clone)]
pub struct DeviceOf {
    pub key: [u8; 32],
    pub device: Option<String>,
}

/// One chat as the board found it: where it is, and how much the host knows of it.
#[derive(Debug, Clone)]
struct Found {
    plane: PlaneId,
    session: u32,
    level: u8,
}

struct Inner {
    opened: Arc<dyn Opened>,
    device: DeviceOf,
    /// The phone's views, by chat and the view number the phone was told, to the app's own.
    views: Mutex<HashMap<(String, u32), (PlaneId, u32, u32)>>,
    /// Timelines the phone attached, by chat, each a task that ends on detach.
    timelines: Mutex<HashMap<(String, u32), tokio::task::JoinHandle<()>>>,
    next_view: Mutex<u32>,
}

/// The host, cheap to clone: one per link.
#[derive(Clone)]
pub struct Board(Arc<Inner>);

impl Board {
    pub fn new(opened: Arc<dyn Opened>, device: DeviceOf) -> Board {
        Board(Arc::new(Inner {
            opened,
            device,
            views: Mutex::new(HashMap::new()),
            timelines: Mutex::new(HashMap::new()),
            next_view: Mutex::new(0),
        }))
    }

    fn device(&self) -> &DeviceOf {
        &self.0.device
    }

    /// Every chat a phone may name, with what the board knows of it.
    fn found(&self) -> Vec<(Chat, Found)> {
        self.0
            .opened
            .with_planes(&mut |planes| {
                let mut out = Vec::new();
                for plane in planes.open_now() {
                    let Ok(held) = planes.held(&plane) else { continue };
                    let project = held.project_id();
                    let project_path = held.root().to_string_lossy().into_owned();
                    for open in held.chats().open_now() {
                        let Some(id) = held.chats().chat_at(open.session).and_then(|at| at.identity.id) else {
                            continue;
                        };
                        let level = match open.harness {
                            None => 1,
                            Some(_) => 2,
                        };
                        let glance = held.board().glance(open.session);
                        let state = state_word(glance.state, glance.asking);
                        out.push((
                            Chat {
                                chat: id,
                                project: project.clone().unwrap_or_default(),
                                project_path: Some(project_path.clone()),
                                title: Some(open.name.clone()),
                                state: state.to_owned(),
                                workspace: open.workspace.clone(),
                                level: Some(level),
                            },
                            Found { plane: plane.clone(), session: open.session, level },
                        ));
                    }
                }
                out
            })
            .unwrap_or_default()
    }

    fn find(&self, chat: &str) -> Result<Found, Refusal> {
        self.found()
            .into_iter()
            .find(|(c, _)| c.chat == chat)
            .map(|(_, found)| found)
            .ok_or_else(|| Refusal {
                code: NO_SUCH_CHAT.into(),
                why: format!("no chat {chat} on this host"),
            })
    }

    fn with_held<R>(&self, found: &Found, f: impl FnOnce(&crate::planes::Held) -> R) -> Result<R, Refusal> {
        self.0
            .opened
            .with_planes(&mut |planes| planes.held(&found.plane).ok().map(|held| f(&held)))
            .flatten()
            .ok_or_else(|| Refusal {
                code: NO_SUCH_CHAT.into(),
                why: "the chat's project is no longer open".into(),
            })
    }
}

/// The chat's state as the protocol words it (V76): `working`, `needs_you`, `idle`, `ended`.
fn state_word(state: purlis_core::state::State, asking: bool) -> &'static str {
    use purlis_core::state::State;
    match (state, asking) {
        (_, true) => "needs_you",
        (State::Running, _) => "working",
        (State::Waiting | State::Unknown, _) => "idle",
        (State::Done | State::Failed, _) => "ended",
    }
}

fn refuse(code: &str, why: impl Into<String>) -> Refusal {
    Refusal {
        code: code.into(),
        why: why.into(),
    }
}

impl Host for Board {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        match command {
            Command::List => Ok(Answer::Chats(self.found().into_iter().map(|(c, _)| c).collect())),
            Command::Attach { chat } => {
                let found = self.find(&chat)?;
                let opener = peer.views().clone();
                let attacher = Attacher::new(opener, Limits::default());
                let number = {
                    let mut next = self.0.next_view.lock().unwrap_or_else(PoisonError::into_inner);
                    *next += 1;
                    *next
                };
                // The app's view: its sink feeds the protocol's view. The feed is held by the
                // sink's closure, so the view ends when the app's view is closed.
                let (size, app_view) = self.with_held(&found, |held| {
                    let feed = Arc::new(Mutex::new(None));
                    let feeding = Arc::clone(&feed);
                    let watching = held.chats().sessions().watch(
                        found.session,
                        Box::new(move |text: String| {
                            if let Some(feed) = feeding.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
                                let _ = purlis_session_protocol::view::Feed::push(feed, Bytes::from(text));
                            }
                        }),
                    )?;
                    Ok::<_, String>((watching, feed))
                })?
                .map(|(watching, feed)| ((watching.size, watching.view), feed))
                .map_err(|why| refuse(NO_SUCH_CHAT, why))?;
                let ((size, app_view), feed_slot) = (size, app_view);
                let snapshot = Bytes::new();
                let feed = attacher
                    .attach(ViewId(number), snapshot)
                    .await
                    .map_err(|e| refuse("link_failed", e.to_string()))?;
                *feed_slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(feed);
                self.0
                    .views
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert((chat, number), (found.plane.clone(), found.session, app_view));
                Ok(Answer::Attached(Attached {
                    view: number,
                    cols: size.cols,
                    rows: size.rows,
                }))
            }
            Command::Detach { chat, view } => {
                let taken = self
                    .0
                    .views
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&(chat, view));
                if let Some((plane, session, app_view)) = taken {
                    let _ = self.0.opened.with_planes(&mut |planes| {
                        if let Ok(held) = planes.held(&plane) {
                            let _ = held.chats().sessions().unwatch(session, app_view);
                        }
                    });
                }
                Ok(Answer::Done)
            }
            Command::Write { chat, bytes } => {
                let found = self.find(&chat)?;
                self.with_held(&found, |held| held.chats().sessions().input(found.session, &bytes))?
                    .map_err(|why| refuse(NO_SUCH_CHAT, why))?;
                Ok(Answer::Done)
            }
            Command::Prompt { chat, text } => {
                let found = self.find(&chat)?;
                if found.level < 2 {
                    return Err(refuse(NOT_AN_AGENT, "a shell tab takes keystrokes, not a prompt"));
                }
                let pasted = format!("{}\r", purlis_core::curation::bracketed(&text));
                self.with_held(&found, |held| {
                    held.chats().sessions().input(found.session, pasted.as_bytes())
                })?
                .map_err(|why| refuse(NO_SUCH_CHAT, why))?;
                super::adapters::audit::prompted(self.device(), &chat, text.len());
                Ok(Answer::Done)
            }
            Command::Answer { chat, ask, answer } => {
                let found = self.find(&chat)?;
                let by = Answerer::admitted(Admitted::RemoteDevice).expect("a human scope");
                self.with_held(&found, |held| held.hooks().answer_as(found.session, &ask, &answer, by))?
                    .map_err(|why| refuse("ask_closed", format!("{why}; it may have been answered on the desktop")))?;
                super::adapters::audit::answered(self.device(), &chat, &ask, &answer);
                Ok(Answer::Done)
            }
            Command::Stop { chat } => {
                let found = self.find(&chat)?;
                self.with_held(&found, |held| held.chats().close(found.session))?
                    .map_err(|why| refuse(NO_SUCH_CHAT, why))?;
                Ok(Answer::Done)
            }
            Command::AttachTimeline { chat } => {
                let found = self.find(&chat)?;
                let number = {
                    let mut next = self.0.next_view.lock().unwrap_or_else(PoisonError::into_inner);
                    *next += 1;
                    *next
                };
                let task = super::timeline::attach(self.clone(), peer.views().clone(), found.clone(), number).await?;
                self.0
                    .timelines
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert((chat, number), task);
                Ok(Answer::TimelineAttached(TimelineAttached { timeline: number, level: found.level }))
            }
            Command::DetachTimeline { chat, timeline } => {
                if let Some(task) = self
                    .0
                    .timelines
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&(chat, timeline))
                {
                    task.abort();
                }
                Ok(Answer::Done)
            }
            // Refused by the grants table before they reach here; answered for completeness.
            Command::Resize { .. } | Command::Start(_) => Err(refuse(session::NOT_ALLOWED, "a device never does this")),
            Command::Subscribe { .. } | Command::Unsubscribe => Ok(Answer::Done),
        }
    }

    async fn elicits_a_secret(&self, chat: &str, ask: &str) -> bool {
        self.ask_of(chat, ask)
            .map(|raised| raised.ask.elicits_secret)
            // An ask the host cannot find is taken to elicit one: fail closed.
            .unwrap_or(true)
    }

    async fn choice_lasts(&self, chat: &str, ask: &str, answer: &str) -> bool {
        self.ask_of(chat, ask)
            .and_then(|raised| {
                raised
                    .ask
                    .options
                    .iter()
                    .find(|choice| choice.id == answer)
                    .map(|choice| choice.scope == ChoiceScope::Always)
            })
            // An ask or a choice the host cannot find is taken to last: fail closed.
            .unwrap_or(true)
    }

    async fn subscribe(
        &self,
        since: session::Since,
        _peer: &Peer,
    ) -> Result<tokio::sync::mpsc::Receiver<session::Pushed>, Refusal> {
        super::timeline::subscribe(self.clone(), since).await
    }
}

impl Board {
    /// The open ask `ask` of `chat`, if it is still open.
    fn ask_of(&self, chat: &str, ask: &str) -> Option<purlis_core::harness::asks::Raised> {
        let found = self.find(chat).ok()?;
        let id: purlis_core::harness::asks::AskId = ask.parse().ok()?;
        self.with_held(&found, |held| {
            held.hooks()
                .asks()
                .pending(std::time::Instant::now())
                .into_iter()
                .find(|raised| raised.id == id && raised.chat == found.session.to_string())
        })
        .ok()
        .flatten()
    }

    pub(super) fn opened(&self) -> &Arc<dyn Opened> {
        &self.0.opened
    }
}
```

`Hooks::answer_as` in `hooks.rs`, beside `answer`:

```rust
    /// [`Hooks::answer`] by a human scope that is not the window: a paired phone.
    pub fn answer_as(&self, session: u32, ask: &str, option: &str, by: Answerer) -> Result<(), String> {
        let id: purlis_core::harness::asks::AskId = ask.parse()?;
        let answered = self.asks.answer(&session.to_string(), &id, option, by, std::time::Instant::now());
        crate::asking::tell(&self.plane, &self.asks, &self.asks_told);
        answered.map(|_| ()).map_err(|refused| refused.to_string())
    }
```

and `answer` becomes `self.answer_as(session, ask, option, crate::asking::the_window())`.

`Held::project_id` in `planes.rs`:

```rust
    /// The project's stable id (`[project].id`, V76), minted on the first ask. `None` where the
    /// manifest cannot be written.
    pub fn project_id(&self) -> Option<String> {
        purlis_core::scaffold::planefile::ensure_project_id(self.root()).ok()
    }
```

`Opener: Clone` in the protocol crate: check `link::Opener` derives `Clone` (it wraps a
yamux control handle, which is `Clone`); if not, add `#[derive(Clone)]`.

The `Attach` arm as written holds the app's view open until the protocol view's feed is
dropped by the sink: simplify if the borrow checker objects, keeping the two facts: the app's
`watch` sink pushes into the protocol `Feed`, and `Detach` calls `unwatch`.

- [ ] **Step 4: Run (the user), clippy, commit**

Run: `cargo test -p purlis-app remote::board_tests 2>&1 | grep -E "test result|panicked" | head`
Expected: 9 PASS. `cargo clippy -p purlis-app --all-targets -- -D warnings` clean.

```bash
git add app/src-tauri crates/purlis-core && git commit -q -m "App: the host a paired phone talks to, over the session host and the board"
```

---

### Task 4: The level-2 timeline from the event log and the asks

**Files:**
- Modify: `app/src-tauri/src/remote/timeline.rs`
- Test: `app/src-tauri/src/remote/timeline_tests.rs`

**Interfaces:**
- Consumes: `eventlog::{subscribe, Delivery, Event, read}`, `Hooks::asks()`, `Board::{opened, find}` (Task 3), `timeline::{Attacher, Item, What, Limits, TimelineId, Role, Choice}` (1A), `session::{Pushed, Event as WireEvent, Since}`.
- Produces: `timeline::attach(board, opener, found, number) -> Result<JoinHandle<()>, Refusal>`; `timeline::subscribe(board, since) -> Result<mpsc::Receiver<Pushed>, Refusal>`; `timeline::item_of(event: &eventlog::Event, chat_id: &str) -> Option<Item>` (pure: `run.started` → `Turn { began: true }`, `hook.userpromptsubmit` → `Turn { began: true }`, `hook.stop` → `Turn { began: false, stop_reason: "end_turn" }`, `hook.sessionend` → `Turn { began: false, stop_reason: "ended" }`, a tool event (`body.tool`, `decision`, `hook_ms`/`tool_ms`) → `Tool { tool: event.ulid, title: body.tool, kind: "tool", status: decision word, duration_ms: tool_ms }`, anything else → `None`); `timeline::ask_item(raised: &Raised) -> Item` (`Ask { ask: id, summary, choices (id, label, lasts = scope == Always), elicits_secret }`); `timeline::closed_item(id, by) -> Item`.

- [ ] **Step 1: Write the failing unit tests** (`timeline_tests.rs`)

```rust
use purlis_session_protocol::timeline::{Role, What};

use super::timeline::{ask_item, item_of};

fn event(kind: &str, body: serde_json::Value) -> purlis_core::eventlog::Event {
    purlis_core::eventlog::Event {
        v: 1,
        device_id: "01J9DEVCE00000000000000000".into(),
        seq: 7,
        ulid: "01J9ZQ6000000000000000000B".into(),
        chat: Some("01J9ZQ3V7K8M2N4P6R8T0V2X4Z".into()),
        run: Some("01J9ZQ7000000000000000000C".into()),
        parent_run: None,
        kind: kind.into(),
        body,
    }
}

#[test]
fn a_prompt_submitted_begins_a_turn_and_a_stop_ends_it() {
    let began = item_of(&event("hook.userpromptsubmit", serde_json::json!({})), "01J9ZQ3V7K8M2N4P6R8T0V2X4Z").unwrap();
    assert_eq!(began.level, 2);
    assert!(matches!(began.what, What::Turn { began: true, .. }));
    let ended = item_of(&event("hook.stop", serde_json::json!({})), "01J9ZQ3V7K8M2N4P6R8T0V2X4Z").unwrap();
    assert!(matches!(ended.what, What::Turn { began: false, stop_reason: Some(ref r), .. } if r == "end_turn"));
}

#[test]
fn a_tool_event_is_a_tool_item_with_name_verdict_and_timing_and_never_its_arguments() {
    let item = item_of(
        &event("hook.pretooluse", serde_json::json!({"tool": "Bash", "call": "c1", "args": "deadbeef", "decision": "allow", "hook_ms": 3, "tool_ms": 1200})),
        "01J9ZQ3V7K8M2N4P6R8T0V2X4Z",
    )
    .unwrap();
    match item.what {
        What::Tool { title, status, duration_ms, .. } => {
            assert_eq!(title, "Bash");
            assert_eq!(status, "allow");
            assert_eq!(duration_ms, Some(1200));
        }
        other => panic!("{other:?}"),
    }
    assert!(!serde_json::to_string(&item).unwrap().contains("deadbeef"));
}

#[test]
fn another_chats_event_and_an_unknown_kind_are_not_items() {
    assert!(item_of(&event("hook.stop", serde_json::json!({})), "01J9OTHER00000000000000000").is_none());
    assert!(item_of(&event("trust.sandbox.grant", serde_json::json!({})), "01J9ZQ3V7K8M2N4P6R8T0V2X4Z").is_none());
}

#[test]
fn an_ask_item_carries_the_sources_choices_and_marks_the_lasting_one() {
    let raised = purlis_core::harness::asks::test_support::raised_with_an_always_choice("01J9ZQ3V7K8M2N4P6R8T0V2X4Z");
    let item = ask_item(&raised);
    match item.what {
        What::Ask { choices, elicits_secret, .. } => {
            assert!(!elicits_secret);
            assert!(choices.iter().any(|c| c.lasts));
            assert!(choices.iter().any(|c| !c.lasts));
        }
        other => panic!("{other:?}"),
    }
    let _ = Role::User;
}
```

- [ ] **Step 2: Run (the user) to see it fail; Step 3: write `timeline.rs`**

```rust
//! A chat's level-2 story for a phone (iOS design §5.1): turns and tools from the event log,
//! asks from the hooks, nothing from the terminal. The event log is the named-cause record
//! (ADR 0076); it is read with `eventlog::subscribe` from the chat's first event the host
//! still holds, so the snapshot is what the log holds and the live items follow at the poll.

use std::sync::Arc;
use std::time::Duration;

use purlis_core::harness::asks::Raised;
use purlis_core::harness::model::ChoiceScope;
use purlis_session_protocol::link::Opener;
use purlis_session_protocol::session::{self, Pushed, Refusal};
use purlis_session_protocol::timeline::{Attacher, Choice, Item, Limits, TimelineId, What};
use tokio::sync::mpsc;

use super::board::{Board, Found};

/// How often a timeline looks for new events while attached.
const POLL: Duration = Duration::from_millis(250);

pub fn item_of(event: &purlis_core::eventlog::Event, chat_id: &str) -> Option<Item> {
    if event.chat.as_deref() != Some(chat_id) {
        return None;
    }
    let turn = event.run.clone().unwrap_or_default();
    let what = match event.kind.as_str() {
        "run.started" | "hook.userpromptsubmit" => What::Turn { turn, began: true, stop_reason: None },
        "hook.stop" => What::Turn { turn, began: false, stop_reason: Some("end_turn".into()) },
        "hook.sessionend" => What::Turn { turn, began: false, stop_reason: Some("ended".into()) },
        kind if kind.starts_with("hook.") && event.body.get("tool").is_some() => What::Tool {
            tool: event.ulid.clone(),
            title: event.body["tool"].as_str().unwrap_or("tool").to_owned(),
            kind: "tool".into(),
            status: event.body["decision"].as_str().unwrap_or("none").to_owned(),
            duration_ms: event.body.get("tool_ms").and_then(serde_json::Value::as_u64),
        },
        _ => return None,
    };
    Some(Item { level: 2, what })
}

pub fn ask_item(raised: &Raised) -> Item {
    Item {
        level: 2,
        what: What::Ask {
            ask: raised.id.to_string(),
            summary: raised.ask.summary.to_string(),
            choices: raised
                .ask
                .options
                .iter()
                .map(|c| Choice { id: c.id.clone(), label: c.label.clone(), lasts: c.scope == ChoiceScope::Always })
                .collect(),
            elicits_secret: raised.ask.elicits_secret,
        },
    }
}

pub fn closed_item(ask: &str, by: &str) -> Item {
    Item { level: 2, what: What::AskClosed { ask: ask.into(), by: by.into() } }
}

/// Attach a timeline: the snapshot from the log and the open asks, then each new item at the
/// poll, until the task is aborted (detach) or the feed is closed.
pub async fn attach(board: Board, opener: Opener, found: Found, number: u32) -> Result<tokio::task::JoinHandle<()>, Refusal> {
    let (log_dir, chat_id, asks) = board.log_and_asks_of(&found)?;
    let mut subscription = purlis_core::eventlog::subscribe(&log_dir, 0);
    let mut snapshot = Vec::new();
    for delivery in subscription.poll().unwrap_or_default() {
        if let purlis_core::eventlog::Delivery::Event(event) = delivery {
            snapshot.extend(item_of(&event, &chat_id));
        }
    }
    let mut open: Vec<String> = Vec::new();
    for raised in asks.pending(std::time::Instant::now()).into_iter().filter(|r| r.chat == found.session.to_string()) {
        open.push(raised.id.to_string());
        snapshot.push(ask_item(&raised));
    }
    let feed = Attacher::new(opener, Limits::default())
        .attach(TimelineId(number), snapshot)
        .await
        .map_err(|e| Refusal { code: "link_failed".into(), why: e.to_string() })?;
    Ok(tokio::spawn(async move {
        loop {
            tokio::time::sleep(POLL).await;
            for delivery in subscription.poll().unwrap_or_default() {
                if let purlis_core::eventlog::Delivery::Event(event) = delivery
                    && let Some(item) = item_of(&event, &chat_id)
                    && feed.push(&item).is_err()
                {
                    return;
                }
            }
            let now = std::time::Instant::now();
            let pending: Vec<Raised> = asks.pending(now).into_iter().filter(|r| r.chat == found.session.to_string()).collect();
            for raised in &pending {
                let id = raised.id.to_string();
                if !open.contains(&id) {
                    open.push(id);
                    if feed.push(&ask_item(raised)).is_err() {
                        return;
                    }
                }
            }
            open.retain(|id| {
                let still = pending.iter().any(|r| r.id.to_string() == *id);
                if !still {
                    let _ = feed.push(&closed_item(id, "desktop"));
                }
                still
            });
        }
    }))
}

/// `subscribe(since)`: the event log's events as the protocol pushes them, one task per link.
pub async fn subscribe(board: Board, since: session::Since) -> Result<mpsc::Receiver<Pushed>, Refusal> {
    let (log_dir, device) = board.log_dir()?;
    let from = since.get(&device).copied().unwrap_or(0);
    let (tx, rx) = mpsc::channel(256);
    tokio::spawn(async move {
        let mut subscription = purlis_core::eventlog::subscribe(&log_dir, from);
        loop {
            for delivery in subscription.poll().unwrap_or_default() {
                let pushed = match delivery {
                    purlis_core::eventlog::Delivery::Event(e) => Pushed::Event(session::Event {
                        v: e.v, device_id: e.device_id, seq: e.seq, ulid: e.ulid, chat: e.chat, run: e.run, parent_run: e.parent_run, kind: e.kind, body: e.body,
                    }),
                    purlis_core::eventlog::Delivery::Missed { device, after, resumes_at } => Pushed::Missed { device, after, resumes_at },
                    _ => continue,
                };
                if tx.send(pushed).await.is_err() {
                    return;
                }
            }
            tokio::time::sleep(POLL).await;
        }
    });
    Ok(rx)
}
```

`Board::log_and_asks_of(&found) -> Result<(PathBuf, String, Arc<Asks>), Refusal>` and
`Board::log_dir() -> Result<(PathBuf, String /*device*/), Refusal>` go in `board.rs`: the
log directory is the recorder's (`Hooks` holds `Events = Arc<Mutex<Recorder>>`; expose
`Hooks::events_dir()` returning `recorder.dir()` and the device id from `machine::known_device_id`);
`asks` is `held.hooks().asks()` as an `Arc` (expose `Hooks::asks_arc()` or wrap). The
`Delivery` enum's variants (`Event`, `Missed { device, after, resumes_at }`, `Ahead`) are at
`eventlog.rs:667`; map `Ahead` to nothing (the client carries on). `Found` becomes `pub(super)`
with its fields.

- [ ] **Step 4: Run (the user), clippy, commit**

Run: `cargo test -p purlis-app remote::timeline_tests 2>&1 | grep "test result"` → 4/4; the board tests still pass; clippy clean.

```bash
git add app/src-tauri crates/purlis-core && git commit -q -m "App: a chat's level-2 timeline for a phone, from the event log and the asks"
```

---

### Task 5: The adapters and the listener's lifecycle

**Files:**
- Modify: `app/src-tauri/src/remote/adapters.rs`, `app/src-tauri/src/remote/gateway.rs`, `app/src-tauri/src/ipc_commands.rs` (the new commands in `value_free`), `app/src-tauri/src/lib.rs` (`app.manage(remote::gateway::Remote::new(…))` in setup, `remote::gateway::start_if_on(&app)` after the planes are set up)
- Test: `app/src-tauri/src/remote/gateway_tests.rs`

**Interfaces:**
- Produces:
  - `adapters::Pins(PathBuf)` → `gateway::Pins` over `peers::scope_of` (word → `Scope::from_word`);
  - `adapters::DeskAdapter { desk: Arc<Desk>, tickets: Mutex<HashMap<[u8; 32], Ticket>> }` → `PairingDesk`: `take()` = `desk.take(Instant::now())` → `Window { code: Code::from_hex, ends }`; `offered(offer)` = `desk.offer(Offer{…}, ends)` then `spawn_blocking(move || desk.wait_verdict(ticket, ends))` mapped to the crate's `Verdict`; `withdraw(&offer)` = `desk.withdraw(ticket)`;
  - `adapters::revocations(desk: &Desk) -> gateway::Revoked` (a `watch` channel whose sender the desk's `on_revoke` bumps);
  - `adapters::audit::{heard(events: &Events, heard: Heard), prompted(device, chat, bytes), answered(device, chat, ask, choice)}` writing `remote::audit::Act`s through the app's `Events`;
  - `gateway::Remote` (managed): `Remote::new(config: PathBuf, events: Option<Events>, device: String)`; `fn identity(&self) -> Result<noise::Identity, String>` (from `linkkey::load_or_mint` through a `secrets::Ctx` on the local project root… use `Ctx::new(&config_root, Env::real())`); `fn start(&self, app: tauri::AppHandle) -> Result<Vec<SocketAddr>, String>` (reads `machine::read(config).store.gateway`, binds each address, spawns `Gateway::listen` per listener, keeps the handles); `fn stop(&self)`; `fn bound(&self) -> Vec<SocketAddr>`; `fn primary_address() -> Option<Ipv4Addr>` (the UDP trick);
  - commands (all `value_free`): `remote_status() -> RemoteStatus { on, bound: Vec<String>, port, addresses, peers: Vec<PeerRow>, keep_awake }`, `remote_set_on(on: bool) -> RemoteStatus`, `remote_set_addresses(addresses: Vec<String>)`, `remote_pair_start() -> String` (the QR text), `remote_pair_pending() -> Option<PendingOffer { ticket: u64, device, name, fingerprint }>`, `remote_pair_confirm(ticket: u64) -> Result<(), String>`, `remote_pair_cancel(ticket: u64)`, `remote_unpair(device: String)`, `remote_set_keep_awake(on: bool)`.

- [ ] **Step 1: Write the failing tests** (`gateway_tests.rs`)

```rust
use std::net::{Ipv4Addr, SocketAddr};

use super::gateway::{bind_addresses, primary_address};

#[test]
fn the_listener_binds_each_address_it_can_and_never_everything() {
    // 0.0.0.0 is refused outright; an address that is not an address is skipped and reported;
    // the port is the configured one unless the address carries its own.
    let planned = bind_addresses(
        Some(Ipv4Addr::new(192, 168, 9, 155)),
        &["0.0.0.0".into(), "100.64.0.9".into(), "not an address".into(), "10.0.0.5:7800".into()],
        7777,
    );
    assert_eq!(
        planned.bind,
        vec![
            SocketAddr::from(([192, 168, 9, 155], 7777)),
            SocketAddr::from(([100, 64, 0, 9], 7777)),
            SocketAddr::from(([10, 0, 0, 5], 7800)),
        ]
    );
    assert_eq!(planned.skipped, vec![("0.0.0.0".to_owned(), "never every interface".to_owned()), ("not an address".to_owned(), "not an address".to_owned())]);
}

#[test]
fn with_no_primary_address_only_the_configured_ones_are_bound() {
    let planned = bind_addresses(None, &["100.64.0.9".into()], 7777);
    assert_eq!(planned.bind, vec![SocketAddr::from(([100, 64, 0, 9], 7777))]);
}

#[test]
fn the_primary_address_is_never_loopback_or_unspecified() {
    if let Some(ip) = primary_address() {
        assert!(!ip.is_loopback() && !ip.is_unspecified(), "{ip}");
    }
}

#[tokio::test]
async fn the_desk_adapter_takes_once_and_withdraws_by_key() {
    use purlis_session_protocol::gateway::{Offered, PairingDesk};
    let dir = tempfile::tempdir().unwrap();
    let desk = std::sync::Arc::new(purlis_core::remote::pairing::Desk::new(dir.path().to_path_buf()));
    let host = purlis_core::remote::linkkey::LinkKey::from_private([9u8; 32]);
    desk.open_window(&host, "01J9DEVCE00000000000000000", vec![], std::time::Instant::now());
    let adapter = super::adapters::DeskAdapter::new(desk.clone());
    assert!(adapter.take().is_some());
    assert!(adapter.take().is_none(), "one try per code");
    let offer = Offered { device: "01J9PHONE00000000000000000".into(), name: "phone".into(), key: [1u8; 32], fingerprint: "f".into() };
    let verdict = adapter.offered(offer.clone());
    assert!(desk.pending().is_some());
    adapter.withdraw(&offer);
    assert!(desk.pending().is_none(), "withdrawn by key");
    drop(verdict);
}

#[tokio::test]
async fn a_revocation_on_the_desk_bumps_the_watch() {
    let dir = tempfile::tempdir().unwrap();
    let desk = std::sync::Arc::new(purlis_core::remote::pairing::Desk::new(dir.path().to_path_buf()));
    let mut revoked = super::adapters::revocations(&desk);
    purlis_core::remote::peers::add(dir.path(), purlis_core::remote::peers::Peer { device: "01J9PHONE00000000000000000".into(), name: "p".into(), key: [1u8; 32], paired: 1 }).unwrap();
    desk.unpair("01J9PHONE00000000000000000").unwrap();
    assert!(revoked.has_changed().unwrap());
    assert_eq!(*revoked.borrow_and_update(), 1);
}
```

- [ ] **Step 2: Run (the user) to see it fail; Step 3: write `adapters.rs` and `gateway.rs`**

`adapters.rs` (the shapes above; `audit` as a submodule with three functions that lock
`Events` and call `purlis_core::remote::audit::record`, mapping `Heard::Linked → Act::Linked`,
`Refused → Act::Refused`, `Paired → Act::Paired`, `PairRefused → Act::Refused` with the key,
`Unlinked →` nothing). `gateway.rs`:

```rust
pub struct Planned { pub bind: Vec<SocketAddr>, pub skipped: Vec<(String, String)> }

pub fn bind_addresses(primary: Option<Ipv4Addr>, configured: &[String], port: u16) -> Planned {
    let mut planned = Planned { bind: vec![], skipped: vec![] };
    if let Some(ip) = primary { planned.bind.push(SocketAddr::from((ip, port))); }
    for text in configured {
        let parsed = text.parse::<SocketAddr>().or_else(|_| text.parse::<IpAddr>().map(|ip| SocketAddr::new(ip, port)));
        match parsed {
            Ok(addr) if addr.ip().is_unspecified() => planned.skipped.push((text.clone(), "never every interface".into())),
            Ok(addr) if !planned.bind.contains(&addr) => planned.bind.push(addr),
            Ok(_) => {}
            Err(_) => planned.skipped.push((text.clone(), "not an address".into())),
        }
    }
    planned
}

/// The address this machine reaches the world from: a UDP socket "connected" to a public
/// address sends nothing and learns which local address the route uses.
pub fn primary_address() -> Option<Ipv4Addr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:53").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}
```

`Remote::start`: build `Gateway { identity, host_device, pins, desk, revoked, host: Board::new(Arc::new(app.clone()), DeviceOf{…}) … }` — one `Board` per link needs the link's key: wrap `Gateway`'s host as a factory? `Gateway.host: H` is one value cloned per link; `Board` carries `DeviceOf` for the audit, which differs per link. **Ruling for the executor:** add to the protocol crate's `Gateway` a `host_for: Arc<dyn Fn(&ProvenDevice) -> H + Send + Sync>` in place of `host: H` (a small 1B amendment, with its test `each_link_gets_a_host_made_for_its_key` in the gateway test file), so the board knows its phone. Then `remote_status` and the rest are thin: read the store, call `Remote`, return rows (`PeerRow { device, name, fingerprint, paired }` from `peers::read`).

Audit on `Heard` goes through `on_event` → `adapters::audit::heard`. `start_if_on(&app)` runs at setup after the planes exist; `Remote::stop` on `remote_set_on(false)` and from the app's quit path (find where `Planes::let_go_of_all` is called on quit and stop beside it).

- [ ] **Step 4: Run (the user), clippy, commit**

Run: `cargo test -p purlis-app remote 2>&1 | grep "test result"` → all pass; `cargo clippy -p purlis-app --all-targets -- -D warnings` clean; the TypeScript bindings regenerate on the next `cargo build` of the app (`BINDINGS`), and `npm run typecheck` in `app/` still passes.

```bash
git add app/src-tauri crates/session-protocol && git commit -q -m "App: the gateway's adapters, its listener's lifecycle and the commands the window will call"
```

---

### Task 6: Keep this Mac awake while chats are running

**Files:**
- Modify: `app/src-tauri/src/remote/awake.rs`, `app/src-tauri/src/remote/gateway.rs` (`Remote::reconsider_awake(&self, app)`), wherever the app hears a chat move (`hooks::Moved` tell, in `lib.rs`'s `Planes::telling` closure: call `reconsider_awake` after the window is told)
- Test: `app/src-tauri/src/remote/awake_tests.rs`

**Interfaces:**
- Produces: `awake::Holder::new(program: Program)` where `Program { fn spawn(&self) -> std::io::Result<Child> }` (real: `caffeinate -i -w <pid>` on macOS, `systemd-inhibit --what=idle --who=purlis --why="chats are running" sleep infinity` on Linux; a test double runs `sleep 60`); `Holder::set(&self, wanted: bool)` (spawns or kills; idempotent); `Holder::held(&self) -> bool`; `awake::wanted(keep_awake: bool, any_working: bool) -> bool`.

- [ ] **Step 1: Write the failing tests**

```rust
use super::awake::{Holder, Program, wanted};

#[test]
fn the_setting_and_a_working_chat_are_both_needed() {
    assert!(!wanted(false, true));
    assert!(!wanted(true, false));
    assert!(wanted(true, true));
}

#[test]
fn the_assertion_follows_the_working_chats() {
    let holder = Holder::new(Program::test_double());
    holder.set(true);
    assert!(holder.held());
    let pid = holder.child_pid().unwrap();
    holder.set(true);
    assert_eq!(holder.child_pid(), Some(pid), "idempotent");
    holder.set(false);
    assert!(!holder.held());
    assert!(holder.child_pid().is_none());
}

#[test]
fn the_real_program_is_the_platforms_and_bound_to_this_process() {
    let real = Program::real();
    #[cfg(target_os = "macos")]
    assert_eq!(real.argv()[0], "caffeinate");
    #[cfg(target_os = "linux")]
    assert_eq!(real.argv()[0], "systemd-inhibit");
    assert!(real.argv().iter().any(|a| a == &std::process::id().to_string()) || cfg!(target_os = "linux"));
}
```

- [ ] **Step 2: Run (the user); Step 3: write `awake.rs`**

```rust
//! "Keep this Mac awake while chats are running" (iOS design §6.4, D10): while the setting is
//! on and a chat is `working` or `needs_you`, a child holds the operating system's idle
//! assertion; it is released when none is, when the setting turns off, and when the app
//! quits. A child, not an FFI call: the assertion API needs `unsafe`, which AGENTS.md keeps
//! to two audited blocks, and `caffeinate -w <pid>` is bound to this process so a crash lets
//! the Mac sleep.

use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, PoisonError};

pub fn wanted(keep_awake: bool, any_working: bool) -> bool {
    keep_awake && any_working
}

#[derive(Clone)]
pub struct Program(Vec<String>);

impl Program {
    pub fn real() -> Program {
        let pid = std::process::id().to_string();
        if cfg!(target_os = "macos") {
            Program(vec!["caffeinate".into(), "-i".into(), "-w".into(), pid])
        } else {
            Program(vec!["systemd-inhibit".into(), "--what=idle".into(), "--who=purlis".into(), "--why=chats are running".into(), "sleep".into(), "infinity".into()])
        }
    }

    #[cfg(test)]
    pub fn test_double() -> Program {
        Program(vec!["sleep".into(), "60".into()])
    }

    pub fn argv(&self) -> &[String] {
        &self.0
    }

    fn spawn(&self) -> std::io::Result<Child> {
        Command::new(&self.0[0]).args(&self.0[1..]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
    }
}

pub struct Holder {
    program: Program,
    child: Mutex<Option<Child>>,
}

impl Holder {
    pub fn new(program: Program) -> Holder {
        Holder { program, child: Mutex::new(None) }
    }

    pub fn set(&self, wanted: bool) {
        let mut child = self.child.lock().unwrap_or_else(PoisonError::into_inner);
        match (wanted, child.as_mut()) {
            (true, None) => *child = self.program.spawn().ok(),
            (true, Some(running)) if matches!(running.try_wait(), Ok(Some(_))) => *child = self.program.spawn().ok(),
            (false, Some(running)) => {
                let _ = running.kill();
                let _ = running.wait();
                *child = None;
            }
            _ => {}
        }
    }

    pub fn held(&self) -> bool {
        self.child.lock().unwrap_or_else(PoisonError::into_inner).is_some()
    }

    pub fn child_pid(&self) -> Option<u32> {
        self.child.lock().unwrap_or_else(PoisonError::into_inner).as_ref().map(Child::id)
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        self.set(false);
    }
}
```

`Remote::reconsider_awake(&self, app)`: `any_working` = any held plane's `chats().open_now()`
whose `board().glance(session)` is `Running` or asking; `wanted(store.keep_awake, any_working)`
→ `holder.set(..)`. Called after every `Moved` the window is told, and from
`remote_set_keep_awake`.

- [ ] **Step 4: Run (the user), clippy, commit**

```bash
git add app/src-tauri && git commit -q -m "App: keep this Mac awake while chats are running, through caffeinate or systemd-inhibit"
```

---

### Task 7: Docs and the dossier

- Spec §7's Phase 1 row: "**1C, the desktop's wiring, is built:** `docs/superpowers/plans/2026-10-12-ios-phase-1c-app-wiring.md`."; §3's unit table row for the app: "the app holds the `Host` adapter (`app/src-tauri/src/remote/board.rs`), the timeline adapter, the gateway's lifecycle and keep-awake".
- `~/purlis-research/README.md` §10: 1C done, 1D next.

```bash
git add docs/superpowers/specs/2026-10-11-ios-mobile-design.md && git commit -q -m "Design: Phase 1C built"
```

---

## Self-review (done while writing)

- **Spec coverage:** §4.1 keys → Tasks 0, 1; §4.3 answers → Tasks 2, 3 (`elicits_a_secret` and `choice_lasts` from the ask itself; `resize`/`start` by the table); §4.4 audit → Task 5's `adapters::audit` with Task 3's calls, no text; §5.1 level-2 items → Task 4; §5.2 prompt by level → Task 3 (level 3 is Phase 2); §5.5 → Task 3; §6.4 → Task 6; §8 "Gateway" tests: prompt routing, `choice_lasts`, timeline from recorded reports, nothing of a message's text in the log → Tasks 3, 4, 5. The window's pixels (Settings → Devices, the Pair sheet, the QR code, the keep-awake toggle, the changelog fragment) are 1D's.
- **Placeholders:** the two spots that say "check" name the grep and the fallback; `Gateway.host_for` is an explicit 1B amendment with its test named.
- **Type consistency:** `Board` is `Clone` through an `Arc`, as `Gateway<H: Host + Clone>` needs; `Found` is `pub(super)` for `timeline`; `DeviceOf` is what the audit calls take; the state words are 1A's four.
- **Review Focus:** each line names its test and task.
- **The sandbox:** the app's tests run only outside it (the Tauri build script writes inside the clone); the plan says "the user" at each such step, and the ledger records what was run where.
