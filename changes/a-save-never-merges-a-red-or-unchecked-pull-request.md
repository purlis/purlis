### Fixed

- **A save's pull request is never merged while it is red, or before its checks could run.** In
  `pr-merge`, purlis read the checks once, straight after it opened the pull request, and took
  "no check yet" for "no check": it merged, and CI then failed on a pull request already
  merged. Where the repository had auto-merge, a failing check that the branch did not require
  merged too. Now purlis merges the pull request itself, on every forge and whatever the
  repository's settings are, and only once it has read every check on the pushed commit as
  passed. A save merges nothing: purlis looks again by itself, a minute after the push and
  then every few minutes, so a pull request that goes green is merged without you saving
  again. With no check at all it is merged once the commit has been there a minute and nothing
  shows the repository runs checks on its pull requests.

### Changed

- **A red pull request of a save stays open, and says which checks failed.** A failed check of
  any kind, required or not, leaves the pull request open and unmerged. The Saving view names
  each failed check with a link to it, the title bar's save mark turns to the alert, and a
  repo's row says the same. Saving is not blocked: your next save goes onto the same pull
  request. A cancelled or timed-out check counts as failed; neutral and skipped ones pass.
- **A `pr-merge` save no longer turns on the forge's auto-merge.** A branch that lands through
  a merge queue or a merge train is put in it by purlis once its checks have passed.
