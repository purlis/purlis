### Added

- `purlis recall` answers from a project search index the session host keeps, so it stays fast on a large project. It finds the same memories as before, now ranked by relevance, and says `refreshing` when the index may be behind the files. Deleting the index is safe: recall reads the files until the host has built it again. A secret-shaped span in a memory never reaches the index.
