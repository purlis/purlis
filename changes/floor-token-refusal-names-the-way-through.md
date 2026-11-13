### Changed

- **An unattended run refused for printing the forge token is told how to get through.** The
  refusal no longer opens with the release floor's sentence about cutting a release. It says a
  stored credential is on the floor and names the supported route: keep the token in a vault and
  hand it to the one command with `purlis secret exec <vault> --env NAME=<key> -- <command>` (in
  a sandboxed chat, a tool the persona declares, and `git` only with its `args`), or run the step
  attended. Reading a secret from the keychain or a password manager is told the same. Printing
  the token is still refused (#6314).
