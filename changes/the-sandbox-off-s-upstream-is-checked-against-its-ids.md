### Changed

- **The team's sandbox off is named by its commit only where that history checks out.** purlis
  names the commit and author of a project's sandbox off when the commit is on the upstream it
  fetched itself. Each commit on the way from that upstream down to the off is now read back and
  checked against its own id, and so are the author and the manifest the ask names the commit by.
  The off is called local where one does not match, and where the upstream fetched has since
  turned the sandbox back on (#2166).
