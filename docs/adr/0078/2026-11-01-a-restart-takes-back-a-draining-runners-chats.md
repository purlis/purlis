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

**Ruled by the person on 2026-11-01**: a paired desktop's link may ask its own runner which chats
are running (`programs_of`), so a restarted window takes them back. That one request only. The
rest are *decided in implementation, to be confirmed*.

5. **The window asks its pool's runner now as it asks a draining one.** The refusal this replaces
   kept a restarted window from finding a chat on any runner.
6. **A runner answers a link between hosts only what was opened for the window that link named.**
   Never its own window's, another window's, or anything for a link that named none, and only a
   confinement `take_back` names that window: a chat composed with none named is answered without
   it. A chat now records who opened it, as a browser does.
7. **A program's note stays the window's on its own host** (`note_program`, a start's `note`), and
   a link between hosts still does not count as coming back when its ports are withdrawn: its
   `take_back` gives them back.
8. **Session protocol 1.66 gates it, as the person ruled on 2026-11-01.** A runner of 1.66 or later
   that does not say its programs counts as one that may have run outside the sandbox (D-2050-27),
   unless it was a draining runner removed while it was asked. A runner before 1.66 is not asked and
   not marked, and nothing is taken back from it.
9. **What a restarted window finds and no tab takes back is closed**, on a pool's runners as on this
   machine's host (#2006), on a runner it asked. A runner it did not ask is #4433.

This replaces the `programs_of` line of item 4 of the amendment for #2407 and of item 1 of the
amendment for #2025 in ADR 0068, which kept that request to the window on its own host. The rest of
those items stands.
