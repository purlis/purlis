### Fixed

- **`purlis secret exec` in a chat no longer waits on the app in silence.** If the app hasn't
  answered after 3 seconds, the command now says what the app is doing for it: reading the vault
  from the system keyring or from 1Password, or minting a GitHub App's token. It also says when it
  will stop waiting. If the app sends nothing for 3 minutes before the command starts, it stops
  with an error. The error names what the app was waiting on, and says nothing ran. Before, a
  stall in the app (as with a `gh` run) hung the chat with no output.
