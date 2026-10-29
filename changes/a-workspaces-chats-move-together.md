### Changed

- **A workspace's chats move together.** Moving a chat to a pool's runner, or back here, moves
  every chat of its workspace running where it runs, since they share its clones; its tab's row
  names them. Each keeps working until it is at a pause, none starts where it moved before all of
  their work is there, and if one cannot move, none does (#3453).

### Fixed

- **A move back here is no longer refused for a clone the runner never had**: a repository added
  here while the chat ran on the runner stays as it is. A repository cloned only on the runner
  stays there, as Move here's row says (#3454).
- **A refused move says the pool's own reason**: no pool of that name, no runner of its image, or
  its runner draining, rather than always that its runner is not known to be yours. A move that
  failed names where the chat still runs once (#3646).
