# Amended 2026-10-27: the window reads a runner's event log over the link from its own cursor

**Built by #3393**, under spec #2041 (stories 12, 15 and 69). §7's row for events stands: *"the
desktop subscribes with a cursor per device. The runner's host is the only writer of the
runner's event log. The desktop never copies the runner's events into its own."* This says how,
and was **decided in implementation, to be confirmed** (D-3393-1 to D-3393-4 in the pull request).

1. **Session protocol 1.41.** `log_end` answers a host's device and the `seq` of its log's last
   line. A `subscribe` whose `since` names that device is pushed each line after it, then each
   as it is written, as `event`, and a gap as `missed`; one that names no such device is pushed
   none of the log, so every client before 1.41 is pushed what it was before. Only the window, and
   its desktop's host relaying it, may read a host's log (`remote-link` "relayed").
2. **The window reads from where the log ends as it links**, and on from its cursor after it fell
   behind or the link came back, so no line comes twice and a gap is said, never skipped.
3. **Only what the window draws is held, and small**: of each line, a top-level run's end, its
   chat's id, one of three words for how it ended, and its time, each field a few bytes; every
   other line moves the cursor and is not kept. At most 1,024 of them and 128 KiB per link, in the
   window's memory alone, written nowhere on the desktop's disk: a window started again reads on
   from where the runner's log ends. Only a chat the window routed to that runner is said.
   #3472 widens what is held.
4. **The first view that reads them is "While you were away"** (#2051): each run of the
   project's chats that ended on a runner while the person was away, with its device and its
   time in the person's time zone, and a runner whose link is down said to be.
5. **§7's re-arm, as built (#2051).** The desktop's host re-arms the runners its stop stopped. A
   runner stopped from its own command line answers that it was stopped already, is not counted
   as reached, and keeps the re-arm of its own §7 names. A `rearm` the window sends on a link its
   host relays to a runner reaches that runner whatever stopped it: the person's own act (V29b).
