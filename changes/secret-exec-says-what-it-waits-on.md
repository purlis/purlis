### Fixed

- **`purlis secret exec` in a chat no longer waits on the app in silence.** If the app hasn't
  answered after 3 seconds, the command now says what the app is doing for it: reading the vault
  from the system keyring or from 1Password, or minting a GitHub App's token. It also says how
  long it will wait. If the app then sends nothing before the command starts, it stops with an
  error before the chat's harness would end it: after 90 seconds on Claude Code and opencode, or
  3 minutes where the harness's own timeout isn't known. The error names what the app was
  waiting on, says nothing ran, and says to retry once the person has answered. Before, a stall
  in the app (as with a `gh` run) hung the chat with no output.
