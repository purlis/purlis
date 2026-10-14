### Added

- **A persona's tool can be handed several values from its vault.** A tool that needs a user
  name and a password, a key and a secret, or a client id and a token is declared with each
  under `values`: `[sandbox.personas.<persona>] tools = { curl = { vault = "elastic-logs-master",
  values = { KIBANA_USER = { key = "kibana-username", as = "env" }, KIBANA_PASS = { key =
  "kibana-password", as = "file" } } } }`. A sandboxed chat runs `curl` as it would any tool,
  and purlis hands it every value. The Allow on each machine covers every value, the Notice and
  Settings › Sandbox name each one, and any change asks again. The one-value form is unchanged
  (T-1).
