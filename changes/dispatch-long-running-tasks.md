### Added

- **Long-running tasks: `purlis dispatch --long`.** A task started with `--long`, or with
  `long: true` on the `dispatch` tool, works through many pieces and sends progress with
  `dispatch note`. purlis never asks it for a report and never makes one from its last message.
  It ends when it reports `done` or `failed`, or when it is cancelled. `dispatch list` marks it
  long-running and shows its last note. The open-tasks roster marks it long-running too. Plain
  dispatches work as before (#3905).
