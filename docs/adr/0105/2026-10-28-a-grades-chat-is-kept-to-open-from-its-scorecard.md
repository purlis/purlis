# Amended 2026-10-28: a grade's chat is kept to open from its scorecard

Ticket #3239 (SK-59), from spec #2762 decision 43 ("each fixture's chat one click away"). It adds
to §3, where a grade's chat is drawn nowhere and its throwaway piece goes when it closes, and to §5.

**purlis keeps what each grade's chat showed as it ended**: its screen and scrollback, as the
escape sequences that draw them, the snapshot every view of a session is sent first. It is taken
as the grade ends the chat, before the close, and kept in this machine's state,
`<state>/skill-grade-chats/<skill>/<grade>/<chat>.screen` (`docs/plane-format.md`). It is the
terminal's, so it means the same on every harness, and it needs no harness's conversation file,
which Claude Code keeps by the folder the chat ran in and which the piece's removal leaves behind
under a folder that is gone.

**Kept for each skill's three newest grades.** When a grade ends, the chats of the skill's older
grades go. A grade's id is a ULID, so the newest are known by name. A teammate who pulls the eval
record sees the grade and not its chats: they ran on another machine, and the scorecard says so.

**A row of the Grades panel opens the grade's scorecard**, a tab of its own: the grade as its row
says it, then each trigger test with the skill and without it, what each run started, and each of
its chats, one click away. A chat opens read-only in a tab of its own, drawn by the window's
terminal from the kept screen and taking no input. The chat itself is not resumed: a grade's chat
ran on a throwaway piece purlis has removed, with limits no ordinary chat has.

**A grade's chat that asks the person something says so in the grade** (§3's "asks nobody"). It is
still ended at its first ask, no run and never a test that did not trigger, and none of its asks is
listed or notified. Each trigger test's score keeps how many of its chats did so (`asked`), the
grade's end says how many in all, and the scorecard says it beside each test, with each such chat
one click away to read what it asked.

## Rejected

- **Keeping each grade's throwaway pieces and resuming its chats.** A resumed chat could act again,
  outside the grade's limits, and the piece holds a copy of the skill the person may have edited
  since. What the person needs is what the chat did, and that is what it showed.
- **Reading each harness's own transcript.** It is a format of each harness's, read from the
  person's own folders, and Claude Code files it by a folder that no longer exists.
- **Keeping every grade's chats.** A skill graded after each edit would keep hundreds; the newest
  three are the ones a person compares.
