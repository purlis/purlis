### Fixed

- **A clone signs in once, with your purlis account, and has no time limit** (#4079). A clone the
  window runs now signs in with your account for the repo's real server, port included, so a
  self-hosted forge's account is found. A large repo on a slow line takes as long as it takes:
  git is stopped only after five minutes with nothing new, not after two minutes in all. A clone
  lands whole or not at all, so a failed or cancelled one leaves no folder and Retry really clones.
  The chip's card and the Repos row show one percent and the stage, and both offer Cancel. What a
  failed clone says comes from what failed, and the window's words no longer name `gh` or `glab`.
