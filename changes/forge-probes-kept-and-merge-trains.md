### Added

- **A GitLab repo with merge trains joins its train.** From GitLab 19.1, saving a merge request
  on a repo with merge trains adds it to the target branch's train, and the save says so (#805).

### Changed

- **purlis asks a forge what it can do once in a while, not on every question.** A probe's answer
  is kept in memory for 15 minutes, for the account, surface and host that asked it, and is never
  written to disk. A refusal is asked again next time. A question the forge refused or did not
  answer now says so, apart from one purlis never asked (#802).
- **A merge-train repo on a GitLab older than 19.1 is no longer set to auto-merge.** Before 19.1
  GitLab's auto-merge call merged past the train, so purlis sends nothing and leaves the merge
  request open for a person, with the reason. An instance that does not say its version is
  treated the same way (#805).
