### Fixed

- **The first new chat after purlis starts opens at once.** The picker's first answer is read
  as soon as a project is open, and again once its chats are back, so the first open no longer
  reads it behind the restore of every chat; and it no longer waits for the sidebar's re-read of
  the project before serving what it kept. A slow first answer's line in the slow-answer log now
  names how long each of its phases took.
