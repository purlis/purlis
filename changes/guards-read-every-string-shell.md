### Security

- **The guards read more shells' command strings, and the project-root guard reads them
  too.** A string handed to the ksh family, `rbash`, `yash`, `posh`, `csh`, `tcsh` or `fish`
  is read as `sh -c '…'` is, in each spelling those shells give their `-c`, so an ask or deny
  rule and a consent rule hold inside it. The guard that keeps the project root on its
  branch now reads a string a shell runs (`sh -c`, `eval`, a here-string) as a line of its
  own, from where that shell runs (#1325).
