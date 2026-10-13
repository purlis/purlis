# iOS Phase 1E: `purlis-mobile-core` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The phone's Rust side (iOS design §3, §4.2, §6.2, §6.3): its link key and device id, the desktops it has paired with, pairing from a scanned code, a link to a desktop that lists chats, opens views and timelines, prompts, answers and stops, the `subscribe(since)` cursor per desktop, and a reconnect that tries each address and says when the desktop is unreachable — plain Rust, tested against a real gateway in process, with no Tauri in the crate.

**Architecture:** A new workspace crate `crates/purlis-mobile-core` over `purlis-session-protocol` (features `noise`, `any-stream` for the duplex tests). A `Store` trait keeps the phone's few records (its identity, its name, the pinned desktops, the cursors) as strings: `MemoryStore` for tests, `FileStore` (0600 JSON files under the app's data dir) for the phone, and `KeychainStore` for the link key through the `keyring` crate (the macOS/iOS Keychain), chosen by the app in 1F. `pair` runs Noise XX against the scanned host key, sends the one proof frame and reads the verdict. `Link` wraps the protocol's `Client` plus a task that takes each stream the host opens and hands it to whoever attached that view or timeline. `Session` owns a desktop's link across reconnects: it tries the desktop's addresses in order with a short timeout, keeps the `subscribe` cursor, and re-attaches what was open. Every network function is generic over the IO, so the tests run over `tokio::io::duplex` and the phone over TCP. The Tauri commands are 1F's (`mobile/src-tauri`), one thin layer over this crate.

**Tech Stack:** Rust 2024, tokio, `purlis-session-protocol` (`noise`, `any-stream`), `keyring` 4, `ulid`, `serde`/`serde_json`, `hex` through the protocol's own helpers.

