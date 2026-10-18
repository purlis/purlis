### Changed

- **A New tab asks git about `charter.local.toml` once, not about seven times.** Opening the
  picker and starting a chat each checked whether git would carry the project's local file
  every time something read that file, and each check is a `git status`. Each now asks once and
  gives every reader that answer, and the next one asks again. The check refuses exactly what it
  refused before (#2179).
