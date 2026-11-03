# Amended 2026-11-03: a refused move is not asked again by the same chat

**Built by #4862**, on point 4 of the amendment of 2026-11-02 (the person's ruling that a chat's
dispatch that moves work needs their Ask). What follows is *decided in implementation, to be
confirmed* (the D-lines of the pull request that built it).

1. **The person's Refuse of a move holds for that chat and place.** Once the person refuses a
   chat's dispatch that names a place other than where its target's levels put it, that chat's
   later dispatches to the same place (the same pool, or this machine) start nothing and are not
   held for the person: the chat is told, in words, that the person refused it before. Other
   places, and other chats, still ask.
2. **It lasts while the chat runs.** It is kept in memory with the held asks, follows the chat
   when it is started again under a new number, and goes when the chat closes or the app quits.
   A move the levels no longer make (the person set the workspace to run there) is no move, so it
   starts as any dispatch does. Nothing else lifts it: there is no control for it.
3. **The chat's other asks to that place go with the refusal.** Asks of the same chat to the same
   place still waiting when the person refuses one are refused with it, and the chat is told of
   each, so the person isn't left to answer them one by one. Its asks to other places still wait.
4. **A refusal is never a grant.** What is kept only refuses: it starts nothing and stands in for
   no Approve, so point 4 of 2026-11-02 holds as it was. An Approve keeps nothing, and a held ask
   of a chat on a pool's runner (2026-10-30-07) is held for another reason, so its refusal is
   not kept as a refusal of a place.
5. **Both sides are told before.** The chat's held answer says that a refusal holds for that
   place, and the ask's tab tells the person that, if they refuse, the chat can't ask them again
   to run its work there while it's open.
