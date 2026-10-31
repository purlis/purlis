### Security

- **A studio extension's program starts programs only from folders it cannot write.** That is the
  system's program folders and the extension's own folder, less its state folder and temporary
  folder, on macOS and Linux. An extension that runs an interpreter installed elsewhere, such as
  under a home folder, needs it in one of those. On macOS it also loads no native library from
  its state or temporary folder, and on Linux it makes no file in memory (`memfd_create`).
