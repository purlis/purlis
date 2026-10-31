### Changed

- **A workspace's stale repos and its layout behind hang from its chip** (#3650). A repo left off
  its default branch ("Switch to main") or on a moved remote ("Use the new address"), and a
  workspace behind the current layout ("Update workspace layout"), each show as a ▲ warning on
  that workspace's chip in the title bar, with the same buttons as in the Notices centre. They
  are still listed there. Dismissing a stale repo in either place dismisses it in both, until
  the finding changes. A layout behind has no Dismiss in either place: it goes once it is
  updated.
