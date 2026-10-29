### Fixed

- **A chat no longer stops on a permission prompt for its own work.** A Claude Code chat now
  runs its own memory, todos and branch folder without Claude Code asking: `purlis persona
  remember` (`--shared` included), `purlis workspace remember` and `note`, `purlis ws todo` to
  add, close or list, and `purlis worktree add`, `update`, `push`, `done`, `abandon` and
  `list`, with their tools (`todo_add`, `todo_done`, `memory_add`, `persona_remember`,
  `workspace_vision`, `workspace_section`). purlis's reads (`recall`, `persona show` and
  `list`, `workspace list` and `current`, `docs show`, `news`, `theme check` and `list`,
  `dispatch list` with arguments) run unasked too, and `ask_operator` asks you its question
  once instead of twice. Anything that reaches past the chat's own work, such as `secret exec`,
  `forget`, `move-memory`, `save` or `clone`, is still asked about, and your own `ask` or `deny`
  rules still win (#3829).
