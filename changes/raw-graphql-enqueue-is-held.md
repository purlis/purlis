### Fixed

- **A chat's raw GraphQL enqueue is held with an Ask that names the pull request.** An
  `enqueuePullRequest` that asked its merge queue entry back (`mergeQueueEntry { … }`), as
  `gh api graphql` documents do, was refused flat as a merge with no Ask; it is now held like any
  other merge (#7678).
