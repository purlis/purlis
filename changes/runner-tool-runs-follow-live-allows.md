### Fixed

- **A runner chat's tool reaches the hosts you allowed for the chat since it started.** Allow on
  a block's Notice for a chat on a pool now reaches its declared tool's next run on the runner,
  and taking the Allow back narrows that run again, as for a chat on this machine (#3407).
- **A runner chat's tool Ask goes once its command has gone.** When the link to a pool's runner
  goes down, each command of its that waits on your Allow is let go of: its Ask leaves the Inbox,
  no vault is read for it, and nothing is sent. Values you allowed for a command that went
  meanwhile are never sent (#3408).
- **Keep blocked also ends a host's earlier Allow for this chat** (#3407).
