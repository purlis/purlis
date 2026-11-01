### Fixed

- **A chat at the default sandbox level can use `~/.local` and `~/.config` on a home without
  them.** `mkdir -p ~/.config/sometool` and `mkdir -p ~/.local/share/sometool` used to be refused
  when those folders were missing. Purlis now makes `~/.local`, `~/.local/share` and `~/.config`
  as a chat starts, where they are missing.
