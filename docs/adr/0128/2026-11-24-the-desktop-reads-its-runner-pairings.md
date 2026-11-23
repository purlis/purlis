# Amendment 2026-11-24: the desktop reads its runner pairings

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D9 (desktop
forwarding). Refs #7290 (slice J).

## What changed

`RunnerLink`, which D9 calls a stub, now has its desktop side: `forgeroute::pairings::DesktopPairings`.
It is installed by `forgeroute::install` and reads the machine store on each ask.

- **Paired** is a device id and key this desktop pinned at a runner's pairing: a Docker pool's
  runner (unless it answered that this desktop is out of its peers), a cluster pool's runner, or a
  chat's fresh runner.
- **The project is the pairing's.** Only a chat's fresh runner records one, the checkout its chat
  started from, and **by id**: the checkout counts only while it still holds the project the row
  names (`planefile::id_to_hold_by` equals the row's plane, as `FreshOf::is_of` holds it), so a
  project moved away names none, and one replaced at the same path names none only when the
  project holds a stored `[project] id`. For a project with none, `id_to_hold_by` is a hash of
  the path, which a replacement shares: a known limit, the same one `FreshOf::is_of` has. A pool's own runner serves any
  project, so it names none and `for_runner` refuses it: its chats keep their token connections.
  A fresh runner's row with no checkout is refused the same way.
- **The chat is the pairing's.** A fresh runner is its one chat's: `RunnerLink::project_of` takes
  the chat, and a runner asking as another chat is refused.
- **A run is never another's.** `for_runner` refuses a run already live for another peer or for a
  desktop chat, so a runner cannot stop or take over another route; a run of the same peer
  starting again replaces its own.
- **Rows that disagree pair none.** Where more than one row matches a device and key and they
  differ on project or chat, nothing says which is right, so none is served.

## What did not change

Nothing calls `for_runner` yet. **The channel is still not built**: the forwarded-request message
on ADR 0078's link, and Open item 11's confirmation against ADR 0078, stay open. No wire format and
no store changed, so there is no new tier.

## Open for the wiring slice

- **No cap on routes per peer.** A paired runner may start a route for each run the link names
  for it, and each holds a listener, a folder and a relay until `stop` or `stop_peer`. The wiring
  slice decides a limit before anything calls `for_runner`.
