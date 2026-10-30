# Amended (2026-10-30): a long-running task, and a reported task that still runs

Ruled by the operator on 2026-10-29 (#3897, #3899, #3905). A coordinator ran builder tasks that
worked through ticket after ticket and sent progress with `note`. purlis treated each one as a
single job with a single answer. When a builder's turn ended twice without a report, purlis
made one from its last message, and the coordinator could then no longer tell it anything or
cancel it, though it was still committing.

- **"Reported" does not mean "unreachable" while the program runs** (#3899). A task whose
  report was made from its last message, or came out `blocked`, owes one more report
  (`Owed::Again`, #1768) and keeps its program. While that program runs, it is **reported,
  still running**: that is its state in `dispatch list`, in the open-tasks roster and in the
  awareness lines. In this state `tell`, `answer`, `note` and `ask` all work, and `cancel` too.
  A cancel asks for its one more report, which comes out `cancelled`. If the cancel's turn
  ends without one, the earlier report stands and the program is ended, as at any report
  (#1485). `close` ends the program as before. A task whose one report was sent (`done` or
  `failed`) is ending at that report and is not in this state. Once the program has ended,
  the "has finished" refusal stands.
- **A long-running task: `--long`** (#3905). `purlis dispatch --long --name …`, or `long: true`
  on the `dispatch` tool, starts a task that does many pieces of work and reports progress with
  `note` as it goes. Its notes are its heartbeat. purlis never asks it for a report and never
  makes one from its last message: a turn that ends without a report is just a turn. It ends
  when it reports `done` or `failed`, or when it is cancelled or stopped. A `blocked` report
  leaves it open, as it leaves any task open. Going quiet is still noted, as for any task.
  - The app records the mode on the task's lineage record (`long` beside `report` and `mode`),
    so it survives a restart. Nothing a task sends can set it.
  - Purlis adds one line of its own under the task's stamp. The line says the task is
    long-running, that `dispatch_note` is its heartbeat, and that it reports once when all the
    work is done.
  - `dispatch list` says "long-running" and shows the task's last note with its age, quoted as
    data. The roster says "long-running" beside the state. It carries no task content, so the
    note stays in the list.
  - **Pre-allowed in a Claude Code chat**: `Bash(purlis dispatch --long *)` joins the
    flag-first dispatch spellings of ADR 0064 (V98b), and the `long` field rides the `dispatch`
    tool, which is already pre-allowed. Consent is still the dispatch grant.
- **Plain dispatches are unchanged.** A task without `--long` is one job with one answer: it is
  asked for its report once, and then purlis makes one from its last message (#1981).
