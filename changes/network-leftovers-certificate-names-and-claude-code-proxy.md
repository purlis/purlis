### Changed

- **More database clients keep the certificate's name through the tunnel.** A brokered
  `purlis secret exec` honours `PGSSLMODE=verify-full` beside a `postgres://` URL or a libpq
  string that names no mode, gives a SQL Server URL `hostNameInCertificate`, and points
  `MYSQL_HOST`/`MYSQL_TCP_PORT` and `REDIS_HOST`/`REDIS_PORT` at the tunnel as it does
  `PGHOST`/`PGPORT`. A value whose client checks the certificate by name with no way to keep it
  (MySQL's `VERIFY_IDENTITY`, MongoDB over TLS, `rediss://`, a `verify-full` value run by `usql`)
  is handed as it is, with a note naming the variable (#1708).

- **A Claude Code chat starts only its proxy.** purlis no longer makes a temp folder and an ssh
  route beside a Claude Code chat, which never used them (#1699).

- **The old `.purlis/app/sandbox-blocks.json` is taken away** when the project opens: nothing
  has read it since the network record took its place (#1681).

### Added

- **`purlis doctor` says an administrator's local-port setting.** Where managed Claude Code
  settings turn `allowLocalBinding` on, which purlis's own setting does not outrank, the new
  `sandbox local ports` row names the file (#1699).
