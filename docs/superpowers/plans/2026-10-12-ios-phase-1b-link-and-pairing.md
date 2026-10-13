# iOS Phase 1B: the link, the keys and the pairing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A paired phone's link: Noise `XX` against pinned keys in the protocol crate, the
pairing proof and its window, a gateway accept loop that admits a pinned key as
`remote-device` and drops any other before a frame, and in `purlis-core` the desktop's link
key in the Keyring, `<config>/peers.json`, one-time pairing codes with the window's
confirmation, unpairing that closes live links, and audit records that hold no text.

**Architecture:** Three layers, each testable alone. (1) `session-protocol` gains `noise`
(the handshake and an encrypting pump, behind a `noise` feature that pulls `snow`), `pairing`
(the one proof frame and its verdict) and `gateway` (serve one accepted stream: pinned →
`serve_as_a_device` as `remote-device`; unpinned inside a window → one proof frame then the
window's verdict; unpinned otherwise → dropped; plus `listen` for TCP). The gateway talks to the
host through two small traits, `Pins` and `PairingDesk`, and watches a revocation counter.
(2) `purlis-core::remote` holds the sync state: `linkkey` (the X25519 identity in the Keyring,
through `secrets::keyring::store`), `peers` (`peers.json`, written like `machine.json`),
`pairing` (codes, the QR payload, the pending confirmation the window answers, unpair bumping
the revocation counter) and `audit` (`remote.*` events through the `Recorder`). It depends on
neither tokio nor the protocol crate. (3) Plan 1C wires them in the app: the `Pins` and
`PairingDesk` adapters, the `session::Host` adapter over `SessionHost`/`ChatBoard`, the
runtime, Settings → Devices. Everything in (1) and (2) moves to `purlisd` unchanged.

**Tech Stack:** Rust 1.99, tokio, yamux (unchanged), `snow` 0.10
(`Noise_XX_25519_ChaChaPoly_BLAKE2s`), `hmac`/`sha2`/`subtle`/`getrandom` (already in both
crates), `serde_json`, `ulid` (in core).

**Spec:** `docs/superpowers/specs/2026-10-11-ios-mobile-design.md` §3, §4.1, §4.2, §4.4 and
§8 "Gateway: pairing", and `docs/adr/0078-…` §3 and §8 (the link key, Noise `XX`, the prologue,
the two pinned records). Plan 1A built the protocol additions this consumes
(`docs/superpowers/plans/2026-10-12-ios-phase-1a-protocol.md`).

**Two rulings against the spec's diagram, made here so the later plans inherit them:**

1. **The handshake, the pairing wire and the accept loop live in `session-protocol`, not in
   `purlis-core::remote`.** The crate's `ProvenDevice` doc already says only that handshake may
   mint one, `purlis-core` has no async runtime (tokio is a dev-dependency only) and does not
   depend on the protocol crate, and the phone (1D) needs the same handshake code. The spec's
   "gateway inside `purlis-core`" becomes: the sync state in core, the async link in the
   protocol crate, the wiring in the app. Cost if wrong: the module moves, the code does not.
2. **The `session::Host` adapter over `SessionHost`/`ChatBoard` is the app's (1C), not
   core's**, because those traits are defined in `app/src-tauri/src/host.rs`. It moves with
   them when `purlisd` splits. Cost if wrong: same.

## Global Constraints

- Local branch `ios-mobile-design` of `~/purlis-research/purlis`. **Never push, never open a PR,
  never write to GitHub.**
- Commit messages: plain "what was done" only. **No `Co-Authored-By`, no AI trailers, no mention
  of AI.** purlis's `commit-msg` hook adds `Assisted-by:` under the chat's `GIT_CONFIG_*` env;
  commit from a Terminal without it, or amend as 1A's plan says. If the executor's sandbox
  cannot write `.git`, record each task's commit message in the ledger for the user (1A's
  Setup ruling).
- AGENTS.md: **a store names its tier in `docs/plane-format.md` before any code writes it**
  (Task 0, before Task 4); nothing parses harness output; no `unsafe`; no colour; a change
  people would notice gets a `changes/<slug>.md` fragment: **this plan adds none** (nothing is
  reachable by a person until 1C's Settings → Devices and the listener switch; 1C adds the
  fragment for both).
- Spec §4.2: outside a pairing window an unpinned key is dropped **before any frame is read**;
  inside one, the link carries **one frame only**, the proof; a wrong proof, or anything but a
  proof, closes the link and the code is spent; the code lives 5 minutes and is single use;
  **only the window confirms** a pairing.
- Spec §4.4/D9: pairing, unpairing, connecting, answering and prompting go to the event log as
  metadata only (device id, chat, kind, choice id, key fingerprint); **never a prompt's,
  message's or answer's text**.
- ADR 0078 §3: cipher suite `25519_ChaChaPoly_BLAKE2s`; the prologue binds the protocol name
  and version and the host's device id; either side aborts on a key mismatch before any
  transport message; refusals record the offered key's fingerprint.
- The Keyring convention: every service purlis makes starts with `purlis/`
  (`secrets::keyring::SERVICE_PREFIX`); a fenced build (every test build) never reaches the
  OS store and uses the stub file store, so core's tests run against
  `secrets::keyring::store(&ctx)` as any build does.
- `peers.json` is Machine, device-bound, `0600`, written whole by a rename through
  `machine::write_beside`, under the machine store's `flock` (`machine::Lock`), denied to chats.
- Clippy `-D warnings`; `cargo fmt`. Check commands: `cargo test -p purlis-session-protocol
  --features noise`, `cargo test -p purlis-core --features fenced remote`, and the clippy lines
  in each task. In the chat's sandbox: `CARGO_TARGET_DIR=$TMPDIR/purlis-target`, and 15 of the
  protocol crate's socket tests fail there only (1A's ruling). `snow` may need fetching:
  `cargo fetch` once (crates.io).

## Review Focus

1. **An unpinned key outside a pairing window** must be dropped with no frame read and no
   byte of the session protocol sent, and the refusal recorded with the key's fingerprint
   (Task 3: `an_unpinned_key_outside_a_window_is_dropped_before_a_frame`).
2. **A proof that is right but never confirmed** (the person closes the window, or 5 minutes
   pass) must pin nothing and spend the code (Task 6:
   `an_unconfirmed_pairing_pins_nothing_and_spends_the_code`; Task 3:
   `a_verdict_that_never_comes_closes_the_link_when_the_window_ends`).
3. **A key revoked mid-link** must close that link at once, not at its next command (Task 3:
   `a_key_revoked_mid_link_closes_the_link_at_once`).
4. **The host's own key never leaves the Keyring in the clear**: `Identity`'s `Debug` prints no
   private half, the stub store holds hex the way the OS store would, and no log line carries it
   (Task 1: `an_identity_never_prints_its_private_half`; Task 4:
   `the_link_key_is_read_back_from_the_keyring_and_never_logged`).
5. **A second proof frame, or any frame that is not a proof, while a verdict is pending** must
   close the link and spend the code, so a phone cannot keep a window open by talking (Task 3:
   `anything_but_one_proof_frame_closes_the_link_and_spends_the_code`).

---

### Task 0: The rows in `docs/plane-format.md`, first

**Files:**
- Modify: `docs/plane-format.md` lines 6503 (`machine.json.lock`), 6528 (`events.jsonl`, the kinds), 6543 (`peers.json`), 6546 (the link key)
- Test: `crates/purlis-core/tests/every_store_the_plane_format_names_has_a_tier.rs` (already exists; it must stay green)

**Interfaces:** none.

- [ ] **Step 1: Widen the four rows**

Replace the `peers.json` row (line 6543) with:

```markdown
| `<config>/peers.json` | Machine, device-bound | **decided** (ADR 0078), **written from the iOS design's Phase 1B on** for a desktop's paired phones. On any host with paired devices: each paired device's id, its name as the person sees it, its pinned public link key (32 bytes, hex) and when it was paired, as `{"version":1,"peers":[{"device":"<ULID>","name":"…","key":"<hex>","paired":<secs>}]}`. The host refuses a link from any other key, and a device is admitted as `remote-device` (a phone) or, on a runner, `remote-link` (the desktop). Written whole by a rename, `0600`, under the machine store's lock. Denied to chats | `purlis_core::remote::peers`, at pairing and unpairing (the window, iOS design §4.2); on a runner its `purlisd` (RR-1) |
```

Replace the link-key row (line 6546) with:

```markdown
| keyring item for the link key | Keyring | **decided** (ADR 0078), **written from the iOS design's Phase 1B on**. This device's static X25519 key for the Noise `XX` handshake of a device link, apart from the device key: service `purlis/link-key`, account this device's id, value the private key as 64 hex digits, minted the first time the gateway or a pairing asks (`purlis_core::remote::linkkey`). Rotating it keeps the device id and needs a new pairing. On a headless runner the link key is held in the same age-encrypted form as the device key. Denied to chats | `purlis_core::remote::linkkey`; on a runner `purlis runner add` or its `purlisd` (RR-1) |
```

In the `machine.json.lock` row (line 6503), change "the `flock` that makes a read-modify-write of `machine.json` one act" to "the `flock` that makes a read-modify-write of `machine.json`, and of `peers.json` beside it, one act".

In the `events.jsonl` row (line 6528), after "`hook.unknown` (with the `word`, shortened) for a tool hook word it does not." insert:

```markdown
 `remote.paired`, `remote.unpaired`, `remote.linked`, `remote.refused`, `remote.answered` and `remote.prompted` (iOS design §4.4) for a paired device: each with `device` (the device's id), `fingerprint` (the first 16 hex digits of its public link key's SHA-256; for a refused link the key offered), `scope` (`remote-device`), and for `answered` the `chat`, `ask` and `choice` ids and for `prompted` the `chat` and the prompt's byte length — never the text of a prompt, a message or an answer (D9).
```

- [ ] **Step 2: Run the tier test**

Run: `cd ~/purlis-research/purlis && cargo test -p purlis-core --test every_store_the_plane_format_names_has_a_tier 2>&1 | tail -3`
Expected: `test result: ok`.

- [ ] **Step 3: Commit**

```bash
git add docs/plane-format.md && git commit -q -m "Plane format: peers.json and the link key on a desktop with paired phones, and the remote.* event kinds"
```

---

### Task 1: `noise`: the handshake and the encrypting pump

**Files:**
- Modify: `crates/session-protocol/Cargo.toml` (feature `noise`, dep `snow`)
- Create: `crates/session-protocol/src/noise.rs`
- Modify: `crates/session-protocol/src/lib.rs` (`#[cfg(feature = "noise")] pub mod noise;`), `crates/session-protocol/src/link.rs` (`ProvenDevice::proven`, `ProvenDevice::key`)
- Test: `crates/session-protocol/tests/a_noise_link_proves_both_keys_against_what_each_side_expects.rs`

