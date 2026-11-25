### Security

- **A running chat's decisions always use the current policy.** A tightening applies at its next
  check and a loosening at its next check after you press Accept; only the shell sandbox, compiled
  when the chat started, waits for a restart (M75-23, #7032).
- **A chat whose shell sandbox is older than the policy is marked, and offers Restart chat.** A
  hook decision already follows a tightening at once; what a Never, a removed grant, a path rule
  that tightened (a forbid, or a permit that narrowed), a new workspace, the sealed answers' deny
  state or a purlis restart leaves old is the sandbox the chat's shell commands run in, compiled at
  its start. The mark says so, is not "project updated", and the window asks again when the plane's
  `HEAD` moves, a Never or grant is written, or you press Accept (M75-23, #7032).
