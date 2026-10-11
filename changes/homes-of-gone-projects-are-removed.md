### Fixed

- **The cache and Codex homes of a project that is gone are removed.** A project's package
  caches and Codex home under purlis's data home are taken away once the project's folder is
  gone (not merely on a disk that is not mounted) and nothing has used them for a month (#1364).
