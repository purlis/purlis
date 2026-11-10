# Amended for #6078, a runner chat is told all it lacks there, and again once taken back, 2026-11-10

The amendment for #2977 of the same day has a runner say the skills a chat it composed lacks
there, but only installed and built-in ones (its D-2977-2), only in the start's answer, and names
the runner as the window's link reached it. A runner chat's Notice was then less than a chat's on
this machine on four counts (#6078). This note says how each is closed. The amendments above are
left as accepted, and this note adds to them; it takes the place of D-2977-2. The choices are
*decided in implementation, to be confirmed*.

1. **Session protocol 1.83, a minor** (1.82 is #6030's audit of a terminal's skill change), adding
   fields to an answer, so no `KNOWS_*` gate. `SkillsNotThere` may carry `no_workspace`,
   `not_in_its_workspace` and `beside_a_built_in`, names as the persona's or the repo's list
   writes them, and `broken` and `by_a_repo`, each a `SkillNotLoaded {name, level, local?, why}`:
   the level as an armed skill's word, and why as the runner's one sentence. A host before 1.83
   says 1.81's two lists alone, and its chat is shown those, as before.
2. **The runner says all it read, and the window recomputes none of it** *(D-6078-1, replacing
   D-2977-2)*. A skill that could not be loaded, and one the repo leaves out, are readings of the
   runner's own copy of the plane and of the clone the chat works in there, which is what that
   chat runs with; the window's copy may differ. So the runner's composition hands its host the
   whole of `hoststart::skills_not_here`, the same call a chat here is checked by.
3. **Bounded, never refused** *(D-6078-2)*. `missing` and `no_built_in` stay at most
   `MOST_ARMED_SKILLS`; each 1.83 list holds at most `MOST_SKILLS_NOT_LOADED` (64), each name and
   level bounded as an armed skill's. A why loses its control characters and is cut at
   `MOST_SKILL_WHY` bytes (512) at a character's edge, rather than the skill dropped, since the
   skill is the news. A window drops a skill whose level it does not know, as it does an armed one.
4. **`programs` answers it too, kept in memory with the chat** *(D-6078-3)*, as 1.77's
   `armed_skills` is: the runner's host keeps what the composition said beside the chat, and a
   window that restarts and reattaches a runner chat shows its Notice again, naming the runner as
   at its start. Nothing is written to the host's record, so a runner host that restarts says
   none, as it says no armed skills. A chat this window composed is still not checked again at
   its reattach.
5. **The runner is named as the window names it elsewhere** *(D-6078-4)*. `<pool>/<runner>` where
   the link names a runner of a Docker or local pool, as Devices says *Runner builds-1 of pool
   builds*. The pool alone where the link names none, and for a cluster pool's runner, which the
   window names nowhere but by its pool: the pool's shared runner is reached as `<pool>/<pool>`,
   and a chat's own runner as `<pool>/fresh-<id>`, an id made from the chat's (#3818), which no
   other part of the window shows.
