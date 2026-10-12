### Added

- **A GitLab repo with merge trains joins its train.** Saving a merge request on a repo with
  merge trains adds it to the target branch's train, and the save says so (#805). From GitLab
  19.1 it joins through GitLab's auto-merge. On an older GitLab, or one that does not say its
  version, it joins through GitLab's merge trains API, never through the merge call, which
  merged past the train before 19.1. Where that API is missing or will not take the merge
  request, nothing is merged and the merge request stays open for a person, with the reason.

### Changed

- **purlis asks a forge what it can do once in a while, not on every question.** A probe's answer
  is kept in memory for 15 minutes, for the account, surface and host that asked it, and is never
  written to disk. A refusal is asked again next time. A question the forge refused or did not
  answer now says so, apart from one purlis never asked (#802).
