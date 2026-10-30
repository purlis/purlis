### Fixed

- **Grading after each edit keeps grading after the first edit of a skill that has a previous
  version.** Each saved edit is committed, so the version a grade compares with is the one before
  that edit, and it moves with every edit. The opt-in no longer pauses when it does. It still
  waits for you when a grade would run on another profile or model, run other trigger tests, run
  more chats or may spend more (#4016).
