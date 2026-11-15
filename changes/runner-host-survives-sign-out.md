### Added

- **On a machine you reach over SSH, purlis keeps running after you sign out, and starts again
  after the machine restarts.** Run `purlis serve --install` there: on Linux it becomes a
  systemd service of your own and stays on after you sign out (purlis turns that on, or says
  the one command to ask an administrator for), and on a Mac it becomes a LaunchAgent. Chats on
  that machine keep running when your SSH connection drops. After a restart the chats that were
  running are kept as ended, and new ones can start straight away. In a container, run `purlis
  serve --foreground` as its command instead. `purlis doctor` there says whether purlis is kept
  running, whether it stays on after you sign out, and whether it answers (RR-15, #724).
