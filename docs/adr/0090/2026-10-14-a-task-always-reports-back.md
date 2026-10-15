# Amended (2026-10-14): a task always reports back

Ruled by the operator on 2026-10-14 (#1981, D-1 to D-6). A task that ended its turn without a
report left the chat that asked with nothing: `purlis dispatch tell` waited for a turn an idle
task never started, and the person relayed by hand.

- **A follow-up starts an idle task's turn, in every harness** (D-3). purlis types its one line
  into a task at rest on Claude Code, on Codex once its hooks are trusted, and on opencode,
  whose plugin hands the turn that line starts its context through `chat.message`. A task
  mid-turn reads the follow-up when that turn ends, as before. purlis ends a turn for a task
  only on a harness that ends one on a single Escape (Claude Code and Codex): a stop or a
  cancel of an opencode task mid-turn asks it for its report when that turn ends.
- **A turn that ends owing the report is asked for it once** (D-1). purlis types its line
  asking for `purlis dispatch report`, which starts one more turn. Not while the task's turn's
  end is not the end of its work: a task of its own still owes a report, it is paused on a
  question to the chat that asked, a stop or a cancel of it is under way, or helpers of its
  own are still at work.
- **If that turn also ends without one, purlis sends a report purlis made from its last
  message** (D-1, D-5). Its harness hands its `Stop` hook the message the turn ended on
  (Claude Code's and Codex's `last_assistant_message`; purlis's opencode plugin reads it from
  opencode's own client and sends it in the same field). The hook sends it to the app on a line
  of its own, before the turn's report, which is never spooled and never recorded, and which
  the app takes only from the run it adopted for the chat. The app keeps it in memory for a
  task alone, masked as tool output is, until the task's next turn begins. "Report" stays the
  task's own word: the chat that asked and the person are both told this one is purlis's,
  under purlis's heading, with the message quoted as data. Its outcome is purlis's alone, and
  no line a chat sends can claim it.
- **It settles nothing for good.** The task stays open and owes one more report (#1768): a
  report of its own later reaches the chat that asked too, and takes this one's place on its
  dispatch record. The chat that asked is flagged for the person, as for a task that came to
  nothing, and the count puts it there.
- **The chat that asked can read what its task said and ran** (D-2): `purlis dispatch read
  <chat>`, for its own tasks alone, by the app's record of whose task it is, while the task runs
  and after it closed. It prints the task's last three messages and its last twenty shell
  commands with the status its harness said, each line quoted as data under purlis's sentence,
  masked as tool output is. The commands reach the app from the shell tool's post hooks on the
  line the last message travels on, never spooled or recorded, and are kept in memory for a
  task alone. The full conversation stays the person's, in the task's tab. Claude Code still
  asks the person before `read` runs: it joins the pre-allowed dispatch words of ADR 0064 only
  when that is ruled.
  - **An exception the operator ruled** (#1981, D-2) to the rule of ADR 0066 and ADR 0075 that
    the app keeps a tool call's arguments only as a digest: for a task alone, a command line,
    masked, is held in memory so the chat that asked can read it. Nothing of it is written.
  - **Implementation choices, to be confirmed by the operator:** the counts kept (the last
    three messages and twenty commands); that they are kept after the task closes, for the
    chat that asked, until that chat closes; and that purlis's opencode plugin waits up to
    3 seconds for opencode's client before a turn's end is sent without its last message.
- **Silence, not heartbeats** (D-6). The chat that asked never polls. purlis watches each
  task's last activity: what its harness tells purlis's hooks (a turn beginning or ending, a
  tool starting or coming back, a notification, a turn's last message), never its terminal's
  bytes, which a spinner keeps redrawing. While a task's turn runs and shows the person no
  prompt, a stretch with nothing heard for `quiet-note-minutes` (15 by default) is noted, and
  one shell command that has not come back for `one-command-note-minutes` (30 by default) is
  noted by its program where the hook named one from purlis's fixed list. One note a stretch:
  any activity starts a new one. The note goes to the chat that asked, in purlis's own words on
  its next turn and in the line purlis types where it may, and to the task's tab as a Notice
  that comes down when the task is heard from again. It never ends the task. Both limits are
  settings in the `[dispatch]` table, beside the dispatch limits, with the same levels: a
  workspace's and a persona's override the project's, this machine's only lowers them, and a
  policy is a ceiling. A task whose program exits, or whose chat is closed, is reported at once
  as ended, as before (#1443).

## Amended 2026-10-15: a finished task, and closing it (#1981 D-4)

Ruled by the operator, 2026-10-15: **#1485 stands**. A task's program ends at its report, and
what stays is its finished row under the chat that asked: a readable record of its report,
with **Reopen** to continue its conversation as an ordinary chat, and **Clear** to put the row
away. Nothing else closes by itself.

- **`purlis dispatch close <chat>`** closes a finished task the asking chat dispatched itself,
  and no other chat's, by the app's record: its finished row goes, and a task kept open after
  its report (it came out blocked, or purlis made its report from its last message) has its
  program ended first, by the chat's own close. Its report stays on its dispatch record.
- **It is refused** for a task that has not reported (a cancel asks it for its report), one in
  a turn or showing a prompt, one the person is stopping, one the person typed in, which is
  theirs to close from its tab, and the two the #1485 end holds back: one the person is
  looking at, and one with a task of its own still at work. Each is read again under the lock
  a report is taken under, and the chat is closed in that same hold. Claude Code asks the
  person before it runs: it ends a chat, and is no read.
