### Fixed

- **A plugin install still on its old names is kept current.** After `purlis migrate --undo`, the
  app's launch refresh repoints that install when its hook binary moves, under the same old names,
  instead of skipping it. It never moves the names, so the undo stays undone (#1305).
