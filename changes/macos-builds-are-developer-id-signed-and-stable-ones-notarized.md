### Changed

- **macOS builds are Developer ID signed, and a stable release is notarized.** purlis, Inc now has
  an Apple Developer account, so a release made with its secrets is signed with its Developer ID
  and a stable one is also notarized and stapled, which lets a first install from a browser open
  with no extra step. The Developer ID key is used only on a GitHub-hosted runner, after the
  checks have passed, never on a machine that builds. A dev build is signed and not notarized, as
  it reaches people through the updater. A build with no Apple secrets is still ad-hoc signed
  (ADR 0042, amended 2026-11-25).