**Spec:** `docs/superpowers/specs/2026-10-11-ios-mobile-design.md` — §3 (`crates/purlis-mobile-core`: connection, reconnect from cursor, the link key in the iOS Keychain, a small set of Tauri commands), §4.2 (pairing: scan, Noise XX, one proof frame, the window's verdict, both sides pin; unpair), §4.3 (what `remote-device` may do), §6.2 (backgrounding: reconnect, `subscribe(since)` from the cursor, re-attach), §6.3 (disconnected: Connected / Reconnecting… / Desktop unreachable; nothing queued), §8 (`purlis-mobile-core`: integration tests against a real gateway in process, plain Rust).

## Global Constraints

- **No dependency on `purlis-core`**: the phone carries no desktop code. The QR payload's shape (`purlis-pair:` + JSON `{version, host_device, host_key, addresses, code}`) is copied, with a test that the two parse each other's output kept in core's test support is out of reach, so the shape is pinned by a literal in this crate's test.
- **The code and the link key never reach a log line**: `Debug` on anything holding them redacts (as core's `Payload` does).
- **Nothing is queued to be sent later** (§6.3): a call on a link that is down fails now with `Error::NotConnected`; the app says so.
- **A choice that lasts is never offered** (D8): `Link::answer` is a pass-through, the filtering is the UI's from `Choice::lasts`; this crate does not hide it, so the UI's test can see it.
- **Commit messages plain.** No `unsafe`. Clippy `-D warnings` on the crate.

## Review Focus

1. A desktop whose key changed (it rotated its link key): the handshake fails against the pinned key, the session says `Unreachable` with the reason "the desktop's key is not the one you paired with", and nothing re-pairs on its own — Task 4's test `a_desktop_with_another_key_is_refused_not_re_paired`.
2. A link dropped mid-attach: the reader channel ends, the app sees `None`, and a re-attach on the next link gets a new epoch — Task 3's test `a_view_reader_ends_when_the_link_drops`.
3. A pairing code typed in by hand (the Copy pairing text path): `Payload::from_qr_text` takes the same text; leading and trailing whitespace is trimmed — Task 2's test.
4. Two desktops paired at once: cursors and hosts are keyed by the desktop's device id, never by address — Task 1's and Task 4's tests.
5. The file store on a phone with a full disk: a failed write refuses the operation and keeps the old record (write to a temp file, then rename) — Task 1's test `a_write_that_fails_leaves_the_old_record`.

---

### Task 0: The crate

**Files:**
- Create: `crates/purlis-mobile-core/Cargo.toml`, `crates/purlis-mobile-core/src/lib.rs`
- Modify: `Cargo.toml` (workspace `members` and `default-members` gain `crates/purlis-mobile-core`), `Cargo.lock` (the new package's entry: `purlis-mobile-core` depending on `keyring`, `purlis-session-protocol`, `serde`, `serde_json`, `tempfile`, `thiserror`, `tokio`, `ulid` — written by hand as 1B's was, since the sandbox cannot run cargo against the clone; verified with `cargo test --locked` in place)

```toml
[package]
name = "purlis-mobile-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "The phone's side of purlis: its link key, the desktops it paired with, and the link to one."

[dependencies]
purlis-session-protocol = { path = "../session-protocol", features = ["noise"] }
keyring = "4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["io-util", "macros", "net", "rt", "sync", "time"] }
ulid = "1"

[dev-dependencies]
purlis-session-protocol = { path = "../session-protocol", features = ["any-stream", "noise"] }
tempfile = "3"
tokio = { version = "1", features = ["io-util", "macros", "net", "rt-multi-thread", "sync", "test-util", "time"] }
```

(Check the versions against the lockfile: `thiserror`, `ulid`, `keyring`, `tempfile` as core uses them; the `keyring` features core enables.)

`lib.rs`:

```rust
//! The phone's side of purlis (iOS design §3): what it keeps (its link key, its name, the
//! desktops it paired with, where each subscription stands), how it pairs ([`pair`]), the
//! link to a desktop ([`link`]) and a desktop across reconnects ([`session`]). Plain Rust: the
//! Tauri commands are the phone app's, one thin layer over this.

pub mod hosts;
pub mod identity;
pub mod link;
pub mod pair;
pub mod payload;
pub mod session;
pub mod store;

/// Why something here failed, in words the app shows.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Store(String),
    #[error("not a purlis pairing code: {0}")]
    Payload(String),
    #[error("the desktop refused the pairing: {0}")]
    PairingRefused(String),
    #[error("the desktop's key is not the one you paired with")]
    WrongKey,
    #[error("no link to the desktop")]
    NotConnected,
    #[error("the desktop is unreachable: {0}")]
    Unreachable(String),
    #[error("{0}")]
    Link(String),
    #[error(transparent)]
    Refused(#[from] purlis_session_protocol::session::Refusal),
}

impl From<purlis_session_protocol::session::CallError> for Error {
    fn from(e: purlis_session_protocol::session::CallError) -> Self {
        use purlis_session_protocol::session::CallError;
        match e {
            CallError::Refused(r) => Error::Refused(r),
            CallError::Closed(_) => Error::NotConnected,
            CallError::Unexpected(why) => Error::Link(why),
        }
    }
}
```

- [ ] Build: `cargo build --locked --offline -p purlis-mobile-core` (with the modules as one-line stubs first). Commit: `Mobile core: the crate`.

---

### Task 1: The store, the identity and the hosts

**Files:** `src/store.rs`, `src/identity.rs`, `src/hosts.rs`; tests `tests/the_phone_keeps_its_key_its_name_and_its_desktops.rs`.

`store.rs`:

```rust
//! Where the phone keeps its few records, each a string under a key: the link key, the
//! device's own id and name, the pinned desktops, the cursors. Three stores: memory (tests),
//! a directory of 0600 files (the app's data dir), and the Keychain for the link key.

use std::path::{Path, PathBuf};

use crate::Error;

pub trait Store: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, Error>;
    fn set(&self, key: &str, value: &str) -> Result<(), Error>;
    fn delete(&self, key: &str) -> Result<(), Error>;
}

#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<std::collections::HashMap<String, String>>);

impl Store for MemoryStore { /* lock, get/insert/remove */ }

/// One file per key under `dir`, written whole to a temporary file beside it and renamed over,
/// so a write that fails leaves the old record.
pub struct FileStore { dir: PathBuf }

impl FileStore {
    pub fn new(dir: impl Into<PathBuf>) -> FileStore
    fn path(&self, key: &str) -> PathBuf  // `<dir>/<key>.json`; a key is [a-z0-9-]+ (asserted)
}

impl Store for FileStore {
    fn get: read_to_string, NotFound → None
    fn set: create_dir_all(dir); write to `<path>.tmp` with 0600 (unix); rename over
    fn delete: remove_file, NotFound ok
}

/// The link key in the Keychain (`keyring`): service `purlis/phone`, account = the key.
pub struct KeychainStore;
impl Store for KeychainStore { keyring::Entry::new("purlis/phone", key) → get_password/set_password/delete_credential; NoEntry → None }
```

`identity.rs`:

```rust
//! Who this phone is: its Noise link key (minted once, in the Keychain on a phone) and the
//! device id and name the desktop shows ("Noro's iPhone").

use purlis_session_protocol::noise::Identity;

pub const LINK_KEY: &str = "link-key";      // 64 hex of the private key
pub const DEVICE: &str = "device";          // JSON {id, name}

#[derive(Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Device { pub id: String, pub name: String }   // Debug plain: no secret

/// The link key, minted and stored on the first ask.
pub fn link_key(store: &dyn Store) -> Result<Identity, Error>  // get LINK_KEY → from_private(unhex) | generate → set(private_hex)

/// The device, minted with `name` on the first ask (a ULID id); `None` name keeps a stored one.
pub fn device(store: &dyn Store, name: Option<&str>) -> Result<Device, Error>
pub fn rename(store: &dyn Store, name: &str) -> Result<Device, Error>
/// Forget the key: every pairing is void until the phone pairs again.
pub fn forget_link_key(store: &dyn Store) -> Result<(), Error>
```

(`unhex` and `hex` helpers in `store.rs` or a `hex.rs`: 32 bytes ↔ 64 lowercase hex, as core's `remote::{hex, unhex32}`.)

`hosts.rs`:

```rust
//! The desktops this phone paired with: each by its device id, with the key it pinned and
//! where to reach it. One record under `hosts`.

pub const HOSTS: &str = "hosts";

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Host { pub device: String, pub name: String, pub key: [u8; 32] (hex on disk via serde_with? — store `key_hex: String` and expose `key()`), pub addresses: Vec<String>, pub paired: u64 }

pub fn read(store) -> Result<Vec<Host>, Error>
pub fn add(store, host) -> Result<(), Error>      // replaces by device
pub fn remove(store, device) -> Result<Option<Host>, Error>
pub fn rename(store, device, name) -> Result<(), Error>
pub fn set_addresses(store, device, addresses) -> Result<(), Error>
pub fn find(store, device) -> Result<Option<Host>, Error>
```

Tests (one file): the link key minted once and read back the same; forget then mint gives another; the device id is a ULID and the name is kept and renamed; hosts add/replace/remove/rename/addresses; FileStore writes 0600 and `a_write_that_fails_leaves_the_old_record` (dir made read-only after a first write; second `set` errs; `get` still returns the first).

- [ ] Commit: `Mobile core: the store, the link key and the paired desktops`.

---

### Task 2: The payload and pairing

**Files:** `src/payload.rs`, `src/pair.rs`; tests `tests/a_phone_pairs_with_a_scanned_code_through_a_real_gateway.rs`.

`payload.rs`: `Payload { version, host_device, host_key, addresses, code }` with redacting `Debug`, `from_qr_text(text)` (trimmed; prefix `purlis-pair:`; JSON), `host_key()` → `[u8; 32]` (hex), `code()` → `pairing::Code::from_hex`.

`pair.rs`:

```rust
/// Pair over `io` (a TCP stream to one of the payload's addresses): Noise XX against the
/// scanned host key, the one proof frame, the desktop's verdict. `Ok` is the desktop to pin.
pub async fn pair<S>(io: S, me: &Identity, device: &Device, payload: &Payload) -> Result<Host, Error>
where S: AsyncRead + AsyncWrite + Unpin + Send + 'static
{
    let host_key = payload.host_key()?;
    let secured = noise::connect(io, me, &payload.host_device, Some(host_key)).await.map_err(|e| match e { noise::Error::WrongHost{..} => Error::WrongKey, e => Error::Link(e.to_string()) })?;
    let mut plain = noise::pump(secured);
    let proof = Proof { device: device.id.clone(), name: device.name.clone(), proof: pairing::prove(&payload.code()?, &me.public(), &host_key, &payload.host_device) };
    pairing::send_proof(&mut plain, &proof).await.map_err(|e| Error::Link(e.to_string()))?;
    match pairing::read_verdict(&mut plain).await.map_err(|e| Error::Link(e.to_string()))? {
        Verdict::Paired { device } => Ok(Host { device, name: String::new() /* the app names it */, key: host_key, addresses: payload.addresses.clone(), paired: now_secs() }),
        Verdict::Refused { why } => Err(Error::PairingRefused(why)),
    }
}

/// `pair` over TCP: each address in order, the first that connects within `timeout`.
pub async fn pair_tcp(payload, me, device, timeout: Duration) -> Result<Host, Error>
```

Test rig (in `tests/support/mod.rs`, shared by Tasks 2–4): a gateway as the protocol's gateway test builds it — `Pins` over a `Mutex<HashMap<[u8;32], Scope>>`, a `Desk` whose `take` hands a `Window` with a code the test chose and whose `offered` answers `Paired { device }` (or `Refused`) and pins the key, `host_for` → a `FakeHost` (Task 3's), and `on_event` collecting `Heard`. `fn phone_side(rig) -> (DuplexStream, JoinHandle)` spawns `gateway.serve_one(b)` and returns `a`. Tests: the happy path pins the desktop (hosts::add → `find` returns it with the payload's addresses and the scanned key); a wrong code → `PairingRefused` and no host; a payload with a different `host_key` than the gateway's identity → `WrongKey`; `from_qr_text` trims and refuses junk; the payload's `Debug` shows `code: …`.

- [ ] Commit: `Mobile core: the pairing code and pairing`.

---

### Task 3: The link

**Files:** `src/link.rs`; tests `tests/a_link_lists_attaches_prompts_answers_and_stops.rs`.

```rust
/// A link to a desktop: the protocol's client, and the streams the desktop opens routed to
/// whoever attached them.
pub struct Link { client: Client, routes: Arc<Mutex<Routes>>, router: JoinHandle<()>, events: Mutex<Option<Events>> }

struct Routes { views: HashMap<u32, mpsc::Sender<Chunk>>, timelines: HashMap<u32, mpsc::Sender<Item>>, viewer: Viewer, timelines_reader: Timelines }

impl Link {
    /// Open over `io`: Noise XX against the pinned key, then the session protocol as `remote-device`.
    pub async fn open<S>(io: S, me: &Identity, host: &Host) -> Result<Link, Error>
    pub async fn list(&self) -> Result<Vec<Chat>, Error>
    /// Attach a view: the answer, and the chunks as they come (`None` when the stream ends).
    pub async fn attach(&self, chat: &str) -> Result<(Attached, mpsc::Receiver<Chunk>), Error>
    pub async fn attach_timeline(&self, chat: &str) -> Result<(TimelineAttached, mpsc::Receiver<Item>), Error>
    pub async fn detach(&self, chat, view) / detach_timeline
    pub async fn write(&self, chat, bytes) / prompt(chat, text) / answer(chat, ask, answer) / stop(chat)
    pub async fn subscribe(&self, since: Since) -> Result<Events, Error>   // takes the events receiver once
    pub fn ack(&self, view: u32, bytes: usize)   // the view reader's ack, from the app, through the route
    pub fn close(self)
}
```

Routing: the router task loops `acceptor.accept()` → `stream::take` → by kind: `viewer.accept_opened` → `Reader` → spawn a task pumping `reader.next()` into the view's sender (registered before the attach call returns: `attach` inserts the sender under the view number from `Attached.view`; a stream that arrives before the number is known is parked by (view id, epoch) for 2 s then dropped). Acks: the view `Reader::ack` must be called as the app consumes; keep each view's `Reader` in the pump task and ack what was forwarded once the receiver has taken it (a bounded channel of 64 chunks: forward, then ack the chunk's length — the host's window is what the phone has not yet acked, so acking on forward bounds memory to the channel).

`FakeHost` in `tests/support`: a `session::Host` whose `call` records every command and answers: `List` → one level-2 chat `KEPT`; `Attach` → opens a view via `view::Attacher` with snapshot `"screen\r\n"` then pushes `"live\r\n"`; `AttachTimeline` → a timeline with `[Turn began, Ask{...}, CaughtUp]` then pushes `AskClosed`; `Write/Prompt/Answer/Stop/Detach*` → Done; `subscribe` → a channel the test pushes two `Pushed::Event`s on.

Tests: list; attach gets the snapshot then the live bytes; attach_timeline gets the items; prompt/answer/stop reach the host as the commands they are (`write` bytes verbatim, `prompt` text verbatim); `a_view_reader_ends_when_the_link_drops` (drop the gateway's side; the receiver yields `None`); a call after close is `NotConnected`.

- [ ] Commit: `Mobile core: the link`.

---

### Task 4: The session across reconnects

**Files:** `src/session.rs`; tests `tests/a_session_reconnects_from_its_cursor_and_says_when_the_desktop_is_unreachable.rs`.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing { Connected, Reconnecting, Unreachable(String) }

/// How a session reaches its desktop: TCP to the host's addresses, or, in a test, a dial the
/// test hands it.
pub trait Dial: Send + Sync { fn dial(&self, addresses: &[String]) -> Pin<Box<dyn Future<Output = Result<Box<dyn Io>, Error>> + Send>>; }
pub trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
pub struct Tcp { pub timeout: Duration }   // each address in order; the first that connects

pub struct Session { store: Arc<dyn Store>, me: Identity, host: Host, dial: Arc<dyn Dial>, link: Mutex<Option<Arc<Link>>>, standing: watch::Sender<Standing>, open: Mutex<Vec<Open>> /* what to re-attach */ }

impl Session {
    pub fn new(store, me, host, dial) -> Session
    pub fn standing(&self) -> watch::Receiver<Standing>
    /// Connect, or connect again: `Reconnecting` while it tries, then `Connected` or `Unreachable(why)`.
    pub async fn connect(&self) -> Result<(), Error>
    pub async fn link(&self) -> Result<Arc<Link>, Error>   // NotConnected when down
    /// `subscribe` from the cursor kept under `cursor:<device>`; each event advances it.
    pub async fn subscribe(&self) -> Result<Events, Error>
    pub fn cursor(&self) -> Since
    pub fn saw(&self, event: &Event)   // advances the cursor and stores it
    /// The chats whose view or timeline the app has open, to re-attach after a reconnect.
    pub fn opened(&self, chat: &str, what: Open) / closed(...)
    pub fn to_reattach(&self) -> Vec<Open>
}
```

Tests with a `Dial` over duplex to a rig (Task 2's support): connect → `Connected`; the gateway's side dropped → `link()` is `NotConnected` → `connect()` again → `Connected` with a new link, and `to_reattach` lists what was open; a dial that fails every address → `Unreachable("…")`; a rig whose identity differs from the pinned key → `Unreachable("the desktop's key is not the one you paired with")` and `hosts::find` still holds the old key (`a_desktop_with_another_key_is_refused_not_re_paired`); the cursor: `saw` two events for device D → `cursor()` is `{D: 2}`, stored under `cursor:<D>`, read back by a new `Session`; two sessions to two desktops keep two cursors.

- [ ] Commit: `Mobile core: a desktop across reconnects`.

---

### Task 5: Docs and the dossier

- Spec §3 row for `crates/purlis-mobile-core`: "**As built (1E):** …"; §7 row: "**1E, the phone's Rust core, is built:** …; the rest: 1F".
- `~/purlis-research/README.md` §10.

- [ ] Commit: `Design: Phase 1E built`.

## Self-review (done while writing)

- **Spec coverage:** §3's crate row (connection, reconnect from cursor, the link key in the Keychain) → Tasks 1, 3, 4 (Keychain through `keyring`, chosen by the app); §4.2 pairing from the phone's side → Task 2; §4.3 the commands a device may send → Task 3 (no `resize`, no `start`); §6.2 reconnect + `subscribe(since)` + re-attach → Task 4; §6.3 nothing queued → `NotConnected` now; §8 tests against a real gateway in process → the rig over duplex (the sandbox binds no port; TCP is one `Dial`).
- **Placeholders:** the sketches above name every function and type; the executor fills bodies from them.
- **Type consistency:** `Host`, `Device`, `Payload`, `Link`, `Session`, `Standing`, `Dial`, `Store` named once each and used by name.
- **Review Focus:** each line names its test and task.
