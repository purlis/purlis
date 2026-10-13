### Added

- **The diff engine can hide whitespace as `git diff -w` does.** Lines that differ only in
  spaces, tabs or line ends count as the same, and the file list stays the same. Where two
  diffs are equally short, a hunk may sit elsewhere than git's (ADR 0084). The Review tab's
  toggle comes with RC-4 (#704).

### Fixed

- **A changed path through git's own folder is named as refused, not left out.** A comparison
  and the explorer's change markers now list such a path, and any other path purlis will not
  read, with its mark, and never open it (#704).
