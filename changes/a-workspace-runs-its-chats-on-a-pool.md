### Added

- **A workspace's chats run on a Docker pool** (#2046). Once this machine keeps a pool, a
  workspace's settings offer **Runs on**: this machine, or a pool. A chat started in a workspace
  set to a pool runs on the pool's runner and behaves as a local one: its tab, input, asks and
  stop work the same. The workspace's code goes to the runner as the chat starts, with your
  uncommitted work. The runner's branches come back when a chat there ends and when you return
  to the window. Changes lists them under each repo, with the commits not on this machine. Chat numbers
  never repeat between this machine and its pools. Shells still run on this machine.
