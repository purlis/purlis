### Changed

- **Advice about a purlis variable names it under both of its names.** The nested-plane alert
  says to unset `PURLIS_ROOT` and `CHARTER_ROOT`, and an unattended run with no workspace is told
  to re-launch with `PURLIS_WORKSPACE=<name> CHARTER_WORKSPACE=<name>`, since either name still
  decides while the rename's window is open (#1284).
