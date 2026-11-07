### Fixed

- **Listing a memory store looks at each file once.** Every recall that reads the files, and the
  project index's catch-up when purlis starts, asked the disk about each memory twice; on a
  project of 200,000 memories the catch-up took about a quarter longer than it needed to (#5486).
