# Amended for #2003, the event log and a program's end are kept in the session host, 2026-10-15

With the start-time switch on, the session host is the event log's one writer, and what a chat's
program's end leaves in the log and in its dispatch record is kept there, whether or not a window
holds the project then. Spec #1974's grill ruled the place ("the event log has one writer, the
session host"; "what happens at its program's end happens in the session host, so that it happens
even if the window is gone"). §6 and the amendments above are left as accepted, and this note
adds to them; it closes the double record #2000's item 8 described. Items 1 to 8 are **decided
in implementation, to be confirmed** (D-2003-1 to D-2003-8). With the switch off nothing changes:
the window opens the log and writes it, as before.

1. **The host opens the log; the window opens none.** The host opens the device's log the
   first time it needs it, which is as it first holds a project: only a window whose chats run
   in the host holds one, so a host whose window keeps the log itself never opens it. The
   window, with the switch on, never opens the log, and hands the host every write it decides.
   The log's lock (`events.lock`) still refuses a second writer. *Rejected:* both opening it
   and taking turns at the lock (a writer that waits on a window is a log that stops with it).
   *D-2003-1.*
2. **A write travels as what it stands for.** Each kind of write the window makes is one
   `purlis_core::eventlog::Entry` (the run a start begins, a start's sandbox decision, a
   person's audited act, a kill switch line, a hook line with what the board made of it), and
   the host writes it with the code the window's own path runs (`Recorder::apply`). So the two
   paths write the same events. A run whose start may still be put aside (#2250, ADR 0066 as
   amended 2026-10-16) travels as a held `Begin`, and its start going on as `Begun`: the host
   holds its `run.started` as the window's own log holds it. *Rejected:* the window composing finished events (the run a
   number maps to, a sub-agent's child run and a tool call's pairing are the writer's state,
   and would be decided twice). *D-2003-2.*
3. **Session protocol 1.8, a minor.** It adds `record` (one entry, answered once it is kept,
   in the order the window sent them) and the refusal `not_recorded`, each recorded in the
   protocol's fixture. FD-27's table gains a row, `local-ui` only: the log is the audit's
   source (ADR 0075), so a scope that could write in it could record an act no person made.
   Minors 1.6 and 1.7 are #2175's and #2196's. *D-2003-3.*
4. **A hook line is recorded on the thread that heard it.** The window still decides what a
   line moves (D-2000-1); what its hearing would have written of a line the host handed on goes
   back with the line's reply (`relay::carry`, `Replied::Record`), and the host writes it before
   the hook is told the line is taken. It is written once the window's hearing returns, so after
   the board and the window have moved: nothing the window is told reads the event log, and the
   hook's promise (a line is taken only once it is kept, FD-30) holds. The host writes only an
   entry that is the record of the line it came back with: that line's kind, saying what it
   said, of its chat and its project; and the log writes a trust or audited entry only under a
   `trust.` kind. The host drains each chat's spool, as the project is
   held, soon after the chat is heard from, as it is closed and as its program ends. So a line
   and its spooled copy are told apart in one process, and a line its hook spooled after the
   host took it late is recorded once (#952). *Rejected:* the window sending a hook line's write
   as `record` (the copy's check is per process, and the drain is the host's). *D-2003-4.*
5. **The host keeps what a program's end leaves.** As a program ends, the host tells its
   binary how, with its own act that ended it where one did (the kill switch, a quit, a stop, a
   close: the highest where several did, ADR 0076 §2) and whether a window held the project
   then (`purlis_host::Record::ended`). The binary drains what the chat's hooks spooled and ends
   its sub-agents' runs with its own in the log, window or no window. *D-2003-5.*
6. **With no window, a task's end is its dispatch's.** Where no window holds the project and
   the program ended by itself, the dispatch the chat worked ends as one that went without its
   report, owed to its asking chat (`dispatchrecord::ended_unwatched`), as a project opened
   later settles one whose chat is gone; its row is kept, since the chat that asked may still
   be at work, and the window hands it the report when it is back. While a window holds the
   project, the window decides that end, and what it moves, from the board and the asking
   chat's record it holds, as before, until the board moves into the host (#2194). A quit, the
   kill switch, a stop and a close end no dispatch here, as D-1443-10 rules. *D-2003-6.*
7. **A chat another chat dispatches starts in the host.** The window's answer to a dispatch
   starts the task through the same seam every chat starts through, so with the switch on the
   host runs it. Who may dispatch, the grants, and what the task gets are the window's decision,
   unchanged. *D-2003-7.*
8. **A dispatch asked with no window is not answered in the host yet.** The decision reads the
   board and the asking chat's record, which are the window's until #2194: an ask no window is
   there to hear is answered as an app with nothing to answer it (D-2000-5), and the asking chat
   is told so. *D-2003-8.*
