# Amendment, 2026-11-10: budgets and loops pause into asks, and what each one is

Built for #6015 from [ADR 0121](../0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)
decision 20 and [ADR 0076](../0076-a-run-moves-only-by-a-named-cause-and-a-chats-state-is-read-from-its-runs.md)
N4 and N8. That decision says a chat that reaches its budget, or is found looping, is paused and
asks. It leaves open what a budget is, what looping is, and what paused means in practice:

- **A budget is a dispatch limit the person set.** No new budget system is added. A budget is
  `minutes-per-task` (a task's working time) or `tokens-per-session` (a session's tokens, as its
  harnesses report them), both from #1512. Each is off until set. Before this change, a task at
  either limit was asked for its report and then ended. Now it is paused, and the person is asked
  **Continue**, **Raise the budget** or **Stop**. Stop is the earlier stop: the task is asked for
  its report and then ended, and the chat that asked is told which limit it reached. Continue lets
  the task go on past that limit, and the same limit never pauses it again. A new limit can.
  Raise the budget opens Settings › Project › Dispatch at its limits. The chat stays paused until
  a look of the clock finds it within the limit, and then it goes on. A paused task's working
  time does not count. Only tasks are paused, as only tasks were stopped: no limit measures the
  chat the person started. There is no spend or tool-call budget yet. ADR 0076 N4 names both, and
  no figure for either is kept.
- **Looping is Stuck?'s failing rule at twice its threshold, or refusals piling up.** The same
  command failing `2 × stuck-failing-times` times in a row (6 by default) is a loop, and so is the
  person refusing that many of the chat's permission prompts with no allow in between. Each
  threshold comes from the one Stuck? setting, so a person who raises one raises both. Unlike
  Stuck?, the counts are not reset when the chat's state moves, since each permission prompt moves
  it. The person is asked **It's fine, continue**, which starts the counts again, or **Stop**,
  which is the window's Stop. Denials the sandbox or a guard makes are not counted: the app does
  not hear them per chat as answers.
- **Paused means held still** (ADR 0076 §4). The chat's program and its tasks at work below it
  each have their process group stopped with `SIGSTOP`, and `SIGCONT` lets them go on. A host
  does this through `hold`, added in session protocol 1.85. A host before 1.85 cannot hold a
  chat: the chat is still asked about, and it keeps running meanwhile. ADR 0076 §4 also says the
  guard refuses a tool call while its chat is paused, and that is not built. A paused chat's
  board state is not `paused` yet either. Its ask is what shows it, in Asks and on its own tab.
- **Where it stands in Asks** ([the amendment ordering asks by facts](2026-11-10-asks-are-ordered-by-facts-as-built.md),
  #6006): a pause has no end of its own, so it is never Expiring. It waits for the person as long
  as that takes. But a paused task holds up the chat that dispatched it, which waits on its
  report, and so does every chat up that chain. Its ask carries `blocks` as any other does: it
  reads "Blocks N chats" and stands before the asks that only wait. A loop in a chat nobody
  waits on stands by its wait.
- **In memory.** A relaunch starts every chat unpaused. A task still past its limit is paused
  again at the clock's first look, unless the person let it continue.
