### Fixed

- **A LIVE workspace's files say what git will actually commit.** *This machine only* and *Saved
  with the project* are now git's own answer, so a file git still tracks is marked saved, and a
  nested `.gitignore` or `info/exclude` is honoured. A saved name spelled in decomposed Unicode,
  as names from an older Mac disk or an archive are, is now committed. Removing a workspace drops
  what it saved with the project, so a new workspace of the same name shares nothing you did not
  pick.
