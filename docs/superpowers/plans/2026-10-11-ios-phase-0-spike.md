# purlis on iPhone, Phase 0 spike: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Answer the three questions the spec's Phase 0 row asks, with evidence, so that Phase 1 can be planned on facts: (1) does `session-protocol` + `snow` + tokio build for `aarch64-apple-ios` and link to a host over Wi-Fi from a Tauri 2 iOS shell; (2) does xterm.js in WKWebView survive the spec's 2 MB and 13 MB bursts without freezing; (3) does a bracketed-paste prompt sent mid-turn queue correctly in Claude Code, Codex and opencode.

**Architecture:** Everything here is **throwaway** and lives outside the purlis clone, in `~/purlis-research/spike/`: a `noise` crate (Noise `XX` over TCP, pumped into a `tokio::io::DuplexStream` so the protocol crate sees an ordinary stream), a `host` binary on the Mac that serves the existing `purlis-session-protocol` as a device link and feeds the burst corpus into a view, and a `phone` Tauri 2 iOS app whose Rust side is the protocol client and whose webview is xterm.js. Experiment 3 needs no code of ours: tmux drives each harness in a PTY and sends exactly the bytes purlis's writer sends. Only the findings go into the clone, on the local branch `ios-mobile-design`.

**Tech Stack:** Rust 1.99.0 (the clone's pin), tokio, `purlis-session-protocol` (path dependency, `any-stream` feature), `snow` 0.10, Tauri 2.12 (`@tauri-apps/cli`, `create-tauri-app` 4.7), React + TypeScript via Vite, `@xterm/xterm` 6, tmux, Xcode 26.6 with the iOS 26.5 SDK, a real iPhone signed with a free Apple ID.

**Spec:** `docs/superpowers/specs/2026-10-11-ios-mobile-design.md`, §7 row "0. Spike". Decisions D1–D12 and §4–§6 are the design the spike serves; this plan does not build any of them.

## Global Constraints

- **Nothing is pushed, ever. No PRs. No `gh` writes.** The clone's branch `ios-mobile-design` receives only the findings note, the spec's pointer to it and README §10. The spike's own git repository is `~/purlis-research/spike/` and has no remote.
- **Commit messages are plain "what was done". No Co-Authored-By, no AI trailers, no mention of AI.** This applies in both repositories.
- **The spike never touches the clone's crates.** `purlis-session-protocol` is consumed by path as it is on `main @ 03e3b5c4`. If something must change in it, that is a finding, not an edit.
- **The toolchain is the clone's pin:** `rust-toolchain.toml` says `channel = "1.99.0"`. The spike copies that file so both build with one compiler.
- **Nothing parses harness output to decide anything** (AGENTS.md). Experiment 3's verdict is read by a person from a captured screen and written down; no script decides it.
- **Each test is trusted only once it has been seen to fail for the right reason** (AGENTS.md). Every task below runs the test red before the code.
- **This Mac's Wi-Fi address today is `192.168.20.126`** (`ipconfig getifaddr en0`). The phone and the Mac must be on the same Wi-Fi. There is no Tailscale on this machine; the spec allows either.
- **Spec budgets the spike reads against:** L3 "2 MB and 13 MB bursts: no freeze; input and other panes responsive" (desktop measured the longest frame at 42 / 52 ms). The spike records the phone's numbers beside them; it does not have to meet them.
- **If experiment 2 fails** (the webview freezes or the app is killed), the spec's rule applies: Phase 1's level-2 screen is timeline-only with "View on desktop". The findings note says which.
- **The host crate uses `ProvenDevice::stand_in()`**, which exists only with the `any-stream` feature and stands in for the Noise handshake RR-13 will build. In the spike, `snow` does the real handshake *outside* the protocol crate and the crate is told the device is proved. That is the spike's shape, not the gateway's (§3 of the spec puts the handshake inside `purlis-core::remote`).

## Review Focus

Inputs the spec implies and no spike experiment names. Each has its test added to the task that owns the code.

1. **The phone goes to the background mid-burst** (iOS suspends the app, the TCP link dies). The host must drop that view and keep serving the next connection, never panic or wedge. → Task 3, test `a_client_that_vanishes_mid_burst_does_not_stop_the_host`.
2. **A plaintext write larger than one Noise message** (65,535 bytes including the 16-byte tag). A 16 KiB view chunk fits, but a snapshot or a paste can be bigger. The pump must split, never truncate or error. → Task 2, test `a_write_bigger_than_one_noise_message_arrives_whole`.
3. **Something that is not a Noise initiator connects** (a port scanner, a browser, a chat on the Mac). The host closes it without a panic and serves the next real link. → Task 3, test `garbage_on_the_port_is_closed_and_the_next_link_is_served`.
4. **Bytes that are not text are written from the phone** (the quick-key bar's Ctrl-C is `0x03`; Esc is `0x1b`). `write` carries bytes, base64 on the wire; the phone must not pass them through a string. → Task 4, test `write_carries_a_control_byte_unchanged`.
5. **The host is unreachable** (wrong address, Mac asleep, other Wi-Fi). The phone's Connect must fail within a bounded time with a message, never hang the UI. → Task 4, test `connecting_to_nothing_fails_within_ten_seconds`.

---

## File structure

```
~/purlis-research/spike/                      a new git repository, no remote
├── Cargo.toml                                 [workspace] members = ["noise", "host"]
├── rust-toolchain.toml                        copied from the clone (1.99.0)
├── .gitignore                                 target/, node_modules/, phone/src-tauri/gen/apple/build
├── noise/                                     Noise XX over any stream, pumped into a DuplexStream
│   ├── Cargo.toml
│   └── src/lib.rs                             keypair(), initiate(), respond(), pump()
├── host/                                      the Mac side
│   ├── Cargo.toml
│   ├── src/lib.rs                             serve_on(): TCP listener → noise::respond → serve_as_a_device → session::serve
│   ├── src/board.rs                           the Host: list, attach (bursts), write (logged), stop
│   ├── src/main.rs                            flags, the corpus, the keypair; calls spike_host::serve_on
│   └── tests/
│       └── a_phone_links_lists_and_reads_a_burst.rs
├── phone/                                     Tauri 2 iOS app (create-tauri-app, react-ts)
│   ├── package.json
│   ├── src/App.tsx                            connect · chats · attach · xterm · composer · counters
│   ├── src/main.tsx                           (template)
│   └── src-tauri/
│       ├── Cargo.toml                         + purlis-session-protocol (any-stream), spike-noise, base64
│       ├── src/lib.rs                         Tauri commands: connect, attach, write
│       ├── src/link.rs                        plain Rust: Linked, connect_to(), read_view(); testable on macOS
│       ├── tests/link.rs                      the two Review Focus tests
│       └── gen/apple/                         `tauri ios init` output; Info.plist gains NSLocalNetworkUsageDescription
└── midturn/
    ├── midturn.sh                             tmux: start a harness, prompt, paste mid-turn, capture
    └── *.log                                  one capture per harness (committed as evidence)

~/purlis-research/purlis/  (branch ios-mobile-design)
├── docs/superpowers/plans/2026-10-11-ios-phase-0-spike.md        this plan
├── docs/superpowers/specs/2026-10-11-ios-phase-0-findings.md     Task 7 writes it
└── docs/superpowers/specs/2026-10-11-ios-mobile-design.md        §7 gains one line pointing at the findings
```

---

### Task 0: The toolchain, the harnesses and a green baseline

**Files:**
- Create: `~/purlis-research/spike/rust-toolchain.toml` (a copy)
- Create: `~/purlis-research/spike/Cargo.toml`, `~/purlis-research/spike/.gitignore`

**Interfaces:**
- Produces: a working `cargo` for 1.99.0 with the targets `aarch64-apple-ios` and `aarch64-apple-ios-sim`; `pod`; `codex` and `opencode` on `PATH`; the clone's protocol tests green, which is the baseline every later test is compared to.

- [ ] **Step 1: Install rustup and the pinned toolchain**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.99.0 --profile minimal -c rustfmt -c clippy
source "$HOME/.cargo/env"
rustup target add aarch64-apple-ios aarch64-apple-ios-sim --toolchain 1.99.0
rustc --version
```

Expected: `rustc 1.99.0 (…)`. If 1.99.0 is not a published stable yet on this date, `rustup` says so; then read the clone's `rust-toolchain.toml` again and install exactly what it names.

- [ ] **Step 2: Install CocoaPods (Tauri's iOS prerequisite) and the two missing harnesses**

```bash
brew install cocoapods
npm install -g @openai/codex opencode-ai
pod --version && codex --version && opencode --version
```

Expected: three version lines. Then log each harness in **yourself** (interactive; in this session type `! codex login` and `! opencode auth login`). `claude` is already logged in.

- [ ] **Step 3: Prove the clone's protocol tests are green with this compiler**

```bash
cd ~/purlis-research/purlis && cargo test -p purlis-session-protocol 2>&1 | tail -5
```

Expected: every test binary ends `test result: ok`. This is the baseline. If anything is red here, stop: the spike cannot be told apart from a broken toolchain.

- [ ] **Step 4: Start the spike repository**

```bash
mkdir -p ~/purlis-research/spike && cd ~/purlis-research/spike && git init -q
cp ~/purlis-research/purlis/rust-toolchain.toml .
cat > Cargo.toml <<'EOF'
[workspace]
resolver = "3"
members = ["noise", "host"]

[workspace.dependencies]
purlis-session-protocol = { path = "../purlis/crates/session-protocol", features = ["any-stream"] }
tokio = { version = "1", features = ["io-util", "macros", "net", "rt-multi-thread", "sync", "time"] }
snow = "0.10"
bytes = "1"
EOF
cat > .gitignore <<'EOF'
target/
node_modules/
phone/src-tauri/gen/apple/build/
phone/dist/
EOF
git add -A && git commit -q -m "Start the Phase 0 spike workspace with the clone's toolchain pin"
```

Expected: one commit. `phone/` is not a member: Tauri's `src-tauri` is its own crate with its own `Cargo.toml`, added in Task 4.

---

### Task 1: `noise`: a keypair, and the XX handshake over any stream

**Files:**
- Create: `~/purlis-research/spike/noise/Cargo.toml`
- Create: `~/purlis-research/spike/noise/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub struct Keypair { pub private: Vec<u8>, pub public: Vec<u8> }` and `pub fn keypair() -> Keypair`
  - `pub async fn initiate<S: AsyncRead + AsyncWrite + Unpin>(io: &mut S, ours: &Keypair) -> Result<Proved, Error>`
  - `pub async fn respond<S: AsyncRead + AsyncWrite + Unpin>(io: &mut S, ours: &Keypair) -> Result<Proved, Error>`
  - `pub struct Proved { pub remote_public: Vec<u8>, transport: snow::TransportState }`
  - `pub const PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s"`
  - Framing on the wire, used by Task 2 too: each Noise message is a big-endian `u16` length then the message.

- [ ] **Step 1: Write the failing test**

`noise/src/lib.rs`:

```rust
//! Noise `XX` over any ordered byte stream, for the Phase 0 spike only. The real handshake is
//! RR-13's, inside `purlis-core::remote` (spec §3); this one exists to show `snow` links and
//! runs on an iPhone and that the protocol crate is happy behind it.

use tokio::io::{AsyncRead, AsyncWrite};

pub const PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

#[derive(Debug, Clone)]
pub struct Keypair {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

pub struct Proved {
    pub remote_public: Vec<u8>,
    pub(crate) transport: snow::TransportState,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("noise: {0}")]
    Noise(#[from] snow::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("the peer sent a message of {0} bytes, more than a Noise message holds")]
    TooLong(usize),
}

pub fn keypair() -> Keypair {
    todo!()
}

pub async fn initiate<S: AsyncRead + AsyncWrite + Unpin>(
    _io: &mut S,
    _ours: &Keypair,
) -> Result<Proved, Error> {
    todo!()
}

pub async fn respond<S: AsyncRead + AsyncWrite + Unpin>(
    _io: &mut S,
    _ours: &Keypair,
) -> Result<Proved, Error> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn each_side_learns_the_others_static_key() {
        let (mut a, mut b) = tokio::io::duplex(4096);
        let phone = keypair();
        let desktop = keypair();
        let (p, d) = tokio::join!(initiate(&mut a, &phone), respond(&mut b, &desktop));
        let (p, d) = (p.unwrap(), d.unwrap());
        assert_eq!(p.remote_public, desktop.public);
        assert_eq!(d.remote_public, phone.public);
        assert_ne!(phone.public, desktop.public);
    }
}
```

`noise/Cargo.toml`:

```toml
[package]
name = "spike-noise"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
snow.workspace = true
tokio.workspace = true
bytes.workspace = true
thiserror = "2"
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cd ~/purlis-research/spike && cargo test -p spike-noise each_side_learns 2>&1 | tail -15`
Expected: FAIL, panicking at `not yet implemented` in `keypair`.

- [ ] **Step 3: Implement the keypair and the three XX messages**

Replace the three `todo!()` bodies and add the two frame helpers:

```rust
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The most a Noise message holds, tag included.
const MOST_MESSAGE: usize = 65_535;

pub fn keypair() -> Keypair {
    let pair = snow::Builder::new(PATTERN.parse().expect("the pattern is well formed"))
        .generate_keypair()
        .expect("the OS generator works");
    Keypair {
        private: pair.private,
        public: pair.public,
    }
}

fn builder(ours: &Keypair) -> Result<snow::Builder<'_>, Error> {
    Ok(snow::Builder::new(PATTERN.parse().expect("the pattern is well formed"))
        .local_private_key(&ours.private)?)
}

/// One framed Noise message out: a big-endian `u16` length, then the bytes.
pub(crate) async fn send_frame<S: AsyncWrite + Unpin>(io: &mut S, message: &[u8]) -> Result<(), Error> {
    if message.len() > MOST_MESSAGE {
        return Err(Error::TooLong(message.len()));
    }
    io.write_all(&(message.len() as u16).to_be_bytes()).await?;
    io.write_all(message).await?;
    io.flush().await?;
    Ok(())
}

/// One framed Noise message in.
pub(crate) async fn read_frame<S: AsyncRead + Unpin>(io: &mut S) -> Result<Vec<u8>, Error> {
    let mut len = [0u8; 2];
    io.read_exact(&mut len).await?;
    let mut message = vec![0u8; u16::from_be_bytes(len) as usize];
    io.read_exact(&mut message).await?;
    Ok(message)
}

pub async fn initiate<S: AsyncRead + AsyncWrite + Unpin>(
    io: &mut S,
    ours: &Keypair,
) -> Result<Proved, Error> {
    let mut hs = builder(ours)?.build_initiator()?;
    let mut buf = vec![0u8; MOST_MESSAGE];
    // -> e
    let n = hs.write_message(&[], &mut buf)?;
    send_frame(io, &buf[..n]).await?;
    // <- e, ee, s, es
    let m = read_frame(io).await?;
    hs.read_message(&m, &mut buf)?;
    // -> s, se
    let n = hs.write_message(&[], &mut buf)?;
    send_frame(io, &buf[..n]).await?;
    finish(hs)
}

pub async fn respond<S: AsyncRead + AsyncWrite + Unpin>(
    io: &mut S,
    ours: &Keypair,
) -> Result<Proved, Error> {
    let mut hs = builder(ours)?.build_responder()?;
    let mut buf = vec![0u8; MOST_MESSAGE];
    // <- e
    let m = read_frame(io).await?;
    hs.read_message(&m, &mut buf)?;
    // -> e, ee, s, es
    let n = hs.write_message(&[], &mut buf)?;
    send_frame(io, &buf[..n]).await?;
    // <- s, se
    let m = read_frame(io).await?;
    hs.read_message(&m, &mut buf)?;
    finish(hs)
}

fn finish(hs: snow::HandshakeState) -> Result<Proved, Error> {
    let remote_public = hs
        .get_remote_static()
        .expect("XX has exchanged both static keys by now")
        .to_vec();
    Ok(Proved {
        remote_public,
        transport: hs.into_transport_mode()?,
    })
}
```

If `snow` 0.10's `local_private_key` returns a `Builder` rather than a `Result`, drop the `?` after it; the compiler says which.

- [ ] **Step 4: Run the test to see it pass**

Run: `cargo test -p spike-noise each_side_learns 2>&1 | tail -5`
Expected: `test result: ok. 1 passed`.

- [ ] **Step 5: Commit**

```bash
git add noise && git commit -q -m "Noise XX handshake over any stream, both sides learn the other's static key"
```

---

### Task 2: `noise::pump`: the transport as an ordinary `AsyncRead + AsyncWrite`

The protocol crate wants a plain stream. Rather than implement `AsyncRead`/`AsyncWrite` by hand over `snow`, two tasks pump between the TCP socket and one end of a `tokio::io::duplex`, and the other end is handed to the protocol crate.

**Files:**
- Modify: `~/purlis-research/spike/noise/src/lib.rs`

**Interfaces:**
- Produces: `pub fn pump<S>(io: S, proved: Proved) -> tokio::io::DuplexStream` where `S: AsyncRead + AsyncWrite + Unpin + Send + 'static`. Everything written to the returned stream leaves `io` encrypted; everything decrypted from `io` is readable from it. Dropping the returned stream ends both pump tasks.

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module in `noise/src/lib.rs`:

```rust
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn pumped() -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
        let (mut a, mut b) = tokio::io::duplex(256 * 1024);
        let (p, d) = tokio::join!(initiate(&mut a, &keypair()), respond(&mut b, &keypair()));
        (pump(a, p.unwrap()), pump(b, d.unwrap()))
    }

    #[tokio::test]
    async fn plaintext_written_on_one_end_is_read_on_the_other() {
        let (mut phone, mut desktop) = pumped().await;
        phone.write_all(b"hello").await.unwrap();
        let mut got = [0u8; 5];
        desktop.read_exact(&mut got).await.unwrap();
        assert_eq!(&got, b"hello");
        desktop.write_all(b"back").await.unwrap();
        let mut got = [0u8; 4];
        phone.read_exact(&mut got).await.unwrap();
        assert_eq!(&got, b"back");
    }

    #[tokio::test]
    async fn a_write_bigger_than_one_noise_message_arrives_whole() {
        // Review Focus 2: a snapshot or a paste can exceed 65,519 plaintext bytes.
        let (mut phone, mut desktop) = pumped().await;
        let big: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let sent = big.clone();
        let writer = tokio::spawn(async move { phone.write_all(&sent).await.unwrap(); phone });
        let mut got = vec![0u8; big.len()];
        desktop.read_exact(&mut got).await.unwrap();
        assert_eq!(got, big);
        drop(writer.await.unwrap());
    }

    #[tokio::test]
    async fn the_bytes_on_the_wire_are_not_the_plaintext() {
        let (mut a, mut b) = tokio::io::duplex(256 * 1024);
        let (p, d) = tokio::join!(initiate(&mut a, &keypair()), respond(&mut b, &keypair()));
        let mut phone = pump(a, p.unwrap());
        let _desktop_keeps_its_end = d.unwrap();
        phone.write_all(b"secret secret secret").await.unwrap();
        let mut wire = vec![0u8; 2 + 20 + 16];
        b.read_exact(&mut wire).await.unwrap();
        assert!(!wire.windows(6).any(|w| w == b"secret"));
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p spike-noise 2>&1 | grep -E "error|test result" | head`
Expected: a compile error, `cannot find function pump`.

- [ ] **Step 3: Implement `pump`**

Add to `noise/src/lib.rs`:

```rust
use std::sync::{Arc, Mutex};
use tokio::io::DuplexStream;

/// The most plaintext one Noise message carries: the message less its 16-byte tag.
const MOST_PLAINTEXT: usize = MOST_MESSAGE - 16;

/// Pump plaintext between the returned stream and `io`, encrypting on the way out and
/// decrypting on the way in. Two tasks, one per direction; both end when either side closes.
pub fn pump<S>(io: S, proved: Proved) -> DuplexStream
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (ours, theirs) = tokio::io::duplex(256 * 1024);
    let transport = Arc::new(Mutex::new(proved.transport));
    let (mut wire_r, mut wire_w) = tokio::io::split(io);
    let (mut plain_r, mut plain_w) = tokio::io::split(theirs);

    let out = Arc::clone(&transport);
    tokio::spawn(async move {
        let mut plain = vec![0u8; MOST_PLAINTEXT];
        let mut cipher = vec![0u8; MOST_MESSAGE];
        loop {
            let n = match plain_r.read(&mut plain).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let len = match out.lock().unwrap().write_message(&plain[..n], &mut cipher) {
                Ok(len) => len,
                Err(_) => break,
            };
            if send_frame(&mut wire_w, &cipher[..len]).await.is_err() {
                break;
            }
        }
        let _ = wire_w.shutdown().await;
    });

    tokio::spawn(async move {
        let mut plain = vec![0u8; MOST_MESSAGE];
        loop {
            let frame = match read_frame(&mut wire_r).await {
                Ok(frame) => frame,
                Err(_) => break,
            };
            let n = match transport.lock().unwrap().read_message(&frame, &mut plain) {
                Ok(n) => n,
                Err(_) => break,
            };
            if plain_w.write_all(&plain[..n]).await.is_err() {
                break;
            }
        }
        let _ = plain_w.shutdown().await;
    });

    ours
}
```

The `Mutex` is `std`'s and is never held across an `.await`, so it never blocks the runtime. The pump runs on `tokio`'s multi-thread runtime because it `spawn`s; the host and the phone both build one.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p spike-noise 2>&1 | tail -5`
Expected: `test result: ok. 4 passed`.

- [ ] **Step 5: Commit**

```bash
git add noise && git commit -q -m "Pump the Noise transport into a duplex stream, splitting writes bigger than one message"
```

---

### Task 3: `host`: the Mac serves the session protocol as a device link and feeds the bursts

**Files:**
- Create: `~/purlis-research/spike/host/Cargo.toml`
- Create: `~/purlis-research/spike/host/src/lib.rs` (the library: `board` and `serve_on`, so the tests and the phone's tests can import them; a binary crate cannot be imported)
- Create: `~/purlis-research/spike/host/src/board.rs`
- Create: `~/purlis-research/spike/host/src/main.rs` (the binary, a thin caller of the library)
- Create: `~/purlis-research/spike/host/tests/a_phone_links_lists_and_reads_a_burst.rs`

**Interfaces:**
- Consumes: `spike_noise::{keypair, respond, pump, Keypair}`; from the protocol crate `link::{serve_as_a_device, connect_as_a_device, ProvenDevice, Acceptor}`, `session::{serve, Host, Client, Command, Answer, Chat, Peer, Refusal}`, `version::{Speaks, Version}`, `view::{Attacher, Feed, Limits, ViewId, Viewer, Chunk}`. Neither `Feed` nor `Client` is `Clone`; both are shared through `Arc`.
- Produces:
  - `spike_host::board::Board` implementing `Host`; `Board::new(corpus: Bytes) -> Board`; `Board` is `Clone`
  - `spike_host::serve_on(listener: TcpListener, ours: Keypair, board: Board) -> impl Future<Output = ()>`: accepts forever, one task per connection
  - Chats listed: `burst2` (corpus ×16), `burst13` (corpus ×99), `echo` (a 24-line snapshot, then every `write` echoed back as live bytes, so a keystroke's round trip can be timed by eye)
  - `write` bytes are printed to stderr as `write <chat> <len> bytes: <hex of the first 16>`
  - The binary: `spike-host [--listen 0.0.0.0:7777] [--corpus ~/purlis-research/purlis/fixtures/corpora/claude-code-session.raw]`; it prints its public key as hex on start.

- [ ] **Step 1: Write the failing integration test**

`host/tests/a_phone_links_lists_and_reads_a_burst.rs`:

```rust
//! A phone, stood in for by this test on the Mac, links over TCP through Noise, lists the
//! chats, attaches `burst2` and reads the whole 2 MB burst, acknowledging as it draws.

use bytes::Bytes;
use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::{Client, Command};
use purlis_session_protocol::version::{Speaks, Version};
use purlis_session_protocol::view::{Chunk, Viewer};
use spike_host::board::{Board, CORPUS_TIMES_2MB};
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::net::{TcpListener, TcpStream};

const CORPUS: &[u8] = include_bytes!("../../../purlis/fixtures/corpora/claude-code-session.raw");

fn speaks() -> Speaks {
    Speaks::new([Version { major: 1, minor: 0 }])
}

async fn host_on_a_port() -> (std::net::SocketAddr, spike_noise::Keypair) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let ours = spike_noise::keypair();
    let board = Board::new(Bytes::from_static(CORPUS));
    tokio::spawn(spike_host::serve_on(listener, ours.clone(), board));
    (addr, ours)
}

async fn phone_linked(addr: std::net::SocketAddr) -> (Client, Acceptor) {
    let mut tcp = TcpStream::connect(addr).await.unwrap();
    let proved = spike_noise::initiate(&mut tcp, &spike_noise::keypair()).await.unwrap();
    let stream = spike_noise::pump(tcp, proved);
    let link = link::connect_as_a_device(stream, speaks()).await.unwrap();
    let (client, acceptor, _events) = Client::new(link);
    (client, acceptor)
}

#[tokio::test(flavor = "multi_thread")]
async fn the_phone_lists_three_chats_and_reads_the_whole_2mb_burst() {
    let (addr, _) = host_on_a_port().await;
    let (client, mut acceptor) = phone_linked(addr).await;

    let chats = client.list().await.unwrap();
    let names: Vec<&str> = chats.iter().map(|c| c.chat.as_str()).collect();
    assert_eq!(names, ["burst2", "burst13", "echo"]);

    let attached = client.attach("burst2").await.unwrap();
    assert_eq!((attached.cols, attached.rows), (120, 40));
    let mut reader = Viewer::default().accept(acceptor.accept().await.unwrap()).await.unwrap();
    let want = CORPUS.len() * CORPUS_TIMES_2MB;
    let mut got = 0usize;
    while got < want {
        match reader.next().await.unwrap().unwrap() {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => {
                reader.ack(bytes.len()).await.unwrap();
                got += bytes.len();
            }
        }
    }
    assert_eq!(got, want);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_write_is_echoed_back_on_the_echo_chat() {
    let (addr, _) = host_on_a_port().await;
    let (client, mut acceptor) = phone_linked(addr).await;
    client.attach("echo").await.unwrap();
    let mut reader = Viewer::default().accept(acceptor.accept().await.unwrap()).await.unwrap();
    client.write("echo", b"ls\r").await.unwrap();
    // The snapshot's chunks come first and are acked and dropped; the live ones are the echo.
    let mut live = Vec::new();
    while live.len() < 3 {
        match reader.next().await.unwrap().unwrap() {
            Chunk::Snapshot(b) => reader.ack(b.len()).await.unwrap(),
            Chunk::Live(b) => {
                reader.ack(b.len()).await.unwrap();
                live.extend_from_slice(&b);
            }
        }
    }
    assert_eq!(live, b"ls\r");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_client_that_vanishes_mid_burst_does_not_stop_the_host() {
    // Review Focus 1: iOS suspends the app and the TCP link dies mid-burst.
    let (addr, _) = host_on_a_port().await;
    {
        let (client, mut acceptor) = phone_linked(addr).await;
        client.attach("burst13").await.unwrap();
        let mut reader = Viewer::default().accept(acceptor.accept().await.unwrap()).await.unwrap();
        let _first = reader.next().await.unwrap().unwrap();
        // The phone vanishes: everything is dropped, the socket closes under the host.
    }
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let (client, _) = phone_linked(addr).await;
    assert_eq!(client.list().await.unwrap().len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn garbage_on_the_port_is_closed_and_the_next_link_is_served() {
    // Review Focus 3: a port scanner, a browser, a chat on the Mac.
    let (addr, _) = host_on_a_port().await;
    let mut tcp = TcpStream::connect(addr).await.unwrap();
    tcp.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
    let mut sink = [0u8; 64];
    let closed = tokio::time::timeout(std::time::Duration::from_secs(12), tcp.read(&mut sink)).await;
    assert!(matches!(closed, Ok(Ok(0)) | Ok(Err(_))), "the host should close a non-Noise peer");
    let (client, _) = phone_linked(addr).await;
    assert_eq!(client.list().await.unwrap().len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn resize_and_start_are_refused_as_the_spec_rules_for_a_device() {
    let (addr, _) = host_on_a_port().await;
    let (client, _) = phone_linked(addr).await;
    let resized = client.call(Command::Resize { chat: "echo".into(), cols: 40, rows: 20 }).await;
    assert!(resized.is_err(), "resize must be refused: spec §5.4");
}
```

`host/Cargo.toml`:

```toml
[package]
name = "spike-host"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
spike-noise = { path = "../noise" }
purlis-session-protocol.workspace = true
tokio.workspace = true
bytes.workspace = true
hex = "0.4"
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p spike-host 2>&1 | grep -E "^error|could not find" | head -5`
Expected: compile errors, `could not find board in spike_host` / `cannot find function serve_on`.

- [ ] **Step 3: Implement the board and the server**

`host/src/board.rs`:

```rust
//! What the spike host answers: three chats and nothing else. `burst2` and `burst13` push the
//! bench's corpus into the view ×16 and ×99, the spec's L3 bursts; `echo` writes back what is
//! written to it, so a keystroke's round trip can be seen on the phone.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use purlis_session_protocol::session::{self, Answer, Attached, Chat, Command, Host, Peer, Refusal};
use purlis_session_protocol::view::{Attacher, Feed, Limits, ViewId};

pub const CORPUS_TIMES_2MB: usize = 16;
pub const CORPUS_TIMES_13MB: usize = 99;
const COLS: u16 = 120;
const ROWS: u16 = 40;
/// What one PTY read hands a host on macOS: the pieces a burst arrives in.
const PIECE: usize = 64 * 1024;

/// `Feed` is not `Clone`: the burst task and the `write` handler share one through an `Arc`.
#[derive(Clone)]
pub struct Board {
    corpus: Bytes,
    feeds: Arc<Mutex<HashMap<String, Arc<Feed>>>>,
    next_view: Arc<Mutex<u32>>,
}

impl Board {
    pub fn new(corpus: Bytes) -> Board {
        Board {
            corpus,
            feeds: Arc::new(Mutex::new(HashMap::new())),
            next_view: Arc::new(Mutex::new(1)),
        }
    }

    fn chat(id: &str, title: &str, state: &str) -> Chat {
        Chat {
            chat: id.into(),
            project: "spike".into(),
            project_path: Some("/spike".into()),
            title: Some(title.into()),
            state: state.into(),
        }
    }

    fn refuse(code: &str, why: impl Into<String>) -> Refusal {
        Refusal { code: code.into(), why: why.into() }
    }

    fn view_id(&self) -> ViewId {
        let mut next = self.next_view.lock().unwrap();
        let id = ViewId(*next);
        *next += 1;
        id
    }

    async fn attach(&self, chat: &str, peer: &Peer) -> Result<Answer, Refusal> {
        let times = match chat {
            "burst2" => Some(CORPUS_TIMES_2MB),
            "burst13" => Some(CORPUS_TIMES_13MB),
            "echo" => None,
            _ => return Err(Self::refuse(session::NO_SUCH_CHAT, format!("no chat {chat}"))),
        };
        let view = self.view_id();
        let snapshot = Bytes::from_static(b"\x1b[2J\x1b[Hspike$ ");
        let feed = Arc::new(
            Attacher::new(peer.views().clone(), Limits::default())
                .attach(view, snapshot)
                .await
                .map_err(|e| Self::refuse("failed", e.to_string()))?,
        );
        if let Some(times) = times {
            let corpus = self.corpus.clone();
            let burst_feed = Arc::clone(&feed);
            tokio::spawn(async move {
                for _ in 0..times {
                    for piece in corpus.chunks(PIECE) {
                        if burst_feed.push(Bytes::copy_from_slice(piece)).is_err() {
                            return;
                        }
                    }
                }
            });
        }
        self.feeds.lock().unwrap().insert(chat.to_owned(), feed);
        Ok(Answer::Attached(Attached { view: view.0, cols: COLS, rows: ROWS }))
    }
}

impl Host for Board {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        match command {
            Command::List => Ok(Answer::Chats(vec![
                Self::chat("burst2", "2 MB burst", "working"),
                Self::chat("burst13", "13 MB burst", "working"),
                Self::chat("echo", "echo", "idle"),
            ])),
            Command::Attach { chat } => self.attach(&chat, peer).await,
            Command::Detach { chat, .. } => {
                self.feeds.lock().unwrap().remove(&chat);
                Ok(Answer::Done)
            }
            Command::Write { chat, bytes } => {
                let head: Vec<u8> = bytes.iter().copied().take(16).collect();
                eprintln!("write {chat} {} bytes: {}", bytes.len(), hex::encode(head));
                if chat == "echo" {
                    if let Some(feed) = self.feeds.lock().unwrap().get(&chat) {
                        let _ = feed.push(Bytes::from(bytes));
                    }
                }
                Ok(Answer::Done)
            }
            Command::Resize { .. } => Err(Self::refuse("not_allowed", "a device never resizes (spec §5.4)")),
            Command::Start(_) => Err(Self::refuse("not_allowed", "a device never starts a chat in phase 1")),
            Command::Stop { chat } => {
                eprintln!("stop {chat}");
                self.feeds.lock().unwrap().remove(&chat);
                Ok(Answer::Done)
            }
            Command::Answer { chat, ask, answer } => {
                eprintln!("answer {chat} {ask} {answer}");
                Ok(Answer::Done)
            }
            Command::Subscribe { .. } | Command::Unsubscribe => Ok(Answer::Done),
        }
    }
}
```

`session::NO_SUCH_CHAT` is public: the crate's own tests use it.

`host/src/lib.rs`:

```rust
//! The Phase 0 spike host: on the Mac, serve the session protocol to a phone over Noise XX.

use purlis_session_protocol::link::{self, ProvenDevice};
use purlis_session_protocol::session;
use purlis_session_protocol::version::{Speaks, Version};
use tokio::net::TcpListener;

pub mod board;
use board::Board;

/// Accept links forever: one task per connection, each a Noise responder, then the protocol.
pub async fn serve_on(listener: TcpListener, ours: spike_noise::Keypair, board: Board) {
    loop {
        let (mut tcp, from) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(e) => {
                eprintln!("accept: {e}");
                continue;
            }
        };
        let ours = ours.clone();
        let board = board.clone();
        tokio::spawn(async move {
            let proved = match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                spike_noise::respond(&mut tcp, &ours),
            )
            .await
            {
                Ok(Ok(proved)) => proved,
                Ok(Err(e)) => return eprintln!("{from}: not a Noise peer: {e}"),
                Err(_) => return eprintln!("{from}: handshake timed out"),
            };
            eprintln!("{from}: linked, phone key {}", hex::encode(&proved.remote_public));
            let stream = spike_noise::pump(tcp, proved);
            let speaks = Speaks::new([Version { major: 1, minor: 0 }]);
            let link = match link::serve_as_a_device(stream, speaks, ProvenDevice::stand_in()).await {
                Ok(link) => link,
                Err(e) => return eprintln!("{from}: link: {e}"),
            };
            if let Err(e) = session::serve(link, board, None).await {
                eprintln!("{from}: ended: {e}");
            }
            eprintln!("{from}: gone");
        });
    }
}
```

`host/src/main.rs`:

```rust
use std::path::PathBuf;

use bytes::Bytes;
use spike_host::board::{self, Board};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let mut listen = "0.0.0.0:7777".to_owned();
    let mut corpus = PathBuf::from(std::env::var("HOME").unwrap())
        .join("purlis-research/purlis/fixtures/corpora/claude-code-session.raw");
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().expect("each flag takes a value");
        match flag.as_str() {
            "--listen" => listen = value,
            "--corpus" => corpus = value.into(),
            other => panic!("unknown flag {other}"),
        }
    }
    let corpus = Bytes::from(std::fs::read(&corpus).expect("the corpus is readable"));
    let ours = spike_noise::keypair();
    eprintln!("spike-host on {listen}, public key {}", hex::encode(&ours.public));
    eprintln!("corpus {} bytes: burst2 = {} bytes, burst13 = {} bytes",
        corpus.len(), corpus.len() * board::CORPUS_TIMES_2MB, corpus.len() * board::CORPUS_TIMES_13MB);
    let listener = TcpListener::bind(&listen).await.expect("the port is free");
    spike_host::serve_on(listener, ours, Board::new(corpus)).await;
}
```

The `Speaks` version: read `crates/session-protocol/src/version.rs` for the constant the crate's own tests use (`Version { major: 1, minor: 0 }` in `a_view_is_a_snapshot_then_the_live_stream.rs`). If the host refuses the test's version, both sides name the same one, so change both to what the crate speaks today.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p spike-host 2>&1 | tail -8`
Expected: `test result: ok. 5 passed`. The 13 MB vanish test may take a second or two.

- [ ] **Step 5: Run the host by hand once and keep it running for Task 5**

Run: `cargo run -p spike-host --release`
Expected on stderr: `spike-host on 0.0.0.0:7777, public key …` and the corpus line showing `burst2 = 2114336 bytes, burst13 = 13082454 bytes`. Leave it; Ctrl-C when the phone work is done.

- [ ] **Step 6: Commit**

```bash
git add host && git commit -q -m "Spike host: serve the session protocol as a device link over Noise, with 2 MB and 13 MB burst chats"
```

---

### Task 4: `phone`: the Tauri 2 iOS shell, its Rust link, and the two link tests on macOS

The Rust side is plain Rust plus three Tauri commands. `link.rs` has no Tauri in it, so its tests run with `cargo test` on the Mac. iOS comes in Task 5.

**Files:**
- Create: `~/purlis-research/spike/phone/` from `create-tauri-app`
- Modify: `~/purlis-research/spike/phone/src-tauri/Cargo.toml`
- Create: `~/purlis-research/spike/phone/src-tauri/src/link.rs`
- Modify: `~/purlis-research/spike/phone/src-tauri/src/lib.rs`
- Create: `~/purlis-research/spike/phone/src-tauri/tests/link.rs`

**Interfaces:**
- Consumes: `spike_noise::{keypair, initiate, pump}`; `link::connect_as_a_device`; `session::{Client, Acceptor}`; `view::{Viewer, Chunk}`.
- Produces (in `link.rs`):
  - `pub struct Linked { pub client: Client, pub acceptor: Acceptor }` (`Acceptor` is `purlis_session_protocol::link::Acceptor`)
  - `pub async fn connect_to(addr: &str) -> Result<Linked, String>`: TCP + Noise + protocol, all inside a 10 s `tokio::time::timeout`
  - `pub async fn read_view(acceptor: &mut Acceptor, mut on_bytes: impl FnMut(Kind, Vec<u8>)) -> Result<(), String>`: accepts the next view stream, acks every chunk, calls `on_bytes` per chunk; `pub enum Kind { Snapshot, Live }`
  - `pub async fn write(client: &Client, chat: &str, bytes: &[u8]) -> Result<(), String>`
- Produces (Tauri commands in `lib.rs`): `connect(addr) -> Vec<ChatRow>`, `attach(chat) -> AttachedRow`, `write_text(chat, text)`, `write_bytes(chat, bytes: Vec<u8>)`; event `"view"` with payload `{ kind: "snapshot"|"live", b64: string }`.

- [ ] **Step 1: Scaffold the app**

```bash
cd ~/purlis-research/spike
npm create tauri-app@4.7.5 phone -- --template react-ts --manager npm --identifier dev.purlis.spike --yes
cd phone && npm install && npm install @xterm/xterm@^6.0.0 @xterm/addon-fit@^0.11.0
cd src-tauri && cat >> Cargo.toml <<'EOF'

[dependencies.spike-noise]
path = "../../noise"

[dependencies.purlis-session-protocol]
path = "../../../purlis/crates/session-protocol"
features = ["any-stream"]

[dependencies.tokio]
version = "1"
features = ["io-util", "macros", "net", "rt-multi-thread", "sync", "time"]

[dependencies.base64]
version = "0.23"
EOF
cp ../../rust-toolchain.toml .
cargo build 2>&1 | tail -3
```

Expected: the template builds on macOS (`Finished`). If `create-tauri-app` asks questions despite `--yes`, answer: TypeScript/JavaScript → npm → React → TypeScript.

- [ ] **Step 2: Write the failing tests**

`phone/src-tauri/tests/link.rs`:

```rust
//! The phone's Rust side, tested on the Mac: it is plain Rust until Tauri wraps it.

use std::time::{Duration, Instant};

use bytes::Bytes;
use phone_lib::link::{connect_to, read_view, write, Kind};
use spike_host::board::Board;
use tokio::net::TcpListener;

async fn host() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    tokio::spawn(spike_host::serve_on(listener, spike_noise::keypair(), Board::new(Bytes::from_static(b"corpus"))));
    addr
}

#[tokio::test(flavor = "multi_thread")]
async fn connecting_to_nothing_fails_within_ten_seconds() {
    // Review Focus 5: a wrong address or a sleeping Mac must not hang the UI.
    let began = Instant::now();
    let err = connect_to("192.0.2.1:7777").await.err().expect("nothing listens on TEST-NET-1");
    assert!(began.elapsed() < Duration::from_secs(11), "took {:?}", began.elapsed());
    assert!(!err.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn write_carries_a_control_byte_unchanged() {
    // Review Focus 4: Ctrl-C from the quick-key bar is 0x03, never text.
    let addr = host().await;
    let mut linked = connect_to(&addr).await.unwrap();
    linked.client.attach("echo").await.unwrap();
    write(&linked.client, "echo", &[0x03]).await.unwrap();
    let mut live = Vec::new();
    read_view(&mut linked.acceptor, |kind, bytes| {
        if kind == Kind::Live {
            live.extend(bytes);
        }
    })
    .await
    .ok();
    assert_eq!(live, [0x03]);
}
```

For the second test, `read_view` must return once the stream ends; the test drops the link after the echo arrives by stopping the host task. Make the test deterministic: run `read_view` in a task with a 2 s timeout and assert on what it collected:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn write_carries_a_control_byte_unchanged() {
    let addr = host().await;
    let mut linked = connect_to(&addr).await.unwrap();
    linked.client.attach("echo").await.unwrap();
    write(&linked.client, "echo", &[0x03]).await.unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let reading = tokio::spawn(async move {
        read_view(&mut linked.acceptor, |kind, bytes| {
            if kind == Kind::Live {
                let _ = tx.send(bytes);
            }
        })
        .await
    });
    let live = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.unwrap().unwrap();
    assert_eq!(live, [0x03]);
    reading.abort();
}
```

Add to `phone/src-tauri/Cargo.toml`:

```toml
[dev-dependencies]
spike-host = { path = "../../host" }
bytes = "1"
```

The template names the library `phone_lib` (`[lib] name = "phone_lib"`); if it chose another name, use that in the test's `use`.

- [ ] **Step 3: Run the tests to see them fail**

Run: `cd ~/purlis-research/spike/phone/src-tauri && cargo test --test link 2>&1 | grep -E "^error" | head -3`
Expected: `error[E0432]: unresolved import phone_lib::link`.

- [ ] **Step 4: Implement `link.rs` and the commands**

`phone/src-tauri/src/link.rs`:

```rust
//! The phone's link to a host: TCP, Noise XX, then the session protocol as a device. Plain
//! Rust, so it is tested on the Mac; the Tauri commands in `lib.rs` are a thin skin over it.

use std::time::Duration;

use purlis_session_protocol::link::{self, Acceptor};
use purlis_session_protocol::session::Client;
use purlis_session_protocol::version::{Speaks, Version};
use purlis_session_protocol::view::{Chunk, Viewer};
use tokio::net::TcpStream;

pub struct Linked {
    pub client: Client,
    pub acceptor: Acceptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Snapshot,
    Live,
}

const CONNECT_WITHIN: Duration = Duration::from_secs(10);

pub async fn connect_to(addr: &str) -> Result<Linked, String> {
    tokio::time::timeout(CONNECT_WITHIN, async {
        let mut tcp = TcpStream::connect(addr).await.map_err(|e| format!("connect {addr}: {e}"))?;
        tcp.set_nodelay(true).ok();
        let proved = spike_noise::initiate(&mut tcp, &spike_noise::keypair())
            .await
            .map_err(|e| format!("handshake: {e}"))?;
        let stream = spike_noise::pump(tcp, proved);
        let speaks = Speaks::new([Version { major: 1, minor: 0 }]);
        let link = link::connect_as_a_device(stream, speaks)
            .await
            .map_err(|e| format!("link: {e}"))?;
        let (client, acceptor, _events) = Client::new(link);
        Ok(Linked { client, acceptor })
    })
    .await
    .unwrap_or_else(|_| Err(format!("no answer from {addr} within {CONNECT_WITHIN:?}")))
}

/// Accept the next view the host opens and hand every chunk over, acknowledging each one, until
/// the stream ends.
pub async fn read_view(
    acceptor: &mut Acceptor,
    mut on_bytes: impl FnMut(Kind, Vec<u8>),
) -> Result<(), String> {
    let stream = acceptor.accept().await.map_err(|e| format!("accept: {e}"))?;
    let mut reader = Viewer::default().accept(stream).await.map_err(|e| format!("view: {e}"))?;
    while let Some(chunk) = reader.next().await {
        let chunk = chunk.map_err(|e| format!("read: {e}"))?;
        let (kind, bytes) = match chunk {
            Chunk::Snapshot(b) => (Kind::Snapshot, b),
            Chunk::Live(b) => (Kind::Live, b),
        };
        reader.ack(bytes.len()).await.map_err(|e| format!("ack: {e}"))?;
        on_bytes(kind, bytes.to_vec());
    }
    Ok(())
}

pub async fn write(client: &Client, chat: &str, bytes: &[u8]) -> Result<(), String> {
    client.write(chat, bytes).await.map_err(|e| e.to_string())
}
```

`phone/src-tauri/src/lib.rs` (replace the template's greet command):

```rust
pub mod link;

use std::sync::Arc;

use base64::Engine;
use purlis_session_protocol::link::Acceptor;
use purlis_session_protocol::session::Client;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

/// `Client` is not `Clone`; its methods take `&self`, so an `Arc` shares it between commands.
#[derive(Default)]
struct Held {
    client: Option<Arc<Client>>,
    acceptor: Option<Arc<Mutex<Acceptor>>>,
}

type Shared = Arc<Mutex<Held>>;

#[derive(Serialize, Clone)]
struct ChatRow {
    chat: String,
    title: String,
    state: String,
}

#[derive(Serialize, Clone)]
struct AttachedRow {
    view: u32,
    cols: u16,
    rows: u16,
}

#[derive(Serialize, Clone)]
struct ViewEvent {
    kind: &'static str,
    b64: String,
}

#[tauri::command]
async fn connect(addr: String, held: State<'_, Shared>) -> Result<Vec<ChatRow>, String> {
    let linked = link::connect_to(&addr).await?;
    let chats = linked.client.list().await.map_err(|e| e.to_string())?;
    let mut held = held.lock().await;
    held.client = Some(Arc::new(linked.client));
    held.acceptor = Some(Arc::new(Mutex::new(linked.acceptor)));
    Ok(chats
        .into_iter()
        .map(|c| ChatRow { chat: c.chat, title: c.title.unwrap_or_default(), state: c.state })
        .collect())
}

#[tauri::command]
async fn attach(chat: String, app: AppHandle, held: State<'_, Shared>) -> Result<AttachedRow, String> {
    let (client, acceptor) = {
        let held = held.lock().await;
        (
            held.client.clone().ok_or("not connected")?,
            held.acceptor.clone().ok_or("not connected")?,
        )
    };
    let attached = client.attach(&chat).await.map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn(async move {
        let mut acceptor = acceptor.lock().await;
        let result = link::read_view(&mut acceptor, |kind, bytes| {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let kind = match kind {
                link::Kind::Snapshot => "snapshot",
                link::Kind::Live => "live",
            };
            let _ = app.emit("view", ViewEvent { kind, b64 });
        })
        .await;
        let _ = app.emit("view-ended", result.err().unwrap_or_default());
    });
    Ok(AttachedRow { view: attached.view, cols: attached.cols, rows: attached.rows })
}

#[tauri::command]
async fn write_text(chat: String, text: String, held: State<'_, Shared>) -> Result<(), String> {
    write_bytes(chat, text.into_bytes(), held).await
}

#[tauri::command]
async fn write_bytes(chat: String, bytes: Vec<u8>, held: State<'_, Shared>) -> Result<(), String> {
    let client = held.lock().await.client.clone().ok_or("not connected")?;
    link::write(&client, &chat, &bytes).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Shared::default())
        .invoke_handler(tauri::generate_handler![connect, attach, write_text, write_bytes])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

`held.client.clone()` clones the `Arc`, never the `Client`.

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test --test link 2>&1 | tail -5`
Expected: `test result: ok. 2 passed`. The first takes up to 10 s if the kernel does not refuse TEST-NET-1 at once.

- [ ] **Step 6: Commit**

```bash
cd ~/purlis-research/spike && git add phone && git commit -q -m "Phone: Tauri 2 shell with a plain-Rust link (connect within 10 s, read a view, write bytes)"
```

---

### Task 5: The webview: xterm.js, the bursts measured, the composer and the quick keys

**Files:**
- Modify: `~/purlis-research/spike/phone/src/App.tsx` (replace the template)
- Modify: `~/purlis-research/spike/phone/src/App.css`

**Interfaces:**
- Consumes: the four commands and the `view` / `view-ended` events from Task 4.
- Produces: a screen with (a) an address field defaulting to `192.168.20.126:7777` and **Connect**; (b) the chats as buttons; (c) an xterm.js terminal sized `cols×rows` from the attach; (d) a status line: `drawn N B · T s since attach · longest frame F ms · taps K`; (e) a **Tap me** button that increments `taps` (the "input stays responsive" probe); (f) a composer whose **Send** writes the text as one bracketed paste plus `\r`; (g) quick keys Esc ↑ ↓ Enter Tab y n Ctrl-C.

- [ ] **Step 1: Write the screen**

`phone/src/App.tsx`:

```tsx
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import "./App.css";

type ChatRow = { chat: string; title: string; state: string };
type Attached = { view: number; cols: number; rows: number };

/** Exactly what purlis's writer sends: one bracketed paste, then Enter (`curation::bracketed`). */
const PASTE_BEGINS = "\x1b[200~";
const PASTE_ENDS = "\x1b[201~";

const QUICK: Array<[string, number[]]> = [
  ["Esc", [0x1b]],
  ["↑", [0x1b, 0x5b, 0x41]],
  ["↓", [0x1b, 0x5b, 0x42]],
  ["Enter", [0x0d]],
  ["Tab", [0x09]],
  ["y", [0x79]],
  ["n", [0x6e]],
  ["Ctrl-C", [0x03]],
];

function fromB64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

export default function App() {
  const [addr, setAddr] = useState("192.168.20.126:7777");
  const [chats, setChats] = useState<ChatRow[]>([]);
  const [current, setCurrent] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [drawn, setDrawn] = useState(0);
  const [since, setSince] = useState(0);
  const [longest, setLongest] = useState(0);
  const [taps, setTaps] = useState(0);
  const [text, setText] = useState("");
  const holder = useRef<HTMLDivElement>(null);
  const term = useRef<Terminal | null>(null);
  const attachedAt = useRef(0);
  const drawnRef = useRef(0);

  // Longest gap between animation frames: a frozen webview shows up here.
  useEffect(() => {
    let last = performance.now();
    let id = 0;
    const tick = (now: number) => {
      setLongest((l) => Math.max(l, now - last));
      last = now;
      id = requestAnimationFrame(tick);
    };
    id = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(id);
  }, []);

  useEffect(() => {
    const unlisten = listen<{ kind: string; b64: string }>("view", (e) => {
      const bytes = fromB64(e.payload.b64);
      if (e.payload.kind === "snapshot" && drawnRef.current === 0) term.current?.reset();
      term.current?.write(bytes, () => {
        drawnRef.current += bytes.length;
        setDrawn(drawnRef.current);
        setSince((performance.now() - attachedAt.current) / 1000);
      });
    });
    const ended = listen<string>("view-ended", (e) => setError(`view ended ${e.payload}`));
    return () => {
      unlisten.then((f) => f());
      ended.then((f) => f());
    };
  }, []);

  async function connect() {
    setError("");
    try {
      setChats(await invoke<ChatRow[]>("connect", { addr }));
    } catch (e) {
      setError(String(e));
    }
  }

  async function attach(chat: string) {
    setError("");
    try {
      const a = await invoke<Attached>("attach", { chat });
      term.current?.dispose();
      const t = new Terminal({ cols: a.cols, rows: a.rows, fontSize: 11, scrollback: 2000 });
      t.open(holder.current!);
      t.onData((d) => invoke("write_text", { chat, text: d }));
      term.current = t;
      drawnRef.current = 0;
      setDrawn(0);
      setLongest(0);
      attachedAt.current = performance.now();
      setCurrent(chat);
    } catch (e) {
      setError(String(e));
    }
  }

  async function send() {
    if (!current) return;
    await invoke("write_text", { chat: current, text: `${PASTE_BEGINS}${text}${PASTE_ENDS}\r` });
    setText("");
  }

  return (
    <main>
      <div className="row">
        <input value={addr} onChange={(e) => setAddr(e.target.value)} />
        <button onClick={connect}>Connect</button>
      </div>
      <div className="row">
        {chats.map((c) => (
          <button key={c.chat} onClick={() => attach(c.chat)} className={c.chat === current ? "on" : ""}>
            {c.title} · {c.state}
          </button>
        ))}
        <button onClick={() => setTaps((t) => t + 1)}>Tap me</button>
      </div>
      <div className="status">
        drawn {drawn.toLocaleString()} B · {since.toFixed(2)} s since attach · longest frame {longest.toFixed(0)} ms · taps {taps}
        {error && <span className="error"> · {error}</span>}
      </div>
      <div className="term" ref={holder} />
      <div className="row">
        {QUICK.map(([label, bytes]) => (
          <button key={label} disabled={!current} onClick={() => invoke("write_bytes", { chat: current, bytes })}>
            {label}
          </button>
        ))}
      </div>
      <div className="row">
        <textarea value={text} onChange={(e) => setText(e.target.value)} rows={2} />
        <button disabled={!current} onClick={send}>Send</button>
      </div>
    </main>
  );
}
```

`phone/src/App.css` (replace the template's):

```css
main { display: flex; flex-direction: column; gap: 6px; padding: 8px; padding-top: env(safe-area-inset-top); height: 100vh; box-sizing: border-box; }
.row { display: flex; gap: 6px; flex-wrap: wrap; align-items: center; }
.row input, .row textarea { flex: 1; font: 14px ui-monospace, monospace; }
.status { font: 12px ui-monospace, monospace; }
.error { color: red; }
.term { flex: 1; overflow: auto; -webkit-overflow-scrolling: touch; }
button.on { font-weight: bold; }
```

Colour and copy rules are the clone's and do not apply to a throwaway outside it; the real `mobile/` reuses `app/src/theme` (spec §3).

- [ ] **Step 2: Check it in the desktop shell against the running host**

Run (Task 3's host still running): `cd ~/purlis-research/spike/phone && npm run tauri dev`
Expected: a macOS window. Connect to `127.0.0.1:7777` lists three chats; **echo** shows `spike$` and typing echoes; **2 MB burst** draws and the status line's `drawn` reaches `2,114,336`. This proves the screen before the iPhone is in the loop.

- [ ] **Step 3: Commit**

```bash
cd ~/purlis-research/spike && git add phone/src && git commit -q -m "Phone screen: connect, chats, xterm at the host's size, burst counters, composer and quick keys"
```

---

### Task 6: iOS: build for the device, run on the iPhone, record the numbers

**Files:**
- Modify: `~/purlis-research/spike/phone/src-tauri/gen/apple/phone_iOS/Info.plist` (generated by `tauri ios init`; the directory name follows the app name)
- Modify: `~/purlis-research/spike/phone/src-tauri/tauri.conf.json`

**Interfaces:**
- Produces: experiment 1's answer (does it build and link), experiment 2's numbers (per burst: `drawn`, seconds, longest frame, whether **Tap me** kept counting), and a keystroke-echo feel on `echo`.

- [ ] **Step 1: The cheap gate first: cross-compile the Rust library for the device**

```bash
cd ~/purlis-research/spike/phone/src-tauri && cargo build --lib --target aarch64-apple-ios 2>&1 | tail -3
```

Expected: `Finished`. This is experiment 1's first half: `session-protocol` + `snow` + tokio + `purlis-same-user` (it is `cfg(unix)` and has an `ios` branch) compile for iOS. If a crate fails here, the finding is which one and why; try `--target aarch64-apple-ios-sim` too to tell "iOS" from "this target".

- [ ] **Step 2: Initialise the Xcode project and declare local networking**

```bash
cd ~/purlis-research/spike/phone && npm run tauri ios init
ls src-tauri/gen/apple
```

Then add to the generated `Info.plist` (inside the top `<dict>`):

```xml
<key>NSLocalNetworkUsageDescription</key>
<string>The spike connects to purlis on your Mac over Wi-Fi.</string>
```

Without it iOS 14+ silently blocks LAN sockets from the app.

- [ ] **Step 3: Sign with a personal team and run on the iPhone**

On the iPhone: Settings → Privacy & Security → **Developer Mode** on (reboots). Plug it in by USB and trust the Mac.

```bash
npm run tauri ios dev -- --open --host 192.168.20.126
```

Xcode opens. In the project's **Signing & Capabilities**, set *Team* to your Apple ID's Personal Team (add the Apple ID under Xcode → Settings → Accounts if it is not there), tick *Automatically manage signing*. Pick the iPhone as the run destination and press Run. On the first launch iOS asks to trust the developer: Settings → General → VPN & Device Management → trust. Allow the local-network prompt.

Expected: the screen from Task 5 on the phone, served from the Mac's Vite dev server on `192.168.20.126`. If `--host` is not accepted by this CLI version, set `build.devUrl` in `tauri.conf.json` to `http://192.168.20.126:1420` and run `npm run dev -- --host` in another shell.

- [ ] **Step 4: Run the three measurements and write them down**

With the host from Task 3 running on the Mac and the phone on the same Wi-Fi:

1. Connect to `192.168.20.126:7777`. Record: the three chats listed (experiment 1's second half).
2. Attach **echo**. Type on the phone's keyboard; note whether the echo feels immediate. Tap each quick key once; the host's stderr shows `write echo 1 bytes: 03` for Ctrl-C.
3. Attach **2 MB burst**. While it draws, press **Tap me** repeatedly. Record `drawn`, seconds, longest frame, and whether taps kept counting. Wait until `drawn` stops at `2,114,336`.
4. Attach **13 MB burst**. Same. Record the same four. Note whether iOS killed the app (it returns to the home screen) or the webview went white.
5. Lock the phone mid-13 MB-burst for 30 s, unlock, reopen the app. Record what the host's stderr printed (`gone` after the suspend) and that **Connect** works again. This is Review Focus 1 on real hardware.

Write the numbers into `~/purlis-research/spike/RESULTS.md` as you go, in this table:

```markdown
| Experiment | Result | Numbers |
|---|---|---|
| 1a cross-compile aarch64-apple-ios | pass / fail (crate, error) | |
| 1b phone lists the host's chats over Wi-Fi | pass / fail | |
| 2 echo keystroke feel | immediate / laggy | |
| 2 burst 2 MB | no freeze / froze / killed | drawn, s, longest frame ms, taps kept counting? |
| 2 burst 13 MB | no freeze / froze / killed | drawn, s, longest frame ms, taps kept counting? |
| 2 lock mid-burst then reconnect | pass / fail | host stderr lines |
```

- [ ] **Step 5: Commit**

```bash
cd ~/purlis-research/spike && git add -A && git commit -q -m "iOS build of the phone, local-network permission, and the measured burst results"
```

---

### Task 7: Experiment 3: a prompt pasted mid-turn into Claude Code, Codex and opencode

No code of ours decides anything. tmux runs each harness in a 120×40 PTY, sends a first prompt that keeps the agent busy for about half a minute, then sends *exactly* the bytes purlis's writer sends (`curation::bracketed`: `ESC[200~` text `ESC[201~`, then `\r`, in **one** write), and captures the screen every five seconds. A person reads the captures.

**Files:**
- Create: `~/purlis-research/spike/midturn/midturn.sh`
- Create: `~/purlis-research/spike/midturn/claude.log`, `codex.log`, `opencode.log` (captures)

**Interfaces:**
- Produces: three captures and, per harness, one of three verdicts: **queued** (the second prompt is acted on after the first turn ends), **interrupted** (it cut the first turn short), **lost** (it never appears or is mangled).

- [ ] **Step 1: Write the script**

`midturn/midturn.sh`:

```sh
#!/bin/sh
# usage: midturn.sh <name> <command...>
# Starts <command> in a tmux PTY, prompts it, pastes a second prompt mid-turn the way purlis
# does (one write: bracketed paste then Enter), and captures the screen every 5 s for a minute.
set -eu
name=$1; shift
here=$(cd "$(dirname "$0")" && pwd)
log="$here/$name.log"
: > "$log"
tmux kill-session -t midturn 2>/dev/null || true
tmux new-session -d -s midturn -x 120 -y 40 "cd $HOME/purlis-research/spike/midturn/scratch && $*"
sleep 12
tmux send-keys -t midturn -l 'Count from 1 to 25, one number per line, and pause about one second between numbers. Use no tools.'
tmux send-keys -t midturn Enter
sleep 8
printf '\n===== pasted mid-turn at t+8s =====\n' >> "$log"
tmux send-keys -t midturn -l "$(printf '\033[200~When the count is done, reply with the single word PINEAPPLE.\033[201~\r')"
i=0
while [ $i -lt 12 ]; do
  sleep 5; i=$((i+1))
  printf '\n===== t+%ss =====\n' $((8 + i*5)) >> "$log"
  tmux capture-pane -t midturn -p -S -200 >> "$log"
done
tmux send-keys -t midturn C-c
sleep 1
tmux kill-session -t midturn 2>/dev/null || true
echo "captured to $log"
```

```bash
chmod +x ~/purlis-research/spike/midturn/midturn.sh
mkdir -p ~/purlis-research/spike/midturn/scratch && cd ~/purlis-research/spike/midturn/scratch && git init -q
```

The `scratch` directory is an empty git repository so that each harness has a harmless cwd and never sees the clone.

- [ ] **Step 2: Run it against Claude Code and read the capture**

```bash
~/purlis-research/spike/midturn/midturn.sh claude claude
grep -n "PINEAPPLE\|25" ~/purlis-research/spike/midturn/claude.log | head
```

Expected if **queued**: a capture after the count reaches 25 shows the agent replying `PINEAPPLE`, and the earlier captures show Claude Code's queued-message indicator with the pasted text. If a capture shows a permission ask instead, the first prompt was ignored ("Use no tools"); re-run with the ask answered by hand in another terminal (`tmux attach -t midturn`).

- [ ] **Step 3: Codex and opencode**

```bash
~/purlis-research/spike/midturn/midturn.sh codex codex
~/purlis-research/spike/midturn/midturn.sh opencode opencode
```

Read each `.log` the same way. opencode's TUI may need a different first keystroke to focus the composer; if the first prompt never appears in the capture, `tmux attach -t midturn` once by hand to see what it wants, then adjust only the `sleep 12` or add a `tmux send-keys -t midturn Escape` before the first prompt, and say so in the findings.

- [ ] **Step 4: Record the verdicts and commit the captures**

Append to `~/purlis-research/spike/RESULTS.md`:

```markdown
| 3 Claude Code mid-turn paste | queued / interrupted / lost | capture: midturn/claude.log, t+… |
| 3 Codex mid-turn paste | queued / interrupted / lost | capture: midturn/codex.log, t+… |
| 3 opencode mid-turn paste | queued / interrupted / lost | capture: midturn/opencode.log, t+… |
```

```bash
cd ~/purlis-research/spike && git add midturn RESULTS.md && git commit -q -m "Mid-turn paste captures for Claude Code, Codex and opencode, with verdicts"
```

---

### Task 8: The findings, into the clone

**Files:**
- Create: `~/purlis-research/purlis/docs/superpowers/specs/2026-10-11-ios-phase-0-findings.md`
- Modify: `~/purlis-research/purlis/docs/superpowers/specs/2026-10-11-ios-mobile-design.md` (§7, the Phase 0 row gains a trailing sentence)
- Modify: `~/purlis-research/README.md` §10

**Interfaces:**
- Produces: the document the Phase 1 plan is written from.

- [ ] **Step 1: Write the findings note**

`docs/superpowers/specs/2026-10-11-ios-phase-0-findings.md`, filled from `~/purlis-research/spike/RESULTS.md`:

```markdown
# purlis on iPhone: Phase 0 findings

**Status:** the spike the spec's §7 row 0 asked for, run <date> on <iPhone model, iOS version> against
this Mac (macOS 26.2, Xcode 26.6, Rust 1.99.0) over Wi-Fi. The spike's code is throwaway and lives
outside this repository (`~/purlis-research/spike/`, no remote); nothing of it is proposed.

## 1. `session-protocol` + `snow` + tokio on iOS

- Cross-compile for `aarch64-apple-ios`: <pass / which crate failed and the error>.
- A Tauri 2 iOS app linked them and listed a host's chats over Wi-Fi through Noise XX: <pass / fail>.
- What had to change to get there: <nothing / the list>. Anything that would change a clone crate is listed here as a Phase 1 item, not done.

## 2. xterm.js in WKWebView

| Burst | Drawn | Seconds | Longest frame | Input stayed responsive |
|---|---|---|---|---|
| 2 MB (2,114,336 B) | | | | |
| 13 MB (13,082,454 B) | | | | |

Desktop reference (spec L3): longest frame 42 / 52 ms, no freeze.

**Decision for Phase 1:** <the terminal mirror stays in the level-2 screen / the level-2 screen is
timeline-only with "View on desktop", per the spec's rule>.

Locking the phone mid-burst and reopening: <what the host saw; reconnect worked / did not>.

## 3. A prompt pasted mid-turn

| Harness | Verdict | Evidence |
|---|---|---|
| Claude Code | queued / interrupted / lost | `midturn/claude.log` t+… |
| Codex | | |
| opencode | | |

**Consequence for §5.2:** <"a harness that is mid-turn queues it its own way" holds for … / for … the
phone must refuse `prompt` with `turn_running` at level 2 too, which needs the hook state>.

## 4. What Phase 1's plan must take from this

1. …
```

- [ ] **Step 2: Point the spec at it and update README §10**

In the spec's §7, append to the Phase 0 row's text: ` **Done: see `2026-10-11-ios-phase-0-findings.md`.**`

In `~/purlis-research/README.md` §10 "Where it is", add a row `| Phase 0 findings | docs/superpowers/specs/2026-10-11-ios-phase-0-findings.md on the branch; spike code in ~/purlis-research/spike/ (own git repo, no remote) |` and change the Process row's "Next" to: **Next: plan Phase 1 (`superpowers:writing-plans`) from the findings.**

- [ ] **Step 3: Commit in the clone, local only**

```bash
cd ~/purlis-research/purlis && git add docs/superpowers/specs/2026-10-11-ios-phase-0-findings.md docs/superpowers/specs/2026-10-11-ios-mobile-design.md && git commit -q -m "Design: Phase 0 spike findings (iOS build, WKWebView bursts, mid-turn paste)" && git log --oneline -1
```

Expected: one new commit on `ios-mobile-design`; `git status` clean; nothing pushed.
