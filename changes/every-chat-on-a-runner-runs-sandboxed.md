### Changed

- **Every chat on a runner starts inside purlis's sandbox.** A Codex, opencode or declared
  harness's chat there that would start without it is refused in words, as a Claude Code chat
  already was, and a runner made before purlis 0.5.0 carries no sign-ins to its pool's next
  runner: those profiles are signed in again once.
- **A sign-in in a shell on a pool's runner lets a drain carry it again**, as the driven sign-in
  does, and a sign-out ends the watch of that sign-in.

### Security

- **A runner's keep takes out the MCP servers a chat added to a project** in Claude Code's state
  file, and a runner that cannot read who listens for a sign-in no longer passes the browser's
  answer on: the sign-in says to sign in again.
- **A keep refused for a linked settings folder names the folder** and says to make it a plain
  folder on the runner, then keep the profile again.
