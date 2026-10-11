### Added

- **Every third-party crate and npm package purlis ships, with its licence text.** About ›
  Licences now lists, under purlis's own notice and licence, each Rust crate and npm package the
  build was made from, grouped by the licence text it ships. The macOS app and the Linux
  packages carry the same list as `THIRD-PARTY-NOTICES` beside `LICENSE` and `NOTICE`. The list
  is generated from the locked dependency graph each time purlis is built, so it cannot fall
  behind, and a build fails when a package it ships has no licence text (#1784).
