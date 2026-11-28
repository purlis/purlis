### Changed

- **Switching tabs does less work in a busy project.** The Changes view, the panels and the
  explorer are no longer drawn again when you switch tabs, since nothing they show changes. A
  change to the project's settings draws them once at most, where it drew them three times (#7910).
