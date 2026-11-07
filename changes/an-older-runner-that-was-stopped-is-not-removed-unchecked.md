### Fixed

- **An older runner that was stopped, or whose container was removed outside purlis, is no
  longer removed with its home unchecked.** purlis can't ask a stopped runner what it holds
  without starting it, so the runner stays. Settings › Pools and Devices say it was not checked.
  Start its container and purlis checks it at its next look, or let it go in Devices.
