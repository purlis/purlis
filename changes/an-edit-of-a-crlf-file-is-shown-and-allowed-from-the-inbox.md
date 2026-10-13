### Changed

- **An edit of a file with CRLF line endings can be allowed from the Inbox.** Its change is drawn
  with `␍` at the end of each line that ends in CRLF, and says so, where before it was left to
  the chat's pane. A carriage return anywhere else still sends the edit to its pane (#1146).
