# Amended for #2007, the supervisor, the breaker and the run journal, 2026-10-16

When the session host is killed outright, the window starts it again, and every chat it ran comes
back as failed or hibernated with a one-click Resume; three unexpected exits within five minutes
trip a breaker, and the window says so. Spec #1974's grill ruled the outcome on 2026-10-14 ("the
host restarts under its supervisor, with a breaker after three unexpected exits in five minutes";
"each chat shown as failed or hibernated with a one-click resume"), as §3 and FD-29 (#666) had.
§2, §3 and the amendments above are left as accepted, and this note adds to them. Items 1 to 7 are
**decided in implementation, to be confirmed** (D-2007-1 to D-2007-7).

1. **The window is the supervisor, on every platform, for now.** §3 has the window supervise
   where no service manager may be used without registering a background item, and a systemd
   user unit (transient, `systemd-run --user`) or a launchd agent elsewhere. While the host ends
   with the window (the window holds its standard input, D-1997-4) and is not a login item, a
   unit would hold neither end of that pipe, so the window, which already starts its host, starts
   it again too: on the thread that started it, watching the process and the link. A link that
   ends while the window has not asked counts as an exit: the host is ended first, since the
   window cannot speak to it. A host that found another serving (exit code 3) is not started
   again. The service manager's units come with the login item (#655), which is when the host
   outlives the window. *Rejected:* a separate launcher process (a third process that dies with
   the window all the same, and one more thing to supervise); a transient systemd unit now (no
   pipe to end it with the window, and Linux alone). *D-2007-1.*
2. **The breaker: three unexpected exits within five minutes, kept in memory.** A sliding window
   of the last exits, as systemd's `StartLimitBurst` and `StartLimitIntervalSec` count them.
   Once it trips, the window starts no chat, each start says why, and a Notice says the host
   stopped unexpectedly three times in five minutes, with the last lines it wrote to its log and
   **Try again**, which starts the count over and the host once more. The count is not kept
   across a launch of the window: a service manager's count starts over with the manager, and
   a count kept on disk would hold a host stopped across the update that fixes it. *Rejected:*
   persisting the breaker; a back-off between starts (the limit already ends a loop, and a chat
   waits for the next host as it waits for the first). *D-2007-2.*
3. **The run journal: JSON Lines in the host's own folder.** `<config>/purlisd/journal.jsonl`,
   beside the kill switch and the credentials, which every chat's sandbox denies (ADR 0067 §5):
   a `started` line as each program starts (its id in the host, its project's id and path, its
   number, its run, its process and when the kernel says that process started) and an `ended`
   line once what its end leaves is kept; each line written through to disk before the start is
   answered. A host that ends every program as it stops empties it; the next host to take the
   lock reads what is left, and empties it, before it serves anyone. Machine tier, device-bound
   (`docs/plane-format.md`). *Rejected:* the event log (the device's audit, which a host reads
   back only from a cursor, and which keeps no process to end); the socket's folder (the system
   may empty a temporary folder); a database (one writer, a handful of lines). *D-2007-3.*
4. **The next host settles what the last one left.** For each program the journal says was left
   running, it ends the program's process group where its leader is still the process that was
   started, by the kernel's word on when it started, and leaves any other process alone; then the
   binary keeps the end as a host crash's: what the chat's hooks spooled is recorded, and its
   sub-agents' runs end `failed` for `host-crash` (ADR 0076 §5, §6). A host that ended as it
   should leaves nothing to settle. *D-2007-4.*
5. **Failed or hibernated is ADR 0076's table, read from the board as the chat went.** The window
   hears the link end, and each chat that ran in that host ends with it. Its run is left where
   the table puts a host crash (`Run::after_a_host_crash`): a chat mid-turn, starting, paused, or
   stopped on an ask has **failed**; one idle at a turn's end, with a conversation to resume, is
   **hibernated**. Whether it can be resumed natively is read as whether it has a conversation
   to resume, which is what Resume needs. The chat's pane says which, with **Resume** (its
   restart on its conversation, one click) and Dismiss. The board, until it moves onto runs
   (#791), reads such a chat as a program that ended by a signal. *D-2007-5.*
6. **A chat that ended with its host did not end by itself.** As after a quit (D-1443-10),
   nothing tells its asking chat that a task failed, and a task chat reports once it is resumed.
   *D-2007-6.*
7. **The window holds its projects again on the host that replaces the last.** Each project is
   held, and saved, on the link it was held on; a start, and a project's saving, hold it again
   on the new link, so its hook channel and its saver run in the new host. While the host is
   being started again, a start waits for it as the first start waits for the first. The window
   says nothing over the session protocol it did not say before, so the protocol keeps its
   version. *D-2007-7.*
