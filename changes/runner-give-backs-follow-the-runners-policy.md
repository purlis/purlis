### Changed

- **A runner gives a chat back a port only while its own policy still gives it.** After a
  re-link, each port a runner withdrew for want of a window is checked against the runner's
  sandbox for the project as it is then. Forgetting a project also removes the name its window
  link kept, so the project opened again takes back none of the old runner chats (#2738).
