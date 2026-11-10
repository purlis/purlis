### Changed

- **A task you asked for notifies you when it fails, or when it finishes after a long while
  with you away.** A failure names the task, its project and the first line of what went wrong.
  A finish notifies only after more than 5 minutes of work while no purlis window is in front;
  the minutes are `notifications.finishedAfter` in the layout file. Nothing else in Updates
  notifies, and the bell counts as before. A task one of your chats dispatched reports to that
  chat and notifies you of nothing. While Quiet these wait, and the one notification afterwards
  counts them ("While you were quiet: 2 finished, 1 failed"). A click opens the task's line in
  Updates, its report open (#6005).
