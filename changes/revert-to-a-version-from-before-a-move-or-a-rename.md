### Changed

- **Revert works on a skill's versions from before a move or a rename** (#4017). The version's
  files come back in the folder the skill has now, as one commit, with the same checks as any
  revert. A version from before a rename comes back with its `name:` line changed to the name the
  skill has now. The History tab's confirm says where the version is from and what changes.
