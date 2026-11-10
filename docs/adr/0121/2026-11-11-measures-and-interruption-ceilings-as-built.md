# Amendment, 2026-11-11: the week's measures and the interruption ceilings, as built

Built for #6021 from [ADR 0121](../0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)
decision 18, taking over #1052 (the median time to answer) and #1079 (an interrupt budget for
every flow). What the decision leaves open, as built:

- **"This week" is the last seven days**, as long as answers are kept (`askrules::OFFER_WINDOW_SECS`),
  per project on this machine. Nothing is synced: the line says what this machine saw.
- **An ask is an answer given in Asks.** Every answer there is now noted in
  `<data>/inbox/rules.json`, not only one whose request the core names (an unnamed one has an
  empty `request`, so it offers no rule and starts no cool-off). An ask answered only in its chat's
  terminal, or never, is not counted: purlis does not see it answered. The cap on answers kept
  rose from 500 to 2000 for a busy week across projects.
- **Time to answer is from the ask's own raised time** (its `since`, as Asks orders by it), to the
  answer. A timed-out ask answered late counts from when it was raised (#6007). An ask whose source
  does not know when it was raised counts as an ask and not as a wait.
- **An ask saved is read, never kept.** A rule's grant is met by the sandbox with no ask, so nothing
  in `asking::with_rules` sees it. What is recorded is each connection purlis's own proxy carried
  (`sandboxblock::record`, 30 days): an ask saved is a chat reaching a host rule's host, let through
  at the rule's scope (this chat's, `you` or `project`, or `you` for this machine), after the rule
  was made, once per chat per day, and not the rule's own chat on the day it was made. Rule.saved,
  the counter #6010 left, is gone: All chats and Rules I made both read the record.
  **Not counted yet:** a folder's, a service's and a run's rule, whose grant is met with no record
  of it. Their saves are not guessed; a record of a grant used would let them count.
- **The line is quiet.** One `ui-text-meta` line per project with asks or saves, read once as All
  chats opens (`ask_measures`, off the window's thread), with no control, no count on any icon
  and no notification. A project with nothing to say has no line.
- **Every scenario flow has a ceiling.** Each test of every scenario spec is a flow, counted by
  the root hooks every config shares (`e2e/leftBehind.ts`, `e2e/interrupts.ts`): each prompt shown
  unbidden, each rise of the ✋, and each notification sent (`asks-notified`). A prompt is what
  W10's budget calls one (`src/interruptBudget.ts`), and it counts only where no action of the
  flow's own (a press, or Enter, Space, Escape or a key with a modifier) is waiting for it: the
  picker, a menu's dialog, the palette or the confirm a flow's own press opened is what it asked
  for, and a second prompt with no action between, such as a trust question after the picker,
  counts. Nothing about it is timed. The ceiling is W10's three prompts, three asks and one
  notification unless the file is named in `CEILINGS` with its reason. A file whose flows went
  over fails in its `after all`, naming each flow and each prompt it counted. #1079's flows (a
  relaunch, a restart to update, someone else's project, a handoff) keep their three prompts. A
  page loaded again mid-flow goes uncounted, said in the run's output.
