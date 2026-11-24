### Fixed

- **A chat can push a commit the forge already has.** Fast-forwarding a branch to a commit that is on
  the forge, or making a branch at one, sends git no new objects, and purlis refused it with "the
  quarantine has no commit". purlis now asks the forge whether it has the commit, reads the
  push's changes against it as for any push, and holds a change to CI files as before. A commit
  the forge does not have is still refused.
- **A push purlis does not send says why in git's output.** A push refused for a temporary reason
  (the base is not read yet), held for CI files or otherwise refused used to show only `RPC failed;
  HTTP 403` and then `Everything up-to-date`. Git now prints `! [remote rejected]` for each ref with
  purlis's reason, and the push still exits non-zero.
