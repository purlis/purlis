# Amended for #2006, chats survive a crash of the window and reattach, 2026-10-16

§2's table says what the host does when the app crashes: it keeps every session for a **grace
period** (ruled, V22a: ten minutes), a window that starts within it reattaches with nothing
lost, and past it the host ends the sessions as a quit would. Spec #1974 puts this in its fifth
slice, and #646 (FD-7) is its delivery. This note builds §2's crash row and lifts #1997's interim
items 5 and 6 (D-1997-4, D-1997-5). The grace period and the quit as said are §2's, ruled. The
items below are **decided in implementation, to be confirmed** (D-2006-1 to D-2006-10), but
item 11, which the dispatcher decided.

1. **The window says it quits; a link that ends without a word is a crash.** The window's quit
   sends `quit`, and the host ends every program as a quit (the end each keeps is
   `stopped: quit`), answers once they are gone, and stops. The host's standard input no longer
   ends it: it says only that the window that started it has gone. A window that does not say
   `quit` (a crash, a kill, a quit the host did not answer) leaves the host keeping its programs
   for the grace period. *D-2006-1, to be confirmed.*
2. **Each project waits out the grace period on its own.** A project no link holds, kept for
   its programs (#2194) or its saving (#2002), is let go of once the grace period has passed
   since its last hold went, as a quit would: its programs end all at once, running or ended,
   then its confinements, its saving, its hook channel and its kept board go. So a window that
   comes back and does not open a project does not leave that project's agents running unseen,
   and a project whose programs have all ended is let go of after a window had the grace period
   to read how each ended (#2262). This is the window's host only: a runner's host keeps its
   chats running with the desktop away (spec #2041). *D-2006-2, to be confirmed.*
3. **The host stops** once no window is linked and either nothing is left that a window could
   come back for, or the grace period has passed since the last window went; a host whose
   window has not linked yet waits for it until the window that started it goes. A window is a
   `local-ui` link that subscribed or held a project. Nothing polls: the host sleeps until the
   next grace period ends or a window or a hold comes or goes. Ending as a quit leaves the
   window's record of what was open as it was, so the next start offers those chats for
   reopening, as after any quit (§2). *D-2006-3, to be confirmed.*
4. **Session protocol 1.13, a minor.** It adds `programs_of` (a held project, from a link that
   holds it), answered `programs`: each harness and shell the project still has, running or
   ended, with its number, its process, its run, its exit, the place of the confinement it runs
   in, and its size; `quit`, answered once every program has ended; and the refusal
   `too_large` for a `board` larger than one answer carries. FD-27's table gains a row for each
   command, `local-ui`'s alone: what `programs_of` answers places a chat's confinement, which
   only the window confines, and a quit is the person's act. `restarting` is not added: an
   update still ends every chat until the upgrade handoff (#665, ruled 2026-10-14), so it is a
   quit. *D-2006-4, to be confirmed.*
5. **A window started beside a serving host reattaches to it** (lifts D-1997-5). The window
   still starts its host; one that exits because another serves this user (D-1997-2) leaves the
   window to connect to the one serving, with the credentials that one minted. The window's
   supervisor (#2007) does not start a host again while that link holds: the host is not the
   window's to end, so its end is its link's, and once the link ends a host is started again,
   counted toward the breaker as any unexpected end is. A quit ends the window's supervision
   before it tells the host, so nothing is started again after a quit. The host runs in a
   process group of its own, so a signal to the window's group does not end it with the
   window. *D-2006-5, to be confirmed.*
6. **A window that comes back holds the chats as the first one did** (D-1997-3). On a host a
   window started, each process admitted as `local-ui` is taken as a holder beside the window
   that started it: a peer below any of them is inside a chat. Holders are only ever added; past
   a bound, a window from yet another process is refused. *D-2006-6, to be confirmed.*
7. **A chat put back is reattached where the host still has it.** As the window holds a
   project it asks `programs_of`, after it takes the kept board. A chat its record puts back
   under a number the host still has is reattached: the window keeps everything of it as for a
   start (its tab, its board entry where the kept board did not bring one, its sandbox's
   hearing) and starts nothing, so no run begins, no trust event or opt-out count is written,
   no first message is typed and no credentials are handed; its run is the one it was. Its
   confinement is the one it ran in, heard and answered by this window from now on. One that
   ended while no window was there is reattached and told ended at once. A dispatched task the
   host still runs comes back as it is, at work and owing its report, whatever its record says
   it could be resumed by, and is told nothing. *D-2006-7, to be confirmed.*
8. **What no chat put back reattached is closed** once the project's put back has settled
   (a start fresh, a record that could not be read), so nothing runs with no tab. *D-2006-8, to
   be confirmed.*
9. **A kept board too large to hand over is said on each chat reattached** (#2194's D-2194-6
   left it to the log): the host refuses `board` with `too_large` and why, and each chat
   reattached in that project carries a Notice, until dismissed, saying what it was doing could
   not be brought back and that its state is right again from its next report. The reason is
   in the log. *D-2006-9, to be confirmed.*
10. **A line handed on to a window that went before replying moves the kept board.** As the
   last link that held a project ends, the project's hook channel is told no window holds it:
   every line still waiting for that window's reply is let go of, and each report, tool call or
   refused commit among them is heard by the kept board, as a line no window took is. Its hook
   spools it as before. A window that is only slow is not this: it still moves its own board and
   tells the host. *D-2006-10, to be confirmed.*

11. **A chat the host still runs is reattached, never refused or closed by what its start
   would check now** (decided by the dispatcher). Its profile gone or no longer allowed, its
   sandbox refused, or its line refused refuse a start, and nothing starts: the chat is taken
   back as its record has it. What narrowed while no window was there reaches it the way it
   reaches a running chat. `programs_of` names the plan its confinement was started from (the
   window's own, as it sent it); where that is not the plan the window would make now, the
   chat's sandbox reads as one that changed, and it is offered a restart on the sandbox of now
   as any chat a change left behind is (#1428). A reattached chat is closed only for item 8's
   reasons. *D-2006-11.*

**What this does not do.** An update still ends every chat (#665). A host that is killed ends
every chat (§3; #2007's supervisor and run journal). A browser a project holds is not
reattached yet: browsers are spec #2008's to bring back, and one in a project let go of ends
with it.
