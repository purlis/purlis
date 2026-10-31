# Amended 2026-10-30: grading after each edit compares with the version before each edit

Ticket #4016. It changes one sentence of the 2026-10-28 amendment that let a grade follow each
edit ("or compare with another version (a fixture or a commit a chat could make), nothing
starts").

**What the person accepts by turning grading after each edit on is "compare with the version
before each edit", not one commit.** The window's save of a skill's editor commits its `SKILL.md`
(`verbs::edit`). After that save the version a grade compares with (`history::previous_of`, the
newest version whose folder held something else) is the one the person just edited, not the one
the plan showed when the opt-in was turned on. Holding the opt-in to that one commit paused it on
the first edit of every skill that had a previous version, and on every edit after that. The
seam test `after_edit::tests::across_edits` shows this on a real repository.

So a grade after an edit no longer waits because the version it compares with changed. The opt-in
keeps the version the plan showed (`Accepted::previous`) as a record of what was shown, and holds
a grade to nothing else of it. Everything that changes what a grade spends, or what it runs,
still waits for the person: the profile, the model its chats run on, the trigger tests by digest,
more chats (a previous version's arm arriving is one), a larger token cap and a larger dollar
limit.

## Why this, and not pinning the comparison to the version accepted

- **An edit is weighed against the version before it.** The 2026-10-28 amendment that added the
  previous arm rejected "the version the last grade graded" as a baseline for that reason. A
  grade after each edit that compared with a version from many edits back would say how far the
  skill moved since the opt-in, not what this edit did.
- **The spend is unchanged.** The arm is the same arm, with the same chats, held to the same cap
  and dollar limit, whichever version it is laid out from. A version is read from git's tree of
  its commit, never from the folder.
- **A commit a chat could make adds nothing a chat could not already do.** It can move the
  baseline to a version it committed. The skill's own arm is already read from the folder, which a
  chat can write, and neither can make a grade spend more than was accepted or start a grade: only
  the window's save does.
- Pinning would also take a second way to plan a grade (against a named commit rather than the
  version before), which a grade pressed by hand would never use.
