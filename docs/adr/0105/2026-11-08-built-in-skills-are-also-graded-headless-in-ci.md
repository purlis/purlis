# Amended 2026-11-08: built-in skills are also graded headless in CI

Ticket #5719, from the person's spec #5714 (purlis's built-in skills trigger at the right moment),
whose decisions Q1 to Q36 were settled with the person on 2026-11-08. It adds a second place
trigger tests are graded, in CI and for built-in skills alone. It keeps §2's scoring and changes
nothing of §3 to §5 for a grade in the window.

**purlis's built-in skills are graded in CI by a CI grader.** Every built-in skill carries
trigger tests in Skill studio's format, and a CI job grades them on every harness purlis supports:
Claude Code, Codex and opencode. It starts each harness non-interactively, by the harness's own
print mode, on a throwaway CI runner, with what a person's chat is handed: purlis's plugin, the
session-start briefing and purlis's MCP tools, by the same routes the app uses for each harness.
Each test runs among a **crowd fixture**, about forty skills purlis writes itself so the listing
is as full as a person's, and once more without it, so a wording problem can be told from a
crowding problem.

**A skill start is read from the harness's machine-readable output and scored by §2's rules.**
The grader reads the structured event stream the harness writes in its print mode (its tool calls,
never text the model printed) and counts a skill as started by §2's routes: the harness's own
Skill tool, a read of a briefed `SKILL.md`, or `skill_load`, with a plugin's namespace no part of
the name. A test runs three times and passes at half or more, as a grade's does. A prompt that
starts the wrong built-in skill counts as a **miss** for the skill it belongs to and a false
trigger for the one that started. Each skill is held to its **trigger bar**, declared beside its
tests: strict (at least 95% recall, at most 5% false triggers) where a miss leaks a secret, loses
work or loses context; standard (at least 85%, at most 5%) for the rest. Each result records the
harness, its version and the model it ran on.

**Why §3's reasons against print modes do not hold there.** ADR 0105 rejected print modes for a
grade's chats because such a chat has no event log writer, so its skill runs would not be
recorded as every other chat's are, and because its sandbox and arming would be a second copy of
the window's. Both reasons protect a person's machine and a person's record. On a throwaway CI
runner neither is present:

- **Nothing is recorded that a log would have to hold.** The CI grader writes no skill run, no
  skill log and no row of the eval record. Its result is the CI run's own: the run's summary, a
  pull request's check, and the nightly's issue. A start read from the event stream is the one
  fact it needs, and the stream holds it.
- **The runner is the sandbox.** The runner is a fresh machine made for the job and thrown away
  after it, holding no person's files, credentials or plugins. Its model keys are in a GitHub
  environment of their own, `skill-grades`, each with a hard spend cap at its provider, so a run
  can spend no more than the cap. There is nothing for a second copy of the window's sandbox to
  guard.
- **No person is present.** §3's consent, the plan shown before Grade is pressed and the
  person's token cap exist because a grade spends the person's tokens on their press. A CI grade
  spends the project's, under the environment's caps, and that is why it may run on every pull
  request that touches a built-in skill and nightly on `main`, where the window's grade never
  runs on a schedule (decision 22).

For the same reason the 2026-10-28 amendment's ruling that opencode never grades does not reach
CI: a fresh runner has no plugin installed but purlis's own, so there is no other plugin to turn
off.

**It decides no run's state.** ADR 0076 holds: no purlis run exists in the CI grader for its
output to move. It reads the harness's structured output to score a test, as a grade's Grader
reads a run-chat result, and only in CI.

**What it measures, and what it does not.** The CI grader measures only what a harness chooses
given the listing: whether, among the skills it is handed, it starts the one a prompt belongs to,
and leaves the others alone. It does not say how well a skill does its job once started (outcome
evals stay the window's), nor how a skill fares among the person's own skills and plugins (the
crowd fixture stands in for them). It is not a **Grade**: it is never shown in the Skill studio,
never written to a project's eval record, and never runs a skill the person made.

**In-app Grades are unchanged.** A grade in the window still runs real chats the app starts
(§3), with only purlis's plugin on, held to the person's cap and consent, and recorded in the eval
record. Built-in skills are still not graded there, and opencode still cannot grade there; this
amendment makes neither gradable in the window.

## Rejected

- **Running the window's grade in CI.** It needs the app, a display and the person's consent per
  grade, and it can grade only Claude Code. The CI grader shares its scoring, not its runner.
- **Scoring by the first tool call alone** (skill-creator's probe, rejected in §2 for the same
  reason): it works only on Claude Code. Reading every route §2 names makes "triggered" mean the
  same in CI and in the window.
- **Reading starts from what the model printed.** A model can name a skill without starting it.
  The structured stream's tool calls are what the harness did.
- **Writing CI's results into the eval record.** That record is a project's evidence about its
  own skills' versions; a built-in skill's version is purlis's release, and its evidence is the
  CI run.
- **Grading in an empty listing alone.** Recall in a listing with nothing else in it flatters
  every description; the bare run is reported beside the crowded one, never instead of it.
