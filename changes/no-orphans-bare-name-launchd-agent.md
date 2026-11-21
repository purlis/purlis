### Fixed

- CI's check for processes a suite leaves running no longer fails a macOS job on a launchd agent such as `cloudphotod` whose program the kernel names by a bare name: its executable is now resolved before the system-folder skip, and a failure names each process's parent and executable.
