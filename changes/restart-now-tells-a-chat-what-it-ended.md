### Fixed

- **Restart now on a chat waiting on its own background work goes ahead and tells the chat.**
  Restart now, a restart without the sandbox and a restart for changed project settings end the
  chat at once, and the chat is told in its first message how many shells, monitors, subagents
  and workflows the restart ended. Any other restart of such a chat waits until its work wakes
  it, as the window's already did. After the hour's wait lapses, the chat still shows as needing
  you, and a restart or a move to a runner goes ahead and tells the chat what it ended (#5753).
