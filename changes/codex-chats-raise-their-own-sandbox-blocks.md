### Added

- **A sandboxed Codex chat raises its own blocks.** A failed lookup, a refused socket or a
  refusal a command printed in a Codex chat now gets a Notice, as it does in Claude Code and
  opencode chats. Codex is armed with one more hook after each shell command, so the first Codex
  chat after the update asks you once to trust purlis's hooks again (#1663).

