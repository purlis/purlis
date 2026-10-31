### Added

- **A Claude Code profile signs in on a Docker pool's runner from its line in Settings › Pools,
  with no terminal.** Sign in opens the line in place: purlis runs Claude Code's own login on the
  runner, opens its page in your browser, takes the code the page shows in one field, and says
  Signed in once the runner's own status check agrees. Each step says where it is ("Step 2 of
  3"). Nothing of this Mac's sign-in goes to the runner. Other harnesses still sign in in a shell
  tab for now (#4042).
