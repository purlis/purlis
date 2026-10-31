### Changed

- **Removing a pool ends its links at once.** Chats still on a pool's runner end as they do when
  their host goes, and the next chat reaches the runner kept under that name now.
- **Tighter checks before a vault value or token goes to a runner.** A cluster runner whose claim
  or pool is stopped, or whose operator no longer says it vouches for it, is sent nothing. A kept
  claim's pod made again asks again for a browser login. A pool that a chat ran on without the
  sandbox is sent no tool values, tokens or browser logins.
- **A cluster operator's statement may hold at most fifteen minutes ahead.** A desktop whose clock
  runs more than about five minutes behind its cluster is refused, in words that say to check it.
- **A kubectl or Docker command given a time ends what it started** when that time is up, such as
  a credential plugin that hangs.
