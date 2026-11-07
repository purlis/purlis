### Changed

- **Finding duplicate memories stays quick on a large store.** `purlis persona dedupe`,
  `workspace optimize`, `persona optimize` and `persona stats` compare each memory only with the
  memories that share one of its rarer words, instead of with every other memory. They find
  exactly the pairs they found before, and a store of 10,000 memories takes seconds instead of
  minutes.
