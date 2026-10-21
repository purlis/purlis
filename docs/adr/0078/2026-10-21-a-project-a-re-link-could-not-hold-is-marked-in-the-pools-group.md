# Amended 2026-10-21: a project a re-link could not hold is marked in the Pools group

**Built by #2846** on item 3 of the amendment of 2026-10-20 for #2645, which left the mark until
the window had a screen for its runners. What follows was decided in #2846 and is marked
*decided in implementation, to be confirmed*.

1. **The window keeps each project the runner would not hold again**, from the runner's refusal
   of its hold after `not_held` until a hold of it is taken, by this window's run only. Only a
   refusal of the hold is kept; what purlis itself could not do is said where it happened. The
   runner's words are kept as plain text: every control and direction mark dropped, each run of
   white space one space, at most 300 characters. The window is told each time they change.
   *Decided in implementation, to be confirmed* (D-2846-1).
2. **The Pools group draws a standing Notice for each, on the row of the pool its chats run
   on**, in the runner's words, set apart (`<bdi>`) so they cannot turn the sentence around
   them. It says that no chat starts or answers there, and that purlis asks again the next time
   a chat starts or is used in the project. Its way out is Dismiss, which puts it away until
   the runner says something else of the project. *Decided in implementation, to be confirmed*
   (D-2846-2).
3. **Which pool is read from this machine's `chatsRunOn`**, the pools the project's workspaces
   run their chats on: the mark is drawn on the first of them the group lists. The window does
   not yet say which pool a project's link is relayed to, so this is the pool its workspaces
   name, not the one a link proves; a project whose pool this machine cannot name is marked
   under the list. *Decided in implementation, to be confirmed* (D-2846-3).
4. **The mark goes once the project runs its chats on no pool here**: forgotten, its pool
   removed, or none of its workspaces running its chats on a pool any more. Each read of the
   marks checks again, and forgetting a project or removing a pool clears them at once.
   *Decided in implementation, to be confirmed* (D-2846-4).
