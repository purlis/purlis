### Security

- A sandboxed chat no longer writes gh's or glab's settings and extensions where your environment
  moves them (`GH_CONFIG_DIR`, `GLAB_CONFIG_DIR`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`), as it
  already could not at `~/.config/gh`.
