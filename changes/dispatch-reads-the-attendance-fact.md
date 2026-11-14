### Changed

- **A dispatch reads whether a chat is attended from one fact.** A task another chat
  dispatched now counts as a chat nobody is at, so its own dispatches go only by grants that
  already stand, and the rest are refused with nothing held for the person. A chat started with
  its prompts off, or whose harness reported them off, still counts as one nobody is at, and
  without a sandbox such a chat still dispatches to no persona but its own (R-110, R-113,
  #6410).
