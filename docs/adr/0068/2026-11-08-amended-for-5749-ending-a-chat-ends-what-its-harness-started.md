# Amended for #5749, ending a chat ends what its harness started, 2026-11-08

Spec #5743 (story 30) says that when purlis ends a chat, none of the chat's own work outlives its
harness, so that a wake line saying the work "ended" is true. Ending the program's process group
did not do that. Claude Code runs each shell its model starts in a session of its own, so a
background shell is not in the harness's group. Quitting, restarting, stopping or a host settling
a crash ended the harness and left the shell running as an orphan. The scenarios in
`app/src-tauri/src/handoff/tests/across_restart.rs` showed it.

1. **What a chat's harness started outside its group ends with it.** When a chat's program is
   ended while it still runs (a close, a quit, a restart's end of the old run, a stop), and when
   the next host settles a program the last one left (§4 of the 2026-10-16 amendment), the
   program's descendants that are in another process group are read first. Each one, and the
   group of each one that leads a group, is hung up together with the program's group and killed
   with it. These are `purlis_terminal::process::Strays`. Each one is signalled only while the
   process list still shows its id with the start time it was read with. An id that has gone to
   another process in the meantime is left alone. A person's own shell tab is not a chat's
   harness: `nohup` and `setsid` are how a job there is meant to outlive the tab, so its spec does
   not ask for this (`Spec::ends_what_left_its_group`). The host record keeps the answer for the
   crash path as `ends_strays`.
2. **They are read before the first signal.** A process whose parent has gone is handed to the
   system's first process and is no longer anyone's descendant. A stop reads them before its
   interrupt, because a harness that leaves on the interrupt hands them over at once.
3. **How they are read.** On Linux each process's `/proc/<pid>/stat` is read. On macOS, `/bin/ps`
   runs once for each program ended. A call to the system's own process listing needs `unsafe`,
   which would be a fifth audited block. *Decided in implementation, to be confirmed.*
4. **What is still not reached.** Nothing is read once the program has ended by itself: its
   descendants are nobody's by then. The same holds for a host crash where the harness died on
   the terminal's hangup before the next host settled it. To outlive its chat on purpose, a
   process must leave the harness's tree (a double fork, a service manager), not only its group.
   Before this amendment, leaving the group was enough. *Steward-decided under standing
   instruction 2026-11-08.*
