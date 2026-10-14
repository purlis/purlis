### Added

- **Resolve a save's conflict from the window.** Where a save stopped because the project's
  own commits and the remote's change the same lines, the Saving view offers _Resolve_: purlis
  replays the commits itself and stops on the conflict. Settle the files yourself or with
  _Resolve in a chat_, which starts a chat that only edits them, then press _Continue_. purlis
  checks no conflict marker is left, stages exactly those files, carries on and pushes. A
  binary file, or one a side deleted, is settled in a terminal. _Skip_ drops a commit that
  settling left empty, and _Abort_ puts the project back where it was.
