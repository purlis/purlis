### Fixed

- **Saving settings reads the project's settings file as a chat's start reads it.** A file past
  1 MiB is refused with a sentence instead of read whole under the project's lock, and a save of
  the machine-only file reads the shared one with no link followed and without waiting on it
  (#1100).
