### Added

- **`purlis doctor` says when this purlis cannot read the hook spool's keys.** A `keys.json` a
  newer purlis wrote stops every drain of the project's hook spool and every new chat's key.
  That used to be said only in the app's log. Now a `hook spool` row says it and says how to fix
  it. The row reads only the file's version, and drains and writes nothing (#983).
