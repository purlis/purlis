### Changed

- **`memory_search` with no query shows the newest 20 memories, not all of them.** They come
  newest first, with a cursor to the 20 before them, so a chat never fills its context with a
  workspace's whole memory. A search with a query answers as before (#5500).
