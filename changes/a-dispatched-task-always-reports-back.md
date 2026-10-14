### Changed

- **A follow-up wakes an idle task, on every harness.** `purlis dispatch tell` to a task at
  rest starts its turn at once, now on opencode too, as it did on Claude Code and Codex. A task
  mid-turn reads it when that turn ends, as before. A stop or a cancel of an opencode task
  mid-turn asks it for its report when that turn ends, since opencode ends a turn on a second
  Escape (#1981).
- **The chat that asked always hears back.** A task whose turn ends without its report is
  asked for it once, in purlis's own line. If the turn that starts also ends without one, purlis sends the chat that asked the task's last message, marked as **a report
  purlis made from its last message** and quoted as data. You see it on the chat that asked,
  and on the task's finished row. The task stays open, and a report it sends later reaches the
  chat that asked too and takes this one's place on its record (#1981).
