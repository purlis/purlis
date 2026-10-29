### Fixed

- **A refusal a chat can fix itself no longer sends it to you.** `git add` or `git commit` in a
  branch folder of a sandboxed chat is now refused before it runs, naming `purlis worktree
  commit -m "<message>" <path>…`, instead of failing on the sandbox and raising a Notice for
  you. And a refusal that already names the shape to retry (a dispatch, handoff or commit
  beside another command, a live substitution in a forge's or purlis's own text, a commit that
  skips the hooks, a handoff's brief) no longer ends by telling the chat to have you run it in
  your own terminal; refusals with no fix the chat can make still do (#3830).
