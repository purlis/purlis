# Amended 2026-10-27: a runner a live chat owns is never idle, and one left a day without a link suspends itself

**Built by #2051**, under spec #2041 (the person's rulings of 2026-10-14, stories 37, 39, 40 and
41: *"a runner that a live chat owns, or whose browser holds an Ask, is never idle"*, *"a runner
left 24 hours without a link suspends itself (configurable per pool)"*, *"Stop all means all"*).
Items 4 to 6 were decided in #2051 and are **decided in implementation, to be confirmed**
(D-2051-1 to D-2051-6 in the pull request).

1. **§9's idle gains two things that are never idle.** A runner on which a live chat runs, or is
   waiting on the person between turns, and a runner whose browser holds an Ask, are not idle,
   whatever the idle timeout: overnight work is never stopped as idle. Idle stays §9's *no running
   chat, no open stream, no forwarded request, no build*, read with these two. Purlis has no idle
   stop yet (the cluster operator's is #2058); this is what it is held to.
2. **A runner left without a link suspends itself.** A runner's host that has had no link from
   its desktop for a day stops serving: every chat it runs ends as at a quit, and its process
   ends, so a Docker pool's container stops and keeps its home volume. A link that ends does not
   stop a chat: a laptop closed overnight stops nothing before then. Suspending is not idle: a
   chat working with nobody linked for a day is suspended, as story 39 asks. **This supersedes
   V29c for the unattended case** (ADR 0078, ruled V29: *"a runner never stops itself after a
   silence. The window names every runner a stop has not reached and keeps trying, and `purlis
   stop --all` on the runner is the fallback"*): the person's ruling of 2026-10-14 (spec #2041,
   story 39) has a runner left a day without a link suspend itself. V29c stands for the kill
   switch: a runner a stop did not reach keeps its chats running and is named, as ADR 0078 §7
   says. **Only a runner made to suspend itself does** (`purlis serve --suspends-unlinked`), and
   only a Docker pool makes its runners so, until the cluster operator suspends its claims
   (#2058): a pod that ended would only be started again in place. A Docker runner made before
   #2051 keeps the words it was made with and never suspends itself, which is the safe side.
3. **The kill switch reaches every runner this machine keeps,** as ADR 0078 §7 says (and see
   ADR 0078's amendment of 2026-10-27 for how the window reads a runner): the
   desktop's host sends a stop to each, a runner not reached is named in what the switch says
   until it is, and it is tried again while the desktop stays stopped. **The window's re-arm
   re-arms the runners that stop reached** (ADR 0078 §4, V29b), so a runner's `remote-link` is
   granted `rearm` "relayed" in the session protocol's table: the desktop's host sends one only
   as its own window re-arms it, to the runners its stop stopped. A runner that says it was
   stopped already (from its own command line) is not counted as reached, and is left stopped,
   as §7 says. "Reached" is what the runner says of itself. A link's stop and re-arm are written
   in the runner's journal `by: link`, the re-arm a person's and the stop unattributed, with the
   desktop's device id (ADR 0071 and ADR 0075 as amended by ADR 0078). Which runners were
   reached, and which are owed a stop, are kept beside the switch, and a host that starts
   stopped sends its stop again.
4. **The day is purlis's default for every pool for now.** A pool setting for it is a new key
   in the pools' file, which waits on the person's word (#2051's leftovers). *Decided in
   implementation, to be confirmed (D-2051-3).*
5. **A suspended Docker runner is started again only by the window's relayed link**, which the
   window makes as a chat of its workspace starts or is taken back there, once its container is
   checked as it was made, and checked whole once it runs (stopped again where that fails). A
   pool's check, a browser's view, a draining runner's poll, an unpairing and the kill switch
   never wake one: the check says it is suspended, and the poll counts it as running no chat. A
   link made to it while the desktop is stopped carries a stop first, and a start relayed while
   the desktop is stopped is refused. *Decided in implementation, to be
   confirmed (D-2051-4).*
6. **A runner's host that no desktop is paired with never suspends**, so `purlis serve` on a
   machine of its own lives as before. *Decided in implementation, to be confirmed (D-2051-5).*
