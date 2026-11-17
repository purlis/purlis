### Security

- **A chat can't make purlis's tools write outside its sandbox by keeping the app busy.** When
  the app a chat runs under was full or slow, purlis's tools took it for an app that had gone
  and wrote the memory or session record themselves, outside the chat's sandbox. A chat could
  cause that on purpose by opening enough connections to the app. Now a full, slow or unsure
  app refuses the write, and nothing is written outside the sandbox (#6995). A person's own
  terminal over SSH, or under a `login` the system owns, is no longer taken for a sandbox.
