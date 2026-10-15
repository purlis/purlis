# Amended 2026-10-15: printed text is never a block (#1972)

A chat ran `sed -n` over a test file that quotes `dial unix …: connect: operation not
permitted`. A socket block and a Notice came up on its tab, and the chat was told to wait,
although nothing was refused. The operator's ticket: a block is raised only for a refusal the
command itself met, "the failed command's own error, the sandbox's own report, or purlis's
proxy. Text a command merely printed never raises one."

This replaces D-sock-5's sentence that what a command that succeeded printed "raises a block
that names no socket". One reader (`sandboxblock::detect_with_targets`) serves every harness:

- **A failed command's error** (Claude Code's `PostToolUseFailure`, opencode's failed command)
  and **the sandbox's own report** are read as before.
- **A result with no failure beside it** says nothing of how each program of its line ended: a
  pipe drops a status (`gh pr list 2>&1 | head`), and Codex says nothing of it at all. Its line
  is the one signal left, read the way the certificate check's rule above already reads it. A
  line whose every program only reads or prints (`cat`, `sed -n`, `grep`, `git show`, also
  inside a string a shell is handed) merely printed what it handed back, and raises no refusal
  of the network from it, on either stream or as one string. Any other line may have met the
  refusal its programs said, and is read as before, naming no socket or service from it. This
  reading errs toward a program: a line too big for the guards, one that does not tokenize, a
  string a shell is handed nested past what is read, and a reader that can start a program of
  its own (`find` with `-exec`, `-execdir`, `-ok` or `-okdir`; `awk`; `sed` with any script but
  lines to print or delete by number) each count as running one. The certificate check keeps
  its stricter naming rule.
- **A background command's output read back** later with the harness's file tool has no line
  beside it and raises nothing. Polled with the harness's own tool (`BashOutput`), one that
  ended failed (`status` `failed`, or a non-zero `exitCode`) is read as a failed command, and
  still shown as a quiet line read back; one that did not fail raises nothing.
- **A program's own refusal of a path on standard error** is read whatever the line runs: a
  reader can be refused a path itself (`tee` writing one), and standard error is its own
  error stream, not what it printed.
- **A container client's line** names its socket as before.

purlis's own proxy tells the app each host it refused, as before.

**Why the line, and not only the harness's exit status.** The live cases this ADR was written
from are failures a pipe hid (`docker run … 2>&1 | tail`, `gh pr list … 2>&1 | head`, a
brokered run's `2>&1 | tail`), and a Codex chat's result never says how a command ended. Reading
only a failure would leave each of them with no Notice. What the ticket names, a file printed,
is a line of readers, and a line of readers raises nothing whatever it printed.
