### Changed

- **The cluster holds the operator to a runner's shape.** A new admission policy refuses its
  service account a template outside purlis's pod shape, and any upstream object outside a runner
  namespace (#3754). A policy made again under its binding, or a binding changed since it was
  made, now counts as made again for Claims and runner namespaces (#3854).
