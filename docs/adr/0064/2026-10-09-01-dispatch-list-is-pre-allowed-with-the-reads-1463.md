# Amended 2026-10-09: `dispatch_list` is pre-allowed with the reads (#1463)

Decided in implementation under delegation (D-m62-core-1) and flagged for the operator:
`mcp__purlis__dispatch_list` joins `chattools::PRE_ALLOWED`, on the rules above (an `allow`
only, by its full name, for Claude Code only, and an operator's `ask` or `deny` still wins).

- **It is the same read as a command line that is already allowed.** The tool asks the app
  that started the chat for the list `purlis dispatch list` prints, and
  `Bash(purlis dispatch list)` runs without asking since D-T59-j8 (above). Asking at one
  entrance and not the other protected nothing.
- **It answers nothing beyond the chat's own tasks.** The app reads the list from its own
  record of the chat that asks, bound to the sender: the tasks that chat dispatched, and the
  ones the person started from its tab, and no other chat's. Each row carries the task's chat
  number, name, persona, the workspace it works in, its state, how long ago it started, and
  the branch purlis cut for it with how that branch stands: the app's own record, never a
  brief or a transcript. It takes no argument, so it cannot name another chat.
- **A helper's call is still refused in front of it.** The dispatch hook's matcher names the
  tool, and a sub-agent's call of it is refused as before: the pre-allow stands in for the
  harness's prompt and nothing else.
