### Added

- **Undo a turn.** On the latest turn not yet undone, a file's diff offers *Undo turn N…*, which
  asks once, listing every file it will change, and puts each back as it was before the turn:
  files the turn added, deleted or renamed included, and nothing committed. It is refused while a
  chat is working in the folder, naming the chat and its state, and when a file the turn changed
  has changed since, naming the file. After an undo the turn before it is offered. The agent's
  next turn is told that a turn was undone, by the files' paths alone (#5411).
- **Discard on a turn's diff.** *Discard* is back on one turn's diff, and puts the hunk back as it
  was at that turn's start, not the base. A file you changed since the turn ended is drawn from the
  turn's start to the file as it is now, so each hunk left can be discarded too (#5411).
