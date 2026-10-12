### Added

- **Get a project from your forge by its URL.** Paste a project's HTTPS or SSH URL on the
  welcome's **Get a project from your forge** page, and purlis clones it into
  `~/Projects/<name>`, or into the place you picked last with **Choose a folder…**. The clone
  shows its progress and can be cancelled. A cancelled or failed clone leaves nothing behind,
  and a bad URL, a missing project and no access each say why, with **Retry**. The project then
  opens through the usual trust question. A private project is cloned with your own git sign-in
  on this machine: your credential helper for HTTPS, or your SSH key (#1815).
