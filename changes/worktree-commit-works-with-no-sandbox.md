### Changed

- **`purlis worktree commit` works in a chat with no sandbox** (#4534). It used to refuse and
  point at plain git, which a harness's own rules can refuse too, leaving the chat no way to
  commit. It now commits there with that chat's own git, with your hooks, signing and filters,
  and a terminal does the same. To bring the refusal back, set `[plane]
  refuse_unsandboxed_commit = true` (Settings › Saving). Sandboxed chats commit as before.
- **A chat the app starts with no sandbox in a project whose sandbox is on is told so** (#4534).
  Your own level can start a dispatched chat with no sandbox while the chat that asked for it
  has one, and until now only its tab said so.
