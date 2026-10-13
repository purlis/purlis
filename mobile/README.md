# purlis on your phone

The Tauri 2 iOS app (iOS design, `docs/superpowers/specs/2026-10-11-ios-mobile-design.md`):
pair with your desktop once, then open, read, prompt, answer and stop its chats from your
phone over your own network. Everything of substance is `crates/purlis-mobile-core`; this
directory is the screens (`src/`) and a thin set of Tauri commands over the core
(`src-tauri/src/`).

## Building

```sh
cd mobile
npm install
npm run typecheck && npm test
```

A desktop build runs the same screens in a window, for development. It has no camera and no
lock (those plugins are the phone's), so it pairs from pasted text and opens unlocked, and it
keeps the link key in a private file rather than the Keychain:

```sh
npm run tauri dev
```

### On an iPhone

The first time:

1. `npm run tauri ios init` generates `src-tauri/gen/apple` (not committed).
2. Declare local networking once:
   ```sh
   /usr/libexec/PlistBuddy -c "Add :NSLocalNetworkUsageDescription string 'purlis connects to purlis on your Mac over your Wi-Fi.'" \
     src-tauri/gen/apple/purlis-mobile_iOS/Info.plist
   /usr/libexec/PlistBuddy -c "Add :NSCameraUsageDescription string 'purlis scans the pairing code on your Mac.'" \
     src-tauri/gen/apple/purlis-mobile_iOS/Info.plist
   /usr/libexec/PlistBuddy -c "Add :NSFaceIDUsageDescription string 'purlis locks itself behind Face ID.'" \
     src-tauri/gen/apple/purlis-mobile_iOS/Info.plist
   ```
   (The target is named after the package, `purlis-mobile_iOS`.)
3. `nix` 0.31.3 does not build for `aarch64-apple-ios` (Phase 0's finding). `src-tauri/Cargo.toml`
   already patches it with the spike's corrected copy at `~/purlis-research/spike/vendor/nix`,
   by relative path; keep the spike beside this clone, or copy that directory to
   `src-tauri/vendor/nix` and point the patch at it.
4. Xcode runs its build scripts with `/usr/bin:/bin` only, so the "Build Rust Code" phase
   cannot find `npm` or `cargo`: put `export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"`
   first in that script (`gen/apple/project.yml` and the `shellScript` in
   `gen/apple/purlis-mobile.xcodeproj/project.pbxproj`), or build from the terminal with
   `npm run tauri ios dev` without `--open`.
5. Signing: Xcode → Settings → Accounts → add your Apple ID, then pick its Personal Team on the
   `purlis-mobile_iOS` target, or set `DEVELOPMENT_TEAM` under `settingGroups.app.base` in
   `gen/apple/project.yml` and in both build configurations of `project.pbxproj` (the team id
   is the ten characters Xcode shows beside the team's name). A sideloaded build lasts seven
   days.

Then `npm run tauri ios dev --open` (or `tauri ios build`). On the phone, allow local
networking when iOS asks, or turn it on by hand under Settings → Privacy → Local Network.

## Pairing

On the desktop: Settings › You › Devices → Phone access on → Pair a phone. On the phone: scan
the code, or paste the text that Copy pairing text put on the desktop's clipboard. Confirm the
phone on the desktop by its name and key fingerprint. From then on the phone links with its
key alone; unpair from either side.
