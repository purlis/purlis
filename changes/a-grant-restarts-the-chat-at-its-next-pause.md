### Changed

- **Allowing a folder or a socket restarts the chat at its next pause, and says first what will
  stop.** The Ask, and its row in the Inbox, names each program the chat started that listens on
  a port ("its node on port 5173 will stop") and counts the others, or says plainly that purlis
  cannot tell. A chat whose program ended first is not restarted. Restart now still restarts it
  at once (#1986).
