# Amended 2026-11-02: a draft is graded against the current version

Ticket #2780 (SK-17), from spec #2762's "Improve from runs", whose draft carries a grade next to the
current version's. It adds to §2, §3 and §5 and to the 2026-10-28 amendment that built the
`previous` arm. The operator's rulings, relayed by the `skills` chat, are of 2026-11-01.

**What is graded is what Accept would write.** The skill's arm is the project's files of the skill
with the draft's over them, read once, without `evals/`. A draft's page asks for the grade; the
plan, its consent and limits, the running page and the scorecard are a skill's grade's.

**The current version runs as the `previous` arm, read from the folder, for draft grades only.**
This departs from the 2026-10-28 amendment, which reads a previous version from git alone so that
nothing a chat could write since reaches the arm. A draft is weighed against what Accept would
replace, and that is what the folder holds, committed or not. The skill's own arm is already read
from the folder, so a chat that could write there can move either arm of an ordinary grade. A new
skill has no current version, and runs the two arms against no skill. A row may therefore carry a
`previous` arm and no `previous` commit, only where it marks a draft.

**Its tests come from the draft's overlay.** A fixture file the draft writes is the one that runs,
and one it leaves is the folder's. A draft that changes only `evals/` gets no grade: both arms
would be the same skill. Outcome evals stay off until ticked.

**A draft's grade is this machine's alone.** Its row goes to
`<data>/app-state/<clone key>/skill-grades/` whatever the copy, with `draft: {id, digest}`: the
draft's mint, so the next draft of the skill starts with no grade though it has the same id, and a
digest of what it wrote, so a draft changed since says so. The row stays out of the Grades tab, the
estimate and `purlis skill grade`; only its id, or the draft's own page, reads it. The draft is not
the project's: a teammate has no copy of what was graded.

**A grade never blocks Accept**, in any state: not graded, running, better, worse, no different,
stopped early, changed since, only changes evals, or no tests. Accepting or discarding a draft stops
its grade, as no page would show it any more.

## Rejected

- **Reading the current version from git for a draft too.** A skill with uncommitted changes would
  be compared with a version Accept does not replace, and a local skill has no version at all.
- **Keeping a draft's grades in the committed record.** It would publish a score of text nobody else
  can read, and the draft's own Discard would leave it behind in history.
- **Keeping the grade in the draft's folder, gone with the draft.** The eval record's reads, limits
  and scorecard already hold; a row marked by a mint needs nothing new to read it.
