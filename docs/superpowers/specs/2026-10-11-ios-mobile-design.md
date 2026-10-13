# purlis on iPhone: continue a chat from your phone

**Status:** design, agreed in a brainstorming session on 2026-10-11. It's exploratory: it's built
on a local branch, and nothing is pushed or proposed upstream. It is written to purlis's rules
(AGENTS.md) so that it could be proposed later. Each place that needs an operator ruling is listed
in §10.

**Concept (ADR 0072):** **Chat**. A paired phone is a second client of the chats on one machine.
The pairing itself is Machine state.

## 1. Goal

A developer walks away from the desk, opens purlis on the iPhone and sees every chat on that Mac
with its live state. They open one, read what the agent did, answer its "Allow `cargo test`?",
type "also add a test for expiry", and watch it carry on. It works for Claude Code, Codex and
opencode today, and gets richer as each harness gains ACP (level 3).

### Decided

| # | Decision |
|---|---|
| D1 | The phone app is **Tauri 2 for iOS**, with the Rust side linking purlis's own `session-protocol` crate |
| D2 | The phone's main job is **continuing the work**: open a running chat, read it, prompt it, answer its asks, stop it |
| D3 | A chat is shown **as a chat**: messages, tool cards, plan and ask buttons. This works only where the chat speaks ACP (level 3) |
| D4 | A chat that can't feed that view **degrades gracefully**: a timeline built from hook events, plus a mirror of its terminal. It is never hidden |
| D5 | The network is **direct first** (LAN or Tailscale), then a relay with push notifications in phase 3, over the same encrypted link |
| D6 | **Pair once, see all.** A paired phone sees every project, workspace and chat on that machine. Your own devices only, no sharing with other people |
| D7 | The phone **connects to a gateway inside the desktop app's process** now, behind the existing `SessionHost`/`ChatBoard` seam. The gateway moves into `purlisd` unchanged when ADR 0068's split lands |
| D8 | The phone **never offers an "allow always" choice**. It offers only *once* and *this session*. A choice that writes a lasting rule stays on the desktop |
| D9 | A chat's history on the phone goes back as far as the host still holds it (the ACP client's bounded buffer). No transcript is kept anywhere for this feature |
| D10 | The desktop gains an opt-in setting, **"Keep this Mac awake while chats are running"**, off by default |
| D11 | The lock screen says **"<workspace> · <chat> needs you"**, decrypted on the phone (phase 3) |
| D12 | The work happens on a local branch of `purlis/purlis` and is never pushed |

### Not in scope

- Running a harness on the phone. The agents always run on the desktop (or later on a runner).
- Starting new chats from the phone (`start`). It becomes a candidate in phase 4.
- Sharing a chat or a workspace with another person (the Team tier).
- Answering an ask that requests a secret. ADR 0068 as amended by FD-27 keeps those to the window.
- Changing a chat's terminal size from the phone (§5.4).
- Android, an iPad-specific layout, a PWA.

## 2. What exists and what doesn't (2026-10-11, main @ `03e3b5c4`)

| Piece | State | Where |
|---|---|---|
| Session host seam | Built, in the app's process | `app/src-tauri/src/host.rs` (`SessionHost`, `ChatBoard`) |
| Session protocol: list, attach (snapshot + live view), write, resize, answer, stop, start, subscribe(since) | Built and tested, not served to anyone outside the app | `crates/session-protocol` (tokio, Yamux, HMAC admission, N−1 compatibility) |
| Client scopes table | Built | `session-protocol::grants`. `remote-link` exists, and no build admits it until RR-13's handshake |
| One `Ask` model across Claude Code, Codex, opencode and ACP, each choice with a `ChoiceScope` of *once*, *session* or *always* | Built. opencode's "always" is read as *session*, because it lasts only for the rest of the session | `crates/purlis-core/src/harness/asked.rs` |
| A chat as `list` describes it: id, project, path hint, title, state. **No workspace, no level** | Built | `session-protocol::session::Chat` |
| ACP client (level 3): message text, tool calls, plan, usage, asks, elicitation as events | Built. opencode only for now, and refused in sandboxed projects (D-87h) | `crates/purlis-core/src/acp.rs` |
| Event log with subscribe-from-cursor | Built. Holds **a digest of tool arguments, never the arguments** (ruling O3) | `crates/purlis-core/src/eventlog.rs` |
| Link key (X25519), Noise `XX`, the two pinned records | **Decided, not yet written** | ADR 0078 §3; `docs/plane-format.md` rows `<config>/peers.json` and the link-key keyring item |
| `purlisd` as its own process | **Decided, not built** | ADR 0068 |
| A relay | **Decided, gated** (GT-CLOUD, CF-26, LW-1) | ADR 0078 §2 |

