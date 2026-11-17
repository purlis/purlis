### Fixed

- **A runner kept by systemd or launchd no longer keeps a host running for every version ever
  installed.** Once a newer version is installed beside it, with its service enabled, an old
  version's host stops itself
  after it has had nothing to run and no desktop linked for a minute. It exits cleanly, so the
  service manager does not start it again until the machine restarts. The next install then
  removes that version: its service first, then its folders. A host still running a chat is
  never stopped or removed. Pruning also now looks for a host in the user manager's runtime
  folder (`/run/user/<uid>`) as well as the one its environment names, so it never takes a
  serving version for a drained one. To go back to an older version, disable or remove the
  newer version's service, and the older host serves on.
