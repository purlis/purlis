### Added

- **Your own sandbox level, per project and per persona** (#2071). Settings › Project ›
  Sandbox › Your level sets how much the sandbox holds for your chats in this project:
  full sandbox, floor or no sandbox, for the project and, if you want, for each persona. It is
  kept in this machine's `purlis.local.toml` under `[sandbox] level` and `[sandbox.personas]`,
  never in a committed file, and a value that is not a level is refused with a sentence.
  Chats take it at their start once the floor lands; until then it is saved and shown only.
- **Move a plain-file vault into the keychain.** The same page lists the project's plain-file
  vaults, which a chat with no sandbox could read like any file, and moves each one this machine
  registers into the system keychain: every secret is written and read back before the file
  goes, and a vault the team's file registers is the team's to move.
