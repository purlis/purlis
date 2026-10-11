### Fixed

- **A file an agent writes just as you open a folder shows up.** When finding the branch for the
  folder's watch took longer than two seconds, the explorer drew the folder without waiting, and a
  file made before the watch held was not drawn until something else changed there. The folder is
  now read once more when its watch holds (#1189).
