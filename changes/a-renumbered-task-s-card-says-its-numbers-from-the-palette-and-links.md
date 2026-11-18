### Fixed

- **A finished task's card from the palette or a `chat 702` link has the number it closed
  under**: a task renumbered by a restart before it finished opened as `#702`, or from `chat 731`
  as a bare closed chat without its report. Both now open the task's card as `#731`, saying
  `(was #702)` where you asked for 702 (#7145).
- **A closed chat is listed once in the Chats filter**: a task's chat could show both as its
  finished-task row and as a row under Closed chats. It now shows as its task's row where the
  filter finds that row, and under Closed chats only where it does not (#7145).
- **Two closed chats that once had the same number can be reopened at once**: the second Reopen
  was refused as if it were the first chat starting twice (#7145).
- **Typing a number too large to be a chat's in the Chats filter looks for it as words**: past
  4294967295 it said the closed chats could not be read (#7140).
