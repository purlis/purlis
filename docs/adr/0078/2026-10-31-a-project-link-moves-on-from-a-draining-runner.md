# Amended 2026-10-31: a project's link moves on from a draining runner, and its chats stay

**Built by #4248**, on item 4 of the amendment of 2026-10-21. What follows is marked *decided in
implementation, to be confirmed*.

1. **A new chat follows the pool, an existing chat stays.** As the window asks for a project's link
   to a pool and the runner that link reached drains now, it keeps that link for the chats started
   over it and dials the pool's name, which reaches the pool's runner now. Every command about a
   chat or a browser goes over the link it was started on. The desktop's host still refuses a start
   on a link to a draining runner: that refusal is the rule, and the move is what keeps a person
   from meeting it. *Decided in implementation, to be confirmed* (D-4248-1).
2. **A window's name for a link is held per runner.** While a client link holds a name on a runner,
   no other link takes it there; another runner of the pool may be reached under the same name. A
   name takes back only what that runner started for it, so the rule still gives one link per name
   per runner, and a window that restarts takes back its chats on the pool's runner under the name
   it always kept. *Decided in implementation, to be confirmed* (D-4248-2).
3. **A window that restarts during a drain still reaches only the pool's runner**, as before this
   amendment. Reaching a draining runner's chats across a restart stays open (#4271).
