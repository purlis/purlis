### Added

- **Your own workspace files can be written from Explorer.** With the workspace picked, a folder
  of yours offers **New file…**, **New folder…** and **Import files…**, and any file or folder of
  yours offers **Rename…** and **Delete…**. The view's head has New file, New folder and Import
  files for the top of the workspace's folder. Delete sends to the system's Trash after one
  confirmation. Where there is no Trash it is refused, and nothing is deleted for good. An import
  copies the files you pick, and a name already taken gets a ` (2)` suffix. A rename or delete of
  a file a chat is writing says so first. Repos, branch folders and the files purlis keeps stay
  read-only, and no write reaches outside the workspace's folder (#5376).
