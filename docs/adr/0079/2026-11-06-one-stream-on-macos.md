# One stream on macOS, and no sweep (2026-11-06, #5499)

The person ruled on the scale spec (#5486, Q4, Q13 and Q21 of 2026-11-06: "yes to all"). §4's
"between commits, the watcher's paths" changes like this:

1. **On macOS, `planewatch` is one recursive FSEvents stream over the project root.** The
   folders §4 names, and the rest of the set `planewatch` reads, are filtered in purlis from
   the stream's paths. They are no longer each a watch. One watch per folder restarted the
   stream for each folder added: 376.8 s of setup at 1,000 workspaces (#5497's first run),
   against a budget of 1 s.
2. **Linux keeps one non-recursive inotify watch per folder.** The memory and session folders
   are already in the set (FD-10).
3. **The five-second sweep of every watched folder is removed, on both platforms.** A full
   reconcile (every reader reads again) runs only when the stream reports that it lost
   events: `MustScanSubDirs`, a dropped-events flag, or inotify's overflow.
4. **A folder whose watch is refused falls back to a timed check of that folder only.** The
   sidebar and the panels are no longer read from disk on every request while the watch is
   partial.
