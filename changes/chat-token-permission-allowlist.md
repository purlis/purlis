### Security

- **A chat's GitHub token carries only what a chat needs.** The token minted for a chat now asks
  GitHub for contents, issues, metadata and pull requests alone, so a permission the App gains
  later (administration, actions, statuses, deployments) is not on it. An App missing one of them
  mints nothing and says which, rather than minting wider (#7206, #7133). Deleting or editing a
  release is still open to a chat: GitHub gives it no permission narrower than contents.
