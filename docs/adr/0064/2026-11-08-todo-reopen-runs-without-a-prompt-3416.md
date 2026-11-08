# Amended 2026-11-08: `todo_reopen` runs without a prompt (#3416)

**`todo_reopen` joins `chattools::OWN_SCOPE_TOOLS`, beside `todo_done`,** on the terms of the
amendment for #3829: it writes only the chat's own workspace's todos (a done one moves back to
`todos/`, and nothing is journalled; ADR 0106 as amended 2026-11-08) and reaches no one new.

- **Only the tool.** The command `ws todo reopen` still asks. #3829 keeps a command's todo verbs
  asked because Claude Code matches a rule before the shell strips quotes, which is a reason a
  tool's call does not have.
- **A grade's chat is handed none of it** (SK-16), like every own-scope tool.
- **An operator's or a project's `ask` or `deny` still wins**, as Claude Code merges them.
- **`todo_list` stays one tool.** Its done todos are an optional `done: true`, so
  `chattools::PRE_ALLOWED` did not change.
