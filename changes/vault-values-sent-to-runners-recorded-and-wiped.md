### Added

- **Your desktop records each run's vault values it sends to a runner.** The record,
  `secret.forwarded`, names the vault, the keys (never a value), the runner, its pool, the chat,
  the persona and the tool, and the runner's own record of the run says its values came from your
  desktop, naming the same request. Like every record of a secret handed out, it is kept until it
  is audited (#3406).

### Fixed

- **A chat that stays on a draining runner runs its tools again.** Once a pool moved on to a new
  runner, a chat still on its older runner was sent no vault value for its declared tools and no
  browser login. It is now judged by its pool and that runner as they are kept, and asked about as
  any runner chat's tool is (#3417).

### Security

- **Vault values handed to a run are wiped from memory as they go**, on the runner and on your
  desktop: as the window reads them, as the run reads them, every copy a run is prepared with, and
  what the window measures before it sends them, on success and on every refusal (#3420).
