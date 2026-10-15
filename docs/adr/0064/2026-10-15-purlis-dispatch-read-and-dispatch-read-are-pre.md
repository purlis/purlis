# Amended 2026-10-15: `purlis dispatch read` and `dispatch_read` are pre-allowed (#2084)

Decided in implementation under delegation and ruled by the operator, 2026-10-15, on the rules of the
amendment below: `Bash(purlis dispatch read *)` joins
`harness::claude::DISPATCH_TASK_ALLOW`, and the matching chat tool `mcp__purlis__dispatch_read`
joins `chattools::PRE_ALLOWED` (an `allow` only, by its full name, for Claude Code only, and an
operator's `ask` or `deny` still wins).

- **It only reads, and only what the chat's own record lets it.** The app answers from its
  record of whose task the chat named is: a task the chat dispatched itself, read while it runs
  and after it closed, and every other chat refused in the sentence a wait is refused in. It
  starts, ends and sends nothing.
- **What it prints is the task's words as data.** The last messages and commands it returns
  are quoted under purlis's sentence and masked as tool output is (ADR 0090, 2026-10-14).
- **A helper's call is still refused in front of it.** The dispatch hook's matcher names the
  tool, and the command is refused to a sub-agent as every `purlis dispatch` word is.
