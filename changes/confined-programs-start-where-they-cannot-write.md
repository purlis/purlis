### Security

- **A studio extension's program starts programs only from folders it cannot write.** Its state
  folder and temporary folder hold data; a program it runs lives in its own folder or the
  system's. On Linux it also makes no file in memory (`memfd_create`).