**Interfaces:**
- Consumes: `link::ProvenDevice { scope }` (1A), `auth::Scope`.
- Produces:
  - `noise::PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s"`, `noise::PROTOCOL: &str = "purlis session protocol 1"` (the prologue's first part);
  - `noise::Identity { fn generate() -> Identity; fn from_private(bytes: [u8; 32]) -> Result<Identity, Error>; fn public(&self) -> [u8; 32]; fn private_hex(&self) -> String }`, `Debug` prints the public key only;
  - `noise::fingerprint(public: &[u8; 32]) -> String` (first 16 hex digits of SHA-256);
  - `noise::Error` (`Noise`, `Io`, `TooLong`, `WrongHost { expected, offered }`, `Prologue`);
  - `noise::Secured<S> { fn remote_public(&self) -> [u8; 32]; fn proven_as(self, scope: Scope) -> (ProvenDevice, DuplexStream) }`;
  - `noise::accept(io, ours: &Identity, host_device: &str) -> Result<Secured, Error>` (responder; `host_device` is this host's device id, in the prologue);
  - `noise::connect(io, ours: &Identity, host_device: &str, expected_host: Option<[u8; 32]>) -> Result<Secured, Error>` (initiator; aborts with `WrongHost` when the host's static key is not `expected_host`);
  - `noise::pump<S>(io: S, secured: Secured<S>) -> DuplexStream` (the plaintext end);
  - `link::ProvenDevice::key(&self) -> [u8; 32]`, `pub(crate) fn proven(scope, key)`.

- [ ] **Step 1: Add the feature and the dependency**

In `crates/session-protocol/Cargo.toml`, under `[dependencies]` after `tempfile.workspace = true`:

```toml
# A device link's Noise `XX` handshake (ADR 0078 §3, iOS design §4): the standard for proving
# two static keys over an untrusted stream, as WireGuard and libp2p do. Only with the `noise`
# feature, which the gateway and the phone turn on, so a host that serves `charterd.sock` alone
# carries no cipher.
snow = { version = "0.10", optional = true }
```

Under `[features]` add:

```toml
# `noise`, `pairing` and `gateway`: a device link over Noise `XX` against pinned keys.
noise = ["dep:snow"]
```

Under `[dev-dependencies]` change the self-dependency to
`purlis-session-protocol = { path = ".", features = ["any-stream", "noise"] }`.

Run: `cargo fetch 2>&1 | tail -2` (needs crates.io once).

- [ ] **Step 2: Write the failing test**

Create `crates/session-protocol/tests/a_noise_link_proves_both_keys_against_what_each_side_expects.rs`:

```rust
//! Noise `XX` over any ordered byte stream (ADR 0078 §3): each side learns the other's static
//! key inside the handshake, the client aborts when the host's key is not the one it pinned,
//! the prologue binds the host's device id, and what crosses the wire is never the plaintext.

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::noise::{self, Error, Identity};
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

const HOST: &str = "01J9DEVCE00000000000000000";

#[tokio::test]
async fn each_side_learns_the_others_static_key() {
    let (a, b) = duplex(64 * 1024);
    let host = Identity::generate();
    let phone = Identity::generate();
    let (served, connected) = tokio::join!(
        noise::accept(b, &host, HOST),
        noise::connect(a, &phone, HOST, Some(host.public()))
    );
    let served = served.unwrap();
    let connected = connected.unwrap();
    assert_eq!(served.remote_public(), phone.public());
    assert_eq!(connected.remote_public(), host.public());
}

#[tokio::test]
async fn the_client_refuses_a_host_whose_key_is_not_the_one_it_pinned() {
    let (a, b) = duplex(64 * 1024);
    let host = Identity::generate();
    let other = Identity::generate();
    let phone = Identity::generate();
    let (_served, connected) = tokio::join!(
        noise::accept(b, &host, HOST),
        noise::connect(a, &phone, HOST, Some(other.public()))
    );
    match connected {
        Err(Error::WrongHost { expected, offered }) => {
            assert_eq!(expected, other.public());
            assert_eq!(offered, host.public());
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_different_host_device_id_in_the_prologue_fails_the_handshake() {
    let (a, b) = duplex(64 * 1024);
    let host = Identity::generate();
    let phone = Identity::generate();
    let (served, connected) = tokio::join!(
        noise::accept(b, &host, HOST),
        noise::connect(a, &phone, "01J9OTHER0000000000000000", Some(host.public()))
    );
    assert!(served.is_err() || connected.is_err(), "the prologue did not bind the device");
}

#[tokio::test]
async fn plaintext_written_on_one_end_is_read_on_the_other_and_is_not_on_the_wire() {
    let (a, b) = duplex(64 * 1024);
    let host = Identity::generate();
    let phone = Identity::generate();
    let (served, connected) = tokio::join!(
        noise::accept(b, &host, HOST),
        noise::connect(a, &phone, HOST, Some(host.public()))
    );
    let (served, connected) = (served.unwrap(), connected.unwrap());
    // Tap the wire: the host's plaintext end writes, a tee reads the ciphertext.
    let (wire_a, wire_b) = duplex(64 * 1024);
    let mut host_plain = noise::pump(wire_a, served);
    let mut phone_plain = noise::pump(wire_b, connected);
    let secret = b"the quick brown fox jumps over the lazy dog, 70000 times over";
    let big: Vec<u8> = secret.iter().cycle().take(70_000).copied().collect();
    host_plain.write_all(&big).await.unwrap();
    host_plain.flush().await.unwrap();
    let mut got = vec![0u8; big.len()];
    phone_plain.read_exact(&mut got).await.unwrap();
    assert_eq!(got, big);
}

#[tokio::test]
async fn the_bytes_on_the_wire_are_not_the_plaintext() {
    // The pump's own wire is taken apart: a duplex pair where one half is read raw.
    let (a, b) = duplex(64 * 1024);
    let host = Identity::generate();
    let phone = Identity::generate();
    let (served, connected) = tokio::join!(
        noise::accept(b, &host, HOST),
        noise::connect(a, &phone, HOST, Some(host.public()))
    );
    let (served, _connected) = (served.unwrap(), connected.unwrap());
    let (wire_a, mut wire_b) = duplex(64 * 1024);
    let mut host_plain = noise::pump(wire_a, served);
    host_plain.write_all(b"PINEAPPLE PINEAPPLE PINEAPPLE").await.unwrap();
    host_plain.flush().await.unwrap();
    let mut raw = vec![0u8; 2 + 29 + 16];
    wire_b.read_exact(&mut raw).await.unwrap();
    assert!(!raw.windows(9).any(|w| w == b"PINEAPPLE"), "the plaintext is on the wire");
}

#[test]
fn an_identity_never_prints_its_private_half() {
    let id = Identity::generate();
    let shown = format!("{id:?}");
    assert!(!shown.contains(&id.private_hex()), "{shown}");
    assert!(shown.contains(&noise::fingerprint(&id.public())), "{shown}");
}

#[test]
fn a_fingerprint_is_sixteen_hex_digits_of_the_keys_hash() {
    let id = Identity::generate();
    let fp = noise::fingerprint(&id.public());
    assert_eq!(fp.len(), 16);
    assert!(fp.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_ne!(fp, noise::fingerprint(&Identity::generate().public()));
}

#[tokio::test]
async fn a_secured_stream_proven_as_a_scope_is_that_scope_with_that_key() {
    let (a, b) = duplex(64 * 1024);
    let host = Identity::generate();
    let phone = Identity::generate();
    let (served, _connected) = tokio::join!(
        noise::accept(b, &host, HOST),
        noise::connect(a, &phone, HOST, Some(host.public()))
    );
    let (device, _stream) = served.unwrap().proven_as(Scope::RemoteDevice);
    assert_eq!(device.scope(), Scope::RemoteDevice);
    assert_eq!(device.key(), phone.public());
}
```

- [ ] **Step 3: Run it to see it fail**

Run: `cargo test -p purlis-session-protocol --features noise --test a_noise_link_proves_both_keys_against_what_each_side_expects 2>&1 | grep -E "^error" | head -3`
Expected: FAIL to compile: `could not find noise in purlis_session_protocol`.

- [ ] **Step 4: Write `noise.rs`**

```rust
//! A device link's Noise `XX` handshake (ADR 0078 §3, iOS design §4): both static keys are
//! proved inside the stream, against what each side pinned, before any transport message.
//!
//! `XX` sends each side's static key encrypted inside the handshake: the host learns the
//! device's key in the third message and looks it up in its peers; the device compares the
//! host's key, from the second message, with the one it pinned at pairing (or read from the QR
//! code). Either side aborts on a mismatch before a transport message. The prologue binds the
//! protocol's name and the host's device id, so a link to a different host than the one named
//! fails even when that host would accept the key.
//!
//! **On the wire,** each Noise message is one frame: a big-endian `u16` length, then the
//! message. After the handshake every frame is one Noise transport message holding up to
//! [`MOST_PLAINTEXT`] bytes of the session protocol's own bytes ([`pump`]).
//!
//! This module never logs. [`Identity`]'s `Debug` prints the public key's fingerprint only.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};

use crate::auth::Scope;
use crate::link::ProvenDevice;

/// The Noise pattern and cipher suite (ADR 0078 §3). A change of suite is a protocol version.
pub const PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// The first part of the prologue: which protocol, which version of its handshake.
pub const PROTOCOL: &str = "purlis session protocol 1";

/// The most a Noise message holds, tag included.
const MOST_MESSAGE: usize = 65_535;

/// The most plaintext one transport message carries: the message less its 16-byte tag.
pub const MOST_PLAINTEXT: usize = MOST_MESSAGE - 16;

/// A device's static X25519 key pair. The private half never leaves this struct in the clear
/// except through [`Identity::private_hex`], for the Keyring.
#[derive(Clone)]
pub struct Identity {
    private: [u8; 32],
    public: [u8; 32],
}

impl Identity {
    /// A fresh key pair from the operating system's generator.
    pub fn generate() -> Identity {
        let pair = snow::Builder::new(pattern())
            .generate_keypair()
            .expect("the OS generator works");
        let mut private = [0u8; 32];
        private.copy_from_slice(&pair.private);
        let mut public = [0u8; 32];
        public.copy_from_slice(&pair.public);
        Identity { private, public }
    }

    /// The key pair whose private half is `private`, as read back from the Keyring.
    pub fn from_private(private: [u8; 32]) -> Result<Identity, Error> {
        let public = x25519_public(&private)?;
        Ok(Identity { private, public })
    }

    pub fn public(&self) -> [u8; 32] {
        self.public
    }

    /// The private half as 64 hex digits: what the Keyring item holds.
    pub fn private_hex(&self) -> String {
        hex(&self.private)
    }
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Identity({})", fingerprint(&self.public))
    }
}

/// The public key from a private one: a scalar multiplication by the base point, through snow's
/// own resolver so there is one X25519 in the crate.
fn x25519_public(private: &[u8; 32]) -> Result<[u8; 32], Error> {
    use snow::resolvers::{CryptoResolver, DefaultResolver};
    use snow::types::Dh;
    let mut dh = DefaultResolver
        .resolve_dh(&snow::params::DHChoice::Curve25519)
        .ok_or(Error::Prologue)?;
    dh.set(private);
    let mut public = [0u8; 32];
    public.copy_from_slice(dh.pubkey());
    Ok(public)
}

/// A key's fingerprint, as the window and the audit show it: the first 16 hex digits of its
/// SHA-256.
pub fn fingerprint(public: &[u8; 32]) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(public))[..16].to_owned()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn pattern() -> snow::params::NoiseParams {
    PATTERN.parse().expect("the pattern is well formed")
}

/// Why a handshake, or the pump, failed. `Debug` and `Display` carry keys only as
/// fingerprints.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("noise: {0}")]
    Noise(#[from] snow::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("the peer sent a message of {0} bytes, more than a Noise message holds")]
    TooLong(usize),
    /// The host's static key is not the one this device pinned.
    #[error(
        "the host's key is {} and this device pinned {}: not the host that was paired",
        fingerprint(offered), fingerprint(expected)
    )]
    WrongHost {
        expected: [u8; 32],
        offered: [u8; 32],
    },
    /// The prologue could not be built.
    #[error("the handshake's prologue could not be built")]
    Prologue,
}

fn prologue(host_device: &str) -> Vec<u8> {
    let mut p = Vec::with_capacity(PROTOCOL.len() + 1 + host_device.len());
    p.extend_from_slice(PROTOCOL.as_bytes());
    p.push(0);
    p.extend_from_slice(host_device.as_bytes());
    p
}

fn builder<'a>(ours: &'a Identity, host_device: &'a [u8]) -> Result<snow::Builder<'a>, Error> {
    Ok(snow::Builder::new(pattern())
        .local_private_key(&ours.private)?
        .prologue(host_device)?)
}

/// One framed Noise message out: one buffer, one write, so no length waits for its body.
async fn send_frame<S: AsyncWrite + Unpin>(io: &mut S, message: &[u8]) -> Result<(), Error> {
    if message.len() > MOST_MESSAGE {
        return Err(Error::TooLong(message.len()));
    }
    let mut frame = Vec::with_capacity(2 + message.len());
    frame.extend_from_slice(&(message.len() as u16).to_be_bytes());
    frame.extend_from_slice(message);
    io.write_all(&frame).await?;
    io.flush().await?;
    Ok(())
}

async fn read_frame<S: AsyncRead + Unpin>(io: &mut S) -> Result<Vec<u8>, Error> {
    let mut len = [0u8; 2];
    io.read_exact(&mut len).await?;
    let mut message = vec![0u8; usize::from(u16::from_be_bytes(len))];
    io.read_exact(&mut message).await?;
    Ok(message)
}

/// A stream whose handshake is done: the other side's static key is known, and transport
/// messages may flow. Made only by [`accept`] and [`connect`].
pub struct Secured<S> {
    io: S,
    remote_public: [u8; 32],
    transport: snow::TransportState,
}

impl<S> Secured<S> {
    /// The other side's static key, as the handshake delivered it.
    pub fn remote_public(&self) -> [u8; 32] {
        self.remote_public
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send + 'static> Secured<S> {
    /// The host's word that the key is pinned as `scope`: the proof the link is served under,
    /// and the plaintext end of the encrypted stream.
    pub fn proven_as(self, scope: Scope) -> (ProvenDevice, DuplexStream) {
        let device = ProvenDevice::proven(scope, self.remote_public);
        (device, pump(self))
    }
}

/// The host's side: run `XX` as the responder with `ours`, binding `host_device` (this host's
/// device id) into the prologue, and return the stream with the device's static key known.
/// Whether that key is pinned is the caller's next question; nothing of the session protocol
/// has crossed yet.
pub async fn accept<S: AsyncRead + AsyncWrite + Unpin>(
    mut io: S,
    ours: &Identity,
    host_device: &str,
) -> Result<Secured<S>, Error> {
    let prologue = prologue(host_device);
    let mut hs = builder(ours, &prologue)?.build_responder()?;
    let mut buf = vec![0u8; MOST_MESSAGE];
    // <- e
    let m = read_frame(&mut io).await?;
    hs.read_message(&m, &mut buf)?;
    // -> e, ee, s, es
    let n = hs.write_message(&[], &mut buf)?;
    send_frame(&mut io, &buf[..n]).await?;
    // <- s, se
    let m = read_frame(&mut io).await?;
    hs.read_message(&m, &mut buf)?;
    finish(io, hs)
}

/// The device's side: run `XX` as the initiator with `ours`, binding `host_device` (the host's
/// device id as this device expects it) into the prologue, and abort before any transport
/// message when the host's static key is not `expected_host`. `None` accepts any host key: for
/// the first link of a pairing, where the QR code gave the key and the caller compares it.
pub async fn connect<S: AsyncRead + AsyncWrite + Unpin>(
    mut io: S,
    ours: &Identity,
    host_device: &str,
    expected_host: Option<[u8; 32]>,
) -> Result<Secured<S>, Error> {
    let prologue = prologue(host_device);
    let mut hs = builder(ours, &prologue)?.build_initiator()?;
    let mut buf = vec![0u8; MOST_MESSAGE];
    // -> e
    let n = hs.write_message(&[], &mut buf)?;
    send_frame(&mut io, &buf[..n]).await?;
    // <- e, ee, s, es
    let m = read_frame(&mut io).await?;
    hs.read_message(&m, &mut buf)?;
    if let Some(expected) = expected_host {
        let offered = remote_of(&hs);
        if offered != expected {
            return Err(Error::WrongHost { expected, offered });
        }
    }
    // -> s, se
    let n = hs.write_message(&[], &mut buf)?;
    send_frame(&mut io, &buf[..n]).await?;
    finish(io, hs)
}

fn remote_of(hs: &snow::HandshakeState) -> [u8; 32] {
    let mut key = [0u8; 32];
    key.copy_from_slice(
        hs.get_remote_static()
            .expect("XX has delivered the remote static key by now"),
    );
    key
}

fn finish<S>(io: S, hs: snow::HandshakeState) -> Result<Secured<S>, Error> {
    let remote_public = remote_of(&hs);
    Ok(Secured {
        io,
        remote_public,
        transport: hs.into_transport_mode()?,
    })
}

/// Pump plaintext between the returned stream and the wire, encrypting on the way out and
/// decrypting on the way in: two tasks, one per direction, both ending when either side closes
/// or a frame cannot be read. The transport state is behind a mutex that is never held across
/// an await.
pub fn pump<S>(secured: Secured<S>) -> DuplexStream
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (ours, theirs) = tokio::io::duplex(256 * 1024);
    let transport = Arc::new(Mutex::new(secured.transport));
    let (mut wire_r, mut wire_w) = tokio::io::split(secured.io);
    let (mut plain_r, mut plain_w) = tokio::io::split(theirs);

    let out = Arc::clone(&transport);
    let outbound = tokio::spawn(async move {
        let mut plain = vec![0u8; MOST_PLAINTEXT];
        let mut cipher = vec![0u8; MOST_MESSAGE];
        loop {
            let n = match plain_r.read(&mut plain).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let len = {
                let mut t = out.lock().unwrap_or_else(|p| p.into_inner());
                match t.write_message(&plain[..n], &mut cipher) {
                    Ok(len) => len,
                    Err(_) => break,
                }
            };
            if send_frame(&mut wire_w, &cipher[..len]).await.is_err() {
                break;
            }
        }
        let _ = wire_w.shutdown().await;
    });
    let inbound = tokio::spawn(async move {
        let mut plain = vec![0u8; MOST_MESSAGE];
        loop {
            let frame = match read_frame(&mut wire_r).await {
                Ok(frame) => frame,
                Err(_) => break,
            };
            let n = {
                let mut t = transport.lock().unwrap_or_else(|p| p.into_inner());
                match t.read_message(&frame, &mut plain) {
                    Ok(n) => n,
                    Err(_) => break,
                }
            };
            if plain_w.write_all(&plain[..n]).await.is_err() {
                break;
            }
        }
        let _ = plain_w.shutdown().await;
        outbound.abort();
    });
    ours
}
```

Note the test's `noise::pump(wire_a, served)` shape: the pump takes the `Secured`, which owns its
stream. Change the two pump tests to build the handshake over the real pair and pump the same
`Secured`s: replace `let (wire_a, wire_b) = duplex(..); let mut host_plain = noise::pump(wire_a, served); let mut phone_plain = noise::pump(wire_b, connected);` with
`let mut host_plain = noise::pump(served); let mut phone_plain = noise::pump(connected);`, and
in `the_bytes_on_the_wire_are_not_the_plaintext` tap the wire differently: hand `accept` a
`duplex` half whose other half you read raw after the handshake, i.e.

```rust
    let (a, b) = duplex(64 * 1024);
    let (tap_in, tap_out) = duplex(64 * 1024);
    // The host's side runs over `b`; the phone's over `a`. After the handshake the phone's
    // Secured is dropped and the raw bytes the host writes are read from `a` by hand.
    let host = Identity::generate();
    let phone = Identity::generate();
    let (served, connected) = tokio::join!(noise::accept(b, &host, HOST), noise::connect(a, &phone, HOST, Some(host.public())));
    let served = served.unwrap();
    let mut raw_end = connected.unwrap().into_io();
    let _ = (tap_in, tap_out);
    let mut host_plain = noise::pump(served);
    host_plain.write_all(b"PINEAPPLE PINEAPPLE PINEAPPLE").await.unwrap();
    host_plain.flush().await.unwrap();
    let mut raw = vec![0u8; 2 + 29 + 16];
    raw_end.read_exact(&mut raw).await.unwrap();
    assert!(!raw.windows(9).any(|w| w == b"PINEAPPLE"), "the plaintext is on the wire");
```

with `Secured::into_io(self) -> S` added (`pub fn into_io(self) -> S { self.io }`, for tests
and the gateway's refusal path, which drops the stream). Drop the unused `tap_in`/`tap_out`
lines.

In `link.rs`, make `ProvenDevice` carry the key and add the constructor:

```rust
#[derive(Debug)]
pub struct ProvenDevice {
    scope: Scope,
    key: [u8; 32],
}

impl ProvenDevice {
    /// The scope the handshake proved the device to be: which pinned record its key matched.
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// The device's static public key, as the handshake delivered it.
    pub fn key(&self) -> [u8; 32] {
        self.key
    }

    /// What [`crate::noise::Secured::proven_as`] makes, and nothing else.
    pub(crate) fn proven(scope: Scope, key: [u8; 32]) -> ProvenDevice {
        ProvenDevice { scope, key }
    }
}
```

and `stand_in_as` fills `key: [0; 32]`. In `lib.rs` add `#[cfg(feature = "noise")] pub mod noise;` after `pub mod local;`'s block, and in the layer list a line "2b. [`noise`] (feature `noise`): a device link's handshake, which proves both static keys against the pinned records before anything else and encrypts what follows (ADR 0078 §3)".

- [ ] **Step 5: Run the test file, then every feature combination**

Run: `cargo test -p purlis-session-protocol --features noise --test a_noise_link_proves_both_keys_against_what_each_side_expects 2>&1 | grep -E "^test |test result|panicked" | head -12`
Expected: 8 PASS.

Run: `cargo test -p purlis-session-protocol 2>&1 | grep -E "^test result" | sort | uniq -c` and `cargo clippy -p purlis-session-protocol --all-targets --features noise -- -D warnings` and `cargo clippy -p purlis-session-protocol --all-targets -- -D warnings`
Expected: every target ok (the sandbox's 15 socket failures aside); both clippy runs clean (the
crate builds with and without `noise`).

- [ ] **Step 6: Commit**

```bash
cargo fmt -p purlis-session-protocol && git add crates/session-protocol Cargo.lock && git commit -q -m "Session protocol: a device link's Noise XX handshake against pinned keys, behind the noise feature"
```

---

### Task 2: `pairing`: one proof frame and its verdict

**Files:**
- Create: `crates/session-protocol/src/pairing.rs`
- Modify: `crates/session-protocol/src/lib.rs` (`#[cfg(feature = "noise")] pub mod pairing;`)
- Test: unit tests in `pairing.rs`, and `crates/session-protocol/tests/a_pairing_proof_is_one_frame_bound_to_both_keys_and_the_code.rs`

**Interfaces:**
- Consumes: `noise::{Identity, fingerprint}`, `version::{send, receive}` (make both `pub(crate)` callers inside the crate: they already are).
- Produces:
  - `pairing::Code([u8; 32])` with `Code::mint()`, `Code::from_hex`, `Code::to_hex`, constant-time `PartialEq`, `Debug` as `Code(…)`;
  - `pairing::Proof { device: String, name: String, proof: String }` (serde; the one frame);
  - `pairing::Verdict` serde enum: `Paired { device: String }` (the host's device id) / `Refused { why: String }`;
  - `pairing::prove(code: &Code, device_public: &[u8; 32], host_public: &[u8; 32], host_device: &str) -> String` (HMAC-SHA256 hex over `LABEL`, both keys, the host device id, each zero-terminated);
  - `pairing::verify(code: &Code, proof: &Proof, device_public, host_public, host_device) -> bool` (constant time);
  - `pairing::MOST_NAME_BYTES = 64`; `Proof::check(&self) -> Result<(), String>` (name length, device is a ULID);
  - `pairing::send_proof(io, &Proof)`, `pairing::read_proof(io) -> Result<Proof, Refused>`, `pairing::send_verdict`, `pairing::read_verdict` — the crate's `version` framing (MAGIC + length + JSON, `MOST_HELLO_BYTES`).

- [ ] **Step 1: Write the failing tests**

Unit tests inside `pairing.rs` (write them with the module in Step 3; they are the type's own
shape) and the integration test
`crates/session-protocol/tests/a_pairing_proof_is_one_frame_bound_to_both_keys_and_the_code.rs`:

```rust
//! The pairing proof (iOS design §4.2): one frame inside the encrypted channel, an HMAC under
//! the one-time code over both static keys and the host's device id, so a proof recorded on
//! one link proves nothing on another.

use purlis_session_protocol::noise::Identity;
use purlis_session_protocol::pairing::{self, Code, Proof, Verdict};
use tokio::io::duplex;

const HOST: &str = "01J9DEVCE00000000000000000";
const PHONE: &str = "01J9PHONE0000000000000000";

#[test]
fn a_proof_holds_only_for_its_code_keys_and_host() {
    let code = Code::mint();
    let host = Identity::generate();
    let phone = Identity::generate();
    let proof = Proof {
        device: PHONE.into(),
        name: "Noro's iPhone".into(),
        proof: pairing::prove(&code, &phone.public(), &host.public(), HOST),
    };
    assert!(pairing::verify(&code, &proof, &phone.public(), &host.public(), HOST));
    assert!(!pairing::verify(&Code::mint(), &proof, &phone.public(), &host.public(), HOST));
    assert!(!pairing::verify(&code, &proof, &Identity::generate().public(), &host.public(), HOST));
    assert!(!pairing::verify(&code, &proof, &phone.public(), &Identity::generate().public(), HOST));
    assert!(!pairing::verify(&code, &proof, &phone.public(), &host.public(), "01J9OTHER0000000000000000"));
}

#[tokio::test]
async fn the_proof_and_the_verdict_cross_as_one_frame_each() {
    let (mut a, mut b) = duplex(4096);
    let proof = Proof {
        device: PHONE.into(),
        name: "Noro's iPhone".into(),
        proof: "ab".repeat(32),
    };
    pairing::send_proof(&mut a, &proof).await.unwrap();
    let read = pairing::read_proof(&mut b).await.unwrap();
    assert_eq!(read, proof);
    pairing::send_verdict(&mut b, &Verdict::Paired { device: HOST.into() })
        .await
        .unwrap();
    assert_eq!(
        pairing::read_verdict(&mut a).await.unwrap(),
        Verdict::Paired { device: HOST.into() }
    );
}

#[tokio::test]
async fn a_frame_that_is_not_a_proof_is_refused_not_read_as_one() {
    use tokio::io::AsyncWriteExt;
    let (mut a, mut b) = duplex(4096);
    a.write_all(b"\x89CSP\r\n\x1a\n\x00\x0f{\"call\":{\"id\":1}}").await.unwrap();
    assert!(pairing::read_proof(&mut b).await.is_err());
}

#[test]
fn a_name_longer_than_the_bound_or_a_device_that_is_not_a_ulid_is_refused() {
    let long = Proof { device: PHONE.into(), name: "x".repeat(65), proof: String::new() };
    assert!(long.check().is_err());
    let odd = Proof { device: "phone".into(), name: "ok".into(), proof: String::new() };
    assert!(odd.check().is_err());
    let fine = Proof { device: PHONE.into(), name: "Noro's iPhone".into(), proof: String::new() };
    assert!(fine.check().is_ok());
}

#[test]
fn a_code_prints_nothing_of_itself_and_round_trips_hex() {
    let code = Code::mint();
    assert_eq!(format!("{code:?}"), "Code(…)");
    assert_eq!(Code::from_hex(&code.to_hex()).unwrap(), code);
    assert!(Code::from_hex("zz").is_err());
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p purlis-session-protocol --features noise --test a_pairing_proof_is_one_frame_bound_to_both_keys_and_the_code 2>&1 | grep -E "^error" | head -2`
Expected: FAIL to compile: `could not find pairing`.

- [ ] **Step 3: Write `pairing.rs`**

```rust
//! Pairing's one frame (iOS design §4.2): inside the encrypted channel a device not yet pinned
//! proves it holds the one-time code the window showed, by an HMAC under that code over both
//! static keys and the host's device id. The host answers with one verdict, once the window has
//! confirmed. Nothing else may cross such a link.
//!
//! The frames use the handshake's framing ([`crate::version`]): the magic, a length, JSON.
//! `{"pair":{"device":"<ULID>","name":"Noro's iPhone","proof":"<hex>"}}`, then
//! `{"paired":{"device":"<host ULID>"}}` or `{"refused":{"why":"…"}}`.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::version::{self, Refused};

/// What the proof is made over first, so a MAC the code made for anything else is never a
/// proof.
pub const LABEL: &[u8] = b"purlis session protocol 1: a device proves the pairing code";

/// The most bytes a device's name may be.
pub const MOST_NAME_BYTES: usize = 64;

/// A one-time pairing code: 256 bits from the operating system's generator, shown to the phone
/// inside the QR code, compared in constant time, never printed.
#[derive(Clone)]
pub struct Code([u8; 32]);

impl Code {
    pub fn mint() -> Code {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).expect("the OS generator works");
        Code(bytes)
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn from_hex(text: &str) -> Result<Code, String> {
        if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("a pairing code is 64 hex digits".into());
        }
        let mut bytes = [0u8; 32];
        for (i, pair) in text.as_bytes().chunks(2).enumerate() {
            bytes[i] = u8::from_str_radix(std::str::from_utf8(pair).unwrap_or("00"), 16)
                .map_err(|e| e.to_string())?;
        }
        Ok(Code(bytes))
    }
}

impl PartialEq for Code {
    fn eq(&self, other: &Self) -> bool {
        use subtle::ConstantTimeEq;
        bool::from(self.0.ct_eq(&other.0))
    }
}

impl Eq for Code {}

impl std::fmt::Debug for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Code(…)")
    }
}

/// The one frame a device sends on an unpinned link inside a pairing window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proof {
    /// The device's id, a ULID it minted for itself.
    pub device: String,
    /// What the person calls it, as the window shows it ("Pair 'Noro's iPhone'?").
    pub name: String,
    /// [`prove`]'s hex.
    pub proof: String,
}

impl Proof {
    /// Whether the frame is well formed: a ULID for the device, a name within the bound. Checked
    /// before the proof is, so a refusal of shape says nothing about the code.
    pub fn check(&self) -> Result<(), String> {
        if self.device.len() != 26 || !self.device.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err("a device id is a ULID".into());
        }
        if self.name.is_empty() || self.name.len() > MOST_NAME_BYTES {
            return Err(format!("a device's name is 1 to {MOST_NAME_BYTES} bytes"));
        }
        Ok(())
    }
}

/// The host's one answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Both sides pin: here is the host's device id for the phone's record.
    Paired { device: String },
    Refused { why: String },
}

/// The proof: HMAC-SHA256 keyed by the code over [`LABEL`], the device's public key, the
/// host's public key and the host's device id, each ended by a zero byte.
pub fn prove(code: &Code, device_public: &[u8; 32], host_public: &[u8; 32], host_device: &str) -> String {
    use hmac::{KeyInit, Mac};
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(&code.0)
        .unwrap_or_else(|_| unreachable!("HMAC takes a key of any length"));
    for part in [LABEL, device_public.as_slice(), host_public.as_slice(), host_device.as_bytes()] {
        mac.update(part);
        mac.update(&[0]);
    }
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Whether `proof` is [`prove`]'s answer for these keys and this code, compared in constant
/// time.
pub fn verify(code: &Code, proof: &Proof, device_public: &[u8; 32], host_public: &[u8; 32], host_device: &str) -> bool {
    use subtle::ConstantTimeEq;
    let expected = prove(code, device_public, host_public, host_device);
    bool::from(expected.as_bytes().ct_eq(proof.proof.as_bytes()))
}

#[derive(Serialize, Deserialize)]
struct Pair {
    pair: Proof,
}

pub async fn send_proof<W: AsyncWrite + Unpin>(io: &mut W, proof: &Proof) -> Result<(), Refused> {
    version::send(io, &Pair { pair: proof.clone() }).await
}

/// The one frame, or a refusal when it is not a proof. What was read past it is dropped: nothing
/// may follow a proof.
pub async fn read_proof<R: AsyncRead + Unpin>(io: &mut R) -> Result<Proof, Refused> {
    let (frame, _left): (Pair, _) = version::receive(io).await?;
    Ok(frame.pair)
}

pub async fn send_verdict<W: AsyncWrite + Unpin>(io: &mut W, verdict: &Verdict) -> Result<(), Refused> {
    version::send(io, verdict).await
}

pub async fn read_verdict<R: AsyncRead + Unpin>(io: &mut R) -> Result<Verdict, Refused> {
    let (verdict, _left): (Verdict, _) = version::receive(io).await?;
    Ok(verdict)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frames_are_snake_case_json() {
        let v = serde_json::to_value(Verdict::Paired { device: "d".into() }).unwrap();
        assert_eq!(v, serde_json::json!({"paired": {"device": "d"}}));
        let p = serde_json::to_value(Pair { pair: Proof { device: "d".into(), name: "n".into(), proof: "p".into() } }).unwrap();
        assert_eq!(p, serde_json::json!({"pair": {"device": "d", "name": "n", "proof": "p"}}));
    }
}
```

`version::Refused` must be `pub` (check: it is re-exported by `version`; `version::send` and
`receive` are `pub(crate)`, fine from here). Add `#[cfg(feature = "noise")] pub mod pairing;`
to `lib.rs`.

- [ ] **Step 4: Run, clippy, commit**

Run: `cargo test -p purlis-session-protocol --features noise --test a_pairing_proof_is_one_frame_bound_to_both_keys_and_the_code --lib pairing 2>&1 | grep -E "test result|panicked"`
Expected: 5 + 1 PASS.

```bash
cargo clippy -p purlis-session-protocol --all-targets --features noise -- -D warnings && cargo fmt -p purlis-session-protocol
git add crates/session-protocol && git commit -q -m "Session protocol: the pairing proof, one frame under the one-time code over both keys and the host's device id"
```

---

### Task 3: `gateway`: serve one stream by what its key is

**Files:**
- Create: `crates/session-protocol/src/gateway.rs`
- Modify: `crates/session-protocol/src/lib.rs` (`#[cfg(feature = "noise")] pub mod gateway;`)
- Test: `crates/session-protocol/tests/a_gateway_admits_a_pinned_key_pairs_inside_a_window_and_drops_the_rest.rs`

**Interfaces:**
- Consumes: `noise::{accept, Identity, Secured, fingerprint}`, `pairing::*`, `link::{serve_as_a_device, ProvenDevice}`, `session::{serve, Host, speaks}`, `auth::Scope`.
- Produces:
  - `gateway::Pins: Send + Sync` trait: `fn scope_of(&self, key: &[u8; 32]) -> Option<Scope>`;
  - `gateway::Offered { device: String, name: String, key: [u8; 32], fingerprint: String }`;
  - `gateway::Window { code: Code, ends: Instant }`;
  - `gateway::PairingDesk: Send + Sync` trait: `fn window(&self) -> Option<Window>` (the live code, if any), `fn spend(&self)` (the code is used up), `fn offered(&self, offer: Offered) -> Pin<Box<dyn Future<Output = Verdict> + Send>>` (resolves when the window answers; the desk pins on `Paired`);
  - `gateway::Revoked` = `tokio::sync::watch::Receiver<u64>` (a generation; a bump closes links whose key is no longer pinned);
  - `gateway::Heard` enum reported to `on_event: &dyn Fn(Heard)`: `Linked { key, scope }`, `Refused { key }`, `PairOffered { key }`, `Paired { key, device }`, `PairRefused { key, why }`, `Unlinked { key }` (the audit's input; no text);
  - `gateway::Gateway<H> { identity: Identity, host_device: String, pins: Arc<dyn Pins>, desk: Arc<dyn PairingDesk>, revoked: Revoked, host: H, on_event: Arc<dyn Fn(Heard) + Send + Sync> }` with `Gateway::serve_one<S>(&self, io: S) -> Result<(), String>` and `Gateway::listen(self: Arc<Self>, listener: tokio::net::TcpListener) -> JoinHandle<()>` (accept loop, `set_nodelay`, one task per link, a 10 s handshake timeout).

- [ ] **Step 1: Write the failing test**

Create `crates/session-protocol/tests/a_gateway_admits_a_pinned_key_pairs_inside_a_window_and_drops_the_rest.rs`:

```rust
//! The gateway (iOS design §4.2, §4.3): a pinned key is served the session protocol as
//! `remote-device`; an unpinned key inside a pairing window may send one proof and gets the
//! window's verdict; an unpinned key outside one is dropped before a frame; a key revoked
//! mid-link loses the link at once.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::gateway::{Gateway, Heard, Offered, PairingDesk, Pins, Window};
use purlis_session_protocol::link::{self, ProvenDevice};
use purlis_session_protocol::noise::{self, Identity};
use purlis_session_protocol::pairing::{self, Code, Proof, Verdict};
use purlis_session_protocol::session::{self, Answer, Client, Command, Host, Peer, Refusal};
use tokio::io::{AsyncReadExt, duplex};
use tokio::sync::{oneshot, watch};

const HOST: &str = "01J9DEVCE00000000000000000";
const PHONE: &str = "01J9PHONE0000000000000000";

#[derive(Clone, Default)]
struct Board;

impl Host for Board {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        assert_eq!(peer.scope(), Scope::RemoteDevice);
        match command {
            Command::List => Ok(Answer::Chats(vec![])),
            _ => Ok(Answer::Done),
        }
    }
}

#[derive(Default)]
struct Peers(Mutex<HashMap<[u8; 32], Scope>>);

impl Pins for Peers {
    fn scope_of(&self, key: &[u8; 32]) -> Option<Scope> {
        self.0.lock().unwrap().get(key).copied()
    }
}

/// A desk whose window and verdicts the test sets.
struct Desk {
    window: Mutex<Option<Window>>,
    spent: Mutex<u32>,
    offers: Mutex<Vec<(Offered, oneshot::Sender<Verdict>)>>,
}

impl Desk {
    fn with_window(code: Code, for_secs: u64) -> Arc<Desk> {
        Arc::new(Desk {
            window: Mutex::new(Some(Window {
                code,
                ends: Instant::now() + Duration::from_secs(for_secs),
            })),
            spent: Mutex::new(0),
            offers: Mutex::new(vec![]),
        })
    }

    fn closed() -> Arc<Desk> {
        Arc::new(Desk {
            window: Mutex::new(None),
            spent: Mutex::new(0),
            offers: Mutex::new(vec![]),
        })
    }

    fn spent(&self) -> u32 {
        *self.spent.lock().unwrap()
    }

    /// The pending offer, and the sender its verdict goes through.
    fn pending(&self) -> Option<(Offered, oneshot::Sender<Verdict>)> {
        self.offers.lock().unwrap().pop()
    }
}

impl PairingDesk for Desk {
    fn window(&self) -> Option<Window> {
        self.window.lock().unwrap().clone()
    }

    fn spend(&self) {
        *self.spent.lock().unwrap() += 1;
        *self.window.lock().unwrap() = None;
    }

    fn offered(&self, offer: Offered) -> Pin<Box<dyn std::future::Future<Output = Verdict> + Send>> {
        let (tx, rx) = oneshot::channel();
        self.offers.lock().unwrap().push((offer, tx));
        Box::pin(async move {
            rx.await.unwrap_or(Verdict::Refused {
                why: "the window closed".into(),
            })
        })
    }
}

struct Rig {
    gateway: Arc<Gateway<Board>>,
    host: Identity,
    peers: Arc<Peers>,
    desk: Arc<Desk>,
    heard: Arc<Mutex<Vec<Heard>>>,
    revoke: watch::Sender<u64>,
}

fn rig(desk: Arc<Desk>) -> Rig {
    let host = Identity::generate();
    let peers = Arc::new(Peers::default());
    let heard = Arc::new(Mutex::new(vec![]));
    let (revoke, revoked) = watch::channel(0u64);
    let sink = Arc::clone(&heard);
    let gateway = Arc::new(Gateway {
        identity: host.clone(),
        host_device: HOST.into(),
        pins: peers.clone(),
        desk: desk.clone(),
        revoked,
        host: Board,
        on_event: Arc::new(move |event| sink.lock().unwrap().push(event)),
    });
    Rig { gateway, host, peers, desk, heard, revoke }
}

/// A phone's side of a link: the handshake, then the plaintext end.
async fn phone_link(rig: &Rig, phone: &Identity) -> (tokio::io::DuplexStream, tokio::task::JoinHandle<Result<(), String>>) {
    let (a, b) = duplex(256 * 1024);
    let gateway = Arc::clone(&rig.gateway);
    let serving = tokio::spawn(async move { gateway.serve_one(b).await });
    let secured = noise::connect(a, phone, HOST, Some(rig.host.public())).await.unwrap();
    (noise::pump(secured), serving)
}

#[tokio::test]
async fn a_pinned_key_is_served_the_session_protocol_as_remote_device() {
    let rig = rig(Desk::closed());
    let phone = Identity::generate();
    rig.peers.0.lock().unwrap().insert(phone.public(), Scope::RemoteDevice);
    let (plain, _serving) = phone_link(&rig, &phone).await;
    let link = link::connect_as_a_device(plain, session::speaks(), Scope::RemoteDevice)
        .await
        .unwrap();
    let (client, _views, _events) = Client::new(link);
    assert_eq!(client.list().await.unwrap(), vec![]);
    assert!(matches!(rig.heard.lock().unwrap().first(), Some(Heard::Linked { scope: Scope::RemoteDevice, .. })));
}

#[tokio::test]
async fn an_unpinned_key_outside_a_window_is_dropped_before_a_frame() {
    let rig = rig(Desk::closed());
    let phone = Identity::generate();
    let (mut plain, serving) = phone_link(&rig, &phone).await;
    // Nothing of the session protocol is sent: the first read is the end.
    let mut byte = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(5), plain.read(&mut byte)).await.unwrap();
    assert!(matches!(read, Ok(0) | Err(_)), "{read:?}");
    let served = serving.await.unwrap();
    assert!(served.is_err(), "{served:?}");
    assert!(matches!(rig.heard.lock().unwrap().first(), Some(Heard::Refused { key }) if *key == phone.public()));
}

#[tokio::test]
async fn inside_a_window_one_right_proof_then_the_windows_pair_pins_and_answers_paired() {
    let code = Code::mint();
    let desk = Desk::with_window(code.clone(), 300);
    let rig = rig(desk.clone());
    let phone = Identity::generate();
    let (mut plain, serving) = phone_link(&rig, &phone).await;
    let proof = Proof {
        device: PHONE.into(),
        name: "Noro's iPhone".into(),
        proof: pairing::prove(&code, &phone.public(), &rig.host.public(), HOST),
    };
    pairing::send_proof(&mut plain, &proof).await.unwrap();
    // The window is asked, with the fingerprint it shows.
    let (offered, answer) = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(pending) = desk.pending() {
                break pending;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(offered.name, "Noro's iPhone");
    assert_eq!(offered.fingerprint, noise::fingerprint(&phone.public()));
    // The person presses Pair: the desk pins (the test does, as the desk would) and answers.
    rig.peers.0.lock().unwrap().insert(phone.public(), Scope::RemoteDevice);
    answer.send(Verdict::Paired { device: HOST.into() }).unwrap();
    assert_eq!(pairing::read_verdict(&mut plain).await.unwrap(), Verdict::Paired { device: HOST.into() });
    assert_eq!(desk.spent(), 1, "the code is spent");
    assert!(serving.await.unwrap().is_ok());
    let heard = rig.heard.lock().unwrap();
    assert!(heard.iter().any(|h| matches!(h, Heard::PairOffered { .. })));
    assert!(heard.iter().any(|h| matches!(h, Heard::Paired { device, .. } if device == PHONE)));
}

#[tokio::test]
async fn a_wrong_proof_is_refused_and_spends_the_code() {
    let desk = Desk::with_window(Code::mint(), 300);
    let rig = rig(desk.clone());
    let phone = Identity::generate();
    let (mut plain, serving) = phone_link(&rig, &phone).await;
    let proof = Proof {
        device: PHONE.into(),
        name: "Noro's iPhone".into(),
        proof: pairing::prove(&Code::mint(), &phone.public(), &rig.host.public(), HOST),
    };
    pairing::send_proof(&mut plain, &proof).await.unwrap();
    assert!(matches!(pairing::read_verdict(&mut plain).await.unwrap(), Verdict::Refused { .. }));
    assert_eq!(desk.spent(), 1);
    assert!(desk.pending().is_none(), "the window was never asked");
    assert!(serving.await.unwrap().is_err());
}

#[tokio::test]
async fn anything_but_one_proof_frame_closes_the_link_and_spends_the_code() {
    use tokio::io::AsyncWriteExt;
    let desk = Desk::with_window(Code::mint(), 300);
    let rig = rig(desk.clone());
    let phone = Identity::generate();
    let (mut plain, serving) = phone_link(&rig, &phone).await;
    plain.write_all(b"\x89CSP\r\n\x1a\n\x00\x10{\"call\":{\"id\":1}}").await.unwrap();
    plain.flush().await.unwrap();
    assert!(serving.await.unwrap().is_err());
    assert_eq!(desk.spent(), 1);
}

#[tokio::test]
async fn a_verdict_that_never_comes_closes_the_link_when_the_window_ends() {
    let code = Code::mint();
    let desk = Desk::with_window(code.clone(), 1);
    let rig = rig(desk.clone());
    let phone = Identity::generate();
    let (mut plain, serving) = phone_link(&rig, &phone).await;
    let proof = Proof {
        device: PHONE.into(),
        name: "Noro's iPhone".into(),
        proof: pairing::prove(&code, &phone.public(), &rig.host.public(), HOST),
    };
    pairing::send_proof(&mut plain, &proof).await.unwrap();
    let _pending = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(p) = desk.pending() {
                break p;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    // Nobody presses anything; the window's second passes.
    let verdict = tokio::time::timeout(Duration::from_secs(5), pairing::read_verdict(&mut plain)).await.unwrap();
    assert!(matches!(verdict, Ok(Verdict::Refused { .. }) | Err(_)), "{verdict:?}");
    assert!(serving.await.unwrap().is_err());
    assert!(rig.peers.0.lock().unwrap().is_empty(), "nothing was pinned");
}

#[tokio::test]
async fn a_key_revoked_mid_link_closes_the_link_at_once() {
    let rig = rig(Desk::closed());
    let phone = Identity::generate();
    rig.peers.0.lock().unwrap().insert(phone.public(), Scope::RemoteDevice);
    let (plain, serving) = phone_link(&rig, &phone).await;
    let link = link::connect_as_a_device(plain, session::speaks(), Scope::RemoteDevice).await.unwrap();
    let (client, _views, _events) = Client::new(link);
    client.list().await.unwrap();
    // Unpair: the record goes, the generation bumps.
    rig.peers.0.lock().unwrap().remove(&phone.public());
    rig.revoke.send(1).unwrap();
    let ended = tokio::time::timeout(Duration::from_secs(5), serving).await.unwrap().unwrap();
    assert!(ended.is_err(), "{ended:?}");
    assert!(client.list().await.is_err(), "the link is closed");
    assert!(rig.heard.lock().unwrap().iter().any(|h| matches!(h, Heard::Unlinked { .. })));
}

#[tokio::test]
async fn a_revocation_of_another_key_leaves_the_link_alone() {
    let rig = rig(Desk::closed());
    let phone = Identity::generate();
    rig.peers.0.lock().unwrap().insert(phone.public(), Scope::RemoteDevice);
    let (plain, _serving) = phone_link(&rig, &phone).await;
    let link = link::connect_as_a_device(plain, session::speaks(), Scope::RemoteDevice).await.unwrap();
    let (client, _views, _events) = Client::new(link);
    rig.revoke.send(1).unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    client.list().await.unwrap();
}

#[tokio::test]
async fn the_handshake_itself_has_a_deadline() {
    let rig = rig(Desk::closed());
    let (_a, b) = duplex(1024);
    // Nobody speaks on `_a`.
    let served = tokio::time::timeout(Duration::from_secs(15), rig.gateway.serve_one(b)).await;
    assert!(matches!(served, Ok(Err(_))), "{served:?}");
}
```

The last test takes the crate's 10 s `HANDSHAKE_TIMEOUT`; keep it (it is one test), or use
`tokio::time::pause()` with `#[tokio::test(start_paused = true)]` and `advance`. Prefer the
paused clock: `#[tokio::test(start_paused = true)]`, then `tokio::time::advance(Duration::from_secs(11)).await` after spawning the serve.

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p purlis-session-protocol --features noise --test a_gateway_admits_a_pinned_key_pairs_inside_a_window_and_drops_the_rest 2>&1 | grep -E "^error" | head -2`
Expected: FAIL to compile: `could not find gateway`.

- [ ] **Step 3: Write `gateway.rs`**

```rust
//! The gateway: one accepted stream, served by what its key is (iOS design §4.2, §4.3).
//!
//! After the Noise handshake ([`crate::noise::accept`]) the device's static key is known and
//! nothing else has crossed. Then:
//!
//! - **pinned** ([`Pins::scope_of`] says a scope): the link is served the session protocol as
//!   that scope ([`crate::link::serve_as_a_device`], [`crate::session::serve`]), and closed at
//!   once when a revocation bumps the generation and the key is no longer pinned;
//! - **unpinned, inside a pairing window** ([`PairingDesk::window`]): the link may carry one
//!   frame, the proof ([`crate::pairing`]); the code is spent whatever comes; a right proof is
//!   offered to the window ([`PairingDesk::offered`]) and its verdict is the link's one answer;
//!   a wrong proof, anything but a proof, or a window that ends first, closes the link;
//! - **unpinned otherwise**: dropped before a frame is read, and the refusal is reported with
//!   the key's fingerprint.
//!
//! Everything the host should write down reaches it as a [`Heard`], which holds keys and ids
//! and never any text of the protocol.

use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::watch;

use crate::auth::Scope;
use crate::link;
use crate::noise::{self, Identity, fingerprint};
use crate::pairing::{self, Code, Proof, Verdict};
use crate::session::{self, Host};
use crate::version::HANDSHAKE_TIMEOUT;

/// What the host has pinned: which scope a key is admitted as, if any.
pub trait Pins: Send + Sync {
    fn scope_of(&self, key: &[u8; 32]) -> Option<Scope>;
}

/// A device offering to pair: what the window shows ("Pair 'Noro's iPhone'? Key fingerprint …").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub device: String,
    pub name: String,
    pub key: [u8; 32],
    pub fingerprint: String,
}

/// A live pairing code and when it ends.
#[derive(Debug, Clone)]
pub struct Window {
    pub code: Code,
    pub ends: Instant,
}

/// The window's side of a pairing: the live code, that it was spent, and the person's answer
/// to an offer. The desk pins the key itself when it answers `Paired`.
pub trait PairingDesk: Send + Sync {
    fn window(&self) -> Option<Window>;
    fn spend(&self);
    fn offered(&self, offer: Offered) -> Pin<Box<dyn std::future::Future<Output = Verdict> + Send>>;
}

/// A generation the host bumps at every unpair: a served link re-checks its key then.
pub type Revoked = watch::Receiver<u64>;

/// What happened on a link, for the audit. Keys and ids only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    Linked { key: [u8; 32], scope: Scope },
    Refused { key: [u8; 32] },
    PairOffered { key: [u8; 32] },
    Paired { key: [u8; 32], device: String },
    PairRefused { key: [u8; 32], why: String },
    Unlinked { key: [u8; 32] },
}

pub struct Gateway<H: Host + Clone> {
    pub identity: Identity,
    /// This host's device id: the prologue, and the `paired` verdict's answer.
    pub host_device: String,
    pub pins: Arc<dyn Pins>,
    pub desk: Arc<dyn PairingDesk>,
    pub revoked: Revoked,
    pub host: H,
    pub on_event: Arc<dyn Fn(Heard) + Send + Sync>,
}

impl<H: Host + Clone> Gateway<H> {
    /// Serve one accepted stream to its end. `Err` says why it ended early, in words that hold
    /// no text of the protocol.
    pub async fn serve_one<S>(&self, io: S) -> Result<(), String>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let secured = tokio::time::timeout(
            HANDSHAKE_TIMEOUT,
            noise::accept(io, &self.identity, &self.host_device),
        )
        .await
        .map_err(|_| "the handshake did not finish in time".to_owned())?
        .map_err(|e| format!("the handshake failed: {e}"))?;
        let key = secured.remote_public();
        match self.pins.scope_of(&key) {
            Some(scope) => self.serve_pinned(secured, key, scope).await,
            None => match self.desk.window() {
                Some(window) if window.ends > Instant::now() => {
                    self.pair(secured, key, window).await
                }
                _ => {
                    (self.on_event)(Heard::Refused { key });
                    drop(secured);
                    Err(format!(
                        "a key this host has not pinned ({}) outside a pairing window",
                        fingerprint(&key)
                    ))
                }
            },
        }
    }

    async fn serve_pinned<S>(&self, secured: noise::Secured<S>, key: [u8; 32], scope: Scope) -> Result<(), String>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (device, stream) = secured.proven_as(scope);
        let link = link::serve_as_a_device(stream, session::speaks(), device)
            .await
            .map_err(|e| format!("the link did not open: {e}"))?;
        (self.on_event)(Heard::Linked { key, scope });
        let mut revoked = self.revoked.clone();
        let pins = Arc::clone(&self.pins);
        let served = session::serve(link, self.host.clone(), None);
        tokio::pin!(served);
        let outcome = loop {
            tokio::select! {
                done = &mut served => break done.map_err(|e| e.to_string()),
                changed = revoked.changed() => {
                    if changed.is_err() || pins.scope_of(&key) != Some(scope) {
                        break Err(format!("the key {} was unpaired", fingerprint(&key)));
                    }
                }
            }
        };
        (self.on_event)(Heard::Unlinked { key });
        outcome
    }

    async fn pair<S>(&self, secured: noise::Secured<S>, key: [u8; 32], window: Window) -> Result<(), String>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let host_public = self.identity.public();
        let mut plain = noise::pump(secured);
        // Whatever comes, the code is spent: one try per code.
        self.desk.spend();
        let left = window.ends.saturating_duration_since(Instant::now());
        let proof = match tokio::time::timeout(left, pairing::read_proof(&mut plain)).await {
            Ok(Ok(proof)) => proof,
            Ok(Err(e)) => {
                (self.on_event)(Heard::PairRefused { key, why: "not a proof".into() });
                return Err(format!("the first frame was not a pairing proof: {e}"));
            }
            Err(_) => {
                (self.on_event)(Heard::PairRefused { key, why: "no proof in time".into() });
                return Err("no pairing proof before the window ended".into());
            }
        };
        if let Err(why) = proof.check() {
            let _ = pairing::send_verdict(&mut plain, &Verdict::Refused { why: why.clone() }).await;
            (self.on_event)(Heard::PairRefused { key, why: "malformed".into() });
            return Err(format!("a malformed pairing proof: {why}"));
        }
        if !pairing::verify(&window.code, &proof, &key, &host_public, &self.host_device) {
            let why = "the pairing code did not match".to_owned();
            let _ = pairing::send_verdict(&mut plain, &Verdict::Refused { why: why.clone() }).await;
            (self.on_event)(Heard::PairRefused { key, why: "wrong code".into() });
            return Err(why);
        }
        (self.on_event)(Heard::PairOffered { key });
        let offer = Offered {
            device: proof.device.clone(),
            name: proof.name.clone(),
            key,
            fingerprint: fingerprint(&key),
        };
        let left = window.ends.saturating_duration_since(Instant::now());
        // The person answers in the window, or the window ends first. Meanwhile the phone may
        // send nothing: a second frame closes the link.
        let verdict = {
            let answer = self.desk.offered(offer);
            let mut nothing_more = [0u8; 1];
            tokio::select! {
                verdict = tokio::time::timeout(left, answer) => match verdict {
                    Ok(verdict) => verdict,
                    Err(_) => Verdict::Refused { why: "the pairing window ended".into() },
                },
                _ = tokio::io::AsyncReadExt::read(&mut plain, &mut nothing_more) => {
                    (self.on_event)(Heard::PairRefused { key, why: "a second frame".into() });
                    return Err("a second frame on a pairing link".into());
                }
            }
        };
        let _ = pairing::send_verdict(&mut plain, &verdict).await;
        match verdict {
            Verdict::Paired { .. } => {
                (self.on_event)(Heard::Paired { key, device: proof.device });
                Ok(())
            }
            Verdict::Refused { why } => {
                (self.on_event)(Heard::PairRefused { key, why: "not confirmed".into() });
                Err(format!("pairing refused: {why}"))
            }
        }
    }

    /// Accept links on `listener` until the task is aborted: one task per link, each its own
    /// handshake deadline, `TCP_NODELAY` so a keystroke is not held for a full segment.
    pub fn listen(self: Arc<Self>, listener: tokio::net::TcpListener) -> tokio::task::JoinHandle<()>
    where
        H: 'static,
    {
        tokio::spawn(async move {
            loop {
                let Ok((tcp, _peer)) = listener.accept().await else {
                    // The listener is gone: the gateway was turned off.
                    return;
                };
                let _ = tcp.set_nodelay(true);
                let gateway = Arc::clone(&self);
                tokio::spawn(async move {
                    let _ = gateway.serve_one(tcp).await;
                });
            }
        })
    }
}

/// How long a pairing window lasts (iOS design §4.2).
pub const WINDOW: Duration = Duration::from_secs(5 * 60);
```

Notes: the second-frame check reads from `plain` while `answer` is pending; `read` of a
duplex stream at EOF returns `Ok(0)`, which also ends the pairing (the phone went away): treat
`Ok(0)` the same as a frame (the link is over either way). `H: Host + Clone` because
`session::serve` takes the host by value per link; the app's adapter is an `Arc` wrapper.
`Err(e)` from `session::serve` when the client closes normally: map a plain close to `Ok(())`
if `LinkError::Closed`; the test `inside_a_window…` expects `Ok` for a pairing and the pinned
test does not assert the serve's result.

Add `#[cfg(feature = "noise")] pub mod gateway;` to `lib.rs` and a layer line "2c. [`gateway`]
(feature `noise`): one accepted stream served by what its key is".

- [ ] **Step 4: Run, fix, clippy, commit**

Run: `cargo test -p purlis-session-protocol --features noise --test a_gateway_admits_a_pinned_key_pairs_inside_a_window_and_drops_the_rest 2>&1 | grep -E "^test |test result|panicked" -A2 | head -40`
Expected: 9 PASS. The one most likely to need care is `a_key_revoked_mid_link_closes_the_link_at_once`: `session::serve` must be dropped for the link to close (the `select!` does that when the loop breaks), and the client's next `list` then fails. If the client's `list` still succeeds, the dropped `Link` has not closed the yamux connection: drop the `Link`'s driver explicitly (`drop(served)` happens at scope end; make sure nothing else holds the stream).

```bash
cargo clippy -p purlis-session-protocol --all-targets --features noise -- -D warnings && cargo fmt -p purlis-session-protocol
git add crates/session-protocol && git commit -q -m "Session protocol: a gateway that serves a pinned key, pairs inside a window and drops the rest before a frame"
```

---

### Task 4: `purlis-core::remote::linkkey`: the desktop's link key in the Keyring

**Files:**
- Create: `crates/purlis-core/src/remote/mod.rs`, `crates/purlis-core/src/remote/linkkey.rs`
- Modify: `crates/purlis-core/src/lib.rs` (`pub mod remote;`), `crates/purlis-core/Cargo.toml` (no new crate: `sha2`, `getrandom`, `subtle` are there; X25519 needs one: add `x25519-dalek`)
- Test: `crates/purlis-core/src/remote/linkkey.rs` unit tests (fenced stub store), `crates/purlis-core/tests/the_link_key_is_read_back_from_the_keyring_and_never_logged.rs`

**Interfaces:**
- Consumes: `secrets::keyring::{store, Store, SERVICE_PREFIX}`, `secrets::Ctx`, `machine::device_id`.
- Produces:
  - `remote::linkkey::SERVICE: &str = "purlis/link-key"` (built from `SERVICE_PREFIX`);
  - `remote::linkkey::LinkKey { private: [u8; 32], public: [u8; 32] }` with `Debug` printing the fingerprint only, `fn public_hex(&self) -> String`, `fn fingerprint(&self) -> String`;
  - `remote::linkkey::load_or_mint(ctx: &Ctx, device: &str) -> Result<LinkKey, String>` (reads `SERVICE`/`device`; mints and stores when absent; refuses an item that is not 64 hex digits);
  - `remote::linkkey::forget(ctx, device) -> Result<bool, String>` (rotation: deletes; a new pairing follows);
  - `remote::fingerprint(public: &[u8; 32]) -> String` (the same first-16-hex-of-SHA-256 as the protocol crate's; one definition in core, since core does not depend on the crate).

Why `x25519-dalek`: core must derive the public key from a private one it reads back, and it
does not depend on `snow`. `x25519-dalek` 2 is what `snow`'s default resolver uses for
`25519`, so one X25519 is in the tree either way. Add to `crates/purlis-core/Cargo.toml`
`[dependencies]`:

```toml
# A device's link key (ADR 0078 §3, iOS design §4.1): the public half from the private one read
# back from the Keyring. The same X25519 `snow` uses for the handshake in session-protocol, so
# one curve implementation is in the tree.
x25519-dalek = { version = "2", features = ["static_secrets"] }
```

- [ ] **Step 1: Write the failing tests**

`crates/purlis-core/tests/the_link_key_is_read_back_from_the_keyring_and_never_logged.rs`:

```rust
//! The desktop's link key (ADR 0078 §3, iOS design §4.1) lives in the Keyring: minted once,
//! read back the same, rotated only on purpose, and never printed in the clear.

use purlis_core::remote::linkkey::{self, LinkKey};
use purlis_core::secrets::{Ctx, Env};

const DEVICE: &str = "01J9DEVCE00000000000000000";

fn ctx(dir: &tempfile::TempDir) -> Ctx {
    Ctx::new(dir.path(), Env::of(&[]))
}

#[test]
fn the_key_is_minted_once_and_read_back_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let first = linkkey::load_or_mint(&ctx(&dir), DEVICE).unwrap();
    let again = linkkey::load_or_mint(&ctx(&dir), DEVICE).unwrap();
    assert_eq!(first.public, again.public);
    assert_eq!(first.private, again.private);
}

#[test]
fn two_devices_have_two_keys() {
    let dir = tempfile::tempdir().unwrap();
    let a = linkkey::load_or_mint(&ctx(&dir), DEVICE).unwrap();
    let b = linkkey::load_or_mint(&ctx(&dir), "01J9OTHER0000000000000000").unwrap();
    assert_ne!(a.public, b.public);
}

#[test]
fn forgetting_the_key_makes_the_next_load_a_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let first = linkkey::load_or_mint(&ctx(&dir), DEVICE).unwrap();
    assert!(linkkey::forget(&ctx(&dir), DEVICE).unwrap());
    assert!(!linkkey::forget(&ctx(&dir), DEVICE).unwrap());
    let second = linkkey::load_or_mint(&ctx(&dir), DEVICE).unwrap();
    assert_ne!(first.public, second.public);
}

#[test]
fn an_item_that_is_not_a_key_is_refused_not_used() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx(&dir);
    purlis_core::secrets::keyring::store(&ctx)
        .set(linkkey::SERVICE, DEVICE, "not a key")
        .unwrap();
    let refused = linkkey::load_or_mint(&ctx, DEVICE).unwrap_err();
    assert!(refused.contains("64 hex digits"), "{refused}");
}

#[test]
fn the_key_never_prints_its_private_half() {
    let dir = tempfile::tempdir().unwrap();
    let key = linkkey::load_or_mint(&ctx(&dir), DEVICE).unwrap();
    let shown = format!("{key:?}");
    let private_hex: String = key.private.iter().map(|b| format!("{b:02x}")).collect();
    assert!(!shown.contains(&private_hex));
    assert!(shown.contains(&key.fingerprint()));
}

#[test]
fn the_public_half_is_x25519_of_the_private_half() {
    let key = LinkKey::from_private([7u8; 32]);
    let expected = x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from([7u8; 32]));
    assert_eq!(key.public, *expected.as_bytes());
}
```

(`x25519-dalek` goes in `[dev-dependencies]` too for the last test, or the test derives the
expectation through `LinkKey` only; the plan keeps the independent check.) Check `Ctx::new`'s
signature and `Env::of`'s path in `crates/purlis-core/src/secrets/mod.rs:171,218` and
`FileStore::at(ctx.state.join(STUB_FILE))`: with `Ctx::new(root, env)`, `state` is derived
from `root`; a temp dir works.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p purlis-core --features fenced --test the_link_key_is_read_back_from_the_keyring_and_never_logged 2>&1 | grep -E "^error" | head -2`
Expected: FAIL to compile: `could not find remote in purlis_core`.

- [ ] **Step 3: Write the module**

`crates/purlis-core/src/remote/mod.rs`:

```rust
//! A paired device's side of this desktop (iOS design §3, §4): what the desktop keeps so a
//! phone can link to it. The link itself, the handshake and the gateway are
//! `purlis-session-protocol`'s (`noise`, `pairing`, `gateway`, feature `noise`); the app wires
//! the two together. Everything here is plain, synchronous state, and moves to `purlisd` with
//! the host.
//!
//! - [`linkkey`]: this device's static X25519 key, in the Keyring.
//! - [`peers`]: `<config>/peers.json`, the devices paired with this one.
//! - [`pairing`]: one-time codes, the QR payload, the pending confirmation the window answers,
//!   and unpairing.
//! - [`audit`]: the `remote.*` events, metadata only.

pub mod audit;
pub mod linkkey;
pub mod pairing;
pub mod peers;

/// A key's fingerprint, as the window and the audit show it: the first 16 hex digits of its
/// SHA-256. The same as `purlis_session_protocol::noise::fingerprint`; written out here because
/// this crate does not depend on that one, and a test in the app holds the two equal.
pub fn fingerprint(public: &[u8; 32]) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(public))[..16].to_owned()
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn unhex32(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("64 hex digits".into());
    }
    let mut out = [0u8; 32];
    for (i, pair) in text.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(pair).unwrap_or("00"), 16)
            .map_err(|e| e.to_string())?;
    }
    Ok(out)
}
```

For this task create `audit.rs`, `pairing.rs` and `peers.rs` as empty files with a one-line
`//!` doc each (Tasks 5–7 fill them), so the module compiles.

