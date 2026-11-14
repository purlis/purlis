# Amended 2026-11-14: a pool on another machine is owned by the person's mark, bound to its pin

**Built by #6271**, under ADR 0123 §5 (the person's ruling of 2026-11-11 that added Docker pools
on another machine) and spec #6257. It extends item 3 of the amendment of 2026-10-26 to a third
kind of pool, and item 4 of the amendment of 2026-10-14 says where that mark is kept for it. The
choices not quoted from the record were made in #6271 and are named in its pull request.

1. **A pool on another machine (`remote-docker`) is owned only by the person's mark.** Its runner
   is not this machine, so it is not owned without one. The mark is per pool, not set by default,
   and the person's alone: their answer to the last question of Connect, asked as the accepted
   prototype asks it (#6265, choice 9): *"Who can sign in to {host}?"*, with *Others might, or
   I'm not sure* recommended, and *Only me, and people I trust with my logins* the mark. The same
   question stands on the pool's row in Settings › Pools. It is never set from a chat or by the
   machine.
2. **The mark is bound to the pin.** It names the machine as pinned (the host key or the CA, and
   the engine), and is kept as that pin (`owned` in `runners.json`). A pin kept again that differs
   clears it, so after **Check and trust again** the person marks the machine again if they still
   mean it, and the next login asks again.
3. **A marked pool's runner may receive a vault value**, a browser login's while the desktop is
   connected and under the person's opt-in, and a tool's value for one run, only while no run has
   found its pin changed. An unmarked one's never does, and the chat is told why and how the
   person marks it.
4. **The opt-in is bound to the runner that receives the values.** For this kind, the engine an
   opt-in (`poollogins::Bound`) is bound to is the machine and engine as pinned, in place of the
   local socket a Docker pool on this machine names, beside the pool's definition digest (which
   holds the pin too), its image, and the receiving runner's device id, pinned link key and image.
   A pin change makes every opt-in on the pool ask again. Taking the mark back forgets them.
5. **A local Docker pool is unchanged.** Its engine is a socket on this machine purlis trusts, and
   needs no mark. An engine reached any other way, a raw `tcp://` or `ssh://` `DOCKER_HOST`, or a
   socket forwarded from another machine by anything other than this kind, is still not taken for
   this machine, whatever pool on another machine is marked.
