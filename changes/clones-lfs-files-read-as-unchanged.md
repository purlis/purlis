### Fixed

- A clone purlis wired no longer shows its Git LFS files as changed in purlis's own git, and a save there stores an LFS file as its pointer rather than whole. A file purlis checks out there stays a pointer until `git lfs pull`. A push from purlis does not upload LFS objects yet: run `git lfs push --all origin` in the clone.
