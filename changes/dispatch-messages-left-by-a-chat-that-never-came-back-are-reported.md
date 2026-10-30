### Fixed

- **Messages queued by a chat that never came back after a restart are reported, not cleared
  silently.** Each is kept, marked not delivered, on the dispatch it was between, where the
  Dispatches tab shows it; what no running dispatch can keep is one Notice in the Inbox, naming
  the chat and how many, until you put it away (#3958).
