### Fixed

- **Chats of one project put back at a launch no longer fail to start because another started
  beside them.** Each start's write of the project's package-cache settings has a file of its
  own, so a chat is no longer refused with "could not make …/cargo/config.toml … No such file or
  directory" and left for Retry now (#4980).
