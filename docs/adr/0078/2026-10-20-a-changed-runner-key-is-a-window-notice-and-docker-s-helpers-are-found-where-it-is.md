# A changed runner key is a window Notice, and Docker's helpers are found where it is, 2026-10-20

This note adds to *The desktop's host relays a window to a pool's runner* (#2679) and changes
none of its rulings. Each item is **decided in implementation, to be confirmed** (D-2752-1 to
D-2752-3, D-2752-9).

1. **A relayed link's `key_changed` is said once for the project, in the window** (#2752). The
   window's link hears `link_down` with `key_changed` and tells the window the project's chats
   are dimmed, then that the runner's key changed (`chats-reconnecting`, `key_changed`). Each
   chat's pane stays dimmed: unlike a link that is only down, the host does not link to that
   runner again until it is removed and added anew. The window says why in one Notice at its
   own level, with **Open Pools settings**, where the pool is removed, and **Dismiss**, and
   puts it away once the chats are said to be reconnected. *Rejected:* a Notice on each chat's pane, since a
   runner's outage is one state for the project and never a notice for each chat (#2496).
   *D-2752-1.*
2. **The Notice names no pool.** The window's link does not say which pool's runner it is
   relayed to, and the person's Pools list says which runner's key changed. *D-2752-2.*
3. **`docker` is run with its credential helpers looked for where it is found.** Docker runs a
   registry's helper by name on its own `PATH`, and a window opened from the Finder has a
   `PATH` that names none of Docker's folders. So purlis runs `docker` with a `PATH` of the
   folder it was found in and the one its link leads to, then each place Docker's installers
   put it, then the window's `PATH`, of its absolute folders alone: the order it finds `docker`
   in, so no folder a chat could write comes before Docker's own. *Rejected:* reading the
   helper's name from Docker's settings and looking it up purlis's own way, which would read
   Docker's configuration where the rest of purlis leaves it to `docker`. *D-2752-3.*
4. **A place in the home is taken only from the installer's own folder.** A `docker`, or a
   helpers' folder, under `~/.docker`, `~/.orbstack` or `~/.rd` is taken only where that folder
   is there as itself: its real path is the home's real path and that very name, byte for byte.
   A link in its place, or another spelling of its name on a disk that folds case, is passed
   over, whichever harness's sandbox held it. *D-2752-9.*
