### Security

- **A chat's GitHub token carries only what a chat needs.** The token minted for a chat, on a
  desktop, a runner or in a cluster, asks GitHub for contents, issues, metadata and pull requests
  at most, and only those the App's installation has, so a hand-made App without issues or pull
  requests still mints, with a narrower token. A permission the App has beyond them (actions,
  checks, statuses, deployments, administration) or gains later is not on the token, and a token
  GitHub returns with more than that is dropped. A refusal about permissions says none was minted
  and none widened. Deleting or editing a release is still open to a chat: GitHub gives it no
  permission narrower than contents. Until a follow-up wires the person's workflows tick into the
  mint, a token carries no workflows permission, so `gh api` writes to `.github/workflows/*` fail
  for an App with that box ticked (#7206, #7133, #7249).
