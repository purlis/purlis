### Fixed

- **Dispatch commands no longer say "the purlis app did not answer" while the app is only
  busy.** A list, tell or note used to type queued messages into other chats' panes before it
  answered. Under load that took longer than the command waits, and the message stayed queued.
  The answer now goes back first, and the lines are typed after it. When an answer does run out
  of time, the command says the app is running but busy and names how long it waited. A closed
  connection and an unreadable reply are each said in their own words (#5772).
