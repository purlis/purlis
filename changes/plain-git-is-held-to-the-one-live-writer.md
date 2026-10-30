### Changed

- **A chat's plain git is held to a branch folder's one live writer too.** A chat started
  without the sandbox that runs `git commit`, `git push`, `git pull` or `git rebase` (or a merge,
  cherry-pick, revert or am) in a branch folder another live chat holds is refused before the
  command runs, naming the chat that holds it, in the words `purlis worktree commit`, `update` and
  `push` are refused in. The first chat to write in a free folder takes it. When the app
  doesn't answer, the git runs. Your own terminal, and a harness you start yourself, are never
  held to it (#3931).
