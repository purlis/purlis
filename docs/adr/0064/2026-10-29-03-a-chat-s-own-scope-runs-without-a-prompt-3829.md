# Amended 2026-10-29: a chat's own-scope purlis commands and tools run without a prompt (#3829)

Ruled by the operator on 2026-10-29, on the terms of the amendment for #3785 the same day:
**the person is asked only when something new reaches someone new; own-scope work runs without
a prompt.** Chats in a loop stopped on Claude Code's own permission prompt for purlis commands
and tools that only touch the chat's own work, because purlis never pre-allowed them. By
frequency: memory, todo and recall writes; branch-folder commands (the briefing tells every
piece chat to run `purlis worktree done`); read-only purlis commands; and `ask_operator`, asked
twice (the harness's prompt, then the question).

What a Claude Code chat the app starts is handed, each rule by name and never a bare
`purlis *` (`harness::claude`):

| Group | Rules | Where |
|---|---|---|
| Reads | `persona where`, `persona show *`, `persona list`, `persona recall *`, `recall *`, `workspace recall *`, `workspace list`, `workspace current`, `docs show *`, `news` (bare and with arguments), `theme check *`, `theme list` (bare and with arguments) | `READ_ALLOW` |
| The chat's own tasks | `dispatch list *`, beside the bare `dispatch list` | `DISPATCH_TASK_ALLOW` |
| Memory | `persona remember *` (`--shared` included, ruled by name), `workspace remember *`, `workspace note *` | `OWN_SCOPE_ALLOW` |
| Todos | `ws todo` and `workspace todo`: bare and `--done` (list), `done *` (close), and a quoted text (add) | `OWN_SCOPE_ALLOW` |
| Branch folder | `worktree add *`, `update` (bare and with `--continue`/`--abort`), `push` (bare and with arguments; the folder's own branch, never its base, ruled by name), `done`, `abandon *`, `list` (bare and with arguments) | `OWN_SCOPE_ALLOW` |
| Tools | `todo_add`, `todo_done`, `memory_add`, `persona_remember`, `workspace_section`, `workspace_vision`, `ask_operator` | `chattools::OWN_SCOPE_TOOLS` |

- **Still asked**: `secret exec`; `persona`/`workspace` `forget`, `archive`, `move-memory`,
  `edit-memory`; `save`; `clone`; `workspace reinit` and `edit`; `ws todo forget`, `reopen`
  and `promote` (which also keeps the consent rule `init` writes); `worktree remove`; the
  browser's upload and save; every word with no rule. An operator's or a project's `ask` or
  `deny` still wins over every allow here, as Claude Code merges them (measured on 2.1.283).
- **A grade's chat is handed none of the writes** (SK-16: it keeps its prompts, and its server
  answers the reads alone). The own-scope tools are a list of their own, apart from
  `chattools::PRE_ALLOWED`, for that reason.
- **A todo is added by a quoted text only** (`purlis ws todo "<text>"`): `ws todo` tells its
  verbs from text by the number of words, so a rule over every argument would also allow
  `forget` and `promote`. An unquoted one-word todo is asked about.
