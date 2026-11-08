# Amended 2026-11-08: built-in skills are also graded in CI, by a headless CI grader

Ticket #5719 (M65.1) of spec #5714 ("purlis's built-in skills trigger at the right moment"),
whose decisions were grilled with the person on 2026-11-08 (Q8 to Q12, Q22, Q23 and Q36: "agree
at all").

## Where purlis was

A grade is real chats purlis runs in the window, on the person's press and the person's account
(§3, §4). Built-in skills are never graded there, only Claude Code can grade, and CI runs no
grade at all. So no built-in skill had a single trigger test, and nobody could say how often a
harness picks one when it should. §"What was rejected" turned down the harnesses' print modes
(`claude -p`, `codex exec`, `opencode run`) for a grade's chats: such a chat has no event log
writer, so its skill runs are not recorded as every other chat's are, and its sandbox and arming
would be a second copy of the window's.

## The decision

**purlis's built-in skills are also graded in CI, by a CI grader.** It is a second grading path,
for built-in skills in CI only, beside the grade in the window, which it does not change.

- **Each harness is started headless, as a person's chat sees it.** The CI grader starts Claude
  Code, Codex and opencode non-interactively, each with purlis's plugin, the session-start
  briefing and purlis's MCP tools, on the trigger test's query.
- **Skill starts are read from the harness's own machine-readable output** and scored by the
  rules for a skill run started (§2): the harness's Skill tool, a read of a briefed `SKILL.md`
  (Codex), or `skill_load`, with a plugin's namespace no part of the name. Each test runs three
  times and passes at half or more, as §2 scores it, so "triggered" means the same in CI and in
  the window. The scoring reads recorded output and runs offline; only the CI job calls models.
- **Among a crowd.** Every grade runs among a **crowd fixture** of about forty synthetic skills
  committed with the grader, written by us and sized near Claude Code's listing budget. Each
  skill's **trigger bar** (strict or standard, declared beside its tests) is held there. A bare
  run without the crowd is reported beside it, to tell a wording problem from a crowding one.
- **A wrong built-in counts on both sides**: a **miss** for the skill the query belongs to, and
  a false trigger for the one that started.
- **Pinned on pull requests, latest at night.** A pull request is graded only for the built-ins
  it touches, against harness versions pinned in one committed file. The nightly run grades every
  built-in on main against the latest versions. Each result records the harness, its version and
  the model it ran on.
- **CI's own keys, never the person's.** The model keys are held in a GitHub environment with a
  hard spend cap per provider. The results go in the run's summary, never in a project's eval
  record (§5).

**Why §"What was rejected" does not hold here.** Both of its reasons are about a person's
machine. The event log matters because a chat in the window records skill runs that the person's
skill history and totals read; the CI grader reads the starts straight from the harness's output
by the same rules, and it records nothing for a person. A second sandbox matters because the
person's files, Allows and account are within reach of the chat; a CI runner is thrown away after
the run and holds none of them, and no person is present to be asked. The rejection stands for a
grade in the window, which keeps its chats, sandbox, plan, consent and limits as §3 and §4 say.

**What it measures, and what it does not.** The CI grader measures only what a harness chooses
given the listing: whether it starts the skill, from its name and description, among the others.
It does not measure what the skill does once started (outcome evals stay in the window), what a
person's own catalogue adds to the listing, or what a chat would do under the window's sandbox.

## Rejected

- **Grading built-in skills in the window.** A built-in skill is the same in every person's
  chat, and its grade belongs to the change that edits it, before it ships. A grade in the window
  runs on one person's account, after the fact.
- **Making Codex and opencode gradable in the window for this.** The CI grader needs none of the
  window's limits that only Claude Code can hold today (§3); #3272 stays its own work.
- **An empty listing.** A skill alone in its listing is picked far more often than among a
  person's real catalogue, so a score without the crowd flatters it.

## Consequences

- Every built-in skill carries trigger tests, and CI fails a pull request that adds one without
  them.
- CI holds model keys for the first time, in one environment, each under a hard spend cap.
- A nightly drop below a trigger bar opens or updates one issue per skill and harness, labelled
  `skill-triggers`, and closes it when the skill recovers.
- purlis-report's trigger grade (#5699) runs on this grader rather than a job of its own.
