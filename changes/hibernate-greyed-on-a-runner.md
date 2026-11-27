### Added

- **Hibernate is in a chat's menu.** A chat's menu, on its sidebar row, on a right-click and
  under its pane's ⋯, now has Hibernate as the first row under Stop, above the ways to stop that
  end the conversation. Before, Hibernate was only in the command palette (SC-4, #7877, #1391).

### Fixed

- **Hibernate is greyed on a chat that runs on a runner.** A chat on a pool's runner, local or
  remote, was offered Hibernate, and the act was refused. Its menu, under Stop, and the palette
  now show Hibernate greyed, with the reason: it runs on a runner, not on this machine (SC-4,
  #7877, #1391).
