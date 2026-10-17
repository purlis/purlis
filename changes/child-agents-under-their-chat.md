### Added

- **A chat's helpers show under it.** A Claude Code sub-agent or a Codex child is a helper of
  its chat: the chat's row in the explorer says how many it has, and unfolds to each with what
  it is doing. A Claude Code sub-agent or a Codex child reads working, then done when it
  finishes (a Codex chat's hooks gain `SubagentStop`, so Codex asks once more to trust them,
  #1086). The kill switch, or closing the chat, stops its
  sub-agents with it, and a stopped chat's sub-agents read as ended with it. A sub-agent's
  question is its chat's, and its own turns never move the chat out of running. opencode
  reports no sub-agents, and its chat says so (FD-18, #656).
