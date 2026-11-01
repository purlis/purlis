### Fixed

- **A chat at the default sandbox level can make its tools' folders on a home without `~/.local`
  or `~/.config`.** `mkdir -p ~/.config/sometool` and `mkdir -p ~/.local/share/sometool` used to be refused
  when those folders were missing. Purlis now makes `~/.local`, `~/.local/share` and `~/.config`
  as a chat starts, where they are missing.
