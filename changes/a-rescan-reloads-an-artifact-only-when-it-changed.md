### Fixed

- **An open artifact no longer reloads while nothing in it changed.** When macOS drops file
  events under a build's load, an artifact's page reloads only if its files or its data differ
  from what it shows, and the window re-reads the project at most once every five seconds for
  such events.
