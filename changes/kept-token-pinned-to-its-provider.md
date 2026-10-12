### Security

- **A token kept in the keyring is held to the vault's provider and its file of references.**
  A commit that changes which provider reads a shared vault, or the file it reads references
  from, now stops the kept token being used, as a change to its variables or its 1Password
  vault already did. A record made before is held to them from the next time the token is
  stored (#1528).
