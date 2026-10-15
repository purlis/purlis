# Built: the import of an existing CLI login (#1376, 2026-10-12)

§5 is written, as an offer beside the token sign-in on the welcome's sign-in page and in
Settings › You › Accounts. Within this record, these were settled while building it:

- **What is offered:** `gh` or `glab` signed in to its forge's own host (github.com,
  gitlab.com), where purlis has no signed-in account. The offer asks the CLI only `auth status`
  and never for a token. A GHES or self-managed login is not offered yet; those hosts take a
  token (§4).
- **The offer's words are true today (dispatcher, 2026-10-12):** the token is copied into
  purlis's own keyring item, which purlis never hands to a chat; a chat that runs the CLI itself
  can still use the CLI's own stored login, and the offer says to sign out of the CLI to stop
  that. The stronger denial, a chat refused the CLI's stored login on an imported host, is
  SD-2's follow-up. **Not
  now** comes first, a declined offer is not drawn again in that run of the window, and nothing
  is imported unless the person presses **Use my … login**.
- **The import reads the CLI's stored login,** `gh auth token --hostname <host>` or `glab config
  get token --host <host>`, with none of either CLI's token variables in its environment, so a
  token the window was started with is never taken for the login. It is not a network call and
  is not listed in the network log. The answer is wiped as it is taken, registered with
  `keep_out`, checked with the forge as a pasted token is, and kept as the account's keyring
  item with the method `import`.
- **A refusal is in purlis's words,** never the CLI's, which could quote what it printed.
- **Chats lose the login in every process purlis runs for them.** In the window, the resolver
  already refused a chat on an imported host (§7). A process that holds no sign-in, which is
  every process a chat runs, now refuses a chat the CLI transport on a host whose login was
  imported too, reading only the accounts file. A chat that runs `gh` or `glab` itself still
  reaches the CLI's own copy until SD-2 denies it (ADR 0070 §4).
- **A `glab` login made through glab's own OAuth sign-in is refused,** because a GitLab token's
  scopes are read from its own personal-access-token record (#1814). A `glab` signed in with a
  personal access token imports.
