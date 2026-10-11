### Security

- **A brokered commit, clone or branch folder refuses more of a repository's own config.**
  The app already refused to run git for a sandboxed chat in a repository whose own config
  names a program. That list now also covers a trailer's command and a hook defined in config
  rather than in the hooks folder. A brokered commit also refuses a repository whose own config
  names a signer, and says so: such a repository means its commits to be signed, and the app
  never signs for a chat, so commit those from your own terminal. A signer you set for
  yourself alone is not read. The commit itself is told never to open an editor (#1476).
- **`git worktree repair` is let through only where a folder was moved.** purlis's own git
  lets that one command past the check of a folder's git link, so a rename can repair the
  folders it moved. It now covers only the plain command in a folder whose old place is gone,
  and nothing that would repair another folder's link (#1476).
