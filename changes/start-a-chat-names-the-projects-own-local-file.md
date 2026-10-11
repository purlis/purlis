### Fixed

- **Start a chat names the project's own local settings file.** Where it says git would carry
  the file, or where to declare your own profiles, it named `charter.local.toml` whatever the
  project's file is called. It now names the file purlis read, `purlis.local.toml` or
  `charter.local.toml`, as Settings does (#1719).

- **A busy Undo keeps the keyboard on its line.** While a memory's Undo is on its way, its
  button says it is busy and ignores a second press, instead of being disabled, which dropped
  the keyboard focus to the page (#1190).
