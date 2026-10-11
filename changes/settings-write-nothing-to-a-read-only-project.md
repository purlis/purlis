### Fixed

- **Settings writes nothing committed to a project this purlis may only read.** In a project
  that requires a feature this purlis lacks, or declares a newer format, a save of the shared
  settings file, a value moved between the shared and the machine's own file, a dispatch grant
  for everyone (given, moved or revoked), a forge added or removed, and a workspace's settings
  are each refused, saying nothing was written and why. The machine's own settings file is
  still written. A shared file that is not valid TOML still makes the project read-only, and
  Edit as TOML is still how it is mended: that one save writes it, and nothing else does
  (#1759).
