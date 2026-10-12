### Security

- **The guards read busybox and `coproc` through to what they run.** A command busybox runs as
  an applet, or one `coproc` runs, named or not, is read as the command it is, so an ask or deny
  rule and a consent rule hold behind either, as they do behind `env`. A string handed to
  busybox's own shells, `ash -c` and `hush -c`, is read as `sh -c '…'` is (#1325).
