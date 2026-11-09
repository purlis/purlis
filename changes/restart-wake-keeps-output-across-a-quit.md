### Fixed

- **After a quit, a chat is told what its background shells last printed.** Its first message
  after the quit used to say "output gone" for each shell and monitor, because the chat's own
  temporary folder, where that output is written, went with the chat. A chat that ends with work
  of its own now keeps the folder until its next run ends, or until it is closed (#5827).
