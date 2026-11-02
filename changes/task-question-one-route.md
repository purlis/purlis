### Changed

- On Claude Code, a dispatched task's question prompt and a plain `ask_operator` now go to the chat that dispatched it, and the task is told who answered. Handoffs, tasks you started from a tab, your own chats and browser takeovers still ask you directly. If a redirect fails, the question stays in the task's pane and the Inbox.
