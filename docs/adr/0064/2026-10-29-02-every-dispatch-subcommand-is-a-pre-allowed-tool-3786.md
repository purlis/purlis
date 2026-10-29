# Amended 2026-10-29: every dispatch subcommand is a pre-allowed chat tool (#3786)

Ruled by the operator in the grill of 2026-10-29, beside #3785. Only `dispatch`,
`dispatch_report`, `dispatch_list` and `dispatch_read` had tools. Every other subcommand was a
shell command, which agents wrote with `| head`, `2>&1` or `&&` and so met the rider guard,
and which Codex has no per-command allow for.

- **Seven tools join them**, one per subcommand and named after it: `dispatch_wait`,
  `dispatch_tell`, `dispatch_answer`, `dispatch_cancel`, `dispatch_close`, `dispatch_note` and
  `dispatch_ask`. Each runs the same function as its `purlis dispatch` subcommand, so the tool
  and the command cannot answer one ask two ways, and each returns its sentence as the result.
  There is nothing to pipe.
- **Pre-allowed in a Claude Code chat, each by its full name.** `dispatch_wait` goes into
  `chattools::PRE_ALLOWED` with the other reads. The six that write go into
  `chattools::DISPATCH_TOOLS` beside `dispatch` and `dispatch_report`. Each reaches only what
  the app's record lets its caller reach. An operator's own `ask` or `deny` still wins.
- **A helper's call is still refused in front of each one.** The `pretooluse-dispatch` matcher
  names all eleven tools, and `dispatchguard::OUR_TOOLS` is held to it by a test.
- **One neutral tool set, served on every harness.** Claude Code is the one adapter with a
  per-tool allow today. On Codex and opencode the tools are served the same way, and the
  harness's own approval rules decide them.
- **Taught first.** The persona skill and the start-of-chat note name the tools first and the
  command second. The command stays for terminals and scripts, under #3785's rules.