## 3. Architecture

```
 iPhone (Tauri 2 iOS)                          Desktop (purlis app process)
┌──────────────────────────────┐             ┌───────────────────────────────────────┐
│ React UI (WKWebView)         │             │ existing: SessionHost + ChatBoard      │
│  home · chat view ·          │             │   (host.rs) · ACP chats · Asks         │
│  timeline · xterm.js mirror  │             │              ▲                          │
│        ▲  Tauri commands     │             │              │ same traits              │
│ Rust: purlis-mobile-core     │             │ NEW purlis-core::remote (the gateway)  │
│  · session-protocol client ◀─┼── TCP ──────┼─▶ · Noise XX against pinned keys       │
│  · Noise XX (snow)           │  LAN or     │   · session_protocol serve(), scope    │
│  · link key in Keychain      │  Tailscale; │     remote-device                      │
└──────────────────────────────┘  relay in   │   · timeline adapter                   │
                                   phase 3   └───────────────────────────────────────┘
```

| Unit | Job | Depends on |
|---|---|---|
| `purlis-core::remote` | **As built (1B):** the handshake (`noise`), the proof (`pairing`) and the accept loop (`gateway`) are `session-protocol`'s, behind its `noise` feature; `purlis-core::remote` holds the link key (`linkkey`, Keyring), `peers.json` (`peers`), the pairing desk (`pairing`: codes, the QR payload, the window's answer, unpairing) and the audit (`audit`), plain sync state with no runtime. **As built (1C):** the app wires them (`app/src-tauri/src/remote/`): the `Host` adapter over `SessionHost`/`ChatBoard` (`board.rs`, one per link, since those traits are the app's), the level-2 timeline from the event log and the hooks' asks (`timeline.rs`), the gateway's `Pins`/`PairingDesk`/revocation adapters and the audit (`adapters.rs`), the listener's lifecycle and the window's commands (`gateway.rs`: primary IPv4 + configured addresses, never `0.0.0.0`), and keep-awake through a `caffeinate -w <pid>` / `systemd-inhibit` child (`awake.rs`); `machine.json` gains `gateway` and `keep_awake`, `Admitted::RemoteDevice` answers an ask as a human scope. Together: listens when the operator turns it on, runs Noise `XX`, refuses a key it has not pinned before reading any frame (except during a pairing window, §4.2), then serves the session protocol under `remote-device` | session-protocol, the pinned records, the link key |
| Timeline adapter (in `remote`) | Turns `acp::Event`, hook reports and the board's `Ask`s into one ordered stream per chat. Never reads terminal bytes | `acp`, the board, `harness::asked` |
| `session-protocol` additions | The `remote-device` scope, the `timeline` stream kind, the `prompt` command, and `workspace` and `level` on a listed chat (§5) | — |
| `crates/purlis-mobile-core` | The phone's Rust side: connection, reconnect from cursor, the link key in the iOS Keychain, a small set of Tauri commands. **As built (1E):** plain Rust with no Tauri and no `purlis-core`: `store` (memory, private files, the Keychain through `keyring`), `identity` (the link key minted once; the device's ULID and name), `hosts` (the pinned desktops by device id), `payload` (the scanned code), `pair` (Noise XX against the scanned key, the proof, the verdict), `link` (the protocol's client with the desktop's streams routed to whoever attached them, acknowledged as the app takes them), `session` (a desktop across reconnects: Connected / Reconnecting / Unreachable, the `subscribe(since)` cursor per desktop, what to attach again); every network function generic over the IO, tested against the protocol crate's own gateway in process. The Tauri commands are the app's (1F) | session-protocol (`noise`), `keyring`, `ulid` |
| `mobile/` | The Tauri 2 iOS app (`tauri ios init`) and its React UI. It reuses `app/src/theme` and the copy rules, and none of the desktop's components. **As built (1F):** `src-tauri` holds one `Phone` (the Keychain store on iOS and macOS, the file store elsewhere; a `Session` per paired desktop; the open views and timelines) behind the commands `pair`, `desktops`, `connect`, `reconnect_all`, `list`, `attach`/`ack`/`detach`, `attach_timeline`/`detach_timeline`, `write`, `prompt`, `answer`, `stop`, the renames, `unpair` and `forget_link_key`, emitting `view`, `timeline`, `standing` and `event`; `src/` is the lock, Pair (scan or paste), Home, Chat (the strip over an xterm mirror at the desktop's size, quick keys, composer, Stop; a shell tab gets the mirror and the keys), the ask sheet and Settings, with the pure parts (`home`, `timeline`, `lock`, `terminal`, `quickKeys`) tested as modules | purlis-mobile-core |

