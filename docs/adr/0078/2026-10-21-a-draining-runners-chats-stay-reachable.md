# Amended 2026-10-21: a draining runner's chats stay reachable until the last one ends

**Built by #2865** on items 2, 6 and 7 of the ruling of 2026-10-20 for #2749. What follows was
decided in #2865 and is marked *decided in implementation, to be confirmed*.

1. **A draining runner is reached as `<pool>/<runner>`.** `relay_to` names what this machine
   keeps: a pool's name reaches its runner of the desktop's version, and `<pool>/<runner>`
   reaches that pool's runner of that name only while it is kept draining
   (`purlis_core::runners::reachable`). Pool and runner names hold no `/`, so the two never meet,
   and `relay_to` takes the same one string as before: no protocol change. Once the runner is
   removed, the name reaches nothing. *Decided in implementation, to be confirmed* (D-2865-1).
2. **A link to a draining runner starts no new chat there.** The desktop's host refuses
   `start_program` on a link it relays to a draining runner, `not_allowed`, in words that say new
   chats start on the pool's runner; everything else on the link reaches the runner, so its chats
   are listed, answered, watched and ended. *Decided in implementation, to be confirmed*
   (D-2865-2).
3. **The Pools group says what a draining runner waits on.** Each look counts the chats still
   running on each draining runner, and the pool's row says how many, and that ending them lets
   purlis remove the older runner; so an idle chat holds a drain only while the person keeps it.
   *Decided in implementation, to be confirmed* (D-2865-3).
4. **The window's own link to a draining runner comes with the window's routing of a project's
   link to its pool**, which no window sends yet; until then a draining runner's chats are
   reachable through any link that names it so. *Decided in implementation, to be confirmed*
   (D-2865-4).
