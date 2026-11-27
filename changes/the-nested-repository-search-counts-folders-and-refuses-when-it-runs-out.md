### Fixed

- **A chat's start counts the folders inside a repository, and refuses when the search runs
  out.** The search for repositories nested in another's work tree now counts folders, as the
  search outside every repository does, instead of entries, so a repository behind many files is
  found. Where it does run out, the chat is not started, and the notice names the repository
  whose work tree held the most, instead of the start going ahead without the repositories it
  never reached (#6377).
