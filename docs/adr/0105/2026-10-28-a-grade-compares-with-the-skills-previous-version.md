# Amended 2026-10-28: a grade compares with the skill's previous version

Ticket #3237 (SK-57), from spec #2762 decision 22 ("each fixture with and without the skill (or
against the previous version)") and decision 43 ("a scorecard against no skill and against the
previous version"). It adds to §2, §3 and §5, and builds the `previous` arm §1 spelled.

**A skill with a previous version is graded against it too.** Its grade runs three arms: the skill
as the copy holds it, no skill, and the skill as its previous version had it. The previous version
is the newest of the skill's history whose folder held another tree than the copy's: for a copy
with changes not committed, its newest version; for one committed, the version before it. That
arm is the baseline, so the grade's difference is against it (§2), and the scorecard says the
difference against no skill beside it. A skill with no other version runs the two arms as before.

**The previous version is read from git, never from a folder.** Its throwaway piece is laid out
from git's tree of that commit (`skills::history::files_of_version`), as Try it lays a folder out:
each plain file, as text, without `evals/`, at most 32 files 8 folders deep. A link or another
repository in that tree is never followed and is left out, and so is a dot-name, a file past
1 MiB and a file that is not text. Nothing a chat could write since that commit reaches the arm.
A version that is no whole skill of this name (one from before a rename) is not compared, and the
plan says why.

**The plan says which version it compares with**, by commit and subject, and its chats grow by
the arm's: the cost the plan shows is of all three. The consent covers the arms and the previous
version's commit and files, so a grade started on a plan whose previous version changed is
refused (§3).

**The record says it** (§5): `previous`, the commit the arm was laid out from. A row with a
`previous` arm that names no version, or names one that is no whole hex id, is skipped. The
Grades panel shows the arm and its version, and its difference against that version.

## Rejected

- **The version the record's last grade graded as the baseline.** A grade is of a version, and the
  person edits a version: the one before it is what an edit is weighed against, whether or not it
  was graded.
- **Reading the previous version from a kept copy in this machine's state.** git already holds
  every version by content, the same on every machine, and a kept copy would be a second record
  of what git says.
