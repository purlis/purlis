# Amended 2026-10-20: the desktop's host relays a window to a pool's runner

**Built by #2679** on §2, §3 and §4. They are left as accepted, and this note says how the
window first reaches a Docker pool's runner. *Decided in implementation, to be confirmed*
(D-2679-5 to D-2679-7). The protocol's part is ADR 0068's amendment of the same day.

1. **The window's link is relayed in the desktop's host, never in the window** (§4: the link
   lives in the desktop's `purlisd`). The window names the pool's runner with `relay_to` as its
   link's first command, and the host relays that link as #2215 keeps any relayed link.
2. **The host reaches only a runner this machine paired and keeps**, read from `runners.json`
   as each `relay_to` is taken: the pool's runner, through §2's container preset, with the
   container checked as it was made before every connection (the amendment of 2026-10-17, item
   2), and linked with this desktop's link key against the key pinned at its pairing. A name
   that is not kept, or a pool whose runner is not, is refused, and nothing is run.
3. **A runner that answers with another key is refused on every try and said to have changed
   its key**, so the window offers to remove it (§3: a link whose keys do not match is refused,
   whatever the connector).
4. **This machine's `docker` is where Docker's installers put it, else the first on `PATH`**:
   Docker Desktop's own, `/usr/local`'s, Homebrew's and the system's, then, in the home folder,
   Docker Desktop's, OrbStack's and Rancher Desktop's, and only then `PATH`'s absolute folders
   (`purlis_core::dockerpool::Docker::on_this_machine`). A window opened from the Finder has a
   `PATH` that names none, and the program the host runs to reach a runner is taken from where
   Docker installs it before anywhere else.
