### Fixed

- **A menu row that turns unavailable under the keyboard says why.** The row used to be drawn
  afresh as it gained its reason, so on macOS and Linux, where purlis runs in WebKit, the tooltip
  did not show. The row now stays as it is, the keyboard stays on it, and its reason shows at once
  (#4659).
