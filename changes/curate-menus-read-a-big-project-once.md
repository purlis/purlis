### Fixed

- **The Curate menus no longer take seconds on a big project.** Their actions were read again for
  each workspace and persona; with 1,000 workspaces the window waited 1.5 s, and 35 s while the
  project opened. They are now read once (#5486).
