### Changed

- **Chat settings files are held as integrity paths.** A Claude Code chat whose settings are too
  long for its command line gets them from a file in the project's state folder. That folder is
  now held as purlis's own, like its other records, so no sandboxed chat writes it. Each file is
  named by the digest of what purlis wrote, and purlis reads one back only while it still holds
  that.
