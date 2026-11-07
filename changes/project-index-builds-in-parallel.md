### Changed

- **A project's search index builds about six times faster.** The first index of a project at
  the scale spec's size (1,000 workspaces, 200,000 memories) took about two minutes; it now
  takes under 20 seconds, because each memory is read and scanned for secrets on every core while
  the index is written. What the index holds, and every answer it gives, is unchanged.
