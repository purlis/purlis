### Fixed

- **A long message opened in a short pane stays in the waiting strip's room.** It scrolls inside
  the strip, so the terminal below keeps its share of the pane (#4843).

### Security

- **A secret purlis reads after a chat said it is masked in what that chat said, too.** The
  waiting strip, the Inbox card and the quiz no longer show it until the chat's next turn
  (#4843).
