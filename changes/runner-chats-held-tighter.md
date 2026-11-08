### Changed

- **A chat on a pool's runner is held to where it runs.** A message or report between a runner
  chat and a task on this machine says which pool's runner it came from. A chat with tasks still
  working on this machine stays here until they end or report. A launch no longer starts a task a
  runner chat asked for: it waits under that chat for Try to start again or End task. The chats of
  one pool hold at most 8 opens and dispatches for the person together, and a runner chat's git
  work is done on its runner, never on this machine (ADR 0078).
