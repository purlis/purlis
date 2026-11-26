### Fixed

- **A chat's command no longer waits out its whole deadline when the app fails while
  answering it.** The app kept the command's connection open after the answer failed, so the
  command waited, then said it could not tell whether anything ran. It now hears at once that
  nothing answered.
