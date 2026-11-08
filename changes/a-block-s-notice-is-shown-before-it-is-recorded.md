### Fixed

- **A sandbox block's Notice shows at once.** The app tells the window of a block before it
  writes the block to this machine's network record, and that record is written under a lock in
  purlis's own folder, which no chat can hold, so a chat holding the record's folder no longer
  delays a Notice by ten seconds or keeps the block out of Blocked lately.
