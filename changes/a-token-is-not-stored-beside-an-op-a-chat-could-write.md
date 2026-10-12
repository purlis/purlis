### Security

- **A 1Password token is not stored while the only `op` found is where a chat can write.** The
  token used to be stored with no program pinned, and every read was refused until it was given
  again. Storing it from a vault's tab, moving it from the environment, or setting up a vault
  with a token is now refused with nothing written, and the message says where `op` was found
  (#1517).
