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
| Own memory | `persona remember <own persona> *` (`--shared` included, ruled by name), `workspace remember -w <own workspace> *`, `workspace note -w <own workspace> *` | `own_scope_rules`, written per chat |
| Todos | `ws todo` and `workspace todo`, bare and `--done`: the whole line, which lists | `OWN_SCOPE_ALLOW` |
| Branch folder | `worktree add -w <own workspace> *` (per chat); `update` (bare and with `--continue`/`--abort`), `push` (bare and with arguments; the folder's own branch, never its base, ruled by name), `done`, `abandon *`, `list` (bare and with arguments) | `OWN_SCOPE_ALLOW` |
| Tools | `todo_add`, `todo_done`, `memory_add`, `persona_remember`, `workspace_section`, `workspace_vision`, `ask_operator` | `chattools::OWN_SCOPE_TOOLS` |

- **Still asked**: `secret exec`; `persona`/`workspace` `forget`, `archive`, `move-memory`,
  `edit-memory`; `save`; `clone`; `workspace reinit` and `edit`; `ws todo forget`, `reopen`
  and `promote` (which also keeps the consent rule `init` writes); `worktree remove`; the
  browser's upload and save; every word with no rule. An operator's or a project's `ask` or
  `deny` still wins over every allow here, as Claude Code merges them (measured on 2.1.283).
- **A grade's chat is handed none of the writes** (SK-16: it keeps its prompts, and its server
  answers the reads alone). The own-scope tools are a list of their own, apart from
  `chattools::PRE_ALLOWED`, for that reason.

## On review of #3841 (the dispatcher's decisions, 2026-10-29)

- **No rule adds or closes a todo through the command.** Claude Code matches a rule against
  the line as written, and the shell strips its quotes afterwards, so `purlis ws todo "forget"
  <slug>` met the rule written for a quoted text and ran `forget`. A chat adds and closes a
  todo through the `todo_add` and `todo_done` tools, which run unasked; the briefing names
  `todo_add` first and the command second. The command's add, close and every other verb ask.
  Only the whole bare line and `--done`, which list, are allowed; no quoted verb reaches them.
- **Own scope is the chat's OWN persona and workspace.** `persona remember *` also matched
  `persona remember <another persona> …`, and `workspace remember|note *` matched `-w
  <another workspace>`. The app writes these rules per chat, naming the persona it runs as and
  the workspace its folder is in: `persona remember <own> *` (`remember` takes two words at
  most, so no other persona can follow), and `workspace remember|note -w <own> *` and
  `worktree add -w <own> *` with the workspace first (the command takes `-w` once, so a second
  one on the line is refused by the command). A chat on no persona, or at the plane root, gets
  no rule of that kind; a name with any character but ASCII letters, digits, `-`, `_` and `.`
  gets none, so no glob character from a name can widen a rule. The unnamed spellings
  (`persona remember "<text>"`, `workspace remember "<text>"`) ask; `memory_add` and
  `persona_remember` run unasked.
- **`worktree add` is written per chat too**, on the same terms: it names a workspace with
  `-w`, and a folder in another workspace is something new reaching someone new.
