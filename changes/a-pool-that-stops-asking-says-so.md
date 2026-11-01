### Added

- **The window says when a cluster pool starts minting without asking you.** When a pool's admin
  turns on minting in the cluster, you see one Notice saying its chats no longer ask before each
  GitHub App token, and that the admin controls that. It blocks nothing. It shows once for each
  change, and again if the setting goes off and back on.

### Fixed

- **A chat's several runs of a refused tool on a cluster runner ask you once.** Runs started at the
  same time now wait for the first one's answer rather than each asking. A chat started again under
  the same number no longer waits on the pace its predecessor left.
