### Fixed

- **A chat starts faster in a project with many workspaces.** The list of other workspaces a
  chat is told about at its start counted every memory of every workspace first: 30 ms of each
  start at 1,000 workspaces, and 430 ms with a cold disk. The project index now answers it from
  one lookup per workspace, and is rebuilt once after the update (#5486).
