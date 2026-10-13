### Added

- **`purlis doctor` reads the block that hides purlis's files in a clone.** The `workspace
  layer` row now has a line for each clone's and worktree's `.git/info/exclude`: missing or
  stale (`purlis workspace reinit --all` writes it), keeping a line purlis cannot prove it no
  longer needs, or leaving one out because it would hide a file of yours. It also names a
  machine-local settings file purlis is holding back for that reason, a copied agent or skill
  the next `reinit` takes out, a record purlis could not write there, and a record that
  repository commits. Each is said with what clears it, and nothing is written (#998).
