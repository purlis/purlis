### Changed

- **More tools' folders join the names a chat doesn't write, and you can allow one for one chat.**
  The folders fish, Neovim, bash completion and the session bus load code from under
  `~/.local/share` are now held. At the default sandbox level, a block on one of these, or on the
  `gh`, `glab` or `kubectl` settings, offers **Allow for this chat**, naming the folder whole.
- **`.claude/rules` is held in every folder**, as `.claude/commands` is, with no Allow. A chat can't
  write a repository's rules, and a chat's update of a clone that changes them is refused; make that
  change in your own terminal.
