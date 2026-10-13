# iOS Phase 1F: The Phone App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `mobile/`, the Tauri 2 iOS app (iOS design §6): Pair (scan the desktop's code, or paste its text), Home (needs-you first, then each desktop's chats by workspace), the level-2 Chat screen (the timeline strip, an xterm mirror at the desktop's size, the quick-key bar, a composer whose Send is `prompt`, Stop), the Ask sheet (the source's choices minus any that lasts; "Answered on the desktop" when it closed elsewhere), Settings (paired desktops: rename, unpair, addresses; the Face ID lock; this phone's name), and the connection Notice (Connected / Reconnecting… / Desktop unreachable, with Retry) — over `purlis-mobile-core` through a thin set of Tauri commands.

**Architecture:** `mobile/` is the spike's shell grown up: its own npm package and its own Cargo workspace (`mobile/src-tauri`, excluded from the root workspace, with the spike's `nix` patch for iOS), React 19 in a WKWebView, `@xterm/xterm` with the WebGL addon. The Rust side (`mobile/src-tauri/src/`) holds one `Phone`: the stores (link key in the Keychain through `KeychainStore`, the rest in `FileStore` under the app's data dir), the identity, and a `Session` per paired desktop; each command is a few lines over `purlis-mobile-core`. Streams reach the webview as Tauri events (`view` chunks base64, `timeline` items JSON, `standing` changes, `event` for the subscription), and the webview acknowledges a view's bytes from xterm's write callback — the host's watermark follows the terminal, as the spike proved. Colours come from `app/src/theme` (the two theme JSONs and `drawIn`/`xtermTheme`), imported across the repo so there is one vocabulary; no desktop component is reused. Backgrounding: on `visibilitychange` to visible the app calls `reconnect_all` and re-attaches what each Chat screen holds; the lock (biometric plugin) runs on every open and after five minutes hidden.

**Tech Stack:** Tauri 2 (iOS), React 19, TypeScript, Vite, vitest + Testing Library (unit tests of the pure parts and the screens with the IPC mocked, as `app/` does), `@xterm/xterm` + `addon-fit` + `addon-webgl`, `tauri-plugin-barcode-scanner`, `tauri-plugin-biometric`; Rust: `tauri`, `purlis-mobile-core`, `tokio`, `base64`, `serde`.

**Spec:** `docs/superpowers/specs/2026-10-11-ios-mobile-design.md` §6.1 (the five screens), §6.2 (backgrounding, local network permission, the lock), §6.3 (disconnected: the Notice, stale list, nothing queued, the race's answer), §5.1–5.5 (what the timeline carries, `prompt`, no `resize`), D3/D4/D8, §8 (vitest units; scenario tests as a desktop build are a later phase; copy and colour guards apply).

## Global Constraints

- **No colour outside `app/src/theme/`**: the phone's stylesheet uses the same custom properties; xterm takes `xtermTheme(theme)`.
- **Copy per `docs/ui-copy.md`**: sentence case, *purlis* lowercase, no "please", no exclamation marks; a Notice has a way out (Retry).
- **Never an *always* choice on the phone (D8)**: the Ask sheet filters `Choice.lasts`; a secret-eliciting ask shows no choices, only "Answer this on the desktop".
- **Nothing queued while disconnected (§6.3)**: the composer, the quick keys, the ask buttons and Stop are disabled unless the desktop's standing is Connected; a refusal is shown, not retried.
- **No `resize` from the phone**: the terminal is created at the desktop's `cols × rows` and scrolls.
- **The sandbox builds neither `mobile/`'s Rust (the barcode and biometric plugin crates are not in the offline cache) nor its TypeScript (no `node_modules`)**: the Rust commands are written over the tested core and checked by a scratch `cargo check` with the two plugins' lines removed; the user's first `npm install && npm run tauri ios dev` outside is the build gate, with the spike's notes (`spike/RESULTS.md`: PATH in the Xcode build phase, `NSLocalNetworkUsageDescription`, a Personal Team).
- **Commit messages plain.**

## Review Focus

1. A view's acks: the webview acknowledges exactly the bytes xterm has written, per chunk, so the host's window never runs ahead of the screen and never starves — Task 2's `ack` command and Task 3's `draw` callback; the unit test pins that a chunk is acknowledged once and after the write.
2. Foregrounding mid-view: `reconnect_all` opens new links; a Chat screen that was open re-attaches (new view number, new epoch) and clears its terminal before the new snapshot — Task 3's `Chat` effect on `standing` → Connected.
3. Two desktops: Home groups by desktop and every command carries the desktop's device id; a chat id is unique per desktop, so keys are `${device}/${chat}` — Task 3's `home.ts` test.
4. An ask that closed under the sheet: the sheet shows "Answered on the desktop" (or "Answered on this phone" when it was) and closes on tap — Task 3's `AskSheet` test over timeline items.
5. The lock: on every launch and after five minutes hidden, before anything is drawn; a failed or cancelled check keeps the lock screen with Try again — Task 3's `Lock` (the plugin call is behind one function the test replaces).

---

### Task 0: The app's scaffold

**Files (create):** `mobile/package.json`, `mobile/vite.config.ts`, `mobile/tsconfig.json`, `mobile/tsconfig.node.json`, `mobile/index.html`, `mobile/.prettierrc.json` (copy of `app/`'s), `mobile/src/main.tsx`, `mobile/src/test-setup.ts`, `mobile/src/vite-env.d.ts`, `mobile/src-tauri/Cargo.toml` (its own `[workspace]`, members `.`; `[patch.crates-io] nix = { path = "../../spike-vendor/nix" }` is NOT used — instead copy the spike's vendored `nix` to `mobile/src-tauri/vendor/nix` and patch it there, since the spike is outside the repo), `mobile/src-tauri/build.rs`, `mobile/src-tauri/src/main.rs`, `mobile/src-tauri/tauri.conf.json` (identifier `dev.purlis.mobile`, product `purlis`, window title `purlis`, `bundle.icon` from `app/src-tauri/icons` copied into `mobile/src-tauri/icons`), `mobile/src-tauri/capabilities/default.json` (`core:default`, `barcode-scanner:allow-scan`, `barcode-scanner:allow-request-permissions`, `biometric:allow-authenticate`, `biometric:allow-status`), `mobile/README.md` (how to build: the spike's notes), root `Cargo.toml` (`exclude = ["mobile/src-tauri"]`), root `.gitignore` (`mobile/node_modules`, `mobile/dist`, `mobile/src-tauri/gen`, `mobile/src-tauri/target`).

`package.json`:

```json
{
  "name": "purlis-mobile",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "typecheck": "tsc --noEmit",
    "test": "vitest run",
    "format:check": "prettier --check .",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "~2.12.0",
    "@tauri-apps/plugin-barcode-scanner": "^2.3.0",
    "@tauri-apps/plugin-biometric": "^2.3.0",
    "@xterm/addon-fit": "^0.11.0",
    "@xterm/addon-webgl": "^0.19.0",
    "@xterm/xterm": "^6.0.0",
    "react": "^19.1.0",
    "react-dom": "^19.1.0"
  },
  "devDependencies": {
    "@tauri-apps/cli": "~2.12.0",
    "@testing-library/jest-dom": "^7.0.1",
    "@testing-library/react": "^16.3.3",
    "@testing-library/user-event": "^14.6.7",
    "@types/react": "^19.1.8",
    "@types/react-dom": "^19.1.6",
    "@vitejs/plugin-react": "^6.0.2",
    "jsdom": "^30.1.2",
    "prettier": "^3.9.9",
    "typescript": "~6.0.3",
    "vite": "^8.3.3",
    "vitest": "^5.0.3"
  }
}
```

`vite.config.ts` as the spike's plus `server.fs.allow: [".."]` (the theme is read from `../app/src/theme`) and `test: { environment: "jsdom", setupFiles: ["./src/test-setup.ts"] }`. `tsconfig.json` as the spike's with `"include": ["src", "../app/src/theme"]`.

`src-tauri/Cargo.toml`:

```toml
[workspace]

[package]
name = "purlis-mobile"
version = "0.1.0"
edition = "2024"
rust-version = "1.98"

[lib]
name = "purlis_mobile_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-barcode-scanner = "2"
tauri-plugin-biometric = "2"
purlis-mobile-core = { path = "../../crates/purlis-mobile-core" }
purlis-session-protocol = { path = "../../crates/session-protocol", features = ["noise"] }
tokio = { version = "1", features = ["io-util", "macros", "net", "rt-multi-thread", "sync", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
base64 = "0.23"

[patch.crates-io]
# nix 0.31.3 gates `KextControl = libc::SYSPROTO_CONTROL` on every Apple target, but libc has
# the constant for macOS only, so the crate does not build for aarch64-apple-ios (Phase 0).
nix = { path = "vendor/nix" }

[profile.release]
codegen-units = 1
lto = true
opt-level = 3
panic = "abort"
strip = true
```

- [ ] Commit: `Mobile: the app's scaffold`.

---

### Task 1: The Rust side: one Phone, a few commands

**Files:** `mobile/src-tauri/src/lib.rs`, `mobile/src-tauri/src/phone.rs`, `mobile/src-tauri/src/commands.rs`.

`phone.rs`:

```rust
pub struct Phone {
    secrets: Arc<dyn Store>,    // KeychainStore on a device; FileStore under the data dir in a simulator or a desktop build (TAURI_ENV? decide by target_os = "ios" → Keychain)
    records: Arc<dyn Store>,    // FileStore(app_data_dir)
    me: Identity,
    device: Device,
    sessions: Mutex<HashMap<String, Arc<Session>>>,   // by the desktop's device id
    /// Each open view: the task pumping chunks to the webview, and where its acks go.
    views: Mutex<HashMap<(String, String), ViewOpen>>, // (device, chat)
    timelines: Mutex<HashMap<(String, String), JoinHandle<()>>>,
}
struct ViewOpen { view: u32, task: JoinHandle<()>, acks: mpsc::UnboundedSender<usize> }
```

Commands (`commands.rs`, all `async`, each returning `Result<_, String>` with `Error`'s words):

- `phone() -> PhoneInfo { id, name, fingerprint }`; `rename_phone(name)`.
- `desktops() -> Vec<DesktopRow { device, name, addresses, standing: "connected" | "reconnecting" | "unreachable", why: Option<String> }>`.
- `pair(text: String) -> DesktopRow`: `Payload::from_qr_text`, `pair_tcp(me, device, payload, 10 s)`, `hosts::add` (name = the payload's host device until renamed… the desktop's name is not in the payload: name it by its first address until renamed), then `connect(device)`.
- `rename_desktop(device, name)`, `set_addresses(device, addresses)`, `unpair(device)` (drops the session and its views).
- `connect(device) -> DesktopRow` (makes the `Session` if none, `Session::connect`, then `subscribe` and spawn the events task that emits `event` and advances the cursor); `reconnect_all() -> Vec<DesktopRow>`.
- `list(device) -> Vec<Chat>` (the protocol's `Chat`, serialised as is).
- `attach(device, chat) -> Attached`: `link.attach` → spawn: for each chunk emit `view` `{ device, chat, view, kind: "snapshot" | "live", b64 }`; the chunk's bytes are acknowledged when the webview's `ack(device, chat, n)` says they were drawn (the core's `attach` answers the ack sender — see the 1E fix pass); on stream end emit `view-ended { device, chat, view }`.
- `ack(device, chat, n)`.
- `detach(device, chat)`.
- `attach_timeline(device, chat) -> TimelineAttached`: items emitted as `timeline { device, chat, item }`; `detach_timeline`.
- `write(device, chat, bytes: Vec<u8>)`, `prompt(device, chat, text)`, `answer(device, chat, ask, answer)`, `stop(device, chat)`.
- `standing(device) -> DesktopRow`.

Events the webview listens to: `view`, `view-ended`, `timeline`, `timeline-ended`, `event` (`{ device, event }`), `standing` (`{ device, standing, why }`).

`lib.rs` registers the two plugins and the commands; `run()` resolves the data dir (`app.path().app_data_dir()`), builds the `Phone` and manages it.

- [ ] Check: a scratch copy of `mobile/src-tauri` with the two plugin lines and deps removed, `cargo generate-lockfile --offline && cargo check --offline` in `$TMPDIR` (the plugins are the only thing the cache lacks). Commit: `Mobile: the phone's Rust side over purlis-mobile-core`.

---

### Task 2: The screens

**Files (create):** `mobile/src/App.tsx` (the shell: lock → pair-if-no-desktop → home/chat/settings; the Notice), `mobile/src/ipc.ts` (typed `invoke`/`listen` wrappers), `mobile/src/theme.ts` (draws `app/src/theme`'s default by `prefers-color-scheme`), `mobile/src/Lock.tsx` (+ `lock.ts`: when to lock), `mobile/src/Pair.tsx` (scan via the barcode plugin; a Paste field; "Confirm on your desktop…" while pairing), `mobile/src/Home.tsx` (+ `home.ts`: the grouping — needs-you across desktops first, then desktop → workspace → chats with state words), `mobile/src/Chat.tsx` (+ `terminal.ts`: xterm at cols×rows, WebGL, write + ack; `quickKeys.ts`), `mobile/src/Timeline.tsx` (+ `timeline.ts`: reduce items to the strip's rows and the open asks), `mobile/src/AskSheet.tsx`, `mobile/src/Composer.tsx`, `mobile/src/Settings.tsx`, `mobile/src/Notice.tsx`, `mobile/src/App.css`.

Copy, in the window's voice: Home title "purlis"; empty Home "No chats are open on {desktop}."; Notice "Connected" / "Reconnecting…" / "{desktop} is unreachable" with Retry; Pair "Scan the code on your desktop" / "Or paste the pairing text" / "Confirm on your desktop…" / "The desktop refused the pairing: …"; Ask sheet "Answered on the desktop" / "Answered on this phone" / "This ask wants a secret. Answer it on the desktop."; Chat "Stop" (ends the chat; the spec's §6.1 word), composer placeholder "Message"; Settings "Paired desktops", "Unpair", "Rename", "Also reach it at", "Lock with Face ID", "This phone's name".

Tests (vitest, pure modules and screens with `mockIPC`): `home.test.ts` (grouping and order; two desktops), `timeline.test.ts` (items → rows; open asks; a lasting choice left out; AskClosed → who), `lock.test.ts` (on open; after 5 min hidden; not before), `terminal.test.ts` (ack once per chunk after write, through a fake terminal), `Pair.test.tsx` (paste → `pair` → home), `Chat.test.tsx` (disabled while reconnecting; Send is `prompt`; Stop is `stop`; quick key is `write`), `AskSheet.test.tsx`.

- [ ] Commit: `Mobile: the screens`.

---

### Task 3: Docs and the dossier

- `mobile/README.md` (build and run on a phone, from the spike's notes); spec §3 row for `mobile/` and §7 row: "**1F, the phone's screens, is built** … Phase 1 complete, unverified on a device: the TestFlight checklist is the next step"; `~/purlis-research/README.md` §10; `changes/a-phone-app-steers-your-chats.md` (### Added).

- [ ] Commit: `Design: Phase 1F built`.

## Self-review (done while writing)

- **Spec coverage:** §6.1's five screens → Task 2 (Chat at level 2 and the shell tab: level 1 has no composer, no timeline, no Stop; ACP/level 3 is Phase 2 and is drawn as level 2 until then); §6.2 → `reconnect_all` on foreground and the lock; §6.3 → the Notice and the disabled controls; D8 → the sheet's filter; §5.4 → no resize; §8 → vitest units (scenario tests are later).
- **Placeholders:** the screens' code is not reproduced in the plan (it would be the files themselves); each file is named with what it holds, and the copy and the tests are listed. The executor writes them from the spike's `App.tsx`, the core's API and `app/src`'s conventions.
- **Review Focus:** each line names its owner.
