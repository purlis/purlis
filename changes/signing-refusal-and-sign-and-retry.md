### Added

- **A refusal of unsigned commits says so, where it happened.** When GitHub refuses a chat's
  push because the repo only accepts signed commits, the lane's row in Tasks and the chat show
  "GitHub only accepts signed commits here" with **Set up signing**. Once signing is set up,
  **Sign and retry** signs the branch's own commits, never what's already pushed, and sends it
  again: "Signed and sent ✓". The chat is told plainly it can't set signing up itself (#5630).

### Changed

- **A chat's unsigned commit gets one quiet line**, "Not signed: signing isn't set up on this
  Mac", pointing at the Signed commits card, in place of a warning (#5630).
