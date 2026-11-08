### Security

- **What a profile sends to a runner is read in more of the ways a credential is written.** An
  address's signing query value (`?api_key=…`) is emptied wherever it is, an address written with
  JSON's escaped slashes loses its user information, a Codex server whose address is signed goes
  whole, and so do permission rules, `--key`, `-p` glued to a MySQL password, and settings named
  for a credential (`apiToken`, `GH_PAT`). Codex's provider query parameters and a profile's own
  shell policy variables are removed, and an `.mdx` file is filtered as Markdown. A script or
  other file sent as written that holds a value shaped like a token is refused in words, naming
  the file, so nothing is sent until the key is moved to a vault.
