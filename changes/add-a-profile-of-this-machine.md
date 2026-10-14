### Added

- **Add a profile from the window, or with `purlis profile create`.** Settings › You › Profiles
  makes a profile of Claude Code, Codex or opencode that every project on this machine can use:
  a folder of its own in purlis's data home, its settings imported from this machine's harness
  settings and never its sign-in, and a chat on it runs its harness in that folder. Each profile
  says when its settings were imported and offers Import again (`purlis profile import`).
  Remove names the conversations that go and asks first (`purlis profile remove --yes`); a
  harness's own folder adopted as a profile is only forgotten, never deleted (#2034).
