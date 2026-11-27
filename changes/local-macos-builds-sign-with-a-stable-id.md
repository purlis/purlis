### Changed

- **A local macOS build no longer asks for your Keychain password after every rebuild.** Cargo
  now signs each binary it runs with one stable identity (an Apple Development certificate, or a
  self-signed `purlis dev`), so the Keychain sees the same app across rebuilds. CI and builds
  without an identity are unchanged. Set up in `docs/development.md`, "Local macOS signing".
