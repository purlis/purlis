### Fixed

- A chat can remove a finished branch folder whose squash merge the base has since changed again, or whose base branch is gone. Commits made after the merge still refuse it. Folders only the harness or a build writes, such as `.claude/.cc-writes/`, `node_modules/` and Tauri's generated schemas, no longer refuse it; other ignored files, such as `.env`, still do.
