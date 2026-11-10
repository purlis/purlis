# Amended for #2977, a runner says the skills a chat lacks there, 2026-11-10

A chat on this machine checks the skills its persona names against what its harness has here,
and its tab's Notice says the ones it started without (SK-5, #2768; spec decision 41, never a
silent drop). A chat a runner composes was not checked at all (SK-5d, #2977): the window read
nothing of the runner's harness, and the spec says a chat on a runner, in Docker or on k8s gets
its set "through the same adapter, built from the same plane files". This note says how the
session protocol carries the runner's check. The amendments above are left as accepted, and this
note adds to them. The choices in items 2 and 3 are *decided in implementation, to be confirmed*.

1. **Session protocol 1.81, a minor**, adding a field to an answer, so no `KNOWS_*` gate. `program_started` may carry
   `skills_not_there: {missing?, no_built_in?}`: the harness skills the chat's persona names that
   its harness does not have on the host's machine, and the built-in skills the host's purlis has
   none of, each as the persona's `skills:` list writes it. Names alone, each list at most
   `MOST_ARMED_SKILLS`, each name bounded as an armed skill's is; a window takes only what fits.
   It is said only for a start the host composed, only to a link that may launch, and only where
   some name is missing. A host before 1.81 says none, and its chat is shown none, as before.
2. **The runner checks, and says the result; the window does not recompute it.** The host's
   composition reads the chat's skills as it arms it, and asks `hoststart::skills_not_here` of
   them: the same call the window makes for a chat of its own, which lists the harness's skills
   by its adapter's `skills_at` on the runner's machine and honours the Skill studio's flag the
   desktop sent with the start. The alternative, the runner sending every skill its harness has
   and the window comparing, sends the whole list on every start and compares against the
   window's plane rather than the runner's checkout of it. *(D-2977-1.)*
3. **Missing and built-in names alone.** A plane skill that could not be loaded, and one the
   repo leaves out, are readings of plane and repo files, not of the runner's harness, and are
   not carried by 1.81. *(D-2977-2.)*
4. **The Notice names the runner** as `<pool>/<runner>` where the window's link to it names the
   runner (1.48), its pool otherwise, or a chat's own cluster runner as the window keeps it. The
   word is kept in memory with the chat, as a local chat's is; a chat reattached is not checked
   again.
