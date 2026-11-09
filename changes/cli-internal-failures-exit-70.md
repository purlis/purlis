### Changed

- **More of purlis's own failures exit 70 and are drafted as a Report.** `purlis workspace list`
  and `workspace optimize` where `workspaces/` cannot be listed, `workspace vision` where
  `workspace.md` cannot be written, `purlis skill edit` where its editing copy cannot be made or
  read back, and `purlis dispatch` and `purlis report` where the app or the Reports module answer
  something the command did not ask for now exit 70, the code for a failure inside purlis, in
  place of 1. purlis's tools for dispatched tasks mark the same failure in their error. A refusal
  and a usage error still exit 1.
