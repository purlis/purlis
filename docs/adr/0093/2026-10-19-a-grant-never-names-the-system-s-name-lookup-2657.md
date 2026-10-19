# Amended 2026-10-19: a grant never names the system's name lookup (#2657)

**Decided in implementation (D-2657-1..5), to be confirmed.** Follows
[a refusal at the system's name lookup socket is a lookup](2026-10-18-the-system-s-name-lookup-socket-is-a-lookup-2428.md).

A chat's names are looked up for it by purlis's proxy, so the system's own name lookup is never
something to allow. Its Notice already offers only "start without the sandbox"; this keeps
every grant in step with that offer.

- **No socket or service grant names the system's name lookup** (D-2657-1): Linux's sockets
  (`sandboxblock::RESOLVER_SOCKETS`, which still sorts a refusal at one as a lookup), and the
  Mac's own name lookup is refused too (`services::NAME_LOOKUP_SOCKETS`,
  `services::NAME_LOOKUP_SERVICES`). A path is matched as written, with `.` and `..` read
  lexically, and as the path it leads to.
- **Refused where an Allow is judged, and where one is kept** (D-2657-1). `services::socket` and
  `services::service` refuse it, so a block's Notice, the centralized Ask and Settings refuse
  it at every level; `kept_for`, the one keep, refuses it whichever path reached it.
- **Refused before what is on disk is looked at** (D-2657-2), so a path that is not there on
  this machine is refused for this reason, not as "not a local socket now".
- **Told as one that no grant names** (D-2657-3): the Notice offers the chat without the
  sandbox, and says the rule and why, one sentence each.
- **One kept already reaches no chat** (D-2657-4): a start never compiles it in, the Granted
  list marks its row with the same words and still removes it, and Doctor counts it as held.
- **The Granted list makes no grant of these** (D-2657-5): its part is the marked row.
