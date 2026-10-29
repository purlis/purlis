### Added

- **Your desktop records each run's vault values it sends to a runner.** The record,
  `secret.forwarded`, names the vault, the keys (never a value), the runner the chat's link
  reached, its pool, the chat, the persona and the tool, and the runner's own record of the run
  says its values came from your desktop, naming the same request. Like every record of a secret
  handed out, it is kept until it is audited (#3406).

### Fixed

- **A chat that stays on a draining runner is asked about as the runner it runs on.** Once a pool
  moved on to a new runner, a chat still on its older runner had its tools' values, its browser
  logins and the person's Ask bound to the pool's new runner. The session host now tells the
  window which runner a link reached, and such a chat is judged by its pool and that runner as
  they are kept. With a session host from before this, such a chat is sent no value until it
  moves (#3417).

### Security

- **Vault values handed to a run are wiped from memory as they go**, on the runner and on your
  desktop: as the window reads them, as the run reads them, every copy a run is prepared with, and
  what the window measures before it sends them, on success and on every refusal (#3420).
