### Changed

- **Blocked lately is now the Blocked log.** Settings › Sandbox › Network lists what was blocked
  for chats in this project in the last 30 days, read from this machine's event log, newest
  first. Each row says who blocked it (the sandbox, purlis's guards, the harness's own check)
  and what it was aimed at: a host, a folder in the project, a service or a guard's rule. A
  refusal that is still waiting on you stays in Asks, and one you allowed is not listed. Allow
  is offered only on a host purlis's own proxy saw blocked. A chat that ended before purlis
  last restarted, and that purlis won't reopen, drops off the list (T1-32, #6423).
