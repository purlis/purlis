### Added

- **A persona's tool can be handed several values from its vault.** A tool that needs a key and a
  secret, or a user name and a password, is declared with each under `values`:
  `[sandbox.personas.<persona>] tools = { aws = { vault = "cloud", values = { AWS_ACCESS_KEY_ID =
  { key = "aws-access-key-id", as = "env" }, AWS_SECRET_ACCESS_KEY = { key =
  "aws-secret-access-key", as = "env" } } } }`. A sandboxed chat runs `aws` as it would any tool,
  and purlis hands it every value in the variable or file declared; the program reads them from
  there. The Allow on each machine covers every value, the Notice and Settings › Sandbox name
  each one, and any change asks again. The one-value form is unchanged (T-1). For curl, which
  reads no credential from a variable, the secrets guide shows curl 8.3's own `--variable` and
  `--expand-user`.