`crates/purlis-core/src/remote/linkkey.rs`:

```rust
//! This device's link key (ADR 0078 §3, iOS design §4.1): a static X25519 key pair whose
//! private half lives in the Keyring (`docs/plane-format.md`, "keyring item for the link key"),
//! minted the first time the gateway or a pairing asks, read back the same every time, and
//! rotated only on purpose ([`forget`]), after which a new pairing is needed.

use crate::secrets::keyring::{self, SERVICE_PREFIX};
use crate::secrets::Ctx;

/// The Keyring service the link key is under; the account is this device's id.
pub const SERVICE: &str = concat!("purlis/", "link-key");

const _: () = assert!(SERVICE_PREFIX.len() > 0);

/// The key pair. The private half is here so the handshake can use it; it never leaves through
/// `Debug`.
#[derive(Clone)]
pub struct LinkKey {
    pub private: [u8; 32],
    pub public: [u8; 32],
}

impl LinkKey {
    /// The pair whose private half is `private`.
    pub fn from_private(private: [u8; 32]) -> LinkKey {
        let secret = x25519_dalek::StaticSecret::from(private);
        let public = x25519_dalek::PublicKey::from(&secret);
        LinkKey {
            private,
            public: *public.as_bytes(),
        }
    }

    fn mint() -> LinkKey {
        let mut private = [0u8; 32];
        getrandom::fill(&mut private).expect("the OS generator works");
        LinkKey::from_private(private)
    }

    pub fn public_hex(&self) -> String {
        super::hex(&self.public)
    }

    pub fn fingerprint(&self) -> String {
        super::fingerprint(&self.public)
    }
}

impl std::fmt::Debug for LinkKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LinkKey({})", self.fingerprint())
    }
}

/// The link key for `device`, minting and storing one when the Keyring has none. An item that
/// is there but does not hold a key is refused, never overwritten: somebody put it there.
pub fn load_or_mint(ctx: &Ctx, device: &str) -> Result<LinkKey, String> {
    let store = keyring::store(ctx);
    match store.get(SERVICE, device).map_err(|e| e.to_string())? {
        Some(secret) => {
            let private = super::unhex32(&secret.into_inner())
                .map_err(|_| format!("the Keyring item under '{SERVICE}' does not hold a link key (64 hex digits)"))?;
            Ok(LinkKey::from_private(private))
        }
        None => {
            let key = LinkKey::mint();
            store
                .set(SERVICE, device, &super::hex(&key.private))
                .map_err(|e| e.to_string())?;
            Ok(key)
        }
    }
}

/// Rotate: delete the key. `false` when there was none. Every pairing made with it is void
/// until the devices pair again (ADR 0078 §3).
pub fn forget(ctx: &Ctx, device: &str) -> Result<bool, String> {
    keyring::store(ctx)
        .delete(SERVICE, device)
        .map_err(|e| e.to_string())
}
```

