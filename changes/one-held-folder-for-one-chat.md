### Changed

- **You can allow a held tool folder for one chat.** At the default sandbox level, a chat blocked on
  the `gh`, `glab` or `kubectl` settings, or on the folders below, offers **Allow only for this
  chat**, naming the folder whole, and never Always. On macOS, a Codex or opencode chat may then
  write that one folder. Claude Code's sandbox can't allow one folder alone, so a Claude Code chat
  may write that name in any folder for its run; the Notice says so. A chat purlis wraps on Linux,
  and a chat on a runner, still get no Allow here.
- **The folders fish, Neovim, bash completion and the session bus load code from** under
  `~/.local/share` are now held from chats, and can be allowed the same way.
- **`.claude/rules` is held in every folder**, as `.claude/commands` is, with no Allow. A chat can't
  write a repository's rules, and a chat's update of a clone that changes them is refused; make that
  change in your own terminal.
