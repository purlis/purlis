### Changed

- **A chat never writes a workspace's `workspace.json`** (#2046). The app keeps a workspace's
  repos and settings there, so a chat's sandbox holds it, as it holds the project's manifests. A
  chat's `purlis clone` and `purlis workspace snapshot` are still recorded there, by the app.
  `purlis workspace rename` and `fork`, and `purlis skill grant --on/--off`, say inside a
  sandboxed chat that they are the person's to run, and change nothing.
