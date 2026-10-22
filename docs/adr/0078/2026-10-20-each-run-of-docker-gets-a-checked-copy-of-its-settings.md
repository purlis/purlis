# Each run of docker gets a checked copy of its settings, 2026-10-20

This note adds to *A changed runner key is a window Notice, and Docker's helpers are found where
it is* (#2752) and changes none of its rulings. Item 1 is **the person's ruling of 2026-10-20**;
items 2 to 4 are **decided in implementation, to be confirmed** (D-2752-6 to D-2752-8).

1. **purlis's own runs of `docker` never read the person's Docker settings.** Each one is given
   `DOCKER_CONFIG` naming a copy made fresh for it from `~/.docker/config.json` (or the window's
   `$DOCKER_CONFIG`), in `<data>/docker-settings/`, `0600`. The copy keeps `auths`, and
   `credsStore` and `credHelpers` only where the helper is in Docker's own folders (where its
   installers put `docker`), not the home's folders and not the window's `PATH`. It drops CLI
   plugin folders, plugin hooks, contexts and every other setting, and the engine is pinned.
2. **The engine is the window's own `DOCKER_HOST`, else the system's socket, else the
   installer's socket in the home**, in the order `docker` is looked for: `/var/run/docker.sock`,
   then `~/.docker/run/docker.sock`, `~/.orbstack/run/docker.sock` and `~/.rd/docker.sock`, each
   in the home only where its installer's folder is there as itself. A socket is used only where,
   as it resolves, it is in a folder only root writes, or under one every chat's sandbox holds
   (item 3); anything else, the window's own `DOCKER_HOST` included, refuses the run with a
   sentence that names its folder. A context is never read, `DOCKER_CONTEXT` is not handed on,
   and `docker context` is not followed: an engine reached only through one is named by
   `DOCKER_HOST` or by linking `/var/run/docker.sock` to its socket. *Rejected:* reading the
   current context's endpoint from Docker's settings. *D-2752-6.*
3. **Where an engine keeps its socket is held from chats**, under every spelling: `~/.docker/run`,
   `~/.orbstack/run`, `~/.rd/docker.sock`, `~/.colima` and `~/.lima` in the home, `podman/` in the
   temporary folder (a Podman machine's helper), and `docker.sock` and `podman/` in the runtime
   folder (a rootless engine). *D-2752-7.*
4. **The copy goes with its run**: a link's `docker exec` holds its copy for as long as the link
   runs (`purlis_host::CommandConnector::preparing`), and every other run until it has ended.
   *D-2752-8.* Since #2893, each run holds its folder's `run.lock` while it goes, and a `run-*`
   folder not changed for a day whose lock no run holds is swept as a run is prepared and as the
   app starts; a run still going keeps its copy however old (decided in implementation, to be
   confirmed: D-2893-1).
5. **A run's `PATH` is Docker's own folders and the system's** (`/bin`, `/usr/sbin`, `/sbin`):
   nothing of the home or of the window's `PATH`, so the helper `docker` runs is the one the copy
   was checked against. *D-2752-10.*
