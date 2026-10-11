### Changed

- **A credential file's database value reaches its host through the tunnel.** A brokered
  `purlis secret exec` now points a value a `--file` holds alone, and each value of a
  `--dotenv` file, at the tunnel as it does an `--env` value, in the file itself; every other
  line stays as it was (#1708).

- **More certificates checked by name are handed as they are.** `amqps://` URLs, CockroachDB
  URLs with `sslmode=verify-full`, and `postgresql+asyncpg://` (or another driver that is not
  libpq's) with `verify-full` are handed to the command unchanged with a note, instead of being
  pointed at a tunnel whose address the certificate would not match (#1708).

- **A background command's output read back with another tool is read for a sandbox block.**
  A Claude Code chat that reads a background run's output file with its file tool, or with the
  harness's own tool for background output, now raises the same Notice as reading it with
  `cat` (#1683).

### Security

- **A held ask is not dropped.** A burst of new hosts a chat's connections ask about no longer
  pushes out an ask whose connection still waits on your answer (#1709).

- **A granted folder's path is checked.** purlis refuses to grant a folder whose name it could
  not show as written (#1709).
