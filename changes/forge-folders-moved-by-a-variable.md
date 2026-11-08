### Security

- A sandboxed chat no longer writes gh's or glab's settings and extensions where your environment
  moves them (`GH_CONFIG_DIR`, `GLAB_CONFIG_DIR`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`), as it
  already could not at `~/.config/gh`.
- A runner clears opencode's `package.json` and `node_modules` in a kept profile's settings
  folder each time it keeps it, so a chat there leaves nothing for the next one's plugins to
  load. opencode makes them again as it starts.
