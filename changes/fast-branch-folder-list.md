### Changed

- **The branch folder list answers at once, however many branch folders a repo has.** It names
  every folder first and reads each folder's uncommitted changes after, several at a time, where
  it used to read them one after another before answering (21.5 s with about sixty folders). A
  folder whose state is still coming says "reading…" in the Explorer until it is in (#5321).
