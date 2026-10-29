# Amended 2026-10-29: an agent's GitHub token is minted from the organisation's own App (#2053)

Ruled 2026-10-29 by the person, on #2053 (E1 to E5, accepted as recommended). This builds SD-7a
on this machine and settles what 0077 §7 and §8 left to it. Nothing here changes the human's
sign-in: purlis's own registrations still mint nothing for an agent, and the person's forge
token still never reaches a chat.

## §7, amended: where SD-7a keeps the App's key

§7 said "Where SD-7a keeps that App's private key is SD-7a's to decide, with its tier; it is
never purlis's sign-in item." **Decided: it is a vault value the person holds, and the tool line
that mints with it marks it, so purlis never hands it over raw** (E2).

- **The declaration** (E1) is a persona's tool line in the committed project file:
  `gh = { vault = "<vault>", mint = "github-app", app = "<client id>", key = "<key>" }`. The
  client id is public and is the JWT's issuer. The program is handed the minted token in
  `GH_TOKEN` and nothing else (`docs/plane-format.md`, `[sandbox.personas.<persona>].tools`).
- **The mark.** §7 kept the forge token out of every vault because "a gate still opens": an
  approved request resolves a vault value into a chat's command. The App's key mints tokens for
  every repo of every installation, so it is held to the same bar by another route: every read of
  a vault value that would hand it to anything but a mint refuses a key a `mint` line names in
  that line's vault. That is `secret exec`, `get` and `cp`, any other tool, a browser login, a
  profile's credentials, an approved MCP server's line, a `--dotenv` entry naming it, and the Vaults
  tab's reveal and copy. No approval opens it. A line marks its key whether or not the line is
  otherwise taken, so a line with a typo still marks it. The only read is the mint's, in the app's
  process, and the key goes when the JWT is signed.
- **What reaches the chat** is the minted token alone. It is the agent's own identity, limited
  to the chat's repos, and it lasts an hour. As with every tool, the chat's arguments are its
  own, so gh can show the token it holds. That is accepted for a token of this reach.
- **The chat's repos** (E3) are the GitHub remotes of the clones in the chat's workspace, read
  when the token is minted. A token is minted for one installation, with `repositories` limited
  to those remotes. The installation is the one the run's clone is on, or else the only one the
  remotes are on.
- **A runner chat** with its desktop present is sent the token its desktop minted, through the
  same per-run Ask (ADR 0078 §7, #2047). The desktop never sends the key. Without its desktop a
  runner chat's run waits for it, until a pool opts in (E4, below).

## §8, amended: no new store

SD-7a adds no store. The key is a value in a vault the person already holds, in that vault's own
store and tier (ADR 0069's rows for each vault provider). A minted token is kept for its chat in
the app's memory alone, until ten minutes before GitHub says it expires, and is never written.
The JWT is made for one mint and dropped.

## E4: a pool may let its cluster mint (to be built)

A pool opts in with `Pool.spec.forge.appKeySecretRef`, which names an admin's Secret, and a matching
flag in the pools' file. The operator then mints per-claim tokens, so a runner chat pushes and
opens pull requests with its desktop away. Its keys are documented, and the operator built, in
their own change.

## SD-7b: GitLab, for now

GitLab mints no token that lives less than a day (E5). So for now a chat's own GitLab identity is
a project or group access token the organisation makes, kept in a vault and declared as any tool
is: `glab = { vault = "<vault>", as = "GITLAB_TOKEN env", key = "<key>" }`. Minting one per chat
waits until GitLab mints one shorter than a day.