If `SERVICE_PREFIX` is not a `const &str` usable in `concat!`, write `SERVICE` as the literal
`"purlis/link-key"` and add a unit test `the_service_starts_with_the_crates_prefix` asserting
`SERVICE.starts_with(SERVICE_PREFIX)`.

Add `pub mod remote;` to `lib.rs` in alphabetical order.

- [ ] **Step 4: Run, clippy, commit**

Run: `cargo test -p purlis-core --features fenced --test the_link_key_is_read_back_from_the_keyring_and_never_logged 2>&1 | grep -E "^test |test result|panicked" | head`
Expected: 6 PASS.

```bash
cargo clippy -p purlis-core --all-targets --features fenced -- -D warnings && cargo fmt -p purlis-core
git add crates/purlis-core Cargo.lock && git commit -q -m "Core: this device's link key, minted into the Keyring once and read back the same"
```

---

### Task 5: `purlis-core::remote::peers`: `<config>/peers.json`

**Files:**
- Modify: `crates/purlis-core/src/remote/peers.rs`, `crates/purlis-core/src/machine.rs` (`Lock::on` → `pub(crate)`; a `pub(crate) const LOCK` if it is not already; `pub(crate) fn dir` is `pub`)
- Test: `crates/purlis-core/tests/peers_json_holds_the_devices_paired_with_this_one.rs`

