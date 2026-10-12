### Changed

- **An extension's manifest is `purlis-extension.json`.** purlis read only the old name,
  `charter-extension.json`, and ignored a manifest under the new one. It now reads
  `purlis-extension.json` first, and the built-in extensions ship under that name. A folder with
  only `charter-extension.json` is still read, for one release: rename the file before then. An
  approved extension whose manifest you rename is asked about again, because its files changed
  (#1802).
