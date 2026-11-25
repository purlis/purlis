### Added

- **Hibernate a chat from its row, and Resume it.** A chat that is waiting at the end of a turn,
  on a harness that can pick its conversation back up by itself, can be hibernated: its program
  ends and its memory is returned, its row reads hibernated, and its pane says Hibernated with a
  Resume button that starts the same conversation again. Its Chat log says it was hibernated and
  woken. A chat that is working, asking or paused, whose harness cannot
  resume by itself, that has work running of its own, that dispatched a task or that is itself a
  task is refused, with the reason in words. Nothing hibernates by itself yet, and a hibernated
  chat is not kept across a restart of purlis (SC-4a, #7630, #1391).
