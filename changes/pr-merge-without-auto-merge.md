### Fixed

- **A `pr-merge` save merges where the forge has no auto-merge.** A private repo on GitHub's
  Free plan has no auto-merge, so a save's pull request stayed open and nothing said why. Now,
  purlis merges it itself once it is ready: when the forge says it may merge and every check
  on the pushed commit has passed, asked again on every save and fetch. A failing check, a
  conflict or a review the branch requires leaves the pull request open, and the save says
  why, with its link. GitLab merge requests are merged the same way.
