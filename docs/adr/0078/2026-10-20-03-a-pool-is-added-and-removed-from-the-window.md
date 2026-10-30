# Amended 2026-10-20: a pool is added and removed from the window

**Built by #2679** on §2, §3 and the amendments of 2026-10-18 on removing a runner. They are left
as accepted, and this note says how the person reaches a Docker pool from the window. *Decided in
implementation, to be confirmed* (D-2679-8 to D-2679-10), but for item 4, which carries out what
the steward decided on 2026-10-20.

1. **Nothing about pools shows until one is added.** The palette's *Add a Docker pool…* is the
   way to the first: a dialog asks the pool's name, its image by digest and its limits, each
   left empty for the default, and the pool is added at once, its runner made, checked and
   paired. Settings › This machine › Pools, and its row in the palette, are offered only while
   this machine keeps a pool. *Amended 2026-10-30 for Settings: the Pools group, and its row in
   the palette, are always listed* ([the amendment](2026-10-30-the-pools-group-is-shown-before-the-first-pool.md)).
2. **Removing a pool asks first, then unpairs its runner over a link of its own while it answers** (§3: the
   desktop's key is taken out of the runner's peers when the link is up), then removes its
   container, its home and the pool, and sets every workspace that ran its chats on it back to
   this machine.
3. **A runner whose key changed is said in a Notice with Remove.** The Pools group asks each
   pool's runner once, as it comes on screen, whether it answers with the key pinned for it; one
   that answers with another key is refused (§3), and its row's Notice offers Remove.
4. **A workspace whose chats this machine's store sends to a pool starts none, and says why.**
   A pool's runner does not hold a workspace's project until code travels over the link
   (§7, RR-16), so such a start is refused in words that name the pool and how to run the
   chats here again, and is never run on this machine in its place. The workspace setting, the
   routing of a workspace's chats to the pool's runner, and the chat numbers this machine and a
   pool deal for one project come with that step, where they can be shown to work.
