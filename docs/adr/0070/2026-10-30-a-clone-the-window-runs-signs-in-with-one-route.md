# A clone the window runs signs in by one route, with no total deadline (#4079, 2026-10-30)

A repo cloned into a workspace from the window was the last git of the window's that did not
reach the person's account, for two reasons. `Forge::for_record` answers the kind's default host,
so an account on a self-hosted server was never asked about. And `repocmd::clone` runs each clone
on a thread of its own, which the window's scope (`as_the_window`) did not reach, so on those
threads no git was the window's at all.

- **One route.** The forge of a clone is the record's kind at the server its URL names, port
  included (`Forge::for_clone`), and an account is still matched on the server, host and port
  (#1850). Each clone's threads carry the window's scope, as they carry a chat's isolation. The
  CLI's helper stays only where there is no account, and in the window only where the CLI is on
  the machine. A chat's brokered clone still never gets the person's token.
- **Still under the one-credential rule.** The clone runs through `git::spawn_network`, which
  holds `NETWORK_RULE` and differs from `run_network` only in that its caller reads git as it
  runs. It is not the person's own clone of "Get a project" (`spawn_persons_clone`): these URLs
  come from a committed inventory, not from the person's paste.
- **A stall limit in place of a total deadline.** A clone that is not a chat's is stopped after
  five minutes with no new output, and `http.lowSpeedLimit`/`http.lowSpeedTime` let curl end a dead
  transfer. The window offers Cancel. A chat's brokered clone keeps `git::NETWORK`, because
  `gitask` bounds its wait on it (#4080).
- **It lands whole.** git clones into `.cloning-<name>` beside the destination, held by a lock for
  as long as it runs, and a finished clone is renamed into place. Whatever else ends it removes the
  folder. A leftover one, which nothing holds, is replaced; one another run holds is left to it.
- **`clone_repo` takes a channel** and so, as `get_project` did, leaves the UI RPC's client: a link
  has no channel, and a clone writes a folder here with the person's sign-in.
