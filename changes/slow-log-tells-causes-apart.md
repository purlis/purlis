### Changed

- **The slow log tells a stall's causes apart.** A stall in `slow.jsonl` now says how much of
  it React's renders took, names each region by its own time and its commits (PlaneView's body
  apart from the side views inside it), counts the core's events once each apart from the
  listener calls they made, and marks a chat's pane mounting and a tab switching. Each change of
  whether the window can be seen is logged too. Each kind of line keeps its own threshold and
  cap, so stalls and slow answers each keep at least the 2.3 hours they covered before. The
  `scripts` field, which WebKit never filled, is gone (FS-M2, #7891).
