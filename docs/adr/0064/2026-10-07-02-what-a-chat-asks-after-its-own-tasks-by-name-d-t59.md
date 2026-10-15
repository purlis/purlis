# Amended 2026-10-07: what a chat asks after its own tasks, by name (D-T59-j8, #1441, #1442)

Under the operator's ruling V98b, decided at the assembly of train 59 under delegation and
flagged for the operator, on the rule above that there is no bare `purlis dispatch *`
(D-T59-18): each subcommand that #1441 and #1442 bring is its own pinned `allow` (`harness::claude::DISPATCH_TASK_ALLOW`), on the rules above (an
`allow` only, by its full name, for Claude Code only, and an operator's `ask` or `deny` still
wins). What each can reach is decided by the app from its own record of the chat that runs it,
never by what the command says:

- `Bash(purlis dispatch --wait *)`: the dispatch itself, waited on. It reaches what a dispatch
  reaches, which the dispatch grant consents to, and then waits for that one task's report.
  Matched only where the flag comes first; any other order is asked about.
- `Bash(purlis dispatch wait *)`: waits for the report of a task the chat itself dispatched. A
  number that is not such a task is refused.
- `Bash(purlis dispatch list)`: lists the tasks under the chat, by name and state. A task the
  person started from its tab is listed and no more.
- `Bash(purlis dispatch read *)`: reads what a task the chat itself dispatched said last and
  the commands it ran (#1981, #2084). A number that is not such a task is refused.
- `Bash(purlis dispatch cancel *)`: asks a task the chat itself dispatched for a short report.
  It ends no chat, and reaches no chat the caller did not dispatch, nor one the person
  started from its tab, nor one the person is stopping.
- `Bash(purlis dispatch tell *)`: leaves a follow-up for a task the chat itself dispatched,
  quoted as data on that task's next turn.
- `Bash(purlis dispatch note *)`: a task leaves a progress note for its own asking chat. It
  names no recipient.
- `Bash(purlis dispatch ask *)`: a task asks its own asking chat a question, and waits on it.
  It names no recipient, and a question for the person is never sent this way.
- `Bash(purlis dispatch answer *)`: answers a question a task the chat itself dispatched has
  open. With none open it is refused.

A subcommand added later still asks until it is ruled in here.
