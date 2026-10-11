### Security

- **A brokered commit, clone or branch folder refuses more of a repository's own config.**
  The app already refused to run git for a sandboxed chat in a repository whose own config
  names a program. That list now also covers what a commit could start: a signer, a trailer's
  command, and a hook defined in config rather than in the hooks folder. The commit itself is
  told never to open an editor (#1476).
- **`git worktree repair` is let through only where a folder was moved.** purlis's own git
  lets that one command past the check of a folder's git link, so a rename can repair the
  folders it moved. It now covers only the plain command in a folder whose old place is gone,
  and nothing that would repair another folder's link (#1476).
