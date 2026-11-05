### Added

- **An agent hears what you changed at its next turn.** When you discard a hunk of a chat's
  change, that chat's next turn starts with one line of context naming the files you changed
  since its last turn, paths only and never their contents, so it reads them again before it
  builds on code that is gone. Changes from several sittings come in that one line, it is never
  sent as a prompt for you, and it is not repeated at the turn after (#5409).
