### Changed

- **Agents writing the plane without pause make the sidebar read it again about once a second,
  not four times.** The plane watcher now waits for 250 ms with no change before it tells the
  window, and tells it at most 1 s after the first change of a run. A single change still shows
  within about a quarter of a second (#4741).
