### Fixed

- **A `pr-merge` save merges where the forge has no auto-merge.** A private repo on GitHub's
  Free plan has no auto-merge, so a save's pull request stayed open and nothing said why. Now,
  when the forge cannot auto-merge it, purlis merges it itself once it is ready: at once when
  the forge says it may merge and its checks passed (or there are none), or once running checks
  pass, asked again on every save and fetch. A failing check, a conflict or a review the branch
  requires leaves the pull request open, and the save says why, with its link. GitLab merge
  requests with no pipeline to wait for are merged the same way.
