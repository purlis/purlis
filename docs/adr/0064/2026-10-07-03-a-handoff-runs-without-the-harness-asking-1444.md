# Amended 2026-10-07: a handoff runs without the harness asking (#1444)

The handoff command is converted: a handoff is a dispatch in handoff mode (ADR 0090 as
amended, spec #1434 decisions 4 and 5), decided by the app as a task is. So a Claude Code chat
the app starts carries an `allow` for `purlis handoff` beside the dispatch's, on the same terms:
an `allow` only, for Claude Code only, an operator's `ask` or `deny` still wins, and **each
spelling by name, never a bare wildcard** (D-T59-18). The rules are the five flags a handoff's
line can start with (`--name`, `--report`, `--persona`, `--create`, `--vision`) and
`purlis handoff report`. This is V98b's ruling applied to the handoff spelling of the same act;
the note above that the handoff command keeps its prompt is superseded.

- **The spelling the handoff skill writes puts a flag first**:
  `purlis handoff --name "<task>" <workspace>`. A line that starts with the workspace matches
  none of these rules, so the harness asks about it by its own defaults, as it asks about any
  command it has no rule for. Nothing is refused for it.
- **`purlis init` no longer writes the `ask` rule** for `purlis handoff *`, and
  `purlis doctor --fix handoff-rule` removes the one it wrote, that exact rule, by name only.
  A project that still carries it keeps being asked by its harness, because an `ask` outranks
  an `allow`.
- **The handoff guard no longer refuses an unattended run or a spelling the rule did not
  match.** Both stood in for the prompt. The app refuses a chat nobody is at what no standing
  grant covers (#1446), and a spelling the guard reads word for word reaches the same decision.
- **What is not measured, and what purlis does about it (D-1444-14).** That a compound
  command holding a pre-allowed line is still asked about was measured for
  `purlis session record` and is carried over to the dispatch and the handoff, not measured
  again. A handoff adds a shape the record command never had: **every handoff is
  multi-line**, because its brief is a heredoc, so a command can stand on a line after it,
  and there is a recorded case (2026-09-11) of an `ask` rule that did not fire for a line
  after a heredoc. So purlis does not lean on the harness here. Where a tool call runs a
  dispatch or a handoff and any other command (joined, piped, on a line before or after the
  heredoc, or in a substitution), purlis's own `PreToolUse` hook answers `ask` for the call
  and names the other command, and it asks where it cannot read the call. This covers the
  dispatch rules above too. **Still unmeasured, and to be measured on real hardware before
  a release:** that Claude Code honours a hook's `ask` over its own `allow` rule for the
  same call; what it does with a redirection on the pre-allowed line; and whether a rider
  the shell reader does not see as a command (a function defined earlier in the call, an
  alias) is asked about.
