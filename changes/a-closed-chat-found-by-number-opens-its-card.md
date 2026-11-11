### Added

- **A closed or finished chat, found by its number, opens its card.** Type `641` or `#641` in the
  command palette. When that chat has finished or closed, its row opens a card beside it. The card
  shows "#641 · <its task>", how and when it ended, who started it, where it ran and its report.
  *Read it* opens the task's last screen, read-only, in a tab. *Reopen* starts its conversation
  again as an ordinary chat. A chat started again under another number is found under its new one.
- **A dispatched task's last screen is kept when its chat closes**, on this machine and never in
  the committed project, so *Read it* can show it. No chat can read it. It goes when the task's
  dispatch record is collected. Set `[task_screens] keep = false` in `charter.local.toml` to stop
  keeping them. Turning it off deletes every kept screen.
