### Fixed

- **Undo a turn and Discard keep what the file was.** Overwrite on a discard keeps a file's CRLF
  line ends where the repo converts them, instead of writing every line with LF. Undo puts back
  an executable bit the turn changed, and is refused when a file's mode changed since the turn
  ended. Choosing a turn whose diff could not be read no longer offers Undo of the turn shown
  before it.
