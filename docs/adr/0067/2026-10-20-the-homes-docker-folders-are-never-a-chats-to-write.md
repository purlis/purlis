# The home's Docker folders are never a chat's to write, 2026-10-20

This note adds to the later-code class (ruling V73b) and changes none of its rulings. Its item
is **decided in implementation, to be confirmed** (D-2752-4).

1. **`~/.docker/bin`, `~/.orbstack/bin` and `~/.rd/bin` are denied to every chat's writes**
   (#2752). Docker Desktop, OrbStack and Rancher Desktop link `docker` and its credential
   helpers there, and purlis runs `docker` from there outside any sandbox for a Docker pool. They
   are denied as paths of the home, where a link in their place leads included, at every level
   and on every harness. A project's own `.docker` folder, and Docker's settings beside them
   (`~/.docker/config.json`, `~/.docker/buildx`), stay the chat's. *Rejected:* adding the three
   names to the later-code names held at any depth, which would stop a chat making, moving or
   removing a project's `.docker` folder of compose files. *D-2752-4.*
