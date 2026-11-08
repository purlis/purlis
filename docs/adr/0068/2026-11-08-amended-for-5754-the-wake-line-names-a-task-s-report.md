# Amended (2026-11-08) for #5754: the wake line names a task's report and does not quote it

The amendment of 2026-11-08, "a reopened chat is told what the restart ended", says the wake
line "carries the chat's dispatched tasks and any report that arrived or was pending". #5754
builds that part. This note records how the line carries it, because the spec's user story 16
reads "that report typed into my wake line".

**The decision.** The wake line of a chat that asked tasks lists:

- each task the launch restored, by its chat number and where it stands: brought back on its
  conversation and carrying on, still running because the session host kept it, or ended
  without a report because purlis could not bring it back;
- each task whose report waits for the chat, by its chat number and how it ended, in the words
  the chat's task list uses ("done", "blocked", "ended without a report").

The line does **not** quote a report's text. The report reaches the chat the way a report always
does: as context for a turn, quoted as data under purlis's own heading. A report pending at the
quit is still in the chat's folder when the chat starts, so it is handed to the turn the wake
line starts. A report kept on the dispatch record (one that reached no chat) is handed over only
after the chat has started, so it may arrive with the next turn instead. `purlis dispatch wait
<chat>` prints any of these reports at once, from the task's dispatch record. The line says both.

**Why.** Report text is a chat's own words, and a line a harness is started with is the one place
where purlis has never put such words unquoted. The hook path already quotes report text as data,
checks it against what charter would send, and hands each report over once. Copying the text
into the line as well would hand it to the chat twice and make the line long, while a first
message has a size limit. Each report named in the line is held to a record in the dispatch
store, which no chat can write, so a file a chat forged in the folder names nothing.

**What follows.**

- `purlis dispatch wait` answers a task whose report the app no longer holds in memory, after a
  restart or after a turn has read it, with the report its dispatch record keeps. A task asked
  for by the chat it resumed counts too. A cleared row answers nothing.
- A task that is told to carry on and that also asked tasks gets one line: `CARRY_ON`, then its
  own tasks. It is not told twice not to dispatch them again.
