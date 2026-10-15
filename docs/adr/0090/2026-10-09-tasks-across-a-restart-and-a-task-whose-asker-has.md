# Amended (2026-10-09): tasks across a restart, and a task whose asker has gone

The operator's rulings V100-63 and V100-64 (spec #1483, built in #1513).

- **A task that had not reported when the app quit comes back under the chat that asked**, on
  its conversation, by the lineage its own entry in the record of open chats already keeps. It
  is told one fixed sentence of purlis's to carry on, never its brief, and only where the
  app's own dispatch record of it agrees that it is that chat's running task.
- **One that cannot be told to carry on has ended by itself.** A task with no conversation to
  resume, or no profile to start on, is not started (a fresh chat would need its brief
  again). Its record ends as "ended without a report", keeps its conversation for a Reopen,
  and the chat that asked is told once.
- **A refused start is not an end.** A task the launch could not start (every agent stopped,
  a profile to approve again, a folder that moved) waits with the other chats that did not
  start, its dispatch still running, drawn under the chat that asked (#1497); **Try to start
  again** starts it told to carry on, and only **End task** on that row (or Forget, where it
  is listed with the chats that did not start) ends it and tells the chat that asked. A record of open chats that could not be read ends
  no dispatch.
- **Nothing is dispatched twice.** Besides the brief never being sent again, a chat started
  again since it dispatched a task, which sends the same brief to the same persona while that
  task still works, is refused with the task's number.
- **A task whose asking chat has gone works on to its report**, as a top-level row that says
  "asked by <chat> (closed)". Its report is kept on its dispatch record, where the person reads
  it, and for the workspace, as before. When the chat that asked comes back (a launch, Retry
  now, or Resume from any of its session records) it is handed every such report once, and
  the copy kept for the workspace is taken back first; one that lands after the Resume goes to
  the chat that resumed it. A report it never read before it closed is handled the same way.
  A task the person started from a tab is not: its report is theirs.

**The follow-ups of #1513 (#1546), 2026-10-09:**

- **A report kept for an asking chat is owed until it has been left for that chat.** Taken
  from the workspace as the chat starts, it is claimed only once the chat has started and as
  it is left for it; a start the app dies in, or a report that cannot be written, leaves it
  owed on its record, and kept for its workspace again should the chat not start after all. Reopen on a finished task's row hands
  the reopened chat what was kept for the tasks it had asked, as Resume does.
- **What a launch believes is also held to the dispatch record's profile and folder.** A task
  is told to carry on only where its entry runs on the profile, and in the folder, the
  dispatch started it on, as well as by the ids and persona; and **Try to start again** tells it to carry on only while
  its dispatch is still running.
- **A question a task asked its asking chat before a restart is not kept.** The task is told
  to ask again if its work waits on the answer; the asking chat, answering the question it
  still reads, is told it was not kept, and that it can answer when it is asked again.
- **Decided not to, for now:** a line on the asking chat's next turn listing its tasks brought
  back (a second dispatch of one is refused with its number, `purlis dispatch list` lists
  them, and each report is announced as any report is); a line typed into a chat that came
  back when a kept report is handed to it (purlis's line names the task by a number its asking
  chat may wait on, and a chat that came back is not that chat: the report reaches its next
  turn); and writing a restored entry's lineage over from the dispatch record (an
  entry the record contradicts is told and reported for in nothing already, and the record of
  open chats is out of a sandboxed chat's reach).

**An orphaned task ends at its report** (#1510, 2026-10-10; delegated to the dispatcher under
V100-64, in force as the interim; confirmation pending). This replaces the interim of 2026-10-09, which
kept a task whose asker had gone open, asking for the person as #1448 had it, until Past tasks
gave its report a place in the window. Past tasks is now the Dispatches tab, narrowed to a
workspace and a date, and it shows the report from the task's dispatch record.

- **A task whose asking chat has gone ends at its report, as every other task does.** Its
  report is kept on its record and for the workspace, as before. The task is told it is
  finished, and its program is ended once its turn is over, by the same rules as a task whose
  report reached its chat: held while a chat it started is at work or while the person is
  looking at it. No needs-you item is raised for it, at its report or when the chat that asked
  closes after it reported.
- **What decides it is the app's own record of the delivery it just made**, never the
  dispatch record on disk.
- **What stays open is what stays open anywhere**: a task the person started from a tab
  (D-1443-9), a blocked task, a handoff's chat, and a task the person has taken up again since
  it reported. Each of these is a needs-you item that says its report had nowhere to go.
