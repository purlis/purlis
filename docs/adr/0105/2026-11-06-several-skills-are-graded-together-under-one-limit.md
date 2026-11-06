# Amended 2026-11-06: several skills are graded together, under one limit

Ticket #5356, on the operator's ruling of 2026-11-05 (Grade in the Library's Select mode, #5332).
It adds to §3 and §4.

**Select's Grade grades the skills chosen, each a grade of its own.** Each starts as a press of
Grade on its Grades tab starts one, on the project's default profile and model, with its trigger
tests and no outcome evals (those wait to be ticked on the skill's own plan, §4), and goes through
the same check (`plan_holds`) against the consent its estimate showed. Each is held to its own
token cap and dollar limit, recorded as any grade is, and shown on its skill's Grades tab.

**The estimate comes first.** Before anything runs, the window shows what each skill's last grade
spent, worked out for this one (§4's estimate), added; names each skill it has nothing to go on
for (never graded here, or a last grade whose chats said nothing of what they spent), never a
figure that was not measured; and names every skill left out, with why: no tests, outcome evals
alone, a grade of it running, a built-in or a skill from outside the project. Nothing starts until
the person sets a limit and presses Start. Where some have nothing to go on, the limit is not
filled in for them.

**A few run at a time.** Three grades of one batch run together; any other grade still runs alone
in its project, and none starts while a batch runs. Within each grade one chat still runs at a
time (§3).

**The person's limit holds them all, and stops cleanly.** `purlis_core::skills::grades::batch`
adds what the grades have spent so far, as each of their chats ends. Once it reaches the limit, no
grade starts and those running are stopped, which scores and records what they ran, so every grade
that ended is kept; the skills never started are named, with why. A chat that does not say what it
spent stops the batch too, as a limit cannot hold what it is not told. A batch goes past its limit
by no more than what the chats running at that moment spend, as a grade goes past its own.

**A batch is held in memory alone.** A quit ends it, and each of its grades is recorded as a quit
leaves any grade (`grades::underway`). It is no store of its own.

## Rejected

- **One grade at a time, in turn.** The ruling asked for a few at a time; three keeps a batch of
  ten from taking an hour of the person's afternoon, and no more than three chats spend at once.
- **Splitting the limit between the grades up front.** A skill with fewer tests would leave its
  share unspent while another stops short; one limit over the whole batch spends where the tests
  are.
- **Outcome evals in a batch.** They stage files and spend a turn and a judge each, and the plan
  of each asks the person to tick them one by one after reading what they hand a chat (§4); a
  batch would hand that over unread.
