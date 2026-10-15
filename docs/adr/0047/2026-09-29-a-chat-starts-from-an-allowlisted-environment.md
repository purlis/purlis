# Amendment, 2026-09-29: a chat starts from an allowlisted environment

The strip above is a list of names purlis knows to remove, so every other variable the app
inherited reached each chat and, through the harness, the model's shell. A chat now starts the
way the extension executor starts a program: from an empty environment plus a keep-list
(`charter_core::chatenv`).

- **The keep-list** is what a program needs to run as the operator (`PATH`, `HOME`, the locale,
  `TMPDIR`, `SSH_AUTH_SOCK`, `XDG_*`, the proxies, purlis's own `CHARTER_*`), plus what the
  chat's harness declares for itself (`Harness::env_passed`: data per harness, so a new harness
  brings its own names), plus the operator's `[chat_env] pass` in the plane's
  `charter.local.toml`.
- **Credentials are held back by class.** Forge, cloud, model-provider and registry credentials,
  and any name a profile's `env` may not use (`KEY`, `TOKEN`, `SECRET`, `PASSWORD`), pass only
  when the operator lists the exact name. A prefix the operator writes does not admit one.
- **What is never passed stays that way.** Every `OP_*`, every identity variable a vault
  declares, a harness's identity and where the launcher's chat was, whoever lists them. `TERM`
  is always the chat's own.
- **Only the local file extends it.** A committed list would let a teammate's push decide what
  of one machine's environment every chat there gets, which is the rule `charter.local.toml`
  already carries for `[harness]`.

`purlis secret exec` stays the way a command is given a credential. What the harness itself
then hands its tools is the harness's own policy.
