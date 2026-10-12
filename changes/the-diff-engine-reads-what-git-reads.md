### Fixed

- **Comparisons match `git diff` for four more kinds of change.** A file turned into a link,
  or a link into a file, is drawn as git draws it: the old kind out and the new kind in. The
  `diff` attribute decides whether a file is binary before its bytes do (`-diff`, `binary`,
  `diff`). A working-tree file is compared as `git add` would store it, so a file checked out
  with CRLF line ends shows only the lines you edited. A submodule's pointer that moved, came,
  went or was renamed is listed and drawn as a `Subproject commit` line. No filter or diff
  program a repository names is run for any of them (#704).
