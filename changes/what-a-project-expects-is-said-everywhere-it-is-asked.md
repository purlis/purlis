### Fixed

- **The Inbox says "This project expects it."** for a blocked socket or system service the
  project's settings declare, as the chat's own Notice does (#1888).
- **An Allow on this machine shows wherever the question is open.** Settings › Sandbox and the
  project's "This project expects more than this machine allows" Notice read what waits again
  when the other one allows it, so neither offers the same Allow a second time (#1888).
- **A chat owed a restart says the restart waits for its turn as soon as it is mid-turn.** Before,
  a chat whose state was not known yet kept saying to restart it by hand after it started a turn
  (#1888).
