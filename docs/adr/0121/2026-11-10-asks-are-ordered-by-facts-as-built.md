# Amended 2026-11-10: Asks are ordered by facts, as built

Decision 5 is built (#6006), with these particulars.

- **One pure function orders Asks** (`app/src/asksOrder.ts`, with its table test): the asks are
  grouped by chat, and a group stands by its soonest Expiring ask, then by how many chats wait on
  it (more first), then by its oldest ask. Inside a group, its Expiring asks come first, soonest
  first, then the rest, oldest first. Every key is a time or a count the ask carries, never when
  the window first saw the list, so a new ask takes its place and never the top for being new, and
  an open Answer stays open (it is kept by the ask's key).
- **An ask carries its end** (`Shown.expires`, seconds since 1970), filled by the registry where
  the table in this ADR's Quiet amendment says what ends it: a hook ask at its deadline
  (`Ask::deadline`, below the harness's hook timeout), a held connection with its hold
  (`held_until`), a run grant and a Secret ask at their first command's hold
  (`secrets::ask::holds_for`, three quarters of the chat's harness's command timeout), and a login
  and a fill at purlis's own wait (`loginask::THE_PERSON_IS_WAITED_FOR`). An ACP ask has no
  deadline, and a pool tool's wait is not carried yet. An end already past is no longer Expiring:
  the ask is ordered as any other until #6007 says what a timed-out ask does.
- **"Blocks N chats" counts the open chats that wait on its chat's report**, directly or up a
  dispatched chain (`purlis_core::dispatchwaits`), from each chat's own record of the chat that
  dispatched it with its report still due. Nothing a chat says of itself is read.
- **The reason line** says one fact: "Expires in 40 s" (by the second under a minute, then whole
  minutes rounded up), "With its chat's ask that expires", "Blocks 2 chats", or "Waiting 12 min".
  It is part of the row's description, so a screen reader hears it with the row.
- **Every row names its project**, not only under All projects, beside its workspace and persona.
- **Other projects' asks** are still listed after this project's, read-only, as #5999 left them;
  they are not interleaved by this order.
- Nothing new is stored, so no tier is named (ADR 0069).
