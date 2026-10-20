# Amended 2026-10-20: the window is told of a project a re-link could not hold, and holds it again

**Built by #2645** on item 2 of the amendment of 2026-10-18 for #2535, which left the window
counting as held a project that a renewed or restored link to the runner could not hold. What
follows was decided in #2645 and is marked *decided in implementation, to be confirmed*.

1. **The desktop's relay tells the window, in the runner's words.** Once the last pass of a
   carry-over could not hold a project the window holds, the relay pushes the window
   `not_held`, with the project and the runner's own words for the refusal (session protocol
   1.30), one for each project, as the carry-over ends. It is the relay's own word: a `not_held`
   the runner pushes is never handed on, as `link_down` and `link_back` are not. Only a link
   that holds the project is told, so only the window's. *Decided in implementation, to be
   confirmed* (D-2645-1, D-2645-2).
2. **The window holds the project again.** On `not_held` the window stops counting the project
   as held on that link and holds it again over it at once, with what a first hold does: it
   hears the project's hook lines, says the numbers already dealt and takes the kept board, so
   its chats and their hooks carry on. A project the window has held since on another link is
   left as it is. *Decided in implementation, to be confirmed* (D-2645-3).
3. **Where the runner refuses again, the project is not held there, and says why.** The window
   keeps no hold it was refused. Every command that needs the hold, such as a chat's start,
   tries it again and is refused in words that say the project is not held where its chats run,
   followed by the runner's own. Nothing is sent to the runner in its place. *Decided in
   implementation, to be confirmed* (D-2645-4).
