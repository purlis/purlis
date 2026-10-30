### Fixed

- **A workspace's notice card now ends up centred under its chip after the strip scrolls the
  chip into view.** WebKit, which purlis runs in on macOS and Linux, can finish a smooth scroll
  without a scroll event for its last steps. The card stopped where the last event left it,
  short of the chip. It now keeps following the chip until the chip stops moving.
- **Tabbing into a notice card just after opening it from its dot now stays put.** The card
  pulled the keyboard back to its edge for a few frames while it was being placed (#3896).
