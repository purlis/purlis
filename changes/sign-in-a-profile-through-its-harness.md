### Added

- **Sign in a profile through its harness's own sign-in.** A profile made in Settings › You ›
  Profiles says how it signs in, on this Mac, and whether its harness says it is signed in.
  **Sign in** opens a shell tab running Claude Code's, Codex's or opencode's own sign-in with
  the profile's folder set, and `purlis profile sign-in <name>` does the same in your terminal
  (`--check` says whether it is signed in). An opencode profile is signed in to ChatGPT or
  GitHub Copilot, never a Claude plan. purlis reads none of it: the harness keeps its sign-in in
  the profile's folder. A chat on a profile its harness says is not signed in is refused before
  it starts, and one Ask, Sign in <profile>, waits in the Inbox until it is (#2035).
