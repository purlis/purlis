### Fixed

- **A chat can push the task branch purlis cut for it, though the branch is on the forge
  already, and a rebased branch is no longer held for CI changes it did not make.** The forge
  route refused every push to a branch that existed (403, protected ref); the branch folders a
  chat holds are now its own from the start, while the default branch and any other branch stay
  protected. A forced push after a rebase was read against the branch's old tip, so every
  `.github/` change the default branch had gained counted as the push's; it is now read against
  where the branch leaves the default branch.
