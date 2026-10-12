### Added

- **`purlis doctor` names an extension still on the old manifest name.** An extension whose
  folder holds `charter-extension.json` and no `purlis-extension.json` gets an
  `extension manifests` row asking for the rename; the old name is read for one more release
  (#1802).

- **`purlis doctor` says when the sandbox drops a forge host.** With the sandbox and its `forge`
  preset on, a `[[forge]]` host sandboxed chats cannot reach gets a `sandbox forge hosts` row, as
  Settings already said (#1405).

- **`purlis doctor` shows the size of a project's sandbox cache home.** Where a project's
  sandboxed chats have a package cache home on this machine, a `sandbox cache home` row says how
  much disk it takes (#1364).

### Fixed

- **A chat too long to start is refused in plain words.** Every start now measures its program's
  words with the whole environment it starts with, and refuses one the system would not start,
  before anything is opened, instead of failing with `E2BIG` (#1425).
