### Added

- **Your Claude Code, Codex and opencode folders are now profiles, with nothing to do.** When
  purlis starts after the upgrade, each harness folder this Mac already uses becomes a profile in
  place, named after its harness (*Claude Code (this Mac)*): the same folder, so the same sign-in,
  settings and conversations. Nothing is copied or moved, nothing in the folder is changed, and
  you are not asked to sign in again. Every chat resumes its own conversation, and profiles in a
  project's `charter.local.toml` work as before. `purlis profile list` shows each adopted profile
  with its folder (#2033).
