# A machine that answers and refuses is not retried, 2026-11-19

This note bounds *A runner's link down past a blip says why and offers Try now* (2026-11-18,
#7134), whose item 2 has purlis keep linking again whatever the reason. The ruling of 2026-11-18
on #7187 (all eight items accepted as recommended) separates a machine that does not answer from
one that answers and says no: retrying the second on a timer is only told the same thing, and
leaves a person waiting on a link that cannot come back by itself. Decided in implementation, to
be confirmed (D-7187-1 to D-7187-4).

1. **Refused stops the retries** (session protocol 1.94). Where the machine a pool is reached
   through answers, but its sign-in, Docker or forwarding fails, the kept link is told down once,
   `refused`, and not tried again on a timer, as a changed key and a runner gone already are. The
   next command that needs it takes the links anew once and, if it is refused again, is refused,
   saying the machine refused and that purlis stopped trying. A machine whose key or engine
   changed is held the same way (its own state, **Host key changed**, below), and stays a security
   stop. **Try now** on a refused link dials once, ahead of anything the connector remembers of
   the last failure, and is told the same if nothing changed. *Rejected:* a longer pause between
   tries, which still leaves a start waiting on a link that cannot come up. *D-7187-1.*
2. **A start does not wait on it.** A start waits up to 30 seconds for a link that is merely
   down (item 2 of the ruling); a refused link is not one that is coming back, so the wait ends
   at once and the start is refused with the reason. *D-7187-2.*
3. **The wire carries it as one more field.** `link_down` gains `refused` and `host_key_changed`, absent
   when false, as `key_changed` and `gone` are; a host that cannot tell Refused from Host key
   changed says neither. A window before 1.94 ignores it and reads any link that is
   down as before, so it keeps offering Try now, which takes links anew once; a host before 1.94
   never says it, and its link is read as Unreachable. Nothing is sent to a host for it, so there
   is no `KNOWS_` version to gate on. *D-7187-3.*
4. **The window says Refused where it says a link is down.** The Notice names the pool, gives
   the host's reason, says purlis stopped trying on its own and offers Check pool and Try now, and it
   is said at once, not after 30 seconds, since waiting does not mend it. Settings › Pools says
   "refused" beside the pool. A pool forgotten on this machine ends its chats as **Pool
   forgotten**, not as a runner gone: its runner and home may be there still. A runner gone ends
   its chats too (**Start again on this pool**); it does not dim them as a changed key does. A
   Docker still starting after a reboot is **Unreachable**, not Refused: it is retried. *D-7187-4.*
