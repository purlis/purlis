### Changed

- **The project's sandbox reads faster.** Settings and the sandbox notices ask git once whether
  the local settings file is ignored, where they asked twice, and read the project's manifest and
  the sandbox's locks once, where they read them about 25 and 10 times (#7900).
