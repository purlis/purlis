### Fixed

- A clone purlis made no longer shows its Git LFS files as changed in purlis's own git, and a commit purlis makes there stores an LFS file as its pointer rather than whole. A file purlis checks out there stays a pointer until `git lfs pull`.
