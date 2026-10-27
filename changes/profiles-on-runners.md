### Added

- **Your profiles go with you to a pool's runner, and each is signed in there once.** Adding a
  Docker pool now ends with this machine's profiles, each kept on the pool's runner, with Sign in
  for each not signed in there yet. Sign in runs the harness's own sign-in on the runner from a
  shell tab; the runner keeps it, and nothing of this machine's sign-in is copied there. Your own
  Claude Code, Codex and opencode folders are the runner's own default folders there, so a runner
  you signed in before stays signed in. A chat started on a pool, a moved one too, starts
  silently on a profile signed in there; one that is not waits in the Inbox as *Profile <profile>
  on pool <pool>*, with *Sign in on pool <pool>*. Settings › Profiles says where each profile is
  signed in: this Mac, and each pool. For now this is for pools on this machine's own Docker, and
  for profiles signed in by a subscription (#2049).
