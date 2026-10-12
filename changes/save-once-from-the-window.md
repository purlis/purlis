### Changed

- **A project cloned before purlis had its own sign-in says to save it once from the window.**
  On a machine without `gh` or `glab`, its automatic save no longer fails with a bare sign-in
  error: it commits locally, and the Saving view names the project's remote with *Save once from
  here*, which confirms that remote. From then on automatic saves push with your purlis sign-in
  (#1848).
