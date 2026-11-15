### Added

- **A chat's Chat log says what purlis can't tell about its harness.** On opencode, and on Codex
  for now, the harness doesn't report what its own check blocks, so the log says it can't tell
  rather than showing nothing. Codex reports the same mode for every chat, so a Codex chat's
  log says purlis can't tell its mode, and never shows a mode for it.

### Changed

- **Unverified blocks take one line in the Chat log.** A chat's unverified blocks collapse into
  one line with their count, such as *12 unverified*. Its detail lists the latest 10. Each one
  says only who blocked a call and when, in purlis's own words: never the tool, the target or
  the reason that came from the chat. Blocks purlis can't tell more about get a line of their
  own.