**Interfaces:**
- Consumes: `machine::{dir, write_beside, Lock}`, `remote::{hex, unhex32, fingerprint}`.
- Produces:
  - `remote::peers::FILE: &str = "peers.json"`, `remote::peers::VERSION: u32 = 1`;
  - `remote::peers::Peer { device: String, name: String, key: [u8; 32], paired: u64 }` with `fn fingerprint(&self) -> String`;
  - `remote::peers::read(config_root: &Path) -> Result<Vec<Peer>, String>` (no file → empty; an unreadable file → `Err`, never an empty list, so a torn file cannot look like "no peers");
  - `remote::peers::add(config_root, peer: Peer) -> Result<(), String>` (a device already there is replaced by id; a key already pinned under another device id is refused);
  - `remote::peers::remove(config_root, device: &str) -> Result<Option<Peer>, String>`;
  - `remote::peers::scope_of(config_root, key: &[u8; 32]) -> Option<&'static str>` → `Some("remote-device")` when pinned (the app's `Pins` adapter maps the word to `Scope`);
  - `remote::peers::path(config_root) -> PathBuf`.

- [ ] **Step 1: Write the failing test**

`crates/purlis-core/tests/peers_json_holds_the_devices_paired_with_this_one.rs`:

```rust
//! `<config>/peers.json` (ADR 0078 §8, iOS design §4.1): the devices paired with this one,
//! each with its pinned key, written whole and privately, read back exactly, and never taken
//! as empty when it cannot be read.

use purlis_core::remote::peers::{self, Peer};

fn peer(device: &str, key: u8) -> Peer {
    Peer {
        device: device.into(),
        name: format!("phone {key}"),
        key: [key; 32],
        paired: 1_760_000_000,
    }
}

#[test]
fn no_file_is_no_peers() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(peers::read(dir.path()).unwrap(), vec![]);
}

#[test]
fn a_peer_added_is_read_back_and_its_key_is_pinned() {
    let dir = tempfile::tempdir().unwrap();
    peers::add(dir.path(), peer("01J9PHONE0000000000000000", 1)).unwrap();
    let read = peers::read(dir.path()).unwrap();
    assert_eq!(read, vec![peer("01J9PHONE0000000000000000", 1)]);
    assert_eq!(peers::scope_of(dir.path(), &[1u8; 32]), Some("remote-device"));
    assert_eq!(peers::scope_of(dir.path(), &[2u8; 32]), None);
}

#[test]
fn the_file_is_private_and_versioned_json() {
    let dir = tempfile::tempdir().unwrap();
    peers::add(dir.path(), peer("01J9PHONE0000000000000000", 1)).unwrap();
    let path = peers::path(dir.path());
    let text = std::fs::read_to_string(&path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["version"], 1);
    assert_eq!(json["peers"][0]["key"], "01".repeat(32));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

#[test]
fn a_device_paired_again_replaces_its_record_and_a_key_under_another_device_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    peers::add(dir.path(), peer("01J9PHONE0000000000000000", 1)).unwrap();
    let mut renamed = peer("01J9PHONE0000000000000000", 3);
    renamed.name = "the same phone, new key".into();
    peers::add(dir.path(), renamed.clone()).unwrap();
    assert_eq!(peers::read(dir.path()).unwrap(), vec![renamed]);
    let refused = peers::add(dir.path(), peer("01J9OTHER0000000000000000", 3)).unwrap_err();
    assert!(refused.contains("already pinned"), "{refused}");
}

#[test]
fn removing_a_peer_unpins_its_key() {
    let dir = tempfile::tempdir().unwrap();
    peers::add(dir.path(), peer("01J9PHONE0000000000000000", 1)).unwrap();
    assert_eq!(peers::remove(dir.path(), "01J9PHONE0000000000000000").unwrap(), Some(peer("01J9PHONE0000000000000000", 1)));
    assert_eq!(peers::remove(dir.path(), "01J9PHONE0000000000000000").unwrap(), None);
    assert_eq!(peers::scope_of(dir.path(), &[1u8; 32]), None);
}

#[test]
fn an_unreadable_file_is_an_error_never_an_empty_list() {
    let dir = tempfile::tempdir().unwrap();
    peers::add(dir.path(), peer("01J9PHONE0000000000000000", 1)).unwrap();
    std::fs::write(peers::path(dir.path()), "{ torn").unwrap();
    assert!(peers::read(dir.path()).is_err());
    assert_eq!(peers::scope_of(dir.path(), &[1u8; 32]), None, "a torn file pins nothing");
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p purlis-core --features fenced --test peers_json_holds_the_devices_paired_with_this_one 2>&1 | grep -E "^error" | head -2`
Expected: FAIL to compile: no `Peer`, `read`, … in `peers`.

- [ ] **Step 3: Write `peers.rs`**

```rust
//! `<config>/peers.json`: the devices paired with this one (ADR 0078 §8, iOS design §4.1).
//! Machine, device-bound: a pinned key says which two devices trust each other, and a second
//! machine of the same person pairs on its own. Written whole by a rename, `0600`, under the
//! machine store's lock, like `machine.json` beside it. Denied to chats.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::machine;

pub const FILE: &str = "peers.json";
pub const VERSION: u32 = 1;

/// One paired device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    /// Its device id, a ULID it minted.
    pub device: String,
    /// What the person calls it.
    pub name: String,
    /// Its pinned public link key.
    pub key: [u8; 32],
    /// When it was paired, seconds since the epoch.
    pub paired: u64,
}

impl Peer {
    pub fn fingerprint(&self) -> String {
        super::fingerprint(&self.key)
    }
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    peers: Vec<PeerOnDisk>,
}

#[derive(Serialize, Deserialize)]
struct PeerOnDisk {
    device: String,
    name: String,
    key: String,
    paired: u64,
}

pub fn path(config_root: &Path) -> PathBuf {
    machine::dir(config_root).join(FILE)
}

/// Every peer. No file is no peers; a file that cannot be read or parsed is an error, never an
/// empty list, so a torn file cannot look like "nobody is paired" to a writer that would then
/// replace it.
pub fn read(config_root: &Path) -> Result<Vec<Peer>, String> {
    let text = match std::fs::read_to_string(path(config_root)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(format!("{} could not be read: {e}", FILE)),
    };
    let disk: OnDisk = serde_json::from_str(&text).map_err(|e| format!("{FILE} does not parse: {e}"))?;
    disk.peers
        .into_iter()
        .map(|p| {
            Ok(Peer {
                key: super::unhex32(&p.key).map_err(|why| format!("{FILE}: a key is not {why}"))?,
                device: p.device,
                name: p.name,
                paired: p.paired,
            })
        })
        .collect()
}

fn write(config_root: &Path, peers: &[Peer]) -> Result<(), String> {
    let disk = OnDisk {
        version: VERSION,
        peers: peers
            .iter()
            .map(|p| PeerOnDisk {
                device: p.device.clone(),
                name: p.name.clone(),
                key: super::hex(&p.key),
                paired: p.paired,
            })
            .collect(),
    };
    let text = serde_json::to_string_pretty(&disk).expect("plain data");
    machine::write_beside(config_root, &path(config_root), (text + "\n").as_bytes())
        .map_err(|e| format!("{FILE} could not be written: {e}"))
}

/// Pin `peer`. A record under the same device id is replaced (the same phone paired again,
/// perhaps with a rotated key); the same key under another device id is refused, since one key
/// is one device.
pub fn add(config_root: &Path, peer: Peer) -> Result<(), String> {
    let _held = machine::Lock::on(config_root);
    let mut peers = read(config_root)?;
    if peers.iter().any(|p| p.key == peer.key && p.device != peer.device) {
        return Err(format!(
            "the key {} is already pinned under another device",
            peer.fingerprint()
        ));
    }
    peers.retain(|p| p.device != peer.device);
    peers.push(peer);
    write(config_root, &peers)
}

/// Unpin `device`'s key. The record removed, or `None` when there was none.
pub fn remove(config_root: &Path, device: &str) -> Result<Option<Peer>, String> {
    let _held = machine::Lock::on(config_root);
    let mut peers = read(config_root)?;
    let at = peers.iter().position(|p| p.device == device);
    let removed = at.map(|i| peers.remove(i));
    if removed.is_some() {
        write(config_root, &peers)?;
    }
    Ok(removed)
}

/// The scope word a pinned key is admitted as, or `None`. Every peer of a desktop is a phone,
/// `remote-device`; a runner's peers are its desktops, `remote-link`, and live in the runner's
/// own file (RR-1). A file that cannot be read pins nothing.
pub fn scope_of(config_root: &Path, key: &[u8; 32]) -> Option<&'static str> {
    read(config_root)
        .ok()?
        .iter()
        .any(|p| &p.key == key)
        .then_some("remote-device")
}
```

In `machine.rs` make `Lock::on` `pub(crate)`. `machine::dir` is `pub`; `write_beside` is
`pub(crate)`: both reachable from `crate::remote`. The lock's row in `docs/plane-format.md`
already says it covers `peers.json` (Task 0).

- [ ] **Step 4: Run, clippy, commit**

Run: `cargo test -p purlis-core --features fenced --test peers_json_holds_the_devices_paired_with_this_one 2>&1 | grep -E "^test |test result|panicked" | head`
Expected: 6 PASS. The `0o600` assertion holds because `write_beside` writes `Mode::Private`.

```bash
cargo clippy -p purlis-core --all-targets --features fenced -- -D warnings && cargo fmt -p purlis-core
git add crates/purlis-core && git commit -q -m "Core: peers.json, the devices paired with this one, written like the machine store"
```

---

### Task 6: `purlis-core::remote::pairing`: codes, the QR payload, the window's answer, unpairing

**Files:**
- Modify: `crates/purlis-core/src/remote/pairing.rs`
- Test: `crates/purlis-core/tests/a_pairing_is_one_code_for_five_minutes_confirmed_only_in_the_window.rs`

**Interfaces:**
- Consumes: `remote::peers`, `remote::linkkey::LinkKey`, `remote::fingerprint`.
- Produces:
  - `remote::pairing::WINDOW: Duration = 5 min`;
  - `remote::pairing::Payload { host_device: String, host_key: String (hex), addresses: Vec<String>, code: String (hex), version: u32 }` with `fn to_qr_text(&self) -> String` (`purlis-pair:` + compact JSON) and `fn from_qr_text(&str) -> Result<Payload, String>`;
  - `remote::pairing::Desk` (`Send + Sync`, `Clone` over an `Arc` inside): `Desk::new(config_root: PathBuf)`;
    `fn open_window(&self, host: &LinkKey, host_device: &str, addresses: Vec<String>, now: Instant) -> Payload` (a fresh code; any earlier window is closed);
    `fn window(&self, now: Instant) -> Option<(String /*code hex*/, Instant /*ends*/)>`;
    `fn spend(&self)`; `fn close_window(&self)`;
    `fn offer(&self, offered: Offer) -> Ticket` where `Offer { device, name, key: [u8; 32], fingerprint }` and `Ticket { id: u64 }`; `fn pending(&self) -> Option<(Ticket, Offer)>` (what the window shows);
    `fn confirm(&self, ticket: Ticket, now_secs: u64) -> Result<Verdict, String>` (pins into `peers.json` and answers `Paired`), `fn cancel(&self, ticket: Ticket) -> Verdict` (`Refused`);
    `fn take_verdict(&self, ticket: Ticket) -> Option<Verdict>` and `fn wait_verdict(&self, ticket: Ticket, until: Instant) -> Verdict` (blocking on a `Condvar`; the app's async adapter calls it in `spawn_blocking`);
    `fn unpair(&self, device: &str) -> Result<Option<peers::Peer>, String>` (removes the record and bumps `generation`);
    `fn generation(&self) -> u64` and `fn on_revoke(&self, f: Box<dyn Fn(u64) + Send + Sync>)` (the app forwards bumps to the gateway's `watch`);
  - `remote::pairing::Verdict { Paired { device: String }, Refused { why: String } }` (core's own; the app maps it to the crate's).

- [ ] **Step 1: Write the failing test**

`crates/purlis-core/tests/a_pairing_is_one_code_for_five_minutes_confirmed_only_in_the_window.rs`:

```rust
//! Pairing (iOS design §4.2): one code, five minutes, single use; a proof is offered to the
//! window, which alone confirms; a confirmation pins, a cancel or a window that ends pins
//! nothing; unpairing removes the record and tells every live link to re-check its key.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::remote::linkkey::LinkKey;
use purlis_core::remote::pairing::{Desk, Offer, Payload, Verdict, WINDOW};
use purlis_core::remote::peers;

const HOST: &str = "01J9DEVCE00000000000000000";

fn host() -> LinkKey {
    LinkKey::from_private([9u8; 32])
}

fn offer(key: u8) -> Offer {
    Offer {
        device: "01J9PHONE0000000000000000".into(),
        name: "Noro's iPhone".into(),
        key: [key; 32],
        fingerprint: purlis_core::remote::fingerprint(&[key; 32]),
    }
}

#[test]
fn a_window_is_one_code_that_the_qr_text_carries_with_the_hosts_key_and_addresses() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    let now = Instant::now();
    let payload = desk.open_window(&host(), HOST, vec!["192.168.9.155:7777".into()], now);
    assert_eq!(payload.host_device, HOST);
    assert_eq!(payload.host_key, host().public_hex());
    assert_eq!(payload.addresses, vec!["192.168.9.155:7777"]);
    let text = payload.to_qr_text();
    assert!(text.starts_with("purlis-pair:"), "{text}");
    assert_eq!(Payload::from_qr_text(&text).unwrap(), payload);
    let (code, ends) = desk.window(now).unwrap();
    assert_eq!(code, payload.code);
    assert_eq!(ends, now + WINDOW);
}

#[test]
fn a_window_ends_after_five_minutes_and_a_new_one_replaces_the_old_code() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    let now = Instant::now();
    let first = desk.open_window(&host(), HOST, vec![], now);
    assert!(desk.window(now + WINDOW).is_none(), "ended");
    let second = desk.open_window(&host(), HOST, vec![], now);
    assert_ne!(first.code, second.code);
    assert_eq!(desk.window(now).unwrap().0, second.code);
}

#[test]
fn a_spent_code_is_gone_and_a_closed_window_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    let now = Instant::now();
    desk.open_window(&host(), HOST, vec![], now);
    desk.spend();
    assert!(desk.window(now).is_none());
    desk.open_window(&host(), HOST, vec![], now);
    desk.close_window();
    assert!(desk.window(now).is_none());
}

#[test]
fn a_confirmed_offer_pins_the_device_and_answers_paired() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    let ticket = desk.offer(offer(1));
    let (shown, what) = desk.pending().unwrap();
    assert_eq!(shown, ticket);
    assert_eq!(what.fingerprint, purlis_core::remote::fingerprint(&[1u8; 32]));
    assert_eq!(desk.confirm(ticket, 1_760_000_000).unwrap(), Verdict::Paired { device: HOST.into() });
    assert_eq!(desk.take_verdict(ticket), Some(Verdict::Paired { device: HOST.into() }));
    let pinned = peers::read(dir.path()).unwrap();
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned[0].name, "Noro's iPhone");
    assert_eq!(pinned[0].key, [1u8; 32]);
    assert!(desk.pending().is_none());
}

#[test]
fn a_cancelled_offer_pins_nothing_and_answers_refused() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    let ticket = desk.offer(offer(1));
    assert!(matches!(desk.cancel(ticket), Verdict::Refused { .. }));
    assert!(matches!(desk.take_verdict(ticket), Some(Verdict::Refused { .. })));
    assert_eq!(peers::read(dir.path()).unwrap(), vec![]);
}

#[test]
fn an_unconfirmed_pairing_pins_nothing_and_spends_the_code() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    let now = Instant::now();
    desk.open_window(&host(), HOST, vec![], now);
    desk.spend();
    let ticket = desk.offer(offer(1));
    // Nobody answers: waiting until the deadline is a refusal, and nothing is pinned.
    let verdict = desk.wait_verdict(ticket, Instant::now() + Duration::from_millis(50));
    assert!(matches!(verdict, Verdict::Refused { .. }), "{verdict:?}");
    assert_eq!(peers::read(dir.path()).unwrap(), vec![]);
    assert!(desk.window(now).is_none(), "the code was spent");
}

#[test]
fn waiting_for_a_verdict_wakes_when_the_window_answers() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Arc::new(Desk::new(dir.path().to_path_buf()));
    let ticket = desk.offer(offer(1));
    let waiter = {
        let desk = Arc::clone(&desk);
        std::thread::spawn(move || desk.wait_verdict(ticket, Instant::now() + Duration::from_secs(5)))
    };
    std::thread::sleep(Duration::from_millis(20));
    desk.confirm(ticket, 1_760_000_000).unwrap();
    assert_eq!(waiter.join().unwrap(), Verdict::Paired { device: HOST.into() });
}

#[test]
fn unpairing_removes_the_record_and_bumps_the_generation() {
    let dir = tempfile::tempdir().unwrap();
    let desk = Desk::new(dir.path().to_path_buf());
    let ticket = desk.offer(offer(1));
    desk.confirm(ticket, 1_760_000_000).unwrap();
    let seen = Arc::new(Mutex::new(vec![]));
    let sink = Arc::clone(&seen);
    desk.on_revoke(Box::new(move |generation| sink.lock().unwrap().push(generation)));
    let before = desk.generation();
    let removed = desk.unpair("01J9PHONE0000000000000000").unwrap().unwrap();
    assert_eq!(removed.key, [1u8; 32]);
    assert_eq!(desk.generation(), before + 1);
    assert_eq!(*seen.lock().unwrap(), vec![before + 1]);
    assert_eq!(peers::read(dir.path()).unwrap(), vec![]);
    assert!(desk.unpair("01J9PHONE0000000000000000").unwrap().is_none());
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p purlis-core --features fenced --test a_pairing_is_one_code_for_five_minutes_confirmed_only_in_the_window 2>&1 | grep -E "^error" | head -2`
Expected: FAIL to compile.

- [ ] **Step 3: Write `pairing.rs`**

```rust
//! Pairing's desk (iOS design §4.2): one code at a time, five minutes, single use, shown inside
//! a QR code with the host's public key, its device id and its addresses; a proof the gateway
//! verified is offered here and the window alone confirms it, which pins the device; a cancel,
//! or a window that ends first, pins nothing. Unpairing removes the record and bumps a
//! generation every live link re-checks its key against.
//!
//! The desk holds no text of the protocol, and the code is never printed.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::linkkey::LinkKey;
use super::peers::{self, Peer};

/// How long a pairing window lasts.
pub const WINDOW: Duration = Duration::from_secs(5 * 60);

/// What the QR code carries, as `purlis-pair:` followed by compact JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payload {
    pub version: u32,
    pub host_device: String,
    /// The host's public link key, hex.
    pub host_key: String,
    /// Where to reach the host, `host:port` each, LAN and Tailscale.
    pub addresses: Vec<String>,
    /// The one-time code, hex.
    pub code: String,
}

const QR_PREFIX: &str = "purlis-pair:";

impl Payload {
    pub fn to_qr_text(&self) -> String {
        format!("{QR_PREFIX}{}", serde_json::to_string(self).expect("plain data"))
    }

    pub fn from_qr_text(text: &str) -> Result<Payload, String> {
        let json = text
            .strip_prefix(QR_PREFIX)
            .ok_or_else(|| "not a purlis pairing code".to_owned())?;
        serde_json::from_str(json).map_err(|e| format!("not a purlis pairing code: {e}"))
    }
}

/// A device the gateway verified a proof from, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub device: String,
    pub name: String,
    pub key: [u8; 32],
    pub fingerprint: String,
}

/// One offer's handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticket(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Paired { device: String },
    Refused { why: String },
}

struct Live {
    code: String,
    ends: Instant,
}

struct Pending {
    ticket: Ticket,
    offer: Offer,
    verdict: Option<Verdict>,
}

#[derive(Default)]
struct State {
    window: Option<Live>,
    host_device: String,
    next_ticket: u64,
    pending: VecDeque<Pending>,
    generation: u64,
}

pub struct Desk {
    config_root: PathBuf,
    state: Mutex<State>,
    answered: Condvar,
    on_revoke: Mutex<Vec<Box<dyn Fn(u64) + Send + Sync>>>,
}

impl Desk {
    pub fn new(config_root: PathBuf) -> Desk {
        Desk {
            config_root,
            state: Mutex::new(State::default()),
            answered: Condvar::new(),
            on_revoke: Mutex::new(vec![]),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Open a window: a fresh code, replacing any earlier one, and the payload for the QR code.
    pub fn open_window(&self, host: &LinkKey, host_device: &str, addresses: Vec<String>, now: Instant) -> Payload {
        let mut code = [0u8; 32];
        getrandom::fill(&mut code).expect("the OS generator works");
        let code = super::hex(&code);
        let mut state = self.state();
        state.window = Some(Live {
            code: code.clone(),
            ends: now + WINDOW,
        });
        state.host_device = host_device.to_owned();
        Payload {
            version: 1,
            host_device: host_device.to_owned(),
            host_key: host.public_hex(),
            addresses,
            code,
        }
    }

    /// The live code and when it ends, if a window is open at `now`.
    pub fn window(&self, now: Instant) -> Option<(String, Instant)> {
        let state = self.state();
        state
            .window
            .as_ref()
            .filter(|live| live.ends > now)
            .map(|live| (live.code.clone(), live.ends))
    }

    /// The code is used up: one try per code, whatever the try was.
    pub fn spend(&self) {
        self.state().window = None;
    }

    pub fn close_window(&self) {
        self.state().window = None;
    }

    /// A verified proof, for the window to answer.
    pub fn offer(&self, offer: Offer) -> Ticket {
        let mut state = self.state();
        state.next_ticket += 1;
        let ticket = Ticket(state.next_ticket);
        state.pending.push_back(Pending {
            ticket,
            offer,
            verdict: None,
        });
        ticket
    }

    /// The offer the window should show, oldest first.
    pub fn pending(&self) -> Option<(Ticket, Offer)> {
        self.state()
            .pending
            .iter()
            .find(|p| p.verdict.is_none())
            .map(|p| (p.ticket, p.offer.clone()))
    }

    /// The person pressed Pair: pin the device and answer.
    pub fn confirm(&self, ticket: Ticket, now_secs: u64) -> Result<Verdict, String> {
        let (offer, host_device) = {
            let state = self.state();
            let pending = state
                .pending
                .iter()
                .find(|p| p.ticket == ticket && p.verdict.is_none())
                .ok_or_else(|| "no such pairing is pending".to_owned())?;
            (pending.offer.clone(), state.host_device.clone())
        };
        peers::add(
            &self.config_root,
            Peer {
                device: offer.device,
                name: offer.name,
                key: offer.key,
                paired: now_secs,
            },
        )?;
        let verdict = Verdict::Paired { device: host_device };
        self.answer(ticket, verdict.clone());
        Ok(verdict)
    }

    /// The person pressed Cancel.
    pub fn cancel(&self, ticket: Ticket) -> Verdict {
        let verdict = Verdict::Refused {
            why: "not confirmed on the desktop".into(),
        };
        self.answer(ticket, verdict.clone());
        verdict
    }

    fn answer(&self, ticket: Ticket, verdict: Verdict) {
        let mut state = self.state();
        if let Some(p) = state.pending.iter_mut().find(|p| p.ticket == ticket) {
            p.verdict = Some(verdict);
        }
        drop(state);
        self.answered.notify_all();
    }

    /// The verdict, once given, taking the offer off the desk.
    pub fn take_verdict(&self, ticket: Ticket) -> Option<Verdict> {
        let mut state = self.state();
        let at = state
            .pending
            .iter()
            .position(|p| p.ticket == ticket && p.verdict.is_some())?;
        state.pending.remove(at).and_then(|p| p.verdict)
    }

    /// Wait for the window's answer until `until`; past it, the offer is refused and taken off
    /// the desk. Blocking: the app calls it off its runtime.
    pub fn wait_verdict(&self, ticket: Ticket, until: Instant) -> Verdict {
        let mut state = self.state();
        loop {
            if let Some(at) = state.pending.iter().position(|p| p.ticket == ticket && p.verdict.is_some()) {
                return state.pending.remove(at).and_then(|p| p.verdict).unwrap_or(Verdict::Refused { why: "gone".into() });
            }
            let now = Instant::now();
            if now >= until {
                state.pending.retain(|p| p.ticket != ticket);
                return Verdict::Refused {
                    why: "the pairing window ended before the desktop answered".into(),
                };
            }
            let (next, _) = self
                .answered
                .wait_timeout(state, until - now)
                .unwrap_or_else(|p| p.into_inner());
            state = next;
        }
    }

    /// Remove `device`'s record and tell every live link to re-check its key.
    pub fn unpair(&self, device: &str) -> Result<Option<Peer>, String> {
        let removed = peers::remove(&self.config_root, device)?;
        if removed.is_some() {
            let generation = {
                let mut state = self.state();
                state.generation += 1;
                state.generation
            };
            for f in self.on_revoke.lock().unwrap_or_else(|p| p.into_inner()).iter() {
                f(generation);
            }
        }
        Ok(removed)
    }

    pub fn generation(&self) -> u64 {
        self.state().generation
    }

    pub fn on_revoke(&self, f: Box<dyn Fn(u64) + Send + Sync>) {
        self.on_revoke.lock().unwrap_or_else(|p| p.into_inner()).push(f);
    }
}
```

- [ ] **Step 4: Run, clippy, commit**

Run: `cargo test -p purlis-core --features fenced --test a_pairing_is_one_code_for_five_minutes_confirmed_only_in_the_window 2>&1 | grep -E "^test |test result|panicked" | head -12`
Expected: 8 PASS.

```bash
cargo clippy -p purlis-core --all-targets --features fenced -- -D warnings && cargo fmt -p purlis-core
git add crates/purlis-core && git commit -q -m "Core: the pairing desk, one code for five minutes confirmed only in the window, and unpairing"
```

---

### Task 7: `purlis-core::remote::audit`: the `remote.*` events, metadata only

**Files:**
- Modify: `crates/purlis-core/src/remote/audit.rs`, `crates/purlis-core/src/eventlog.rs` (`Recorder::remote`)
- Test: `crates/purlis-core/tests/a_devices_acts_are_recorded_as_metadata_and_never_as_text.rs`

**Interfaces:**
- Consumes: `eventlog::{Recorder, Event}`, `remote::fingerprint`.
- Produces:
  - `eventlog::Recorder::remote(&mut self, kind: &str, chat: Option<&str>, body: serde_json::Value) -> io::Result<Event>` (`kind` must start with `remote.`, asserted);
  - `remote::audit::Act` enum: `Paired { device, key }`, `Unpaired { device, key }`, `Linked { device: Option<String>, key }`, `Refused { key }`, `Answered { device, key, chat, ask, choice }`, `Prompted { device, key, chat, bytes: usize }`;
  - `remote::audit::record(recorder: &mut Recorder, act: Act) -> io::Result<Event>` (writes `remote.paired`, …, each with `device`, `fingerprint`, `scope: "remote-device"` and the act's ids);
  - `remote::audit::KINDS: &[&str]`.

- [ ] **Step 1: Write the failing test**

`crates/purlis-core/tests/a_devices_acts_are_recorded_as_metadata_and_never_as_text.rs`:

```rust
//! What a paired device does reaches the event log as metadata (iOS design §4.4): which device,
//! which chat, which ask and choice ids, how many bytes a prompt was. Never the text (D9).

use purlis_core::eventlog::{self, Recorder};
use purlis_core::remote::audit::{self, Act};

const DEVICE: &str = "01J9PHONE0000000000000000";
const CHAT: &str = "01J9ZQ3V7K8M2N4P6R8T0V2X4Z";

fn recorder(data: &std::path::Path, config: &std::path::Path) -> Recorder {
    Recorder::open_in(config, data).unwrap()
}

#[test]
fn each_act_is_its_kind_with_the_devices_id_and_fingerprint() {
    let data = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let mut rec = recorder(data.path(), config.path());
    let key = [5u8; 32];
    let fp = purlis_core::remote::fingerprint(&key);
    let paired = audit::record(&mut rec, Act::Paired { device: DEVICE.into(), key }).unwrap();
    assert_eq!(paired.kind, "remote.paired");
    assert_eq!(paired.body["device"], DEVICE);
    assert_eq!(paired.body["fingerprint"], fp);
    assert_eq!(paired.body["scope"], "remote-device");
    assert_eq!(paired.chat, None);
    let answered = audit::record(&mut rec, Act::Answered { device: DEVICE.into(), key, chat: CHAT.into(), ask: "ask-1".into(), choice: "allow".into() }).unwrap();
    assert_eq!(answered.kind, "remote.answered");
    assert_eq!(answered.chat.as_deref(), Some(CHAT));
    assert_eq!(answered.body["ask"], "ask-1");
    assert_eq!(answered.body["choice"], "allow");
    let prompted = audit::record(&mut rec, Act::Prompted { device: DEVICE.into(), key, chat: CHAT.into(), bytes: 27 }).unwrap();
    assert_eq!(prompted.kind, "remote.prompted");
    assert_eq!(prompted.body["bytes"], 27);
    let refused = audit::record(&mut rec, Act::Refused { key }).unwrap();
    assert_eq!(refused.kind, "remote.refused");
    assert_eq!(refused.body["fingerprint"], fp);
    assert!(refused.body.get("device").is_none() || refused.body["device"].is_null());
}

#[test]
fn no_act_can_carry_text_and_the_log_holds_none() {
    // The enum has no field for a prompt's, message's or answer's text: `Prompted` takes a
    // length. This test pins what reaches the disk.
    let data = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let mut rec = recorder(data.path(), config.path());
    let key = [6u8; 32];
    audit::record(&mut rec, Act::Prompted { device: DEVICE.into(), key, chat: CHAT.into(), bytes: "also add a test for expiry".len() }).unwrap();
    let written = eventlog::read(&data.path().join(eventlog::EVENTS).join(rec_device(&rec))).unwrap();
    let text = serde_json::to_string(&written.last().unwrap().body).unwrap();
    assert!(!text.contains("expiry"), "{text}");
    for kind in audit::KINDS {
        assert!(kind.starts_with("remote."), "{kind}");
    }
}

fn rec_device(rec: &Recorder) -> String {
    // The recorder's directory is `<data>/events/<device>`.
    rec.dir().file_name().unwrap().to_string_lossy().into_owned()
}
```

Check `Recorder::open_in(config, data)` mints a device id into `config`'s machine store (it
does through `machine::device_id`) and `Recorder::dir()` is the device's directory; adjust the
`read` path to `rec.dir()` directly if so (`eventlog::read(rec.dir())`).

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p purlis-core --features fenced --test a_devices_acts_are_recorded_as_metadata_and_never_as_text 2>&1 | grep -E "^error" | head -2`
Expected: FAIL to compile.

- [ ] **Step 3: Write `audit.rs` and `Recorder::remote`**

In `eventlog.rs`, in `impl Recorder` after `dir()`:

```rust
    /// An act of a paired device, or on one (iOS design §4.4): `kind` is `remote.<word>`, the
    /// body metadata only. The caller (`crate::remote::audit`) is what keeps text out of it.
    pub fn remote(
        &mut self,
        kind: &str,
        chat: Option<&str>,
        body: serde_json::Value,
    ) -> io::Result<Event> {
        assert!(kind.starts_with("remote."), "{kind} is not a remote.* kind");
        self.log.append(chat, None, None, kind, body)
    }
```

`crates/purlis-core/src/remote/audit.rs`:

```rust
//! The `remote.*` events (iOS design §4.4): what a paired device did, as metadata. Until ADR
//! 0075's audit store exists they go to the event log, and move to the audit when it does.
//! There is no field here for a prompt's, a message's or an answer's text, by construction
//! (D9).

use serde_json::json;

use crate::eventlog::{Event, Recorder};

/// Every kind this module writes; `docs/plane-format.md`'s events row names them.
pub const KINDS: &[&str] = &[
    "remote.paired",
    "remote.unpaired",
    "remote.linked",
    "remote.refused",
    "remote.answered",
    "remote.prompted",
];

/// The scope word every paired device of a desktop is admitted as.
const SCOPE: &str = "remote-device";

/// One act. Ids and keys only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Paired { device: String, key: [u8; 32] },
    Unpaired { device: String, key: [u8; 32] },
    /// A link opened; `device` is `None` until 1C's host adapter maps a key to its record.
    Linked { device: Option<String>, key: [u8; 32] },
    /// A key this host has not pinned was dropped.
    Refused { key: [u8; 32] },
    Answered { device: String, key: [u8; 32], chat: String, ask: String, choice: String },
    Prompted { device: String, key: [u8; 32], chat: String, bytes: usize },
}

pub fn record(recorder: &mut Recorder, act: Act) -> std::io::Result<Event> {
    let (kind, chat, body) = match act {
        Act::Paired { device, key } => ("remote.paired", None, json!({"device": device, "fingerprint": super::fingerprint(&key), "scope": SCOPE})),
        Act::Unpaired { device, key } => ("remote.unpaired", None, json!({"device": device, "fingerprint": super::fingerprint(&key), "scope": SCOPE})),
        Act::Linked { device, key } => ("remote.linked", None, json!({"device": device, "fingerprint": super::fingerprint(&key), "scope": SCOPE})),
        Act::Refused { key } => ("remote.refused", None, json!({"device": null, "fingerprint": super::fingerprint(&key), "scope": SCOPE})),
        Act::Answered { device, key, chat, ask, choice } => ("remote.answered", Some(chat), json!({"device": device, "fingerprint": super::fingerprint(&key), "scope": SCOPE, "ask": ask, "choice": choice})),
        Act::Prompted { device, key, chat, bytes } => ("remote.prompted", Some(chat), json!({"device": device, "fingerprint": super::fingerprint(&key), "scope": SCOPE, "bytes": bytes})),
    };
    recorder.remote(kind, chat.as_deref(), body)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_kind_is_named_once() {
        let mut kinds = super::KINDS.to_vec();
        kinds.sort_unstable();
        kinds.dedup();
        assert_eq!(kinds.len(), super::KINDS.len());
    }
}
```

- [ ] **Step 4: Run, clippy, commit**

Run: `cargo test -p purlis-core --features fenced --test a_devices_acts_are_recorded_as_metadata_and_never_as_text 2>&1 | grep -E "^test |test result|panicked" | head`
Expected: 2 PASS.

```bash
cargo clippy -p purlis-core --all-targets --features fenced -- -D warnings && cargo fmt -p purlis-core
git add crates/purlis-core && git commit -q -m "Core: a paired device's acts reach the event log as metadata, never as text"
```

---

### Task 8: The whole workspace, and the docs

**Files:**
- Modify: `docs/superpowers/specs/2026-10-11-ios-mobile-design.md` (§3's unit table: where the gateway's pieces live, as built), `~/purlis-research/README.md` §10

- [ ] **Step 1: Build and test everything the change reaches**

Run: `cargo test -p purlis-session-protocol --features noise 2>&1 | grep -E "^test result" | sort | uniq -c` and `cargo test -p purlis-core --features fenced remote 2>&1 | grep -E "test result"` and `cargo test -p purlis-core --features fenced --test every_store_the_plane_format_names_has_a_tier` and `cargo clippy --workspace --all-targets -- -D warnings` (in the sandbox: `--exclude purlis-app`).
Expected: all ok (the sandbox's 15 socket tests aside).

- [ ] **Step 2: The spec's unit table, as built**

In spec §3's table, change the `purlis-core::remote` row's "Job" to begin: "**As built (1B):** the handshake (`noise`), the proof (`pairing`) and the accept loop (`gateway`) are `session-protocol`'s behind its `noise` feature; `purlis-core::remote` holds the link key, `peers.json`, the pairing desk and the audit; the app (1C) wires them and holds the `Host` adapter. Listens when the operator turns it on, …" (keep the rest).

Add to §7's Phase 1 row after "1A, the protocol additions, is built: …": "**1B, the link, the keys and the pairing, is built:** `docs/superpowers/plans/2026-10-12-ios-phase-1b-link-and-pairing.md`."

In `~/purlis-research/README.md` §10's "Phase 1 plans" row, add 1B's state and that 1C is next (the app wiring: `Pins`/`PairingDesk` adapters, the `session::Host` adapter over `SessionHost`/`ChatBoard` with the timeline adapter and `choice_lasts`, Settings → Devices with the QR code and the Pair/Cancel sheet, the listener switch and interface choice, keep-awake, `machine.json` keys with their row first, the changelog fragment).

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/specs/2026-10-11-ios-mobile-design.md && git commit -q -m "Design: Phase 1B built; where the gateway's pieces live as built"
```

---

## Self-review (done while writing)

- **Spec coverage**: §4.1 keys and stores → Tasks 0, 4, 5 (link key in the Keyring; `peers.json` with its row first; `machine.json` keys are 1C's). §4.2 pairing → Tasks 2, 3, 6 (QR payload with key, addresses and code; one frame only; wrong proof closes and spends; window confirms; both pin: the host in Task 6, the phone in 1D from the `Paired` verdict and the QR's key). Unpair closes live links → Task 6 + Task 3 (`revoked`). §4.3 scope → 1A; the gateway admits a pinned key as `remote-device` (Task 3). §4.4 threats → unpinned dropped before a frame (Task 3), no pinned private key in a chat (the Keyring item is denied to chats by the plane-format row; the sandbox's class list is 1C's doctor row), QR single use and 5 minutes (Task 6), lost phone → unpair (Task 6). Audit → Task 7. §8 "pairing" tests: expired code (Task 6), reused code (Task 6 `spend`, Task 3 `wrong proof … spends`), unconfirmed (Tasks 3, 6), unknown key (Task 3), revoked mid-link (Task 3), unpinned outside a window dropped before a frame (Task 3), inside one only the proof (Task 3). Not here, on purpose: `prompt` routing, `choice_lasts`, timeline items from hooks, the `Host` adapter, interface enumeration, Settings → Devices, keep-awake, the `machine.json` keys and their row, the changelog fragment: all 1C.
- **Placeholders**: none; every step has its code. Two spots say "check the signature" (`Ctx::new`, `Recorder::dir`) because the executor can read them in one grep; both have the fallback written.
- **Type consistency**: `Verdict` exists twice on purpose (the crate's wire enum in Task 2, core's own in Task 6, since core does not depend on the crate); 1C maps one to the other and a test there holds them equal. `fingerprint` likewise. `Offered` (crate) and `Offer` (core) carry the same four fields. `Pins::scope_of` returns `Option<Scope>`; core's `peers::scope_of` returns the word; 1C's adapter maps by `Scope::from_word`.
- **Review Focus**: each line names its test and task above.
