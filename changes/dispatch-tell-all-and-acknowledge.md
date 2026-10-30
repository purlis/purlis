### Added

- **Tell every task at once, and see who took it in.** `purlis dispatch tell --all "<text>"`
  (or the `dispatch_tell` tool's `all`) sends one numbered instruction to every task of the
  chat still working. A task acknowledges it with `purlis dispatch note --ack <n>` (or the
  `dispatch_note` tool's `ack`), and `purlis dispatch list` shows which tasks have (#3904).
