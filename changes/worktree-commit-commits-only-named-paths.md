### Fixed

- **`purlis worktree commit <paths>` commits only the paths you name.** In a chat with no
  sandbox or a terminal, it committed everything already staged in the branch folder, under a
  message that described one path. Now anything else staged stays staged and out of the
  commit; `--all` still commits every tracked change (#7928).
