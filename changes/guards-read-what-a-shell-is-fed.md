### Security

- **The guards read more of a line's shell strings.** The guard that keeps the project root
  on its branch reads more of what a line hands a shell to run, and every guard that reads a
  script handed to `fish` reads it as fish does (#1325).
