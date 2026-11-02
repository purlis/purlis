### Changed

- **You can allow the `gh`, `glab` or `kubectl` settings for one chat.** At the default sandbox level
  on macOS, a Codex or opencode chat blocked on one of these offers **Allow for this chat**, naming
  the folder whole.
- **`.claude/rules` is held in every folder**, as `.claude/commands` is, with no Allow. A chat can't
  write a repository's rules, and a chat's update of a clone that changes them is refused; make that
  change in your own terminal.
