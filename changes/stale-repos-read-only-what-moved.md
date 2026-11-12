### Changed

- **The look for stale repos asks git nothing of a repo that has not moved.** Every look asked git
  four to six questions of each repo in every workspace. A repo whose branch, default branch and
  remote settings are as they were at its last look is now answered from that look, and one that
  moved is read in full as before. The four reads that follow each fetch (the title bar's save
  state, the Saving rows, the Inbox's alerts and the stale repos) now start a second and a half
  apart instead of in the same second.
