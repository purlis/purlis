# Amended 2026-11-08: the CI grader reads a harness's output to score, and moves no run

Spec #5714 ("purlis's built-in skills trigger at the right moment"), following
[ADR 0105's amendment of 2026-11-08](../0105/2026-11-08-built-in-skills-are-also-graded-in-ci-by-a-headless-ci-grader.md)
(built-in skills are also graded in CI, by a headless CI grader), which the person accepted with
the spec's decisions on 2026-11-08.

This record says nothing parses harness output to decide anything: a run's state moves only by a
named cause. The CI grader reads a harness's machine-readable output (Claude Code's
`stream-json`, `codex exec --json`, `opencode run --format json`) to find which skill a trigger
test's chat started, and scores it. That is a reading of harness output, so its place is said
here.

- **It scores a trigger test in CI, and nothing else.** The grader runs in CI only
  (`crates/skills/src/grade/ci.rs`, run by `skill-grader` from `.github/workflows/skill-grades.yml`),
  on a throwaway runner, on chats no person started. What it reads decides one test's pass or
  fail in that run's summary. It is not the window's grade, which still scores by the skill runs
  its chats' hooks record (ADR 0105 §2).
- **A run's state still never moves on parsed output.** No run, chat, board row, needs-you item
  or record of a person's is moved, opened or closed by what the grader reads. Its chats are not
  purlis's runs: no `purlisd` holds them and no event log records them. Every cause in §2 stays
  the only way a run moves.
- **Nothing in the shipped path reads it.** The parser lives with the grader, behind its CI
  entry, and the app and the `purlis` binary never call it to decide anything.

## Rejected

- **Scoring CI's chats by hooks, as the window does.** It would need an event log writer on a CI
  runner and a second copy of the window's arming (ADR 0105's amendment, "Why §What was rejected
  does not hold here"). The harnesses' JSON output names the same starts by the same rules.
