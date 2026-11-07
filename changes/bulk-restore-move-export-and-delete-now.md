### Added

- **Bring back, move, export and delete now several skills from the command line** (#5492).
  `purlis skill restore` and `mv` take several names, `mv` and `export` take `--source <url>`,
  and `purlis skill delete-now` takes skills off the Deleted list now. Each makes its change as
  one step after printing its review and asking you at your terminal. Delete now asks even for
  one skill, since it can't be undone. A chat or a script with no terminal can't make these
  changes: the command writes nothing and puts the change in front of you as a proposed change.
  For a proposed export, you pick where it goes when you apply it.
