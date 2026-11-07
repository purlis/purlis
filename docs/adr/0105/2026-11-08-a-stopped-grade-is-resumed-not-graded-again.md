# Amended 2026-11-08: a stopped grade is resumed, not graded again

Ticket #5718, on the operator's six rulings of 2026-11-08. It adds to §2, §3 and §4, and to the
amendments of 2026-11-02 (a draft's grade) and 2026-11-06 (several skills under one limit).

**A grade that stopped short can be resumed.** At its token cap or its dollar limit, at the
program's usage limit, with its service out of reach, after chats that did not start, after one
that did not say what it spent, or by the person's Stop, a grade of purlis's own runner keeps
what each of its chats did, in its place: each test or outcome eval, in each arm, on each run (a
*case*). **Resume runs only the cases without a result**, and a case cut off as it ran (a chat
stopped, ended at a limit, or never started; an eval whose judge was cut off) runs again whole.
The kept and the new results are scored by the same code (`grade::resume`), so the scorecard has
a whole grade's shape, its row in the record is a grade of its own, and it is marked resumed. A
grade purlis stopped under before it was scored (§3, interrupted) has nothing kept and is graded
again; a Grader's grade is the Grader's to run, and is not resumed.

**Only while nothing that decides the result changed.** What a grade's result depends on is kept
with it as digests (`grades::resumable::Deciding`): the skill's text (its files but its fixtures,
what its folder held, and the version it compares with), its tests (its trigger tests, the outcome
evals ticked and the files they stage), the program and the model its chats run on, and the
grade's settings (its arms, runs, shell and Grader). Where any differs now, only Grade again is
offered, with a line saying what changed.

**A new cap, confirmed like any grade.** A resume is planned before it runs: the chats left, about
what they would spend at what the stopped grade's chats spent each, and the project's cap and
dollar limit, which **count only the resume's own spend**. The person confirms it ("about 40,000
tokens left to run; cap 200,000"), on the stopped grade's profile and model, and a press on a plan
that changed is refused (its consent, §4). The resumed report's spend is both runs' together, with
the earlier run's kept beside it (`report.earlier`), and the scorecard shows both.

**A stop says why.** Two ends join §3's: `usage-limit` and `unreachable`, told by the harness's
own hook (Claude Code's `StopFailure`, by its error's category: `rate_limit`, and `server_error`,
which it names a lost connection by), never read from what a chat printed (ADR 0076). The scorecard says "Stopped at the
token cap", "Stopped: Claude Code's usage limit was reached", "Stopped: network unreachable" or
"Stopped by you". Where the program said when its limit resets (its status line's `rate_limits`,
a window used up and its `resets_at`), the grade records it, and Resume says it and waits for it.

**Never by itself.** A resume spends money, so it waits for the person's press. The Skill
studio's Needs you lists each stopped grade the person did not stop themselves, saying why it
stopped and when Resume can run; its one fix opens the scorecard.

**A draft's grade and Select's batch resume too.** A draft's stopped grade resumes against the
draft as it is now, held to the same check. A batch whose grades stopped short offers to resume
them together: each runs only what its grade left, on its own plan, and one new limit the person
sets holds them all, as the batch's did.

**Kept on this machine, one per copy.** What a stopped grade ran and what decided it is a store
of its own (`docs/plane-format.md`, `skill-grade-resume/<grade>.json`; Machine, device-bound,
transient). A new grade of the copy, or of its draft, forgets the stopped one, so Resume always
goes on from the copy's latest grade; losing the store loses only Resume.

## Rejected

- **Resuming whole cells (a test in an arm, all its runs).** Arms take turns run by run, so a grade
  stopped part way has almost no whole cell, and a resume would run nearly all of it again. A case
  is one run; its place keeps each run's pass rate what a whole grade's would be.
- **Resuming automatically once a limit resets.** It spends money: the ruling keeps every resume
  the person's press, and Needs you says when one can run.
- **Reading why a chat stopped from what it printed.** ADR 0076: the harness's hook says it by
  category, and where it says nothing the grade stops as it did before.
