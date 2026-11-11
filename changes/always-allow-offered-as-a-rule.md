### Added

- **Always allow can be a rule, and undone.** After the same Allow twice to a reviewed program,
  its next ask can offer Always allow as the rule, and shows the line it adds to the persona's
  tools first. Undo in **Rules I made** takes that line out of the project's file again, and
  leaves the rest of the persona's list allowed.

### Fixed

- **A risky ask never offers a rule.** An ask marked high risk, such as a host that looks like
  production, no longer offers to make its Allow a rule.
