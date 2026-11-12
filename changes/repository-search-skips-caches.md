### Fixed

- **A project full of cache files no longer refuses every sandboxed chat.** The search for the
  project's repositories counts folders, not files, never enters a folder tagged as a cache
  (`CACHEDIR.TAG`, or a Go build cache), and, when it is still crowded out, names the folder
  whose tree holds the most rather than the one with the most files of its own (#6280).
