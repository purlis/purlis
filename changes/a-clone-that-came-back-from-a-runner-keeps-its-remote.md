### Changed

- **A repository the chat cloned on a runner comes back with its remote.** Move here gives it the
  `origin` the runner's clone had, where that is a forge's HTTPS or SSH address, so its branch can
  be pushed here; the chat's tab names it. Where there is none, the tab says why and how to add
  one (#3985).
