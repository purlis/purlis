# The home's Docker folders are never a chat's to write, 2026-10-20

This note adds to the later-code class (ruling V73b) and changes none of its rulings. Item 2 is
**the person's ruling of 2026-10-20**; the others are **decided in implementation, to be
confirmed** (D-2752-4, D-2752-5).

1. **`~/.docker/bin`, `~/.orbstack/bin` and `~/.rd/bin` are denied to every chat's writes**
   (#2752). Docker Desktop, OrbStack and Rancher Desktop link `docker` and its credential
   helpers there, and purlis runs `docker` from there outside any sandbox for a Docker pool. They
   are denied as paths of the home, where a link in their place leads included, at every level
   and on every harness. A project's own `.docker` folder, and Docker's settings beside them
   (`~/.docker/config.json`, `~/.docker/buildx`), stay the chat's. *Rejected:* adding the three
   names to the later-code names held at any depth, which would stop a chat making, moving or
   removing a project's `.docker` folder of compose files. *D-2752-4.*
2. **The folders that hold them, `~/.docker`, `~/.orbstack` and `~/.rd`, are held as entries
   alone** (the person's ruling): never made, renamed, moved aside or replaced, so no folder
   that brings a `docker` of its own takes their place, while what is in them beside each `bin`
   is written. A denial says how much of its path it holds (`Reach`): everything below it, or
   the entry alone. Claude Code's own sandbox holds everything below a path it is given, so an
   entry held alone is held by purlis's own wraps, as a later-code name that holds only itself
   is.
3. **Each is held under every spelling a case-folding disk takes**, as the later-code names are:
   a folder not there yet is made under the name as typed, so `~/.Docker/Bin` is held as
   `~/.docker/bin` is. Seatbelt is given a regular expression with each letter in either case,
   under both firmlink spellings, and Claude Code each path as written and with each letter as a
   class of both cases. *D-2752-5.*
