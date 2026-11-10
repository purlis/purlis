### Added

- **An extension can ship artifact templates you pin to a workspace.** Under the new `artifacts`
  capability (protocol 8), an extension declares page templates in its own folder.
  `purlis artifact templates` lists what your approved extensions offer, and
  `purlis artifact pin --template <extension-id>.<name>` copies one into the workspace's
  `artifacts/` and pins it as any page is: refused when it holds something shaped like a secret
  or is too large, and its script asks each person before it runs. Not on Windows (ADR 0122,
  #5973).
