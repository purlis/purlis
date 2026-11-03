### Fixed

- **Pools and Devices no longer wait while purlis reads a stopped runner's home.** Checking an older runner's home for chats' conversations could take minutes, and starting a chat on a pool, Replace and Devices waited until it ended. The check now runs on its own. If it keeps failing, purlis waits longer between tries, up to four hours (#4861).
