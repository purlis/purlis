### Fixed

- A secret ask from a chat purlis started no longer fails with "the app that started this chat
  did not answer in time". The chat took its own harness for an unknown one and stopped waiting
  after 25 seconds, while the app held the ask for 90. It now waits past the app's hold and hears
  pending, filled or declined (#5192).
