# Amendment, 2026-11-25: the app and the operator hibernate a rested chat (SC-4, #1391)

Proposed by the SC-4 spec (#1391) from the operator's seven decisions of 2026-11-25. It is
recorded here ahead of the behaviour, which ships with the SC-4 tickets, and it takes effect when
the operator accepts this file. A sentence marked **(proposed)** goes beyond those seven decisions
and is one of the spec's rulings.

§5 says who hibernates and when, and left four things open: what idle means, how long, who else
may hibernate, and what waking brings back. Nothing produces the `idle` cause today, and no
production code writes `hibernated` at all (the window's in-memory crash path aside). This file
closes the four.

**"Idle" is the wrong word for the chat, so §5's idle chat is a *rested* chat.** ADR 0121's
**Idle** is a chat state that means *finished with work left behind*, and CONTEXT.md keeps *idle*
away from run states. The cause keeps its recorded name, `idle`, because §2's table already
spells it so. In prose a chat that may be hibernated by itself is **rested** (CONTEXT.md).

## What a rested chat is

A chat is rested when all of these hold:

- its run is `input-required` with reason `ready` or `turn-ended`, never `asked` (§5, unchanged:
  an ask would be lost with the process);
- its harness answers *yes* to `resumes_by_id` (§5, unchanged);
- **(proposed)** its harness reports its own work, so that "no own work" is something purlis can
  see. Today that is Claude Code's alone (ADR 0073, amended 2026-10-29, and ADR 0068, amended 2026-11-08). A chat on a harness that
  reports none is never judged rested;
- no **own work** is in flight: no shell, monitor, subagent, workflow, cron or wakeup
  (ADR 0068, amended 2026-11-08). A piece past that amendment's 60-minute lapse still counts, since
  the lapse changes only what the board shows;
- every task it dispatched has ended and reported (a hibernated task that has reported counts as
  ended), and it is not waiting on one;
- **(proposed)** if it is a dispatched task, its parent is not waiting on its report and its
  report is not owed;
- it is a **governed**, local chat (§7: an `observed` chat is never hibernated);
- nobody has **looked at it or typed in it** for the setting's time. A chat on screen in a
  focused window is being looked at, and its clock starts when the last pane showing it leaves the screen or its window loses focus.
  **(proposed)** The clock does not run while the machine sleeps, and a wake and the app's own
  start set it to now.

A chat waiting on its own work or on a dispatched task is never rested, however long it waits.

## Who hibernates, and when

- **The app, by itself,** with cause `idle`, for a rested chat, and **only while the device is
  over its hot-chat target** (ADR 0082, amended the same day). Under the target a rested chat
  stays hot, however long it rests. **The app, not `purlisd`, decides** (**proposed**): the dispatch
  ledger, the wake line, the reopen record and the hot count are the app's, so nothing hibernates
  while the app is closed. This replaces §2's `idle` row's *"the host's own act"* and §5's *"the
  host"*.
- **The operator,** from a chat's row or on several chats at once, with a new cause **`operator`**.
  It adds this row to §2's table:

| From | To | Cause | The fact |
|---|---|---|---|
| `input-required (ready or turn-ended)` | `hibernated` | `operator` | the operator hibernated it, from a human client scope. At a turn boundary and on a harness that resumes natively, as `idle` is; nothing else |

  It does not wait for the timer or the target, and it refuses a chat that is not at a turn
  boundary or whose harness cannot resume, saying why in words. **(proposed)** It asks first when
  it would end own work, naming each piece (ADR 0068's *restarts the person makes now end the
  work, and the chat is told*, applied); it refuses a chat with a dispatched task of its own still
  running, a task whose report is owed, and a task whose parent is waiting on its report, because
  that wait would never end.
  On a harness whose own work purlis cannot see, it warns that it cannot see what the chat left
  running.
- **A harness that cannot resume natively is never hibernated automatically** (§5, unchanged,
  X35), and its row says why.

## The setting

**`[chats] hibernate_after`** in a project's `charter.local.toml`, spelled as `autosave_after`
is (`"30m"`), **default 30 minutes**. **(proposed)** `"off"` turns automatic hibernation off, and a
value under 10 minutes, the old default, is refused with a sentence. Each chat's timer is its own
project's, and the target is the device's. §5's *"at least the harness's cache TTL (default 10
minutes)"* is replaced by this, and the 10 minutes stay as the floor.

## What waking brings back

- **Waking is a new run** (§5, unchanged): the hibernated run ends `completed | superseded` and
  ADR 0066's `wake` begins, `queued` and then admitted. The kill switch refuses it. A target
  never does (ADR 0082 §4). A wake is one run however many callers ask at once.
- **Three things wake a chat:** the operator's **Resume** on its pane or row, a message the
  operator types in it, and **a parent's `dispatch_tell` to a hibernated task**. In the last
  case the task is woken first and then reads the message, after its wake line if the line has
  anything to say. Resume here is the wake of the same chat, not §8's Resume from a record, which
  is a new chat.
- **A report does not wake a chat** (**proposed**). A chat with a dispatched task not ended and
  reported is never rested, so none is due to one the app hibernated. **A `dispatch_wait` on a
  hibernated task answers at once** that it is hibernated and that a tell wakes it, and a hibernated
  task is not an ended one: the dispatch ledger writes no *ended without a report* for it.
- **The wake line** (ADR 0068, amended 2026-11-08) is typed when the hibernated chat wakes, as
  that amendment says. Hibernating a rested chat ends no own work, so for an `idle` wake the line
  is usually empty and is not typed. After an `operator` Hibernate that ended own work on purpose,
  it names it.

## A restart keeps hibernated chats hibernated

§3 says *"a run already `hibernated` stays so"* and gives each live run a `reopen` run, staggered,
when it was visible, needed the operator or was mid-turn. **(proposed)** A chat hibernated by
`idle` or by `operator` is never given a `reopen`, visible or not, and wakes only by the operator
or a parent's `dispatch_tell`. A chat that was hot at the quit follows §3 as it is, and a hot chat
whose own work or dispatched task the restart ended is reopened with its wake line, as today.

§3's *"the rest appear hibernated ... and each gets its `reopen` run when it is shown or
prompted"* is the case of a chat that **quit** hibernated. **(proposed)** It is left to SC-20 as
it is, unless the operator rules to unify the two (spec ruling 3).

**The hibernated state is not durable yet.** §3's *"the hibernated state survives the relaunch in
the run journal"* describes this record's own journal (#2289), which is not written. SC-4 makes it
durable with a `hibernated` key, cause and time, on the chat's entry of `app/reopen.json`
(`docs/plane-format.md` first), written with the act's intent before the program ends. The host's
`Stopped::Hibernated` is only its in-memory attribution of that exit and writes no `run.ended`.

**A finished turn the person has not seen stays unseen** (**proposed**): §9's needs-you mark for a
`turn-ended` run is not cleared by hibernating, and the row still carries it beside Hibernated.

## What is kept

A hibernated chat keeps its row, its reopen entry and, when one could be taken and masked, a
**snapshot of its screen and scrollback** on disk, so opening it shows what it showed. A chat
hibernated by a quit, the grace period or a host crash has none, and a snapshot that cannot be taken
or masked never stops a hibernation: the chat is hibernated with none. The store, its tier and its
bound are in ADR 0082 (amended the same day) and `docs/plane-format.md`. The snapshot is a view;
the conversation itself is the harness's, resumed by id.
