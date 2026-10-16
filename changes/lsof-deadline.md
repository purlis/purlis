### Fixed

- **Checking whether a local project is in use no longer waits on a stuck process list.** On
  macOS, purlis asks `lsof` which folders your other programs work in before it moves a local
  project. It now asks `lsof` not to make kernel calls that can block, and gives it 5 seconds.
  When `lsof` takes longer, purlis ends it, leaves the project where it is and says the process
  list could not be read. Before, a stuck `lsof` held the launch with no limit (#2091).
