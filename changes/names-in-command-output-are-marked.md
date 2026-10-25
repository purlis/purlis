### Fixed

- **Names in more command output show their hidden characters.** A colour code, a
  direction-changing or a zero-width character in a deleted skill's name, a name a skill's
  text uses, a commit's author or subject in `purlis skill versions`, an extension command's
  name or title, a session record's title, a curation action's label, or an earlier
  `worktree done` read from the log, is now written as `⟦U+XXXX⟧` instead of reaching your
  terminal, where it could hide or reorder the rest of the line. purlis's own colour is
  unchanged (#3173).
