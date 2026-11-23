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
  started from. A pool's own runner serves any project, so it names none and `for_runner` refuses
  it: its chats keep their token connections. A fresh runner's row with no checkout is refused
  the same way.

## What did not change

Nothing calls `for_runner` yet. **The channel is still not built**: the forwarded-request message
on ADR 0078's link, and Open item 11's confirmation against ADR 0078, stay open. No wire format and
no store changed, so there is no new tier.
