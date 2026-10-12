### Added

- **Guided steps from the welcome to an open project, picked up where you left off.** The
  welcome now walks you through setting up, one step at a time, and says where you are ("Step 2
  of 4"): the welcome, signing in when you choose to, creating, getting or opening a project,
  installing git when this machine has none, and picking your harness, before the project
  opens. Pick your harness shows the Set up a harness rows for the new project: what is
  installed, Install for what is not, and a word that each harness asks for its own sign-in.
  The sign-in and the harness steps have **Skip for now**, and every step can be done from the
  keyboard. If you quit in the middle, the next launch opens at the same step. Your place is
  kept on this machine only, in `onboarding.json` beside the machine store, and nothing secret
  is ever in it. After the first time through, the harness step comes back only while no
  harness is installed (#1817).
