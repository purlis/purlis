### Fixed

- **A chat no longer stops on a permission prompt for its own work.** A Claude Code chat now
  runs its own memory, todos and branch folder without Claude Code asking: the tools
  `todo_add`, `todo_done`, `memory_add`, `persona_remember`, `workspace_vision` and
  `workspace_section`; `purlis persona remember <its persona>` (`--shared` included) and
  `purlis workspace remember` and `note` with `-w <its workspace>`; `purlis ws todo` to list;
  and `purlis worktree add -w <its workspace>`, `update`, `push`, `done`, `abandon` and
  `list`. A line that names another persona or workspace is still asked about. purlis's reads (`recall`, `persona show` and
  `list`, `workspace list` and `current`, `docs show`, `news`, `theme check` and `list`,
  `dispatch list` with arguments) run unasked too, and `ask_operator` asks you its question
  once instead of twice. Anything that reaches past the chat's own work, such as `secret exec`,
  `forget`, `move-memory`, `save` or `clone`, is still asked about, and your own `ask` or `deny`
  rules still win (#3829).