**The gateway sits in `purlis-core`, not in the app crate**, so the core stays free of Tauri and
the gateway moves with the host when `purlisd` splits out. **The phone never gets the UI RPC**: no
vault values, no settings writes, none of the app's own commands. It only gets the public,
versioned session protocol.

## 4. Pairing and security

### 4.1 Keys and stores

- **The desktop's link key** is ADR 0078's X25519 key, in the Keyring tier: the existing
  plane-format row *keyring item for the link key*. There is no second key type.
- **The phone's link key** is X25519 too, made on the phone and kept in the iOS Keychain as
  `WhenUnlockedThisDeviceOnly`. It is never in a backup.
- **The two pinned records are ADR 0078's** (§3, *"the same two pinned records, whatever route
  pairs them"*):
  - **On the desktop:** `<config>/peers.json` (Machine, device-bound), holding each paired
    device's id, name, pinned public key and pairing time. Its plane-format row widens from
    "On a runner" to "on any host with paired devices". **That row changes in
    `docs/plane-format.md` before any code writes the file** (AGENTS.md).
  - **On the phone:** the desktop's device id, pinned key and address candidates.
- The relay in phase 3 pairs into exactly these records, so a phone paired over Wi-Fi and later
  reached through the relay is one pairing (RR-2's rule, applied to devices).
- **The gateway's settings** (off by default; which interfaces it binds; its port) and **D10's
  keep-awake setting** are keys in `<config>/machine.json` (Machine, device-bound: they are
  facts about this Mac, not about a project, and they must not sync to another machine). That
  row's description widens in `docs/plane-format.md` before any code writes either key.
  Neither is a new store.

### 4.2 Pairing

1. **Settings → Devices → Pair a phone** (desktop window only). The window shows a QR code
   carrying the desktop's public link key, its address candidates and a **one-time code**: 5
   minutes, single use.
2. The phone scans it, connects and runs Noise `XX`. **Only while a code is live** does the
   gateway complete a handshake with a key it has not pinned, and such a link may carry one
   frame only: the pairing proof. Inside the encrypted channel the phone proves the code with
   an HMAC. A wrong proof, or anything but a proof, closes the link and the code is spent.
   Outside a pairing window an unpinned key is dropped before any frame is read.
3. **The window asks the person:** *"Pair 'Noro's iPhone'? Key fingerprint 3F2A…"*, with
   **Pair** and **Cancel**. Only the window can confirm, because an agent's word is never consent
   (ADR 0090). A photo of the QR code alone pairs nothing.
4. Both sides pin. From then on, every link is Noise `XX` checked against the pinned keys, with
   no code.

**Unpair** (desktop Settings, or the phone's own Settings) deletes the record and closes any live
link from that device at once.

### 4.3 The `remote-device` scope

A new `auth::Scope`, and a new column in `session-protocol::grants`. Three powers are new:
`Timeline` (attach a timeline), `Prompt`, and `AnswerAlways` (answer with a choice whose scope
is *always*). The last is checked the way `AnswerASecret` is today: `serve` asks the host
`choice_lasts(chat, ask, answer)` before the command reaches it, and a host that does not
say is taken to mean *yes*, so the refusal fails closed. The host answers from the choice's
`ChoiceScope`: *once* and *session* pass, *always* does not.

| Command | `remote-device` |
|---|---|
| `list`, `subscribe`, `unsubscribe` | yes |
| `attach`, `detach` (a view or a timeline) | yes |
| `prompt`, `write` | yes |
| `resize` | **no** (§5.4) |
| `answer` an ask, with a *once* or *this session* choice | yes |
| `answer` with a choice whose scope is *always* (writes a lasting rule) | **no** (D8) |
| `answer` an ask that elicits a secret | **no** (FD-27 item 1) |
| `stop` | yes |
| `start` | no (phase 4 candidate) |
| the UI RPC, vault values, settings writes, re-arming the kill switch | **never** |

The phone draws a refused choice as *"More options on your desktop"*, and an ask that elicits a
secret as *"Answer this on your desktop"* with no buttons.

### 4.4 Threats

| Threat | Answer |
|---|---|
| Someone on the network | Noise `XX` against pinned keys. The gateway is off until turned on, and by default binds only LAN and Tailscale interfaces, never `0.0.0.0` without asking |
| A chat reaching the gateway (an unsandboxed project can open a TCP connection) | It holds no pinned private key and is dropped before any frame. Pairing is confirmed only in the window, so a chat can't pair itself. A sandboxed chat's egress proxy doesn't allow the port at all |
| A stolen QR code | Single use, 5 minutes, and still needs the window's **Pair** |
| A lost phone | Unpair from the desktop. The phone app locks behind Face ID or the passcode on every open. The kill switch still stops everything, and the phone can't re-arm it |
| Raw `write` from a phone | It's the same power as typing at the desk. The desktop's guards and sandbox bound what the agent then does. The residual risk is a stolen, unlocked phone, until it is unpaired |

**Audit:** ADR 0075's store is not built yet. Until it is, pairing, unpairing, connecting,
answering and prompting from a device go to the event log as metadata (device id, chat, what
kind, which choice id), and move to the audit when it exists. Never the text: not a prompt's,
not a message's, not an answer's free text. The event log is on disk; D9 forbids any of that
text leaving the host's memory for this feature.

## 5. Protocol additions (session-protocol, a new minor version)

### 5.1 `timeline`, a stream kind beside `view`

It is opened with `attach_timeline {chat}`. The host opens a stream, writes a **bounded snapshot**
of what it still holds for that chat, then each item as it happens. It uses Yamux per-stream
credit, so a slow phone throttles only its own stream. A gap means re-attaching, as V76 already
rules for views. Every item carries `level`.

| Item | Level 3 (ACP) | Level 2 (hooks) |
|---|---|---|
| `turn` began / ended, with its stop reason | ✓ | ✓ |
| `message` (role, text chunks) | ✓ | — (no text without reading output) |
| `tool` (title, kind, status, duration) | ✓, with the agent's title | ✓, name, verdict and timing only (O3) |
| `plan` (steps) | ✓ | — |
| `ask` (id, summary, choices, `elicits_secret`) / `ask_closed` (who answered) | ✓ | ✓ |
| `usage` | ✓ | — |

As built (1A): each item is one flat JSON object tagged `item`, with `level` on every one; a
`message` chunk carries `done` on its last piece, `ask_closed` says `by` whom, and `caught_up`
ends the snapshot. `crates/session-protocol/src/timeline.rs` is the record.

Message text lives only in the host's memory, in the ACP client's bounded buffer
(`EVENTS_HELD`, `MOST_BYTES_HELD`). It is never written to the event log or to disk for this
feature (D9).

### 5.2 `prompt {chat, text}`

- **ACP chat:** `session/prompt`. If a turn is already running, it is refused with
  `turn_running`, and the phone offers **Stop** or waiting.
- **Terminal chat:** one bracketed paste plus Enter in a single write, through the writer that
  Smart close and curation already use. A harness that is mid-turn queues it its own way.
- **Shell tab:** refused, with `not_an_agent`. The phone offers raw `write` there (§6.1).

### 5.3 `answer`

Unchanged on the wire: an ask id and one of the source's own choice ids. Only the scope table
changes (§4.3).

### 5.4 No `resize` from the phone

A chat's terminal is one terminal. Resizing it to a phone's width would reflow the desktop's
pane. The phone draws the terminal at the desktop's size, zoomable and scrollable sideways.

### 5.5 `list` gains `workspace` and `level`

A listed chat gains two optional fields: `workspace` (its workspace's name, or absent for the
project's root; `Start` already carries the same) and `level` (1, 2 or 3, ADR 0073). Home
groups by the first and the chat screen picks its drawing by the second. A shell tab lists
with `level: 1`.

### 5.6 Compatibility

N−1 holds: a host without `timeline` or `prompt` refuses them as unknown, and the phone falls
back to the terminal view and raw `write`. A host that omits `workspace` and `level` lists its
chats under the project with no grouping, drawn as terminals. The crate's compatibility tests
gain all four.

## 6. The iOS app

### 6.1 Screens

1. **Pair** (first run): QR scan through Tauri's barcode-scanner plugin, then *"Confirm on your
   desktop…"*.
2. **Home:**
   - **Needs you** first, across every project, mirroring the title bar's queue.
   - Then desktops → projects → workspaces → chats, each chat with its state (`working`,
     `needs_you`, `idle`, `ended`). Several desktops may be paired.
3. **Chat**, drawn by the chat's level:
   - **ACP:** messages, tool cards, the plan as a checklist, and ask cards.
   - **Level 2:** the timeline strip above an xterm.js mirror at the desktop's size, and a
     quick-key bar: Esc ↑ ↓ Enter Tab y n Ctrl-C.
   - **Both:** a composer (Send is `prompt`) and **Stop** while a turn runs.
   - **Shell tab (level 1):** the terminal mirror and the quick-key bar only. Typed text goes
     by raw `write`; there is no composer, no timeline and no Stop. It is listed on Home under
     its workspace like any chat, so what the desktop shows the phone shows.
4. **Ask sheet:** the summary, then the choices in the source's order and words, minus any
   *always* choice (D8). It closes with *"Answered on the desktop"* when the answer came from
   there.
5. **Settings:** paired desktops (rename, unpair), the Face ID lock (on by default), connection
   details.

### 6.2 iOS facts the design accepts

- **Backgrounding.** iOS suspends the app within seconds and the link drops. On returning to the
  foreground the app reconnects, `subscribe(since)` from its cursor, and re-attaches what was
  open. **In phases 1–2 there are no alerts while the app is closed.** Push is phase 3, and
  nothing relies on background refresh.
- **Local network.** A LAN link asks iOS's local-network permission
  (`NSLocalNetworkUsageDescription`). Tailscale needs none.
- **Lock.** Tauri's biometric plugin, on every open and after 5 minutes in the background.

### 6.3 Disconnected

- A `Notice` reads *Connected*, *Reconnecting…* or *Desktop unreachable*, with **Retry** as its
  way out.
- While disconnected, the last-known list stays, marked stale.
- **The composer and the ask buttons are disabled. Nothing is queued to be sent later.** A
  prompt or an "Allow" that arrives an hour late is a different act from the one the person
  made.
- If an answer races the desktop's, the host's reply decides, and the phone says which one won.

### 6.4 Keep this Mac awake (D10)

A desktop setting, **"Keep this Mac awake while chats are running"**, off by default. While it
is on and at least one chat's run is `working` or `input-required`, the host holds a macOS
power assertion that prevents idle system sleep (the display may still sleep). It is released
the moment no chat qualifies, when the setting is turned off, and when the app quits. It is
a key in `<config>/machine.json` (§4.1). A Linux equivalent (systemd-inhibit) follows the
same rule.

## 7. Phases

| Phase | Delivers | Rough size |
|---|---|---|
| **0. Spike** (throwaway) | (1) `session-protocol` + `snow` + tokio built for `aarch64-apple-ios` in a Tauri 2 iOS shell, linking to a test host over Tailscale. (2) xterm.js in WKWebView through the spec's 2 MB and 13 MB bursts. (3) A mid-turn bracketed-paste prompt queued correctly by Claude Code, Codex and opencode. **If (2) fails, the level-2 screen is timeline-only with "View on desktop"** **Running: see `2026-10-11-ios-phase-0-findings.md`.** | 3–5 days |
| **1. The link, and continuing the work** | **1A, the protocol additions, is built:** `docs/superpowers/plans/2026-10-12-ios-phase-1a-protocol.md` (session-protocol 1.1). **1B, the link, the keys and the pairing, is built:** `docs/superpowers/plans/2026-10-12-ios-phase-1b-link-and-pairing.md`. **1C, the desktop's wiring, is built:** `docs/superpowers/plans/2026-10-12-ios-phase-1c-app-wiring.md` (the `Host` adapter, the timeline, the gateway's lifecycle and commands, keep-awake). **1D, Settings → Devices, is built:** `docs/superpowers/plans/2026-10-12-ios-phase-1d-settings-devices.md` (an in-repo QR encoder, the Devices page with Pair a phone, the window's Pair/Cancel on an offer, Unpair, the keep-awake setting, `remote_pair_stop`). **1E, the phone's Rust core, is built:** `docs/superpowers/plans/2026-10-12-ios-phase-1e-mobile-core.md` (`crates/purlis-mobile-core`: store, identity, hosts, payload, pair, link, session; 21 tests against a real gateway in process). **1F, the phone's screens, is built:** `docs/superpowers/plans/2026-10-12-ios-phase-1f-mobile-app.md` (`mobile/`: the Tauri 2 app over `purlis-mobile-core`; Pair, Home, the level-2 chat screen with the mirror and the ask sheet, the shell tab, Settings, the lock, the disconnected rules). **Phase 1 complete, unverified on a device:** the TestFlight checklist is the next step. Desktop: the gateway, pairing with the window's confirmation, `peers.json` (row first), `remote-device`, `timeline` for level 2 and asks, `prompt`, `write`, `answer`, `stop`, Settings → Devices, Keep this Mac awake. Phone: Pair, Home, the level-2 chat screen, the ask sheet, the lock, the disconnected rules. **Outcome:** on Wi-Fi or Tailscale, any chat can be steered from the phone | 4–6 engineer-weeks |
| **2. The chat view** | ACP items in `timeline`, and the phone's chat view. Its reach follows upstream: opencode now, Codex with HP-3, Claude Code with HP-4, sandboxed projects when D-87h is lifted | 2–3 weeks |
| **3. Relay and push** | A small Rust relay (both sides dial out over WebSocket; it forwards Noise ciphertext by device id). Pairing through it yields the same pinned records. APNs push with an encrypted payload, decrypted on the phone by a Notification Service Extension into "<workspace> · <chat> needs you" (D11) | 3–4 weeks |
| **4. With `purlisd`** | The gateway moves into the daemon, with no protocol change, so the phone keeps working after the window quits. `start` from the phone becomes a candidate | — |

Each phase that lands something a person would notice (Settings → Devices, the keep-awake
setting, the gateway, the phone app itself) adds `changes/<slug>.md` fragments, never a
CHANGELOG.md edit (AGENTS.md). Phase 0 is throwaway and adds none.

## 8. Testing

Each test follows the repo's own patterns. A test is trusted only once it has been seen to fail
for the right reason (AGENTS.md).

- **Gateway, in `purlis-core`, against `fake-harness`:**
  - pairing: an expired code, a reused code, an unconfirmed pairing, an unknown key, a revoked key mid-link; an unpinned key outside a pairing window is dropped before a frame, and inside one may send only the proof
  - every row of §4.3, the refusals especially: `resize`, `start`, an *always* choice, a secret ask, the UI RPC
  - `choice_lasts` fails closed: a host that does not implement it refuses every answer from `remote-device`
  - timeline items from a fake ACP agent and from recorded hook reports
  - `prompt` routing at each level, `turn_running`, `not_an_agent`
  - nothing of a message's, a prompt's or an answer's text in the event log
- **Protocol:** the crate's N−1 compatibility tests for `timeline`, `prompt`, `workspace` and `level`.
- **`purlis-mobile-core`:** integration tests against a real gateway in process, on the ordinary
  macOS and Linux CI. It's plain Rust, so no phone is needed.
- **UI:**
  - Vitest units.
  - Scenario tests run the mobile UI **as a desktop Tauri build** in WebdriverIO against the e2e
    app and `fake-harness`, because Tauri's WebDriver does not drive iOS.
  - The copy and colour guard tests apply to `mobile/` as to `app/`.
- **Build check:** `tauri ios build` on `macos-latest`.
- **On a real iPhone:** a short TestFlight checklist per build: pair, background and foreground,
  switching Wi-Fi ↔ cellular, answer, prompt, stop, unpair.
- **Security:** the gateway and pairing join M3's external review.

## 9. Budgets (ADR 0086: each names its job)

| # | What | Budget | Kind and job |
|---|---|---|---|
| R1 | phone attach to the first paint of a chat, on LAN | ≤ 1 s | CI relative (`bench`, phone UI as a desktop build); release absolute on a real phone |
| R2 | a phone keystroke to its echo on the desktop terminal, on LAN | ≤ 150 ms | as R1 |
| R3 | the gateway idle with one paired phone connected | ≤ 0.5% of a core, ≤ 1 wakeup/s | CI absolute (`stress`) |
| R4 | the gateway's native memory per connected phone | ≤ 4 MB | CI absolute (`stress`) |

## 10. Where upstream would need a ruling

None of these is needed to build on the local branch. Each would be needed to propose it.

1. **A device scope that answers asks.** V75 names only `local-ui` and `approval`, and FD-27
   item 1 leaves `remote-link`'s answer pending. This design adds `remote-device`, limited per §4.3.
2. **Remote links before the split.** The gateway serves from the app's process, ahead of ADR
   0068's `purlisd`.
3. **A native iOS app rather than the PWA** that LW-1/LW-19 assume. The case: it reuses the Rust
   protocol and Noise code, and a signed app avoids the code-integrity problem LW-19 exists for.
   This would be an ADR.
4. **`peers.json` on a desktop**, not only on a runner (the plane-format row's wording).
5. **The relay** waits on GT-CLOUD, which is the operator's gate. Phase 3 on the local branch is
   built and tested locally only.

## 11. Risks

| Risk | Mitigation, or what is left |
|---|---|
| Upstream moves about 100 commits a day, and the daemon split will reshape the code beside the gateway | The gateway touches only the host traits and the public protocol. Rebasing the branch is still a recurring cost |
| The level-2 experience is thin: a terminal at desktop width, and a sparse timeline (O3, no output reading) | Accepted (D4). The full chat view arrives with HP-3, HP-4 and D-87h, which are upstream work |
| A remote typing path into the machine | §4.4. Left: a stolen, unlocked, still-paired phone |
| Tauri on iOS is young; a Notification Service Extension means editing the generated Xcode project | Phase 0 tests the basics. Phase 3 carries the extension risk |
| A sleeping Mac is unreachable | D10's setting. Otherwise the phone says *Desktop unreachable* |
| Apple Developer Program ($99/year) | Optional in phase 1 (7-day sideloading), needed for TestFlight, and required for APNs in phase 3 |
