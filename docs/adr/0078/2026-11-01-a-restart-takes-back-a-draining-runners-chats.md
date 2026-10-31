# Amended 2026-11-01: a window's restart takes back a draining runner's chats

**Built by #4271**, on item 3 of the amendment of 2026-10-31. What follows is marked *decided in
implementation, to be confirmed*.

1. **A window that restarts asks each draining runner of the pool too.** For a chat it left on a
   pool and that the pool's runner does not have, it asks each draining runner that runs and whose
   drain has not begun, over `<pool>/<runner>`. What it finds is held on a link of its own, as a
   chat that stayed is. New chats still start only on the pool's runner.
2. **That link goes under the pool's one name.** A name takes back only what that runner started
   for it, and each of the pool's runners started under this name.
3. **A suspended draining runner is not asked.** A link would wake it, and it runs no chat.
4. **A runner whose drain began, with only its home left, is removed without being asked** (#4298).
   On a row with no engine kept, its home listed by this Docker is what says this is its engine.
   Docker listing neither container nor home leaves the row as it is, so what is on its engine is
   not orphaned.
